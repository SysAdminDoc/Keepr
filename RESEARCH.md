# Research - Keepr

## Executive Summary
Keepr is a mature offline-first Google Keep clone: React/Vite UI, Tauri 2 shell, Rust/SQLite storage, attachments, reminders, vault notes, voice notes, web clipper, Takeout import, and local release artifacts are already in place. The highest-value direction is trust polish rather than broad feature expansion: close the Private Vault attachment gap, make the one opt-in outbound model download explicit and stronger, publish the Web Clipper as a first-class artifact, add end-to-end smoke coverage for the desktop and extension paths, and keep visible version/package metadata from drifting.

Top opportunities in priority order:
- P0 - Vaulted attachment protection: `SECURITY.md` admits attachments remain plaintext under `resources/` even for vaulted notes; competitors market private vaults as whole-note protection.
- P1 - Speech model provenance and network disclosure: `SECURITY.md` still says no outbound network, while `src-tauri/src/transcribe.rs` downloads a Hugging Face model and verifies SHA-1.
- P1 - Web Clipper release quality: `web-clipper/manifest.json` is v0.1.0, has no package/release build, and lacks context-menu/article-capture parity seen in Joplin/Notesnook/Obsidian clippers.
- P1 - Visible version single source: `src/components/SettingsModal.tsx` still renders `Keepr v0.16.1` in a v0.25.0 repo.
- P2 - Local end-to-end smoke coverage: unit coverage is strong, but desktop install, backup/restore, clipper pairing, and vault attachment behavior need exercised flows.
- P2 - Markdown vault import: Keepr exports Markdown, but cannot import the same shape from Obsidian/Joplin-style folders.
- P2 - Dependency upgrade lane: npm has no production advisories, but `npm outdated --long` shows React 19, Vite 8, Tailwind 4, lucide 1.x, and TypeScript 6 migration work.

## Product Map
- Core workflows: quick capture, note/list editing, labels/smart labels/search, reminders, archive/trash/restore, attachments, voice recording/transcription, Private Vault, backup/restore, Takeout import, Markdown export, browser clipping.
- User personas: Keep migrant, privacy-first local notes user, Windows-first desktop user, browser research clipper user, power user with many labels/reminders/attachments.
- Platforms and distribution: Windows supported; macOS/Linux best-effort; Tauri NSIS/MSI/DMG/deb/AppImage configured; Web Clipper is developer-mode only; GitHub Releases require public-source fallback because `gh` auth returned 401 locally.
- Key integrations and data flows: SQLite WAL with schema v14; resources under `<data_dir>/resources`; `keepr-resource://` serves media; localhost clipper binds `127.0.0.1` with bearer auth; optional whisper model download stores weights under `<data_dir>/models`; backup ZIP and Markdown vault export are user-directed file writes.

## Competitive Landscape
- Google Keep: Sets the UX baseline for labels, colors, reminders, search, archive/trash, image notes, drawings, and keyboard shortcuts. Keepr should preserve the fast card model and avoid account/collaboration features that would weaken the local-first identity.
- Joplin: Strong web clipper, attachment/resource model, sync targets, import/export, and conflict handling. Keepr should learn from its clipper packaging and resource lifecycle, but avoid becoming a notebook/tree app.
- Notesnook: Strong privacy positioning around end-to-end encryption, app lock, private vault, and web clipper. Keepr should match the trust expectation for vaulted attachments, but avoid account-gated sync.
- Obsidian: Strong local Markdown vault, plugin ecosystem, and official Web Clipper. Keepr should borrow import/export and clipper ergonomics, but avoid an extension marketplace or scriptable runtime.
- Memos/Blinko: Fast capture, tags, lightweight cards, and self-hosted/local-first adjacent patterns. Keepr should borrow quick capture and tag ergonomics only where they stay local.
- Anytype/AFFiNE/AppFlowy: Local-first workspaces with sync/collaboration options. Keepr should learn from their recovery/import/sync language, but reject workspace/database complexity.
- Standard Notes: Strong encrypted-note and extension discipline. Keepr should learn from explicit threat models and recovery copy, not from paid tiers or hosted accounts.

