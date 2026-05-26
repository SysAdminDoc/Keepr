//! NF-V0.5-C App Lock — Argon2id PHC hashing for the unlock PIN/password.
//!
//! Threat model: this gates the UI only. The SQLite file on disk is not
//! encrypted, so an attacker with filesystem access can still read every
//! note. See SECURITY.md for the full scope. App Lock defends against:
//!   - casual shoulder-surfing on an unlocked OS session,
//!   - apps that screenshot the foreground window,
//!   - the "Keepr in the tray, attacker doesn't know to dig" case.
//!
//! Lost-PIN policy: there is no recovery. Per the v0.5 research doc
//! (RESEARCH_FEATURE_PLAN_v0.5.md §NF-V0.5-C), users who lose the PIN
//! must clear the `app_lock_pin_phc` row in `app_settings` by editing
//! the SQLite file directly — at which point the data is back to v0.6
//! plaintext (no key was ever holding it captive).

use anyhow::{Context, Result};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use password_hash::rand_core::OsRng;

/// Argon2id parameters. 64 MiB memory, 3 iterations, 1 lane is the
/// OWASP-recommended sweet spot for interactive logins on commodity
/// hardware. Verification takes ~150-300 ms on a 2020s laptop, which is
/// short enough to feel responsive but long enough to make a brute
/// force expensive.
fn params() -> Params {
    Params::new(
        64 * 1024, // m_cost: 64 MiB
        3,         // t_cost: 3 iterations
        1,         // p_cost: 1 lane
        None,
    )
    .expect("argon2 params constants are valid")
}

/// Hash a PIN/password and return its PHC-format string. PHC encodes
/// the algorithm, version, params, salt, and hash, so we only need one
/// column to persist everything `verify_pin` needs later.
pub fn hash_pin(pin: &str) -> Result<String> {
    if pin.is_empty() {
        anyhow::bail!("pin must not be empty");
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params());
    let phc = argon2
        .hash_password(pin.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("argon2 hash failed: {e}"))?
        .to_string();
    Ok(phc)
}

/// Verify a candidate PIN against a stored PHC string. Returns true on
/// match, false otherwise; only returns an error if the stored PHC is
/// malformed (which would indicate disk corruption, not a wrong PIN).
pub fn verify_pin(pin: &str, phc: &str) -> Result<bool> {
    let parsed = PasswordHash::new(phc).context("stored PHC string is malformed")?;
    Ok(Argon2::default()
        .verify_password(pin.as_bytes(), &parsed)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_then_verify_roundtrips() {
        let phc = hash_pin("hunter2").unwrap();
        assert!(verify_pin("hunter2", &phc).unwrap());
    }

    #[test]
    fn wrong_pin_does_not_verify() {
        let phc = hash_pin("correct horse").unwrap();
        assert!(!verify_pin("battery staple", &phc).unwrap());
    }

    #[test]
    fn empty_pin_rejected() {
        assert!(hash_pin("").is_err());
    }

    #[test]
    fn phc_is_argon2id_variant() {
        // PHC strings start with "$argon2id$" for the Argon2id variant
        // — protects against an accidental switch to argon2i or argon2d,
        // which have different security properties for this workload.
        let phc = hash_pin("x").unwrap();
        assert!(phc.starts_with("$argon2id$"), "got: {phc}");
    }

    #[test]
    fn malformed_phc_errors_not_just_false() {
        // A real wrong-PIN call returns Ok(false); a corrupted-disk call
        // returns Err so the UI can distinguish the two.
        let err = verify_pin("anything", "not-a-phc-string").unwrap_err();
        assert!(err.to_string().contains("PHC"));
    }
}
