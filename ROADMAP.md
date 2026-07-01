# Keepr Roadmap

> **Single source of truth for outstanding work.** Shipped releases live in [CHANGELOG.md](CHANGELOG.md); rationale for each cycle's choices is archived under [`docs/research-archive/`](docs/research-archive/). When this file disagrees with anything in the archive, this file wins.

**Priority legend:** P0 = data loss / crash / security / distribution-blocker · P1 = visible bug / high user value · P2 = polish / nice-to-have · P3 = future / experimental.

**Status (2026-07-01):** v0.25.10 ships the Vite 8 / plugin-react 6 migration and clears the dev-only Vite/esbuild audit finding while preserving the React 19 renderer. Blocked signing/biometric/notarization items live in [Roadmap_Blocked.md](Roadmap_Blocked.md). This file lists only actionable open work.

---

## Open: Larger bets (v0.25.x and later)
- [ ] **P3 — MSIX packaging + Microsoft Store** — free signing, Windows Share Target contract, auto-update via Store.
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

## Research-Driven Additions

- [ ] P2 - Tailwind 4 styling lane
  Why: Tailwind latest is 4.3.x and this app depends on Tailwind 3 config-driven palette imports plus `@tailwind` directives.
  Evidence: `npm outdated --long`; `tailwind.config.js`; `postcss.config.js`; `src/index.css`; `src/keep-palette.js`.
  Touches: `package.json`, `package-lock.json`, `tailwind.config.js`, `postcss.config.js`, `src/index.css`, visual regression screenshots.
  Acceptance: Tailwind 4 migration preserves Keep color tokens, dark mode, scrollbar/focus styles, and note card/editor/settings layout on desktop and 390px mobile.
  Complexity: L

- [ ] P2 - TypeScript 6 + ESLint 10 lane
  Why: TypeScript latest is 6.0.x and ESLint latest is 10.x; both can change diagnostics and flat-config behavior.
  Evidence: `npm outdated --long`; `tsconfig.json`; `tsconfig.node.json`; `eslint.config.js`.
  Touches: `package.json`, `package-lock.json`, TypeScript config, ESLint config.
  Acceptance: TypeScript 6 and ESLint 10 land together only after framework lanes are stable; type-check, lint, unit tests, and build are green without suppressing new diagnostics.
  Complexity: L

- [ ] P2 - Zustand pin removal lane
  Why: Zustand latest is 5.0.14, but the repo intentionally pins 5.0.1 until ESM/Vitest singleton behavior is guarded.
  Evidence: `package.json`; `src/store.ts`; `src/__tests__/storeZustandRegression.test.ts`.
  Touches: `package.json`, `package-lock.json`, `src/__tests__/storeZustandRegression.test.ts` if upstream behavior changes.
  Acceptance: upgrade Zustand only after the ESM singleton guard exists and passes; store state/actions remain shared across static and dynamic imports under Vitest.
  Complexity: M
