# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md). Long-form rationale for each item lives in [RESEARCH_FEATURE_PLAN_v0.19.md](RESEARCH_FEATURE_PLAN_v0.19.md). Historical research is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with the research file, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-05-26):** **v0.22.4 shipped.** v0.19+ cycle made the following pass:
- Phase A (distribution credibility): 4 of 5 shipped — Azure signing + auto-updater are both gated on the same signing-key decision and remain blocked.
- Phase B (capture surface): **all 4 shipped** (command palette, hashtag autocomplete, bulk vault, voice notes).
- Phase C (trust + recovery): 2 of 3 shipped — auto-backup rotation + vault recovery seed. Content-addressed attachments deferred (big refactor, marginal value at current install sizes).
- Phase D (power user): 4 of 5 shipped — body diff, smart labels, wiki links (panels), verifier CLI. Biometric per-note re-lock deferred (platform-specific concerns).
- Phase E (quick wins): all addressed (some "already done, audit confirmed" rather than shipped).
- Phase F (web clipper) + G (distribution larger bets): unstarted, larger scope.

107 → **119** vitest + 55 → **58** cargo tests across the cycle. See [CHANGELOG.md](CHANGELOG.md) for per-release detail.

---

## Phase A — Distribution credibility (v0.19.x)

- [x] **P0 — Cross-platform CI matrix** *(v0.19.0 — shipped)*
  - Extend `.github/workflows/ci.yml` from `windows-latest` only to also run `macos-14` + `ubuntu-22.04`. Add the Linux build-dep apt-install step from release.yml. Same four steps everywhere (cargo check + test, npm lint + test + build).
- [ ] **P0 — Azure Trusted Signing for Windows builds** *(BLOCKED — needs paid subscription)*
  - Subscribe to Azure Trusted Signing ($9.99/mo basic). Add sign step to release.yml. Update SECURITY.md.
- [ ] **P0 — `tauri-plugin-updater` scaffolding (Ed25519-signed manifest)** *(BLOCKED — needs signing-key decision; same gate as Azure Trusted Signing)*
  - Add plugin + generate keypair (private in GH Actions secret, public in `tauri.conf.json`). Workflow updates `latest.json` at fixed Releases URL after each tag. App checks at startup + once/week. Settings toggle to disable.
- [x] **P1 — Window state persistence** *(v0.19.2 — shipped)*
  - `tauri-plugin-window-state`. Two-line plugin init.
- [x] **P1 — "Open log folder" + "Open data folder" buttons** *(v0.19.3 — shipped)*
  - Added to Settings → Log folder AND Data folder rows. `tauri-plugin-opener` v2.5.4. Whitelisted to Keepr's own dirs only — no generic `open_path` IPC.
- [x] **P1 — Search clear-button + extra filter chips** *(v0.19.4 — shipped)*
  - Search clear button already existed (TopBar.tsx:131-143); kept. Added 3 new chips: Has image / Has reminder / In vault (vault chip only when initialized + unlocked). Dropped `is:archived` — redundant with Archive section. Pill chip shape fixed (rounded-full → rounded).

## Phase B — Core capture surface (v0.20.x)

- [x] **P1 — Command Palette (Ctrl/Cmd+K)** *(v0.20.0 — shipped)*
  - New `CommandPalette.tsx`; fuzzy across note titles + every settings action + every section + every label. Lazy-loaded behind Suspense.
- [x] **P1 — Tag autocomplete in editor** *(v0.20.1 — shipped)*
  - `#X` triggers an inline chip-strip suggesting up to 5 matching labels; Tab/Enter completes top match; click any chip to pick. New `findPartialHashtag` pure helper + 7 tests.
- [x] **P1 — Bulk "Move to/from Vault"** *(v0.20.2 — shipped)*
  - `BulkActionBar` Lock + Unlock buttons gated on vault init+unlocked. New `move_notes_to_vault` / `move_notes_out_of_vault` Rust commands loop the per-note path (not atomic across batch — accepted trade-off; each per-note commits its own tx).
- [x] **P1 — Audio voice notes** *(v0.20.3 — shipped, end-to-end not mic-tested in CI)*
  - Mic icon in editor → MediaRecorder (opus/webm) → `add_audio_attachment_bytes` Rust command. AttachmentGrid renders `<audio controls>` for audio mimes. CSP `media-src` opened.

## Phase C — Trust + recovery (v0.21.x)

