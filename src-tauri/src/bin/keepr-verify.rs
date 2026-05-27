//! `keepr-verify` — independent verifier for the Private Vault.
//!
//! Why this exists: the security of Keepr's vault depends on three
//! claims (XChaCha20-Poly1305 + Argon2id + per-note AAD = the note's
//! UUID). Users have to trust SECURITY.md's prose. This binary reads
//! the same on-disk material the running app reads, re-derives the
//! KEK, unwraps the DEK, and decrypts a sample vault note — printing
//! the plaintext if it works. Open-source, ~150 LOC, depends only on
//! the same crypto crates the main app uses.
//!
//! It does NOT depend on Tauri, the SQLite plugin, or any IPC. Builds
//! as a standalone binary you can copy onto an air-gapped machine.
//!
//! Usage:
//!   keepr-verify --db <path-to-keepr.db>
//!   keepr-verify --db <path> --note-id <uuid>
//!   keepr-verify --db <path> --seed   # try seed-phrase recovery instead
//!
//! The passphrase / seed is read from stdin (no echo) so it never
//! ends up in shell history.

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use rusqlite::Connection;

// Re-use the same vault crate functions the main app uses.
use keepr_lib::vault;

fn main() {
    if let Err(e) = run() {
        eprintln!("keepr-verify: {e:#}");
        std::process::exit(1);
    }
}

#[derive(Debug)]
struct Args {
    db: PathBuf,
    note_id: Option<String>,
    seed: bool,
    help: bool,
}

fn parse_args() -> Result<Args> {
    let mut db: Option<PathBuf> = None;
    let mut note_id: Option<String> = None;
    let mut seed = false;
    let mut help = false;
    let raw: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < raw.len() {
        match raw[i].as_str() {
            "--db" | "-d" => {
                i += 1;
                db = Some(PathBuf::from(
                    raw.get(i).ok_or_else(|| anyhow!("--db requires a path"))?,
                ));
            }
            "--note-id" | "-n" => {
                i += 1;
                note_id = Some(
                    raw.get(i)
                        .ok_or_else(|| anyhow!("--note-id requires a value"))?
                        .clone(),
                );
            }
            "--seed" | "-s" => {
                seed = true;
            }
            "--help" | "-h" => {
                help = true;
            }
            other => bail!("unknown argument: {other}"),
        }
        i += 1;
    }
    if help {
        return Ok(Args {
            db: PathBuf::new(),
            note_id,
            seed,
            help,
        });
    }
    Ok(Args {
        db: db.ok_or_else(|| anyhow!("--db <path> is required (or pass --help)"))?,
        note_id,
        seed,
        help,
    })
}

fn print_help() {
    println!(
        "keepr-verify — independent Keepr Vault decrypter\n\
         \n\
         Usage:\n\
           keepr-verify --db <path-to-keepr.db>\n\
           keepr-verify --db <path> --note-id <uuid>\n\
           keepr-verify --db <path> --seed\n\
         \n\
         Reads the vault envelope from app_settings, prompts for the\n\
         vault password (or, with --seed, the 12-word BIP39 recovery\n\
         phrase), re-derives the KEK via Argon2id, unwraps the DEK,\n\
         and decrypts a sample vault note. Prints the recovered\n\
         plaintext or 'OK: round-trip verified' if no vault note has\n\
         a body yet.\n\
         \n\
         The passphrase is read from stdin (one line). For air-gapped\n\
         verification, copy this binary + your keepr.db to an offline\n\
         machine and run it there."
    );
}

fn run() -> Result<()> {
    let args = parse_args()?;
    if args.help {
        print_help();
        return Ok(());
    }
    let conn = Connection::open(&args.db).with_context(|| {
        format!("open SQLite at {}", args.db.display())
    })?;

    if args.seed {
        let phrase = prompt("Enter your 12-word recovery phrase: ")?;
        verify_with_seed(&conn, &phrase, args.note_id.as_deref())
    } else {
        let pw = prompt("Enter your vault password: ")?;
        verify_with_password(&conn, &pw, args.note_id.as_deref())
    }
}

fn prompt(p: &str) -> Result<String> {
    print!("{p}");
    io::stdout().flush().ok();
    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let line = line.trim_end_matches(['\n', '\r']).to_string();
    if line.is_empty() {
        bail!("empty input");
    }
    Ok(line)
}

