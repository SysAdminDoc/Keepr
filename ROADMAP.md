# Keepr Roadmap

> Single source of truth for outstanding work. Completed items move to [CHANGELOG.md](CHANGELOG.md). Detailed evidence and rationale for every item live in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md).

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

---

## v0.2 "Trust & Foundations" — SHIPPED 2026-05-25

All P0 audit findings closed. Full notes in [CHANGELOG.md](CHANGELOG.md). Items deferred from v0.2 are now scheduled below.

---

## Phase 2 — v0.3 "Power & Parity"

Highest priority for v0.3: items deferred from v0.2 (drag-reorder needs the masonry replacement first), then Keep parity features.

### Deferred from v0.2

- [ ] **EI-10** Replace `react-masonry-css` with a maintained library that supports virtualization + drag (unblocks NF-05)
- [ ] **EI-18** SQLite FTS5 backend for search (debounce already landed)
- [ ] **EI-24** Full optimistic in-place store updates (commands already return the new Note; just need store reducers)
- [ ] **EI-25** Remaining `useShallow` selectors (slice subscriptions landed in components; sweep remaining call sites)
- [ ] **EI-30** Single source of truth for color palette (de-dup `colors.ts` ↔ `tailwind.config.js`)
- [ ] **EI-39** Full WCAG contrast pass on dark color variants (spot-checked OK; needs a meter)

### v0.3 features

- [ ] **NF-03** Keyboard shortcuts (Keep canonical set) + `?` help overlay [P1, M]
- [ ] **NF-04** Multi-select + bulk actions (pin/archive/trash/color/label) [P1, M]
- [ ] **NF-05** Drag-reorder notes + checklist items + Custom sort menu [P1, L]
- [ ] **NF-06** System-tray icon + global hotkey quick-capture [P1, M]
- [ ] **NF-16** Theme "System default" option + native title-bar theme matching [P1, S]
- [ ] **NF-09** Search filter chips (type / color / label / has-reminder) [P2, M]
- [ ] **NF-15** Auto-backup schedule (daily/weekly ZIP to chosen folder) [P2, S]
- [ ] **NF-17** Configurable trash retention + days-remaining badge [P2, S]
- [ ] **NF-18** "Make a copy" (duplicate note) [P2, S]
- [ ] **NF-19** "Show/Hide checkboxes" toggle in editor menu [P2, S]
- [ ] **NF-20** "Move checked to bottom" with FLIP animation [P2, M]
- [ ] **NF-23** List view toggle (Ctrl+G) [P2, S]

---

## Phase 3 — v0.4 "Multimodal"

The `keepr-resource://` protocol scaffold landed in v0.2, so these features can hook into it without new infrastructure.

- [ ] **NF-01** Image attachments (multi-image per note) [P1, L]
- [ ] **NF-02** Reminders v1 (time-based + recurring + sidebar section + native toast) [P1, L]
- [ ] **NF-07** Inline `#hashtag` labeling (Memos pattern) [P2, M]
- [ ] **NF-08** Markdown-vault export + Google Takeout import [P2, L]

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