## Security, Privacy, and Reliability
- Verified: `SECURITY.md` line 57 says vaulted attachments are unencrypted under `resources/`; `src-tauri/src/commands/mod.rs::move_note_to_vault` encrypts title/body/checklist only. This is the main privacy gap because users naturally treat a vaulted image/audio note as fully vaulted.
- Verified: `SECURITY.md` line 9 says Keepr makes no outbound network requests, but `src-tauri/src/transcribe.rs::download_model` downloads from Hugging Face after consent. The product claim needs to say "no background network; one explicit model download".
- Verified: `src-tauri/src/transcribe.rs` pins `MODEL_SHA1_HEX`; SHA-1 is enough to detect accidental corruption but is weak provenance for downloaded executable-adjacent model data. Use SHA-256 or stronger and expose the expected digest in UI/logs.
- Verified: `src/components/WebClipperSection.tsx` uses `confirm()` before token regeneration, contrary to the repo's no-confirm-dialog rule. Replace with the existing `ConfirmDialog` plus toast feedback.
- Verified: `web-clipper/manifest.json` is v0.1.0 while the app is v0.25.0. Decide whether the clipper is versioned independently; if not, sync it with app releases.
- Verified: `npm audit --omit=dev` found 0 production vulnerabilities. `cargo-audit` could not be installed locally; both normal and `--locked` installs failed compiling a transitive `toml_parser`, so RustSec validation needs another toolchain/pass.
- Verified: `gh release list --repo SysAdminDoc/Keepr` failed with 401, so release metadata was not authenticated locally.

## Architecture Assessment
- `src-tauri/src/commands/mod.rs` remains the largest boundary and the existing roadmap already tracks the split; new vault-attachment work should land after or during that split to avoid another oversized command surface.
- Vaulted attachments need a clear storage boundary: either encrypt per-resource blobs when a note enters the vault, store encrypted vault resources separately, or block attachments from vault until migration exists. The best fit is encrypted vault resources served only while the vault DEK is unlocked.
- The clipper is well-isolated (`web-clipper/` plus `src-tauri/src/web_clipper.rs`), but release packaging and context-menu/page extraction are missing. Add tests around auth, CORS origin handling, body caps, and malformed payloads before widening capture modes.
- Settings has grown into many independent trust-sensitive sections. Version metadata should come from package/app metadata, not a hardcoded footer string.
- Test gaps are workflow-level: create note, attach image, move to vault, lock/unlock, verify attachment behavior; export backup, restore into temp data dir; pair clipper, clip page/selection/link; download-model failure and success paths.
- Documentation gaps: README/SECURITY need precise network language; Web Clipper README needs ZIP/load-unpacked release steps; release docs need artifact naming for extension assets.

## Rejected Ideas
- Hosted sync/collaboration: rejected because ROADMAP binding non-goals keep Keepr single-user and BYO cloud-folder only; Anytype/AppFlowy prove the cost is high.
- Hosted Web Clipper service: rejected because it requires outbound HTTP and account/server state; Joplin/Notesnook clip locally or through their app context.
- Cloud transcription/summarization: rejected because ROADMAP allows local offline inference only; use whisper.cpp-style local models.
- In-app extension marketplace/plugin runtime: rejected because ROADMAP bans eval/plugin marketplaces and Obsidian-style plugins would add sandboxing risk.
- Location reminders: rejected because ROADMAP already bans them and Google deprecated the model.
- Full mobile app now: rejected for this cycle because Tauri/mobile UI and platform notification behavior would distract from Windows-supported trust gaps.
- Generic file embeds via `keepr-resource://`: rejected because ROADMAP already keeps the custom protocol limited to app-owned resources.
- OCR/document scanner: already tracked as an existing P3 item, so no duplicate roadmap item.

## Sources
Competitors:
- https://support.google.com/keep/answer/12862970
- https://support.google.com/keep/answer/2888263
- https://support.google.com/keep/answer/6262765
- https://joplinapp.org/help/apps/clipper/
- https://joplinapp.org/help/apps/attachments/
- https://help.notesnook.com/web-clipper
- https://help.notesnook.com/lock-notes-with-private-vault
- https://www.usememos.com/docs
- https://github.com/blinko-space/blinko
- https://docs.anytype.io/
- https://help.obsidian.md/web-clipper
- https://standardnotes.com/help

Platform and standards:
- https://developer.chrome.com/docs/extensions/reference/api/contextMenus
- https://developer.chrome.com/docs/extensions/reference/api/sidePanel
- https://developer.chrome.com/docs/extensions/reference/api/scripting
- https://developer.mozilla.org/en-US/docs/Mozilla/Add-ons/WebExtensions/API/contextMenus
- https://v2.tauri.app/distribute/windows-installer/
- https://v2.tauri.app/distribute/sign/windows/
- https://v2.tauri.app/plugin/updater/
- https://v2.tauri.app/develop/tests/webdriver/
- https://www.w3.org/WAI/WCAG22/quickref/

Dependencies and advisories:
- https://www.npmjs.com/package/react
- https://www.npmjs.com/package/vite
- https://www.npmjs.com/package/tailwindcss
- https://www.npmjs.com/package/lucide-react
- https://www.npmjs.com/package/zustand
- https://rustsec.org/advisories/

## Open Questions
- Should Web Clipper use the app version or an independent extension version? This blocks version-sync implementation.
- Should vaulted attachments be migrated in place, duplicated into encrypted vault-resource storage, or rejected from vault until encryption is ready? This blocks exact migration design.
