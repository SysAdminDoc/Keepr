# Keepr Roadmap

> Single source of truth for outstanding work. Completed items move to [CHANGELOG.md](CHANGELOG.md). Detailed evidence and rationale for every item live in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md).

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

---

## v0.2 "Trust & Foundations" — SHIPPED 2026-05-25

All P0 audit findings closed. Full notes in [CHANGELOG.md](CHANGELOG.md).

## v0.3 "Power & Parity" — SHIPPED 2026-05-25

Every P1 Keep-parity feature plus the v0.2 deferred items that didn't need new infrastructure. Full notes in [CHANGELOG.md](CHANGELOG.md).

## v0.4 "Multimodal" — SHIPPED 2026-05-25

Image attachments, reminders, inline `#hashtag` labels, Markdown vault export, Google Takeout import. Schema at v3. Full notes in [CHANGELOG.md](CHANGELOG.md).

---

## Phase 4 — v0.5+ "Polish, Power, Sync"

Long-running bets. Pick by user demand.

- [ ] **NF-10** App Lock + Private Vault (Notesnook two-tier model) [P2, XL]
- [ ] **NF-11** Drawing notes (vector canvas) [P3, L]
- [ ] **NF-12** Image OCR via Windows OCR API [P3, M]
- [ ] **NF-13** Rich URL preview cards [P3, M]
- [ ] **NF-14** Note version history with diff [P3, M]
- [ ] **NF-21** Indent sub-items in checklists (1 level, Keep parity) [P3, M]
- [ ] **NF-22** Background image patterns (Keep's 9 textures, sync everywhere) [P3, S]

### Deferred from earlier phases (no perf/UX trigger to ship yet)

- [ ] **EI-10** Replace `react-masonry-css` — still works inside @dnd-kit; replacement is risk-without-benefit until masonry actually misbehaves [P2, M]
- [ ] **EI-18** SQLite FTS5 backend for search — client-side `filterNotes` runs in <1ms at 1k notes; FTS5 pays off above 10k [P2, L]
- [ ] **NF-02 v2** Reminder recurrence (RRULE) + dedicated Reminders sidebar section + toast snooze actions [P2, L]
- [ ] **NF-20 polish** FLIP animation on "Move checked items to bottom" reorder [P3, S]

---

## Open questions awaiting your call

See [RESEARCH_FEATURE_PLAN.md "Open Questions"](RESEARCH_FEATURE_PLAN.md#open-questions). Plan ships sensible defaults so these don't block forward progress.
