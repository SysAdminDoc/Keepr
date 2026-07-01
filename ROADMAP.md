# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md); rationale for each cycle's choices is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with anything in the archive, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-07-01):** v0.25.14 adds a local Microsoft Store installer lane: `npm run build:store-installer` builds the Store-flavored NSIS/MSI installers with the required offline WebView2 installer mode. Partner Center submission, Store signing, package-identity MSIX/share-target work, and Store auto-update validation are blocked on external Microsoft account/product identity. Blocked signing/biometric/notarization items live in [Roadmap_Blocked.md](Roadmap_Blocked.md). This file lists only actionable open work.

---

## Open: Larger bets (v0.25.x and later)
- [ ] **P3 — Document scanner** (OpenCV WASM, ~7 MB renderer payload) — Apple Notes parity, lower-priority capture path.
- [ ] **P3 — Optional LAN-only P2P sync** (mDNS + Yjs CRDT, Anytype model) — only sync model compatible with "no cloud server" non-goal.

---

## Won't ship (rescoped from prior research)

- **NF-12 — Image OCR**: requires per-platform OCR backends; bundling a multi-MB engine into every build fails the cross-platform feature-parity bar. Users can paste OS-extracted text instead.
- **NF-13 — Rich URL preview cards**: requires app-initiated outbound HTTP to arbitrary websites. Directly contradicts the "no background outbound network requests except the opt-in speech model download" promise.

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
