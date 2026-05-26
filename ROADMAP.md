# Keepr Roadmap

> Single source of truth for outstanding work. Completed items move to [CHANGELOG.md](CHANGELOG.md). Detailed evidence and rationale for every item live in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md).

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

---

## v0.2 "Trust & Foundations" — SHIPPED 2026-05-25

All P0 audit findings closed. Full notes in [CHANGELOG.md](CHANGELOG.md).

## v0.3 "Power & Parity" — SHIPPED 2026-05-25

Every P1 Keep-parity feature plus the v0.2 deferred items that didn't need new infrastructure. Full notes in [CHANGELOG.md](CHANGELOG.md). The few items still deferred are flagged below.

---

## Phase 3 — v0.4 "Multimodal"

Depends on the `keepr-resource://` protocol scaffolded in v0.2.

- [ ] **NF-01** Image attachments (multi-image per note) [P1, L]
- [ ] **NF-02** Reminders v1 (time-based + recurring + sidebar section + native toast) [P1, L]
- [ ] **NF-07** Inline `#hashtag` labeling (Memos pattern) [P2, M]
- [ ] **NF-08** Markdown-vault export + Google Takeout import [P2, L]
- [ ] **EI-18** SQLite FTS5 backend (only worth it once notes >> 10k) [P2, L]
- [ ] **EI-10** Replace `react-masonry-css` (it works, but the library is unmaintained — pair this with NF-01 to refresh rendering) [P2, M]
- [ ] **NF-20 polish** FLIP animation on "Move checked to bottom" reorder [P3, S]

---

## Phase 4 — v0.5+ "Polish, Power, Sync"

Long-running bets.

- [ ] **NF-10** App Lock + Private Vault (Notesnook two-tier model) [P2, XL]
- [ ] **NF-11** Drawing notes (vector canvas) [P3, L]
- [ ] **NF-12** Image OCR via Windows OCR API [P3, M]
- [ ] **NF-13** Rich URL preview cards [P3, M]
- [ ] **NF-14** Note version history with diff [P3, M]
- [ ] **NF-21** Indent sub-items in checklists (1 level, Keep parity) [P3, M]
- [ ] **NF-22** Background image patterns (Keep's 9 textures, sync everywhere) [P3, S]

---

## Open questions awaiting your call

See [RESEARCH_FEATURE_PLAN.md "Open Questions"](RESEARCH_FEATURE_PLAN.md#open-questions). Plan ships sensible defaults so these don't block forward progress.
