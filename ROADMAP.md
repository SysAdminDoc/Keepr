# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md); rationale for each cycle's choices is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with anything in the archive, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-06-27):** v0.25.0 ships content-addressed attachment resources. Blocked signing/biometric/notarization items live in [Roadmap_Blocked.md](Roadmap_Blocked.md). This file lists only actionable open work.

---

## Open: Larger bets (v0.25.x and later)
- [ ] **P3 — MSIX packaging + Microsoft Store** — free signing, Windows Share Target contract, auto-update via Store.
- [ ] **P3 — Document scanner** (OpenCV WASM, ~7 MB renderer payload) — Apple Notes parity, lower-priority capture path.
- [ ] **P3 — Optional LAN-only P2P sync** (mDNS + Yjs CRDT, Anytype model) — only sync model compatible with "no cloud server" non-goal.

---

## Won't ship (rescoped from prior research)

- **NF-12 — Image OCR**: requires per-platform OCR backends; bundling a multi-MB engine into every build fails the cross-platform feature-parity bar. Users can paste OS-extracted text instead.
- **NF-13 — Rich URL preview cards**: requires outbound HTTP. Directly contradicts the "no outbound network requests" promise.

---

## Explicit non-goals (binding)

Carried forward across every research cycle:

- Collaboration / real-time co-edit — single-user only
- Cloud sync server (Keepr-hosted) — BYO-cloud-folder only
- Cloud model features / remote retrieval / autocomplete / hosted transcription (anything that ships audio, text, or embeddings to a remote service). Local offline inference (e.g. whisper.cpp for voice notes) is in-bounds — same offline-first / no-account / no-telemetry rules as the rest of Keepr.
- Account / sign-in
- Telemetry
- Folders / hierarchy (labels-only is Keep identity; nested tags also rejected)
- Outliner / block editing
- Feature paywall (MIT, never fragment)
- User scripts attached to notes (sandboxing nightmare)
- Custom protocol expansion to arbitrary file types
- Built-in cloud-sync without user-managed credentials
- Hosted Web Clipper (vs. localhost) — would require outbound HTTP
- In-app extension marketplace / eval()-based plugin API
- Anything that requires admin/elevation
- Location-based reminders (battery-hungry; Google deprecated theirs)
- Markdown editor replacing the plain-text editor (Markdown is for export only)

---

## Resolved decisions

- **Code-signing (v0.5+):** ship unsigned with SmartScreen workaround until Azure Trusted Signing subscription approved.
- **macOS / Linux support tier (v0.10+):** Windows is supported; macOS + Linux are best-effort.
- **App Lock + Private Vault lost-credential policy:** no recovery for App Lock or default Vault. **(Updated v0.21.1: opt-in recovery seed for Vault.)**
- **Reminder scheduler granularity:** 30-second poll. Documented up-to-30-s lag acceptable.
- **Voice transcription scope (v0.22.4):** local offline whisper.cpp is in-bounds; cloud transcription remains banned.

## Research-Driven Additions

- [ ] P1 - Replace hardcoded Settings footer version
  Why: Settings still displays `Keepr v0.16.1` while package/app metadata is v0.25.0.
  Evidence: `src/components/SettingsModal.tsx`; `package.json`; `src-tauri/tauri.conf.json`.
  Touches: `src/components/SettingsModal.tsx`, `src/api.ts`, `src-tauri/src/commands/io.rs` or a shared generated version constant.
  Acceptance: Settings displays the current app version from one source of truth, and a version bump updates README badge, package metadata, Tauri config, and Settings without manual string edits.
  Complexity: S

- [ ] P1 - Harden speech model provenance and network disclosure
  Why: Keepr now has one explicit outbound download path, but `SECURITY.md` still says no outbound network and the model is verified with SHA-1.
  Evidence: `SECURITY.md`; `src-tauri/src/transcribe.rs::MODEL_SHA1_HEX`; `src-tauri/src/transcribe.rs::download_model`; RustSec/source review.
  Touches: `src-tauri/src/transcribe.rs`, `src/components/VoiceTranscriptionSection.tsx`, `README.md`, `SECURITY.md`, tests for model digest failures.
  Acceptance: model verification uses SHA-256 or stronger, the UI/logs show source URL and expected digest, failed verification gives a recovery path, and docs state that Keepr makes no background network requests except the opt-in model download.
  Complexity: M

