//! NF-V0.5-C Private Vault — per-note authenticated encryption.
//!
//! Two-key envelope:
//!   - KEK (key-encryption-key): derived from the vault password via
//!     Argon2id (m=64MiB, t=3, p=1, 32-byte output).
//!   - DEK (data-encryption-key): 32 random bytes, generated once at
//!     vault init and wrapped with the KEK using XChaCha20-Poly1305.
//!     Stored in `app_settings.vault_dek_wrapped` plus a 24-byte
//!     `vault_dek_nonce` plus the Argon2id `vault_kdf_salt`.
//!
//! Changing the password only re-wraps the DEK — no note has to be
//! re-encrypted. Losing the password is unrecoverable; an attacker who
//! steals the file cannot recover the DEK without it.
//!
//! Per-note payload format (the bytes stored in `notes.vault_ciphertext`):
//!   - 24-byte XChaCha20 nonce, prefix
//!   - ciphertext + 16-byte Poly1305 tag (AEAD), suffix
//! Plaintext is a JSON object so the payload is opaque even to a future
//! Keepr that adds new fields. AAD = the note's UUID, so swapping
//! ciphertext between rows fails verification.

use anyhow::{anyhow, bail, Context, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

const KEK_LEN: usize = 32;
const DEK_LEN: usize = 32;
const NONCE_LEN: usize = 24;
const SALT_LEN: usize = 16;

/// Plaintext payload that lives inside `vault_ciphertext`. Whatever the
/// note contains today; new fields appended later still decrypt for
/// older payloads because serde fills missing fields with defaults.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct VaultPayload {
    pub title: String,
    pub body: String,
    pub checklist: Vec<VaultChecklistItem>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct VaultChecklistItem {
    pub id: String,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    /// NF-V0.5-21 (v0.14+): nested sub-item parent reference. Old
    /// ciphertext written before v0.14 doesn't have this field; serde
    /// `default` fills it with None on read.
    #[serde(default)]
    pub parent_id: Option<String>,
}

/// In-memory holder for the unlocked DEK. Zeroizes on drop. The
/// `AppState` keeps an `Option<Dek>` behind a mutex; `lock_vault`
/// replaces it with `None`.
pub struct Dek([u8; DEK_LEN]);

impl Drop for Dek {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl Dek {
    fn from_slice(s: &[u8]) -> Result<Self> {
        if s.len() != DEK_LEN {
            bail!("DEK has wrong length: expected {DEK_LEN}, got {}", s.len());
        }
        let mut k = [0u8; DEK_LEN];
        k.copy_from_slice(s);
        Ok(Dek(k))
    }
    fn as_bytes(&self) -> &[u8; DEK_LEN] {
        &self.0
    }
}

fn kdf_params() -> Params {
    Params::new(64 * 1024, 3, 1, Some(KEK_LEN))
        .expect("argon2 KDF params are valid")
}

fn derive_kek(password: &str, salt: &[u8]) -> Result<[u8; KEK_LEN]> {
    let mut out = [0u8; KEK_LEN];
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, kdf_params());
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut out)
        .map_err(|e| anyhow!("argon2 derive failed: {e}"))?;
    Ok(out)
}

fn fill_random(buf: &mut [u8]) -> Result<()> {
    getrandom::getrandom(buf).context("OS RNG failed")?;
    Ok(())
}

/// Bytes captured at vault init. The caller persists these into
/// `app_settings` under three keys (hex-encoded). `unlock` reads them
/// back and reverses the wrap.
pub struct VaultInit {
    pub salt: [u8; SALT_LEN],
    pub dek_nonce: [u8; NONCE_LEN],
    pub dek_wrapped: Vec<u8>,
}

