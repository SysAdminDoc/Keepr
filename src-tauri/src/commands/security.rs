use super::*;

// --- App Lock (NF-V0.5-C) ---------------------------------------------------
//
// Stores the Argon2id PHC string in `app_settings.app_lock_pin_phc` and
// the idle timeout in `app_settings.app_lock_after_minutes`. PHC absence
// (or NULL) means the lock is disabled. Hashing is the slow step
// (~150-300 ms) and runs on the Tauri command worker — we deliberately
// don't spawn_blocking because the renderer is already waiting on the
// invoke promise and async-blocking would just add overhead.

pub(super) const KEY_APP_LOCK_PHC: &str = "app_lock_pin_phc";
pub(super) const KEY_APP_LOCK_MINUTES: &str = "app_lock_after_minutes";
pub(super) const DEFAULT_LOCK_MINUTES: u32 = 5;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLockSettings {
    pub enabled: bool,
    pub lock_after_minutes: u32,
}

pub(super) fn read_app_setting(
    conn: &rusqlite::Connection,
    key: &str,
) -> Result<Option<String>, String> {
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get::<_, String>(0),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(err(other)),
    })
}

pub(super) fn write_app_setting(
    conn: &rusqlite::Connection,
    key: &str,
    value: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(err)?;
    Ok(())
}

