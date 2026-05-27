# Project Research and Feature Plan — v0.19+ Cycle

> Research date: **2026-05-26** · Target HEAD: `e2e43b5` (v0.18.1 released) · Repo: https://github.com/SysAdminDoc/Keepr
>
> Companion to [ROADMAP.md](ROADMAP.md) (live task list, terse) and the historical [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md) (v0.1→v0.2) + [RESEARCH_FEATURE_PLAN_v0.5.md](RESEARCH_FEATURE_PLAN_v0.5.md) (v0.5→v0.11). This file documents the next planning cycle starting from v0.19. When this file disagrees with ROADMAP.md, the roadmap wins; this file is the long-form rationale.

---

## Executive Summary

Keepr v0.18.1 is a mature offline Google Keep clone — 39 Tauri commands, 23 React components, SQLite schema v11, XChaCha20-Poly1305 vault, App Lock, FTS5 search, 86 vitest + 55 cargo tests, MIT, ships unsigned MSI/NSIS/portable for Windows + best-effort macOS/Linux via tauri-action. The 26-release sprint that ran 2026-05-25→26 closed every NF-V0.5-* roadmap item. The product is now **feature-complete against its stated Keep-parity bar** and has spent the last week on UX polish (zoom, accent picker, font slider, full-screen editor, blurred backdrops, stable pinned grid). The honest backlog in ROADMAP.md is two items: HistoryDrawer body diff (low value) and the auto-updater scaffold (blocked on distribution decisions).

**The highest-value direction for v0.19+ is "earn distribution trust and close the last single-user gaps."** Three threads matter most:
1. **Distribution credibility** — sign Windows builds via Azure Trusted Signing (~$120/yr, removes SmartScreen friction over time), wire `tauri-plugin-updater` with signature pinning, ship a verifier CLI (Notesnook Vericrypt pattern).
2. **Quick-capture surface area** — a tiny localhost-only browser web clipper (Joplin pattern, MV3, token-gated, no outbound HTTP), Windows Share Target integration via MSIX, audio voice notes, document scanner.
3. **Power-user UX gaps that fit the Keep identity** — command palette (Ctrl+K), `[[note title]]` two-way links + "Linked from" panel, saved-search "Smart Labels", tag autocomplete in the editor, recovery seed at vault init.

**Top 10 opportunities, priority order:**

1. **P0 — Cross-platform CI coverage.** `.github/workflows/ci.yml:12` runs only on `windows-latest`. The release workflow happily ships macOS/Linux artifacts the CI never compiled. First Linux/macOS regression will land on a tagged release.
2. **P0 — Code sign Windows builds via Azure Trusted Signing.** Documented as "ship unsigned" in SECURITY.md:5 — but EV-cert SmartScreen-bypass was removed in 2024; Azure Artifact Signing is $9.99/mo with <1hr identity validation. Lowest-friction path off the SmartScreen warning today.
3. **P0 — Wire `tauri-plugin-updater` with signature pinning.** ROADMAP.md backlog item; auto-updater works without a cert, just needs a signed manifest at a fixed GH Releases URL. Drops the "did the user remember to download v0.18.2" failure mode entirely.
4. **P1 — Web clipper (localhost-only, MV3).** Highest-leverage capture surface for the offline-first promise — Joplin's port-fixed REST API is the proven template. Browser extension + Tauri-spawned localhost server on random port + per-install token + `/clip/markdown` `/clip/screenshot` `/clip/selection` endpoints. No outbound HTTP.
5. **P1 — Command palette (Ctrl+K).** Standard Notes pattern, near-zero risk, high payoff for keyboard users. Fuzzy-match across note titles + every settings action + every section + every label. ~1 day implementation on top of existing FTS5 + Zustand store.
6. **P1 — Recovery seed at vault init.** Anytype-style 12-word BIP39 phrase printed once at vault creation, decrypts the wrapped DEK even when the passphrase is lost. Eliminates the SECURITY.md:53 "no recovery" foot-gun for users who actually back up the seed. Doesn't compromise crypto.
7. **P1 — Audio voice notes.** Capture-side gap Keepr's never had; fits the "single note can mix text + image + checklist" shape. Tauri `record-audio` plugin or direct WebRTC `MediaRecorder` to `.m4a`/`.opus`; reuse the attachment pipeline.
8. **P1 — Bulk-action "Move to/from vault."** `BulkActionBar.tsx` currently exposes pin/color/labels/archive/trash/restore/delete. Vault move is one-by-one. Users with sensitive notes today must touch each one individually.
9. **P2 — Per-note re-lock with Windows Hello / Touch ID.** `tauri-plugin-biometric` exists; would let a user keep the DB open but require biometric to view a specific note. Apple Notes parity, fits the App Lock + Vault layered model.
10. **P2 — Two-way `[[Note Title]]` links + "Linked from" panel.** Bear pattern, scoped: link by title (no block IDs), one-way regex extraction at save time, panel under labels showing inbound references. Adds real value without dragging Keepr into PKM territory.

---

## Evidence Reviewed

### Local files and directories inspected