/// Build a fresh vault: random salt + random DEK + wrap.
pub fn init(password: &str) -> Result<(VaultInit, Dek)> {
    if password.is_empty() {
        bail!("vault password must not be empty");
    }
    let mut salt = [0u8; SALT_LEN];
    fill_random(&mut salt)?;
    let mut dek_bytes = [0u8; DEK_LEN];
    fill_random(&mut dek_bytes)?;
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;

    let kek = derive_kek(password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&kek).into());
    let wrapped = cipher
        .encrypt(XNonce::from_slice(&nonce), dek_bytes.as_ref())
        .map_err(|e| anyhow!("DEK wrap failed: {e}"))?;
    let dek = Dek::from_slice(&dek_bytes)?;
    // Best-effort scrub of the local copy; Dek itself is now the owner.
    dek_bytes.zeroize();
    drop_kek(kek);

    Ok((
        VaultInit {
            salt,
            dek_nonce: nonce,
            dek_wrapped: wrapped,
        },
        dek,
    ))
}

/// Try to unwrap the stored DEK with the supplied password. Returns
/// Ok(Some(dek)) on success, Ok(None) on wrong password, Err on
/// malformed inputs.
pub fn unlock(
    password: &str,
    salt: &[u8; SALT_LEN],
    dek_nonce: &[u8; NONCE_LEN],
    dek_wrapped: &[u8],
) -> Result<Option<Dek>> {
    let kek = derive_kek(password, salt)?;
    let cipher = XChaCha20Poly1305::new((&kek).into());
    let result = cipher.decrypt(XNonce::from_slice(dek_nonce), dek_wrapped);
    drop_kek(kek);
    match result {
        Ok(bytes) => {
            let dek = Dek::from_slice(&bytes)?;
            // Caller now owns the DEK; clear the working buffer.
            let mut leftover = bytes;
            leftover.zeroize();
            Ok(Some(dek))
        }
        Err(_) => Ok(None),
    }
}

/// Re-wrap the unlocked DEK with a new password's KEK. Returns the
/// new on-disk material; the DEK itself is unchanged so no note has
/// to be re-encrypted.
pub fn rewrap(dek: &Dek, new_password: &str) -> Result<VaultInit> {
    if new_password.is_empty() {
        bail!("new vault password must not be empty");
    }
    let mut salt = [0u8; SALT_LEN];
    fill_random(&mut salt)?;
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let kek = derive_kek(new_password, &salt)?;
    let cipher = XChaCha20Poly1305::new((&kek).into());
    let wrapped = cipher
        .encrypt(XNonce::from_slice(&nonce), dek.as_bytes().as_ref())
        .map_err(|e| anyhow!("DEK rewrap failed: {e}"))?;
    drop_kek(kek);
    Ok(VaultInit {
        salt,
        dek_nonce: nonce,
        dek_wrapped: wrapped,
    })
}

/// Encrypt a note's payload under the unlocked DEK. Output bundle is
/// `nonce(24) || ciphertext+tag`. AAD = the note's UUID, so a swap
/// between two rows fails verification.
pub fn encrypt_note(dek: &Dek, note_id: &str, payload: &VaultPayload) -> Result<Vec<u8>> {
    let plaintext = serde_json::to_vec(payload).context("payload serialize")?;
    let cipher = XChaCha20Poly1305::new(dek.as_bytes().into());
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let ct = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: note_id.as_bytes(),
            },
        )
        .map_err(|e| anyhow!("encrypt failed: {e}"))?;
    let mut out = Vec::with_capacity(NONCE_LEN + ct.len());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Reverse of `encrypt_note`. Verifies the Poly1305 tag and the AAD
/// (note id); returns the recovered VaultPayload.
pub fn decrypt_note(dek: &Dek, note_id: &str, bundle: &[u8]) -> Result<VaultPayload> {
    if bundle.len() < NONCE_LEN + 16 {
        bail!("vault bundle too short: {}", bundle.len());
    }
    let (nonce_bytes, ct) = bundle.split_at(NONCE_LEN);
    let cipher = XChaCha20Poly1305::new(dek.as_bytes().into());
    let pt = cipher
        .decrypt(
            XNonce::from_slice(nonce_bytes),
            Payload {
                msg: ct,
                aad: note_id.as_bytes(),
            },
        )
        .map_err(|e| anyhow!("decrypt failed: {e}"))?;
    let payload: VaultPayload = serde_json::from_slice(&pt).context("payload parse")?;
    Ok(payload)
}

