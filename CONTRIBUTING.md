# Contributing to Keepr

Thanks for your interest. Keepr is a small, focused offline Google Keep clone — please read this short page before opening a PR so your work lands smoothly.

## Scope guardrails

Keepr is intentionally narrow. Before proposing a feature, sanity-check it against [RESEARCH_FEATURE_PLAN.md "Explicit Non-Goals"](RESEARCH_FEATURE_PLAN.md#explicit-non-goals). Examples of out-of-scope work: hierarchical folders, outliner editing, multi-user real-time collaboration, sync servers, AI features, telemetry, account creation, feature paywalls.

## Project layout

- `src/` — React + TypeScript renderer (the UI)
- `src-tauri/` — Rust backend, SQLite schema, Tauri commands
- `src-tauri/src/db.rs` — schema + migration framework (bump `SCHEMA_VERSION` + add a `MIGRATION_V<n>` block + an arm in `apply_migration` for every schema change)
- `src-tauri/src/commands.rs` — every Tauri command lives here today; split when it grows
- `src/__tests__/` — vitest unit tests
- `RESEARCH_FEATURE_PLAN.md` — the long-form research and rationale for every EI-/NF- item on the roadmap
- `ROADMAP.md` — the live task list
- `CHANGELOG.md` — what shipped in each release

## Commit style

- Conventional commit prefixes: `feat:`, `fix:`, `perf:`, `chore:`, `docs:`, `test:`, `refactor:`. A scope is encouraged (`feat(editor): ...`).
- Subject line under ~72 chars.
- **Do not include a `Co-Authored-By: Claude` trailer.** This repo standardizes on no AI-attribution trailer in commit bodies; commit messages should describe what the change does and which roadmap item (EI-NN / NF-NN) it advances.

## Tests

- Every Rust change in `commands.rs` or `db.rs` should keep `cargo test --lib` green; add a unit test in the same `#[cfg(test)] mod tests` block if behavior is non-trivial.
- Pure helpers in the frontend belong in `src/lib/` so they can be covered by vitest in `src/__tests__/`.
- Don't add Playwright / tauri-driver E2E for small UI changes — manual smoke is fine. Reserve E2E for flows that span at least two screens.

## Before you push

```sh
npm test
npm run build
cargo test --manifest-path src-tauri/Cargo.toml --lib
cargo check --manifest-path src-tauri/Cargo.toml --all-targets
```

CI runs the same four steps on every push to `main` and every pull request.

## Pull request checklist

- [ ] Roadmap item ID(s) referenced in the commit body
- [ ] CHANGELOG.md `## Unreleased` updated if user-visible
- [ ] New schema changes bump `SCHEMA_VERSION` and add a migration
- [ ] New commands listed in `src-tauri/src/lib.rs` invoke_handler
- [ ] Tests for new pure helpers and any new Rust command path
- [ ] No `console.log` / `dbg!` left behind
- [ ] No new `unwrap()` / `expect()` in renderer-facing Rust paths — return `Result<_, String>` so the UI can show a toast

## Reporting a bug

Open a GitHub issue with: Keepr version, OS, reproduction steps, expected vs actual. If it's a data-safety issue (anything in import/export, schema, or the editor), please follow [SECURITY.md](SECURITY.md) and report privately first.