- [ ] P1 - Package Web Clipper as a release artifact
  Why: The clipper is developer-mode only, has no packaging script, and its manifest version is v0.1.0 while Keepr is v0.25.0.
  Evidence: `web-clipper/manifest.json`; `web-clipper/README.md`; Chrome MV3 packaging constraints; Chrome-extension memory guidance.
  Touches: `web-clipper/manifest.json`, `web-clipper/README.md`, release scripts/docs, `README.md`.
  Acceptance: a local build creates a POSIX-path ZIP for Load unpacked install, optionally creates a secondary CRX3 for enterprise/manual tooling, verifies archive contents/icons/manifest load, and documents install/update steps in the release notes.
  Complexity: M

- [ ] P1 - Add Web Clipper context-menu and article capture modes
  Why: Joplin, Notesnook, and Obsidian make clipping available from right-click flows and richer page extraction; Keepr currently clips from the toolbar with a 4 KB text snippet.
  Evidence: `web-clipper/background.js`; `web-clipper/popup.js`; Joplin Web Clipper; Notesnook Web Clipper; Chrome `contextMenus` and `scripting` APIs.
  Touches: `web-clipper/background.js`, `web-clipper/popup.js`, `web-clipper/manifest.json`, `src-tauri/src/web_clipper.rs`, Web Clipper README.
  Acceptance: right-click actions save page, selection, and link; full-page mode produces readable Markdown with source URL and labels; payload caps and bearer auth remain enforced; Firefox/Chrome behavior is smoke-tested.
  Complexity: M

- [ ] P2 - Add local end-to-end smoke coverage for desktop and clipper flows
  Why: unit tests cover many helpers, but the trust-critical workflows are desktop/browser integration paths.
  Evidence: `npm test`; `cargo test --lib`; Tauri WebDriver docs; current clipper/localhost architecture.
  Touches: test harness/scripts, `src-tauri/src/bin/keepr-verify.rs`, `web-clipper/`, README test commands.
  Acceptance: a local smoke command creates a temp data dir, launches Keepr, creates a note, attaches an image, exercises vault lock/unlock behavior, exports/restores a backup, and posts a clip through the localhost server.
  Complexity: L

- [ ] P2 - Import Markdown vault folders
  Why: Keepr exports Markdown vaults, but migration is one-way; Obsidian/Joplin users expect folder import with resources.
  Evidence: `src-tauri/src/commands/io.rs::export_vault`; README migration claims; Joplin attachments docs; Obsidian local-vault model.
  Touches: `src-tauri/src/commands/io.rs`, `src-tauri/src/commands/attachments.rs`, `src/components/SettingsModal.tsx`, tests with Markdown frontmatter and `_resources`.
  Acceptance: importing a folder of `.md` files with YAML frontmatter and sibling resources creates notes, labels, colors where present, attachments, and a collision report without deleting existing notes.
  Complexity: L

- [ ] P2 - Plan the React/Vite/Tailwind/lucide upgrade lane
  Why: `npm outdated --long` shows React 19, Vite 8, Tailwind 4, lucide 1.x, and TypeScript 6 migration work; npm production advisories are clean, so this can be staged deliberately.
  Evidence: `package.json`; `package-lock.json`; `npm audit --omit=dev`; `npm outdated --long`; stack-javascript memory warning about Zustand version drift.
  Touches: `package.json`, `package-lock.json`, `vite.config.ts`, `tailwind.config.js`, `src/index.css`, visual regression screenshots, unit tests.
  Acceptance: upgrade plan lands in small commits with a green lint/test/build after each tier; Zustand remains pinned until the prior ESM/Vitest regression has a dedicated guard test.
  Complexity: L