pub(super) fn delete_app_setting(conn: &rusqlite::Connection, key: &str) -> Result<(), String> {
    conn.execute("DELETE FROM app_settings WHERE key = ?1", params![key])
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn get_app_lock_settings(state: State<'_, AppState>) -> Result<AppLockSettings, String> {
    let conn = state.db.lock();
    let enabled = read_app_setting(&conn, KEY_APP_LOCK_PHC)?
        .map(|s| !s.is_empty())
        .unwrap_or(false);
    let lock_after_minutes = read_app_setting(&conn, KEY_APP_LOCK_MINUTES)?
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(DEFAULT_LOCK_MINUTES);
    Ok(AppLockSettings {
        enabled,
        lock_after_minutes,
    })
}

#[tauri::command]
pub fn enable_app_lock(
    state: State<'_, AppState>,
    pin: String,
    lock_after_minutes: u32,
) -> Result<(), String> {
    if pin.is_empty() {
        return Err("PIN must not be empty".into());
    }
    if !(1..=240).contains(&lock_after_minutes) {
        return Err("lock_after_minutes must be between 1 and 240".into());
    }
    let phc = crate::lock::hash_pin(&pin).map_err(|e| e.to_string())?;
    let conn = state.db.lock();
    write_app_setting(&conn, KEY_APP_LOCK_PHC, &phc)?;
    write_app_setting(&conn, KEY_APP_LOCK_MINUTES, &lock_after_minutes.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn disable_app_lock(state: State<'_, AppState>, current_pin: String) -> Result<(), String> {
    let conn = state.db.lock();
    let phc = read_app_setting(&conn, KEY_APP_LOCK_PHC)?;
    let Some(phc) = phc.filter(|s| !s.is_empty()) else {
        return Err("App Lock is not enabled".into());
    };
    let ok = crate::lock::verify_pin(&current_pin, &phc).map_err(|e| e.to_string())?;
    if !ok {
        return Err("Incorrect PIN".into());
    }
    delete_app_setting(&conn, KEY_APP_LOCK_PHC)?;
    Ok(())
}

#[tauri::command]
pub fn verify_app_lock_pin(state: State<'_, AppState>, pin: String) -> Result<bool, String> {
    let conn = state.db.lock();
    let phc = read_app_setting(&conn, KEY_APP_LOCK_PHC)?;
    let Some(phc) = phc.filter(|s| !s.is_empty()) else {
        return Err("App Lock is not enabled".into());
    };
    crate::lock::verify_pin(&pin, &phc).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_app_lock_minutes(
    state: State<'_, AppState>,
    lock_after_minutes: u32,
) -> Result<(), String> {
    if !(1..=240).contains(&lock_after_minutes) {
        return Err("lock_after_minutes must be between 1 and 240".into());
    }
    let conn = state.db.lock();
    write_app_setting(&conn, KEY_APP_LOCK_MINUTES, &lock_after_minutes.to_string())?;
    Ok(())
}

// --- Private Vault (NF-V0.5-C / 2 of 2) -------------------------------------
//
// Stores three hex-encoded values in app_settings:
//   vault_kdf_salt      — 16 bytes (Argon2id salt)
//   vault_dek_nonce     — 24 bytes (XChaCha20 nonce used to wrap the DEK)
//   vault_dek_wrapped   — wrapped DEK bundle (XChaCha20-Poly1305 ct + tag)
// The unlocked DEK lives in `AppState.vault_dek` and is wiped on
// `lock_vault` or app exit (Drop on Dek zeroizes).

pub(super) const KEY_VAULT_SALT: &str = "vault_kdf_salt";
pub(super) const KEY_VAULT_NONCE: &str = "vault_dek_nonce";
pub(super) const KEY_VAULT_WRAPPED: &str = "vault_dek_wrapped";
// v0.21.1 — opt-in BIP39 recovery seed envelope. Wraps the SAME DEK,
// just derived from the seed entropy instead of a user password.
pub(super) const KEY_VAULT_SEED_SALT: &str = "vault_seed_salt";
pub(super) const KEY_VAULT_SEED_NONCE: &str = "vault_seed_nonce";
pub(super) const KEY_VAULT_SEED_WRAPPED: &str = "vault_seed_dek_wrapped";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
}

pub(super) fn read_vault_material(
    conn: &rusqlite::Connection,
) -> Result<Option<(Vec<u8>, Vec<u8>, Vec<u8>)>, String> {
    let salt = read_app_setting(conn, KEY_VAULT_SALT)?;
    let nonce = read_app_setting(conn, KEY_VAULT_NONCE)?;
    let wrapped = read_app_setting(conn, KEY_VAULT_WRAPPED)?;
    match (salt, nonce, wrapped) {
        (Some(s), Some(n), Some(w)) => {
            let s = crate::vault::from_hex(&s).map_err(|e| e.to_string())?;
            let n = crate::vault::from_hex(&n).map_err(|e| e.to_string())?;
            let w = crate::vault::from_hex(&w).map_err(|e| e.to_string())?;
            Ok(Some((s, n, w)))
        }
        _ => Ok(None),
    }
}

/// v0.21.1 — opt-in seed envelope. Same shape as the password envelope.
pub(super) fn read_vault_seed_material(
    conn: &rusqlite::Connection,
) -> Result<Option<(Vec<u8>, Vec<u8>, Vec<u8>)>, String> {
    let salt = read_app_setting(conn, KEY_VAULT_SEED_SALT)?;
    let nonce = read_app_setting(conn, KEY_VAULT_SEED_NONCE)?;
    let wrapped = read_app_setting(conn, KEY_VAULT_SEED_WRAPPED)?;
    match (salt, nonce, wrapped) {
        (Some(s), Some(n), Some(w)) => {
            let s = crate::vault::from_hex(&s).map_err(|e| e.to_string())?;
            let n = crate::vault::from_hex(&n).map_err(|e| e.to_string())?;
            let w = crate::vault::from_hex(&w).map_err(|e| e.to_string())?;
            Ok(Some((s, n, w)))
        }
        _ => Ok(None),
    }
}

pub(super) fn require_unlocked_dek<'a>(
    guard: &'a parking_lot::MutexGuard<'_, Option<crate::vault::Dek>>,
) -> Result<&'a crate::vault::Dek, String> {
    guard.as_ref().ok_or_else(|| "vault is locked".to_string())
}

#[tauri::command]
pub fn get_vault_status(state: State<'_, AppState>) -> Result<VaultStatus, String> {
    let conn = state.db.lock();
    let initialized = read_vault_material(&conn)?.is_some();
    let unlocked = state.vault_dek.lock().is_some();
    Ok(VaultStatus {
        initialized,
        unlocked,
    })
}

#[tauri::command]
pub fn init_vault(state: State<'_, AppState>, password: String) -> Result<(), String> {
    let conn = state.db.lock();
    if read_vault_material(&conn)?.is_some() {
        return Err("vault already initialized".into());
    }
    let (init_data, dek) = crate::vault::init(&password).map_err(|e| e.to_string())?;
    write_app_setting(
        &conn,
        KEY_VAULT_SALT,
        &crate::vault::to_hex(&init_data.salt),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_NONCE,
        &crate::vault::to_hex(&init_data.dek_nonce),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_WRAPPED,
        &crate::vault::to_hex(&init_data.dek_wrapped),
    )?;
    drop(conn);
    *state.vault_dek.lock() = Some(dek);
    Ok(())
}

#[tauri::command]
pub fn unlock_vault(state: State<'_, AppState>, password: String) -> Result<bool, String> {
    let conn = state.db.lock();
    let Some((salt, nonce, wrapped)) = read_vault_material(&conn)? else {
        return Err("vault is not initialized".into());
    };
    drop(conn);
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| "vault salt has wrong length".to_string())?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| "vault dek nonce has wrong length".to_string())?;
    let dek_opt = crate::vault::unlock(&password, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| e.to_string())?;
    match dek_opt {
        Some(dek) => {
            *state.vault_dek.lock() = Some(dek);
            Ok(true)
        }
        None => Ok(false),
    }
}