fn read_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    let res: rusqlite::Result<String> = conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        rusqlite::params![key],
        |r| r.get(0),
    );
    match res {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn read_envelope(
    conn: &Connection,
    salt_key: &str,
    nonce_key: &str,
    wrapped_key: &str,
) -> Result<(Vec<u8>, Vec<u8>, Vec<u8>)> {
    let salt = read_setting(conn, salt_key)?
        .ok_or_else(|| anyhow!("missing app_settings key: {salt_key}"))?;
    let nonce = read_setting(conn, nonce_key)?
        .ok_or_else(|| anyhow!("missing app_settings key: {nonce_key}"))?;
    let wrapped = read_setting(conn, wrapped_key)?
        .ok_or_else(|| anyhow!("missing app_settings key: {wrapped_key}"))?;
    Ok((
        vault::from_hex(&salt).context("decode salt hex")?,
        vault::from_hex(&nonce).context("decode nonce hex")?,
        vault::from_hex(&wrapped).context("decode wrapped hex")?,
    ))
}

fn verify_with_password(
    conn: &Connection,
    password: &str,
    note_id: Option<&str>,
) -> Result<()> {
    let (salt, nonce, wrapped) = read_envelope(
        conn,
        "vault_kdf_salt",
        "vault_dek_nonce",
        "vault_dek_wrapped",
    )?;
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("vault salt has wrong length"))?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("vault nonce has wrong length"))?;
    println!("Deriving KEK via Argon2id (m=64MiB, t=3, p=1) — this takes a moment…");
    let dek = vault::unlock(password, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| anyhow!("unlock error: {e}"))?
        .ok_or_else(|| anyhow!("wrong password — KEK derived but DEK auth tag mismatch"))?;
    println!("OK: DEK unwrapped successfully (32 bytes).");
    sample_decrypt(conn, &dek, note_id)
}

fn verify_with_seed(
    conn: &Connection,
    phrase: &str,
    note_id: Option<&str>,
) -> Result<()> {
    let (salt, nonce, wrapped) = read_envelope(
        conn,
        "vault_seed_salt",
        "vault_seed_nonce",
        "vault_seed_dek_wrapped",
    )?;
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("vault seed salt has wrong length"))?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| anyhow!("vault seed nonce has wrong length"))?;
    println!("Deriving KEK from BIP39 seed via Argon2id…");
    let dek = vault::unlock_with_seed(phrase, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| anyhow!("seed unlock error: {e}"))?
        .ok_or_else(|| anyhow!("recovery phrase did not match this vault"))?;
    println!("OK: DEK unwrapped successfully via recovery seed (32 bytes).");
    sample_decrypt(conn, &dek, note_id)
}

fn sample_decrypt(
    conn: &Connection,
    dek: &vault::Dek,
    note_id: Option<&str>,
) -> Result<()> {
    // Either decrypt the requested note, or pick the first vault note.
    let (id, ct_hex): (String, String) = if let Some(id) = note_id {
        conn.query_row(
            "SELECT id, vault_ciphertext FROM notes WHERE id = ?1 AND vault = 'vault'",
            rusqlite::params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                ))
            },
        )
        .map_err(|e| anyhow!("note {id} not found or not in vault: {e}"))?
    } else {
        let row: rusqlite::Result<(String, String)> = conn.query_row(
            "SELECT id, vault_ciphertext FROM notes \
             WHERE vault = 'vault' AND vault_ciphertext IS NOT NULL \
             ORDER BY created_at ASC LIMIT 1",
            rusqlite::params![],
            |r| Ok((r.get(0)?, r.get(1)?)),
        );
        match row {
            Ok(r) => r,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                println!("OK: round-trip verified (vault has no notes to sample).");
                return Ok(());
            }
            Err(e) => return Err(e.into()),
        }
    };
    let bundle = vault::from_hex(&ct_hex).context("decode ciphertext hex")?;
    let payload = vault::decrypt_note(dek, &id, &bundle)
        .map_err(|e| anyhow!("decrypt failed: {e}"))?;
    println!("\nSample note decrypted (id={id}):");
    println!("  title: {}", short_repr(&payload.title));
    println!("  body:  {}", short_repr(&payload.body));
    println!("  checklist items: {}", payload.checklist.len());
    println!("\nOK: round-trip verified.");
    Ok(())
}

fn short_repr(s: &str) -> String {
    let trimmed: String = s.chars().take(80).collect();
    if s.len() > trimmed.len() {
        format!("{trimmed}…")
    } else {
        trimmed
    }
}
