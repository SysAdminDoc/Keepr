use super::*;

const MAX_TITLE_CHARS: usize = 1024;
const MAX_BODY_BYTES: usize = 64 * 1024; // 64 KiB
const MAX_CHECKLIST_ITEMS: usize = 1000;
const MAX_CHECKLIST_ITEM_CHARS: usize = 2048;

pub(super) fn validate_note_input(input: &NoteInput) -> Result<(), String> {
    if input.kind != "text" && input.kind != "list" {
        return Err(format!("unknown note kind '{}'", input.kind));
    }
    if input.title.chars().count() > MAX_TITLE_CHARS {
        return Err(format!("title exceeds {MAX_TITLE_CHARS} characters"));
    }
    if input.body.len() > MAX_BODY_BYTES {
        return Err(format!("body exceeds {MAX_BODY_BYTES} bytes"));
    }
    if input.checklist.len() > MAX_CHECKLIST_ITEMS {
        return Err(format!(
            "checklist has {} items (max {})",
            input.checklist.len(),
            MAX_CHECKLIST_ITEMS
        ));
    }
    for (i, item) in input.checklist.iter().enumerate() {
        if item.text.chars().count() > MAX_CHECKLIST_ITEM_CHARS {
            return Err(format!(
                "checklist item {i} exceeds {MAX_CHECKLIST_ITEM_CHARS} characters"
            ));
        }
    }
    // NF-22 — background_pattern is a whitelisted enum-like string.
    // Renderer keeps the full visual list; Rust just enforces the gate.
    if !is_valid_background_pattern(&input.background_pattern) {
        return Err(format!(
            "unknown background_pattern '{}'",
            input.background_pattern
        ));
    }
    // NF-21 — validate parent_id references. Collect the set of items
    // that are themselves top-level (parent_id = None) and confirm any
    // child's parent_id points at one of those (NOT at another child —
    // one nesting level only). Items in the same batch reference each
    // other by `id`, which may be either the renderer-supplied id or
    // None (in which case the item can't be a parent — it has no
    // stable identifier yet).
    let top_level_ids: std::collections::HashSet<&str> = input
        .checklist
        .iter()
        .filter(|it| it.parent_id.is_none())
        .filter_map(|it| it.id.as_deref())
        .collect();
    for (i, item) in input.checklist.iter().enumerate() {
        if let Some(p) = &item.parent_id {
            if !top_level_ids.contains(p.as_str()) {
                return Err(format!(
                    "checklist item {i}: parent_id '{p}' must reference a top-level item in the same batch"
                ));
            }
        }
    }
    Ok(())
}

pub(super) const ALLOWED_BACKGROUND_PATTERNS: &[&str] = &[
    "",
    "groceries",
    "food",
    "music",
    "recipes",
    "notes",
    "places",
    "travel",
    "video",
    "celebration",
];

pub(super) fn is_valid_background_pattern(s: &str) -> bool {
    ALLOWED_BACKGROUND_PATTERNS.contains(&s)
}

pub(super) fn load_note(conn: &Connection, id: &str) -> Result<Option<Note>, rusqlite::Error> {
    load_note_with_vault(conn, id, None)
}