#[tauri::command]
pub fn lock_vault(state: State<'_, AppState>) -> Result<(), String> {
    let mut guard = state.vault_dek.lock();
    *guard = None; // Dek::drop zeroizes
    Ok(())
}

#[tauri::command]
pub fn change_vault_password(
    state: State<'_, AppState>,
    current_password: String,
    new_password: String,
) -> Result<(), String> {
    if new_password.is_empty() {
        return Err("new vault password must not be empty".into());
    }
    // Re-derive KEK from current password to confirm; on success rewrap.
    let conn = state.db.lock();
    let Some((salt, nonce, wrapped)) = read_vault_material(&conn)? else {
        return Err("vault is not initialized".into());
    };
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| "vault salt has wrong length".to_string())?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| "vault dek nonce has wrong length".to_string())?;
    let dek_opt = crate::vault::unlock(&current_password, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| e.to_string())?;
    let dek = dek_opt.ok_or_else(|| "incorrect current vault password".to_string())?;
    let rewrapped = crate::vault::rewrap(&dek, &new_password).map_err(|e| e.to_string())?;
    write_app_setting(
        &conn,
        KEY_VAULT_SALT,
        &crate::vault::to_hex(&rewrapped.salt),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_NONCE,
        &crate::vault::to_hex(&rewrapped.dek_nonce),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_WRAPPED,
        &crate::vault::to_hex(&rewrapped.dek_wrapped),
    )?;
    // Keep vault unlocked with the same DEK so the user doesn't have to
    // re-enter the new password right after changing it.
    *state.vault_dek.lock() = Some(dek);
    Ok(())
}

// --- v0.21.1 vault recovery seed (opt-in BIP39) ----------------------------

/// Returns true if the vault has an opt-in BIP39 recovery seed envelope
/// stored. The password envelope is independent and remains the primary
/// unlock path even when a seed exists.
#[tauri::command]
pub fn vault_has_recovery_seed(state: State<'_, AppState>) -> Result<bool, String> {
    let conn = state.db.lock();
    Ok(read_vault_seed_material(&conn)?.is_some())
}

