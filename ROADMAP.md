# Keepr Roadmap

> Single source of truth for outstanding work. Completed items move to [CHANGELOG.md](CHANGELOG.md). Detailed evidence and rationale for every item live in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md).

Priority legend: **P0** = data loss / crash / security · **P1** = visible bug / high user value · **P2** = polish / nice-to-have · **P3** = future / experimental.

---

## Phase 1 — v0.2 "Trust & Foundations"

Goal: close every P0 audit finding, add a thin test+CI safety net, ship a real portable bundle, and lay the primitives (attachment protocol, schema migration, FTS) that v0.3 depends on.

### P0 — data safety, security, correctness
- [ ] **EI-01** — Fix zip-slip in `import_zip` (validate entry paths, cap entry count + uncompressed bytes)
- [ ] **EI-02** — `PRAGMA wal_checkpoint(TRUNCATE)` before `export_zip`; fsync the zip file
- [ ] **EI-03** — Safe-swap on import partial failure (`keepr.db.prev` snapshot) + busy gate
- [ ] **EI-04** — Add `PRAGMA user_version` + migration framework in `db.rs`
- [ ] **EI-05** — Set real CSP; drop unused `fs:*` capabilities and `tauri-plugin-fs` dep
- [ ] **EI-06** — Fix editor `existing`-ref clobber (background `load()` no longer wipes typing)
- [ ] **EI-07** — Editor re-entrant close handler + register `onCloseRequested` so ALT-F4 saves drafts
- [ ] **EI-08** — Rewrite `list_notes` from N+1 to 3 bulk queries stitched in Rust
- [ ] **EI-09** — Test suite + GitHub Actions CI scaffold

### P1 — foundation & primitives
- [ ] **NF-resource** — Register `keepr-resource://` protocol + `attachments` table (foundation for NF-01/NF-11/NF-12)
- [ ] **EI-10** — Replace `react-masonry-css` (unmaintained, blocks NF-05)
- [ ] **EI-11** — Real portable EXE bundle target + `portable.flag` mode detection
- [ ] **EI-12** — Fix README data path, bundle-targets claim, ROADMAP "portable EXE" checkbox
- [ ] **EI-13** — Aria-labels + focus rings + focus trap + Escape on Settings/Labels + aria-live toast
- [ ] **EI-14** — Shared `useEscape(closeFn)` hook for all modals
- [ ] **EI-15** — Toast queue + Undo action support
- [ ] **EI-16** — Loading state during initial `load()`
- [ ] **EI-17** — Wrap mutations in try/catch + error toasts
- [ ] **EI-18** — Search debounce (150 ms) + FTS5 backend
- [ ] **EI-19** — Click-outside dismiss + viewport-aware placement for popovers
- [ ] **EI-20** — Replace `window.confirm()` with styled dialogs
- [ ] **EI-21** — Editor archive/trash flushes draft first
- [ ] **EI-22** — Fix lossy `setKind` round-trip (preserve `checked` state)
- [ ] **EI-23** — Don't auto-delete on empty-list close
- [ ] **EI-24** — Optimistic in-place updates (replace full `load()` reflows)
- [ ] **EI-25** — Per-store-slice Zustand subscriptions with `useShallow`
- [ ] **EI-26** — Release mutex before `load_note` re-reads in write commands
- [ ] **EI-27** — Standardize commit style (no `Co-Authored-By` trailer) in CONTRIBUTING.md
- [ ] **EI-28** — Commit `src-tauri/Cargo.lock`
- [ ] **EI-29** — Add CONTRIBUTING.md + SECURITY.md

### P2 — hygiene & polish
- [ ] **EI-30** — Single source of truth for color palette (de-dup `colors.ts` ↔ `tailwind.config.js`)
- [ ] **EI-31** — Shared `<IconBtn>` component
- [ ] **EI-32** — Add `idx_notes_state` SQL index; push filtering into SQL
- [ ] **EI-33** — Cap input sizes server-side (title ≤200, body ≤50 KB, ≤200 items)
- [ ] **EI-34** — Replace `.filter_map(|r| r.ok())` with `.collect::<Result<_,_>>()?`
- [ ] **EI-35** — Strip unused Cargo deps (`serde_json`, `thiserror`, `anyhow`, `walkdir`)
- [ ] **EI-36** — `panic = "abort"` + `lto = true` in release profile
- [ ] **EI-37** — Inline boot script in `<head>` to set dark class before paint
- [ ] **EI-38** — Reduced-motion guard on animations
- [ ] **EI-39** — WCAG contrast pass on dark color variants

### P3 — nits
- [ ] **EI-40** — Don't reset `search` on `setSection` change

---

## Phase 2 — v0.3 "Power & Parity"

After Phase 1 is green.

- [ ] **NF-03** — Keyboard shortcuts (Keep canonical set) + `?` help overlay [P1, M]
- [ ] **NF-04** — Multi-select + bulk actions (pin/archive/trash/color/label) [P1, M]
- [ ] **NF-05** — Drag-reorder notes + checklist items + Custom sort menu [P1, L]
- [ ] **NF-06** — System-tray icon + global hotkey quick-capture [P1, M]
- [ ] **NF-16** — Theme "System default" option + native title-bar theme matching [P1, S]
- [ ] **NF-09** — Search filter chips (type / color / label / has-reminder) [P2, M]
- [ ] **NF-15** — Auto-backup schedule (daily/weekly ZIP to chosen folder) [P2, S]
- [ ] **NF-17** — Configurable trash retention + days-remaining badge [P2, S]
- [ ] **NF-18** — "Make a copy" (duplicate note) [P2, S]
- [ ] **NF-19** — "Show/Hide checkboxes" toggle in editor menu [P2, S]
- [ ] **NF-20** — "Move checked to bottom" with FLIP animation [P2, M]
- [ ] **NF-23** — List view toggle (Ctrl+G) [P2, S]

---

## Phase 3 — v0.4 "Multimodal"

Depends on Phase 1's `keepr-resource://` protocol.

- [ ] **NF-01** — Image attachments (multi-image per note) [P1, L]
- [ ] **NF-02** — Reminders v1 (time-based + recurring + sidebar section + native toast) [P1, L]
- [ ] **NF-07** — Inline `#hashtag` labeling (Memos pattern) [P2, M]
- [ ] **NF-08** — Markdown-vault export + Google Takeout import [P2, L]

---

## Phase 4 — v0.5+ "Polish, Power, Sync"

Long-running bets.

- [ ] **NF-10** — App Lock + Private Vault (Notesnook two-tier model) [P2, XL]
- [ ] **NF-11** — Drawing notes (vector canvas) [P3, L]
- [ ] **NF-12** — Image OCR via Windows OCR API [P3, M]
- [ ] **NF-13** — Rich URL preview cards [P3, M]
- [ ] **NF-14** — Note version history with diff [P3, M]
- [ ] **NF-21** — Indent sub-items in checklists (1 level, Keep parity) [P3, M]
- [ ] **NF-22** — Background image patterns (Keep's 9 textures, sync everywhere) [P3, S]

---

## Open questions awaiting your call

See [RESEARCH_FEATURE_PLAN.md "Open Questions"](RESEARCH_FEATURE_PLAN.md#open-questions). Plan ships sensible defaults so these don't block Phase 1.