/// NF-V0.5-C — `load_note` variant that decrypts a vault note in place
/// if the caller passes the unlocked DEK. Without the DEK, vault notes
/// come back with empty title/body/checklist + `vault = "vault"` so the
/// renderer can show the lock placeholder.
pub(super) fn load_note_with_vault(
    conn: &Connection,
    id: &str,
    dek: Option<&crate::vault::Dek>,
) -> Result<Option<Note>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, title, body, color, pinned, archived, trashed, position,
                created_at, updated_at, trashed_at, vault, vault_ciphertext,
                background_pattern
         FROM notes WHERE id = ?1",
    )?;
    let row_opt = stmt
        .query_row(params![id], |row| {
            let id: String = row.get(0)?;
            let kind: String = row.get(1)?;
            let mut title: String = row.get(2)?;
            let mut body: String = row.get(3)?;
            let color: String = row.get(4)?;
            let pinned = row.get::<_, i64>(5)? != 0;
            let archived = row.get::<_, i64>(6)? != 0;
            let trashed = row.get::<_, i64>(7)? != 0;
            let position: i64 = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;
            let trashed_at: Option<String> = row.get(11)?;
            let vault_state: String = row.get(12)?;
            let ct_hex: Option<String> = row.get(13)?;
            let background_pattern: String = row.get(14)?;
            let mut vault_checklist: Vec<ChecklistItem> = Vec::new();
            if vault_state == "vault" {
                match (dek, ct_hex) {
                    (Some(dek), Some(hex)) => {
                        if let Ok(bundle) = crate::vault::from_hex(&hex) {
                            if let Ok(payload) = crate::vault::decrypt_note(dek, &id, &bundle) {
                                title = payload.title;
                                body = payload.body;
                                vault_checklist = payload
                                    .checklist
                                    .into_iter()
                                    .map(|i| ChecklistItem {
                                        id: i.id,
                                        text: i.text,
                                        checked: i.checked,
                                        position: i.position,
                                        parent_id: i.parent_id,
                                    })
                                    .collect();
                            }
                        }
                    }
                    _ => {
                        title = String::new();
                        body = String::new();
                    }
                }
            }
            Ok((
                Note {
                    id,
                    kind,
                    title,
                    body,
                    color,
                    pinned,
                    archived,
                    trashed,
                    position,
                    created_at,
                    updated_at,
                    trashed_at,
                    checklist: vec![],
                    labels: vec![],
                    attachments: vec![],
                    vault: vault_state.clone(),
                    background_pattern,
                },
                vault_state,
                vault_checklist,
            ))
        })
        .optional()?;
    let Some((mut note, vault_state, vault_checklist)) = row_opt else {
        return Ok(None);
    };
    if vault_state == "vault" {
        // Checklist for vault notes lives inside the encrypted payload,
        // not in `checklist_items`. Use whatever decrypt_note recovered
        // (empty when the vault is locked).
        note.checklist = vault_checklist;
    } else {
        let mut cstmt = conn.prepare(
            "SELECT id, text, checked, position, parent_id FROM checklist_items
             WHERE note_id = ?1 ORDER BY position ASC, rowid ASC",
        )?;
        let items = cstmt
            .query_map(params![id], |row| {
                Ok(ChecklistItem {
                    id: row.get(0)?,
                    text: row.get(1)?,
                    checked: row.get::<_, i64>(2)? != 0,
                    position: row.get(3)?,
                    parent_id: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        note.checklist = items;
    }
    let mut lstmt = conn.prepare("SELECT label_id FROM note_labels WHERE note_id = ?1")?;
    let labels: Vec<String> = lstmt
        .query_map(params![id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    note.labels = labels;

    let mut astmt = conn.prepare(
        "SELECT id, note_id, kind, mime, filename, byte_size, width, height, position, created_at, resource_path, thumb_path
         FROM attachments WHERE note_id = ?1 ORDER BY position ASC, rowid ASC",
    )?;
    let attachments = astmt
        .query_map(params![id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                note_id: row.get(1)?,
                kind: row.get(2)?,
                mime: row.get(3)?,
                filename: row.get(4)?,
                byte_size: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                position: row.get(8)?,
                created_at: row.get(9)?,
                resource_path: row.get(10)?,
                thumb_path: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    note.attachments = attachments;
    Ok(Some(note))
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let conn = state.db.lock();

    // EI-08 — 3 bulk queries instead of 1 + 3N. With N=1000 we go from
    // 4001 prepared-statement executions to 3. Order preserved by writing
    // into a Vec<Note> in note-row order, then attaching children by id.
    let mut nstmt = conn
        .prepare(
            "SELECT id, kind, title, body, color, pinned, archived, trashed, position,
                    created_at, updated_at, trashed_at, vault, vault_ciphertext,
                    background_pattern
             FROM notes
             ORDER BY pinned DESC, updated_at DESC",
        )
        .map_err(err)?;
    // We collect (Note, vault_ciphertext) so we can decrypt vault rows
    // after the rest of the query runs without holding the prepared
    // statement borrow.
    let dek_guard = state.vault_dek.lock();
    let dek_opt = dek_guard.as_ref();
    let mut notes: Vec<Note> = Vec::new();
    let mut crows = nstmt.query([]).map_err(err)?;
    while let Some(row) = crows.next().map_err(err)? {
        let id: String = row.get(0).map_err(err)?;
        let vault_state: String = row.get(12).map_err(err)?;
        let mut title: String = row.get(2).map_err(err)?;
        let mut body: String = row.get(3).map_err(err)?;
        let mut vault_label = vault_state.clone();
        if vault_state == "vault" {
            if let Some(dek) = dek_opt {
                let ct_hex: Option<String> = row.get(13).map_err(err)?;
                if let Some(hex) = ct_hex {
                    if let Ok(bundle) = crate::vault::from_hex(&hex) {
                        if let Ok(payload) = crate::vault::decrypt_note(dek, &id, &bundle) {
                            title = payload.title;
                            body = payload.body;
                            // Keep vault_label = "vault" so the UI knows
                            // to show the unlocked vault badge.
                        }
                    }
                }
            } else {
                // Vault is locked — surface placeholders. The frontend
                // discriminates on `vault === "vault"` to render the
                // lock icon.
                title = String::new();
                body = String::new();
                vault_label = "vault".to_string();
            }
        }
        notes.push(Note {
            id,
            kind: row.get(1).map_err(err)?,
            title,
            body,
            color: row.get(4).map_err(err)?,
            pinned: row.get::<_, i64>(5).map_err(err)? != 0,
            archived: row.get::<_, i64>(6).map_err(err)? != 0,
            trashed: row.get::<_, i64>(7).map_err(err)? != 0,
            position: row.get(8).map_err(err)?,
            created_at: row.get(9).map_err(err)?,
            updated_at: row.get(10).map_err(err)?,
            trashed_at: row.get(11).map_err(err)?,
            checklist: Vec::new(),
            labels: Vec::new(),
            attachments: Vec::new(),
            vault: vault_label,
            background_pattern: row.get(14).map_err(err)?,
        });
    }
    drop(crows);
    drop(nstmt);
    let vault_unlocked = dek_opt.is_some();
    drop(dek_guard);
    // For vault rows we also need to decrypt and pull the checklist from
    // the encrypted payload (the rows table is empty for those notes).
    // The plain-text loop below covers all rows, so vault checklists are
    // already in place — only restore-time decrypt is needed when the
    // vault is unlocked.
    if vault_unlocked {
        // Re-decrypt to recover checklist for "vault" rows.
        let dek_guard = state.vault_dek.lock();
        let dek = dek_guard.as_ref().expect("we just confirmed it");
        let mut stmt = conn
            .prepare("SELECT id, vault_ciphertext FROM notes WHERE vault = 'vault'")
            .map_err(err)?;
        let mut rows = stmt.query([]).map_err(err)?;
        let mut decrypted: std::collections::HashMap<String, Vec<ChecklistItem>> =
            std::collections::HashMap::new();
        while let Some(row) = rows.next().map_err(err)? {
            let id: String = row.get(0).map_err(err)?;
            let hex: Option<String> = row.get(1).map_err(err)?;
            if let Some(hex) = hex {
                if let Ok(bundle) = crate::vault::from_hex(&hex) {
                    if let Ok(payload) = crate::vault::decrypt_note(dek, &id, &bundle) {
                        let items: Vec<ChecklistItem> = payload
                            .checklist
                            .into_iter()
                            .map(|i| ChecklistItem {
                                id: i.id,
                                text: i.text,
                                checked: i.checked,
                                position: i.position,
                                parent_id: i.parent_id,
                            })
                            .collect();
                        decrypted.insert(id, items);
                    }
                }
            }
        }
        drop(rows);
        drop(stmt);
        drop(dek_guard);
        for n in notes.iter_mut() {
            if n.vault == "vault" {
                if let Some(items) = decrypted.remove(&n.id) {
                    n.checklist = items;
                }
            }
        }
    }

    // Build an id -> Vec index for in-place stitching.
    use std::collections::HashMap;
    let mut idx: HashMap<String, usize> = HashMap::with_capacity(notes.len());
    for (i, n) in notes.iter().enumerate() {
        idx.insert(n.id.clone(), i);
    }

    let mut cstmt = conn
        .prepare(
            "SELECT note_id, id, text, checked, position, parent_id
             FROM checklist_items
             ORDER BY position ASC, rowid ASC",
        )
        .map_err(err)?;
    let mut crows = cstmt.query([]).map_err(err)?;
    while let Some(row) = crows.next().map_err(err)? {
        let note_id: String = row.get(0).map_err(err)?;
        if let Some(&i) = idx.get(&note_id) {
            notes[i].checklist.push(ChecklistItem {
                id: row.get(1).map_err(err)?,
                text: row.get(2).map_err(err)?,
                checked: row.get::<_, i64>(3).map_err(err)? != 0,
                position: row.get(4).map_err(err)?,
                parent_id: row.get(5).map_err(err)?,
            });
        }
    }

    let mut lstmt = conn
        .prepare("SELECT note_id, label_id FROM note_labels")
        .map_err(err)?;
    let mut lrows = lstmt.query([]).map_err(err)?;
    while let Some(row) = lrows.next().map_err(err)? {
        let note_id: String = row.get(0).map_err(err)?;
        let label_id: String = row.get(1).map_err(err)?;
        if let Some(&i) = idx.get(&note_id) {
            notes[i].labels.push(label_id);
        }
    }

    let mut astmt = conn
        .prepare(
            "SELECT note_id, id, kind, mime, filename, byte_size, width, height, position, created_at, resource_path, thumb_path
             FROM attachments ORDER BY position ASC, rowid ASC",
        )
        .map_err(err)?;
    let mut arows = astmt.query([]).map_err(err)?;
    while let Some(row) = arows.next().map_err(err)? {
        let note_id: String = row.get(0).map_err(err)?;
        if let Some(&i) = idx.get(&note_id) {
            notes[i].attachments.push(Attachment {
                id: row.get(1).map_err(err)?,
                note_id: note_id.clone(),
                kind: row.get(2).map_err(err)?,
                mime: row.get(3).map_err(err)?,
                filename: row.get(4).map_err(err)?,
                byte_size: row.get(5).map_err(err)?,
                width: row.get(6).map_err(err)?,
                height: row.get(7).map_err(err)?,
                position: row.get(8).map_err(err)?,
                created_at: row.get(9).map_err(err)?,
                resource_path: row.get(10).map_err(err)?,
                thumb_path: row.get(11).map_err(err)?,
            });
        }
    }

    Ok(notes)
}

#[tauri::command]
pub fn get_note(state: State<'_, AppState>, id: String) -> Result<Option<Note>, String> {
    let dek_guard = state.vault_dek.lock();
    let conn = state.db.lock();
    load_note_with_vault(&conn, &id, dek_guard.as_ref()).map_err(err)
}

/// EI-18 — FTS5-backed full-text search. Returns the matching note IDs
/// ranked by relevance (FTS5's bm25 default). Capped at 500 to bound
/// the renderer-side narrow step.
///
/// Vault rows are not indexed (see schema v9 migration) — searching for
/// a word inside a vaulted note returns no hit even when unlocked.
/// That's the desired security property; document it in the search UI
/// if it surprises users in practice.
#[tauri::command]
pub fn search_notes(state: State<'_, AppState>, query: String) -> Result<Vec<String>, String> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let fts_query = build_fts5_query(trimmed);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT note_id FROM notes_fts \
             WHERE notes_fts MATCH ?1 \
             ORDER BY rank \
             LIMIT 500",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![fts_query], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<String>>>()
        .map_err(err)?;
    Ok(rows)
}

/// Sanitize user input for FTS5's MATCH operator. We split on
/// whitespace, double-quote each token (which makes it a phrase match
/// that ignores FTS5-special characters like `(`, `)`, `*`, `:`, AND,
/// OR, NEAR, etc.), escape any embedded `"` by doubling, and append
/// `*` so each token is a prefix match (so typing "mil" finds "milk").
/// Tokens are joined with whitespace which FTS5 reads as implicit AND.
pub(super) fn build_fts5_query(input: &str) -> String {
    input
        .split_whitespace()
        .filter(|t| !t.is_empty())
        .map(|t| {
            let escaped = t.replace('"', "\"\"");
            format!("\"{escaped}\"*")
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[tauri::command]
pub fn create_note(state: State<'_, AppState>, input: NoteInput) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    validate_note_input(&input)?;
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    tx.execute(
        "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 0, 0, ?7, ?7)",
        params![
            id,
            input.kind,
            input.title,
            input.body,
            input.color,
            input.pinned as i64,
            now,
        ],
    )
    .map_err(err)?;
    let mut checklist_out: Vec<ChecklistItem> = Vec::with_capacity(input.checklist.len());
    for item in &input.checklist {
        let item_id = item
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        tx.execute(
            "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item_id,
                id,
                item.text,
                item.checked as i64,
                item.position,
                item.parent_id
            ],
        )
        .map_err(err)?;
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
            parent_id: item.parent_id.clone(),
        });
    }
    for label_id in &input.labels {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![id, label_id],
        )
        .map_err(err)?;
    }
    // NF-22 — persist background_pattern (already validated in the
    // input). Default INSERT above wrote the column-default empty string;
    // a single UPDATE keeps the create_note SQL short.
    if !input.background_pattern.is_empty() {
        tx.execute(
            "UPDATE notes SET background_pattern = ?1 WHERE id = ?2",
            params![input.background_pattern, id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    // EI-26 — release the mutex immediately after commit; constructing the
    // returned Note from inputs avoids re-reading the database under lock.
    drop(conn);
    Ok(Note {
        id,
        kind: input.kind,
        title: input.title,
        body: input.body,
        color: input.color,
        pinned: input.pinned,
        archived: false,
        trashed: false,
        position: 0,
        created_at: now.clone(),
        updated_at: now,
        trashed_at: None,
        checklist: checklist_out,
        labels: input.labels,
        attachments: Vec::new(),
        vault: "plain".to_string(),
        background_pattern: input.background_pattern,
    })
}

#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    id: String,
    input: NoteInput,
) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    validate_note_input(&input)?;
    let dek_guard = state.vault_dek.lock();
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let now = now_iso();
    // Read created_at + archived/trashed flags so the returned Note
    // accurately reflects what's on disk (we don't change those fields
    // here). `vault` decides whether we write plaintext columns or
    // re-encrypt into vault_ciphertext.
    let (created_at, archived, trashed, trashed_at, position, vault_state): (
        String,
        i64,
        i64,
        Option<String>,
        i64,
        String,
    ) = tx
        .query_row(
            "SELECT created_at, archived, trashed, trashed_at, position, vault \
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
                ))
            },
        )
        .map_err(|_| format!("note {id} not found"))?;
    let is_vault = vault_state == "vault";
    if is_vault && dek_guard.is_none() {
        return Err("vault is locked — unlock it before editing this note".into());
    }
    // NF-V0.5-D — snapshot the pre-update row into note_snapshots before
    // touching it. Vault rows snapshot their ciphertext as-is; plain
    // rows snapshot the title/body columns plus a JSON-encoded checklist.
    history::snapshot_current_note(&tx, &id, &now)?;
    // Pre-assign ids for new checklist items so the encrypted payload
    // and the returned Note agree on ids.
    let mut checklist_out: Vec<ChecklistItem> = Vec::with_capacity(input.checklist.len());
    for item in &input.checklist {
        let item_id = item
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
            parent_id: item.parent_id.clone(),
        });
    }
    if is_vault {
        let dek = dek_guard.as_ref().expect("checked above");
        let payload = crate::vault::VaultPayload {
            title: input.title.clone(),
            body: input.body.clone(),
            checklist: checklist_out
                .iter()
                .map(|c| crate::vault::VaultChecklistItem {
                    id: c.id.clone(),
                    text: c.text.clone(),
                    checked: c.checked,
                    position: c.position,
                    parent_id: c.parent_id.clone(),
                })
                .collect(),
        };
        let bundle = crate::vault::encrypt_note(dek, &id, &payload).map_err(|e| e.to_string())?;
        tx.execute(
            "UPDATE notes
               SET kind = ?1, title = '', body = '', color = ?2, pinned = ?3, updated_at = ?4,
                   vault_ciphertext = ?5, background_pattern = ?6
             WHERE id = ?7",
            params![
                input.kind,
                input.color,
                input.pinned as i64,
                now,
                crate::vault::to_hex(&bundle),
                input.background_pattern,
                id,
            ],
        )
        .map_err(err)?;
        // Vault rows never carry checklist_items rows — they were
        // deleted at move_note_to_vault time. Skip the per-row inserts.
    } else {
        tx.execute(
            "UPDATE notes
               SET kind = ?1, title = ?2, body = ?3, color = ?4, pinned = ?5, updated_at = ?6,
                   background_pattern = ?7
             WHERE id = ?8",
            params![
                input.kind,
                input.title,
                input.body,
                input.color,
                input.pinned as i64,
                now,
                input.background_pattern,
                id,
            ],
        )
        .map_err(err)?;
        tx.execute(
            "DELETE FROM checklist_items WHERE note_id = ?1",
            params![id],
        )
        .map_err(err)?;
        for item in &checklist_out {
            tx.execute(
                "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    item.id,
                    id,
                    item.text,
                    item.checked as i64,
                    item.position,
                    item.parent_id
                ],
            )
            .map_err(err)?;
        }
    }
    tx.execute("DELETE FROM note_labels WHERE note_id = ?1", params![id])
        .map_err(err)?;
    for label_id in &input.labels {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![id, label_id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    drop(conn);
    drop(dek_guard);
    // Re-read attachments (we don't change them in update_note) for the
    // returned Note. Cheap — one indexed SELECT.
    let conn = state.db.lock();
    let attachments = load_attachments(&conn, &id).map_err(err)?;
    drop(conn);
    Ok(Note {
        id,
        kind: input.kind,
        title: input.title,
        body: input.body,
        color: input.color,
        pinned: input.pinned,
        archived: archived != 0,
        trashed: trashed != 0,
        position,
        created_at,
        updated_at: now,
        trashed_at,
        checklist: checklist_out,
        labels: input.labels,
        attachments,
        vault: vault_state,
        background_pattern: input.background_pattern,
    })
}

pub(super) fn load_attachments(
    conn: &Connection,
    note_id: &str,
) -> Result<Vec<Attachment>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, kind, mime, filename, byte_size, width, height, position, created_at, resource_path, thumb_path
         FROM attachments WHERE note_id = ?1 ORDER BY position ASC, rowid ASC",
    )?;
    let rows = stmt
        .query_map(params![note_id], |row| {
            Ok(Attachment {
                id: row.get(0)?,
                note_id: row.get(1)?,
                kind: row.get(2)?,
                mime: row.get(3)?,
                filename: row.get(4)?,
                byte_size: row.get(5)?,
                width: row.get(6)?,
                height: row.get(7)?,
                position: row.get(8)?,
                created_at: row.get(9)?,
                resource_path: row.get(10)?,
                thumb_path: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// NF-18 — "Make a copy". Server-side duplicate so timestamps and ID
/// generation match every other create path. Pinning is intentionally
/// reset to false (matches Keep's behavior); archive/trash are also
/// reset so the copy lands in the active Notes section regardless of
/// where the source lives.
#[tauri::command]
pub fn duplicate_note(state: State<'_, AppState>, id: String) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let mut conn = state.db.lock();
    let source = load_note(&conn, &id)
        .map_err(err)?
        .ok_or_else(|| format!("note {id} not found"))?;
    if source.vault == "vault" {
        // Duplicating a vault note would silently drop its contents
        // (the row's title/body columns are empty until decrypted). Make
        // the user move the source out of the vault first.
        return Err("vault notes cannot be duplicated — move out of the vault first".into());
    }
    let tx = conn.transaction().map_err(err)?;
    let new_id = Uuid::new_v4().to_string();
    let now = now_iso();
    let copy_title = if source.title.is_empty() {
        String::new()
    } else {
        format!("{} (copy)", source.title)
    };
    tx.execute(
        "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, position, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 0, 0, ?6, ?6)",
        params![new_id, source.kind, copy_title, source.body, source.color, now],
    )
    .map_err(err)?;
    // Two-pass copy: first assign fresh ids, then write rows with
    // parent_id remapped from the source-side id to the new id. NF-21
    // sub-items would otherwise reference rows that don't exist in the
    // duplicate.
    let mut id_map: std::collections::HashMap<String, String> =
        std::collections::HashMap::with_capacity(source.checklist.len());
    for item in &source.checklist {
        id_map.insert(item.id.clone(), Uuid::new_v4().to_string());
    }
    let mut checklist_out: Vec<ChecklistItem> = Vec::with_capacity(source.checklist.len());
    for item in &source.checklist {
        let item_id = id_map.get(&item.id).expect("just inserted").clone();
        let new_parent = item.parent_id.as_ref().and_then(|p| id_map.get(p)).cloned();
        tx.execute(
            "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                item_id,
                new_id,
                item.text,
                item.checked as i64,
                item.position,
                new_parent
            ],
        )
        .map_err(err)?;
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
            parent_id: new_parent,
        });
    }
    for label_id in &source.labels {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![new_id, label_id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    drop(conn);
    Ok(Note {
        id: new_id,
        kind: source.kind,
        title: copy_title,
        body: source.body,
        color: source.color,
        pinned: false,
        archived: false,
        trashed: false,
        position: 0,
        created_at: now.clone(),
        updated_at: now,
        trashed_at: None,
        checklist: checklist_out,
        labels: source.labels,
        // v0.4 — duplicate_note intentionally does NOT copy attachments;
        // they'd be deep clones of file blobs and the use case (template
        // notes) usually doesn't want a megabyte image duplicated. The
        // user can manually re-attach if they really want a copy. Calling
        // it out so the absence isn't a bug.
        attachments: Vec::new(),
        vault: "plain".to_string(),
        background_pattern: source.background_pattern,
    })
}

#[tauri::command]
pub fn reorder_notes(state: State<'_, AppState>, ids: Vec<String>) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    if ids.len() > 100_000 {
        return Err("too many ids in reorder_notes".into());
    }
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute(
            "UPDATE notes SET position = ?1 WHERE id = ?2",
            params![i as i64, id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    Ok(())
}

/// Permanently delete a note + clean up its attachment files on disk.
///
/// Cascading FK takes care of `attachments`, `note_labels`, `reminders`,
/// `note_snapshots`, `checklist_items`, etc. rows. **But the resource
/// files at `<data_dir>/resources/...` are NOT covered by the DB cascade** —
/// they have to be removed manually, or the user's disk fills up with orphaned
/// blobs after every "Delete forever" (silent data / space leak that survived
/// from v0.1 → v0.22).
#[tauri::command]
pub fn delete_note_permanent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let conn = state.db.lock();
    // Snapshot attachment file metadata BEFORE the cascade deletes the rows.
    let files =
        attachments::collect_attachment_files(&conn, std::slice::from_ref(&id)).map_err(err)?;
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(err)?;
    let resources = state.data_dir.join(attachments::RESOURCES_DIR);
    for files in &files {
        attachments::delete_attachment_files(&conn, &resources, files);
    }
    drop(conn);
    Ok(())
}

#[tauri::command]
pub fn set_archived(state: State<'_, AppState>, id: String, archived: bool) -> Result<(), String> {
    let conn = state.db.lock();
    let now = now_iso();
    conn.execute(
        "UPDATE notes SET archived = ?1, trashed = 0, trashed_at = NULL, updated_at = ?2 WHERE id = ?3",
        params![archived as i64, now, id],
    )
    .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn set_trashed(state: State<'_, AppState>, id: String, trashed: bool) -> Result<(), String> {
    let conn = state.db.lock();
    let now = now_iso();
    if trashed {
        conn.execute(
            "UPDATE notes SET trashed = 1, archived = 0, pinned = 0, trashed_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(err)?;
    } else {
        conn.execute(
            "UPDATE notes SET trashed = 0, trashed_at = NULL, updated_at = ?1 WHERE id = ?2",
            params![now, id],
        )
        .map_err(err)?;
    }
    Ok(())
}

#[tauri::command]
pub fn set_pinned(state: State<'_, AppState>, id: String, pinned: bool) -> Result<(), String> {
    let conn = state.db.lock();
    let now = now_iso();
    conn.execute(
        "UPDATE notes SET pinned = ?1, archived = 0, updated_at = ?2 WHERE id = ?3",
        params![pinned as i64, now, id],
    )
    .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn set_color(state: State<'_, AppState>, id: String, color: String) -> Result<(), String> {
    let conn = state.db.lock();
    let now = now_iso();
    conn.execute(
        "UPDATE notes SET color = ?1, updated_at = ?2 WHERE id = ?3",
        params![color, now, id],
    )
    .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let conn = state.db.lock();
    // Snapshot trashed note ids + their attachment files BEFORE cascade.
    let note_ids: Vec<String> = conn
        .prepare("SELECT id FROM notes WHERE trashed = 1")
        .map_err(err)?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(err)?;
    let files = attachments::collect_attachment_files(&conn, &note_ids).map_err(err)?;
    conn.execute("DELETE FROM notes WHERE trashed = 1", [])
        .map_err(err)?;
    let resources = state.data_dir.join(attachments::RESOURCES_DIR);
    for files in &files {
        attachments::delete_attachment_files(&conn, &resources, files);
    }
    drop(conn);
    log::info!(
        "empty_trash: deleted {} notes, cleaned {} attachment files",
        note_ids.len(),
        files.len()
    );
    Ok(())
}
