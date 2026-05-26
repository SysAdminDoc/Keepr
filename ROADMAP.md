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

## Phase B — v0.5.0 "Polish & Reliability"

Goal: close every audit P1, raise tests to 100+ vitest / 25+ cargo, ship the first actual binary via `tauri-action` (unsigned per user decision).

- [ ] **NF-V0.5-F** — Bundled release pipeline (`tauri-action`, unsigned MSI + portable zip) + auto-updater scaffold
- [ ] **NF-V0.5-B** — Image thumbnail pipeline (`<id>.thumb.<ext>`, served via protocol)
- [ ] **EI-V0.5-5** — Selection/Esc/filter interactions (gate Esc behind modal state; bulk count uses filtered intersection; auto-clear filters or hint on section switch)
- [ ] **EI-V0.5-6** — Vault + Takeout correctness (export collision detection, Takeout timestamps + reminders preservation)
- [ ] **EI-V0.5-7** — Surface global-hotkey registration failure as a toast
- [ ] **EI-V0.5-8** — `list_notes` payload trim + `patchNote` skip-sort-when-not-touching-sort-keys
- [ ] **EI-V0.5-9** — Hashtag UX corners (auto-remove deleted-text labels, block label-rename when body still references)
- [ ] **Test coverage** — every new Rust command + key frontend pure helpers (~21 cargo + ~10 vitest)

---

## Phase C — v0.5.1 polish nice-to-haves

- [ ] **EI-V0.5-10** — Refactor mega-files (`commands.rs` split; extract `<ChecklistSection>` + `<EditorToolbar>` from `NoteEditor.tsx`; sectionise `SettingsModal.tsx`)
- [ ] **EI-V0.5-11** — Drop unused capability permissions
- [ ] **EI-V0.5-12** — Scheduler shutdown via tokio task (cancel cleanly on app exit)
- [ ] **EI-V0.5-13** — Backup pipeline polish (stream files into zip, mirror import caps to export, insert-then-write order on Takeout)
- [ ] **EI-V0.5-14** — Reminder schema cleanup (drop unused `reminders.id`, add CHECK on `fire_at`)
- [ ] **EI-V0.5-15** — Toolbar density (kebab "More" overflow) + `aria-pressed` audit across `IconBtn` callsites
- [ ] **EI-V0.5-16** — Docs catch-up (README features list, SECURITY threat model, CONTRIBUTING project-layout, stale-banner)
- [ ] **EI-V0.5-17** — Code-split secondary modals via `React.lazy`; tree-shake lucide-react
- [ ] **EI-V0.5-18** — Nits batch (drag-handle whitespace, duplicate re-opens copy, "Later today" label after 6 PM, rename `nextWeek` → `nextMonday`, delete `_UnusedX` + `void useStore`, BulkActionBar Restore icon, `convertFileSrc` memoization, etc.)
- [ ] **NF-V0.5-H** — Per-label note counts in sidebar
- [ ] **NF-V0.5-I** — Paste image from clipboard + drag-drop onto editor
- [ ] **NF-V0.5-J** — `tauri-plugin-log` + Settings → Open log

---

## Phase D — v0.6 "Reminders v2 + Vault" (one of two as headliner)

- [ ] **NF-V0.5-A** — Reminders v2 (recurrence + dedicated sidebar section + snooze actions + in-app toast on fire) [P1, L]
- [ ] **NF-V0.5-C** — App Lock + Private Vault (Notesnook two-tier model, Argon2id + XChaCha20-Poly1305) [P2, XL]

---

## Phase E — v0.7+ "Long-running bets"

- [ ] **NF-V0.5-D** — Note version history with diff [P2, M]
- [ ] **NF-V0.5-E** — Drawing notes (vector canvas) [P3, L]
- [ ] **NF-V0.5-G** — ICS export of reminders [P3, S]
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
