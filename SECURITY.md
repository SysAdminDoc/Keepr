# Security Policy

## Code signing

Keepr ships **unsigned** as of v0.5. Windows SmartScreen will warn on first launch — click "More info" → "Run anyway". Signed builds will land when distribution scale justifies a $300/year EV certificate; the alternative is sigstore which is still nascent for `.msi` artifacts. The unsigned binary is built reproducibly from `main` by GitHub Actions — verify against the workflow logs at [`.github/workflows/release.yml`](.github/workflows/release.yml).

## Threat model

Keepr is a single-user, offline-first desktop app. It does **not** make outbound network requests, has no account system, and does not collect telemetry. The threats it actively defends against are:

1. **Malicious backup files.** ZIP imports are validated for path traversal, entry count, per-file uncompressed size, and total uncompressed size. A failing restore rolls back to the previous database via a `.prev` snapshot.
2. **Renderer code execution surface.** Tauri's content security policy is locked down to `'self'` origins plus the `keepr-resource://` protocol for attachments; no inline scripts. The `fs:*` capability is not granted to the renderer — all file I/O is in Rust commands.
3. **Schema drift on upgrade.** `PRAGMA user_version` plus a forward-only migration framework prevent partial schema application; databases from a newer Keepr are rejected with a clear message rather than silently corrupted.

The threats it does **not** currently defend against (and where to expect future work):

- **Stolen unlocked laptop.** Notes are plaintext SQLite. NF-10 (App Lock + Private Vault) will add this.
- **Adversary with disk write access while Keepr is open.** A second writer to `keepr.db` outside Keepr's mutex is not detected.
- **Local resource exhaustion.** The 2 GiB total + 512 MiB per-file uncompressed caps on import are sized for normal use, not a malicious operator with admin access.

## Reporting a vulnerability

If you find a security issue — especially anything around the backup pipeline, schema migration, or the Tauri command surface — please **do not** open a public GitHub issue.

- Email: `matt@mavenimaging.com`
- GitHub: open a private security advisory at https://github.com/SysAdminDoc/Keepr/security/advisories/new
- We'll acknowledge within 5 working days and aim to ship a fix within 30.

## Supported versions

Keepr is pre-1.0; only the latest release receives fixes. We'll backport critical patches to the immediately-previous minor if a database upgrade is risky.

## Security-relevant change history

See [CHANGELOG.md](CHANGELOG.md). v0.2 closes the P0 audit items documented in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md): EI-01 (zip-slip), EI-02 (WAL pages lost on export), EI-03 (safe-swap on partial restore), EI-04 (schema migration), EI-05 (CSP + capability tightening).
