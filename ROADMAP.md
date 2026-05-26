# Keepr Roadmap

> Single source of truth for **outstanding work only**. Shipped batches and per-release notes live in [CHANGELOG.md](CHANGELOG.md). Long-form rationale for the v0.5+ items lives in [RESEARCH_FEATURE_PLAN_v0.5.md](RESEARCH_FEATURE_PLAN_v0.5.md); the original v0.1→v0.2 research is in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md). Both research files are historical reference — when they disagree with this roadmap, this roadmap wins.

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

Releases shipped 2026-05-25 → 2026-05-26: **v0.2** → **v0.11.0** (twelve milestones, every NF-V0.5-* roadmap item closed). See [CHANGELOG.md](CHANGELOG.md).

---

## Open — actively planned

These are the items still on deck. They batch into the next handful of minor releases below.

### v0.12.0 "Plumbing pass" — SHIPPED 2026-05-26

- [x] **NF-V0.5-J** — `tauri-plugin-log` wired up + Settings → "Log folder" row with Copy-path button
- [x] **EI-V0.5-14** — Reminder schema cleanup (schema v8 drops `reminders.id`, makes `note_id` the PK, adds CHECK on `fire_at`)
- [x] **EI-V0.5-12** — Scheduler shutdown via `AtomicBool` checked between 1-second sleep slices; wired to `RunEvent::ExitRequested`

### v0.13.0 "Search depth"

- [ ] **EI-18** — SQLite FTS5 backend for search (currently every keystroke runs `filterNotes` over the full note set with `toLowerCase().includes(…)`). Adds `notes_fts` virtual table + triggers; moves search to a Rust command. 150 ms debounce on the input.

### v0.14.0 "Checklist & textures"

- [ ] **NF-21** — Indent sub-items in checklists (1 level only, Keep parity). Tab/Shift+Tab. Adds `parent_id` to `checklist_items`.
- [ ] **NF-22** — Background image patterns (Keep's 9 textures, syncs everywhere). New `background_pattern` column; 9 SVG patterns.
- [ ] **NF-20 polish** — FLIP animation on Move-checked-to-bottom (the data behavior shipped in v0.3; only the animation is left).

### v0.15.0 "Frontend polish"

- [ ] **EI-10** — Replace `react-masonry-css` (unmaintained since Aug 2022; no virtualization). Adopt `masonic` or roll a CSS-Grid `grid-template-rows: masonry` component with a `ResizeObserver` shim for non-Firefox.
- [ ] **EI-V0.5-17** — Code-split secondary modals via `React.lazy` (SettingsModal, LabelsManager, HistoryDrawer, ReminderPicker, VaultSection, AppLockSection, DrawingCanvasModal); tree-shake `lucide-react` to per-icon imports.
- [ ] **EI-V0.5-15** (rest) — Kebab "More" overflow on the editor toolbar (current toolbar is wide; collapse non-essential actions into a dropdown when window narrows).

### v0.16.0 "Refactor pass"

- [ ] **EI-V0.5-10** — Split mega-files. `src-tauri/src/commands.rs` is ~3 k lines — split into `commands/notes.rs`, `commands/labels.rs`, `commands/reminders.rs`, `commands/vault.rs`, `commands/backup.rs`, `commands/io.rs`. `src/components/NoteEditor.tsx` (~1100 lines) — extract `<ChecklistSection>` and `<EditorToolbar>`. `src/components/SettingsModal.tsx` (~400 lines) — sectionise into a `<SettingsSection title=…>` shell.

---

## Open — backlog / not actively scheduled

Items kept in the roadmap so they don't get forgotten, but no current intent to ship.

- [ ] **NF-V0.5-D follow-up** — body **diff** in HistoryDrawer (currently just a 6-line preview). Adds `diff-match-patch` or a small line-diff. Low value while the per-snapshot body cap is small.
- [ ] **Auto-updater scaffold** — needs published Releases manifest first, which requires either an EV cert or an accepted "ship unsigned and warn users" path. Revisit when distribution scale justifies it.

---

## Won't ship (rescoped from the original research plan)

These were on the v0.1 research list but conflict with Keepr's actual design promises. Documented here so they don't keep getting re-added.

- **NF-12 — Image OCR (Windows OCR API)**: requires per-platform OCR backends (`windows-rs::Windows::Media::Ocr` on Windows, Vision framework on macOS, Tesseract on Linux). The cross-platform CI shipped in v0.10 commits us to keeping the three platforms feature-parity; bundling a multi-MB OCR engine into every build (or shipping a Windows-only feature) both fail that bar. Users who need OCR can use the OS-side tool (Snipping Tool, macOS Live Text, GNOME OCR) and paste the extracted text into a Keepr note.
- **NF-13 — Rich URL preview cards**: requires outbound HTTP fetches of every pasted URL. Directly contradicts the README + SECURITY promise that "Keepr does not make outbound network requests". Cannot be opt-in-only either because once an opt-in network surface exists, "is Keepr actually offline?" becomes a per-user-config question. Won't ship.

---

## Explicit non-goals

(carried forward from the original [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md), still binding)

- Collaboration / real-time co-edit — out of scope; single-user offline.
- Location-based reminders — battery hungry; doesn't fit a desktop app.
- Folders / hierarchy — Keep identity is flat; Labels covers it.
- Outliner / block editing — anti-Keep.
- AI features / RAG / autocomplete — preserves the no-network promise.
- Account / sync server — Keepr's value is the absence.
- Telemetry — same.
- Feature paywall — MIT; never fragment.
- User scripts attached to notes — sandboxing nightmare.
- Custom protocol expansion to arbitrary file types — `keepr-resource://` stays strictly for images/audio/drawings.
- Built-in cloud-sync without user-managed credentials — even when (if) sync ships, it should be "watch a folder you point at your existing cloud sync" rather than a Keepr-managed server.

---

## Resolved decisions

- **Code-signing (v0.5+)** — ship unsigned with the SmartScreen workaround documented in [SECURITY.md](SECURITY.md). Revisit when distribution scale justifies a cert.
- **macOS / Linux support tier (v0.10+)** — Windows is the **supported** channel; macOS + Linux are **best-effort** binaries built by the CI matrix so the codebase stays portable. No promise to fix platform-specific bugs.
- **App Lock + Private Vault lost-credential policy (v0.7/v0.8+)** — no recovery, documented loudly. The data on disk is recoverable for App Lock (just delete the PHC row); the data for vaulted notes is permanently inaccessible without the password.
- **Reminder scheduler granularity (v0.4+)** — 30-second poll interval. Documented up-to-30-s lag is acceptable. Revisit if anyone files a bug.
