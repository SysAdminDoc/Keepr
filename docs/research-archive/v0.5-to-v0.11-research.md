# Project Research and Feature Plan — v0.5 cycle

> Companion to the original [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md) (which captured the v0.1 → v0.2 transition). This pass audits the v0.2 → v0.4 work and re-prioritises what's next now that all the original P0/P1 items have shipped. Research date: 2026-05-26 · HEAD: `5858b7d` (tag `v0.4.0`).

## Executive Summary

Keepr is a Tauri 2 + React + Rust + SQLite offline Google Keep clone that has matured rapidly: in a single autonomous loop it went from a v0.1.0 scaffold to a v0.4.0 release covering trust hardening (v0.2), Keep parity (v0.3), and multimodal (v0.4) work — with 57 automated tests, a 4-table schema at v3, optimistic store updates, drag-reorder, multi-select + bulk actions, a system-tray icon with global hotkey, native Windows toast reminders, image attachments via a custom `keepr-resource://` protocol, inline `#hashtag` labels, and a Markdown-vault + Google-Takeout I/O pair. **The product is feature-rich and the original P0 risks are closed.** The next pass should be a focused **stability + observability cycle** before any new bets: a fresh code audit found 127 follow-on issues — four are P0 (reminder loss on toast failure, drag-reorder corrupting cross-section ordering, empty-note + reminder orphan, no single-instance guard), every new Rust command (`reorder_notes`, `duplicate_note`, attachments, reminders, `export_vault`, `import_takeout`) ships with zero test coverage, the bundle has grown 65 % without code-splitting, and the docs trail the code by a full feature wave. The right v0.5 release is "**Polish & Reliability**" — close every P0/P1 from this audit, raise test coverage from 49 vitest + 8 cargo to >100 / >25, and ship a thumbnail pipeline so 5-image notes stop blowing up RAM. Bigger bets (App Lock + Vault, Drawings, OCR, Version History) all wait for v0.6+.

**Top 10 opportunities, priority order:**

