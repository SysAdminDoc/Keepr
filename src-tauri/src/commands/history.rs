use super::*;
// --- Note version history (NF-V0.5-D) ---------------------------------------
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteSnapshot {
    pub id: String,
    pub note_id: String,
    pub kind: String,
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub checklist: Vec<ChecklistItem>,
    pub vault: String,
    pub taken_at: String,
}
/// Snapshot the current row state into note_snapshots. Called inside
/// the update_note transaction (and restore_snapshot, before
/// overwriting). Stores ciphertext as-is for vault rows; for plain rows
/// JSON-encodes the checklist so the restore is a single row.
pub(super) fn snapshot_current_note(
    tx: &rusqlite::Transaction,
    id: &str,
    taken_at: &str,
) -> Result<(), String> {
    let snap_id = Uuid::new_v4().to_string();
    let (kind, title, body, color, pinned, vault_state, vault_ciphertext): (
        String,
        String,
        String,
        String,
        i64,
        String,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT kind, title, body, color, pinned, vault, vault_ciphertext \
             FROM notes WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .map_err(err)?;
    let checklist_json: String = if vault_state == "vault" {
        // Vault rows don't have checklist_items rows; ciphertext owns
        // the checklist payload.
        "[]".to_string()
    } else {
        let items: Vec<ChecklistItem> = tx
            .prepare(
                "SELECT id, text, checked, position, parent_id FROM checklist_items \
                 WHERE note_id = ?1 ORDER BY position, id",
            )
            .map_err(err)?
            .query_map(params![id], |r| {
                Ok(ChecklistItem {
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
        serde_json::to_string(&items).map_err(|e| e.to_string())?
    };
    tx.execute(
        "INSERT INTO note_snapshots \
             (id, note_id, kind, title, body, color, pinned, checklist_json, \
              vault, vault_ciphertext, taken_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            snap_id,
            id,
            kind,
            title,
            body,
            color,
            pinned,
            checklist_json,
            vault_state,
            vault_ciphertext,
            taken_at,
        ],
    )
    .map_err(err)?;
    Ok(())
}
#[tauri::command]
pub fn list_snapshots(
    state: State<'_, AppState>,
    note_id: String,
) -> Result<Vec<NoteSnapshot>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, kind, title, body, color, pinned, checklist_json, \
                    vault, taken_at \
             FROM note_snapshots WHERE note_id = ?1 \
             ORDER BY taken_at DESC, id DESC",
        )
        .map_err(err)?;
    let snaps = stmt
        .query_map(params![note_id], |row| {
            let checklist_json: String = row.get(7)?;
            let checklist: Vec<ChecklistItem> =
                serde_json::from_str(&checklist_json).unwrap_or_default();
            Ok(NoteSnapshot {
                id: row.get(0)?,
                note_id: row.get(1)?,
                kind: row.get(2)?,
                title: row.get(3)?,
                body: row.get(4)?,
                color: row.get(5)?,
                pinned: row.get::<_, i64>(6)? != 0,
                checklist,
                vault: row.get(8)?,
                taken_at: row.get(9)?,
            })
        })
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    Ok(snaps)
}
#[tauri::command]
pub fn restore_snapshot(state: State<'_, AppState>, snapshot_id: String) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let dek_guard = state.vault_dek.lock();
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let now = now_iso();
    let (note_id, kind, title, body, color, pinned, checklist_json, snap_vault, snap_ciphertext): (
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        Option<String>,
    ) = tx
        .query_row(
            "SELECT note_id, kind, title, body, color, pinned, checklist_json, \
                    vault, vault_ciphertext \
             FROM note_snapshots WHERE id = ?1",
            params![snapshot_id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                ))
            },
        )
        .map_err(|_| format!("snapshot {snapshot_id} not found"))?;
    // First snapshot the current state so restore itself is undoable.
    snapshot_current_note(&tx, &note_id, &now)?;
    // Wipe per-row children that will be rebuilt.
    tx.execute(
        "DELETE FROM checklist_items WHERE note_id = ?1",
        params![note_id],
    )
    .map_err(err)?;
    tx.execute(
        "UPDATE notes \
            SET kind = ?2, title = ?3, body = ?4, color = ?5, pinned = ?6, \
                vault = ?7, vault_ciphertext = ?8, updated_at = ?9 \
          WHERE id = ?1",
        params![
            note_id,
            kind,
            title,
            body,
            color,
            pinned,
            snap_vault,
            snap_ciphertext,
            now,
        ],
    )
    .map_err(err)?;
    if snap_vault == "plain" {
        let items: Vec<ChecklistItem> = serde_json::from_str(&checklist_json).unwrap_or_default();
        for item in &items {
            tx.execute(
                "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.id,
                    note_id,
                    item.text,
                    item.checked as i64,
                    item.position,
                    item.parent_id
                ],
            )
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)?;
    drop(conn);
    let conn = state.db.lock();
    let note = notes::load_note_with_vault(&conn, &note_id, dek_guard.as_ref())
        .map_err(err)?
        .ok_or_else(|| "note vanished after restore".to_string())?;
    Ok(note)
}
