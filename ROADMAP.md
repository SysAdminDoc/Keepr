# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md). Long-form rationale for each item lives in [RESEARCH_FEATURE_PLAN_v0.19.md](RESEARCH_FEATURE_PLAN_v0.19.md). Historical research is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with the research file, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-05-26):** v0.18.1 shipped. v0.19+ cycle starting — see the v0.19 research file for the rationale behind each item below.

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
- [ ] **P1 — Audio voice notes** *(v0.20.3)*
  - Editor mic icon → MediaRecorder → save via generalized `add_blob_attachment`. Audio chip preview. `<audio>` in editor. CSP `media-src` updated.

## Phase C — Trust + recovery (v0.21.x)

- [ ] **P1 — Auto-backup → Rust thread + rotation** *(v0.21.0)*
  - Move `src/lib/autoBackup.ts` poll to Rust background thread (same AtomicBool shutdown pattern as reminder scheduler). Keep 7 daily + 4 weekly rotation. Configurable retention.
- [ ] **P1 — Vault recovery seed (BIP39 12-word, opt-in)** *(v0.21.1)*
  - Schema v12: `vault_dek_seed_wrapped`. Vault init shows seed once if opt-in checked. Recovery flow: enter seed → unlock → set new passphrase. Make explicit that opting in trades the absolute "no recovery" guarantee for a recoverable seed.
- [ ] **P1 — Content-addressed attachment storage + orphan sweep** *(v0.21.2)*
  - Hash bytes (BLAKE3), store at `<data_dir>/resources/ab/cd/<hash>.<ext>`. Daily sweep in same thread as auto-backup moves zero-ref blobs >24h old to `.trash/`; auto-purge .trash >30d. Migration: existing UUID-named files keep working; new attachments use hashed layout.

## Phase D — Power user (v0.22.x)

- [ ] **P2 — HistoryDrawer body diff** *(v0.22.0)*
  - Bundle `diff-match-patch` or hand-roll a line-diff. Render added lines accent / removed lines red between selected snapshots.
- [ ] **P2 — Per-note re-lock with biometric** *(v0.22.1)*
  - `tauri-plugin-biometric`. New `notes.note_locked` column. Per-note Lock button when vault initialized.
- [ ] **P2 — Two-way `[[Note Title]]` links + Linked-from panel** *(v0.22.2)*
  - Regex extract on save into `note_links` table. Editor renders `[[Foo]]` as clickable span. Editor footer "Linked from N notes" panel. Title autocomplete on `[[`.
- [ ] **P2 — Saved searches / Smart Labels** *(v0.22.3)*
  - `smart_labels(name, query_json)` table. "Save as Smart Label" button when filter active. Sidebar renders below labels.
- [ ] **P2 — Vault verifier CLI (`keepr-verify`)** *(v0.22.4)*
  - Separate Rust binary `src-tauri/src/bin/keepr-verify.rs`. Reads DB → re-derives Argon2id KEK from passphrase → decrypts sample vault note. Open-source credibility multiplier.

## Phase E — Quick wins / housekeeping (rolled into above phases as fit)

- [ ] **P2 — "Last backup: ..." line in Settings** *(part of v0.21.0)*
- [ ] **P2 — First-run sample notes** *(v0.22.5)*
- [ ] **P2 — Audit `IconBtn` `aria-label` coverage** *(v0.22.6)*
- [ ] **P2 — `role="list"` + `role="listitem"` on note grid** *(part of v0.22.6)*
- [ ] **P2 — Settings → Vault first-run microcopy** *(part of v0.21.1)*
- [ ] **P3 — Default `trashRetentionDays` to 30 if currently 0** *(v0.22.7)*
- [ ] **P2 — Verify pinned stable-grid empty-row behavior; fix if broken** *(v0.22.8)*
- [ ] **P2 — `commands.rs` split (3866 lines monolith)** *(v0.22.9)*
  - Split into `commands/notes.rs`, `commands/io.rs`, `commands/security.rs`, `commands/attachments.rs`, `commands/reminders.rs`, `commands/history.rs`, `commands/labels.rs`. Re-exported from `commands/mod.rs`.

## Phase F — Web Clipper (v0.23.x)

- [ ] **P1 — Web Clipper (browser extension + Tauri localhost server)** *(v0.23.0 — LARGER BET, defer until Phase A-D done)*
  - Rust localhost HTTP server on randomized port; per-install bearer token; MV3 extension at `web-clipper/`. Endpoints `/clip` `/clip/markdown` `/clip/selection` `/clip/screenshot` `/clip/url`. Bundled Readability.js + Turndown.js inside extension (no CDN). Tested on Firefox + Chrome + Edge.

## Phase G — Distribution larger bets (when ready)

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
- AI features / RAG / autocomplete / Gemini-style transcription
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