/// Generate a fresh BIP39 12-word recovery seed for the vault. The
/// caller must supply the current password so we can unlock the DEK
/// first; we wrap the same DEK with a seed-derived KEK and persist the
/// envelope. Returns the 12-word phrase exactly ONCE — Keepr never
/// stores it in plaintext and there's no way to retrieve it again.
///
/// Opt-in: this is the trade-off between "no recovery possible ever"
/// (the original Vault promise) and "recoverable if the user writes
/// down the seed". The UI MUST make this choice explicit.
#[tauri::command]
pub fn setup_vault_recovery_seed(
    state: State<'_, AppState>,
    current_password: String,
) -> Result<String, String> {
    let conn = state.db.lock();
    if read_vault_seed_material(&conn)?.is_some() {
        return Err("vault already has a recovery seed".into());
    }
    let Some((salt, nonce, wrapped)) = read_vault_material(&conn)? else {
        return Err("vault is not initialized".into());
    };
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| "vault salt has wrong length".to_string())?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| "vault dek nonce has wrong length".to_string())?;
    let dek_opt = crate::vault::unlock(&current_password, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| e.to_string())?;
    let dek = dek_opt.ok_or_else(|| "incorrect current vault password".to_string())?;
    let (phrase, envelope) = crate::vault::seed_init(&dek).map_err(|e| e.to_string())?;
    write_app_setting(
        &conn,
        KEY_VAULT_SEED_SALT,
        &crate::vault::to_hex(&envelope.salt),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_SEED_NONCE,
        &crate::vault::to_hex(&envelope.dek_nonce),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_SEED_WRAPPED,
        &crate::vault::to_hex(&envelope.dek_wrapped),
    )?;
    drop(conn);
    // Keep the vault unlocked.
    *state.vault_dek.lock() = Some(dek);
    Ok(phrase)
}

/// Remove the seed envelope. Used when the user wants to take back the
/// recovery-possible trade-off (e.g. they couldn't store the phrase
/// safely after all). The password envelope is untouched.
#[tauri::command]
pub fn remove_vault_recovery_seed(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    write_app_setting(&conn, KEY_VAULT_SEED_SALT, "")?;
    write_app_setting(&conn, KEY_VAULT_SEED_NONCE, "")?;
    write_app_setting(&conn, KEY_VAULT_SEED_WRAPPED, "")?;
    // Empty strings count as "no material" in read_vault_seed_material
    // via from_hex returning an empty Vec, which fails the salt-length
    // try_into in unlock — better to just delete the rows. Use DELETE
    // here directly so the column is truly absent.
    conn.execute(
        "DELETE FROM app_settings WHERE key IN (?1, ?2, ?3)",
        params![
            KEY_VAULT_SEED_SALT,
            KEY_VAULT_SEED_NONCE,
            KEY_VAULT_SEED_WRAPPED
        ],
    )
    .map_err(err)?;
    Ok(())
}

/// Recover access to the vault using the recovery seed. Unlocks the
/// DEK with the seed-derived KEK, then re-wraps the (unchanged) DEK
/// with the supplied new password's KEK so the user has a working
/// password again. Existing vault notes don't need to be re-encrypted
/// since the DEK is unchanged. Returns Ok on success; the vault is
/// left UNLOCKED in memory so the user can immediately access notes.
#[tauri::command]
pub fn recover_vault_with_seed(
    state: State<'_, AppState>,
    mnemonic: String,
    new_password: String,
) -> Result<(), String> {
    if new_password.is_empty() {
        return Err("new vault password must not be empty".into());
    }
    let conn = state.db.lock();
    let Some((salt, nonce, wrapped)) = read_vault_seed_material(&conn)? else {
        return Err("vault has no recovery seed; recovery is not possible".into());
    };
    let salt_arr: [u8; 16] = salt
        .as_slice()
        .try_into()
        .map_err(|_| "vault seed salt has wrong length".to_string())?;
    let nonce_arr: [u8; 24] = nonce
        .as_slice()
        .try_into()
        .map_err(|_| "vault seed nonce has wrong length".to_string())?;
    let dek_opt = crate::vault::unlock_with_seed(&mnemonic, &salt_arr, &nonce_arr, &wrapped)
        .map_err(|e| e.to_string())?;
    let dek = dek_opt.ok_or_else(|| "recovery phrase did not match this vault".to_string())?;
    let rewrapped = crate::vault::rewrap(&dek, &new_password).map_err(|e| e.to_string())?;
    write_app_setting(
        &conn,
        KEY_VAULT_SALT,
        &crate::vault::to_hex(&rewrapped.salt),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_NONCE,
        &crate::vault::to_hex(&rewrapped.dek_nonce),
    )?;
    write_app_setting(
        &conn,
        KEY_VAULT_WRAPPED,
        &crate::vault::to_hex(&rewrapped.dek_wrapped),
    )?;
    drop(conn);
    *state.vault_dek.lock() = Some(dek);
    Ok(())
}

