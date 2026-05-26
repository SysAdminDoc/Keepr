# Keepr

A pixel-close, offline-first clone of Google Keep. Built with Tauri 2 + React + Rust + SQLite. Portable Windows EXE — no installer, no internet, your notes stay on your machine.

## Why

Google Keep is great until the internet goes out. Keepr gives you the same look, the same masonry-grid card UX, the same colors and labels and checklists — running entirely locally. Back up to Google Drive (or anywhere) with a one-click ZIP export, restore with a one-click import.

## Features

- Card masonry grid (pinned + others sections)
- Text notes and checklists
- 12 Keep colors (light + dark variants)
- Labels with chip filtering
- Search across title, body, and checklist items
- Archive and Trash with restore
- Manual ZIP export / import (point at your Google Drive sync folder)
- Light / dark theme
- Single portable EXE, ~8 MB
- SQLite-backed local store at `%APPDATA%\Keepr\keepr.db` (override via Settings)

## Roadmap

See [ROADMAP.md](ROADMAP.md).

## Build from source

Prereqs: Node 20+, Rust 1.80+, Tauri CLI 2.x.

```sh
npm install
npm run tauri dev          # dev (HMR)
npm run tauri build        # release EXE in src-tauri/target/release/
```

## License

MIT. See [LICENSE](LICENSE).
