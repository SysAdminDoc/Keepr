# Keepr

![Version](https://img.shields.io/badge/version-0.26.0-blue)
![License](https://img.shields.io/badge/license-MIT-green)
![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey)

A pixel-close, offline-first clone of Google Keep. Built with Tauri 2 + React + Rust + SQLite. Your notes stay on your machine; back them up to any folder you sync to a cloud provider.

## Why

Google Keep is great until the internet goes out. Keepr gives you the same look, the same masonry-grid card UX, the same colors and labels and checklists — running entirely locally. Back up to Google Drive (or anywhere) with a one-click ZIP export, restore with a one-click import.

## Features

**Capture & edit** — Text notes and checklists, lossless `setKind` round-trip via GFM `- [x]` markers, multi-image attachments (paste / drop / file pick), inline `#hashtag` labels (auto-create + auto-detach on text removal), time-based reminders with native Windows toast, "Make a copy" duplicate.

**Organize** — 12 Keep colors (light + dark paired variants), labels with chip filtering + per-label note counts in sidebar, pin/archive/trash with configurable retention + days-left badge.

**Find & view** — Debounced search across title/body/checklist, filter chips (type / color / label / pinned), grid + list view modes, sort by Modified / Created / Title / Custom (drag-reorder in Custom mode).

**Power user** — Keep's canonical keyboard shortcuts (`c` / `l` / `/` / `?` / `j` / `k` / `f` / `e` / `#` / `Ctrl+G` / `Ctrl+A`) with a `?` help overlay, multi-select + bulk actions (pin / color / labels tri-state / archive / trash / restore / delete forever).

**System integration** — System-tray icon with show-hide + new-note + quit menu, `Ctrl+Alt+N` global hotkey quick-capture, single-instance guard.

**Backup & migration** — Manual ZIP export / import with zip-slip + zip-bomb defenses and `.prev` rollback, auto-backup schedule (daily / weekly to your Drive folder), Markdown vault export/import (one `.md` per note + YAML frontmatter + `_resources/`), Google Takeout import (preserves chronology + reminders + labels + attachments).

**Document scanner** — Capture from webcam or pick an existing photo. OpenCV WASM auto-detects document edges, draggable corner handles let you adjust the crop, perspective warp straightens the page, and enhancement filters (Color, Enhanced, Grayscale, B&W) clean up the result. The ~4 MB WASM payload loads lazily on first use.

**Speech** — Voice-note transcription runs locally with whisper.cpp after an explicit one-time model download. Keepr shows the source URL and expected SHA-256 digest before download, then refuses to run a model that fails verification.

**Theme** — Light / Dark / System (follows OS), masonry grid, full keyboard accessibility, WCAG AAA contrast across all 24 color combinations.

**Distribution** — Unsigned NSIS / MSI installer, portable `.zip`, and Web Clipper ZIP built locally and attached to GitHub Releases. The clipper supports toolbar and right-click article / selection / link capture. See [Install](#install).

## Where Keepr stores your data

Keepr uses Tauri's per-app data directory:

- **Windows:** `%APPDATA%\com.sysadmindoc.keepr\keepr.db` (SQLite, WAL mode)
- **macOS:** `~/Library/Application Support/com.sysadmindoc.keepr/keepr.db` (best-effort builds since v0.10).
- **Linux:** `$XDG_DATA_HOME/com.sysadmindoc.keepr/keepr.db` (best-effort builds since v0.10).

The schema is versioned (`PRAGMA user_version`), so a newer Keepr can upgrade an older database in place. A backup is just a regular ZIP — `keepr.db` at the root plus attachment resources under `resources/`.

## Network behavior

Keepr makes no background outbound network requests. The only app-initiated outbound HTTP path is **Settings → Voice transcription → Download model**, which fetches the whisper.cpp model from Hugging Face after a user click and verifies its SHA-256 digest before use. After the model is on disk, transcription is fully local. The Web Clipper binds to localhost only and does not contact hosted services.

### Portable mode

Drop an empty file named `portable.flag` next to `keepr.exe`. On startup Keepr detects the sentinel, writes `keepr.db` (and any attachments) **in the same folder as the EXE**, and never touches `%APPDATA%`. Copy the folder to a USB stick, run from any Windows box, your notes travel with you. Remove the file to go back to per-user storage.

## Roadmap & changelog

- [ROADMAP.md](ROADMAP.md) — the live task list
- [CHANGELOG.md](CHANGELOG.md) — what shipped in each release
- [`docs/research-archive/`](docs/research-archive/) — archived long-form research that backs prior roadmap cycles

## Install

Pick one of the published builds from [Releases](https://github.com/SysAdminDoc/Keepr/releases):

**Windows (supported)**

- **`Keepr_<version>_x64-setup.exe`** — NSIS installer (default).
- **`Keepr_<version>_x64_en-US.msi`** — Windows Installer alternative.
- **`Keepr-portable.zip`** — extract anywhere, run `keepr.exe`. The bundled `portable.flag` makes the app store `keepr.db` next to the EXE so it travels on a USB stick.

Keepr is unsigned today (see [SECURITY.md](SECURITY.md) for rationale). First launch shows Windows SmartScreen — click "More info" → "Run anyway".

**Browser extension**

- **`Keepr-Web-Clipper-<version>.zip`** — primary Web Clipper package. Extract to a permanent folder, open `chrome://extensions` or `edge://extensions`, enable Developer mode, click **Load unpacked**, and select the extracted folder.
- **`Keepr-Web-Clipper-<version>.crx`** — secondary artifact for enterprise/manual tooling. Modern Chrome/Edge reject self-hosted CRX drag-and-drop installs, so use the ZIP for normal installs.

**macOS (best-effort, v0.10+)**

- **`Keepr_<version>_aarch64.dmg`** — Apple silicon (M1/M2/M3).
- **`Keepr_<version>_x64.dmg`** — Intel.

Unsigned and not notarized. macOS will refuse to launch on first try — right-click the app → Open → confirm, or run `xattr -d com.apple.quarantine /Applications/Keepr.app` to clear the quarantine bit.

**Linux (best-effort, v0.10+)**

- **`keepr_<version>_amd64.deb`** — Debian/Ubuntu.
- **`Keepr_<version>_amd64.AppImage`** — distro-agnostic; `chmod +x` then run.

Built against Ubuntu 22.04 / glibc 2.35. Older distros may need to self-build from source.

## Build from source

Prereqs: Node 20+, Rust 1.80+. The Tauri CLI is bundled as a dev-dependency — no global install.

```sh
npm install
npm run tauri dev          # dev (HMR)
npm run tauri build        # release MSI/NSIS in src-tauri/target/release/bundle/
npm run build:store-installer  # Store-flavored MSI/NSIS with offline WebView2 installer
npm run smoke              # temp DB/storage/vault/clipper/backup smoke
npm test                   # vitest (frontend)
cargo test --manifest-path src-tauri/Cargo.toml --lib   # rust unit tests
```

Releases are built locally on this machine and attached to GitHub Releases manually.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Issues + PRs welcome.

## Security

See [SECURITY.md](SECURITY.md) for the threat model and how to report vulnerabilities.

## License

MIT. See [LICENSE](LICENSE).
