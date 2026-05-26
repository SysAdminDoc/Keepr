# Keepr

A pixel-close, offline-first clone of Google Keep. Built with Tauri 2 + React + Rust + SQLite. Your notes stay on your machine; back them up to any folder you sync to a cloud provider.

## Why

Google Keep is great until the internet goes out. Keepr gives you the same look, the same masonry-grid card UX, the same colors and labels and checklists — running entirely locally. Back up to Google Drive (or anywhere) with a one-click ZIP export, restore with a one-click import.

## Features

- Card masonry grid (pinned + others sections)
- Text notes and checklists (text ↔ checklist round-trips losslessly via GFM-style `- [x]` markers)
- 12 Keep colors (light + dark variants)
- Labels with chip filtering
- Search across title, body, and checklist items
- Archive and Trash with restore
- Manual ZIP export / import with zip-slip + zip-bomb defenses and a `.prev` snapshot the restore can roll back to
- Light / dark theme
- Native Windows installer (`.msi` + `.nsis`); see Roadmap for the upcoming portable-EXE bundle

## Where Keepr stores your data

Keepr uses Tauri's per-app data directory:

- **Windows:** `%APPDATA%\com.sysadmindoc.keepr\keepr.db` (SQLite, WAL mode)
- **macOS / Linux:** the equivalent OS-conventional path (`~/Library/Application Support/com.sysadmindoc.keepr/` on macOS; `$XDG_DATA_HOME/com.sysadmindoc.keepr/` on Linux). macOS and Linux builds are not yet shipped.

The schema is versioned (`PRAGMA user_version`), so a newer Keepr can upgrade an older database in place. A backup is just a regular ZIP — `keepr.db` at the root plus any future attachments.

### Portable mode

Drop an empty file named `portable.flag` next to `keepr.exe`. On startup Keepr detects the sentinel, writes `keepr.db` (and any attachments) **in the same folder as the EXE**, and never touches `%APPDATA%`. Copy the folder to a USB stick, run from any Windows box, your notes travel with you. Remove the file to go back to per-user storage.

## Roadmap & changelog

- [ROADMAP.md](ROADMAP.md) — the live task list
- [CHANGELOG.md](CHANGELOG.md) — what shipped in each release
- [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md) — the long-form research that backs the roadmap

## Install

Pick one of the published builds from [Releases](https://github.com/SysAdminDoc/Keepr/releases):

- **`Keepr_<version>_x64-setup.exe`** — NSIS installer (default).
- **`Keepr_<version>_x64_en-US.msi`** — Windows Installer alternative.
- **`Keepr-portable.zip`** — extract anywhere, run `keepr.exe`. The bundled `portable.flag` makes the app store `keepr.db` next to the EXE so it travels on a USB stick.

Keepr is unsigned today (see [SECURITY.md](SECURITY.md) for rationale). First launch shows Windows SmartScreen — click "More info" → "Run anyway".

## Build from source

Prereqs: Node 20+, Rust 1.80+. The Tauri CLI is bundled as a dev-dependency — no global install.

```sh
npm install
npm run tauri dev          # dev (HMR)
npm run tauri build        # release MSI/NSIS in src-tauri/target/release/bundle/
npm test                   # vitest (frontend)
cargo test --manifest-path src-tauri/Cargo.toml --lib   # rust unit tests
```

The GitHub Actions workflow at [`.github/workflows/release.yml`](.github/workflows/release.yml) builds + uploads a release on every `v*.*.*` tag push.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Issues + PRs welcome.

## Security

See [SECURITY.md](SECURITY.md) for the threat model and how to report vulnerabilities.

## License

MIT. See [LICENSE](LICENSE).