- **Repo-root config & docs:** `README.md`, `ROADMAP.md`, `CHANGELOG.md` (head 50 entries), `CONTRIBUTING.md`, `SECURITY.md`, `LICENSE`, `package.json`, `package-lock.json` (versions only), `tsconfig.json`, `vite.config.ts`, `tailwind.config.js`, `eslint.config.js`, `.gitignore`
- **Rust backend:** `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` (Tauri 2.11.2 confirmed), `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`, `src-tauri/src/main.rs`, `src-tauri/src/lib.rs` (453 lines), `src-tauri/src/db.rs` (778 lines), `src-tauri/src/commands.rs` (3866 lines), `src-tauri/src/vault.rs`, `src-tauri/src/lock.rs`
- **React frontend:** every file under `src/components/` (23 TSX, 6019 LOC), `src/hooks/` (7 hooks), `src/lib/` (6 utilities), `src/store.ts` (683 lines), `src/api.ts`, `src/App.tsx`, `src/main.tsx`, `src/types.ts`, `src/colors.ts`, `src/index.css`, `src/__tests__/` (13 test files)
- **CI/CD:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`
- **Prior research:** `RESEARCH_FEATURE_PLAN.md` (head/exec summary), `RESEARCH_FEATURE_PLAN_v0.5.md` (scanned for shipped items)

### Git history range reviewed

- `rtk git log -50 --oneline`: from `e2e43b5 release: v0.18.1` (HEAD, 2026-05-26) back through `b5fad44 fix(io): vault export collision + Takeout chronology + reminder rrule` and earlier — covers the entire post-v0.2 sprint.
- 26 release commits since v0.5.0; six in the last 6 hours (v0.16.3 → v0.18.1) all UX/polish.

### Build/test/release artifacts verified live

- `cargo test --lib`: **55 passed** at HEAD (this session)
- `npm test`: **86 vitest passed** at HEAD (this session)
- `npm run build`: clean (~313KB JS / 95KB gzipped)
- `npm run lint`: clean
- `npm run tauri build`: produces `Keepr_0.18.1_x64-setup.exe` (NSIS) + `Keepr_0.18.1_x64_en-US.msi` (verified this session — installed and running)
- GH Actions release.yml builds 4 platform variants (win/mac-13/mac-14/ubuntu-22.04) per tag

### External sources reviewed

The competitive landscape research dispatched as a sub-agent covered Google Keep, Joplin, Standard Notes, Simplenote, Obsidian, Bear, Notesnook, Anytype, QOwnNotes, Trilium/TriliumNext, Apple Notes, Microsoft OneNote, plus eight pattern topics (web clippers, Share Target, backlinks, quick-capture, sync-without-server, attachment dedup, encryption stacks, accessibility benchmarks) and three distribution topics (Windows code signing in 2026, Tauri 2 plugin ecosystem, WebView2 footguns). Primary sources cited inline below where claims drive recommendations:

- **Code signing:** [Azure Artifact Signing pricing](https://azure.microsoft.com/en-us/pricing/details/artifact-signing/) ($9.99/mo / $99.99/mo tiers, 24h-lived HSM-backed certs), [GA announcement](https://www.devclass.com/security/2026/01/14/code-signing-windows-apps-may-be-easier-and-more-secure-with-new-azure-artifact-service/4079554) (EV-cert SmartScreen bypass removed in 2024)
- **Joplin web clipper architecture:** [docs](https://joplinapp.org/help/apps/clipper/) — localhost REST on port 41184, token-gated, MV3 extension. Architectural template.
- **Notesnook Vericrypt:** [open-source offline verifier](https://github.com/streetwriters/notesnook) — independent re-derivation of Argon2id key + sample-note decrypt; consumer credibility multiplier.
- **Anytype recovery seed:** [data-and-security docs](https://doc.anytype.io/anytype-docs/advanced/data-and-security/how-we-keep-your-data-safe) — 12-word BIP39 phrase printed at first run.
- **Apple Notes per-note lock:** [Apple security guide](https://support.apple.com/guide/security/secure-features-in-the-notes-app-sec1782bcab1/web) — AES-GCM 256 + PBKDF2-SHA256 per-note passphrase + biometric unlock.
- **Tauri 2 plugins workspace:** [plugins-workspace](https://github.com/tauri-apps/plugins-workspace) — biometric, autostart, deep-link, window-state, clipboard-manager, updater all official.
- **WebView2 memory bloat caveats:** [Windowslatest](https://www.windowslatest.com/2026/01/11/i-uninstalled-windows-11s-whatsapp-webview2-and-replaced-it-with-the-old-native-app-with-a-new-trick/) — relevant for tray-mode resident size.
- **CRDT vs LWW sync gotchas:** [Tonsky local-first essay](https://tonsky.me/blog/crdt-filesync/), [arXiv on iCloud failures](https://arxiv.org/html/2602.19433v3) — relevant if BYO-cloud sync is ever attempted.

### Areas not verified (call out for future passes)

- **Behavior of pinned stable-grid (v0.18.1) when ALL cards in a row are unpinned** — placeholders are `visibility:hidden`, which CSS Grid still counts for row-height min, but the row should collapse if every cell is empty. **Needs live validation** on a multi-row pinned section with progressive unpins.
- **Cross-platform release workflow** — release.yml has built tags previously but the SysAdminDoc Actions billing block (CHANGELOG.md v0.16.1) has been blocking; haven't actually published a macOS/Linux artifact end-to-end yet. The matrix is correct *on paper*.
- **WebView2 resident memory at idle on Windows 11** — no measurement. Likely is 250–500MB based on Tauri norm; would inform whether tray-mode needs explicit renderer cache purge.
- **FTS5 search latency at scale** — last benchmarked at v0.13.0; not retested at v0.18.x with the broader command surface. Likely fine but no measured number.
- **Argon2id wall-clock at the documented m=64MiB/t=3/p=1** — SECURITY.md:30 claims "~150-300ms per attempt"; not re-measured on modern CPUs.

---

## Current Product Map

### Core workflows

1. **Capture quick note** — `NewNoteBar` click → `NoteEditor` modal (90vw × 90vh since v0.17.1) → type → click outside → save (dirty-check skips no-op writes since v0.17.0)
2. **Capture from anywhere** — global hotkey `Ctrl+Alt+N` (`useGlobalHotkey` in `App.tsx`, registered via `tauri-plugin-global-shortcut`) → quick-capture event → editor opens
3. **Capture from tray** — system tray icon (`lib.rs` setup) → "New note" menu item
4. **Organize** — open card → edit title/body/color/labels/pin/background-pattern/reminder/attachments → close
5. **Triage** — hover card → pin/archive/trash icons; or multi-select via `BulkActionBar` for bulk pin/color/labels/archive/trash/restore/delete
6. **Find** — TopBar search → 150ms debounce → FTS5 `notes_fts` lookup via `search_notes` command, falls back to in-memory substring scan; filter chips (type/color/label/pinned)
7. **Sort** — TopBar sort menu: Modified / Created / Title / Custom (drag-reorder under Custom; drag in any mode auto-flips to Custom since v0.17.0)
8. **Reminders** — set per-note via `ReminderPicker`; 30-second poll thread in Rust (`lib.rs:310-385`) fires native toast via `tauri-plugin-notification`; snooze/clear available; export all reminders as ICS (`export_reminders_ics`)
9. **Secrets** — App Lock (PIN gate over the whole UI) + Private Vault (per-note XChaCha20-Poly1305 encryption); both Argon2id-hashed; documented "no recovery" policy
10. **Backup** — manual ZIP export/import, auto-backup (off/daily/weekly to user folder), Markdown vault export (one `.md` per note + YAML frontmatter + `_resources/`), Google Takeout import (shape-based detection since v0.16.3)
11. **History** — every save snapshots a note (auto-trim to 20 per note via SQLite trigger); `HistoryDrawer` lists snapshots + restore button (body diff deferred)

### Platforms & distribution

- **Windows 10/11 x64** — **supported**. NSIS + MSI + portable zip via GH Releases (unsigned; SmartScreen warning expected)
- **macOS 13+/14+ x64 + arm64** — best-effort. DMG via GH Releases (unsigned, not notarized; quarantine bit workaround documented)
- **Linux x64 (Ubuntu 22.04+ glibc 2.35+)** — best-effort. `.deb` + AppImage via GH Releases
- **No web build, no mobile build** — explicit non-goals

### Important integrations & permissions

- **Tauri capabilities** (`src-tauri/capabilities/default.json`) — `core:default`, `dialog:default` + open + save, `global-shortcut:allow-register` + unregister, `notification:default` + allow-notify. **No `fs:*` granted to renderer** — every file op is in Rust. Locked-down by design.
- **CSP** (`tauri.conf.json:25`) — `default-src 'self' tauri: ipc: http://ipc.localhost`; images: `data:`, `blob:`, `asset:`, `keepr-resource://`; scripts: `'self'` only; connect: `ipc:` only. **No outbound HTTP possible** by configuration.
- **Custom protocol** `keepr-resource://<id>` serves attachment blobs + thumbnails from `<data_dir>/resources/`. Path-safety check rejects `..`, `/`, `\` in id.
- **System tray** — Show/Hide window + New Note + Quit. Closing main window minimizes to tray instead of exiting; Quit menu is the only intentional process termination.

### Storage & data

- SQLite WAL at `<data_dir>/keepr.db` (`<exe-parent>/keepr.db` in portable mode if `portable.flag` sentinel exists)
- Attachments: blobs on disk at `<data_dir>/resources/<id>.<ext>`; 32 MiB per-file cap
- `app_settings` table — server-side persistent settings (lock_pin_hash, lock_after_minutes, vault envelope, vault_initialized)
- localStorage (renderer-side) — 11 `keepr:*` keys for theme/view/sort/zoom/font/accent/auto-backup state

### User personas (refined from v0.1 research)

1. **Offline-first private user** (primary) — wants Keep that works on a laptop with no internet, data on their own disk, manual backup. **Well-served by v0.18.1.**
2. **Migrating Keep user** — Google Takeout import addressed in v0.16.3; **well-served.**
3. **Power keyboard user** — Keep canonical shortcuts shipped. **Mostly served**; would benefit from command palette + tag autocomplete.
4. **Sensitive-info user** — App Lock + Vault. **Well-served on encryption**, **under-served on recovery and per-note granular lock**.
5. **Sysadmin / portable-mode user** — `portable.flag` USB-stick mode. **Well-served**; could benefit from `--data-dir <path>` CLI flag for non-portable explicit relocation.
6. **NEW: Multi-device user without sync** — never explicitly addressed. Current answer: "back up to a sync folder yourself". This will keep coming up; eventually deserves a documented "here's the safe pattern" recipe even if Keepr doesn't ship sync code.

---

## Feature Inventory

| Feature | User value | Entry point | Main code | Maturity | Tests / docs | Improvement opportunity |
|---|---|---|---|---|---|---|
| Text + checklist notes | Core capture | `NewNoteBar`, `NoteEditor` | `commands.rs:601 create_note`, `:687 update_note` | Complete | `setKind.test.ts` | Audio + scan note types (see §New Features) |
| Masonry grid | Keep parity, scannable layout | `NoteGrid` | `NoteGrid.tsx`, CSS `column-width` | Complete | No tests | None |
| Stable-grid pinned section | Pinned cards stay put on unpin | `NoteGrid layout="stable-grid"` | `NoteGrid.tsx:127-180` | Complete (v0.18.1) | No test for placeholder slot behavior | Add unit test; verify all-empty-row collapse |
| 12 Keep colors + 9 patterns | Visual differentiation | `ColorPicker` popover | `colors.ts`, `keep-palette.js`, `lib/backgroundPatterns.ts` | Complete | `colors.test.ts`, `backgroundPatterns.test.ts` | None |
| Labels (flat, no hierarchy) | Organization | Sidebar labels list, `LabelsManager` | `commands.rs:2266-2358` | Complete | `hashtags.test.ts`, `hashtagMerge.test.ts` | Tag autocomplete in editor (see §New) |
| Inline `#hashtag` labels | Auto-create labels from body | Type `#word` in body | `lib/hashtags.ts`, `extractHashtagsFromNote` | Complete | Yes | None |
| Pin / archive / trash | Standard triage | Card hover icons, bulk bar | `set_pinned`, `set_archived`, `set_trashed` | Complete | `filterNotes.test.ts` | None |
| Configurable trash retention | Auto-purge old trash | Settings → Trash retention | `lib/trashRetention.ts` | Complete | `trashRetention.test.ts` | Default is `0 = off`; consider sane default (30 days) |
| Multi-select bulk actions | Power editing | Click+`x` or long-press | `BulkActionBar.tsx` | Complete | No tests | Missing "Move to/from vault" bulk action (P1) |
| Drag-to-reorder | Custom ordering | Drag any card in Notes section | `NoteGrid.tsx onDragEnd`, dnd-kit | Complete (v0.17.0, v0.17.1 sticks) | No tests | Add Playwright/Vitest test |
| FTS5 search | Fast title/body/checklist search | TopBar search input | `commands.rs:556 search_notes`, `db.rs:273 v9 migration` | Complete | No FTS-specific test | Latency benchmark at scale; `is:vault` / `has:attachment` filters |
| Filter chips | Narrow by type/color/label/pinned | TopBar below | `FilterChips.tsx` | Complete | No tests | Saved searches (Smart Labels) — see §New |
| Time reminders | Date/time fire + native toast | Bell icon in editor | `commands.rs:1578 set_reminder`, `lib.rs:310 scheduler` | Complete | `reminders.test.ts`, `reminderPresets.test.ts` | Place reminders explicitly non-goal; recurring rrule complete |
| ICS export | Cross-app reminder portability | Settings → Export reminders | `commands.rs:1698 export_reminders_ics` | Complete | No test | None |
| Image attachments | Visual notes | Paste / drop / file pick in editor | `add_image_attachment*` + `keepr-resource://` | Complete | No round-trip test | Content-addressed storage + dedup (see §Improvements) |
| Drawing canvas | Quick sketches → attached PNG | Editor → Drawing button | `DrawingCanvasModal.tsx`, `add_image_attachment_bytes` | Complete | No test | OS-level pen pressure support (P3) |
| Version history | Recover prior states | Editor → kebab → Version history | `HistoryDrawer.tsx`, `commands.rs:2936 list_snapshots` | Complete | No test | Body diff (ROADMAP backlog); per-snapshot author = "you" placeholder |
| App Lock (PIN) | UI gate when idle | Settings → App Lock | `commands.rs:2812-2854`, `lock.rs` (Argon2id) | Complete | `useIdleLock.test.ts` | Biometric unlock alternative (P2) |
| Private Vault (at-rest crypto) | Sensitive notes encrypted on disk | Settings → Vault → Init/Unlock | `commands.rs:2862-2925`, `vault.rs` (XChaCha20-Poly1305) | Complete | No vault round-trip test | Recovery seed (P1); per-note re-lock (P2); verifier CLI (P2) |
| Markdown vault export | Portable per-note `.md` | Settings | `commands.rs:1003 export_vault` | Complete | No test | None |
| Google Takeout import | One-shot Keep migration | Settings | `commands.rs:1190 import_takeout`, shape detector at 1440 | Complete (v0.16.3) | 13 Rust tests | None |
| ZIP backup / restore | Manual full DB roundtrip | Settings | `commands.rs:417 export_zip`, `:418 import_zip` | Complete (zip-slip + zip-bomb defended) | No test on `.prev` rollback | Auto-backup rotation (P2) |
| Auto-backup | Scheduled ZIP dump | Settings → Cadence | `lib/autoBackup.ts` (renderer poll) | Partial | `autoBackup.test.ts` | Move to Rust background thread (runs even with renderer hidden) |
| Light / Dark / System theme | Visual preference | TopBar moon icon, Settings | `store.ts setThemeMode`, `index.html` boot script | Complete | No test | None |
| Accent color picker (6 presets) | Personalization | Settings | `store.ts ACCENT_PRESETS`, App.tsx useEffect mirrors to CSS vars | Complete (v0.18.0) | No test | None |
| Note text size slider | Readability | Settings | `store.ts noteFontSize` + inline style on 4 surfaces | Complete (v0.18.0) | No test | Title size + UI density also adjustable (P3) |
| Card width zoom (Ctrl+Wheel) | Density control | Wheel in main area | `App.tsx wheel handler`, `store.ts cardWidth` | Complete (v0.16.5) | No test | UI affordance for non-mouse users (slider in Settings) |
| Editor 90vw × 90vh + blurred backdrop | Premium feel | Open any note | `NoteEditor.tsx`, `index.css .modal-backdrop` | Complete (v0.17.1) | No test | None |
| System tray | Background presence | Tray icon | `lib.rs setup` | Complete | No test | More tray menu items (recent notes, search box) — see §New |
| Global hotkey `Ctrl+Alt+N` | Quick capture from anywhere | Hotkey | `tauri-plugin-global-shortcut` + `App.tsx` emit | Complete | `globalHotkey.test.ts` | Custom hotkey configuration |
| Single-instance guard | Prevent double-launch DB corruption | Second `keepr.exe` | `tauri-plugin-single-instance` | Complete | No test | None |
| Keep keyboard shortcuts | Power UX | `?` overlay | `useKeepShortcuts`, `HelpOverlay.tsx` | Complete | No test | Command palette to discover them all (P1) |
| Portable mode | USB-stick run | `portable.flag` sentinel next to exe | `lib.rs:24-35 resolve_data_dir` | Complete | No test | `--data-dir <path>` CLI flag (P3) |
| Log folder + copy-path button | Diagnostics | Settings → Log folder | `tauri-plugin-log` + `get_log_dir` | Complete | No test | "Open log folder" button (in addition to copy) |
| Settings modal (wider + scrollable) | Configuration | TopBar gear | `SettingsModal.tsx` (534 lines, max-w-2xl, scrollable body) | Complete (v0.16.4) | No test | Section nav / tabs (530 lines is a lot for one scroll) |