1. **P0 — Fix the reminder scheduler's lost-toast bug** (`commands.rs:1100-1106`). Reminders are marked `fired_at` *before* `notification.show()` runs, so any permission/COM/sleep-resume failure silently loses the reminder forever. Fix by deferring the UPDATE until the toast succeeds; on failure, leave `fired_at` NULL so the next 30 s sweep retries.
2. **P0 — Fix `reorder_notes` cross-section corruption** (`commands.rs:1318-1336` + `NoteGrid.tsx:73-93`). A drag in any section writes `position` for the dragged ids only, leaving every untouched note with stale (likely zero) positions — so Custom-sort ordering becomes incoherent across full grid loads. Either renumber the entire section atomically (`WITH RECURSIVE … OVER section`) or refuse drag in non-Notes sections (and backfill positions on first Custom use — see #6).
3. **P0 — Fix the empty-note + reminder orphan** (`NoteEditor.tsx:320-325` + `563-585`). `setReminderForNote` auto-saves a blank note via `ensureExistingId`; close() then calls `deleteNotePermanent` because the note is empty, which cascades the DB reminder but leaves the renderer-side `reminders` entry dangling and the user wondering where their reminder went. Either suppress the empty-delete when a reminder or attachment was added, or warn before deleting.
4. **P0 — Add `tauri-plugin-single-instance`**. Double-clicking `keepr.exe` in portable mode opens two windows fighting over the same SQLite WAL; their independent `AppState` mutexes can't help.
5. **P1 — Test coverage for every new Rust command** (`reorder_notes`, `duplicate_note`, `add_image_attachment`, `delete_attachment`, `set_reminder`, `clear_reminder`, `take_due_reminders`, `export_vault`, `import_takeout`). 0 tests today; aim for ≥ 20 covering happy + failure paths.
6. **P1 — Backfill `notes.position` in a v4 migration**. Users upgrading from v1/v2 land in Custom sort with `position = 0` everywhere and see a tie-break-by-`updated_at` order that feels random. Run a one-shot `UPDATE` seeding position by current sort.
7. **P1 — Image thumbnails**. `AttachmentGrid` decodes the original at card-preview size — a five-image note × eight visible cards = 40 originals in RAM. Generate a 480-px thumbnail in `add_image_attachment` (Rust `image` crate); serve via `keepr-resource://<id>?thumb=1` (already supported by the protocol handler architecture). Same fix for editor previews.
8. **P1 — Code-split the four secondary modals**. SettingsModal, LabelsManager, HelpOverlay, ReminderPicker aren't on the first-paint path — `React.lazy` them and the 313 KB bundle drops ~40 KB on initial load.
9. **P1 — Reminders v2 surface**: dedicated **Reminders sidebar section** + in-app toast on fire (not just OS-level — easy to miss with Focus Assist on) + reminder recurrence (RRULE). Currently no way to see all upcoming reminders.
10. **P1 — Docs catch-up**: README "Features" still reflects v0.1, SECURITY.md predates the tray + scheduler + custom protocol, RESEARCH_FEATURE_PLAN.md is stale. Plus update CONTRIBUTING's project-layout (`src/hooks/`, `src/lib/`, capabilities).

---

## Evidence Reviewed

### Local files and directories inspected
- All TypeScript under [`src/`](src/): 7 components, 6 hooks, 4 lib helpers, 6 test files, the store, the api, the types.
- All Rust under [`src-tauri/src/`](src-tauri/src/): `main.rs` (5 lines), `lib.rs` (325 lines), `db.rs` (229 lines), `commands.rs` (1853 lines).
- Config: [`package.json`](package.json), [`tsconfig.json`](tsconfig.json), [`tailwind.config.js`](tailwind.config.js), [`vite.config.ts`](vite.config.ts), [`vitest.config.ts`](vitest.config.ts), [`index.html`](index.html), [`src-tauri/Cargo.toml`](src-tauri/Cargo.toml), [`src-tauri/tauri.conf.json`](src-tauri/tauri.conf.json), [`src-tauri/capabilities/default.json`](src-tauri/capabilities/default.json), [`src/keep-palette.js`](src/keep-palette.js).
- Docs: [`README.md`](README.md), [`ROADMAP.md`](ROADMAP.md), [`CHANGELOG.md`](CHANGELOG.md), [`CONTRIBUTING.md`](CONTRIBUTING.md), [`SECURITY.md`](SECURITY.md), [`RESEARCH_FEATURE_PLAN.md`](RESEARCH_FEATURE_PLAN.md), [`.github/workflows/ci.yml`](.github/workflows/ci.yml).
- Audit-notes carry-forward: [`.audit-notes-code.md`](.audit-notes-code.md) (v0.1, mostly closed), [`.audit-notes-keep.md`](.audit-notes-keep.md), [`.audit-notes-competitors.md`](.audit-notes-competitors.md), and the new [`.audit-notes-code-v0.4.md`](.audit-notes-code-v0.4.md) (127 findings, generated this pass).

### Git history range reviewed
- 35 commits between `d67484e` (initial) and `5858b7d` (v0.4.0 release), spanning all of v0.2/v0.3/v0.4. Used `git log --oneline -35` and `git show --stat` on every commit since v0.3 to catalog what landed.

### Build / test / docs / release artifacts inspected
- `npm test` → 49 vitest passing across 7 test files (`filterNotes`, `colors`, `setKind`, `hashtags`, `sortNotes`, `trashRetention`, `autoBackup`).
- `cargo test --lib` → 8 Rust unit tests passing (5 migration tests in `db.rs`, 3 zip-validator tests in `commands.rs`).
- `npm run build` → clean; bundle is 313 KB JS / 94 KB gzip, 24 KB CSS / 5 KB gzip, 1.42 KB HTML.
- `cargo check` → clean. **`cargo test` for the new commands**: none exist (`reorder_notes`, `duplicate_note`, `add_image_attachment`, `delete_attachment`, `set_reminder`, `clear_reminder`, `take_due_reminders`, `export_vault`, `import_takeout` — all zero coverage).
- `.github/workflows/ci.yml` runs both halves on every PR (windows-latest, Rust+Node 20).
- Three tagged releases on GitHub: `v0.2.0`, `v0.3.0`, `v0.4.0`. No release artifacts (`tauri build` has never run in this project — bundle size and Windows install behavior **unverified**).

### External sources reviewed
- The original `.audit-notes-keep.md` and `.audit-notes-competitors.md` are still substantially current (Google Keep hasn't shipped major surface-area changes in the months since they were written; Joplin/Notesnook/Memos likewise stable). Reused those bodies of evidence here rather than re-fetching.
- Tauri 2 plugin landscape spot-checked for v0.5+ candidates: [`tauri-plugin-single-instance`](https://v2.tauri.app/plugin/single-instance/) (need it, see #4), [`tauri-plugin-updater`](https://v2.tauri.app/plugin/updater/) (need it eventually), [`tauri-plugin-store`](https://v2.tauri.app/plugin/store/) (we use localStorage; could replace), [`tauri-plugin-log`](https://v2.tauri.app/plugin/log/) (no logging today).
- Native Win 11 notification action APIs ([`Windows.UI.Notifications` Toast API](https://learn.microsoft.com/en-us/uwp/api/windows.ui.notifications)) — confirmed `tauri-plugin-notification` supports basic toasts via `tauri-winrt-notification` (verified in Cargo.lock), action buttons (Snooze, Mark Done) require deeper integration.
- [SQLite FTS5 docs](https://www.sqlite.org/fts5.html) — re-validated EI-18 deferral; client-side filter remains <1 ms at expected scale.

### Areas not verified
- **Actual release binary size**: `tauri build` has never been run. The CHANGELOG's "Bundle: 313 kB JS" figure is the frontend asset only; the Rust binary + WebView2 stubs are unknown. Verify by building once.
- **Native Windows toast permission behavior on first launch**: `tauri-plugin-notification` may or may not prompt depending on app provisioning. Needs live validation on a fresh Win 11 box.
- **Tray icon visibility on Win 11 24H2 with "show all icons" off** — the default Windows tray hides new icons; users may not see Keepr's icon without a manual unpin. Verify.
- **Two-instance behavior** specifically. Audit asserts it's broken but the SQLite-WAL contention has been verified to be safe in practice; the actual failure mode (which window wins on close, where notifications fire from, etc.) needs a manual test.
- **CI on Linux/macOS** — workflow only runs `windows-latest`. Crossbuild story is unknown.

---

## Current Product Map

### Core workflows
1. **Capture** — `c`, `l`, `Ctrl+Alt+N` (global), tray "New note", or "Take a note…" bar → editor modal.
2. **Edit** — click card or `Enter` on focused card → editor modal with full toolbar (pin, color, list-toggle, label, image, reminder, copy, archive, trash, close).
3. **Organize** — labels (sidebar list + chip filter + inline `#hashtag`), 12 colors, pin, archive, trash with `7-day` auto-purge.
4. **Find** — debounced text search (title + body + checklist), filter chips (type / color / label / pinned).
5. **Triage at scale** — multi-select (hover checkmark, `x`, `Ctrl+A`) → BulkActionBar (pin / color / labels tri-state / archive / trash / restore / delete forever).
6. **Reorder** — Custom sort mode unlocks drag-reorder of cards (`@dnd-kit`); checklist items always draggable via grip handle.
7. **View** — Grid (masonry, default) ↔ List view (`Ctrl+G`), light / dark / system theme, sidebar collapse, sort modes (Modified / Created / Title / Custom).
8. **Remind** — Bell button in editor → ReminderPicker (Later today / Tomorrow morning / Next Monday / custom datetime) → native Windows toast at fire time, badge on card until fired.
9. **Backup / restore** — manual ZIP export + import (with `.prev` rollback, zip-slip + zip-bomb defense, WAL checkpoint), auto-backup schedule (Off / Daily / Weekly) into a configured folder.
10. **Migrate** — Markdown vault export (one `.md` per note + YAML frontmatter + `_resources/`), Google Takeout import (`Keep/*.json` schema → labels + colors + checklists + attachments).
11. **Survive** — Portable mode (`portable.flag` next to EXE) makes Keepr write to its own folder so a USB stick carries everything.

### Existing features (full inventory in next section)
20 user-facing features across capture/edit, organize, find, view, multimodal, system integration, and I/O.

### User personas (carried over from `RESEARCH_FEATURE_PLAN.md`, refined)
1. **Offline-first private user** — primary. Now well-served.
2. **Migrating Keep user** — well-served by NF-08 Takeout import. **Caveat**: import drops original timestamps + reminders, which a migrating user will absolutely notice (audit items #16, #17).
3. **Power keyboard user** — well-served by NF-03 shortcuts + help overlay.
4. **Sensitive-info user** — still unaddressed. App-lock (NF-10) is the largest remaining gap.
5. **Sticky-note user** — well-served by NF-06 tray + hotkey.
6. **Image-note user** (new) — partly served. NF-01 lands the model but the editor's full-res rendering will choke at any scale (audit #30). Thumbnail pipeline is the unlock.
7. **Recurring-reminder user** (new) — unserved. NF-02 ships single-shot only.
8. **Calendar-integrated user** (new) — unserved. ICS export of reminders would be high-leverage given Google deprecating Keep reminders into Tasks.

### Platforms and distribution channels
- **Windows desktop** — sole supported platform. Bundle targets are `nsis + msi`; portable EXE works via `portable.flag` but never released.
- **macOS / Linux** — Tauri supports them, no icons or signing, no CI matrix.
- **Mobile** — out of scope.
- **Distribution** — public GitHub (`SysAdminDoc/Keepr`) with three tags; no GitHub Release assets yet (no `gh release create`).

### Important integrations / permissions / storage / data flows
- **Storage**: SQLite at `app_data_dir()/keepr.db` (= `%APPDATA%\com.sysadmindoc.keepr\keepr.db`) or beside the EXE in portable mode. Schema v3 with `notes`, `checklist_items`, `labels`, `note_labels`, `attachments`, `reminders` + `idx_notes_state`, `idx_checklist_note`, `idx_note_labels_label`, `idx_attachments_note`, `idx_reminders_pending`.
- **Custom protocol**: `keepr-resource://<id>.<ext>` resolves to `<data_dir>/resources/<id>.<ext>`; CSP allows it as `img-src`.
- **Tauri plugins**: `dialog`, `notification`, `global-shortcut`. Capabilities: `core:default`, `dialog:{default,open,save}`, `global-shortcut:{is-registered,register,unregister}`, `notification:{default,is-permission-granted,request-permission,notify}`. **Several granted but unused** — audit items #65, #66.
- **Background work**: reminder scheduler thread polls every 30 s, auto-backup renderer-side tick polls every 30 min, trash-retention sweep ticks hourly.
- **Network**: still zero outbound requests. Identity preserved.

---

## Feature Inventory

For each: name · user value · entry point · main code · current maturity · tests/docs · improvement opportunity.

| # | Feature | User value | Entry | Code | Maturity | Tests/docs | Top improvement |
|---|---|---|---|---|---|---|---|
| F-01 | Masonry grid + pinned/others sections | Visual Keep parity | Always-on | [`NoteGrid.tsx`](src/components/NoteGrid.tsx), [`App.tsx`](src/App.tsx) | Complete | None | Replace `react-masonry-css` (unmaintained) when NF-22 backgrounds land |
| F-02 | Text + checklist notes (with lossless setKind) | Core | `NewNoteBar` | [`NoteEditor.tsx`](src/components/NoteEditor.tsx) | Complete | `setKind.test.ts` 5 cases | Ctrl+Shift+8 to toggle checkboxes; see audit #94 (Shift+↑↓ keyboard reorder) |
| F-03 | 12 Keep colors (light+dark, SSoT) | Visual parity | Color picker | [`keep-palette.js`](src/keep-palette.js), [`ColorPicker.tsx`](src/components/ColorPicker.tsx) | Complete | `colors.test.ts` 4 cases | None pending |
| F-04 | Pin / unpin | Core | Card hover, editor, `f` shortcut | [`NoteCard.tsx`](src/components/NoteCard.tsx), [`commands.rs`](src-tauri/src/commands.rs)`:set_pinned` | Complete | None | Pin button still missing `aria-pressed` (audit #84) |
| F-05 | Archive / restore | Core | Card hover, editor, `e` shortcut, sidebar | [`commands.rs:set_archived`](src-tauri/src/commands.rs) | Complete | None | None pending |
| F-06 | Trash with restore + auto-purge | Core | Card hover, `#` shortcut, sidebar | [`commands.rs:set_trashed`](src-tauri/src/commands.rs), [`trashRetention.ts`](src/lib/trashRetention.ts) | Complete | `trashRetention.test.ts` 7 cases | Days-left badge needs hourly re-render (audit #25) |
| F-07 | Empty Trash | Bulk clean | Trash header button | [`App.tsx`](src/App.tsx) | Complete | None | None pending |
| F-08 | Labels (CRUD + chip filter + sidebar) | Organization | "Edit labels" + sidebar + editor menu | [`LabelsManager.tsx`](src/components/LabelsManager.tsx) | Complete | None | Per-label note counts in sidebar (still missing — was a Keep-win in the audit) |
| F-09 | Inline `#hashtag` labels (NF-07) | Power-user organization | Type in body | [`hashtags.ts`](src/lib/hashtags.ts), [`NoteEditor.tsx`](src/components/NoteEditor.tsx#L286-L301) | Complete | `hashtags.test.ts` 9 cases | Don't re-create labels for hashtags whose text was deleted (audit #12). Highlight in title too (audit #124) |
| F-10 | Search (debounced, title+body+checklist) | Discovery | TopBar search, `/` | [`TopBar.tsx`](src/components/TopBar.tsx), [`filterNotes.ts`](src/lib/filterNotes.ts) | Complete | `filterNotes.test.ts` 7 cases | FTS5 still deferred (no perf trigger) |
| F-11 | Filter chips (NF-09) | Find-by-property | Top of grid | [`FilterChips.tsx`](src/components/FilterChips.tsx) | Complete | None (covered indirectly) | Hide "Pinned" in Trash; auto-clear on section switch (audit #87, #123); drop `_UnusedX = X` line (audit #49) |
| F-12 | Multi-select + bulk actions (NF-04) | Triage at scale | Hover checkmark, `x`, `Ctrl+A` | [`BulkActionBar.tsx`](src/components/BulkActionBar.tsx), [`store.ts`](src/store.ts)`.selectedIds` | Complete | None | Bulk "Make a copy" missing; partial-failure aggregation untested (audit #41) |
| F-13 | Drag-reorder notes (NF-05a, NF-05b) | Custom ordering | Custom sort + drag | [`NoteGrid.tsx`](src/components/NoteGrid.tsx), [`commands.rs:reorder_notes`](src-tauri/src/commands.rs) | **Partial — P0 #2** | None | Cross-section corruption + position backfill (audit #1, #6, #7, #78) |
| F-14 | Drag-reorder checklist items (NF-05b) | Item ordering | GripVertical handle | [`NoteEditor.tsx:ChecklistRow`](src/components/NoteEditor.tsx) | Complete | None | Keyboard reorder (Shift+↑↓) missing (audit #94); drag-handle whitespace on checked rows (audit #19) |
| F-15 | Sort modes (Modified/Created/Title/Custom) | Choose ordering | TopBar Sort menu | [`TopBar.tsx`](src/components/TopBar.tsx), [`store.ts:sortNotes`](src/store.ts) | Complete | `sortNotes.test.ts` 6 cases | Custom mode lacks visual cue ("drag to reorder"); rename `nextWeek` → `nextMonday` (audit #28) |
| F-16 | Light / Dark / System theme (NF-16) | Comfort | TopBar toggle + Settings radio | [`store.ts`](src/store.ts), [`SettingsModal.tsx`](src/components/SettingsModal.tsx), [`index.html`](index.html) boot script | Complete | None | System listener leak in vitest HMR (audit #24) |
| F-17 | Grid / List view + Ctrl+G (NF-23) | Long content | TopBar toggle, `Ctrl+G` | [`NoteGrid.tsx`](src/components/NoteGrid.tsx), [`useGlobalHotkey.ts`](src/hooks/useGlobalHotkey.ts) | Complete | None | None pending |
| F-18 | Keyboard shortcuts + help (NF-03) | Power-user | `?` | [`useKeepShortcuts.ts`](src/hooks/useKeepShortcuts.ts), [`HelpOverlay.tsx`](src/components/HelpOverlay.tsx) | Complete | None | Esc clears selection even while modal open (audit #23); `useKeepShortcuts.ts:106` non-null assertion (audit #56) |
| F-19 | Tray icon + global hotkey (NF-06) | Always-resident | Tray click, `Ctrl+Alt+N` | [`lib.rs`](src-tauri/src/lib.rs) | **Partial — P0 #4** | None | Single-instance guard (#77); Quit-confirmation; remap hotkey UI (#9) |
| F-20 | Manual ZIP backup + auto-backup schedule (NF-15) | Data safety | Settings + 30-min tick | [`SettingsModal.tsx`](src/components/SettingsModal.tsx), [`autoBackup.ts`](src/lib/autoBackup.ts), [`commands.rs:export_zip`](src-tauri/src/commands.rs) | Complete | `autoBackup.ts` 11 cases | `export_zip` collects each file into RAM (audit #71); no caps on export side (#70) |
| F-21 | Trash retention (NF-17) + days-left badge | Configurable purge | Settings + hourly sweep | [`trashRetention.ts`](src/lib/trashRetention.ts) | Complete | 7 vitest cases | Badge refresh after midnight (#25) |
| F-22 | Image attachments (NF-01) | Multimodal | Editor Image button | [`AttachmentGrid.tsx`](src/components/AttachmentGrid.tsx), [`commands.rs:add_image_attachment`](src-tauri/src/commands.rs) | Complete (no thumbnails) | None | Thumbnails (#30); paste from clipboard; drag-drop onto editor; EXIF strip |
| F-23 | Reminders v1 (NF-02) | Time-based recall | Editor Bell button | [`ReminderPicker.tsx`](src/components/ReminderPicker.tsx), [`commands.rs:take_due_reminders`](src-tauri/src/commands.rs), scheduler in [`lib.rs`](src-tauri/src/lib.rs) | **Partial — P0 #1, P0 #3** | None | Lost-toast fix, in-app fire toast, dedicated sidebar section, RRULE, snooze, ICS export |
| F-24 | Markdown vault export + Takeout import (NF-08) | Migration / longevity | Settings | [`commands.rs:export_vault`](src-tauri/src/commands.rs), `import_takeout` | Complete | None | Overwrite collision detection (#14, #15); Takeout timestamps + reminders preservation (#16, #17) |
| F-25 | Make a copy (NF-18) | Templates | Editor Copy button | [`commands.rs:duplicate_note`](src-tauri/src/commands.rs) | Partial | None | Re-open the copy after duplicate; document non-cloning of attachments visibly (#26) |
| F-26 | Move-checked-to-bottom + collapsible group (NF-20) | Checklist ergonomics | Settings toggle + per-list render | [`NoteEditor.tsx:ChecklistRow`](src/components/NoteEditor.tsx) | Partial (no FLIP) | None | FLIP animation polish |
| F-27 | Portable mode (EI-11) | USB-stick portability | `portable.flag` next to EXE | [`lib.rs:resolve_data_dir`](src-tauri/src/lib.rs) | Complete | None | No release build yet to verify in practice |
| F-28 | Confirm dialog + focus trap + Escape (EI-13/14/20) | A11y / trust | All destructive flows | [`ConfirmDialog.tsx`](src/components/ConfirmDialog.tsx) + hooks | Complete | None | A few new IconBtns missing `pressed` (#84) |
| F-29 | Optimistic store reducers (EI-24) | Snappy UI | Every mutation | [`store.ts`](src/store.ts) | Complete | None | `patchNote` re-sorts on every patch — skip when patch doesn't change sort keys (#33) |
| F-30 | `keepr-resource://` protocol | Embedded assets | `<img>` in cards/editor | [`lib.rs:handle_resource_request`](src-tauri/src/lib.rs) | Complete | None | Add `?thumb=1` variant for thumbnails; tighten `Access-Control-Allow-Origin` (#75) |

---

## Competitive and Ecosystem Research

The original `.audit-notes-competitors.md` covered 12 reference apps in depth. **No material changes since** (verified by spot-checking each project's GitHub `Releases` page — Joplin 3.x, Notesnook 3.x, Memos 0.27+, AppFlowy 0.7+ all shipped routine fixes; no new patterns that change v0.5 priorities). Quick reaffirmation of the four-app shortlist that still drives Keepr's direction:

| App | Still-relevant lesson | What we already adopted |
|---|---|---|
| **Joplin** | `:/resource-id` URI scheme; "local filesystem" sync model | We have `keepr-resource://`; we have manual ZIP + auto-backup-to-folder |
| **Notesnook** | Two-tier App Lock + Private Vault; broad importer matrix | We have Takeout import; **app-lock is the largest unshipped competitive item** (NF-10) |
| **Memos** | Inline `#hashtag` tag input | Shipped NF-07 |
| **Obsidian** | Vault-as-folder + per-file Markdown export for longevity | Shipped NF-08 |

**Two new ecosystem developments worth noting for v0.5+:**

1. **Tauri 2 single-instance plugin** ([docs](https://v2.tauri.app/plugin/single-instance/)) is now first-party and trivial to integrate (~5 lines in `lib.rs`). Audit item #77 → roadmap P0.
2. **`tauri-plugin-updater`** ([docs](https://v2.tauri.app/plugin/updater/)) is stable in Tauri 2. Auto-update is now table stakes for desktop apps; Keepr ships unsigned + uninstalled today so this needs paired with a release-builds story.

Things Keep itself has done since the last audit:
- 9to5Google reported (early 2026) that **Keep's Wear OS app was deprecated** further — confirms that Keep's product direction is shrinking, not growing, reinforcing Keepr's "lighter / private alternative" pitch.
- The Reminders → Tasks migration continues. Keepr keeping reminders embedded in notes (rather than splitting them out) remains the right call.

---

## Highest-Value New Features

(Carry-forward + brand-new items, prioritised for v0.5+.)

### NF-V0.5-A — Reminders v2: recurrence, sidebar section, snooze, in-app toast
- **Problem solved**: Reminders v1 is single-shot, has no central "what's coming up" view, and uses only the easily-missed Windows OS toast.
- **Evidence**: Original [`.audit-notes-keep.md` §2](.audit-notes-keep.md) lists recurrence + sidebar + snooze as Keep-standard. Audit items #88 (in-app toast missing), F-23 row above.
- **Proposed behavior**:
  - **Sidebar Reminders section** filters to notes with `reminders.fired_at IS NULL`, sorted by `fire_at` ASC. Overdue items at the top in a red-tinted strip.
  - **In-app toast** on `keepr://reminder-fired` event (already emitted) — duration 8 s, action button "Snooze 10 min".
  - **Snooze actions**: 10 min / 1 hour / Tomorrow morning / Custom (the Snooze field already exists in the schema — just unused).
  - **RRULE recurrence**: ReminderPicker grows a "Repeat" row (None / Daily / Weekly / Monthly / Custom). Store as RFC 5545 RRULE strings (the column exists in schema v3 — just unused). Scheduler computes next `fire_at` after each successful fire.
- **Implementation areas**: `src-tauri/src/commands.rs` (extend `take_due_reminders` to handle snooze + recurrence), `src/components/ReminderPicker.tsx`, new `src/components/RemindersSection.tsx`, `src/store.ts` (add `Section` variant `{ kind: "reminders" }`).
- **Data/API**: `snooze_reminder(note_id, until)` + `set_reminder(note_id, fire_at, rrule?)`; reminder rows already carry the fields.
- **Risks**: RRULE expansion edge cases (DST, leap days); make sure the next-fire calc is in the scheduler thread, not the renderer.
- **Verification**: Rust integration tests for take_due + snooze + recurrence math; manual smoke for OS toast.
- **Complexity**: **L**. **Priority**: **P1**.

### NF-V0.5-B — Image thumbnails (Rust-side, served via `?thumb=1`)
- **Problem solved**: Audit #30 — full-res JPEGs decoded at 250 px slot eats RAM and paint time. Five images × eight cards = 40 originals.
- **Evidence**: F-22 row, audit #30.
- **Proposed behavior**: `add_image_attachment` runs the source through the [`image`](https://crates.io/crates/image) crate at upload time, writes `<id>.<ext>` (original) AND `<id>.thumb.<ext>` (max 480 px on the long edge, JPEG 80 quality). The protocol handler resolves `keepr-resource://<id>.thumb.<ext>` to the thumb; `AttachmentGrid` picks thumb when card-context, original when editor-context or full-image preview.
- **Implementation areas**: `src-tauri/src/commands.rs:add_image_attachment` + new helper, `src/components/AttachmentGrid.tsx`.
- **Data/API**: Persist `width`/`height` on the attachment row (columns already in schema v2 — just NULL today).
- **Risks**: WebP/GIF/SVG edge cases (don't thumb SVG; keep GIF as-is to preserve animation).
- **Verification**: Rust test that a 2000×2000 JPEG yields a 480×480 thumb file; manual on a 5-image note.
- **Complexity**: **M**. **Priority**: **P1**.

### NF-V0.5-C — App Lock + Private Vault (carried from RESEARCH_FEATURE_PLAN.md NF-10)
- **Problem solved**: Stolen unlocked laptop reads every note in plaintext.
- **Evidence**: [Original plan §NF-10](RESEARCH_FEATURE_PLAN.md#nf-10--app-lock--private-vault-notesnook-two-tier-model); audit user persona #4.
- **Proposed behavior**: Notesnook two-tier — App Lock (PIN/Hello, locks entire UI after N min) + Private Vault (separate password, gates per-note "Move to Vault"). Argon2id PHC for PIN, XChaCha20-Poly1305 for vault note title+body. Vault key never persisted.
- **Implementation areas**: New `src-tauri/src/vault.rs`; schema v5 adds `vault: TEXT DEFAULT 'plain'` to `notes` and a `pin_hash` row in a new `app_settings` table; new `LockScreen.tsx`; `tauri-plugin-stronghold` or OS credential store for "remember on this device".
- **Risks**: Key-derivation timing (Argon2id m=64MiB, t=3 is the sweet spot); lost-PIN recovery is intentionally not provided — document loudly.
- **Verification**: Round-trip tests for vault encrypt/decrypt; manual lockout flow.
- **Complexity**: **XL**. **Priority**: **P2**. (Largest competitive gap, but no individual user is currently blocked.)

### NF-V0.5-D — Note version history with diff
- **Problem solved**: Accidental delete or accidental over-write isn't recoverable beyond Trash + 7 days.
- **Evidence**: Carried from RESEARCH_FEATURE_PLAN NF-14; Notesnook + Trilium ship this.
- **Proposed behavior**: Every `update_note` snapshots the prior state into a new `note_snapshots` table; cap at last 20 per note. Editor footer shows "Edited X ago · history…" → modal with timeline + body diff + one-click restore.
- **Implementation areas**: Schema v6 (`note_snapshots`), new `commands::list_snapshots(note_id)` + `restore_snapshot(snapshot_id)`, new `<HistoryDrawer>` in the editor.
- **Risks**: Storage growth (capped); diff UI complexity. Skip per-attachment versioning.
- **Verification**: Integration test snapshot + restore.
- **Complexity**: **M**. **Priority**: **P2**.

### NF-V0.5-E — Drawing notes (vector canvas)
- **Problem solved**: Keep parity for whiteboard/sketch capture.
- **Evidence**: [Original plan §NF-11](RESEARCH_FEATURE_PLAN.md), [`.audit-notes-keep.md` §1](.audit-notes-keep.md).
- **Proposed behavior**: `<canvas>` with Pointer Events + pen pressure; Pen / Marker / Highlighter / Eraser tools. Vector storage as SVG paths + a rasterised PNG thumb. Same protocol handler.
- **Implementation areas**: New `src/components/DrawingCanvas.tsx`; `attachments.kind = 'drawing'` (column already supports it); thumbnail pipeline (NF-V0.5-B prereq).
- **Risks**: Canvas perf on long sessions; mobile/tablet support out of scope for v0.5.
- **Complexity**: **L**. **Priority**: **P3**.

### NF-V0.5-F — Bundled releases + auto-updater
- **Problem solved**: There's no Released binary today — users with `cargo + node` are the only audience. Plus update path doesn't exist.
- **Evidence**: No `gh release` artifacts; bundle size unverified; `tauri-plugin-updater` exists.
- **Proposed behavior**:
  - Set up `tauri-action` in CI to build NSIS + MSI + portable .zip on tag push.
  - Adopt `tauri-plugin-updater` against a GitHub Releases manifest.
  - Resolve code-signing decision: ship unsigned with documented SmartScreen workaround, or pursue a sigstore-free EV cert path later.
- **Implementation areas**: `.github/workflows/release.yml`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json` (`bundle.publisher`), `src-tauri/src/lib.rs` (updater plugin init).
- **Risks**: Signing key management; SmartScreen reputation period for new unsigned EXEs.
- **Verification**: tag a `v0.5.0-rc1`, run workflow, install MSI on a clean Win 11 box.
- **Complexity**: **L**. **Priority**: **P1**.

### NF-V0.5-G — ICS export of reminders
- **Problem solved**: Keep's reminders are being deprecated into Tasks; users want them in their primary calendar.
- **Evidence**: `.audit-notes-keep.md` §2 documents the deprecation. RRULE work in NF-V0.5-A naturally yields ICS-compatible data.
- **Proposed behavior**: Settings → "Export reminders as .ics" → writes a single VEVENT-per-reminder ICS file the user can drag into Google Calendar / Outlook / Apple Calendar.
- **Implementation areas**: New `src-tauri/src/commands.rs::export_ics(dest)`.
- **Complexity**: **S**. **Priority**: **P3**.

### NF-V0.5-H — Per-label note counts in sidebar
- **Problem solved**: Audit-keep called this Keep's #1 complaint; we have it on the list but never shipped.
- **Evidence**: `.audit-notes-keep.md` §11. F-08 row.
- **Proposed behavior**: Sidebar label rows show "Work · 12" with the count of non-trashed notes carrying that label. Updates optimistically on every label toggle.
- **Implementation areas**: `src/components/Sidebar.tsx` (compute counts from `store.notes`).
- **Complexity**: **S**. **Priority**: **P2**.

### NF-V0.5-I — Paste-image-from-clipboard + drag-drop onto editor
- **Problem solved**: Today image upload is a file-picker only. Keep parity expects paste + drop.
- **Evidence**: NF-01 explicitly skipped paste. Audit F-22.
- **Proposed behavior**: `paste` event listener on the editor reads `ClipboardEvent.clipboardData.files[0]` and pipes it through `add_image_attachment`. Drop-zone overlay on the editor accepts file drops.
- **Implementation areas**: `src/components/NoteEditor.tsx`, possibly a small Rust helper to take a Blob via base64 if needed (Tauri's plugin-fs is not enabled — keep the renderer doing the heavy lifting via a base64 → temp file route, OR add a `save_image_blob` Rust command).
- **Complexity**: **M**. **Priority**: **P2**.

### NF-V0.5-J — Logging + diagnostics
- **Problem solved**: When something goes wrong (reminder failed, hotkey didn't register, vault-import partial), users have no place to look.
- **Evidence**: Audit #9 (silent hotkey failure), #20 (silent tray-icon fallback), #60 (silent label-create failure).
- **Proposed behavior**: Adopt `tauri-plugin-log` (or just write a `keepr.log` ourselves) at INFO level by default, with a Settings toggle to enable DEBUG. Surface the log file path in Settings → Open log.
- **Implementation areas**: `src-tauri/src/lib.rs`, `src/components/SettingsModal.tsx`.
- **Complexity**: **S**. **Priority**: **P2**.

### NF-V0.5-K — macOS + Linux build matrix
- **Problem solved**: CI only runs windows-latest, no cross-platform verification.
- **Evidence**: `.github/workflows/ci.yml:24`.
- **Proposed behavior**: Add `macos-latest` and `ubuntu-latest` to the CI matrix. Build for both. Don't ship binaries yet (deferred to NF-V0.5-F), but verify code compiles + tests pass.
- **Implementation areas**: `.github/workflows/ci.yml`.
- **Risks**: Linux WebKitGTK setup is fiddly; tray-icon behavior is OS-specific.
- **Complexity**: **S**. **Priority**: **P3**.

---

## Existing Feature Improvements

Indexed against the carry-forward [`.audit-notes-code-v0.4.md`](.audit-notes-code-v0.4.md) (item numbers in `[brackets]`).

### EI-V0.5-1 [P0, audit #1+#6+#78] — Drag-reorder correctness
- **Current**: `reorder_notes(ids[])` updates only the passed ids' positions. Untouched notes keep stale `position = 0` from the fresh install / unset state. Cross-section drags corrupt active-Notes ordering.
- **Recommended**:
  - Add a v4 migration that backfills `notes.position` with `ROW_NUMBER() OVER (ORDER BY pinned DESC, updated_at DESC)`.
  - Rewrite `reorder_notes` to write `position = section_start + index` (or, simpler, refuse the call when section is not Notes — gate at the renderer level).
- **Touches**: `src-tauri/src/commands.rs:reorder_notes`, `src-tauri/src/db.rs:MIGRATION_V4`, `src/components/NoteGrid.tsx`.
- **Backcompat**: One-shot DB migration; safe.
- **Verify**: New cargo test `reorder_notes_renumbers_section`; manual drag in Notes + check Custom-sort load.
- **Complexity**: **M**.

### EI-V0.5-2 [P0, audit #3] — Reminder scheduler: don't lose toasts on failure
- **Current**: `take_due_reminders` (`commands.rs:1100-1106`) UPDATEs `fired_at` inside the same loop that reads, before `notification.show()` runs at `lib.rs:274-280`. `let _ = ...show()` swallows failures.
- **Recommended**: Refactor to two phases — `peek_due_reminders` (read + return without UPDATE), then per-reminder fire + only-on-success `mark_reminder_fired`. On failure, leave `fired_at` NULL so the next tick retries.
- **Touches**: `src-tauri/src/commands.rs:take_due_reminders`, `src-tauri/src/lib.rs` scheduler loop.
- **Verify**: New Rust test that simulates `show()` failure and asserts the reminder is still pending.
- **Complexity**: **S**.

### EI-V0.5-3 [P0, audit #4] — Empty-note + reminder orphan
- **Current**: `NoteEditor.close()` deletes the auto-saved blank note via `deleteNotePermanent` when no text was added, leaving the renderer's `reminders` entry pointing at a missing note.
- **Recommended**: Either (a) when `isEmptyNow && (noteReminder || attachments.length > 0)`, skip the empty-delete; (b) call `useStore.getState().removeReminder(ex.id)` before the delete; or (c) refuse to allow setting a reminder on a brand-new empty note until the user types something.
- **Touches**: `src/components/NoteEditor.tsx:close`, possibly `src/store.ts`.
- **Verify**: vitest covering the close flow with a reminder set on an empty note.
- **Complexity**: **S**.

### EI-V0.5-4 [P0, audit #77] — Single-instance plugin
- **Current**: Two `keepr.exe` processes can run concurrently in portable mode; their `AppState` mutexes are independent.
- **Recommended**: Add `tauri-plugin-single-instance` ([docs](https://v2.tauri.app/plugin/single-instance/)). Second-launch callback focuses the existing window.
- **Touches**: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`.
- **Verify**: Manual on a portable build.
- **Complexity**: **S**.

### EI-V0.5-5 [P1, audits #5+#23+#123] — Selection + section + filters interactions
- **Current**:
  - Bulk operations show "5 selected" while only 2 cards remain visible during the iteration window (audit #5).
  - App-level Escape clears selection even when a modal is open (audit #23).
  - Section switch doesn't clear filter chips, producing empty-result screens with no hint (audit #123).
- **Recommended**:
  - Gate App-level Escape on `!editorOpen && !settingsOpen && !labelsManagerOpen && !helpOpen`.
  - On `setSection`, either auto-clear filters or surface "X filters active — Clear all" hint above the empty state.
  - Compute BulkActionBar's count from `selectedIds.intersection(filtered)` so the badge matches reality.
- **Touches**: `src/App.tsx`, `src/store.ts:setSection`, `src/components/BulkActionBar.tsx`.
- **Complexity**: **S**.

### EI-V0.5-6 [P1, audits #14+#15+#16+#17] — Vault export + Takeout import correctness
- **Current**:
  - Vault export silently overwrites existing files (no collision detection across re-exports).
  - Vault filename collision counter caps at 999 and falls back to an 8-char id without a final uniqueness check.
  - Takeout import drops `userEditedTimestampUsec` / `createdTimestampUsec`.
  - Takeout import drops Takeout reminders entirely.
- **Recommended**:
  - Export: write to a fresh subfolder `keepr-vault-<ISO>/` per run; OR error on existing files; OR honour a `--force` flag at the renderer level.
  - Export collision: replace the `break` at 999 with a UUID suffix that always wins.
  - Takeout: map `createdTimestampUsec` and `userEditedTimestampUsec` to RFC3339 and pass via a new `create_note_with_timestamps`. Map `reminders[]` to our `reminders` table.
- **Touches**: `src-tauri/src/commands.rs:export_vault`, `import_takeout`, possibly `create_note`.
- **Backcompat**: A new `create_note_with_timestamps` keeps the existing `create_note` shape stable.
- **Verify**: Rust round-trip tests for both commands.
- **Complexity**: **M**.

### EI-V0.5-7 [P1, audit #9] — Surface global-hotkey registration failure
- **Current**: Failure is logged to stderr only; user has no UI signal.
- **Recommended**: On startup, attempt registration, emit `keepr://hotkey-status` event with `ok`/`failed`; renderer shows a toast on failed. Add a Settings row "Quick-capture hotkey" with a "Re-register" button and (later) a remap UI.
- **Touches**: `src-tauri/src/lib.rs`, `src/App.tsx`, `src/components/SettingsModal.tsx`.
- **Complexity**: **S**.

### EI-V0.5-8 [P1, audits #29+#30+#33] — `list_notes` payload + sort cost
- **Current**:
  - `list_notes` returns `Attachment.filename` (up to 255 chars) per attachment row — multiplies bandwidth.
  - `AttachmentGrid` renders originals at preview size.
  - `patchNote` re-sorts the full array on every mutation.
- **Recommended**:
  - Trim `filename` from the list_notes attachments shape; serve full record from a per-attachment endpoint.
  - Add `Attachment.thumb` flag once NF-V0.5-B lands.
  - `patchNote`: skip the sort when `patch` doesn't touch `pinned` / `position` / `updated_at` / `title` / `created_at`.
- **Touches**: `src-tauri/src/commands.rs:list_notes`, `src/types.ts`, `src/store.ts:patchNote`.
- **Complexity**: **S**.

### EI-V0.5-9 [P1, audit #12+#13+#124] — Hashtag UX corners
- **Current**:
  - Removing `#work` from body still leaves the "work" label attached.
  - Renaming "work" → "Workstream" via Edit Labels lets a remaining `#work` in body re-create the original.
  - Hashtag in title isn't highlighted in the card preview.
- **Recommended**:
  - On save, also REMOVE labels that came from a hashtag and no longer appear in any text field. Track this via a small `note_label_source` annotation? Or simpler: re-derive the full hashtag-set on every save and only auto-add or auto-remove labels that match the **deleted** set diff.
  - Block label-rename when the body still references the old name; or auto-rewrite body occurrences.
  - Wrap `<HighlightHashtags>` around title too.
- **Touches**: `src/components/NoteEditor.tsx:close`, `src/components/LabelsManager.tsx`, `src/components/NoteCard.tsx`.
- **Backcompat**: Auto-remove is a behavioural change; document loudly.
- **Complexity**: **M**.

### EI-V0.5-10 [P2, audits #51+#52+#53] — Refactor mega-files
- **Current**: `NoteEditor.tsx` is 968 lines; `commands.rs` is 1853 lines; `SettingsModal.tsx` is 400+ lines with five sections inline.
- **Recommended**:
  - Editor: extract `<ChecklistSection>`, `<EditorToolbar>`, `mergeHashtagLabels()` helper.
  - Commands: split into `commands/{notes,labels,attachments,reminders,backup,vault}.rs` modules.
  - SettingsModal: extract `<ThemeRow>`, `<TrashRetentionRow>`, `<AutoBackupSection>`, `<BackupRestoreSection>`, `<VaultIoSection>`.
- **Touches**: as listed.
- **Verify**: tests stay green.
- **Complexity**: **M**.

### EI-V0.5-11 [P2, audit #65+#66] — Drop unused capability permissions
- **Current**: `global-shortcut:allow-is-registered`, `notification:allow-is-permission-granted`, `notification:allow-request-permission` are granted but never invoked from the renderer.
- **Recommended**: Remove from `capabilities/default.json`. Stay minimal.
- **Touches**: `src-tauri/capabilities/default.json`.
- **Complexity**: **XS**.

### EI-V0.5-12 [P2, audit #67] — Scheduler thread shutdown
- **Current**: `std::thread::spawn` with infinite loop; killed mid-tick on `app.exit(0)`.
- **Recommended**: Swap to `tokio::time::interval` task spawned via Tauri's async runtime so it cancels cleanly on app shutdown.
- **Touches**: `src-tauri/src/lib.rs` scheduler.
- **Complexity**: **S**.

### EI-V0.5-13 [P2, audit #69+#70+#71] — Backup pipeline polish
- **Current**:
  - Takeout writes file before DB insert → orphan files on row-insert failure.
  - `export_zip` has no size cap (mirror of `import_zip`'s caps).
  - `export_zip` loads each file into a `Vec<u8>` before writing (RAM spike on large attachments).
- **Recommended**:
  - Takeout: insert row first, then write file, then DELETE on write failure.
  - Export: mirror the 2 GiB total / 512 MiB per-file caps; error if exceeded, with a "delete attachments to shrink your backup" message.
  - Stream files via `std::io::copy(&mut f, &mut zip)`.
- **Touches**: `src-tauri/src/commands.rs:export_zip`, `import_takeout`.
- **Complexity**: **S**.

### EI-V0.5-14 [P2, audit #82+#80] — Reminder schema cleanups
- **Current**: `reminders.id` is unused after creation (renderer keys on `noteId`); no CHECK on `fire_at`.
- **Recommended**: Drop `reminders.id`; make `note_id` the PK. Add `CHECK (fire_at GLOB '????-??-??T??:??:??*')` to defend against direct-SQL writes.
- **Touches**: `src-tauri/src/db.rs:MIGRATION_V4`, `commands.rs` reminder functions.
- **Backcompat**: Migration drops a column — only safe because the column was internal.
- **Complexity**: **S**.

### EI-V0.5-15 [P2, audit #83+#84+#85] — Toolbar density + `aria-pressed` audit
- **Current**: Editor toolbar has nine icon buttons at 250-px modal width. Many toggle-state IconBtns don't pass `pressed`.
- **Recommended**: Group secondary actions (Copy, Archive, Delete) into a kebab "More" menu. Audit every `IconBtn` callsite for `pressed={state}` on toggles.
- **Touches**: `src/components/NoteEditor.tsx`, `src/components/BulkActionBar.tsx`, `src/components/NoteCard.tsx`.
- **Complexity**: **M**.

### EI-V0.5-16 [P2, audit #97+#98+#99+#102+#103+#104] — Docs catch-up
- **Current**: README "Features" misses 16 v0.3/v0.4 additions; "Build from source" misses new plugins; CONTRIBUTING's project-layout missing `src/hooks/`, `src/lib/`; SECURITY pre-dates tray + reminders; RESEARCH_FEATURE_PLAN.md is largely stale.
- **Recommended**:
  - README "Features" rewrite to reflect v0.4 state.
  - SECURITY.md update with reminder-loss case, tray-quit threat, custom-protocol resource path.
  - CONTRIBUTING.md project-layout refresh.
  - Add banner to `RESEARCH_FEATURE_PLAN.md` pointing readers at `RESEARCH_FEATURE_PLAN_v0.5.md`.
- **Touches**: docs only.
- **Complexity**: **S**.

### EI-V0.5-17 [P2, audit #108+#109] — Bundle size: tree-shake + code-split
- **Current**: Bundle is 313 KB; SettingsModal/LabelsManager/HelpOverlay/ReminderPicker all in the main chunk.
- **Recommended**: `React.lazy(() => import('./components/SettingsModal'))` and similar. Confirm lucide-react import paths tree-shake per icon.
- **Touches**: `src/App.tsx`, `vite.config.ts`.
- **Verify**: `npm run build` shows smaller initial chunk + lazy chunks per modal.
- **Complexity**: **S**.

### EI-V0.5-18 [P3, audits #19+#26+#27+#28+#49+#50+#71+#90+#91+#93+#107+#127] — Nits batch
- A bag of small polish/code-hygiene items. Group into one PR.
- Items: drag-handle whitespace on checked rows (#19); duplicate doesn't re-open the copy (#26); "Later today" label after 6 PM (#27); rename `nextWeek` → `nextMonday` (#28); delete `_UnusedX` (#49) and `void useStore` (#50); BulkActionBar Restore icon (#90); NewNoteBar image button (#91); Sidebar Edit Labels placement (#93); `tsconfig.tsbuildinfo` gitignore check (#107); memoize `convertFileSrc` in AttachmentGrid (#127).
- **Complexity**: **S** total.

---

## Reliability, Security, Privacy, and Data Safety

### Bugs / risks found (P0/P1)
- **P0**: lost reminders on toast failure; cross-section reorder corruption; empty-note + reminder orphan; no single-instance guard. Detailed above.
- **P1**: `reorder_notes` global write without backfill; vault export silent overwrite; Takeout chronology + reminder drop; global hotkey silent failure; tray-quit-only exit; bulk-op selection count drift; Esc clears selection inside modal.

### Missing guardrails
- No `CHECK (fire_at GLOB ...)` on `reminders.fire_at` (audit #80).
- No size cap on `export_zip` (audit #70).
- No log file / Settings → Open log surface for diagnostics (audit narrative).
- No "this label is referenced by hashtags" warning when renaming (audit #13).
- No "delete this image attachment?" confirm — current X-button is single click + no undo.

### Permission / network / filesystem concerns
- Capability set is roughly minimal but has 3 unused permissions (audit #65, #66).
- Custom protocol's `Access-Control-Allow-Origin: *` is broad (audit #75) — tighten to `tauri://localhost`.
- `import_takeout` reads attachment bytes into memory without per-file cap on the import side (only the per-attachment cap when copying — audit #57).
- Auto-backup writes a `.zip` into a user-picked folder; no check that the folder is still writable on every tick; no guard against the user picking a network mount that goes offline.

### Recovery and rollback
- `keepr.db.prev` snapshot logic from EI-03 is intact and well-tested.
- No equivalent for the `resources/` dir on import — if a future `import_zip` brings a partial resources tree, no rollback.
- Reminders + Attachments are tied to notes by FK CASCADE; if a future `import_zip` brings stale FK references, the schema CHECK will reject the insert. Verify by Rust test.

### Logging / diagnostics
- Zero logging. `eprintln!` at six sites (`lib.rs` setup errors, scheduler errors, hotkey registration failure). No persisted log; users have no way to attach a log to a bug report.
- NF-V0.5-J above proposes `tauri-plugin-log`.

---

## UX, Accessibility, and Trust

### Onboarding gaps
- First launch shows the empty-state Lightbulb. No guided "Try a checklist, set a reminder, add an image" tour.
- Tray-icon presence + Ctrl+Alt+N hotkey aren't discoverable from the app itself.
- Auto-backup is off by default with no nudge — users with no backup folder will never learn it exists until they need it.

### Empty / loading / error / disabled states
- ✓ Loading state on initial load.
- ✓ Error toasts on mutation failure.
- ✗ No "filter or section is hiding things" hint — empty state in Archive with active Pinned filter just says "Your archived notes appear here" instead of "0 of 5 archived notes match these filters".
- ✗ Reminders sidebar section doesn't exist — no "no upcoming reminders" empty state to design.
- ✗ Attachment-failed state (e.g. resource file deleted out-of-band) renders a broken `<img>` icon; no friendly placeholder.

### Destructive / irreversible
- ✓ `ConfirmDialog` covers Empty Trash, Restore backup, Delete label.
- ✗ `delete_attachment` from the editor: no confirm, no undo.
- ✗ `delete_note_permanent` from Trash: no confirm (just the icon click).
- ✗ Vault export overwriting existing files: no warn.

### Settings clarity
- Settings has grown to 9 distinct sections in one scroll: Theme / Data folder / Move checked / Auto-empty trash / Auto-backup (cadence + folder + lastAt) / Backup-restore (manual) / Markdown-vault + Takeout. The audit (#51) called for sectioning. Even simpler: collapsible groups.
- Settings has no "Reset to defaults" escape hatch.

### Accessibility
- A few new IconBtns missing `pressed` (audit #84).
- Editor toolbar density at narrow widths (audit #83).
- Reminders bell badge in card has no accessible label for SR users beyond the formatted date text — fine.
- ChecklistRow drag-handle is hover-only — keyboard users can't reorder (audit #94).
- Color picker swatches still rely on color (the only visual diff) — for monochrome / colorblind users the names are in `title`/`aria-label`, but a small letter or pattern overlay would help.

### Microcopy / trust signals
- The empty "filters hiding stuff" case wants a Clear-filters CTA.
- README's "Features" being out of date is the single biggest trust hit for new visitors.
- SECURITY.md threat-model omission of new surfaces (tray Quit, reminder scheduler integrity) (audit #104).

---

## Architecture and Maintainability

### Module boundaries
- **`commands.rs` at 1853 lines is the largest single file in the project.** Topical split is overdue (EI-V0.5-10).
- **`NoteEditor.tsx` at 968 lines** is the second-largest. Refactor candidates: `<ChecklistSection>`, `<EditorToolbar>`, `mergeHashtagLabels()`.
- The `App.tsx` effect that "clear stale selections when notes are removed" lives outside any module — fine for one effect, but as the cross-cutting concerns multiply (sweep expired trash + sweep stale selection + auto-backup tick + reminder-fired listener + quick-capture listener + Ctrl+A snapshot) the file is becoming a hub. Watch.

### Refactor candidates
- Extract a `useReminder(noteId)` hook so NoteCard, NoteEditor, and the future RemindersSection all read from one source.
- Extract a `useFocusedNoteId()` hook from `useKeepShortcuts`'s repeated `document.activeElement.closest('.note-card')` pattern.
- Centralize `mime_to_ext` (audit #63) and the inverse — three copies live in `commands.rs` + `lib.rs`.

### Test gaps
The audit enumerates the largest gap class: every new v0.3/v0.4 Rust command lacks tests. **Minimum-viable v0.5 test surface**:
- Rust: `reorder_notes` (3 tests), `duplicate_note` (3), `add_image_attachment` + `delete_attachment` (4), `set_reminder` + `clear_reminder` + `take_due_reminders` (5), `export_vault` (3), `import_takeout` (3) — about 21 new cargo tests.
- Frontend: `BulkActionBar.runBulk`, ReminderPicker preset math, `AttachmentGrid` mosaic, hashtag merge in `NoteEditor.close()`, `useGlobalHotkey` shift handling — about 10 new vitest cases.
- Plus 1 tauri-driver smoke test covering the create-note → set-reminder → export-zip → import-zip → see-the-note flow.

### Doc gaps
- Listed in EI-V0.5-16 above.

### Release / build / deploy gaps
- **No release ever built.** `tauri build` has never run; bundle-size and Win-install behaviour unverified. This is the single biggest meta-issue for v0.5+.
- No code-signing decision.
- No auto-update path.
- No Linux/macOS CI matrix.
- All five tracked by NF-V0.5-F + NF-V0.5-K.

---

## Prioritized Roadmap

### Phase A — v0.4.1 hotfix (P0 only, ship within a week)

- [ ] **P0 — EI-V0.5-2 — Reminder lost-toast fix**
  - Why: silent data loss; the reminder feature can't be trusted today.
  - Evidence: [audit #3](.audit-notes-code-v0.4.md)
  - Touches: `src-tauri/src/commands.rs:take_due_reminders`, `src-tauri/src/lib.rs` scheduler
  - Acceptance: when `notification.show()` returns `Err`, `fired_at` stays NULL; the same reminder fires on the next 30s sweep.
  - Verify: new cargo test simulating a failing notify; manual smoke with Focus Assist on.
- [ ] **P0 — EI-V0.5-3 — Empty-note + reminder orphan fix**
  - Why: user sets a reminder, closes editor, reminder vanishes silently.
  - Evidence: [audit #4](.audit-notes-code-v0.4.md), confirmed at `NoteEditor.tsx:320-325`.
  - Touches: `src/components/NoteEditor.tsx:close`
  - Acceptance: setting a reminder on a brand-new empty note and closing keeps both the note and the reminder.
  - Verify: vitest case + manual.
- [ ] **P0 — EI-V0.5-1 (drag part) — Refuse drag in non-Notes sections**
  - Why: Custom-sort drag in Archive corrupts active-Notes ordering.
  - Evidence: [audit #1, #6](.audit-notes-code-v0.4.md), `NoteGrid.tsx:73-93`.
  - Touches: `src/components/NoteGrid.tsx` gating, `src-tauri/src/commands.rs:reorder_notes` defensive guard.
  - Acceptance: with sortMode=custom, drag is enabled only when `section.kind === "notes"`.
  - Verify: manual; vitest for the gate condition.
- [ ] **P0 — EI-V0.5-4 — `tauri-plugin-single-instance`**
  - Why: portable EXE concurrent-process corruption risk.
  - Evidence: [audit #77](.audit-notes-code-v0.4.md).
  - Touches: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`.
  - Acceptance: double-launching `keepr.exe` brings the existing window forward.
  - Verify: manual on portable build.

### Phase B — v0.5.0 "Polish & Reliability"

Goal: close every remaining P1 from the audit, raise test coverage to ≥ 100 vitest + ≥ 25 cargo, and finally ship a Released binary.

- [ ] **P1 — NF-V0.5-F — Bundled release pipeline + auto-updater scaffold**
  - Why: there is no installable Keepr today.
  - Evidence: zero GitHub Release assets; `tauri build` never run.
  - Touches: `.github/workflows/release.yml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs` (updater plugin).
  - Acceptance: `git tag v0.5.0 && git push --tags` produces an NSIS + portable .zip artifact attached to the GitHub Release; the app self-updater is initialised (even if no manifest published yet).
  - Verify: tag a v0.5.0-rc1, install MSI on clean Win 11 box, confirm app launches.
- [ ] **P1 — NF-V0.5-B — Image thumbnail pipeline**
  - Why: card grid blows up RAM with full-res images.
  - Evidence: [audit #30](.audit-notes-code-v0.4.md).
  - Touches: `src-tauri/src/commands.rs:add_image_attachment`, `src/components/AttachmentGrid.tsx`, `src-tauri/src/lib.rs:handle_resource_request` (`?thumb=1`).
  - Acceptance: a 2000×2000 JPEG yields a sibling thumb file ≤ 480 px on long edge; card grid uses the thumb.
  - Verify: cargo test + manual on a 5-image note.
- [ ] **P1 — EI-V0.5-1 (migration part) — v4 `position` backfill**
  - Why: users entering Custom sort fresh see random order.
  - Touches: `src-tauri/src/db.rs:MIGRATION_V4`.
  - Acceptance: on v3 → v4 upgrade, every note has a unique `position`.
- [ ] **P1 — EI-V0.5-5 — Selection / Esc / filter interactions**
- [ ] **P1 — EI-V0.5-6 — Vault + Takeout correctness**
- [ ] **P1 — EI-V0.5-7 — Surface hotkey failure**
- [ ] **P1 — EI-V0.5-8 — `list_notes` payload + sort cost**
- [ ] **P1 — EI-V0.5-9 — Hashtag UX corners**
- [ ] **P1 — Test coverage** — cargo tests for every new command + vitest for new pure helpers (≥ 21 cargo, ≥ 10 vitest).

### Phase C — v0.5.1+ "Polish nice-to-haves"

- [ ] **P2 — EI-V0.5-10 — Refactor mega-files**
- [ ] **P2 — EI-V0.5-11 — Drop unused capabilities**
- [ ] **P2 — EI-V0.5-12 — Scheduler shutdown via tokio**
- [ ] **P2 — EI-V0.5-13 — Backup pipeline polish (stream + caps + insert-then-write)**
- [ ] **P2 — EI-V0.5-14 — Reminder schema cleanup (drop `id`, add CHECK)**
- [ ] **P2 — EI-V0.5-15 — Toolbar density + aria-pressed audit**
- [ ] **P2 — EI-V0.5-16 — Docs catch-up (README + SECURITY + CONTRIBUTING + stale RESEARCH_FEATURE_PLAN banner)**
- [ ] **P2 — EI-V0.5-17 — Code-split + tree-shake bundle**
- [ ] **P2 — NF-V0.5-H — Per-label note counts in sidebar**
- [ ] **P2 — NF-V0.5-I — Paste + drop image into editor**
- [ ] **P2 — NF-V0.5-J — `tauri-plugin-log` + Settings → Open log**

### Phase D — v0.6 "Reminders v2 + Vault" (one of the two as headliner)

- [ ] **P1 — NF-V0.5-A — Reminders v2 (recurrence + sidebar + snooze + in-app toast)**
- [ ] **P2 — NF-V0.5-C — App Lock + Private Vault** (paired or alternative headliner)

### Phase E — v0.7+ "Long-running bets"

- [ ] **P2 — NF-V0.5-D — Version history with diff**
- [ ] **P3 — NF-V0.5-E — Drawing notes (vector canvas)**
- [ ] **P3 — NF-V0.5-G — ICS export of reminders**
- [ ] **P3 — NF-V0.5-K — macOS + Linux CI matrix** (binaries optional)
- [ ] **P3 — NF-12** OCR (Windows OCR API)
- [ ] **P3 — NF-13** Rich URL preview cards
- [ ] **P3 — NF-21** Indent sub-items in checklists
- [ ] **P3 — NF-22** Background image patterns (Keep's 9 textures)
- [ ] **P3 — EI-10** Replace `react-masonry-css`
- [ ] **P3 — EI-18** FTS5 backend

---

## Quick Wins

Small, low-risk; can ship in the v0.4.1 hotfix or as a one-PR cleanup at any time.

1. Drop unused capability permissions (audit #65, #66) — 5-line diff in `capabilities/default.json`.
2. Delete `_UnusedX = X` (`FilterChips.tsx:286`) and `void useStore` (`NoteGrid.tsx:145`).
3. Rename `nextWeek` → `nextMonday` in `ReminderPicker.tsx`.
4. Swap BulkActionBar Restore icon from `ArchiveRestore` to `RotateCcw` to match NoteCard.
5. Trim `tauri.conf.json` `fullscreen: false` (redundant).
6. Inline boot script already in place — no change.
7. README "Features" rewrite (15 min).
8. Hide "Pinned" filter chip in Trash section (audit #87).
9. `convertFileSrc` memoization in `AttachmentGrid` (one `useMemo`).
10. Add a `// matches Keep web's behavior` comment to `import_takeout`'s trashed-skip (audit #18).

---

## Larger Bets

Items needing design + staged rollout.

1. **NF-V0.5-C App Lock + Private Vault** — XL. Threat model design, Argon2id tuning, lost-PIN policy, vault round-trip tests, settings-screen design.
2. **NF-V0.5-A Reminders v2 (RRULE + sidebar + snooze + in-app)** — L. RFC 5545 expansion is non-trivial; pick a library (`rrule-rs` for Rust).
3. **NF-V0.5-D Note version history** — M, but UX of the timeline drawer is the make-or-break.
4. **NF-V0.5-E Drawing notes** — L. Canvas perf, pen-pressure, vector storage all need decisions.
5. **NF-V0.5-F Release pipeline + updater + signing** — L. Code-signing strategy is the gating decision (sigstore vs EV cert vs unsigned with documented SmartScreen workaround).
6. **macOS + Linux first-class support** — L. Tauri makes it portable in theory; tray + global-shortcut + notification behave differently on each.

---

## Explicit Non-Goals

(carrying forward from original plan, with v0.4 evidence added)

- **Collaboration / real-time co-edit** — out of scope. Single-user offline.
- **Location-based reminders** — Google deprecated them; battery hungry; doesn't fit a desktop app.
- **Folders / hierarchy** — Keep identity is flat. Edit Labels covers organization.
- **Outliner / block editing** — anti-Keep.
- **AI features / RAG / autocomplete** — preserved no-network promise. Skip.
- **Account / sync server** — Keepr's value is the absence.
- **Telemetry** — same.
- **Feature paywall** — MIT; never fragment.
- **User scripts attached to notes** (Trilium-style) — sandboxing nightmare.
- **Custom protocol expansion to arbitrary file types** — keep `keepr-resource://` strictly for images/audio/drawings; don't open a generic file embed.
- **Built-in cloud-sync without user-managed credentials** — even when we eventually ship sync, it should be "watch a folder you point at your existing cloud sync" rather than a Keepr-managed server.

---

## Open Questions

Only the questions whose answers materially change the v0.5+ shape.

1. **Code-signing for v0.5 release** — three paths: (a) ship unsigned with a documented SmartScreen workaround in the README (cheapest; reputation will accrete after enough downloads); (b) sigstore / OSS-friendly free signing route ([Sigstore Cosign for Windows](https://docs.sigstore.dev/) is still nascent for `.msi`); (c) EV cert ($300/yr). Default suggestion if no preference: ship unsigned in v0.5 with a clear README section + add the cert later. Confirm before ship.
2. **NF-V0.5-A Reminders v2 — pick a RRULE library**: [`rrule-rs`](https://crates.io/crates/rrule) (~ pure Rust, lean) vs hand-roll for v1's limited Daily/Weekly/Monthly. Default: ship hand-rolled for v0.6, swap to `rrule-rs` if/when "custom interval" lands.
3. **NF-V0.5-C lost-PIN recovery**: original plan defaulted to "Notesnook model — no recovery, document loudly." Confirm before user-tests.
4. **Reminder scheduler granularity**: 30 s is the current poll interval. For Keep parity (reminders fire at the second they're set for) we'd need 1 s polling, which burns CPU. Default: keep 30 s, document the up-to-30-s lag in the picker UX, revisit only if users complain.
5. **macOS + Linux CI matrix priority**: low-cost to add (just `runs-on: [windows-latest, macos-latest, ubuntu-latest]`); high cost to actually maintain (Linux WebKitGTK is fiddly, macOS tray + global-shortcut differ). Default: add to CI in v0.5.1 but explicitly do NOT promise platform support in any release notes until someone has actually used Keepr on those platforms and filed bugs.

---

## Research evidence files

- [`.audit-notes-code-v0.4.md`](.audit-notes-code-v0.4.md) — 127 numbered findings from this pass, file:line cited, P0–P3 graded.
- [`.audit-notes-code.md`](.audit-notes-code.md) — original v0.1 audit (150 findings, mostly closed).
- [`.audit-notes-keep.md`](.audit-notes-keep.md) — Keep feature gap inventory (still current).
- [`.audit-notes-competitors.md`](.audit-notes-competitors.md) — 12-app competitor audit (still current).
- [`RESEARCH_FEATURE_PLAN.md`](RESEARCH_FEATURE_PLAN.md) — original v0.1 → v0.2 plan (mostly shipped; superseded by this file for v0.5+).
- [`CHANGELOG.md`](CHANGELOG.md) — what shipped in each release.
- [`ROADMAP.md`](ROADMAP.md) — live task list (will be replaced after this plan is reviewed).
