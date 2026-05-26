# Security Policy

## Code signing

Keepr ships **unsigned** as of v0.5. Windows SmartScreen will warn on first launch — click "More info" → "Run anyway". Signed builds will land when distribution scale justifies a $300/year EV certificate; the alternative is sigstore which is still nascent for `.msi` artifacts. The unsigned binary is built reproducibly from `main` by GitHub Actions — verify against the workflow logs at [`.github/workflows/release.yml`](.github/workflows/release.yml).

## Threat model

Keepr is a single-user, offline-first desktop app. It does **not** make outbound network requests, has no account system, and does not collect telemetry. The threats it actively defends against are:

1. **Malicious backup files.** ZIP imports are validated for path traversal, entry count, per-file uncompressed size, and total uncompressed size. A failing restore rolls back to the previous database via a `.prev` snapshot.
2. **Renderer code execution surface.** Tauri's content security policy is locked down to `'self'` origins plus the `keepr-resource://` protocol for attachments; no inline scripts. The `fs:*` capability is not granted to the renderer — all file I/O is in Rust commands.
3. **Schema drift on upgrade.** `PRAGMA user_version` plus a forward-only migration framework prevent partial schema application; databases from a newer Keepr are rejected with a clear message rather than silently corrupted.

The threats it does **not** currently defend against (and where to expect future work):

- **Disk-level read of plain notes.** Plain (non-vaulted) notes remain plaintext SQLite — an attacker with filesystem access can read them by opening `keepr.db` in any SQLite browser. Move sensitive notes into the **Private Vault** (v0.8.0+) to encrypt them at rest with XChaCha20-Poly1305.
- **Adversary with disk write access while Keepr is open.** A second writer to `keepr.db` outside Keepr's mutex is not detected. The single-instance plugin (v0.4.1) prevents the most common case — two `keepr.exe` processes — but doesn't protect against external tools.
- **Local resource exhaustion.** The 2 GiB total + 512 MiB per-file uncompressed caps on both import AND export (v0.5.0+) are sized for normal use, not a malicious operator with admin access.
- **Notification loss on permission denial.** Reminders fail-soft to the next 30 s sweep (v0.4.1 fix). If Windows Notifications are globally disabled, every sweep will fail and the reminder never visibly fires.

### App Lock (v0.7.0+)

App Lock hides every note behind an Argon2id-hashed PIN whenever Keepr launches or stays idle for N minutes. **It is a UI gate, not at-rest encryption** — for at-rest protection, use Private Vault. App Lock defends against:

- casual shoulder-surfing on an unlocked OS session,
- screenshot tools that capture the foreground window,
- the "Keepr in the tray on an unattended machine" case.

It does **not** defend against:

- an attacker with filesystem access (open `keepr.db` in any SQLite browser),
- an attacker who edits `app_settings` and deletes the `app_lock_pin_phc` row (the data is back to plaintext access, no key was ever holding it captive).

**Lost-PIN policy: there is no recovery.** Argon2id is deliberately slow (~150-300 ms per attempt at m=64MiB, t=3, p=1). A forgotten PIN can be cleared by editing the SQLite file directly with any browser; the plain (non-vault) notes are immediately readable. We deliberately do not ship a reset escape-hatch in the UI because that would be exactly the bypass a casual snoop would use.

### Private Vault (v0.8.0+)

Private Vault is **at-rest encryption** for the notes you move into it. Schema and crypto:

- A random 32-byte data-encryption-key (DEK) is generated once at vault init and held only in process memory after unlock.
- The DEK is wrapped with a KEK derived from the vault password via Argon2id (m=64MiB, t=3, p=1) plus a 16-byte random salt; the wrap uses XChaCha20-Poly1305 with a 24-byte random nonce. The salt, nonce, and wrapped DEK are persisted in `app_settings` (hex-encoded).
- Per-note ciphertext is `nonce(24) || aead(ct+tag)` over a JSON payload of `{ title, body, checklist }`. AAD is the note's UUID, so an attacker who swaps `vault_ciphertext` between two rows fails verification.
- Changing the vault password **rewraps the DEK only** — no note has to be re-encrypted, and the rewrap completes in a single Argon2id derive + one AEAD op.
- The `Dek` newtype implements `Drop` + `Zeroize`, so locking the vault (or process exit) wipes the key from memory. App Lock idle fire also calls `lock_vault()` so an attacker grabbing a laptop in App-Lock state can't read the DEK out of RAM.

Threats Private Vault **does** defend against:

- Disk-level read of `keepr.db` — vaulted note title/body/checklist are ciphertext.
- `vault_ciphertext` cross-row swaps — AAD verification rejects them.
- Tampered ciphertext — Poly1305 tag verification rejects single-bit flips.

Threats it does **not** defend against:

- A keylogger or screenshot tool active while the vault is unlocked.
- Cold-boot / memory-dump attacks while the vault is unlocked — the DEK is in RAM and not specifically pinned/locked.
- Attachments are stored unencrypted on disk under `resources/`; only the SQLite-side title/body/checklist payload is encrypted.

**Lost-password policy: there is no recovery.** The DEK can only be recovered by re-deriving the KEK from the original password. Editing `app_settings` to clear `vault_dek_wrapped` permanently destroys access to every vaulted note (the ciphertext on disk becomes opaque garbage). We deliberately do not ship a reset escape-hatch because that would be a back door.

### Surfaces added since v0.4.0

- **System tray icon** with a Quit menu item. Closing the main window via the title-bar X minimizes to tray instead of exiting; Quit is the only intentional process termination.
- **Global hotkey `Ctrl+Alt+N`** for quick-capture. Registration failure now surfaces as an in-app toast.
- **`keepr-resource://` custom protocol** serves attachment blobs from `<data_dir>/resources/<id>.<ext>` and the v0.5 sibling thumbnail at `<id>.thumb.jpg`. Path-safety check rejects `..`, `/`, `\` in the id; CSP allows the scheme as `img-src` only.
- **Reminder scheduler thread** runs every 30 s, queries pending reminders, fires native toasts. Marks `fired_at` only on successful `notification.show()` (v0.4.1 fix) so a failed permission doesn't lose data.
- **Auto-backup tick** runs every 30 min in the renderer; writes ZIPs into a user-picked folder. No background-process surface — only runs while Keepr is open.

## Reporting a vulnerability

If you find a security issue — especially anything around the backup pipeline, schema migration, or the Tauri command surface — please **do not** open a public GitHub issue.

- Email: `matt@mavenimaging.com`
- GitHub: open a private security advisory at https://github.com/SysAdminDoc/Keepr/security/advisories/new
- We'll acknowledge within 5 working days and aim to ship a fix within 30.

## Supported versions

Keepr is pre-1.0; only the latest release receives fixes. We'll backport critical patches to the immediately-previous minor if a database upgrade is risky.

## Security-relevant change history

See [CHANGELOG.md](CHANGELOG.md). v0.2 closes the P0 audit items documented in [RESEARCH_FEATURE_PLAN.md](RESEARCH_FEATURE_PLAN.md): EI-01 (zip-slip), EI-02 (WAL pages lost on export), EI-03 (safe-swap on partial restore), EI-04 (schema migration), EI-05 (CSP + capability tightening).
