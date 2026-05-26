# Keepr Roadmap

> Single source of truth for outstanding work. Completed items move to [CHANGELOG.md](CHANGELOG.md). Long-form rationale lives in [RESEARCH_FEATURE_PLAN_v0.5.md](RESEARCH_FEATURE_PLAN_v0.5.md). The original [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md) is kept as historical reference for the v0.1 → v0.2 transition.

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

---

## Shipped

- **v0.2 "Trust & Foundations"** — 2026-05-25 — every P0 audit finding closed
- **v0.3 "Power & Parity"** — 2026-05-25 — Keep canonical parity + power-user
- **v0.4 "Multimodal"** — 2026-05-25 — image attachments, reminders, hashtags, vault/Takeout I/O

Full notes per release in [CHANGELOG.md](CHANGELOG.md).

---

## Phase A — v0.4.1 hotfix — SHIPPED 2026-05-26

All four P0s from the v0.5 audit closed. Full notes in [CHANGELOG.md](CHANGELOG.md).

- [x] **EI-V0.5-2** — Reminder lost-toast fix (two-phase fire + retry on failure)
- [x] **EI-V0.5-3** — Empty-note + reminder orphan
- [x] **EI-V0.5-1 (drag)** — Refuse drag in non-Notes sections + position backfill migration (schema v4)
- [x] **EI-V0.5-4** — `tauri-plugin-single-instance`

---

## Phase B — v0.5.0 "Polish & Reliability" — SHIPPED 2026-05-26

Audit P1 batch + first bundled-release pipeline. Full notes in [CHANGELOG.md](CHANGELOG.md).

- [x] **NF-V0.5-F** — Bundled release pipeline (`tauri-action`, unsigned MSI + NSIS + portable zip)
- [x] **NF-V0.5-B** — Image thumbnail pipeline (480 px `<id>.thumb.jpg`, served via protocol with `onError` fallback)
- [x] **EI-V0.5-5** — Selection/Esc/filter interactions tightened
- [x] **EI-V0.5-6** — Vault + Takeout correctness (per-run subfolder; Takeout timestamps + reminders preserved)
- [x] **EI-V0.5-7** — Global-hotkey failure surfaces as toast
- [x] **EI-V0.5-8** — `patchNote` skip-sort when patch doesn't touch sort keys
- [x] **EI-V0.5-9** — Hashtag auto-detach on text removal + title-hashtag highlighting
- [x] **Test coverage** — 89 automated checks (20 cargo + 69 vitest)
- [ ] *Auto-updater scaffold deferred to v0.5.1 — needs published Releases manifest first*
- [ ] *EI-V0.5-8 list_notes payload trim deferred — couldn't quantify visible win at current scale*

---

## Phase C — v0.5.1 polish nice-to-haves — SHIPPED 2026-05-26

Selective; the items most visible to users landed, the heavier refactors slip to v0.5.2+. Full notes in [CHANGELOG.md](CHANGELOG.md).

- [x] **EI-V0.5-11** — Drop unused capability permissions
- [x] **EI-V0.5-13** — Backup pipeline polish (stream files into zip, mirror import caps to export, insert-then-write order on Takeout)
- [x] **EI-V0.5-15** (partial) — `aria-pressed` audit across `IconBtn` callsites (kebab overflow deferred)
- [x] **EI-V0.5-16** — Docs catch-up (README features list, SECURITY threat model, CONTRIBUTING project-layout)
- [x] **EI-V0.5-18** — Nits batch (rename `nextWeek` → `nextMonday`, delete `_UnusedX` + `void useStore`, BulkActionBar Restore icon, etc.)
- [x] **NF-V0.5-H** — Per-label note counts in sidebar
- [x] **NF-V0.5-I** — Paste image from clipboard + drag-drop onto editor
- [ ] **EI-V0.5-10** — Refactor mega-files (`commands.rs` split; extract `<ChecklistSection>` + `<EditorToolbar>` from `NoteEditor.tsx`; sectionise `SettingsModal.tsx`) — deferred to v0.6.1
- [ ] **EI-V0.5-12** — Scheduler shutdown via tokio task — deferred
- [ ] **EI-V0.5-14** — Reminder schema cleanup (drop unused `reminders.id`, add CHECK on `fire_at`) — deferred
- [ ] **EI-V0.5-15** (rest) — Kebab "More" overflow on the editor toolbar — deferred
- [ ] **EI-V0.5-17** — Code-split secondary modals via `React.lazy`; tree-shake lucide-react — deferred
- [ ] **NF-V0.5-J** — `tauri-plugin-log` + Settings → Open log — deferred

---

## Phase D — v0.6.0 "Reminders v2" — SHIPPED 2026-05-26

- [x] **NF-V0.5-A** — Reminders v2 (recurrence whitelist `FREQ=DAILY|WEEKLY|MONTHLY|YEARLY`, dedicated Reminders sidebar section, snooze panel, in-app fire toast with View-note action)

## Phase D — v0.7.0 "App Lock" — SHIPPED 2026-05-26

NF-V0.5-C split in half: App Lock (UI-gating PIN with Argon2id) shipped first; Private Vault (per-note at-rest encryption with XChaCha20-Poly1305) shipped right after in v0.8.0. Full notes in [CHANGELOG.md](CHANGELOG.md) and the threat model in [SECURITY.md](SECURITY.md).

- [x] **NF-V0.5-C (App Lock)** — Argon2id PHC + LockScreen overlay + idle auto-lock + Settings panel + lost-PIN-no-recovery policy documented

## Phase D — v0.8.0 "Private Vault" — SHIPPED 2026-05-26

- [x] **NF-V0.5-C (Private Vault)** — schema v6, XChaCha20-Poly1305 per-note AEAD with note-id AAD, Argon2id-derived KEK wrapping a random DEK, change-password via rewrap only, vault Settings section, NoteEditor Lock/Unlock action, NoteCard locked-placeholder + vaulted badge, App Lock idle fire also drops the DEK

---

## Phase E — v0.9.0 "History & Calendar" — SHIPPED 2026-05-26

- [x] **NF-V0.5-D** — Note version history with one-click restore (last 20 per note, vault snapshots store ciphertext as-is)
- [x] **NF-V0.5-G** — ICS export of active reminders (RFC 5545 VCALENDAR/VEVENT, RRULE preserved, vault titles redacted)

## Phase E — v0.10+ "Long-running bets remaining"

- [ ] **NF-V0.5-E** — Drawing notes (vector canvas) [P3, L]
- [ ] **NF-V0.5-K** — macOS + Linux CI matrix (no platform support promise) [P3, S]
- [ ] **NF-12** Image OCR via Windows OCR API [P3, M]
- [ ] **NF-13** Rich URL preview cards [P3, M]
- [ ] **NF-21** Indent sub-items in checklists [P3, M]
- [ ] **NF-22** Background image patterns (Keep's 9 textures) [P3, S]
- [ ] **EI-10** Replace `react-masonry-css` [P2, M]
- [ ] **EI-18** SQLite FTS5 backend [P2, L]
- [ ] **NF-20 polish** FLIP animation on Move-checked-to-bottom [P3, S]

---

## Resolved open questions

- **Code-signing strategy** — unsigned for now. README will document the SmartScreen workaround. Revisit when distribution scale justifies a cert.

See [RESEARCH_FEATURE_PLAN_v0.5.md "Open Questions"](RESEARCH_FEATURE_PLAN_v0.5.md#open-questions) for the remaining undecided defaults.
