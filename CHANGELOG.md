# Changelog

All notable changes to Keepr are documented here. Format loosely follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). Versioning is [SemVer](https://semver.org/spec/v2.0.0.html).

## Unreleased

(See [ROADMAP.md](ROADMAP.md) for the live task list.)

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

### Known issues at release (tracked in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md))
- `import_zip` was vulnerable to zip-slip; export skipped uncheckpointed WAL pages — both addressed in the v0.2 hardening pass.
- No schema-version table — added in v0.2.
- Tauri CSP was `null` and `fs:*` capabilities were broadly granted but unused — tightened in v0.2.
- N+1 query pattern in `list_notes`, no tests, no CI — addressed in v0.2.
