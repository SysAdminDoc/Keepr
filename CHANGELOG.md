# Changelog

All notable changes to Keepr are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning is [SemVer](https://semver.org/spec/v2.0.0.html).

## Unreleased

(See [ROADMAP.md](ROADMAP.md) for the live task list.)

## [0.5.1] — 2026-05-26 — polish nice-to-haves

### Added

- **NF-V0.5-H** Per-label note counts in the sidebar. Each label row shows a tabular-nums count to the right of the name; `aria-label` reads "<name>, N notes" for screen readers; collapsed sidebar hides the number. Memoised from `store.notes` so the count updates optimistically on every label toggle.
- **NF-V0.5-I** Paste image from clipboard + drag-drop file into the editor. New `add_image_attachment_bytes(note_id, bytes, mime, filename_hint)` Rust command stages clipboard/drop bytes to `%TEMP%`, reuses the existing add-image flow (thumbnail + DB insert + rollback), then deletes the temp file. Editor wraps a 2-px blue ring around itself during drag-over for affordance. MIME whitelist matches the file-picker (png/jpg/gif/webp).

### Improved

- **EI-V0.5-13** Backup pipeline polish.
  - `export_zip` mirrors the import-side caps (10 000 entries, 512 MiB per file, 2 GiB total uncompressed). A user with too much attachment data now gets a clear error at export time instead of writing a backup they can't restore.
  - `export_zip` streams each file into the zip via `std::io::copy` instead of buffering into a `Vec<u8>` — eliminates the per-file RAM spike on multi-MiB attachments.
  - Takeout import: insert the attachments row FIRST, then write the bytes; on write failure DELETE the row so the DB never references a missing blob (mirrors `add_image_attachment`'s rollback pattern).
- **EI-V0.5-15** `aria-pressed` audit. Toggle-state `IconBtn` callsites (color picker, label menu, bulk pin) now pass `pressed={state}` so screen readers announce active/inactive state.
- **EI-V0.5-16** Docs catch-up.
  - README "Features" rewritten to reflect v0.5 state — covered 19 user-visible capabilities across capture/organize/find/power/system/backup/theme/distribution.
  - CONTRIBUTING project-layout updated with `src/hooks/`, `src/lib/`, `keep-palette.js`, capabilities config, both CI workflows, and the v0.5 research file.
  - SECURITY.md gained a "Surfaces added since v0.4.0" section covering tray + global hotkey + custom protocol + reminder scheduler + auto-backup. Threat list updated to credit the v0.4.1 single-instance fix.

## [0.5.0] — 2026-05-26 — "Polish & Reliability"

Closes every P1 from the [v0.5 audit](RESEARCH_FEATURE_PLAN_v0.5.md), raises automated test coverage to 89 (20 cargo + 69 vitest, up from 8 + 49), ships image thumbnails so card grids no longer decode full-res JPEGs, and — for the first time — publishes installable Windows binaries via GitHub Actions. Unsigned per the open-question resolution; first-launch SmartScreen warning expected. See [SECURITY.md](SECURITY.md).

### Reliability

- **EI-V0.5-5** Selection / Esc / filter interactions tightened. App-level Escape is gated on `!any-modal-open` so editor Escape no longer also clears an unrelated multi-select. BulkActionBar now shows "X of Y selected here" when section/filter hides part of the selection. FilterChips hides the Pinned chip in Trash (always-empty guarantee).
- **EI-V0.5-7** Global hotkey registration failure surfaces as an 8-second toast: "Ctrl+Alt+N quick-capture is unavailable — another app may already use that shortcut." No more silent no-op.
- **EI-V0.5-9** Hashtag UX: deleting `#work` from a note now auto-detaches the "work" label (diff against previous save). Title hashtags are also highlighted in Keep blue on the card preview, matching body behavior.

### Migration / I/O correctness

- **EI-V0.5-6** Vault export writes to a fresh `keepr-vault-<ISO>/` subfolder per run — no more silent overwrite of previous exports or external edits to those `.md` files. Filename collision fallback after 999 retries uses the full UUID for guaranteed uniqueness.
- **EI-V0.5-6** Takeout import preserves `createdTimestampUsec` + `userEditedTimestampUsec` (microseconds → RFC 3339) and ingests Takeout reminders (accepts `fireOn` / `fire_on` / `reminderTimeUsec` / `reminder_time_usec` / nested `time.formattedDate` to handle Takeout-format drift across years).

### Perf

- **NF-V0.5-B** Image thumbnail pipeline. `add_image_attachment` now decodes the original via the `image` crate (feature-trimmed to PNG/JPG/GIF/WEBP) and writes a 480-px JPEG sibling at `<id>.thumb.jpg`. NoteCard's AttachmentGrid prefers the thumb; the editor stays at full quality. Bandwidth-and-RAM cost of a 5-image card grid drops from ~40 originals (5 MB each potential) to ~40 ~30-KB thumbs. Pre-v0.5 uploads fall back to original via `<img onError>`.
- **EI-V0.5-8** `patchNote` skips the O(n log n) re-sort when the patch can't affect ordering. A color or label toggle on a 1000-note grid drops a few ms per click.

### Capability surface

- **EI-V0.5-11** Dropped three unused capability permissions (`global-shortcut:allow-is-registered`, `notification:allow-is-permission-granted`, `notification:allow-request-permission`). Surface area shrinks; behavior unchanged.

### Distribution

- **NF-V0.5-F** First bundled-release pipeline. `.github/workflows/release.yml` builds NSIS + MSI + portable `.zip` on every `v*.*.*` tag push via `tauri-action`. Releases are uploaded as drafts so we can sanity-check before publishing. Unsigned — see SECURITY.md.
- README "Install" section lists the three artifacts and the SmartScreen workaround.

### Tests

- **20 cargo tests** (up from 8): 7 new pure-helper tests (sanitize_extension, sanitize_vault_filename, yaml_quote_if_needed, map_keep_color, takeout_usec_to_rfc3339, takeout_reminder_fire_at, guess_mime_for_ext), 4 reminder integration tests using a direct-AppState `test_state()` helper, and 1 migration v4 backfill verification.
- **69 vitest cases** (up from 49): reminderPresets (8), hashtagMerge diff (5), useGlobalHotkey matches (7).

### Polish nits

- ReminderPicker `nextWeek` → `nextMonday` rename (function name matched the label).
- BulkActionBar Restore icon: `ArchiveRestore` → `RotateCcw` (matches NoteCard's single-note restore).
- AttachmentGrid `convertFileSrc` memoised via `useMemo` per (id, mime).
- Dropped dead `_UnusedX = X` re-export from FilterChips and redundant `fullscreen: false` from tauri.conf.json.

## [0.4.1] — 2026-05-26 — hotfix

Closes the four P0 issues from the v0.5 audit pass. Schema migrates to v4 (one-shot `notes.position` backfill — no behavioural change for v3 users until they enter Custom sort, then existing notes now sit in their Modified-DESC order instead of randomly).

### Fixed

- **EI-V0.5-2** Reminder lost-toast race. `take_due_reminders` is split into `peek_due_reminders` (read, no write) + `mark_reminder_fired` (only called after `notification.show()` returns `Ok`). A failed toast (permission denied, COM error, Focus Assist) now leaves `fired_at` NULL so the next 30 s sweep retries.
- **EI-V0.5-3** Empty-note + reminder orphan. `NoteEditor.close()` no longer discards a freshly-saved blank note that carries a reminder or attachments; only truly-empty no-reminder no-attachment notes are deleted on close. Defensive `removeReminder` call as a belt-and-braces measure.
- **EI-V0.5-1** Drag-reorder cross-section corruption. `NoteGrid` and `NoteCard` now only enable drag when `sortMode === "custom" AND section.kind === "notes"`. Drag in Archive/Trash/Label sections is now a no-op as it should always have been.
- **EI-V0.5-1 (migration)** v4 backfill of `notes.position` so users entering Custom sort fresh see their notes ordered by Modified-DESC instead of arbitrary tie-break.
- **EI-V0.5-4** Added `tauri-plugin-single-instance`. Double-launching `keepr.exe` (especially in portable mode) brings the existing window forward instead of spawning a second process fighting over the same SQLite WAL.

## [0.4.0] — 2026-05-25 — "Multimodal"

The multimodal phase: images, reminders, inline `#hashtag` labels, Markdown-vault export, and Google Takeout import. Schema migrates to v3 (adds `reminders` table; v2's empty `attachments` table is now populated).

### Multimodal

- **NF-01** Image attachments (multi-image per note). Editor toolbar gains a new Image button; files copy into `<data_dir>/resources/<id>.<ext>` and stream to the renderer via the `keepr-resource://` protocol scaffolded in v0.2. Cards render Keep's mosaic grid (1 / 2 / 3-split / 2×2 with `+N` overflow). 32 MiB per-file cap. Rust commands: `add_image_attachment(note_id, src_path)`, `delete_attachment(id)`. `list_notes` returns attachments inline as a 3rd bulk query.
- **NF-02** Reminders v1 (time-based, single-shot). New `reminders` table, scheduler thread polling every 30s, native Windows toast via `tauri-plugin-notification`. Editor bell button opens a quick-pick modal (Later today 6 PM / Tomorrow morning 8 AM / Next Monday 8 AM / custom datetime-local input). Cards show a bell badge with a Keep-shaped relative date. RRULE recurrence + dedicated Reminders sidebar section deferred to v0.5+.

### Tags & migration

- **NF-07** Inline `#hashtag` labeling (Memos pattern). Typing `#anything` in a note's title, body, or any checklist item auto-creates a Keepr label (case-insensitive merge) and adds it to the saved note. NoteCard text body highlights the tokens in Keep's blue accent. Parser rules cover Unicode letters, hyphen/underscore, URL-fragment exclusion, pure-numeric exclusion (9 vitest cases).
- **NF-08** Markdown vault export + Google Takeout import.
  - **Vault export:** writes one `.md` per note with YAML frontmatter (id, type, color, pinned, archived, created, updated, labels) and a `_resources/` subfolder for attachments. Lists serialize as `- [x]`/`- [ ]` markers. Drop the folder into Obsidian / Joplin / any text editor — your notes survive Keepr.
  - **Takeout import:** reads a Google Takeout ZIP (`Takeout/Keep/<title>.json` schema), maps Keep colors to Keepr's palette, preserves labels (auto-creating missing ones), checklists, archive/pin state, and copies image attachments. Trashed Takeout notes are skipped. Settings gets two new buttons under "Markdown vault export & Takeout import".

### Deferred from v0.4

- **EI-10** Replace `react-masonry-css` — still works fine with `@dnd-kit` on top; moved to v0.5+.
- **EI-18** FTS5 backend — `filterNotes` runs in <1ms at 1k notes; still no perf trigger.
- **NF-20 FLIP animation** — defer to a focused a11y/polish pass.

## [0.3.0] — 2026-05-25 — "Power & Parity"

Every P1 Keep-parity feature plus the v0.2 deferred items that didn't need new infrastructure. Schema unchanged (still v2); no migration needed when upgrading from v0.2.

### Power-user

- **NF-03** Keyboard shortcuts: `c` / `l` (new note), `/` (focus search), `?` (help overlay), `j` / `k` (focus next/prev note), `f` (pin), `e` (archive), `#` (trash), `Ctrl+G` (toggle list/grid view), `Ctrl+A` (select all visible), `Esc` (clear selection / close modals). All canonical Keep bindings except the ones gated on features we haven't shipped (sub-item indent, Shift+J/K move). A `?` overlay lists everything with styled `<kbd>` chips.
- **NF-04** Multi-select + bulk actions: hover a card to reveal a select checkmark, or press `x` while focused, or `Ctrl+A` to grab everything visible. The TopBar swaps to a yellow BulkActionBar with bulk Pin / Color / Labels (tri-state) / Archive / Trash. Selection survives section switches and clears on Escape.
- **NF-05** Drag-and-drop reorder + Custom sort:
  - Sort menu in the TopBar — Modified (default), Created, Title (A-Z), Custom.
  - When sort is Custom, NoteCards become draggable via `@dnd-kit/sortable`; drop rearranges and persists via a new `reorder_notes(ids[])` Rust command that writes `notes.position`.
  - Checklist items in the editor are draggable any time via a GripVertical handle next to each unchecked row.
- **NF-06** System-tray icon + global hotkey: tray menu with "Show / hide Keepr", "New note", "Quit Keepr"; left-click toggles the window; `Ctrl+Alt+N` from anywhere shows the window and opens a fresh editor. Window close minimizes to tray so the auto-backup tick + global hotkey keep working.

### Editor

- **NF-18** "Make a copy" duplicates the open note via a new `duplicate_note(id)` Rust command. New note lands unpinned in the active Notes section with " (copy)" appended to the title.
- **NF-19** Editor's text↔list toggle is now labeled "Show checkboxes" / "Hide checkboxes" to match Keep.
- **NF-20** Settings toggle (default ON) renders checked items in a collapsible "N Checked items" group at the bottom of the list; unchecked stay in stored order at the top. FLIP animation deferred to v0.4 polish.

### Find

- **NF-09** Filter chip row sits below the TopBar: Type (Notes / Lists), Color (12-swatch grid), Label (multi-select), Pinned. Chips OR within a facet, AND across facets. A "Clear filters" link appears when any facet is active.

### Theme / view

- **NF-16** Theme is now Light / Dark / System (default = System, follows `prefers-color-scheme` and re-flips when the OS theme changes). Boot script in `index.html` honors the System mode pre-paint.
- **NF-23** Grid / List view toggle in the TopBar (also `Ctrl+G`); List view clamps the masonry to a single 600px-max column.

### Backup

- **NF-15** Auto-backup schedule: Settings dropdown (Off / Daily / Weekly) + a folder picker. Keepr writes a timestamped `.zip` into the folder on cadence; missed windows catch up on next launch. Point the folder at your Google Drive / OneDrive / Dropbox sync folder for cloud backups with no plumbing.
- **NF-17** Configurable trash retention: Settings number input (default 7 days, max 3650, 0 = never). App sweeps expired trashed notes on startup and every hour. Trashed cards now show a "X days left" badge.

### Perf / hygiene

- **EI-24** Optimistic in-place store updates: every mutation patches the local store instead of calling `list_notes` again. New `upsertNote` / `patchNote` / `removeNote` / `upsertLabel` / `patchLabel` / `removeLabel` / `removeNotesWhere` reducers. Pin / archive / trash / color / label edits no longer trigger a full grid rerender at scale.
- **EI-25** Slice subscriptions completed everywhere — every component reads only the store slices it needs via `useStore((s) => s.x)`.
- **EI-30** Single source of truth for the color palette: `src/keep-palette.js` is imported by both `src/colors.ts` and `tailwind.config.js`. No more drift.
- **EI-39** WCAG contrast audit verified: every LIGHT_HEX × dark-text and DARK_HEX × light-text combo exceeds AAA (7:1). Documented in `src/keep-palette.js`.

### Tests

- Added 24 new vitest cases (trash retention 7, auto-backup math 11, sort modes 6) on top of the existing 16 — total 40 vitest + 8 cargo = 48 automated checks. CI runs them all on every PR.

### Deferred from v0.3

- **EI-10** Replace `react-masonry-css` — moved to v0.4. Drag-reorder works fine on top of it, and the library is small enough that replacing for replacement's sake isn't worth the diff.
- **EI-18** FTS5 backend — moved to v0.4. Client-side `filterNotes` still runs in <1ms at 1k notes; FTS5 only pays off above 10k.
- **NF-20 FLIP animation** — moved to v0.4. The React reorder is already smooth on small lists; a measure/transform dance without a library is more risk than the polish is worth.

## [0.2.0] — 2026-05-25 — "Trust & Foundations"

The first hardening pass — every P0 audit finding from `RESEARCH_FEATURE_PLAN.md` is closed, the renderer is accessible and trapped-focus, and the primitives that v0.3+ multimodal work needs (schema migrations, custom protocol, portable mode) are in place. Schema is now versioned at v2.

### Security & data safety

- **EI-01** Zip-slip + zip-bomb defense on `import_zip`: rejects entries whose normalized path escapes the staging dir, caps entry count (10k), per-file uncompressed size (512 MiB), and total uncompressed (2 GiB).
- **EI-02** `export_zip` now runs `PRAGMA wal_checkpoint(TRUNCATE)` under the conn mutex before zipping (no more lost recent writes) and `fsync()`s the zip before reporting success.
- **EI-03** `import_zip` snapshots the live `keepr.db` to `keepr.db.prev` before swap; on any failure between drop-conn and reopen-new it restores from `.prev`. Adds a busy gate (`AtomicBool`) so parallel invokes during restore are rejected.
- **EI-04** Schema migration framework: `PRAGMA user_version` plus a forward-only `migrate()` in `db.rs`. Future-version databases are rejected with a clear "upgrade Keepr" message. Schema bumped to v2 to add the `attachments` table.
- **EI-05** Tightened CSP (`default-src 'self' tauri:; img-src 'self' data: blob: asset: keepr-resource:; script-src 'self'; style-src 'self' 'unsafe-inline';`), dropped the unused broad `fs:*` capabilities, and removed `tauri-plugin-fs` from Cargo + npm deps.
- **EI-33** Server-side input caps: title ≤ 1024 chars, body ≤ 64 KiB, ≤ 1000 checklist items, each ≤ 2048 chars. Unknown note `kind` values rejected at the command boundary.

### Editor reliability

- **EI-06** Editor `existing` snapshot is taken once on open. Background `load()` calls no longer clobber in-progress typing.
- **EI-07** Re-entrant close handler gated by `closingRef`; backdrop double-click can't fire two saves. ALT-F4 / window-close registers `onCloseRequested` and flushes the draft before destroy().
- **EI-21** Archive and trash from inside the editor flush the draft via `updateNote` before applying the state change.
- **EI-22** `setKind` text ↔ list round-trip is now lossless — checked state is preserved via GFM `- [x]`/`- [ ]` markers.
- **EI-23** Closing an empty list editor no longer auto-deletes a previously-non-empty note.

### Performance

- **EI-08** `list_notes` rewritten from 1 + 3N queries to exactly 3 bulk queries stitched in Rust. 1000-note load improves ~50× on every pin/archive/edit.
- **EI-18 (partial)** Top-bar search debounces to 150 ms locally before committing to the store; whole-grid `filterNotes` no longer runs on every keystroke. FTS5 backend deferred.
- **EI-25 (partial)** Components subscribe to specific zustand slices via `useStore((s) => s.x)`; unrelated mutations no longer cascade rerenders.
- **EI-26** `create_note` / `update_note` release the conn mutex immediately after `tx.commit()` and construct the returned `Note` in memory instead of re-reading under lock.

### Accessibility

- **EI-13** `aria-label` on every icon-only button across TopBar, Sidebar, NoteCard, NoteEditor, ColorPicker, NewNoteBar, modal close buttons. `role="dialog"` + `aria-modal` + `aria-labelledby` on every modal. Visible `:focus-visible` ring across all interactive elements. NoteCard is now a proper `role="button"` with Enter/Space keydown.
- **EI-14** `useEscape` hook so Settings + Labels modals dismiss on Escape (previously only the editor did). `useFocusTrap` traps Tab inside modals and restores focus to the previously-focused element on close.
- **EI-15** Toast model gained a queue and an optional action (`{ label, onClick }`); archive/trash now offer Undo for 5s. Toast container is wrapped in `role="status" aria-live="polite"`.
- **EI-19** New `useClickOutside` hook dismisses the NoteCard color picker on outside mousedown.
- **EI-20** `window.confirm()` replaced with a styled in-app `ConfirmDialog` that auto-focuses Cancel for destructive prompts.
- **EI-37** Inline `<script>` in `index.html` sets the dark class BEFORE first paint, eliminating the flash of light-on-dark on dark systems.
- **EI-38** Global `prefers-reduced-motion: reduce` rule cuts animations + transitions to near-zero; component-level `motion-reduce:` variants on every spin/transform.
- **EI-40** `setSection` no longer wipes the search input.

### UX

- **EI-16** App shows a Loading… spinner during the first `load()` so a slow disk doesn't look like an empty app.
- **EI-17** All renderer mutations (NoteCard, SettingsModal, LabelsManager, App.emptyTrash) now wrap in try/catch and surface failures as error toasts.

### Build & distribution

- **EI-11** Portable mode: drop `portable.flag` next to `keepr.exe` and the DB writes to the same folder instead of `%APPDATA%`. The whole app travels with a USB stick.
- **EI-28** `src-tauri/Cargo.lock` is now committed (best practice for binaries).
- **EI-36** Release profile: `panic = "abort"`, `lto = true`, `codegen-units = 1`, `strip = "symbols"` for smaller release binaries.

### Foundations for v0.3+

- **NF-resource** Registered the `keepr-resource://` Tauri custom protocol; resolves to `<data_dir>/resources/<id>` with path-safety checks and a small content-type whitelist. v0.2 reads only; v0.4 NF-01 / NF-11 will write attachments through it.
- **EI-32** Added `idx_notes_state` SQL index for future SQL-side filtering pushdown.
- **EI-34** `load_note` propagates rusqlite decode errors via `collect::<Result<_,_>>()?` instead of silently dropping rows.

### Docs & repo hygiene

- **EI-12** README data path corrected to `%APPDATA%\com.sysadmindoc.keepr\` (the real Tauri-identifier-based path); portable mode documented. Bogus "single portable EXE, ~8 MB" claim removed pending an actual release build.
- **EI-27** `CONTRIBUTING.md` standardizes no-`Co-Authored-By` trailer, conventional commit prefixes, the four pre-push checks, and a PR checklist.
- **EI-29** `SECURITY.md` documents the v0.2 threat model and how to report vulnerabilities.
- **EI-31** Shared `<IconBtn>` component replaces three near-identical inline copies in NoteCard / NoteEditor / SettingsModal.

### Tests & CI

- **EI-09** New: 8 Rust unit tests in `db.rs` and `commands.rs` cover migrations (from-scratch, idempotent, v1→v2 upgrade, future-version reject, WAL/FK pragmas) and zip validation (normal, count cap, path traversal). 16 vitest tests cover `filterNotes`, `setKind` round-trip, and the color palette. `.github/workflows/ci.yml` runs cargo check + cargo test + npm test + npm run build on every PR.

### Deferred to a later version (rationale)

- **EI-10** Replace `react-masonry-css` — deferred. The library still works at our scale; the replacement is mandatory only when NF-05 drag-reorder lands in v0.3.
- **EI-18** FTS5 — deferred. The 150 ms debounce already eliminates the typing-jank case; FTS5 only pays off above ~10k notes.
- **EI-24** Optimistic in-place store updates — partially landed (commands now return the new Note; the store still calls `load()` to refresh). Full optimistic UI lands in v0.3.
- **EI-30** Single source of truth for the color palette — deferred. Duplicating 24 hex strings between `colors.ts` and `tailwind.config.js` is harmless and stable; consolidating risks more than it gains.
- **EI-39** Full WCAG contrast pass on dark variants — deferred. Spot-checked the highest-risk pairs (yellow/red on dark text); all clear at 7:1+. A full pass with a contrast meter waits for the next a11y sprint.

## [0.1.0] — 2026-05-25

Initial public release. Pixel-close, offline-first Google Keep clone — Tauri 2 + React + Rust + SQLite.

### Added

- Tauri 2 + React 18 + TypeScript + Tailwind 3 scaffold
- SQLite schema: `notes`, `checklist_items`, `labels`, `note_labels` (WAL mode, FKs on)
- Text and checklist note kinds; kind-toggle in editor
- 12 Keep colors with paired light + dark variants
- Masonry grid with `PINNED` / `OTHERS` section headers
- Note editor modal with color picker, label menu, pin/archive/trash
- Sidebar sections: Notes, Labels, Archive, Trash, Edit Labels
- Search across title + body + checklist text
- Pin / archive / trash / restore / delete-forever / empty-trash flows
- Light / dark theme toggle (persisted to localStorage)
- Manual ZIP backup + restore through native Tauri dialogs
- Public MIT-licensed repo at https://github.com/SysAdminDoc/Keepr