- [x] **P1 — Auto-backup rotation** *(v0.21.0 — shipped, partial)*
  - Rotation shipped: after each successful auto-backup, `prune_auto_backups` deletes everything older than the latest N (default 12, configurable in Settings 0–365; 0 = unlimited). 4 new tests for the prune helper.
  - **Deferred**: moving the poll loop from renderer to Rust background thread. Renderer poll works today; migration cost > marginal reliability win. Revisit if field reports surface missed backups.
- [x] **P1 — Vault recovery seed (BIP39 12-word, opt-in)** *(v0.21.1 — shipped)*
  - No schema migration needed (app_settings is k/v; 3 new keys for the seed envelope). Set up via Settings → Vault → "Set up recovery seed…" when unlocked. Recovery flow on locked vault: "Forgot password? Recover with seed phrase…". 3 new Rust round-trip tests. Explicitly opt-in — preserves the "no recovery" guarantee for users who don't enable it. Settings → Vault microcopy made explicit about the trade-off.
- [ ] **P1 — Content-addressed attachment storage + orphan sweep** *(deferred to v0.23+)*
  - Hash bytes (BLAKE3), store at `<data_dir>/resources/ab/cd/<hash>.<ext>`. Daily sweep moves zero-ref blobs >24h old to `.trash/`; auto-purge .trash >30d. Migration: existing UUID-named files keep working; new attachments use hashed layout.
  - **Deferred this cycle**: substantive refactor (new storage layout, ref-counting, migration), and current install sizes (typically <100 MB resources) don't yet justify the dedup win. Revisit when field reports indicate orphan accumulation or duplicate-photo bloat.

## Phase D — Power user (v0.22.x)

- [x] **P2 — HistoryDrawer body diff** *(v0.22.0 — shipped)*
  - Expand arrow per snapshot → inline LCS line-diff vs current. Hand-rolled (no diff-match-patch), 7 vitest cases. Vault snapshots excluded (ciphertext).
- [ ] **P2 — Per-note re-lock with biometric** *(deferred to v0.23+)*
  - `tauri-plugin-biometric`. New `notes.note_locked` column. Per-note Lock button when vault initialized.
  - **Deferred this cycle**: biometric APIs differ per platform (Windows Hello via WinRT, macOS Touch ID via LocalAuthentication, no Linux equivalent). Need a platform-test rig before shipping. The existing Vault provides at-rest encryption already; this is a granular-lock convenience layer.
- [x] **P2 — Two-way `[[Note Title]]` links + Linked-from panel** *(v0.22.3 — shipped, partial)*
  - Editor footer shows "Mentions" + "Linked from N" panels. Click chips to jump. Renderer-only (no schema, no IPC) — `src/lib/wikiLinks.ts` + 12 tests. Inline `[[Foo]]` rendering as clickable span in the body and `[[` autocomplete dropdown intentionally deferred (would require contenteditable rewrite of the body textarea; not worth it for the gain).
- [x] **P2 — Saved searches / Smart Labels** *(v0.22.2 — shipped)*
  - Schema v12 `smart_labels` table; 4 new Tauri commands; sidebar entries below regular labels; "Save as Smart Label" button when filter active; click to re-apply; X / right-click to delete.
- [x] **P2 — Vault verifier CLI (`keepr-verify`)** *(v0.22.1 — shipped)*
  - Standalone Rust binary at `src-tauri/src/bin/keepr-verify.rs`. Supports `--db`, `--note-id`, `--seed`, `--help`. Reads stdin for passphrase. Builds independently of Tauri runtime.

## Phase E — Quick wins / housekeeping (rolled into above phases as fit)

- [x] **P2 — "Last backup: ..." line in Settings** *(v0.21.0 — already present pre-cycle, verified)*
- [x] **P2 — First-run sample notes** *(v0.21.2 — shipped)*
- [x] **P2 — Audit `IconBtn` `aria-label` coverage** *(v0.21.2 — already clean, verified zero violations)*
- [ ] **P2 — `role="list"` + `role="listitem"` on note grid** *(defer; risks visual layout regressions on multi-column CSS — verify against current masonry first)*
- [x] **P2 — Settings → Vault first-run microcopy** *(part of v0.21.1 — addressed)*
- [x] **P3 — Default `trashRetentionDays` to 30 if currently 0** *(v0.21.2 — already 7, matches Keep mobile)*
- [x] **P2 — Verify pinned stable-grid empty-row behavior; fix if broken** *(v0.21.2 — min-height: 1px guard added to placeholders)*
- [ ] **P2 — `commands.rs` split (now ~4400 lines)** *(deferred to v0.23+)*
  - Split into `commands/notes.rs`, `commands/io.rs`, `commands/security.rs`, `commands/attachments.rs`, `commands/reminders.rs`, `commands/history.rs`, `commands/labels.rs`. Re-exported from `commands/mod.rs`.
  - **Deferred this cycle**: high merge-conflict risk during an active feature cycle. Schedule for a quiet "no other open PRs" day.