/// Hex-encode bytes for storage in TEXT columns. The DEK never goes
/// through this path; only the wrap blob + salt + nonces do.
pub fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn from_hex(s: &str) -> Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        bail!("hex string has odd length");
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(b: u8) -> Result<u8> {
    match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => bail!("invalid hex char: {b}"),
    }
}

fn drop_kek(mut k: [u8; KEK_LEN]) {
    k.zeroize();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_then_unlock_with_same_password_roundtrips() {
        let (init_data, dek) = init("hunter2").unwrap();
        let unlocked = unlock(
            "hunter2",
            &init_data.salt,
            &init_data.dek_nonce,
            &init_data.dek_wrapped,
        )
        .unwrap()
        .expect("correct password must unlock");
        assert_eq!(dek.as_bytes(), unlocked.as_bytes());
    }

    #[test]
    fn unlock_with_wrong_password_returns_none() {
        let (init_data, _) = init("correct").unwrap();
        let r = unlock(
            "wrong",
            &init_data.salt,
            &init_data.dek_nonce,
            &init_data.dek_wrapped,
        )
        .unwrap();
        assert!(r.is_none());
    }

    #[test]
    fn empty_password_rejected_at_init() {
        assert!(init("").is_err());
    }

    #[test]
    fn rewrap_preserves_dek_so_notes_dont_need_reencrypt() {
        let (init_data, dek) = init("old").unwrap();
        let rewrapped = rewrap(&dek, "new").unwrap();
        let unlocked = unlock(
            "new",
            &rewrapped.salt,
            &rewrapped.dek_nonce,
            &rewrapped.dek_wrapped,
        )
        .unwrap()
        .expect("rewrap output must unlock with the new password");
        assert_eq!(dek.as_bytes(), unlocked.as_bytes());
    }

    #[test]
    fn encrypt_decrypt_note_roundtrips() {
        let (_, dek) = init("pw").unwrap();
        let payload = VaultPayload {
            title: "Secret".into(),
            body: "Body text".into(),
            checklist: vec![VaultChecklistItem {
                id: "c1".into(),
                text: "milk".into(),
                checked: false,
                position: 0,
                parent_id: None,
            }],
        };
        let bundle = encrypt_note(&dek, "note-id-123", &payload).unwrap();
        let back = decrypt_note(&dek, "note-id-123", &bundle).unwrap();
        assert_eq!(back.title, "Secret");
        assert_eq!(back.body, "Body text");
        assert_eq!(back.checklist.len(), 1);
        assert_eq!(back.checklist[0].text, "milk");
    }

    #[test]
    fn decrypt_with_wrong_note_id_fails_verification() {
        let (_, dek) = init("pw").unwrap();
        let payload = VaultPayload::default();
        let bundle = encrypt_note(&dek, "id-A", &payload).unwrap();
        let err = decrypt_note(&dek, "id-B", &bundle).unwrap_err();
        assert!(err.to_string().contains("decrypt failed"));
    }

    #[test]
    fn tampered_ciphertext_fails_verification() {
        let (_, dek) = init("pw").unwrap();
        let mut bundle = encrypt_note(&dek, "id-X", &VaultPayload::default()).unwrap();
        let last = bundle.len() - 1;
        bundle[last] ^= 0xff;
        let err = decrypt_note(&dek, "id-X", &bundle).unwrap_err();
        assert!(err.to_string().contains("decrypt failed"));
    }

    #[test]
    fn hex_roundtrip() {
        let raw = vec![0u8, 1, 0xAB, 0xCD, 0xFE, 0xFF];
        let hex = to_hex(&raw);
        assert_eq!(hex, "0001abcdfeff");
        assert_eq!(from_hex(&hex).unwrap(), raw);
    }

    #[test]
    fn hex_rejects_odd_length_and_bad_chars() {
        assert!(from_hex("abc").is_err());
        assert!(from_hex("zz").is_err());
    }
}