**Hidden / undocumented surfaces:** none found. No TODO/FIXME comments left in codebase. All 39 commands wired into `api.ts`. No dead components.

---

## Competitive and Ecosystem Research

(Full sub-agent research preserved in conversation transcript; condensed here.)

| Source | What to steal | What to avoid |
|---|---|---|
| **Google Keep** | Masonry, color-as-secondary-axis, one-tap text⇄checklist conversion, image-with-OCR overlay as a single note type | Cloud sync, Gemini AI, real-time collab cursors |
| **Joplin** | Web clipper architecture (localhost REST + token, MV3 extension), JEX-style single-file backup archive, "open in external editor" round-trip, ENEX importer | Notebook hierarchy, sync server, opt-in encryption default |
| **Standard Notes** | Command palette (Ctrl+K), data-clobber confirmation modal on import, XChaCha20+Argon2id stack (already match) | Account-required model, sync-first architecture, plugin marketplace |
| **Simplenote** | Version-history slider UI (best in field), instant search (no debounce) | Stagnation (cautionary tale) |
| **Obsidian** | "Files-on-disk are source of truth, app is just the editor" principle — Keepr's Markdown vault export already echoes this | Plugin sprawl, graph view, vault hierarchy |
| **Bear** | Inline `#nested/tags` as body text (Keepr already does flat `#tag`), per-note OCR on image attachments, tag icons | Subscription model, Apple lock-in, nested-tag hierarchy |
| **Notesnook** | **Vericrypt verifier** (independent decrypt-from-DB tool), Vault-inside-encrypted-DB pattern, biometric app-lock | Sync/account, free-vs-Pro feature gating |
| **Anytype** | **12-word BIP39 recovery seed** for key backup, LAN-only P2P sync (only model compatible with "no cloud server") | Object/relation model, community marketplace |
| **QOwnNotes** | Native-feel performance bar (≤150MB resident target), browser Web Companion minimal extension | Nextcloud coupling, in-app MCP/AI, wiki-style linking |
| **Trilium / TriliumNext** | **Daily/weekly/monthly DB backup rotation** (Keepr's auto-backup writes one file, no rotation) | Hierarchical tree, in-note scripting, server/sync |
| **Apple Notes** | **Per-note password lock with biometric unlock**, document scanner (VisionKit) with auto-crop, Smart Folders (saved searches) | iCloud coupling, `.icloud` package format |
| **OneNote** | **Audio recording with timestamp anchors** ("click bullet → jump to that audio second"), OCR over embedded images | Free-form canvas (anti-Keep identity), `.one` format, Copilot integration |

**Distribution & platform:**
- **Azure Trusted Signing** ($9.99/mo, 24h-lived HSM-backed certs, identity validation <1hr) is the 2026 best path off SmartScreen for unsigned-today apps. **EV cert SmartScreen-bypass was removed in 2024** — no reason to pay $250-400/yr for an EV.
- **Microsoft Store as MSIX** is the free path — Store re-signs your package, no SmartScreen prompt, but requires MSIX packaging (which also unlocks the Windows Share Target contract).
- **`tauri-plugin-updater`** with signed manifest is mature; needs only signature-pinned trust anchor in `tauri.conf.json`.
- **macOS notarization** ~$99/yr Apple Developer; not urgent given best-effort tier.
- **WebView2 footguns:** memory bloat dominant complaint (WhatsApp +500MB after migration); aggressively prune renderer caches when minimized to tray.

---

## Highest-Value New Features

### F1. Web Clipper (browser extension + Tauri localhost server)

- **Title:** Localhost-only Web Clipper (MV3 extension + Tauri-spawned REST server)
- **User problem:** Capturing web content currently means "screenshot → save → manually drag into note." No "Save to Keepr" button in browser. Highest-leverage gap for the offline-first promise.
- **Evidence:** Joplin's clipper architecture ([docs](https://joplinapp.org/help/apps/clipper/)) is the established pattern. SECURITY.md commits to "no outbound HTTP" — this clipper satisfies that because the browser is the network party, not Keepr.
- **Proposed behavior:**
  - Tauri spawns a localhost HTTP server on a randomized port at startup; port written to `<data_dir>/clipper-port.txt`
  - Per-install bearer token written to `<data_dir>/clipper-token.txt`
  - Browser extension reads both files via native messaging host (one-time setup) OR user copies token into extension settings
  - Endpoints: `POST /clip` (full HTML), `/clip/markdown` (Readability-cleaned), `/clip/selection` (text only), `/clip/screenshot` (PNG), `/clip/url` (URL only). All token-gated.
  - Each clip creates a new note with title = page title, body = content, label = "Web", optional color
- **Implementation areas:**
  - New Rust dep: `axum` or `hyper` for the localhost server (or roll a 200-line raw `std::net::TcpListener` to avoid the dep)
  - New module: `src-tauri/src/clipper.rs` (HTTP server, token check, route handlers)
  - `lib.rs` wires server start/stop into setup + ExitRequested
  - New `src-tauri/capabilities/default.json` allowance (none needed — server is Rust-side)
  - Bundled MV3 extension at `web-clipper/` (separate directory, ships as a zip in releases)
  - Bundled `Readability.js` + `Turndown.js` inside extension (no CDN)
- **Data/API/UI:** New note kind not required; new `web_clip_url` text column on `notes` (schema v12) for "view source" affordance
- **Risks:** Server binding fails (port in use); token leak via shoulder-surfing the token file; corporate firewall blocks localhost (rare). Mitigations: random port + retry, token regeneration in Settings, document the localhost requirement
- **Verification:** unit-test the route handlers in Rust; manual smoke with the extension against Firefox/Chrome/Edge; verify nothing leaves the machine via `netstat` during a clip
- **Estimated complexity:** L
- **Priority:** P1

### F2. Command Palette (Ctrl+K)

- **Title:** Cmd/Ctrl+K command palette
- **User problem:** 39 commands, 11 settings, dozens of labels, ~80 notes typical — keyboard discovery requires memorizing or hunting menus
- **Evidence:** Standard Notes ships this; near-universal in 2026 productivity apps. Keepr already has FTS5 search + Zustand store + the keyboard-shortcut overlay UX precedent
- **Proposed behavior:** Cmd/Ctrl+K opens centered floating modal with a search input. Fuzzy-match across: every note title, every label, every Settings action, every section (Notes / Pinned / Archive / Trash / Reminders), every keyboard shortcut. Hit Enter to navigate/invoke. Esc closes.
- **Implementation areas:**
  - New component: `src/components/CommandPalette.tsx`
  - New hook: extend `useKeepShortcuts` or new `useGlobalKeydown` for Ctrl+K (and Cmd+K on macOS)
  - Fuzzy match: `fzf-for-js` (1KB) or vendored micro-fuzzy
  - Store: `commandPaletteOpen: boolean`, `openCommandPalette()`, `closeCommandPalette()`
  - No backend changes
- **Data/API/UI:** None new; pure UI
- **Risks:** Conflict with browser Ctrl+K (search bar) — but inside Tauri WebView2 we own the keymap; verify
- **Verification:** Vitest for fuzzy-match algorithm; manual test opens palette / Enter navigates / Esc closes
- **Estimated complexity:** M
- **Priority:** P1

### F3. Recovery Seed at Vault Init (BIP39 12-word phrase)

- **Title:** Vault recovery seed printed at init
- **User problem:** SECURITY.md:53-54: "**Lost-password policy: there is no recovery.**" Real users will lose passwords and lose every vaulted note. Documented loudly but still a foot-gun
- **Evidence:** Anytype ships this — [docs](https://doc.anytype.io/anytype-docs/advanced/data-and-security/how-we-keep-your-data-safe). BIP39 is the industry-standard wordlist (Bitcoin, all major crypto wallets); proven safe + memorizable
- **Proposed behavior:**
  - On `init_vault`, generate a 16-byte random seed; encode as 12 BIP39 words via the standard wordlist
  - Derive an *alternate* KEK from the seed using PBKDF2 or Argon2id at lower cost (the seed is high-entropy, doesn't need m=64MiB)
  - Wrap the DEK twice: once with the passphrase-derived KEK (existing) AND once with the seed-derived KEK
  - Store both wrapped envelopes in `app_settings` (`vault_dek_wrapped`, `vault_dek_seed_wrapped`)
  - Modal at vault init shows the 12 words ONCE, with "I wrote it down" confirmation, copy-to-clipboard, and download-as-PDF buttons
  - Settings → Vault gets a "Recover with seed" path: enter 12 words → unlock + immediately prompt for new passphrase → re-wrap
- **Implementation areas:**
  - `src-tauri/src/vault.rs` — seed generation, dual-wrap, BIP39 wordlist (vendor `bip39` crate, ~100KB)
  - `src-tauri/src/commands.rs` — new `recover_vault_with_seed(words: Vec<String>, new_password: String)` command
  - `src/components/VaultSection.tsx` — seed-display modal at init, recovery flow UI
  - Schema migration v12: add `vault_dek_seed_wrapped BLOB` column to `app_settings` table
- **Data/API/UI:** New 11-line modal at init; new "Recover with seed" button in Settings
- **Risks:** User loses BOTH password and seed (no further recovery); user pastes seed into a phishing site (educate in UI); seed-derived KEK weaker than passphrase-derived (use Argon2id at m=8MiB/t=2 — still robust given seed entropy)
- **Verification:** Rust round-trip test (init → forget password → recover with seed → set new password → decrypt note); manual flow
- **Estimated complexity:** M
- **Priority:** P1

### F4. Audio Voice Notes

- **Title:** Inline audio voice notes
- **User problem:** Keep parity gap; capture-side limitation (think-while-driving, hands-busy)
- **Evidence:** Google Keep has voice notes; OneNote has timestamp-anchored audio. Audio is the most common note-attachment type after images
- **Proposed behavior:**
  - Editor toolbar: microphone icon → starts MediaRecorder via Web API → records `audio/webm; codecs=opus`
  - Recording UI: waveform + elapsed time + Stop / Cancel buttons
  - On stop: save bytes via existing `add_image_attachment_bytes` (rename to `add_blob_attachment`) — extend it to accept arbitrary MIME types (currently image-only)
  - Card preview: audio icon + duration; click plays inline via `<audio>` element pointing at `keepr-resource://<id>.opus`
  - Editor: `<audio controls>` inline
- **Implementation areas:**
  - `commands.rs` — generalize `add_image_attachment_bytes` to `add_blob_attachment`; accept `mime: String` arg; update FTS exclusion logic (audio bytes are not searchable)
  - `NoteEditor.tsx` — new audio-record component (`VoiceRecorderModal`)
  - `NoteCard.tsx` — audio chip preview
  - `AttachmentGrid.tsx` — render `<audio>` for audio mime
  - CSP: already allows `blob:` and `keepr-resource://` for media — verify `media-src` directive includes them (likely needs explicit add to CSP)
- **Data/API/UI:** Reuse `attachments` table; new attachment `kind: "audio"` discriminant
- **Risks:** Permission prompt for microphone first time (browser-level — WebView2 handles); audio file size (cap at 32 MiB existing); voice note + drawing canvas + image all in same note?
- **Verification:** Manual record + playback round-trip on Windows; check it works on macOS too if possible
- **Estimated complexity:** M
- **Priority:** P1

### F5. Per-Note Re-Lock with Windows Hello / Touch ID

- **Title:** Per-note biometric re-lock
- **User problem:** Vault is binary (locked → everything hidden, unlocked → everything visible). Apple Notes pattern: lock a single sensitive note even when the app is otherwise open
- **Evidence:** Apple Notes is the model; Notesnook has a similar "vault-in-vault." `tauri-plugin-biometric` exists in the official `plugins-workspace`
- **Proposed behavior:** Per-note "Lock" icon (visible if vault is initialized + unlocked). Locked note shows placeholder card "🔒 Tap to unlock". Tap → biometric prompt. Unlock duration = current session OR N minutes (Settings).
- **Implementation areas:**
  - `src-tauri/Cargo.toml`: add `tauri-plugin-biometric`
  - `commands.rs`: new `lock_note(id)` / `unlock_note(id)` (in-memory unlock state per-session)
  - `notes.note_locked: bool` column (schema v12 — combine with F3 if shipping same release)
  - `NoteCard.tsx`: locked-card placeholder rendering
  - `NoteEditor.tsx`: locked-note unlock prompt
- **Data/API/UI:** New per-note locked-set in store (`unlockedNoteIds: Set<string>`)
- **Risks:** Biometric APIs differ across platforms; macOS Touch ID needs entitlement; Linux has no equivalent (fall back to PIN)
- **Verification:** Manual lock/unlock on Windows Hello; verify cross-platform graceful degrade
- **Estimated complexity:** M
- **Priority:** P2

### F6. Two-Way `[[Note Title]]` Links + Linked-From Panel

- **Title:** Wiki-style note linking with backlink panel
- **User problem:** Cross-referencing notes today requires copy-paste of titles; no way to navigate from one note to a referenced note
- **Evidence:** Bear's `[[note title]]` is the lightweight model that fits flat-labels Keep identity. Obsidian's full backlink architecture is too heavy; Roam's block references are anti-Keep
- **Proposed behavior:**
  - On note save, regex-extract `\[\[([^\]]+)\]\]` from body; for each, resolve to a note ID by title match; persist in `note_links(source_id, target_id, target_title)` table
  - Editor body: render `[[Foo]]` as a clickable accent-colored span; clicking opens that note's editor
  - `NoteEditor` footer: "Linked from N notes" if backlinks exist; click expands to list
  - Title autocomplete: typing `[[` triggers a dropdown of all note titles (fuzzy-match)
- **Implementation areas:**
  - Schema v12: `note_links` table (FK to notes, cascades on delete)
  - `commands.rs`: `list_backlinks(note_id)` command; `update_note` extracts links + upserts `note_links`
  - `NoteEditor.tsx`: autocomplete dropdown, footer "Linked from" panel
  - `src/lib/wikiLinks.ts` (new): regex extraction + title resolution
- **Data/API/UI:** New `note_links` join table; new footer UI in editor
- **Risks:** Title collisions (two notes with same title); link target deleted (leave dangling link visible but greyed); performance with 1000+ notes (index on `target_title`)
- **Verification:** Vitest for extraction regex; manual cross-link test; check delete-cascade works
- **Estimated complexity:** M
- **Priority:** P2

### F7. Saved Searches / Smart Labels

- **Title:** Smart Labels (saved searches as sidebar entries)
- **User problem:** Power users repeatedly run the same filter combos ("pinned + label:work + has:reminder"). Currently no way to save
- **Evidence:** Apple Notes Smart Folders; Bear's saved searches; Obsidian's Dataview queries (read-only flavor only — Keepr won't ship a query language, just bookmarked filter combos)
- **Proposed behavior:** When a filter is active, "Save as Smart Label" appears next to filter chips. Prompts for a name + icon. Persists to `smart_labels(name, query_json)` table. Renders as sidebar entry below labels; clicking applies the saved filter
- **Implementation areas:**
  - Schema v12: `smart_labels` table
  - `commands.rs`: `list_smart_labels`, `create_smart_label`, `update_smart_label`, `delete_smart_label`
  - `Sidebar.tsx`: render below labels list
  - `FilterChips.tsx`: "Save" button when filter active
  - `store.ts`: `smartLabels` field + actions
- **Data/API/UI:** New table; sidebar entries
- **Risks:** Filter shape may change over time (versioning of `query_json`); user creates 50 smart labels and sidebar bloats
- **Verification:** Round-trip save/load test; manual create/use/delete flow
- **Estimated complexity:** M
- **Priority:** P2

### F8. Bulk Action: Move to/from Vault

- **Title:** Bulk "Move to vault" + "Move out of vault"
- **User problem:** `BulkActionBar.tsx` has pin/color/labels/archive/trash/restore/delete but NOT vault move. User with 30 sensitive notes touches each one individually
- **Evidence:** `move_note_to_vault` is per-note (`commands.rs:2914`); no bulk wrapper. The vault stack is performant enough — one transaction per note is fine for N<1000
- **Proposed behavior:** When 1+ notes selected and vault is initialized and unlocked, two new buttons in `BulkActionBar`: "Move to Vault" (notes outside vault) and "Move out of Vault" (notes inside vault). Confirm dialog on first use; remember choice within session
- **Implementation areas:**
  - `commands.rs`: new `move_notes_to_vault(ids: Vec<String>)`, `move_notes_out_of_vault(ids: Vec<String>)` (wrap existing single-note path in a transaction)
  - `BulkActionBar.tsx`: new buttons gated on vault status
  - `api.ts`: wrappers
- **Data/API/UI:** No schema change
- **Risks:** Partial failure mid-batch (one note's encrypt fails); use transaction so all-or-nothing
- **Verification:** Round-trip Rust test (init vault → bulk move 5 notes → bulk move out → all 5 plaintext again); manual UI flow
- **Estimated complexity:** S
- **Priority:** P1

### F9. Document Scanner (Windows: Microsoft Lens-style)

- **Title:** Document scan via webcam → auto-crop → attach as image
- **User problem:** Capturing a printed page/receipt currently means external tool + paste. Apple Notes and Google Keep both ship this
- **Evidence:** OS-level VisionKit on Apple platforms is free; Windows has no equivalent built-in but `tauri-plugin-camera` exposes the webcam stream. OpenCV WASM (`@techstark/opencv-js`, ~7MB) does the perspective correction client-side
- **Proposed behavior:** Editor toolbar: scan icon → modal with camera preview → user aligns document → tap captures → OpenCV detects rectangle → auto-crops + perspective-corrects → saves as image attachment
- **Implementation areas:** New component `DocumentScannerModal.tsx`; bundle OpenCV WASM in renderer; reuse `add_image_attachment_bytes`
- **Risks:** 7MB WASM blob doubles renderer payload; lazy-load on first scan; camera permission prompt
- **Verification:** Manual scan on Windows with a webcam; verify works on macOS too
- **Estimated complexity:** L
- **Priority:** P3 (skip until F1-F8 are done — high implementation cost, narrower use case)

---

## Existing Feature Improvements

### I1. Cross-Platform CI Coverage

- **Current behavior:** `.github/workflows/ci.yml:12` runs only on `windows-latest`. Release matrix (`release.yml`) builds macOS-13, macOS-14, ubuntu-22.04 — none of which CI ever compiles
- **Problem:** First Linux/macOS regression will land at tag time. Worse, AppKit/GTK-specific bugs hide until users complain
- **Recommended change:** Extend CI matrix to include `macos-14` + `ubuntu-22.04`. Same four steps (`cargo check`, `cargo test`, `npm run lint`, `npm test`, `npm run build`). Add Linux build-dep install step from `release.yml:76-85`. Keep `fail-fast: false`
- **Code locations:** `.github/workflows/ci.yml` (add matrix)
- **Backward compatibility:** None — pure CI change
- **Verification:** Open a PR with a deliberate Linux-only break (e.g. invalid `cfg(unix)` block); confirm CI catches it on ubuntu but lets windows pass
- **Estimated complexity:** S
- **Priority:** **P0**

### I2. Code Signing via Azure Trusted Signing

- **Current behavior:** SECURITY.md:5 — "ships unsigned." SmartScreen warning expected on first launch
- **Problem:** Real adoption friction. First-thousand-users see "Windows protected your PC". SmartScreen reputation builds on volume × time but never starts without a sign
- **Recommended change:** Subscribe to Azure Trusted Signing ($9.99/mo basic). Set up the GitHub Actions integration per [Azure quickstart](https://learn.microsoft.com/en-us/azure/artifact-signing/quickstart). Sign all Windows artifacts in `release.yml`. Update SECURITY.md to reflect "Windows builds signed by Microsoft Trusted Signing"
- **Code locations:** `.github/workflows/release.yml` (add sign step after build, before upload); `SECURITY.md` (update)
- **Backward compatibility:** Signed builds work everywhere unsigned ones did. Cost: $120/yr
- **Verification:** Download a signed v0.19.0 build → properties → digital signatures tab shows Microsoft Trusted Signing. SmartScreen warning expected to fade within a few releases as reputation builds
- **Estimated complexity:** S (after admin setup)
- **Priority:** **P0**

### I3. `tauri-plugin-updater` with Signature Pinning

- **Current behavior:** Backlog item in ROADMAP.md ("Auto-updater scaffold"); no implementation
- **Problem:** Users miss updates. The full release notes / changelog ritual is wasted on users still running v0.16.1 because they never re-visited GitHub
- **Recommended change:** Add `tauri-plugin-updater`. Generate an Ed25519 keypair (private key in GH Actions secret, public key in `tauri.conf.json`). After each tag build, the workflow updates a `latest.json` manifest at a fixed GitHub Releases URL with version + per-platform-asset URLs + Ed25519 signature. App checks once at startup (and once / week if running); offers to download + relaunch
- **Code locations:** `src-tauri/Cargo.toml` (+ plugin), `src-tauri/tauri.conf.json` (updater key + endpoint), `src-tauri/src/lib.rs` (initialize plugin), `.github/workflows/release.yml` (sign manifest, publish to Releases), small `UpdateModal.tsx`
- **Backward compatibility:** Opt-in via Settings ("Check for updates automatically" toggle)
- **Verification:** Manual: install v0.18.x, build a fake v0.19.0 manifest, confirm prompt; check signature mismatch rejects an unsigned manifest
- **Estimated complexity:** M
- **Priority:** **P0**

### I4. Auto-Backup Moved to Rust Background Thread + Rotation

- **Current behavior:** `src/lib/autoBackup.ts` polls every 60s in the *renderer*. If user minimizes Keepr to tray, the renderer keeps running. If user closes the window entirely (not minimizes), the poll dies. Only one backup file at a time (no rotation)
- **Problem:** (a) Renderer-driven background work is fragile, (b) no rotation means a corrupt latest backup loses everything, (c) WebView2 idle-pruning may kill the poll quietly
- **Recommended change:** Move auto-backup to a Rust background thread (`lib.rs` next to the reminder scheduler). Keep 7 daily + 4 weekly rotation files (rolling). Configurable retention (default 7+4)
- **Code locations:** `src-tauri/src/lib.rs` (new thread with same AtomicBool shutdown pattern as reminder scheduler at lines 310-385); `src-tauri/src/commands.rs` (new `auto_backup_now()` for manual trigger); delete `src/lib/autoBackup.ts` poll; settings UI updated
- **Backward compatibility:** Existing `autoBackupCadence` + `autoBackupFolder` settings stay. Last-backup-time moves from localStorage to `app_settings`
- **Verification:** Manual: set cadence to Daily, leave Keepr running for 25h, verify two ZIPs in folder. Force-quit during a backup; verify partial-write doesn't corrupt the keepr.db
- **Estimated complexity:** M
- **Priority:** **P1**

### I5. Content-Addressed Attachment Storage + Orphan Sweep

- **Current behavior:** Attachments stored by UUID at `<data_dir>/resources/<id>.<ext>`. Duplicate images (same bytes, two notes) consume 2× disk. No orphan cleanup — deleting an attachment removes the row but a crash mid-delete strands the blob forever
- **Problem:** Migrating user with 200 photos sees 200 entries; if many are duplicates (re-saved screenshots), disk usage inflates. Orphans accumulate silently
- **Recommended change:** Hash bytes (BLAKE3 or SHA-256), store at `<data_dir>/resources/ab/cd/<hash>.<ext>` (2-char fanout), reference-count via SQLite `attachment_refs(note_id, hash)`. Daily background sweep (in same thread as auto-backup) finds zero-ref hashes >24h old, moves to `<data_dir>/resources/.trash/` (not deleted; user can recover). Empty `.trash/` >30d
- **Code locations:** Schema v13 (or v12 if combined): `attachments.storage_hash` column; `commands.rs` `add_image_attachment*` paths (hash before write); `lib.rs` (sweep thread)
- **Backward compatibility:** Existing UUID-named files keep working; migration on next attachment add moves them into the hashed layout
- **Verification:** Rust test: add 3 identical images to 3 notes, assert only 1 blob on disk; delete all 3 notes, advance time, assert blob moves to .trash. Manual: import a Takeout with duplicate photos, verify dedup
- **Estimated complexity:** L
- **Priority:** **P1**

### I6. Tag Autocomplete in Editor

- **Current behavior:** Type `#word` in note body → on save, `extractHashtagsFromNote` regex auto-creates a label. No suggestion as user types
- **Problem:** Users with 20+ labels type `#workl` when they meant `#work`, creating a duplicate. Once created, only manual rename fixes
- **Recommended change:** When user types `#` followed by 1+ chars, show a dropdown of existing labels matching the prefix. Tab/Enter completes. Up/Down navigates
- **Code locations:** `NoteEditor.tsx` body textarea (or refactor body to a contenteditable with annotations); or simpler: `<HashtagAutocomplete>` overlay anchored to caret
- **Backward compatibility:** None — additive
- **Verification:** Vitest for prefix-match; manual flow type `#wo` → see "work" suggestion → tab completes
- **Estimated complexity:** M
- **Priority:** **P1**

### I7. Vault Verifier CLI (Notesnook Vericrypt pattern)

- **Current behavior:** Users have to trust SECURITY.md's claims about XChaCha20-Poly1305 + Argon2id. No independent verification path
- **Problem:** Power users / journalists / activists — exactly the people most likely to use Private Vault — need verifiable crypto, not promised crypto
- **Recommended change:** Ship a separate `keepr-verify` binary (or `keepr verify` subcommand) that: reads `<data_dir>/keepr.db`, extracts the vault envelope from `app_settings`, prompts for passphrase, re-derives KEK via Argon2id, decrypts a sample vault note, prints the decrypted plaintext. Open-sourced, MIT, ~200 lines of Rust
- **Code locations:** New crate `src-tauri/keepr-verify/` or new `src-tauri/src/bin/verify.rs`. Shares vault.rs logic
- **Backward compatibility:** None — additive tool
- **Verification:** Build the verifier, point at a real keepr.db, decrypt a known sample, compare with what the GUI shows
- **Estimated complexity:** S
- **Priority:** **P2**

### I8. HistoryDrawer Body Diff (existing backlog)

- **Current behavior:** ROADMAP.md backlog: "body diff in HistoryDrawer (currently just a 6-line preview)"
- **Problem:** Restoring an old snapshot is risky without seeing what changed
- **Recommended change:** Bundle `diff-match-patch` (Google, ~17KB minified) or hand-roll a simple line-diff. Render added lines green / removed lines red between the previous + selected snapshots
- **Code locations:** `src/components/HistoryDrawer.tsx`
- **Backward compatibility:** None
- **Verification:** Manual snapshot of a note with 3 different bodies; open drawer; diff renders cleanly
- **Estimated complexity:** S
- **Priority:** **P2**

### I9. "Open Log Folder" Button (in addition to Copy Path)

- **Current behavior:** Settings → Log folder row has "Copy path" button. User must then paste into Explorer
- **Problem:** One extra step every time
- **Recommended change:** Add "Open" button next to Copy path. New Tauri command `open_path(p: String)` using `tauri-plugin-shell` `open()` (or `opener` crate). Permission gate to data/log dirs only
- **Code locations:** `SettingsModal.tsx`, `commands.rs`, `Cargo.toml` (+ `tauri-plugin-opener` if not present)
- **Backward compatibility:** None
- **Verification:** Manual click — Explorer opens at log folder
- **Estimated complexity:** S
- **Priority:** **P2**

### I10. Default `trashRetentionDays` to a Sane Non-Zero

- **Current behavior:** `store.ts DEFAULT_TRASH_RETENTION_DAYS = 7` according to my earlier read at v0.18.x; verify — but UI's Off=0 option may mean fresh installs ship with no auto-purge
- **Problem:** Trash accumulates forever if user never sets it
- **Recommended change:** Verify the current default; if 0, change to 30 days (Keep's web default). If 7, leave alone (matches Keep mobile)
- **Code locations:** `src/store.ts` `DEFAULT_TRASH_RETENTION_DAYS`
- **Estimated complexity:** S
- **Priority:** **P3** (low impact — settings already exposed)

### I11. Search "X to Clear" Affordance + `is:vault` / `has:attachment` Filters

- **Current behavior:** TopBar search has no clear button. Filter chips don't include vault membership or attachment presence
- **Recommended change:** Add `<X>` clear button in search input; add `is:vault`, `has:image`, `has:reminder`, `is:archived` filter chips. FTS5 query stays unchanged; filter chips intersect with text matches in renderer
- **Code locations:** `src/components/TopBar.tsx`, `src/components/FilterChips.tsx`, `src/lib/filterNotes.ts`
- **Estimated complexity:** S
- **Priority:** **P2**

### I12. Window State Persistence (`tauri-plugin-window-state`)

- **Current behavior:** Window opens at 1280×800 on first launch (`tauri.conf.json:17-19`). Subsequent launches don't remember the last position/size
- **Problem:** Annoying on multi-monitor setups
- **Recommended change:** Add `tauri-plugin-window-state` (official). Persists position/size to disk; restores on next launch
- **Code locations:** `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs` (init plugin)
- **Backward compatibility:** Saves state file in app data dir; portable mode follows the same pattern
- **Estimated complexity:** S
- **Priority:** **P2**

### I13. Split `commands.rs` (3866 lines)

- **Current behavior:** Monolithic `commands.rs`. CONTRIBUTING.md:20 acknowledges "split when it grows (refactor planned for v0.6+)" — never actioned. v0.16.0 explicitly closed this as low-ROI churn but the file is now 4× the splitting threshold
- **Problem:** Hard to navigate, slow to compile, intimidating for new contributors
- **Recommended change:** Split into `commands_notes.rs`, `commands_io.rs` (backup/restore/Takeout), `commands_security.rs` (App Lock + Vault), `commands_attachments.rs`, `commands_reminders.rs`, `commands_history.rs`, `commands_labels.rs`. Each ~500 lines. `mod.rs` re-exports. Lib.rs invoke_handler list unchanged
- **Code locations:** `src-tauri/src/commands.rs` → `src-tauri/src/commands/`
- **Risks:** Big diff; merge conflicts. Do it on a quiet day with no other open PRs
- **Verification:** `cargo test --lib` still passes; `lib.rs` invoke_handler list unchanged
- **Estimated complexity:** M
- **Priority:** **P2** (defer until v0.20+ unless touching a section heavily)

---

## Reliability, Security, Privacy, and Data Safety

### Bugs / risks found

- **R1. Pinned stable-grid placeholder may collapse empty rows.** Implemented v0.18.1. `visibility:hidden` cells count for height calc, but if ALL cells in a row are empty, CSS Grid is permitted to collapse the row. **Needs live validation** with a 3-column pinned section where the entire bottom row gets unpinned — does the row stay or collapse? If collapses, the user's "cards stay in place" expectation is partially broken on full-row unpins
- **R2. Auto-backup runs in renderer.** See I4 — fragile to renderer pruning, no rotation
- **R3. Single shared SQLite without external-writer detection.** SECURITY.md:18-19 acknowledges. Not actionable except via documentation. Tauri single-instance guard prevents the most common case (two `keepr.exe`)
- **R4. Argon2id wall-clock claim un-verified at 2026 CPUs.** SECURITY.md:30 claims 150-300ms. If actually faster on modern hardware, App Lock + Vault unlock are more brute-forceable than documented. Re-measure
- **R5. `add_image_attachment_bytes` is image-only by MIME check.** Once F4 (audio) lands, this expands to `image/*` + `audio/*`. Make sure CSP `media-src` is updated
- **R6. No write-ack on `auto_backup_folder` if user picks an iCloud / Dropbox / OneDrive path.** Documented external sync gotcha. Document in Settings: "If this folder syncs to cloud (Dropbox/iCloud/OneDrive), recently-written backups may take time to sync. Wait 5min after backup completes before trusting the cloud copy"
- **R7. Tauri 2.11.2 → minor upgrades may ship.** Currently latest stable. Track Tauri 2.x changelog; bump quarterly minimum
- **R8. `tauri-plugin-global-shortcut` registration failure surfaces as toast (good). What if the OS unregisters mid-session (sleep/resume)? Verify re-registration on resume**

### Missing guardrails

- **G1. No file-integrity check on `keepr.db` opening.** A corrupt DB (interrupted write) currently produces opaque rusqlite errors. Add `PRAGMA integrity_check` on startup; on failure, offer to restore from latest backup
- **G2. No "are you sure" on Delete All Data.** Settings exposes it? Verify; if yes, gate behind typing "DELETE" or holding the button 3 seconds
- **G3. Vault unlock count is unlimited.** Could throttle: after 5 wrong attempts in 60s, sleep an extra 1s per attempt. Argon2id already costly so attack rate is low, but explicit rate-limit makes the intent clear
- **G4. Auto-backup path stored as plaintext.** Settings stores `autoBackupFolder` in localStorage. If laptop is stolen, attacker sees where the backup folder is (e.g. `C:\Users\foo\Dropbox\Keepr-Backups\`). Acceptable for v0.19; document in threat model

### Logging / diagnostics

- **L1. `tauri-plugin-log` writes to app log dir.** Good. But no log-rotation cap — could grow unbounded. Add `max_file_size` config
- **L2. No way to attach logs to bug reports.** Add "Export diagnostic bundle" button in Settings — zips last 7d of logs + DB schema dump (no note content) + version info into a single file the user can attach to issues
- **L3. Crash detection on startup.** If the previous launch crashed (no clean exit marker in `app_settings`), show a recovery toast on next launch offering to upload logs (manually, of course — no telemetry)

---

## UX, Accessibility, and Trust

### Onboarding

- **No first-run tutorial.** Fresh install drops user into an empty Notes grid with no guidance about pin/archive/labels/search shortcuts. Add a one-time onboarding overlay: 4 cards introducing capture, organize, find, secure. Skippable
- **No sample notes.** Apple Notes / Bear ship with welcome notes. Keepr should ship 3-5 sample notes (one with checklist, one with image, one with reminder, one in vault demo) — gives the user something to interact with

### Empty / loading / error states

- **`EmptyState` component referenced in App.tsx — verify it gives section-specific guidance.** "No notes yet — `c` to compose or click Take a note" / "Trash is empty" / "Archive is empty" / "No notes match this label"
- **Loading state currently `<LoadingState />`** — verify it shows for >300ms only (skeleton flash on fast disks is worse than nothing)
- **Error states:** every toast on error currently shows the raw `Err` from Rust. Wrap in user-friendly text where possible

### Destructive / irreversible actions

- **Empty Trash:** has ConfirmDialog ✓
- **Delete Note Forever:** verify confirm
- **Disable App Lock / Vault:** verify the "no recovery" warning is loud
- **Restore Backup:** confirms ✓ (it overwrites the live DB; `.prev` rollback is in place)
- **Add to Vault (per-note):** silent move. Should confirm "Once moved, this note is encrypted. If you forget the vault password, it's gone forever"

### Accessibility

- **Most icon-only buttons probably lack `aria-label`.** Audit via NVDA/Narrator: every `<button>` with only an icon child needs an `aria-label`. The `IconBtn` component (44 lines, `src/components/IconBtn.tsx`) is the right pattern — verify it's used universally
- **Color picker accessibility:** swatches are color-only; need `aria-label` per swatch with the color name. Verify
- **Toast aria-live:** verify the `toasts` array renders inside an `aria-live="polite"` region; if not, screen readers miss them
- **Search has 150ms debounce** ✓ but no `aria-busy` while loading FTS5 results
- **Focus rings are accent-colored** ✓ (v0.18.0 + index.css `*:focus-visible`)
- **Keyboard nav** ✓ for Keep canonical shortcuts (`c` / `/` / `?` / `j` / `k` / `e` / `f` / `#` / `Ctrl+G` / `Ctrl+A`)
- **No `role` on the masonry grid.** Cards aren't in a `role="grid"` so screen readers read them as a stream of buttons. Wrap pinned/others sections in `role="list"` with `role="listitem"` for each card (more accurate than grid given variable layout)

### Microcopy & trust signals

- **Settings → Vault first-run.** Copy currently terse ("Init Vault"). Add: "Choose a strong passphrase. **There is no password recovery.** Write it down or use a password manager. Your data is encrypted with XChaCha20-Poly1305 + Argon2id"
- **Settings → App Lock first-run.** Similar: "App Lock is a UI gate, not at-rest encryption. For at-rest encryption, use the Private Vault below"
- **Auto-backup row.** Add visible "Last backup: 2026-05-26 18:32 UTC" line below the cadence selector (data is in `autoBackupLastAt`)
- **Export ZIP success toast.** Currently "Exported to <path>". Add "Open" / "Show in folder" action button on the toast

---

## Architecture and Maintainability

### Module / boundary improvements

- **A1. `commands.rs` split** (I13 above) — 3866 lines monolith
- **A2. `store.ts` split** (683 lines) — could be sliced via Zustand's `combine` middleware or simple split modules. Slices: theme, view (sort/view/zoom/accent/font), notes, ui (toasts/modals), security (app-lock + vault). Defer until pain
- **A3. `NoteEditor.tsx` at 1051 lines** — already extracted ChecklistSection in v0.16.0; further candidates: `EditorToolbar`, `LabelMenu`, `MoreMenu` (kebab popover). Don't force it; v0.16.0 closed this once
- **A4. `SettingsModal.tsx` at 534 lines** — single scroll list of 10+ rows. Consider tab/section navigation. Could become "General / Appearance / Backup / Security / Advanced" tabs. Defer until adding 3+ new settings rows would push past 700 lines

### Test gaps

- **No frontend test for `useStore` actions** — store has 30+ actions, none directly tested. Add a `store.test.ts` with at least: theme transitions, sort/view persistence round-trip, multi-select edge cases
- **No vault encrypt/decrypt round-trip test in Rust** — `vault.rs` has helpers but no integration test that exercises `init → move_to → move_out → assert plaintext`
- **No FTS5 integration test** — `search_notes` has zero tests. Add: insert 100 notes, search across, assert ranking + count
- **No backup/restore round-trip test** — `export_zip` + `import_zip` together. Add: create 5 notes + 2 attachments → export → wipe DB → import → assert all 5 + 2 present
- **No E2E (Playwright / WebDriver) at all.** CONTRIBUTING.md:39 explicitly defers. Reconsider once auto-updater + clipper land — those flows span renderer + Rust + external (browser, network) and benefit from integration coverage
- **No CSS regression test.** Pinned stable-grid (v0.18.1), blurred backdrop (v0.17.1), accent reskin (v0.18.0), 90vw editor (v0.17.1) — all visual changes with no automated check. Add `@playwright/test` with screenshot snapshots for the 3-4 critical layouts

### Documentation gaps

- **No architecture doc for new contributors.** CONTRIBUTING.md lists "what lives where" — but no "how a note save flows from button click → renderer → Tauri IPC → Rust command → SQLite → response back". A single `docs/architecture.md` with sequence diagrams for: note save, ZIP import, FTS5 search, vault unlock would onboard contributors faster
- **No threat model diagram.** SECURITY.md is excellent prose; a one-page diagram showing what's encrypted vs plaintext on disk would be invaluable for users evaluating the security claims
- **No "How to back up to Dropbox/OneDrive/iCloud safely" guide.** The auto-backup folder picker is silent about the cloud-sync caveats. Add `docs/sync-folders.md` with the gotchas from the Tonsky essay

### Release / build / deployment gaps

- **CI doesn't run on Mac/Linux** (I1 — P0)
- **No signed Windows builds** (I2 — P0)
- **No auto-update flow** (I3 — P0)
- **No reproducible-build verification.** README claims unsigned builds are "built reproducibly from main by GitHub Actions" — but there's no documented verification path. Add a `docs/verify-build.md` showing how to compare local build SHA against the CI artifact SHA
- **No MSIX package + Microsoft Store distribution.** Free signing + Share Target contract + auto-update via Store. Larger bet; see Larger Bets section
- **WebView2 version pin.** `tauri.conf.json` doesn't pin a specific WebView2 runtime channel. Evergreen is the default; works fine but means upgrades land unannounced

---

## Prioritized Roadmap

### Phase A — Distribution credibility (v0.19.0)

- [ ] **P0 — Cross-platform CI matrix**
  - Why: Catches macOS/Linux regressions before they ship via release.yml
  - Evidence: `.github/workflows/ci.yml:12` runs windows-latest only; release.yml builds 4 OSes
  - Touches: `.github/workflows/ci.yml`
  - Acceptance: A deliberate Linux-only typo (e.g., `#[cfg(unix)]` wrong path) fails CI on the ubuntu job and lets windows pass
  - Verify: PR with the typo; check Actions tab

- [ ] **P0 — Sign Windows builds via Azure Trusted Signing**
  - Why: Removes SmartScreen friction; current "click More info → Run anyway" loses users
  - Evidence: SECURITY.md:5; [Azure Artifact Signing pricing](https://azure.microsoft.com/en-us/pricing/details/artifact-signing/)
  - Touches: GH org secrets, `.github/workflows/release.yml` (new sign step before upload), `SECURITY.md` (update wording)
  - Acceptance: Signed `Keepr_0.19.0_x64-setup.exe` Properties → Digital Signatures tab shows Microsoft Trusted Signing; SmartScreen behavior tracked over 2-3 releases
  - Verify: Download release artifact → right-click → Properties → Digital Signatures

- [ ] **P0 — Wire `tauri-plugin-updater` with Ed25519-signed manifest**
  - Why: Users on v0.16.1 today never re-visit GitHub; updater closes the loop
  - Evidence: [Tauri 2 updater plugin](https://v2.tauri.app/plugin/updater/)
  - Touches: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`, `.github/workflows/release.yml`, new `src/components/UpdateModal.tsx`, `SettingsModal.tsx` (auto-update toggle)
  - Acceptance: Install v0.19.0, publish a v0.19.1 release with signed manifest, app prompts within 24h
  - Verify: Manual end-to-end; signature mismatch test with hand-edited manifest

- [ ] **P1 — Window state persistence**
  - Why: Quick win; `tauri-plugin-window-state` official; bad UX without it on multi-monitor
  - Evidence: [plugins-workspace](https://github.com/tauri-apps/plugins-workspace)
  - Touches: `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`
  - Acceptance: Move/resize window, quit, re-launch — window restored to same position+size
  - Verify: Manual

### Phase B — Core capture surface (v0.20.0)

- [ ] **P1 — Web Clipper (browser extension + localhost server)**
  - Why: Highest-leverage capture gap; offline-first promise intact
  - Evidence: Joplin clipper architecture
  - Touches: New `src-tauri/src/clipper.rs`, `src-tauri/src/lib.rs` (spawn + shutdown), bundled `web-clipper/` MV3 extension; schema v12 `notes.web_clip_url`
  - Acceptance: Install extension in Firefox + Chrome + Edge; right-click → Clip → note appears in Keepr with title + body + URL; verify no outbound network via Wireshark
  - Verify: Manual + `netstat -an` check during clip

- [ ] **P1 — Command Palette (Ctrl+K)**
  - Why: Power-user multiplier; standard 2026 productivity UX
  - Evidence: Standard Notes Cmd+K
  - Touches: New `src/components/CommandPalette.tsx`, `store.ts` (`commandPaletteOpen`), `App.tsx` (Ctrl+K listener)
  - Acceptance: Ctrl+K opens; fuzzy-match "set" finds Settings; "wo" finds notes titled "work*" + label `#work`; Enter navigates
  - Verify: Manual; vitest for fuzzy-match algorithm

- [ ] **P1 — Audio voice notes**
  - Why: Keep parity; broadens capture
  - Evidence: Google Keep voice notes; OneNote audio
  - Touches: `commands.rs` (generalize `add_image_attachment_bytes` to `add_blob_attachment`); new `VoiceRecorderModal.tsx`; `AttachmentGrid.tsx` audio render; `tauri.conf.json` CSP `media-src`
  - Acceptance: Editor → mic icon → record 10s → stop → audio chip appears in note; reopen note → playback works
  - Verify: Manual on Windows; file at `<data_dir>/resources/<id>.opus` ≤ 32 MiB

- [ ] **P1 — Bulk "Move to/from Vault"**
  - Why: Real workflow gap for sensitive-info persona
  - Evidence: `BulkActionBar.tsx` missing the action; `move_note_to_vault` is per-note
  - Touches: `commands.rs` (`move_notes_to_vault`, `move_notes_out_of_vault`), `BulkActionBar.tsx`, `api.ts`
  - Acceptance: Init vault, select 5 notes, "Move to Vault", all 5 vault-encrypted in one transaction; partial-failure rolls back all
  - Verify: Rust round-trip test; manual UI

### Phase C — Trust + recovery (v0.21.0)

- [ ] **P1 — Vault recovery seed (BIP39 12-word)**
  - Why: Removes the "lost-password = lost-vault" cliff
  - Evidence: Anytype + every crypto wallet; SECURITY.md:53
  - Touches: Schema v12 (`vault_dek_seed_wrapped`), `vault.rs` (dual-wrap), `commands.rs` (`recover_vault_with_seed`), `VaultSection.tsx` (init modal + recovery flow)
  - Acceptance: Init vault → seed displayed once → "forget" password → enter seed → set new password → previously-vaulted notes still decrypt
  - Verify: Rust round-trip test + manual

- [ ] **P1 — Auto-backup → Rust thread + rotation**
  - Why: Renderer-driven backup is fragile; no rotation = single-point-of-failure
  - Evidence: `src/lib/autoBackup.ts` poll; no rotation logic in `commands.rs export_zip`
  - Touches: `src-tauri/src/lib.rs` (new background thread, same AtomicBool pattern as reminder scheduler), `commands.rs` (`auto_backup_now`), delete `src/lib/autoBackup.ts`, `SettingsModal.tsx` (last-backup line)
  - Acceptance: Cadence=Daily; leave running 25h; verify 2 ZIPs in folder with rotation count enforced
  - Verify: Manual; vitest for rotation logic

- [ ] **P1 — Content-addressed attachment storage + orphan sweep**
  - Why: Dedup + safety net for crash-stranded blobs
  - Evidence: Common pattern (Rails ActiveStorage, Zotero, Obsidian community plugin)
  - Touches: Schema v13 (`attachments.storage_hash`), `commands.rs add_image_attachment*` paths, `lib.rs` sweep thread
  - Acceptance: Add identical image to 3 notes → 1 blob on disk; delete all 3 + age 24h → blob in `.trash/`; restore via CLI
  - Verify: Rust integration test

- [ ] **P1 — Tag autocomplete in editor**
  - Why: Stops duplicate-label proliferation
  - Touches: `NoteEditor.tsx` body input; new `HashtagAutocomplete.tsx`
  - Acceptance: Type `#wo` in body → suggestion dropdown shows `#work`; Tab completes
  - Verify: Vitest + manual

### Phase D — Polish + power-user (v0.22.0)

- [ ] **P2 — Per-note re-lock with biometric (Windows Hello / Touch ID)**
- [ ] **P2 — `[[Note Title]]` two-way links + Linked-from panel**
- [ ] **P2 — Saved searches (Smart Labels)**
- [ ] **P2 — Vault verifier CLI (`keepr-verify`)**
- [ ] **P2 — HistoryDrawer body diff**
- [ ] **P2 — "Open log folder" button**
- [ ] **P2 — Search clear button + `is:vault` / `has:attachment` chips**

### Phase E — Larger Bets / future cycles

- [ ] **P3 — Document scanner (OpenCV WASM)**
- [ ] **P3 — Windows Share Target (requires MSIX packaging)**
- [ ] **P3 — Microsoft Store distribution (MSIX, free signing)**
- [ ] **P3 — `--data-dir <path>` CLI flag for non-portable explicit relocation**
- [ ] **P3 — OneNote-style audio with bullet-anchored timestamps** (depends on Phase B audio)
- [ ] **P3 — Local OCR for image attachments** (bundle Tesseract WASM or windows-native)
- [ ] **P3 — Optional LAN-only P2P sync (Anytype-style, mDNS + Yjs CRDT)** — only sync model compatible with "no cloud server"

---

## Quick Wins

Low-risk, ≤1-day-each:

1. **Cross-platform CI matrix** (I1 / P0) — ~30 minute YAML edit
2. **Window state persistence** (I12 / P1) — plugin add + 2-line init
3. **Open log folder button** (I9 / P2)
4. **Search clear button** (part of I11 / P2)
5. **"Last backup: ..." line in Settings** (part of I4 prep)
6. **First-run sample notes** — seed 3-5 demo notes on fresh DB
7. **`role="list"` + `role="listitem"` on note grid** for accessibility
8. **Audit `IconBtn` `aria-label` coverage** — grep + fill any gaps
9. **Settings → Vault first-run microcopy** (loud "no recovery" warning + crypto stack name)
10. **Default `trashRetentionDays` to non-zero** if currently 0 (verify first)

---

## Larger Bets

Multi-week, need design review:

1. **MSIX packaging + Microsoft Store** — unlocks free signing AND Windows Share Target AND auto-update via Store. Probably 2-3 weeks including Microsoft Partner Center setup + manifest tuning
2. **Auto-backup → Rust thread + rotation** (I4 / P1) — meaningful refactor, schema implications for last-backup-time
3. **Web Clipper end-to-end** (F1 / P1) — Rust HTTP server + MV3 extension + 3-browser test matrix
4. **Vault recovery seed** (F3 / P1) — crypto-adjacent; needs careful review of the dual-wrap design before shipping
5. **Content-addressed attachments + orphan sweep** (I5 / P1) — migration path for existing blobs; sweep thread coordination
6. **Optional LAN-only P2P sync** — bigger bet than the others; requires CRDT design (Yjs/Automerge), mDNS service discovery, conflict UI. Defer to v1.0+

---

## Explicit Non-Goals (carried forward, still binding)

- **Real-time collaboration / multi-user co-edit** — Keepr is single-user
- **Cloud sync server** (Keepr-hosted) — BYO-cloud-folder only, if any sync ever
- **AI features / RAG / autocomplete / Gemini-style transcription** — preserves no-network promise + no-AI promise
- **Account / sign-in** — Keepr's value is the absence of one
- **Telemetry** — same
- **Folders / hierarchy** — labels-only is Keep identity; nested tags also rejected
- **Outliner / block editing** — anti-Keep
- **Feature paywall** — MIT, never fragment
- **User scripts attached to notes** — sandboxing nightmare (rejecting again)
- **In-app extension marketplace** — outbound-HTTP attack surface
- **Plugin API in-renderer with eval()** — same
- **Hosted Web Clipper** (vs. localhost) — would require outbound HTTP
- **Anything that requires admin/elevation** — Keepr runs per-user, never asks for elevation
- **Optical mark recognition (handwriting-to-text)** — Keep-style drawing notes are images; preserves cross-platform parity
- **Location-based reminders** — battery-hungry; Google deprecated theirs
- **Markdown editor** as a replacement for the plain-text editor — Keep notes are plain; keep them plain. Markdown is for export only

---

## Open Questions

Only items that genuinely block correct prioritization:

1. **Cost approval for Azure Trusted Signing ($120/yr)?** P0-I2 hinges on this. If "no budget," fall back to "submit to Microsoft Store as free MSIX path" (more work, same end state)
2. **Acceptable to break the existing "no recovery" promise for Vault by adding seed-based recovery?** Some users explicitly want the "if I forget, it's gone" guarantee. Mitigation: make seed generation **opt-in** at vault init (checkbox: "Generate a recovery seed") rather than default-on. Marks the trade-off clearly
3. **Cross-platform CI cost / Actions minutes budget?** I1 (P0) adds 2-3× CI compute. If runners are constrained, run macOS/Linux only on `main` push, not on every PR
4. **Does pinned stable-grid (v0.18.1) handle the "all cards in a row unpinned" case?** Needs live validation before promoting v0.18.1 in changelog. If broken, decide: leave-as-is (rare case) or render min-height placeholder

---

## Appendix: File:Line Citation Index

Key files for the work above:
- `src-tauri/src/commands.rs` — 3866 lines; 39 command handlers; split candidate (I13)
- `src-tauri/src/lib.rs` — 453 lines; reminder scheduler (310-385), tray, custom protocol, plugins
- `src-tauri/src/db.rs` — 778 lines; SCHEMA_VERSION=11; migrations
- `src-tauri/src/vault.rs` — XChaCha20-Poly1305 + Argon2id KDF
- `src-tauri/src/lock.rs` — App Lock PIN hashing
- `src-tauri/tauri.conf.json:25` — CSP (`default-src 'self' tauri: ipc:`)
- `src-tauri/capabilities/default.json` — capability allow-list
- `src-tauri/Cargo.toml` — Tauri 2.11.2 + 5 plugins
- `.github/workflows/ci.yml:12` — `runs-on: windows-latest` (I1)
- `.github/workflows/release.yml:34-50` — 4-OS release matrix
- `src/components/NoteEditor.tsx` — 1051 lines
- `src/components/SettingsModal.tsx` — 534 lines (10+ rows)
- `src/components/NoteGrid.tsx` — 223 lines, stable-grid + masonry layouts
- `src/components/BulkActionBar.tsx` — 283 lines; vault bulk missing
- `src/store.ts` — 683 lines; 11 localStorage keys; sortNotes (lines 649-677)
- `src/lib/autoBackup.ts` — renderer-side poll (I4)
- `src/__tests__/` — 13 vitest files
- `SECURITY.md:5` — unsigned-builds claim; threat model
- `ROADMAP.md` — 2 backlog items (HistoryDrawer diff, auto-updater)
- `CHANGELOG.md` — v0.18.1 head

---

*End of report. Roadmap items above are ready for an implementing agent — each lists touches/acceptance/verify so a follow-on session can pick any item without redoing the research.*