- [x] **P1 — NSIS installer hooks: Start Menu + Desktop shortcuts + README.txt** *(v0.22.4 — shipped)*
  - `src-tauri/installer.nsh` with explicit `NSIS_HOOK_POSTINSTALL` / `NSIS_HOOK_POSTUNINSTALL` macros wired via `bundle.windows.nsis.installerHooks`. README.txt explains the unavoidable manual taskbar-pin step (Windows 10 1809+ blocks programmatic taskbar pinning by design).

## Phase F — Voice notes recovery + transcription (v0.22.5 / v0.23.0)

- [x] **P0 — Voice-note recording fix (WebView2 mic permission)** *(v0.22.5 — shipped)*
  - `additionalBrowserArgs: "--use-fake-ui-for-media-stream --autoplay-policy=no-user-gesture-required --disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection"` on `app.windows[0]` in `tauri.conf.json`. Defensive `navigator.mediaDevices` undefined guard + DOMException-name → actionable-error mapping in `VoiceRecorderModal.tsx`. Stale-deny reset path (`%LOCALAPPDATA%\com.sysadmindoc.keepr\EBWebView\Default\Preferences`) documented in CLAUDE.md gotchas + CHANGELOG notes.

- [ ] **P1 — Offline transcription via whisper.cpp (Vibe-style)** *(v0.23.0 — planned)*
  - `whisper-rs = "0.13"` (Rust bindings for whisper.cpp — same engine [Vibe](https://github.com/thewh1teagle/vibe) uses) + `symphonia` for decoding the recorded webm/opus blob into 16 kHz f32 PCM. New `transcribe_audio_attachment(attachment_id)` Tauri command writes the transcript back as a note-body append or a dedicated `transcript` column. `download_speech_model()` command pulls `ggml-tiny.en-q5_1.bin` (~31 MB) from huggingface.co into the per-app data dir on first use; UI shows a one-time prompt with size + opt-in copy. Settings → new "Voice transcription" section: enable/disable, choose model size (tiny/base/small), delete model. After download, **fully offline** — no network ever. Audio never leaves the machine.
  - **Non-goal note**: the existing "no AI / no transcription" non-goal was scoped to *cloud / Gemini-style* transcription (audio leaves the machine, account required, costs accrue). Local whisper.cpp inference fits the same offline-first / single-user / no-account / no-telemetry promise as the rest of Keepr; treating it as in-bounds. Non-goal language rewritten in this commit.

## Phase G — Web Clipper (v0.24.x)

- [ ] **P1 — Web Clipper (browser extension + Tauri localhost server)** *(v0.23.0 — LARGER BET, defer until Phase A-D done)*
  - Rust localhost HTTP server on randomized port; per-install bearer token; MV3 extension at `web-clipper/`. Endpoints `/clip` `/clip/markdown` `/clip/selection` `/clip/screenshot` `/clip/url`. Bundled Readability.js + Turndown.js inside extension (no CDN). Tested on Firefox + Chrome + Edge.

## Phase H — Distribution larger bets (when ready)

- [ ] **P3 — MSIX packaging + Microsoft Store** — free signing, Windows Share Target contract, auto-update via Store
- [ ] **P3 — macOS notarization** (Apple Developer $99/yr) — when distribution scale justifies
- [ ] **P3 — `--data-dir <path>` CLI flag** for non-portable explicit relocation
- [ ] **P3 — Document scanner** (OpenCV WASM, ~7MB renderer payload) — Apple Notes parity, lower-priority capture path
- [ ] **P3 — Optional LAN-only P2P sync** (mDNS + Yjs CRDT, Anytype model) — only sync model compatible with "no cloud server" non-goal

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

- **Code-signing (v0.5+):** ship unsigned with SmartScreen workaround until Azure Trusted Signing subscription approved
- **macOS / Linux support tier (v0.10+):** Windows is supported; macOS + Linux are best-effort
- **App Lock + Private Vault lost-credential policy:** no recovery for App Lock or default Vault. **(Updated v0.21.1: opt-in recovery seed for Vault.)**
- **Reminder scheduler granularity:** 30-second poll. Documented up-to-30-s lag acceptable.
