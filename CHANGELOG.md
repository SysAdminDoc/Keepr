# Changelog

All notable changes to Keepr are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning is [SemVer](https://semver.org/spec/v2.0.0.html).

## Unreleased

(See [ROADMAP.md](ROADMAP.md) for the live task list.)

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
