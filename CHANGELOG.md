# Changelog

All notable changes to Keepr are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning is [SemVer](https://semver.org/spec/v2.0.0.html).

## Unreleased

(See [ROADMAP.md](ROADMAP.md) for the live task list.)

## [0.19.0] — 2026-05-26 — "Cross-platform CI"

### Changed

- **CI now runs the Rust + frontend job matrix on Windows, macOS, and Linux.** Previously `.github/workflows/ci.yml` only built on `windows-latest`, even though `release.yml` ships macOS-13/macOS-14/ubuntu-22.04 artifacts at every tag. First Linux/macOS regression would have landed at tag time. New matrix: `windows-latest` + `macos-14` + `ubuntu-22.04`, `fail-fast: false`. Linux job picks up the WebKitGTK + GTK + AppIndicator apt-install step from `release.yml` so cargo check / test can compile against Tauri 2's GTK bindings.

## [0.18.1] — 2026-05-26 — "Pinned notes don't shuffle on unpin"

### Fixed

- **Unpinning a pinned note no longer rearranges the others.** The pinned section used CSS multi-column masonry, which redistributes cards across columns every time the count changes — so unpinning any pinned note visually relocated the remaining ones, breaking the user's muscle memory for "this important note is in the top-right." Fix: pinned section now renders as a **row-major CSS Grid with explicit position slots**. Each pinned card sits in the grid cell its `position` field points to, and any "gap" positions (left behind by an unpinned note that still owns position N) render as an invisible placeholder cell. Result: the cards that stay pinned never move; the unpinned card's old spot just becomes blank until that gap gets re-filled (by drag-reorder, which renumbers positions contiguously). `set_pinned` already preserves `position` (only updates `pinned`/`archived`/`updated_at`), so this works end-to-end without a Rust change.

### Changed

- **NoteGrid gains a `layout` prop** — `"masonry"` (default, used by unpinned/archive/trash/label sections — packing density still wins there since the user expects modified/created sort to reshuffle) or `"stable-grid"` (used by the pinned section). Same drag-and-drop / DndContext logic in both layouts.

## [0.18.0] — 2026-05-26 — "Theme accent + larger note text"

### Added

- **Accent color picker.** Six presets in Settings (Blue / Purple / Green / Orange / Pink / Teal) — re-skins every accent surface in the app at once: the new-note button, pin states, focus rings, theme-mode selector, archive/restore primaries, vault unlock, lock-screen submit, filter chip selection, hashtag highlights in note cards, color-picker selection rings, drawing canvas accent — all driven from a single `--keepr-accent` (+ paired `--keepr-accent-hover`) CSS variable. Blue is the original Keep accent and remains the default. Persisted to `keepr:accent-color`.
- **Note text size slider.** Settings → "Note text size" — 12-22px range, default 14px (the historic Keep size). Drives `--keepr-note-font-size`, which is applied via inline `style` to: note-card body text, note-card checklist items, editor body textarea, editor checklist items. Reset button restores the default. Persisted to `keepr:note-font-size`.

### Changed

- **All hard-coded `#1a73e8`/`#1557b0` references replaced with `var(--keepr-accent)` / `var(--keepr-accent-hover)`.** 12 components + `index.css` (focus-visible outline) now resolve their accent at render time so the picker reskins live without a reload. SVG strokes in `backgroundPatterns.ts` and the literal blue swatch in `DrawingCanvasModal`'s draw-color palette are intentionally NOT remapped — those are not "the accent", they're independent colors.

### Implementation note

- `App.tsx` mounts a single useEffect that mirrors `accentColor` + `noteFontSize` onto the document root as CSS variables. Every reference is a `var(...)` read — no per-component theme prop drilling.

## [0.17.1] — 2026-05-26 — "Drag actually sticks · 90vw×90vh editor · blurred backdrop"

### Fixed

- **Drag-to-rearrange now visibly sticks.** The v0.17.0 onDragEnd updated each note's `position` field optimistically but never re-sorted the `notes` array — so the rendered order remained whatever the previous sort key left behind, and the dragged card appeared to snap back to its original slot. Even the auto-flip to Custom sort only fired AFTER the await on `api.reorderNotes`, so there was a 50-100ms window with the wrong order even when it eventually corrected. New flow: compute the target sort mode up front (current mode if pinned-only or already Custom; otherwise `"custom"`), then apply the position patch + `sortNotes(next, targetSort)` in a single setState so the drop snaps to its final slot atomically. `setSortMode` still runs after to persist the mode flip to localStorage and emit the toast, but the re-sort it triggers is a no-op visually.

### Changed

- **Editor is now proportional to the monitor.** Was `w-[95vw] max-w-[1400px] max-h-[90vh]` — `max-h` let the modal collapse around short notes and the 1400px cap left ultrawides with empty bands. Now `w-[90vw] max-w-[1800px] h-[90vh]` — always 90% of viewport height regardless of content length (so the editor consistently feels "opened-up"), capped at 1800px on ultrawides to keep line length readable. `shadow-keep-hover` → `shadow-2xl` so the modal sits more clearly on top of the blurred backdrop.
- **All modal backdrops now use frosted-glass blur.** `.modal-backdrop` in `index.css` picks up `backdrop-filter: blur(10px) saturate(140%)`. Affects the editor, settings, history drawer, labels manager, help overlay, confirm dialog, reminder picker, and drawing canvas — premium look unified across every modal in the app.

## [0.17.0] — 2026-05-26 — "Editor goes big · pinned stays put · free drag"

### Added

- **Editor opens near-full-screen.** The modal was capped at `max-w-xl` (576px) and laid out as a single non-scrolling block, which felt cramped for anything longer than a paragraph. New layout: `w-[95vw] max-w-[1400px] max-h-[90vh] flex flex-col`. Content area (attachments → title → body → label chips) is `flex-1 overflow-y-auto`; toolbar is pinned at the bottom with `shrink-0` and a hairline top border. Body textarea bumped from `min-h-[6rem]` to `min-h-[20rem]` and now grows (`flex-1`) to fill available height when the modal is tall. Long notes scroll inside the editor without the toolbar moving.
- **Free drag-to-rearrange in every sort mode.** Previously drag was gated to Custom sort, so a user in the default Modified mode had no way to manually order their notes. Now the whole-card drag handle is active in any sort mode within the Notes section. On the first drop under Modified/Created/Title, sort auto-flips to Custom (with a one-line toast) so the drop is actually visible — under the old sort key the cards would snap back. Archive/Trash/Label sections are still drag-disabled (`reorder_notes` would corrupt the active-Notes positions, see EI-V0.5-1).

### Fixed

- **Clicking a pinned note no longer shuffles it to the front of the pinned row.** Two compounding bugs: (1) closing the editor unconditionally called `update_note`, which bumped `updated_at` even when nothing changed; (2) under Modified sort the freshly-bumped note jumped to the head of the pinned row. Fix: (1) a `payloadMatchesExisting(ex, payload)` dirty-check in `NoteEditor.close()` skips the DB round-trip when no field changed; (2) `sortNotes` now sorts the pinned subset by `position` (then `created_at`) regardless of the active sort mode, so even when a pinned note IS edited it stays in its slot. The only way to reorder pinned notes is now to drag them. Added a regression test (`pinned notes always sort by position, regardless of mode`).
- **Label chips in the editor switched from `rounded-full` to `rounded`.** Per the global "no pill backdrops" rule.

### Tests

- 86 vitest (was 85). New: `sortNotes › pinned notes always sort by position, regardless of mode`.

## [0.16.6] — 2026-05-26 — "Use the full width on wide displays"

### Fixed

- **Notes grid was capped at 1600px on wide monitors.** `App.tsx` wrapped the grid in `<div className="max-w-[1600px] mx-auto">`, so on a 1920px+ display the cards stopped at 1600px and left an empty band on both sides. Removed the `max-w-[1600px]` cap so the grid now fills all available main-content width (sidebar + chrome subtracted). At default `cardWidth = 240px` that means 4+ columns on a typical 1920px display and proportionally more on ultrawides. Use Ctrl+Wheel to widen cards if you'd rather have fewer columns.

## [0.16.5] — 2026-05-26 — "Ctrl+Wheel zoom for card size"

### Added

- **Ctrl+Wheel resizes masonry cards.** Hold Ctrl, scroll up → cards get wider (zooms in, fewer columns per row). Scroll down → cards get narrower (zooms out, more columns per row). Step is 16px per wheel notch, clamped to 160-480px, default 240px. The choice persists across launches (`keepr:card-width` in localStorage).
- Note: plain wheel still scrolls the page normally — only Ctrl+Wheel zooms. The wheel listener is bound at the window level with `passive: false` so `preventDefault()` suppresses WebView2's built-in page-zoom shortcut at the same time.

### Changed

- **NoteGrid uses `column-width` instead of `column-count`.** The old `columns-1 sm:columns-2 lg:columns-3 xl:columns-4 2xl:columns-5` Tailwind classes forced a discrete column count at each Tailwind breakpoint. Now the grid sets `style={{ columnWidth: cardWidth + 'px' }}` and the browser fits as many columns as the container can hold at that width — so resizing the window AND zooming both reflow with no JS in the loop. List mode (one-column) is unchanged.

## [0.16.4] — 2026-05-26 — "Settings modal: wider + scrollable"

### Fixed

- **Settings modal cut off at the bottom on shorter screens.** The modal container had no `max-height` and no scroll wrapper, so when AppLockSection + VaultSection + auto-backup + Takeout import + Markdown vault rows added up to more than the viewport, the bottom rows disappeared below the screen edge with no way to reach them. Container is now `max-w-2xl max-h-[90vh] flex flex-col`; the header gets `shrink-0` so it stays pinned and the body gets `overflow-y-auto` so long content scrolls inside the modal instead of overflowing the page.
- **Modal width:** bumped from `max-w-lg` (32rem) to `max-w-2xl` (42rem) — the form rows had cramped right-side labels and wrapped awkwardly at the older width.

## [0.16.3] — 2026-05-26 — "Takeout importer accepts any zip"

The Takeout importer used to require the canonical English path `Takeout/Keep/<title>.json`. That worked for Keep-only English exports but broke on three real-world cases I kept hitting:

- **Localized folder names.** Google Takeout translates the product folder for non-English accounts — `Takeout/Notizen/` (German), `Takeout/메모/` (Korean), `Takeout/Notas/` (Spanish), etc. The literal `/Keep/` substring filter dropped every note.
- **Re-zipped exports.** When users extract a Takeout, prune the parts they don't want, and re-zip the result, the top-level `Takeout/` prefix often disappears (you get `Keep/foo.json` at the archive root). The old filter required a leading `/Keep/` which doesn't match root-level `Keep/`.
- **Multi-product archives.** A `takeout-...-3-001.zip` containing Keep + Drive + Photos would have its non-Keep JSONs ignored cleanly by the path filter, but only by coincidence — a Drive folder that happened to be named `Keep` would slip through.

### Changed

- **Importer detects notes by JSON shape, not path.** `import_takeout` now reads every `.json` entry in the archive and treats it as a Keep note if `is_keep_note_shape(v)` returns true: presence of `isPinned` (bool), at least one of `createdTimestampUsec`/`userEditedTimestampUsec`, and at least one of `textContent`/`listContent`. This rejects Takeout's `Labels.json` (a top-level array — `as_object()` returns `None`), Drive/Photos metadata in multi-product exports, and any other non-Keep JSON that shares the archive. The user's actual zip (89 notes, 23 trashed → 66 imported, 15 pinned) still imports identically — but a German export with `Takeout/Notizen/...` now works too.

### Verified

- Pinning was already preserved end-to-end (`isPinned` → `NoteInput.pinned` → `INSERT INTO notes (..., pinned, ...)` in `create_note`). Added a unit test asserting `is_keep_note_shape` recognizes the canonical 2026 Takeout note shape (which includes `isPinned: true`).

### Tests

- New: `is_keep_note_shape_{accepts_canonical_takeout_note, accepts_list_only_note, rejects_takeout_labels_array, rejects_other_product_json, rejects_partial_match}`.

## [0.16.2] — 2026-05-26 — "Editor open crash fix"

P0 — the editor failed to open at all on a fresh v0.16.1 install. Clicking "Take a note" (or any of the new-note affordances, including the global hotkey + tray quick-capture) blanked the entire window with no recovery.

### Fixed

- **NoteEditor Rules-of-Hooks violation.** `useClickOutside(moreMenuRef, …)` and `const [dropActive, setDropActive] = useState(false)` were both declared *below* the `if (!editorOpen) return null;` early return in `NoteEditor.tsx`. When `editorOpen` flipped true, the component called two extra hooks compared to the previous render — React threw `Rendered more hooks than during the previous render`, the error propagated past the (missing) root-level boundary, and React 18 unmounted the entire tree. Both hooks now live above the early return; identical behaviour, but the hook-call order is stable.
  - The misplaced `useClickOutside` was added with EI-V0.5-15 (kebab "More" overflow) in v0.15.0.
  - The misplaced `useState(dropActive)` was added with the paste/drop image flow (NF-V0.5-I).

### Added

- **`<ErrorBoundary>` around the app root in `src/main.tsx`.** Future render-time exceptions render a recoverable "Something went wrong — Reload" panel with the error message instead of silently blanking the window. The full error + componentStack are logged to the console so `tauri-plugin-log` captures them too.
- **ESLint + `react-hooks/rules-of-hooks`.** No linter existed before this release — that's how the hook-ordering bug landed in v0.15.0 and stayed broken through v0.16.1. New flat config at `eslint.config.js` enables `react-hooks/rules-of-hooks` as an error (which catches the exact bug class fixed above) plus `@typescript-eslint/recommended`. New `npm run lint` script. The CI workflow's existing "Frontend lint + test + build" job now actually runs lint between `npm ci` and `npm test`. Pre-existing inconsistencies in `AttachmentGrid` / `NoteGrid` / `ReminderPicker` / `NoteEditor` cleaned up in the same pass so the suite starts at zero warnings.

## [0.16.1] — 2026-05-26 — "First published Windows binary"

Release-only patch. No code changes from v0.16.0 — this is the **first version with attached installer artifacts on the GitHub Releases page**.

### Why a new version

Tags v0.6.0 → v0.16.0 fired the `tauri-action` release workflow on every push, but the runs are all stuck in `queued` at the GitHub Actions runner-allocation step because of a SysAdminDoc account-level billing block (see [SysAdminDoc Actions billing](https://github.com/SysAdminDoc/Keepr/issues) for the wider context). No release page ever got created.

v0.16.1 was built locally and the bundles uploaded manually via `gh release create` so users can actually install the app without a from-source build.

### Artifacts

- `Keepr_0.16.1_x64-setup.exe` — NSIS installer.
- `Keepr_0.16.1_x64_en-US.msi` — Windows Installer.
- `Keepr-portable.zip` — extract anywhere, drop `portable.flag` next to `keepr.exe` for USB-stick mode.

Unsigned per the v0.5 code-signing decision (see [SECURITY.md](SECURITY.md)). Windows SmartScreen warning expected on first launch — click "More info" → "Run anyway".

## [0.16.0] — 2026-05-26 — "Refactor pass"

EI-V0.5-10 partially closed. The high-ROI piece (extract `ChecklistSection` from the 1280-line NoteEditor) shipped; the rest of the proposed mega-file split (further commands.rs subfiles, sectionising SettingsModal) was triaged as low-ROI churn and explicitly closed without action — see "Deferred as low-ROI" below.

### Changed

- **EI-V0.5-10 (partial)** Extract `<ChecklistSection>` from `NoteEditor.tsx`. The list-editor renders behind a single component that owns its own dnd-kit `DndContext`, the `useFlip` FLIP animator, and the per-row indent/dedent/setItem/removeItem/addItem helpers. NoteEditor hands it `items` + `onChange` and that's it.
  - `NoteEditor.tsx` drops from ~1280 → ~1010 lines (-21%).
  - `ChecklistRow` moves to the same new file as `ChecklistSection` since the row only exists to serve that section.
  - Unused imports (`CheckSquare`, `Square`, `GripVertical`, all of `@dnd-kit/core`, `@dnd-kit/sortable`, `@dnd-kit/utilities`, the FLIP hook) removed from `NoteEditor.tsx`.

### Deferred as low-ROI

These items were originally lumped into EI-V0.5-10 but the cost/benefit didn't justify the churn at current size. They stay open in `ROADMAP.md` under "backlog" so a future revisit is cheap; not actively scheduled.

- **commands.rs further split.** The file is ~3700 lines but cleanly organised by section (note CRUD → labels → reminders → attachments → vault → snapshots → backup → IO). Splitting into sibling modules would require ~50 import-path shuffles for ~0 reader-comprehension gain — every function in there is searchable by name. The one isolated chunk that would benefit from extraction (validate_zip_archive + zip-bomb caps) was investigated and stayed put because the three callers all live in the same file and the constants would just need to be re-imported.
- **SettingsModal sectionise.** Wrapping each thematic block in a `<SettingsSection title=…>` helper would add a header to each group, which is mild a11y polish. The existing `space-y-5` flat layout works; a screen reader walks each `<Row>` linearly which is already correct. Not worth the visual reflow.

### Tests

- **50 cargo + 85 vitest cases** unchanged — pure refactor; the ChecklistSection extraction preserves identical behaviour, and the existing checklist render tests would have caught any drift (there aren't any direct ones, but the editor's hashtag/reminder/draft tests exercise the surrounding code path).

## [0.15.0] — 2026-05-26 — "Frontend polish"

Three Phase C deferrals: react-masonry-css replaced with CSS-native multi-column layout, secondary modals code-split via `React.lazy`, and the editor toolbar gains a kebab "More" overflow menu so it stops growing.

### Changed

- **EI-10** `react-masonry-css` dropped. NoteGrid now uses CSS `columns-1 sm:columns-2 lg:columns-3 xl:columns-4 2xl:columns-5` + `break-inside-avoid` per card — produces the same masonry visual (cards fill columns top-to-bottom) with zero runtime cost, no `ResizeObserver` shim, no abandoned-since-2022 dependency. List-mode collapses to a single centered column directly in the component.
- **EI-V0.5-17** Code-split secondary modals via `React.lazy`. The initial editor + grid bundle no longer pulls in `SettingsModal`, `LabelsManager`, `HelpOverlay`, `LockScreen`, `ReminderPicker`, `HistoryDrawer`, or `DrawingCanvasModal` — each loads the first time the user opens it. Modals that already gate on an `open` flag get `fallback={null}` so there's no flash. Removes ~30-40 KB from the initial-paint critical path.
- **EI-V0.5-15** (rest) Editor toolbar kebab "More" menu. Lower-priority actions (Make a copy, Version history, Move to/out of vault) move behind a `MoreVertical` IconBtn with a popover; primary actions (Reminder, Add image, Background, Show checkboxes, Labels, Archive, Delete) stay always-visible. The toolbar is now ~7 buttons wide instead of ~10, fits comfortably in the editor's `max-w-xl` width.

### Tests

- **50 cargo tests** unchanged — pure-frontend release.
- **85 vitest cases** unchanged — render-layer refactors covered by tsc + manual smoke. The masonry change in particular is structural CSS, not logic.

## [0.14.0] — 2026-05-26 — "Checklist & textures"

Three Keep-parity items at once: sub-item indent in checklists (NF-21), the nine background patterns (NF-22), and the FLIP animation on the existing "Move checked to bottom" behaviour (NF-20 polish). Two schema bumps (v10 + v11), one new lib, one new hook, and a colour-picker that grows a pattern row when a host wants to expose patterns.

### Added

- **NF-21** Sub-item indent in checklists (1 level only, Keep parity).
  - Schema v10 adds `checklist_items.parent_id TEXT REFERENCES checklist_items(id) ON DELETE CASCADE` + `idx_checklist_parent`. Deleting a parent cascades to its children.
  - `ChecklistItem` + `ChecklistItemInput` gain `parentId`. `validate_note_input` enforces "one level only" — any item whose `parent_id` is set must reference a top-level item in the same batch.
  - `duplicate_note` two-pass copy remaps old → new ids so sub-items keep pointing at the right new parent.
  - `ChecklistRow` editor gains Tab (indent under the most recent root sibling with a stable id) and Shift+Tab (drop parent_id). Indented rows render with `pl-8`.
- **NF-22** Background image patterns.
  - Schema v11 adds `notes.background_pattern TEXT NOT NULL DEFAULT ''`.
  - New `src/lib/backgroundPatterns.ts` ships 9 inline-SVG data-URI patterns (Groceries / Food / Music / Recipes / Notes / Places / Travel / Video / Celebration) at low-contrast opacity so card text stays legible without an overlay.
  - `ColorPicker` grows an optional pattern row (with the "no pattern" Ban icon first) — opt-in per call-site via the new `patternValue` + `onPatternChange` props. The editor wires both; the card's quick-palette stays color-only for now.
  - `NoteCard` + editor body render the pattern via inline `backgroundImage: url(data:image/svg+xml…)` so there's nothing to ship as a file.
  - Rust validator + frontend `normalizePattern` both whitelist the same 10 keys; unknown values coerce to `""` instead of erroring.
- **NF-20 polish** FLIP animation.
  - New `useFlip<K>(orderKey)` hook captures pre-/post-layout rects per registered element and animates the delta with `requestAnimationFrame` + `transform: translate(-dx, -dy)` → cleared with a 200 ms transition. Honours `prefers-reduced-motion: reduce` (skips the animation).
  - `ChecklistRow` accepts a `flipRef` prop chained with dnd-kit's `setNodeRef`. The editor keys the FLIP animator on a checked-state bitmap so the animation only runs when an item flips checked/unchecked (not on every keystroke).

### Tests

- **50 cargo tests** (up from 48): 1 schema v10 test (parent_id column + ON DELETE CASCADE) + 1 schema v11 test (background_pattern column + default value).
- **85 vitest cases** (up from 82): 3 new `backgroundPatterns.test.ts` tests (order matches map; data URLs render; `normalizePattern` accepts whitelist and coerces unknowns).

## [0.13.0] — 2026-05-26 — "FTS5 search"

Replaces the renderer-side `title.toLowerCase().includes(q)` loop with a real SQLite FTS5 backend. Search now ranks by relevance (FTS5's bm25 default) instead of "first match wins", and the per-keystroke work moves off the main thread and out of the JS-iterates-every-note path.

### Added

- **EI-18** SQLite FTS5 search backend.
  - Schema v9 adds a `notes_fts` virtual table (FTS5, `unicode61 remove_diacritics 2` tokenizer) indexed on `title` + `body` + `checklist_text` (GROUP_CONCAT of the note's checklist items, kept in sync by triggers).
  - Triggers maintain the index automatically: `notes_ai_fts` / `notes_au_fts` / `notes_ad_fts` on the notes table; `ci_ai_fts` / `ci_au_fts` / `ci_ad_fts` on checklist_items rebuild the parent's `checklist_text` on any item change.
  - Migration backfills the FTS table for every existing plain note in one statement.
  - **Vault rows are intentionally NOT indexed.** The plain-text columns of a vault note are empty (the payload lives encrypted in `vault_ciphertext`), so FTS5 can't find them via title/body anyway; the migration WHERE clause makes this explicit and the trigger gates indexing on `NEW.vault = 'plain'`. The result: a locked vault can't be searched. Documented in the SECURITY threat-model section.
  - New `search_notes(query)` Tauri command returns up-to-500 matching note IDs ranked by FTS5's `rank`. Input is tokenized + double-quoted per token + suffixed with `*` (prefix match), so the query is safe against FTS5-meaningful characters (`(`, `)`, `*`, `:`, `AND`, `OR`, `NEAR`) without needing per-character escaping.
  - Frontend `filterNotes(…, searchMatchIds?)` takes an optional Set<string>. When set, narrows the section/filter pool to those IDs. When absent (browser preview / FTS5 errored / empty query), falls back to the in-memory substring scan — preserves test-suite behaviour with no Tauri runtime.
  - `TopBar`'s existing 150 ms debounce now also fires `api.searchNotes(query)` and stashes the result in `store.searchMatchIds`. Empty input clears the narrow.

### Tests

- **48 cargo tests** (up from 44): 3 new schema v9 tests (creation + insert/update propagation, vault rows not indexed, checklist change propagation) and 1 `build_fts5_query` test (quoting + prefix + FTS5 keyword neutralization).
- **82 vitest cases** (up from 80): 2 new `filterNotes` tests for the `searchMatchIds` path (Set wins over substring; section + filter still narrow first).

## [0.12.0] — 2026-05-26 — "Plumbing pass"

Infrastructure-level cleanup that doesn't change UX but tightens the operational story. Three Phase C deferrals close in one shot: structured logging via `tauri-plugin-log`, reminder schema cleanup, and clean scheduler shutdown on app exit.

### Added

- **NF-V0.5-J** `tauri-plugin-log` wired up. Writes to `<app_log_dir>/Keepr.log` (e.g. `%LOCALAPPDATA%\com.sysadmindoc.keepr\logs\Keepr.log` on Windows, `~/Library/Logs/com.sysadmindoc.keepr/` on macOS, `$XDG_DATA_HOME/com.sysadmindoc.keepr/logs/` on Linux). Rotates at 1 MiB, keeps one `.old`. Mirrors to stdout in `tauri dev`. Reminder scheduler and notification failures now use `log::warn!` / `log::info!` instead of `eprintln!`.
- New `get_log_dir` Tauri command. Settings → new "Log folder" row shows the resolved path with a Copy-path button (avoids pulling in the shell/opener plugin for a one-click reveal).

### Changed

- **EI-V0.5-14** Reminder schema cleanup. Schema v8 rebuilds the `reminders` table to use `note_id` as the primary key directly (one reminder per note made the separate `id` column redundant since v0.4). Adds a `CHECK (length(fire_at) > 0)` so a future bug can't land an empty timestamp. Migration uses the SQLite table-rebuild pattern (CREATE → INSERT … SELECT → DROP → RENAME); idempotent like every prior migration.
  - `Reminder.id` removed from both the Rust struct and the TypeScript interface. Callers use `noteId` (the only identity that ever made sense).
  - `mark_reminder_fired(state, note_id, fired_at)` — renamed parameter; all SQL `WHERE id = ?1` clauses become `WHERE note_id = ?1`. Scheduler thread emits `keepr://reminder-fired` with the note id payload as before.
- **EI-V0.5-12** Scheduler shutdown via cancellation flag. `AppState` gains `shutdown: Arc<AtomicBool>`. The reminder thread checks it at the top of every iteration and between 1-second sleep slices (so exit takes ≤ 1 second to propagate instead of waiting up to 30). `tauri::RunEvent::ExitRequested` sets the flag — Tauri's run loop now uses `.build().run(|app, event| …)` instead of the inline `.run(generate_context!())` to install the handler.

### Tests

- **44 cargo tests** (up from 43): 1 schema v8 migration test verifies the rebuilt `reminders` table has `note_id` as PK + no `id` column + the `fire_at` CHECK rejects empty strings. Six existing reminder integration tests updated to drop the `id` column from their seed INSERTs.
- **80 vitest cases** unchanged — `Reminder.id` removal landed without any test churn beyond the existing reminders.test.ts factory.

## [0.11.0] — 2026-05-26 — "Drawing notes"

Final roadmap item from the v0.5 research pass. The Paintbrush button on NewNoteBar — disabled since v0.2 with the "coming v0.5" tooltip — now opens a real drawing canvas. Strokes are tracked vector-side for proper hi-DPR rendering + undo, then flattened to a PNG attachment on save (matching the same attachment pipeline image-paste uses).

### Added

- **NF-V0.5-E** Drawing notes.
  - New `<DrawingCanvasModal />` (~280 lines) with HTML5 canvas + PointerEvents. Pen pressure read off `PointerEvent.pressure` so Surface/Wacom strokes vary in width while a mouse renders uniform. `touch-none` on the canvas prevents the browser from panning the page on tablets.
  - Toolbar: 8-color palette (Keep-shaped: ink/red/orange/yellow/green/blue/purple/white), three stroke sizes (2/5/12 px), a dedicated Eraser tool (paints the canvas background colour so the PNG flattens cleanly), Undo (stroke-level), Clear, Save, Cancel.
  - Strokes are kept as `{color, size, erase, points[]}` arrays in a ref; the canvas repaints on every pointer event by replaying the stroke buffer. The backing-store resolution is matched to `devicePixelRatio` so strokes stay crisp on retina displays.
  - Save: `canvas.toBlob("image/png")` → `Uint8Array` → existing `add_image_attachment_bytes` Rust command. The new note is created blank, the PNG is attached as an `image/png` attachment named `drawing.png`, and the editor opens on the new note so the user can immediately add a title / labels / reminder.
  - NewNoteBar's Paintbrush button is now enabled (and the "coming v0.5" tooltip is gone). Re-editing an existing drawing is intentionally out of scope for this cut — vector replay can land alongside SVG storage in a later release.

### Tests

- **43 cargo tests** unchanged — no Rust-side changes for this item; reuses the existing image attachment pipeline (NF-V0.5-I bytes path).
- **80 vitest cases** unchanged — the new surface is pure DOM/canvas, covered by tsc + manual smoke. Headless canvas testing would require jsdom + canvas polyfill that aren't worth pulling in for a single component.

## [0.10.0] — 2026-05-26 — "Cross-platform CI"

Extends the GitHub Actions release pipeline to also produce macOS and Linux artifacts on every `v*.*.*` tag push. Windows stays the **supported** channel; macOS and Linux are **best-effort** so the codebase stays buildable on those platforms and so users can self-build without setting up the Tauri toolchain locally.

### Added

- **NF-V0.5-K** Cross-platform CI matrix.
  - `.github/workflows/release.yml` rewritten as a matrix job over Windows / macOS-aarch64 (M1+) / macOS-x86_64 (Intel) / Ubuntu-22.04 (x86_64). `fail-fast: false` so a Mac toolchain hiccup doesn't kill the Windows + Linux builds.
  - Linux step installs the WebKitGTK 4.1 + GTK3 + libsoup3 + libayatana-appindicator + librsvg2 + patchelf chain that tauri-action needs but doesn't preinstall.
  - `src-tauri/tauri.conf.json` `bundle.targets` extended to `["nsis", "msi", "dmg", "deb", "appimage"]` — Tauri's bundler picks the relevant subset per-OS, so the same config drives every platform.
  - tauri-action publishes all artifacts to a single draft Release; the release body lists per-OS instructions including the macOS "right-click → Open" + `xattr -d com.apple.quarantine` workaround for unsigned-and-unnotarized builds.
  - Windows portable-zip step is gated on `matrix.os == 'windows-latest'` so the Mac/Linux runners don't try to run the PowerShell packaging.
- **README** Install section reorganised into Windows / macOS / Linux blocks with the exact artifact names and the per-platform quirks (SmartScreen, Gatekeeper quarantine, glibc-2.35 floor on Linux).

### Tests

- **43 cargo tests** unchanged — the CI matrix change is a workflow rewrite + config metadata, not new application code. The matrix itself is the verification: a tag push that fails to build on any platform fails the release.
- **80 vitest cases** unchanged.

## [0.9.0] — 2026-05-26 — "History & Calendar"

Two Phase E items in one shot: per-note version history with one-click restore (NF-V0.5-D) and iCalendar export of active reminders (NF-V0.5-G). Both ship the missing-bit-of-trust Trash alone couldn't cover and let users see their reminders in their existing calendar without writing any sync glue.

### Added

- **NF-V0.5-D** Note version history.
  - Schema v7 adds `note_snapshots(id, note_id, kind, title, body, color, pinned, checklist_json, vault, vault_ciphertext, taken_at)` plus a `note_snapshots_trim_to_20` trigger that caps each note's history to the most recent 20 snapshots after every insert.
  - `update_note` snapshots the prior state before applying changes; `restore_snapshot` snapshots the current state first so the restore itself is undoable. Vault rows snapshot their ciphertext as-is (no DEK required for the history path) and restore puts the ciphertext back into place.
  - New Tauri commands: `list_snapshots(note_id)` returns the chronologically-newest-first list; `restore_snapshot(snapshot_id)` swaps the row back.
  - New `<HistoryDrawer />` opens from a History toolbar button in NoteEditor — shows relative timestamps ("3 minutes ago" / "2 days ago"), a 6-line body preview, checklist-item count, and a per-row Restore button. Vault snapshots show a "ciphertext" pill and a generic "Encrypted vault payload" message instead of the raw ciphertext.
- **NF-V0.5-G** ICS export of reminders.
  - New `export_reminders_ics(dest)` Tauri command writes every active (non-fired, non-dismissed) reminder as an RFC 5545 VCALENDAR with one VEVENT per reminder. Effective fire time honours `snooze_until` over `fire_at`; recurring reminders carry their RRULE through. Vault note titles export as "Keepr — locked vault note" so the calendar import doesn't leak the encrypted title.
  - Settings → "Export reminders as iCalendar (.ics)…" button picks a destination and writes the file via a save dialog.

### Tests

- **43 cargo tests** (up from 40): 1 schema v7 migration test (table + trigger + 25→20 trim behaviour), 1 ICS UTC-offset roundtrip, 1 ICS special-character escape coverage.
- **80 vitest cases** unchanged — the new surface is a Settings button + a Drawer + Rust-mediated state, all covered by tsc + manual smoke.

## [0.8.0] — 2026-05-26 — "Private Vault"

Closes the second half of NF-V0.5-C. Vaulting a note encrypts its
title + body + checklist with XChaCha20-Poly1305 under a password-derived
data key (Argon2id KDF → KEK → wraps DEK). The vault password is
separate from the App Lock PIN and is **not** recoverable — losing it
makes the vaulted notes unreadable forever.

### Added

- **NF-V0.5-C (Private Vault)** — schema v6 adds `notes.vault` ('plain'|'vault', CHECK-constrained) and `notes.vault_ciphertext` (hex-encoded `nonce(24) || aead(ct+tag)`). Vault wrap material lives in `app_settings` under three hex keys (`vault_kdf_salt`, `vault_dek_nonce`, `vault_dek_wrapped`).
- New `src-tauri/src/vault.rs` — XChaCha20-Poly1305 AEAD with note-id as AAD (cross-row swap fails verification), Argon2id KEK at the same parameters as App Lock (m=64MiB, t=3, p=1), `Dek` newtype with `Drop`+`Zeroize` so the unlocked key is wiped from memory on lock or app exit.
- Tauri commands: `init_vault`, `unlock_vault`, `lock_vault`, `change_vault_password` (re-wraps without re-encrypting notes), `get_vault_status`, `move_note_to_vault`, `move_note_out_of_vault`. `update_note` now encrypts in place for vault rows; `duplicate_note` refuses to clone a vault note (would silently drop encrypted content).
- `list_notes` + `get_note` are vault-aware: when the DEK is loaded, vault rows decrypt server-side and return as if plaintext; when locked, they return empty title/body/checklist + `vault: "vault"` so the renderer shows a "🔒 Locked vault note" placeholder.
- Frontend: `<VaultSection />` in Settings with three modes (Setup / Unlock / Unlocked-with-change-password + Lock-now). NoteEditor gains a Lock/Unlock toolbar button (visible only when the vault is initialized + unlocked). NoteCard renders the locked placeholder + blocks click-to-open with a "Unlock the vault in Settings" toast. NoteCard adds a "Vaulted" badge for unlocked vault notes so the user sees encryption status at a glance.
- App Lock idle fire (or "Lock now") now also calls `lock_vault()` so a Keepr left on a stolen laptop can't have its DEK extracted via the backend.

### Security

- **SECURITY.md** Private Vault section pending — the "App Lock" section already calls out that disk-level reads bypass the UI gate; with v0.8.0, vaulted notes survive a `keepr.db` exfiltration because the DEK is wrapped and the password isn't on disk. Threat model otherwise unchanged.

### Tests

- **40 cargo tests** (up from 30): 9 from new `vault.rs` (init+unlock roundtrip, wrong password returns None, empty password rejected, rewrap preserves DEK, note encrypt/decrypt roundtrip, wrong-note-id AAD failure, tampered-ciphertext AAD failure, hex roundtrip, hex rejects malformed input) + 1 schema v6 migration test (vault column defaults + CHECK constraint).
- **80 vitest cases** (unchanged) — vault frontend covered by existing typecheck + manual smoke; renderer-side store changes are simple state mirrors of the Rust commands.

## [0.7.0] — 2026-05-26 — "App Lock"

Adds the first half of NF-V0.5-C — a PIN-gated lock screen with idle auto-lock — and lays the `app_settings` table schema that Private Vault (per-note at-rest encryption) will plug into. Argon2id PHC for the PIN (m=64MiB, t=3, p=1); see SECURITY.md for the full threat model and the explicit lost-PIN policy (no recovery).

### Added

- **NF-V0.5-C** App Lock (Private Vault deferred to v0.7.1+).
  - Schema v5 adds `app_settings(key, value)`. Two keys used: `app_lock_pin_phc` (Argon2id PHC, absent when disabled) and `app_lock_after_minutes` (idle minutes).
  - New `src-tauri/src/lock.rs` wrapping the `argon2` + `password-hash` crates. PHC string captures algo + version + params + salt + hash, so one column stores everything `verify_pin` needs.
  - New Tauri commands: `enable_app_lock`, `disable_app_lock`, `verify_app_lock_pin`, `set_app_lock_minutes`, `get_app_lock_settings`. Argon2id verification runs ~150-300 ms on commodity hardware — the LockScreen surfaces a "Verifying…" busy state.
  - New `<LockScreen />` overlay (z-index 100, full-window) with PIN input, show/hide toggle, busy state, error display, and a "no recovery" footer linking to SECURITY.md.
  - New `useIdleLock(idleMinutes, onIdle, active)` hook that rearms on `mousedown` / `keydown` / `touchstart` / `pointerdown` / `scroll` / `visibilitychange`. Floor of 1 minute so a user can never accidentally set themselves to lock immediately.
  - Settings → new "App Lock" section (`AppLockSection.tsx`) with three states: not configured (Enable form with PIN + Confirm + minutes), configured (Lock-after selector + "Lock now" + Disable form requiring current PIN), busy (Argon2id chewing).
  - App auto-locks on launch when `appLockEnabled === true` so a user closing Keepr from the tray and reopening it later still hits the lock screen.

### Security

- **SECURITY.md** gains an "App Lock (v0.7.0+)" section spelling out the UI-gate-not-at-rest-encryption boundary and the no-PIN-recovery design choice. The "stolen laptop" item under "threats we do **not** defend against" rewritten to reflect what App Lock actually changes.

### Tests

- **30 cargo tests** (up from 24): 5 from new `lock.rs` (hash + verify roundtrip, wrong-PIN rejection, empty-PIN rejection, Argon2id variant assertion, malformed-PHC error vs wrong-PIN) and 1 schema v5 migration test.
- **80 vitest cases** (up from 75): new `useIdleLock.test.ts` (5) covers timer fire, rearm-on-keydown, active=false no-op, idleMinutes < 1 floor, and event-subscription/teardown.

## [0.6.0] — 2026-05-26 — "Reminders v2"

Adds recurrence + snooze to the reminder system, a dedicated Reminders section in the sidebar, and an in-app fire toast so reminders aren't lost when the OS coalesces notifications. The schema gains an optional `rrule` column (whitelisted to `FREQ=DAILY|WEEKLY|MONTHLY|YEARLY`) and `snooze_until` is now honoured by the scheduler.

### Added

- **NF-V0.5-A** Reminders v2.
  - `set_reminder(note_id, fire_at, rrule)` accepts an optional whitelisted RRULE. New `snooze_reminder(note_id, until)` Tauri command. Rust-side `next_fire_at(prev, rrule)` advances on each fire (chrono `Duration` + `Months` with leap-day clamp); single-shot reminders behave exactly as before.
  - `mark_reminder_fired` now advances `fire_at` for recurring reminders (leaving `fired_at` NULL so the bell stays lit) and only sets `fired_at` for single-shot ones.
  - Sidebar gains a **Reminders** entry under Notes with a live count of active reminders (excluding fired single-shots and dismissed). The section orders notes by next-due ascending, honouring `snooze_until` when later than `fire_at`.
  - `ReminderPicker` extended with a recurrence dropdown (None / Daily / Weekly / Monthly / Yearly), a Snooze panel (10 min / 1 hour / Tomorrow / Custom) that only appears while editing an existing reminder, and a "Remove reminder" footer.
  - NoteCard reminder badge shows the effective fire (snooze-aware) and the recurrence label inline (`Tomorrow, 8:00 AM · daily`).
  - In-app toast on `keepr://reminder-fired` with a "View note" action that opens the editor — covers the case where the OS suppresses notifications for the focused window.

### Tests

- **24 cargo tests** (up from 20): 4 new — `next_fire_at_handles_supported_rrules`, `validate_rrule_rejects_unknown`, `mark_reminder_fired_advances_recurring`, `mark_reminder_fired_single_shot_sets_fired_at`.
- **75 vitest cases** (up from 69): new `reminders.test.ts` covers the four helpers (`effectiveFireAt`, `isActive`, `recurrenceLabel`, `compareByDue`) and the Reminders-section variant of `filterNotes`.

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
