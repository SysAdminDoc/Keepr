# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md); rationale for each cycle's choices is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with anything in the archive, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-05-27):** v0.22.10 shipped. 119 → **128** vitest + 68 → **78** cargo across the most recent (audit-hardening) pass. The v0.19→v0.22 cycle is closed; remaining items below either require external decisions (signing keys, paid subscriptions) or open the v0.23+ cycle.

---

## Open: Distribution credibility (blocked on external decisions)

- [ ] **P0 — Azure Trusted Signing for Windows builds** *(BLOCKED — needs paid subscription)*
  - Subscribe to Azure Trusted Signing ($9.99/mo basic). Add sign step to `.github/workflows/release.yml`. Update `SECURITY.md`.
- [ ] **P0 — `tauri-plugin-updater` scaffolding (Ed25519-signed manifest)** *(BLOCKED — needs signing-key decision; same gate as Azure Trusted Signing)*
  - Add plugin + generate keypair (private in GH Actions secret, public in `tauri.conf.json`). Workflow updates `latest.json` at a fixed Releases URL after each tag. App checks at startup + once/week. Settings toggle to disable.

## Open: Power-user (v0.22.x deferred → v0.23+)

- [ ] **P2 — Per-note re-lock with biometric** *(deferred — needs platform-test rig)*
  - `tauri-plugin-biometric`. New `notes.note_locked` column. Per-note Lock button when vault initialized.
  - **Why deferred:** biometric APIs differ per platform (Windows Hello via WinRT, macOS Touch ID via LocalAuthentication, no Linux equivalent). The existing Vault provides at-rest encryption already; this is a granular-lock convenience layer.

## Open: Trust + recovery (v0.21.x deferred → v0.23+)

- [ ] **P1 — Content-addressed attachment storage + orphan sweep** *(deferred — refactor cost > current value)*
  - Hash bytes (BLAKE3), store at `<data_dir>/resources/ab/cd/<hash>.<ext>`. Daily sweep moves zero-ref blobs >24h old to `.trash/`; auto-purge .trash >30d. Migration: existing UUID-named files keep working; new attachments use hashed layout.
  - **Why deferred:** substantive refactor (new storage layout, ref-counting, migration); current install sizes (typically <100 MB resources) don't yet justify the dedup win. Revisit if field reports indicate orphan accumulation or duplicate-photo bloat. (v0.22.10's `delete_note_permanent` / `empty_trash` file-cleanup fix already plugged the worst orphan leak.)

## Open: Voice transcription (v0.23.0)

- [ ] **P1 — Offline transcription via whisper.cpp (Vibe-style)** *(v0.23.0 — planned)*
  - `whisper-rs` (Rust bindings for whisper.cpp — same engine [Vibe](https://github.com/thewh1teagle/vibe) uses) operating on the WAV bytes we already save (v0.22.9 → present). New `transcribe_audio_attachment(attachment_id)` Tauri command writes the transcript back as a note-body append.
  - `download_speech_model()` command pulls `ggml-tiny.en-q5_1.bin` (~31 MB) from huggingface.co into the per-app data dir on first use; UI shows a one-time prompt with size + opt-in copy. Settings → new "Voice transcription" section: enable/disable, choose model size (tiny/base/small), delete model. After download, **fully offline** — no network ever. Audio never leaves the machine.
  - Per the binding non-goal language (rewritten in v0.22.4): cloud AI / Gemini-style transcription stays banned; local offline whisper.cpp is in-bounds — same offline-first / no-account / no-telemetry rules as the rest of Keepr.

## Open: Housekeeping (v0.23+)

- [ ] **P2 — `commands.rs` split** *(now ~4500 lines)*
  - Split into `commands/notes.rs`, `commands/io.rs`, `commands/security.rs`, `commands/attachments.rs`, `commands/reminders.rs`, `commands/history.rs`, `commands/labels.rs`. Re-exported from `commands/mod.rs`.
  - **Why deferred:** high merge-conflict risk during an active feature cycle. Schedule for a quiet "no other open PRs" day.
- [x] **P2 — `role="list"` + `role="listitem"` on note grid** *(v0.22.11 — shipped)*
  - All three NoteGrid layouts (masonry, stable-grid, list) now expose proper list semantics with an optional `ariaLabel` prop. Stable-grid placeholders stay `aria-hidden`. Visual layout unchanged.

## Open: Larger bets (v0.24.x and later)

- [ ] **P1 — Web Clipper (browser extension + Tauri localhost server)** *(v0.24.0 — LARGER BET)*
  - Rust localhost HTTP server on randomized port; per-install bearer token; MV3 extension at `web-clipper/`. Endpoints `/clip` `/clip/markdown` `/clip/selection` `/clip/screenshot` `/clip/url`. Bundled Readability.js + Turndown.js inside extension (no CDN). Tested on Firefox + Chrome + Edge.
- [ ] **P3 — MSIX packaging + Microsoft Store** — free signing, Windows Share Target contract, auto-update via Store.
- [ ] **P3 — macOS notarization** (Apple Developer $99/yr) — when distribution scale justifies.
- [ ] **P3 — `--data-dir <path>` CLI flag** for non-portable explicit relocation.
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
- Cloud AI / RAG / autocomplete / Gemini-style transcription (anything that ships audio, text, or embeddings to a remote service). Local offline inference (e.g. whisper.cpp for voice notes) is in-bounds — same offline-first / no-account / no-telemetry rules as the rest of Keepr.
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