#[tauri::command]
pub fn move_note_to_vault(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let dek_guard = state.vault_dek.lock();
    let dek = require_unlocked_dek(&dek_guard)?;
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let (title, body, vault_state): (String, String, String) = tx
        .query_row(
            "SELECT title, body, vault FROM notes WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(err)?;
    if vault_state == "vault" {
        return Err("note is already in the vault".into());
    }
    let checklist: Vec<crate::vault::VaultChecklistItem> = tx
        .prepare(
            "SELECT id, text, checked, position, parent_id FROM checklist_items \
             WHERE note_id = ?1 ORDER BY position, id",
        )
        .map_err(err)?
        .query_map(params![id], |r| {
            Ok(crate::vault::VaultChecklistItem {
                id: r.get(0)?,
                text: r.get(1)?,
                checked: r.get::<_, i64>(2)? != 0,
                position: r.get(3)?,
                parent_id: r.get(4)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let payload = crate::vault::VaultPayload {
        title,
        body,
        checklist,
    };
    let bundle = crate::vault::encrypt_note(dek, &id, &payload).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE notes SET title = '', body = '', vault = 'vault', \
             vault_ciphertext = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, crate::vault::to_hex(&bundle), now],
    )
    .map_err(err)?;
    tx.execute(
        "DELETE FROM checklist_items WHERE note_id = ?1",
        params![id],
    )
    .map_err(err)?;
    tx.commit().map_err(err)?;
    drop(dek_guard);
    drop(conn);
    notes::get_note(state.clone(), id)
        .and_then(|opt| opt.ok_or_else(|| "note vanished after vaulting".into()))
}

#[tauri::command]
pub fn move_note_out_of_vault(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let dek_guard = state.vault_dek.lock();
    let dek = require_unlocked_dek(&dek_guard)?;
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let (vault_state, ciphertext_hex): (String, Option<String>) = tx
        .query_row(
            "SELECT vault, vault_ciphertext FROM notes WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(err)?;
    if vault_state != "vault" {
        return Err("note is not in the vault".into());
    }
    let bundle_hex = ciphertext_hex.ok_or_else(|| "vault note missing ciphertext".to_string())?;
    let bundle = crate::vault::from_hex(&bundle_hex).map_err(|e| e.to_string())?;
    let payload = crate::vault::decrypt_note(dek, &id, &bundle).map_err(|e| e.to_string())?;
    let now = chrono::Utc::now().to_rfc3339();
    tx.execute(
        "UPDATE notes SET title = ?2, body = ?3, vault = 'plain', \
             vault_ciphertext = NULL, updated_at = ?4 WHERE id = ?1",
        params![id, payload.title, payload.body, now],
    )
    .map_err(err)?;
    // Restore checklist items.
    for item in &payload.checklist {
        tx.execute(
            "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item.id,
                id,
                item.text,
                if item.checked { 1 } else { 0 },
                item.position,
                item.parent_id,
            ],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    drop(dek_guard);
    drop(conn);
    notes::get_note(state.clone(), id)
        .and_then(|opt| opt.ok_or_else(|| "note vanished after unvaulting".into()))
}

#[tauri::command]
pub fn move_notes_to_vault(state: State<'_, AppState>, ids: Vec<String>) -> Result<u32, String> {
    let mut moved: u32 = 0;
    for id in ids {
        move_note_to_vault(state.clone(), id)?;
        moved += 1;
    }
    Ok(moved)
}

/// Bulk-unvault wrapper. Mirrors `move_notes_to_vault` semantics. v0.20.2.
#[tauri::command]
pub fn move_notes_out_of_vault(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, String> {
    let mut moved: u32 = 0;
    for id in ids {
        move_note_out_of_vault(state.clone(), id)?;
        moved += 1;
    }
    Ok(moved)
}
