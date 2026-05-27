use crate::AppState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::{Manager, State};
use uuid::Uuid;
use zip::write::SimpleFileOptions;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    /// NF-V0.5-21 (v0.14+): one-level nesting. When set, this item is
    /// indented under the referenced sibling. Validated server-side so
    /// the referenced parent itself has `parent_id = None` (Keep parity
    /// — only one level deep). Defaults absent for plain top-level items.
    #[serde(default)]
    pub parent_id: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Attachment {
    pub id: String,
    pub note_id: String,
    pub kind: String, // "image" | "drawing" | "audio" | "file"
    pub mime: String,
    pub filename: String,
    pub byte_size: i64,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub position: i64,
    pub created_at: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Note {
    pub id: String,
    pub kind: String, // "text" | "list"
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub archived: bool,
    pub trashed: bool,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
    pub trashed_at: Option<String>,
    pub checklist: Vec<ChecklistItem>,
    pub labels: Vec<String>, // label IDs
    pub attachments: Vec<Attachment>,
    /// NF-V0.5-C — "plain" or "vault". When "vault" + DEK is unlocked,
    /// title/body/checklist are decrypted before being returned. When
    /// "vault" + DEK is locked, the renderer shows a "🔒 Locked" card.
    #[serde(default = "default_vault_state")]
    pub vault: String,
    /// NF-22 (v0.14+): pattern key from the renderer-side whitelist
    /// (`src/lib/backgroundPatterns.ts`). Empty string = no pattern.
    /// Unknown values map to "none" client-side without error.
    #[serde(default)]
    pub background_pattern: String,
}

fn default_vault_state() -> String {
    "plain".to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Label {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct NoteInput {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub checklist: Vec<ChecklistItemInput>,
    pub labels: Vec<String>,
    /// NF-22 (v0.14+): pattern key or "" for none. Validated to be one
    /// of the known whitelist values (or empty) so a renderer bug can't
    /// land an arbitrary string in the column.
    #[serde(default)]
    pub background_pattern: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChecklistItemInput {
    pub id: Option<String>,
    pub text: String,
    pub checked: bool,
    pub position: i64,
    /// NF-V0.5-21 (v0.14+): when set, references another item in the
    /// same `checklist` input array by its `id`. Validated by
    /// `validate_note_input` — must be present in the same array, and
    /// that referenced item must itself have no parent (one level).
    #[serde(default)]
    pub parent_id: Option<String>,
}

fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// EI-33 — server-side input caps. The renderer is trusted but we still defend
// the SQLite store from accidentally-huge payloads or malformed kinds.
const MAX_TITLE_CHARS: usize = 1024;
const MAX_BODY_BYTES: usize = 64 * 1024; // 64 KiB
const MAX_CHECKLIST_ITEMS: usize = 1000;
const MAX_CHECKLIST_ITEM_CHARS: usize = 2048;

fn validate_note_input(input: &NoteInput) -> Result<(), String> {
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

const ALLOWED_BACKGROUND_PATTERNS: &[&str] = &[
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

fn is_valid_background_pattern(s: &str) -> bool {
    ALLOWED_BACKGROUND_PATTERNS.contains(&s)
}

fn load_note(conn: &Connection, id: &str) -> Result<Option<Note>, rusqlite::Error> {
    load_note_with_vault(conn, id, None)
}

/// NF-V0.5-C — `load_note` variant that decrypts a vault note in place
/// if the caller passes the unlocked DEK. Without the DEK, vault notes
/// come back with empty title/body/checklist + `vault = "vault"` so the
/// renderer can show the lock placeholder.
fn load_note_with_vault(
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
    let mut lstmt =
        conn.prepare("SELECT label_id FROM note_labels WHERE note_id = ?1")?;
    let labels: Vec<String> = lstmt
        .query_map(params![id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    note.labels = labels;

    let mut astmt = conn.prepare(
        "SELECT id, note_id, kind, mime, filename, byte_size, width, height, position, created_at
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
            "SELECT note_id, id, kind, mime, filename, byte_size, width, height, position, created_at
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
fn build_fts5_query(input: &str) -> String {
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
            params![item_id, id, item.text, item.checked as i64, item.position, item.parent_id],
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
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .map_err(|_| format!("note {id} not found"))?;
    let is_vault = vault_state == "vault";
    if is_vault && dek_guard.is_none() {
        return Err("vault is locked — unlock it before editing this note".into());
    }
    // NF-V0.5-D — snapshot the pre-update row into note_snapshots before
    // touching it. Vault rows snapshot their ciphertext as-is; plain
    // rows snapshot the title/body columns plus a JSON-encoded checklist.
    snapshot_current_note(&tx, &id, &now)?;
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
        tx.execute("DELETE FROM checklist_items WHERE note_id = ?1", params![id])
            .map_err(err)?;
        for item in &checklist_out {
            tx.execute(
                "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item.id, id, item.text, item.checked as i64, item.position, item.parent_id],
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

fn load_attachments(
    conn: &Connection,
    note_id: &str,
) -> Result<Vec<Attachment>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, note_id, kind, mime, filename, byte_size, width, height, position, created_at
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
    let source = load_note(&conn, &id).map_err(err)?.ok_or_else(|| format!("note {id} not found"))?;
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
        let new_parent = item
            .parent_id
            .as_ref()
            .and_then(|p| id_map.get(p))
            .cloned();
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

// --- NF-08 Markdown vault export + Google Takeout import ---

const VAULT_RESOURCES_DIR: &str = "_resources";

fn sanitize_vault_filename(stem: &str, id: &str) -> String {
    // Filename-safe: keep letters/digits/space/dash/underscore/dot, replace
    // everything else with `-`. Cap at 80 chars. Fall back to the note id
    // when the result is empty.
    let mut out = String::with_capacity(stem.len());
    for c in stem.chars() {
        if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' || c == '.' {
            out.push(c);
        } else {
            out.push('-');
        }
    }
    let trimmed: String = out.trim_matches(|c: char| c == ' ' || c == '.').to_string();
    let capped: String = trimmed.chars().take(80).collect();
    if capped.is_empty() {
        format!("note-{}", id.chars().take(8).collect::<String>())
    } else {
        capped
    }
}

#[tauri::command]
pub fn export_vault(
    state: State<'_, AppState>,
    dest_dir: String,
) -> Result<String, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let parent = PathBuf::from(&dest_dir);
    if !parent.is_dir() {
        return Err(format!("not a directory: {dest_dir}"));
    }
    // EI-V0.5-6 — write to a fresh per-run subfolder so re-exporting
    // never silently overwrites a previous vault (or external edits
    // to those .md files). Folder name is `keepr-vault-<ISO>` with
    // colon and dot stripped for filesystem safety.
    let stamp = chrono::Utc::now()
        .format("%Y-%m-%dT%H-%M-%S")
        .to_string();
    let dest = parent.join(format!("keepr-vault-{stamp}"));
    std::fs::create_dir_all(&dest).map_err(err)?;
    let resources_out = dest.join(VAULT_RESOURCES_DIR);
    std::fs::create_dir_all(&resources_out).map_err(err)?;

    let labels_by_id: std::collections::HashMap<String, String> = {
        let conn = state.db.lock();
        let mut stmt = conn.prepare("SELECT id, name FROM labels").map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                let id: String = r.get(0)?;
                let name: String = r.get(1)?;
                Ok((id, name))
            })
            .map_err(err)?;
        let mut map = std::collections::HashMap::new();
        for row in rows {
            let (id, name) = row.map_err(err)?;
            map.insert(id, name);
        }
        map
    };

    let notes = list_notes(state.clone())?;
    let mut used_names: std::collections::HashSet<String> = std::collections::HashSet::new();

    for n in &notes {
        if n.trashed {
            continue; // never export deleted notes
        }
        let mut name = sanitize_vault_filename(&n.title, &n.id);
        let base = name.clone();
        let mut counter = 2;
        while used_names.contains(&name) {
            name = format!("{base}-{counter}");
            counter += 1;
            if counter > 999 {
                // EI-V0.5-6 — fall back to the full UUID + re-check so we
                // never insert a duplicate name even after 999 collisions.
                name = format!("{base}-{}", &n.id);
                if used_names.contains(&name) {
                    name = n.id.clone();
                }
                break;
            }
        }
        used_names.insert(name.clone());

        let frontmatter = build_frontmatter(n, &labels_by_id);
        let body = if n.kind == "list" {
            n.checklist
                .iter()
                .map(|it| {
                    let mark = if it.checked { "x" } else { " " };
                    format!("- [{mark}] {}", it.text)
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            n.body.clone()
        };
        let mut content = String::new();
        content.push_str(&frontmatter);
        content.push('\n');
        if !n.title.is_empty() {
            content.push_str(&format!("# {}\n\n", n.title));
        }
        content.push_str(&body);
        if !content.ends_with('\n') {
            content.push('\n');
        }
        // Attachment links at the bottom.
        if !n.attachments.is_empty() {
            content.push_str("\n");
            for att in &n.attachments {
                let ext = mime_to_ext(&att.mime);
                let stored_name = format!("{}.{ext}", att.id);
                content.push_str(&format!(
                    "![{}]({}/{})\n",
                    att.filename.replace(']', " ").replace('[', " "),
                    VAULT_RESOURCES_DIR,
                    stored_name
                ));
                // Copy the file alongside.
                let src = state.data_dir.join("resources").join(&stored_name);
                let dst = resources_out.join(&stored_name);
                if src.exists() {
                    let _ = std::fs::copy(&src, &dst);
                }
            }
        }
        let md_path = dest.join(format!("{name}.md"));
        std::fs::write(&md_path, content).map_err(err)?;
    }
    // Return the absolute path to the freshly-written vault folder so the
    // renderer can show it in the toast.
    Ok(dest.to_string_lossy().to_string())
}

fn build_frontmatter(
    n: &Note,
    labels_by_id: &std::collections::HashMap<String, String>,
) -> String {
    let label_names: Vec<String> = n
        .labels
        .iter()
        .filter_map(|id| labels_by_id.get(id).cloned())
        .collect();
    let mut s = String::from("---\n");
    s.push_str(&format!("id: {}\n", n.id));
    s.push_str(&format!("type: {}\n", n.kind));
    s.push_str(&format!("color: {}\n", n.color));
    s.push_str(&format!("pinned: {}\n", n.pinned));
    s.push_str(&format!("archived: {}\n", n.archived));
    s.push_str(&format!("created: {}\n", n.created_at));
    s.push_str(&format!("updated: {}\n", n.updated_at));
    if !label_names.is_empty() {
        s.push_str("labels:\n");
        for name in &label_names {
            s.push_str(&format!("  - {}\n", yaml_quote_if_needed(name)));
        }
    }
    s.push_str("---\n");
    s
}

fn yaml_quote_if_needed(s: &str) -> String {
    // If the value contains : # & * { } [ ] , | > ' " % @ ` or starts with
    // - we double-quote it. Conservative — over-quotes some safe values
    // but never under-quotes.
    let needs = s
        .chars()
        .any(|c| matches!(c, ':' | '#' | '&' | '*' | '{' | '}' | '[' | ']' | ',' | '|' | '>' | '\'' | '"' | '%' | '@' | '`'))
        || s.starts_with('-')
        || s.is_empty();
    if needs {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn mime_to_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => "bin",
    }
}

/// NF-08 — Google Keep Takeout importer. The Takeout export is a ZIP
/// where each note lives next to a sibling HTML rendering (which we
/// ignore) and any binary attachments. The canonical path is
/// `Takeout/Keep/<title>.json`, but Google localizes the folder name
/// for non-English accounts (`Takeout/Notizen/...` in German,
/// `Takeout/메모/...` in Korean) and users sometimes re-zip the
/// extracted tree without the `Takeout/` prefix. So we read every
/// `.json` in the archive and detect Keep notes by JSON shape
/// (`is_keep_note_shape`) rather than path — that way any zip that
/// contains a Keep export will import, regardless of folder naming.
///
/// Non-image attachments (audio voice notes) are skipped — we only
/// surface the image attachments Keepr knows how to render.
///
/// Returns the number of notes successfully imported.
#[tauri::command]
pub fn import_takeout(
    state: State<'_, AppState>,
    src: String,
) -> Result<u32, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let file = File::open(&src).map_err(err)?;
    let mut archive = zip::ZipArchive::new(file).map_err(err)?;

    // Two-pass: first collect attachment bytes keyed by their archive
    // path so we can resolve note-relative references.
    let mut blobs: std::collections::HashMap<String, Vec<u8>> =
        std::collections::HashMap::new();
    let mut note_entries: Vec<(String, String)> = Vec::new(); // (folder, json text)

    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(err)?;
        let name = match entry.enclosed_name() {
            Some(p) => p.to_string_lossy().replace('\\', "/"),
            None => continue,
        };
        if entry.is_dir() {
            continue;
        }
        if name.to_lowercase().ends_with(".json") {
            let mut text = String::new();
            if entry.read_to_string(&mut text).is_err() {
                continue; // binary file with .json extension — skip rather than abort
            }
            // Folder for resolving sibling attachments. Shape check
            // happens in pass 2 so we don't double-parse.
            let folder = name.rsplit_once('/').map(|(d, _)| d.to_string()).unwrap_or_default();
            note_entries.push((folder, text));
        } else if entry.size() > 0 && entry.size() <= MAX_ATTACHMENT_BYTES {
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf).map_err(err)?;
            blobs.insert(name, buf);
        }
    }

    // Resolve the label set in one pass so we don't re-query for every
    // note. Insert any missing labels first.
    let mut imported: u32 = 0;
    for (folder, text) in note_entries {
        let v: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };
        if !is_keep_note_shape(&v) {
            // Filters out Takeout's `Labels.json` (an array), Drive/
            // Photos metadata in multi-product exports, and any other
            // non-Keep JSON that happens to share the archive.
            continue;
        }
        let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let text_body = v
            .get("textContent")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let pinned = v.get("isPinned").and_then(|x| x.as_bool()).unwrap_or(false);
        let archived = v.get("isArchived").and_then(|x| x.as_bool()).unwrap_or(false);
        let trashed = v.get("isTrashed").and_then(|x| x.as_bool()).unwrap_or(false);
        if trashed {
            continue; // Takeout-trashed notes get skipped on import.
        }
        let color = map_keep_color(v.get("color").and_then(|x| x.as_str()).unwrap_or("DEFAULT"));
        let label_names: Vec<String> = v
            .get("labels")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| l.get("name").and_then(|n| n.as_str()).map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // List content (checklist) — when present overrides textContent.
        let (kind, checklist_input) = if let Some(arr) = v.get("listContent").and_then(|x| x.as_array()) {
            let items: Vec<ChecklistItemInput> = arr
                .iter()
                .enumerate()
                .map(|(i, e)| ChecklistItemInput {
                    id: None,
                    text: e
                        .get("text")
                        .and_then(|t| t.as_str())
                        .unwrap_or("")
                        .to_string(),
                    checked: e
                        .get("isChecked")
                        .and_then(|t| t.as_bool())
                        .unwrap_or(false),
                    position: i as i64,
                    parent_id: None,
                })
                .collect();
            ("list".to_string(), items)
        } else {
            ("text".to_string(), Vec::new())
        };

        // Resolve label ids (creating missing ones).
        let mut label_ids: Vec<String> = Vec::new();
        for name in &label_names {
            match create_label(state.clone(), name.clone()) {
                Ok(lbl) => label_ids.push(lbl.id),
                Err(_) => {}
            }
        }

        // Create the note.
        let input = NoteInput {
            kind,
            title,
            body: text_body,
            color,
            pinned,
            checklist: checklist_input,
            labels: label_ids,
            background_pattern: String::new(),
        };
        let created = match create_note(state.clone(), input) {
            Ok(n) => n,
            Err(_) => continue,
        };

        // EI-V0.5-6 — preserve original Takeout chronology. Keep stores
        // created/updated in microseconds since the Unix epoch under
        // `createdTimestampUsec` and `userEditedTimestampUsec`. We rewrite
        // notes.created_at / updated_at directly via SQL rather than
        // through update_note (which would set updated_at = now).
        let created_iso = takeout_usec_to_rfc3339(v.get("createdTimestampUsec"));
        let updated_iso = takeout_usec_to_rfc3339(v.get("userEditedTimestampUsec"));
        if created_iso.is_some() || updated_iso.is_some() {
            let conn = state.db.lock();
            if let Some(ts) = &created_iso {
                let _ = conn.execute(
                    "UPDATE notes SET created_at = ?1 WHERE id = ?2",
                    params![ts, created.id],
                );
            }
            if let Some(ts) = &updated_iso {
                let _ = conn.execute(
                    "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
                    params![ts, created.id],
                );
            }
        }

        // Set archived after creation (NoteInput has no archived field).
        if archived {
            let _ = set_archived(state.clone(), created.id.clone(), true);
        }

        // EI-V0.5-6 — preserve Takeout reminders. Takeout's shape varies
        // by export year; we accept several common forms. Single-shot
        // only; recurring reminders (when they exist in the JSON) get
        // their fire_at imported but the rrule field is ignored.
        if let Some(reminders) = v.get("reminders").and_then(|x| x.as_array()) {
            for r in reminders {
                if let Some(fire_at) = takeout_reminder_fire_at(r) {
                    let _ = set_reminder(
                        state.clone(),
                        created.id.clone(),
                        fire_at,
                        None, // Takeout reminders import as single-shot; v0.6 recurrence happens at edit time
                    );
                    break; // schema only supports one pending reminder per note
                }
            }
        }

        // Attachments — Takeout stores them as siblings of the json,
        // referenced by "attachments": [{"filePath": "...", "mimetype": "..."}].
        if let Some(attachments) = v.get("attachments").and_then(|x| x.as_array()) {
            for a in attachments {
                let rel = a
                    .get("filePath")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                if rel.is_empty() {
                    continue;
                }
                let mime = a
                    .get("mimetype")
                    .and_then(|x| x.as_str())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                if !mime.starts_with("image/") {
                    continue;
                }
                let archive_path = if folder.is_empty() {
                    rel.clone()
                } else {
                    format!("{folder}/{rel}")
                };
                if let Some(bytes) = blobs.get(&archive_path) {
                    let ext = mime_to_ext(&mime);
                    let new_id = Uuid::new_v4().to_string();
                    let stored_name = format!("{new_id}.{ext}");
                    let resources_dir = state.data_dir.join("resources");
                    if std::fs::create_dir_all(&resources_dir).is_err() {
                        continue;
                    }
                    // EI-V0.5-13 — insert row first, then write file; on
                    // write failure DELETE the row so we never reference
                    // a missing blob (mirrors add_image_attachment's
                    // rollback pattern).
                    let now = now_iso();
                    let dest = resources_dir.join(&stored_name);
                    {
                        let conn = state.db.lock();
                        if conn
                            .execute(
                                "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at)
                                 VALUES (?1, ?2, 'image', ?3, ?4, ?5, 0, ?6)",
                                params![new_id, created.id, mime, rel, bytes.len() as i64, now],
                            )
                            .is_err()
                        {
                            continue;
                        }
                    }
                    if std::fs::write(&dest, bytes).is_err() {
                        // Roll back the row so the DB stays consistent.
                        let conn = state.db.lock();
                        let _ = conn.execute(
                            "DELETE FROM attachments WHERE id = ?1",
                            params![new_id],
                        );
                        continue;
                    }
                }
            }
        }

        imported += 1;
    }
    Ok(imported)
}

/// Detect a Google Keep note by JSON shape so we don't depend on the
/// archive path (Takeout localizes the `Keep` folder name and users
/// sometimes re-zip without the `Takeout/` prefix). A Keep note has
/// an `isPinned` boolean, at least one canonical timestamp, and at
/// least one content field (text body or checklist).
fn is_keep_note_shape(v: &serde_json::Value) -> bool {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return false,
    };
    let has_pinned = obj.get("isPinned").map(|x| x.is_boolean()).unwrap_or(false);
    let has_ts = obj.contains_key("createdTimestampUsec")
        || obj.contains_key("userEditedTimestampUsec");
    let has_content = obj.contains_key("textContent")
        || obj.contains_key("listContent");
    has_pinned && has_ts && has_content
}

fn map_keep_color(c: &str) -> String {
    // Keep's color enum -> our color keys.
    match c {
        "RED" => "red".into(),
        "ORANGE" => "orange".into(),
        "YELLOW" => "yellow".into(),
        "GREEN" => "green".into(),
        "TEAL" => "teal".into(),
        "BLUE" => "blue".into(),
        "DARK_BLUE" => "darkblue".into(),
        "PURPLE" => "purple".into(),
        "PINK" => "pink".into(),
        "BROWN" => "brown".into(),
        "GRAY" => "gray".into(),
        _ => "default".into(),
    }
}

/// Takeout JSON stores timestamps as microseconds since the Unix epoch
/// in number fields like `createdTimestampUsec`. Convert to RFC 3339.
/// Returns `None` for null / missing / non-finite inputs.
fn takeout_usec_to_rfc3339(v: Option<&serde_json::Value>) -> Option<String> {
    let usec = v?.as_u64()?;
    let secs = (usec / 1_000_000) as i64;
    let nsec = ((usec % 1_000_000) * 1_000) as u32;
    let dt = chrono::DateTime::<chrono::Utc>::from_timestamp(secs, nsec)?;
    Some(dt.to_rfc3339())
}

/// Best-effort extraction of a fire_at RFC3339 string from a Takeout
/// reminder object. Takeout's shape has drifted over years; we accept
/// `fireOn`/`fire_on` (ISO), `reminderTimeUsec`/`reminder_time_usec`
/// (microseconds), or the nested `time.formattedDate` (ISO).
fn takeout_reminder_fire_at(r: &serde_json::Value) -> Option<String> {
    if let Some(s) = r.get("fireOn").and_then(|x| x.as_str()) {
        if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
            return Some(s.to_string());
        }
    }
    if let Some(s) = r.get("fire_on").and_then(|x| x.as_str()) {
        if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
            return Some(s.to_string());
        }
    }
    if let Some(usec_value) = r.get("reminderTimeUsec").or_else(|| r.get("reminder_time_usec")) {
        if let Some(iso) = takeout_usec_to_rfc3339(Some(usec_value)) {
            return Some(iso);
        }
    }
    if let Some(time_obj) = r.get("time") {
        if let Some(s) = time_obj.get("formattedDate").and_then(|x| x.as_str()) {
            if chrono::DateTime::parse_from_rfc3339(s).is_ok() {
                return Some(s.to_string());
            }
        }
    }
    None
}

// --- NF-02 reminders ---

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Reminder {
    // EI-V0.5-14 (v0.12+): `note_id` is the natural key — one reminder
    // per note. The redundant `id` column was dropped in schema v8.
    pub note_id: String,
    pub fire_at: String,
    pub rrule: Option<String>,
    pub snooze_until: Option<String>,
    pub fired_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub created_at: String,
}

/// Supported recurrence rule shapes (NF-V0.5-A). We accept only the
/// four FREQ= bases that Keep's UI exposes, not arbitrary RFC 5545
/// strings — that lets us expand `next_fire_at` in plain Rust without
/// pulling a 70 KB RRULE crate. Custom intervals (e.g. every 2 weeks)
/// land in a future pass.
const ALLOWED_RRULES: &[&str] = &[
    "FREQ=DAILY",
    "FREQ=WEEKLY",
    "FREQ=MONTHLY",
    "FREQ=YEARLY",
];

fn validate_rrule(rrule: Option<&str>) -> Result<(), String> {
    match rrule {
        None => Ok(()),
        Some(s) if ALLOWED_RRULES.iter().any(|allowed| *allowed == s) => Ok(()),
        Some(other) => Err(format!(
            "unsupported rrule '{other}' — expected one of {:?}",
            ALLOWED_RRULES
        )),
    }
}

/// Compute the next `fire_at` after a successful fire, given the
/// previous `fire_at` and the recurrence rule. Returns None for
/// one-shot reminders (no rrule). NF-V0.5-A.
pub fn next_fire_at(prev_fire_at: &str, rrule: Option<&str>) -> Option<String> {
    use chrono::{DateTime, Datelike, Months, Utc};
    let rule = rrule?;
    let prev = DateTime::parse_from_rfc3339(prev_fire_at).ok()?;
    let prev_utc: DateTime<Utc> = prev.with_timezone(&Utc);
    let next = match rule {
        "FREQ=DAILY" => prev_utc + chrono::Duration::days(1),
        "FREQ=WEEKLY" => prev_utc + chrono::Duration::weeks(1),
        "FREQ=MONTHLY" => prev_utc.checked_add_months(Months::new(1))?,
        "FREQ=YEARLY" => {
            // Construct a new DateTime with year + 1. chrono doesn't
            // have add_years; do via with_year + leap-day clamp.
            let y = prev_utc.year() + 1;
            prev_utc.with_year(y)
                .or_else(|| {
                    // Feb 29 → Feb 28 in non-leap years
                    prev_utc.with_day(28).and_then(|d| d.with_year(y))
                })?
        }
        _ => return None,
    };
    Some(next.to_rfc3339())
}

#[tauri::command]
pub fn set_reminder(
    state: State<'_, AppState>,
    note_id: String,
    fire_at: String,
    rrule: Option<String>,
) -> Result<Reminder, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    // Basic validation: fire_at must be parseable RFC3339.
    if chrono::DateTime::parse_from_rfc3339(&fire_at).is_err() {
        return Err(format!("fire_at not a valid RFC3339 timestamp: {fire_at}"));
    }
    validate_rrule(rrule.as_deref())?;
    let conn = state.db.lock();
    let now = now_iso();
    // UPSERT keyed on note_id (now the PK after v8 schema cleanup) —
    // replacing an existing reminder rather than appending. Resets
    // fired/dismissed/snooze so re-setting effectively re-arms it.
    conn.execute(
        "INSERT INTO reminders (note_id, fire_at, rrule, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(note_id) DO UPDATE SET
            fire_at = excluded.fire_at,
            rrule = excluded.rrule,
            fired_at = NULL,
            dismissed_at = NULL,
            snooze_until = NULL",
        params![note_id, fire_at, rrule, now],
    )
    .map_err(err)?;
    let r = conn
        .query_row(
            "SELECT note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
             FROM reminders WHERE note_id = ?1",
            params![note_id],
            reminder_from_row,
        )
        .map_err(err)?;
    Ok(r)
}

/// NF-V0.5-A — snooze a reminder until a later time. The reminder
/// stays in the pending pool but `take_due_reminders`'s WHERE clause
/// excludes anything with `snooze_until > now`, so the scheduler
/// skips it until the snooze elapses. fired_at is also cleared so
/// a freshly-snoozed reminder fires again.
#[tauri::command]
pub fn snooze_reminder(
    state: State<'_, AppState>,
    note_id: String,
    until: String,
) -> Result<Reminder, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    if chrono::DateTime::parse_from_rfc3339(&until).is_err() {
        return Err(format!("until not a valid RFC3339 timestamp: {until}"));
    }
    let conn = state.db.lock();
    let affected = conn
        .execute(
            "UPDATE reminders
             SET snooze_until = ?1, fired_at = NULL
             WHERE note_id = ?2",
            params![until, note_id],
        )
        .map_err(err)?;
    if affected == 0 {
        return Err(format!("no reminder set for note {note_id}"));
    }
    let r = conn
        .query_row(
            "SELECT note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
             FROM reminders WHERE note_id = ?1",
            params![note_id],
            reminder_from_row,
        )
        .map_err(err)?;
    Ok(r)
}

#[tauri::command]
pub fn clear_reminder(state: State<'_, AppState>, note_id: String) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let conn = state.db.lock();
    conn.execute(
        "DELETE FROM reminders WHERE note_id = ?1",
        params![note_id],
    )
    .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn list_reminders(state: State<'_, AppState>) -> Result<Vec<Reminder>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
             FROM reminders",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], reminder_from_row)
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(rows)
}

/// NF-V0.5-G — write every active (non-fired, non-dismissed) reminder
/// as an iCalendar (RFC 5545) file the user can drop into Google
/// Calendar / Outlook / Apple Calendar. Vault notes export with a
/// generic title so the calendar import doesn't leak the encrypted
/// title; the note id is preserved in the UID so a future re-export can
/// stay deduplicated.
#[tauri::command]
pub fn export_reminders_ics(state: State<'_, AppState>, dest: String) -> Result<String, String> {
    use std::io::Write;
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT r.note_id, r.fire_at, r.rrule, r.snooze_until, \
                    r.created_at, n.title, n.vault \
             FROM reminders r \
             JOIN notes n ON n.id = r.note_id \
             WHERE r.fired_at IS NULL AND r.dismissed_at IS NULL \
             ORDER BY COALESCE(r.snooze_until, r.fire_at)",
        )
        .map_err(err)?;
    let mut rows = stmt.query([]).map_err(err)?;
    let mut count = 0usize;
    let mut ics = String::new();
    ics.push_str("BEGIN:VCALENDAR\r\n");
    ics.push_str("VERSION:2.0\r\n");
    ics.push_str("PRODID:-//Keepr//NF-V0.5-G//EN\r\n");
    ics.push_str("CALSCALE:GREGORIAN\r\n");
    while let Some(row) = rows.next().map_err(err)? {
        let note_id: String = row.get(0).map_err(err)?;
        let fire_at: String = row.get(1).map_err(err)?;
        let rrule: Option<String> = row.get(2).map_err(err)?;
        let snooze_until: Option<String> = row.get(3).map_err(err)?;
        let created_at: String = row.get(4).map_err(err)?;
        let title: String = row.get(5).map_err(err)?;
        let vault: String = row.get(6).map_err(err)?;
        let effective = snooze_until.unwrap_or(fire_at);
        let summary = if vault == "vault" {
            "Keepr — locked vault note".to_string()
        } else if title.is_empty() {
            "Keepr reminder".to_string()
        } else {
            title
        };
        let dtstart = format_ics_utc(&effective)?;
        let dtstamp = format_ics_utc(&created_at)?;
        ics.push_str("BEGIN:VEVENT\r\n");
        ics.push_str(&format!("UID:keepr-{note_id}@keepr.local\r\n"));
        ics.push_str(&format!("DTSTAMP:{dtstamp}\r\n"));
        ics.push_str(&format!("DTSTART:{dtstart}\r\n"));
        ics.push_str(&format!("SUMMARY:{}\r\n", escape_ics(&summary)));
        if let Some(rule) = rrule {
            ics.push_str(&format!("RRULE:{rule}\r\n"));
        }
        ics.push_str("END:VEVENT\r\n");
        count += 1;
    }
    ics.push_str("END:VCALENDAR\r\n");

    let mut f = std::fs::File::create(&dest).map_err(err)?;
    f.write_all(ics.as_bytes()).map_err(err)?;
    f.sync_all().map_err(err)?;
    Ok(format!("{count} reminders exported to {dest}"))
}

/// Convert an RFC 3339 timestamp to the `yyyyMMddTHHmmssZ` form RFC 5545
/// requires for UTC values.
fn format_ics_utc(rfc3339: &str) -> Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map_err(|e| format!("invalid timestamp {rfc3339}: {e}"))?
        .with_timezone(&chrono::Utc);
    Ok(parsed.format("%Y%m%dT%H%M%SZ").to_string())
}

/// Escape ICS-special characters per RFC 5545 §3.3.11. Backslash MUST
/// be replaced first to avoid double-escaping.
fn escape_ics(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace(',', "\\,")
        .replace(';', "\\;")
}

/// Internal — returns pending reminders that should fire now (fire_at <=
/// now AND fired_at IS NULL AND no active snooze). Used by the scheduler
/// thread in lib.rs. PEEK-only: does not write `fired_at`. The scheduler
/// must call `mark_reminder_fired` after `notification.show()` succeeds
/// so a failed toast leaves the reminder pending for retry on the next
/// sweep (EI-V0.5-2 — was the v0.4 "lost-toast" P0).
pub fn peek_due_reminders(
    state: &AppState,
    now_rfc3339: &str,
) -> Result<Vec<(Reminder, String)>, String> {
    // Returns (reminder, note_title) so the scheduler can compose a
    // human-readable notification body.
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT r.note_id, r.fire_at, r.rrule, r.snooze_until,
                    r.fired_at, r.dismissed_at, r.created_at, n.title, n.body
             FROM reminders r
             JOIN notes n ON n.id = r.note_id
             WHERE r.fired_at IS NULL
               AND (r.snooze_until IS NULL OR r.snooze_until <= ?1)
               AND r.fire_at <= ?1
               AND n.trashed = 0",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map(params![now_rfc3339], |row| {
            let r = Reminder {
                note_id: row.get(0)?,
                fire_at: row.get(1)?,
                rrule: row.get(2)?,
                snooze_until: row.get(3)?,
                fired_at: row.get(4)?,
                dismissed_at: row.get(5)?,
                created_at: row.get(6)?,
            };
            let title: String = row.get(7)?;
            let body: String = row.get(8)?;
            let preview = if !title.is_empty() {
                title
            } else if !body.is_empty() {
                body.chars().take(60).collect()
            } else {
                "Untitled note".into()
            };
            Ok((r, preview))
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;
    Ok(rows)
}

/// Mark a reminder as fired. Called by the scheduler after a successful
/// `notification.show()`. If the show failed we deliberately do NOT call
/// this, so the reminder reappears in the next `peek_due_reminders` and
/// retries (EI-V0.5-2).
///
/// NF-V0.5-A — if the reminder has an rrule, this also advances
/// `fire_at` to the next occurrence and clears `fired_at` + `snooze_until`
/// so the recurring reminder re-arms automatically for the next cycle.
pub fn mark_reminder_fired(
    state: &AppState,
    note_id: &str,
    fired_at_rfc3339: &str,
) -> Result<(), String> {
    let conn = state.db.lock();
    // Read the rrule + fire_at so we can decide whether to advance or
    // just mark fired.
    let row: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT fire_at, rrule FROM reminders WHERE note_id = ?1",
            params![note_id],
            |r| {
                let fa: String = r.get(0)?;
                let rr: Option<String> = r.get(1)?;
                Ok((fa, rr))
            },
        )
        .ok();
    if let Some((current_fire_at, rrule)) = row {
        if let Some(next) = next_fire_at(&current_fire_at, rrule.as_deref()) {
            // Advance to next occurrence; leave fired_at NULL.
            conn.execute(
                "UPDATE reminders
                 SET fire_at = ?1, fired_at = NULL, snooze_until = NULL
                 WHERE note_id = ?2",
                params![next, note_id],
            )
            .map_err(err)?;
            return Ok(());
        }
    }
    // Single-shot: just mark fired.
    conn.execute(
        "UPDATE reminders SET fired_at = ?1 WHERE note_id = ?2",
        params![fired_at_rfc3339, note_id],
    )
    .map_err(err)?;
    Ok(())
}

fn reminder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reminder> {
    // Column order matches every "SELECT note_id, fire_at, rrule,
    // snooze_until, fired_at, dismissed_at, created_at FROM reminders"
    // in this module post-v8 schema cleanup (EI-V0.5-14).
    Ok(Reminder {
        note_id: row.get(0)?,
        fire_at: row.get(1)?,
        rrule: row.get(2)?,
        snooze_until: row.get(3)?,
        fired_at: row.get(4)?,
        dismissed_at: row.get(5)?,
        created_at: row.get(6)?,
    })
}

// --- NF-01 attachments ---
//
// File model: bytes live under <data_dir>/resources/<id>.<ext>, served
// to the renderer through the keepr-resource://<id>.<ext> protocol
// (registered in lib.rs). The attachments table holds metadata. We
// resolve the filename suffix from the source file's extension so the
// protocol's content-type whitelist (guess_content_type) picks the
// right MIME.

const RESOURCES_DIR: &str = "resources";

// Mirror of MAX_PER_FILE_BYTES but lower for in-app uploads, matching
// the spirit of Keep's ~10 MB-per-image cap.
const MAX_ATTACHMENT_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB

fn sanitize_extension(src: &Path) -> String {
    // Take at most 8 ASCII letter/digit chars from the extension; default
    // to "bin" if missing/weird. Avoids smuggling odd shell metachars
    // into a filename.
    src.extension()
        .and_then(|s| s.to_str())
        .map(|s| {
            s.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .take(8)
                .collect::<String>()
                .to_ascii_lowercase()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "bin".to_string())
}

fn guess_mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[tauri::command]
pub fn add_image_attachment(
    state: State<'_, AppState>,
    note_id: String,
    src_path: String,
) -> Result<Attachment, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let src = PathBuf::from(&src_path);
    let metadata = std::fs::metadata(&src).map_err(err)?;
    if metadata.len() > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "image exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            metadata.len()
        ));
    }
    let ext = sanitize_extension(&src);
    let mime = guess_mime_for_ext(&ext);
    let original_name = src
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("image")
        .chars()
        .take(255)
        .collect::<String>();

    // Insert metadata first so we know the id; then write the file at
    // <data_dir>/resources/<id>.<ext>. If the write fails, roll back.
    let new_id = Uuid::new_v4().to_string();
    let stored_name = format!("{new_id}.{ext}");
    let resources_dir = state.data_dir.join(RESOURCES_DIR);
    std::fs::create_dir_all(&resources_dir).map_err(err)?;
    let dest = resources_dir.join(&stored_name);

    // Verify the note exists + read the next position in one transaction.
    let mut conn = state.db.lock();
    let now = now_iso();
    let position: i64 = {
        let tx = conn.transaction().map_err(err)?;
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE id = ?1",
                params![note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if exists == 0 {
            return Err(format!("note {note_id} not found"));
        }
        let pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE note_id = ?1",
                params![note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        tx.execute(
            "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at)
             VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id,
                note_id,
                mime,
                original_name,
                metadata.len() as i64,
                pos,
                now,
            ],
        )
        .map_err(err)?;
        // Bump notes.updated_at so the card re-sorts to the top in
        // Modified view.
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        pos
    };
    drop(conn);

    // Now copy the file. If the copy fails we delete the row we just
    // inserted so the DB doesn't reference a missing blob.
    if let Err(copy_err) = std::fs::copy(&src, &dest) {
        let conn = state.db.lock();
        let _ = conn.execute("DELETE FROM attachments WHERE id = ?1", params![new_id]);
        return Err(format!("could not copy attachment: {copy_err}"));
    }

    // NF-V0.5-B — generate a 480-px thumbnail next to the original.
    // Best-effort: a failure here doesn't roll back; the AttachmentTile
    // falls back to the original via onError. Width/height of the source
    // are recorded too so future card layouts can avoid layout shifts.
    let mut width_recorded: Option<i64> = None;
    let mut height_recorded: Option<i64> = None;
    if mime.starts_with("image/") && mime != "image/svg+xml" {
        if let Ok(img) = image::ImageReader::open(&dest).and_then(|r| Ok(r.with_guessed_format()?)) {
            if let Ok(decoded) = img.decode() {
                width_recorded = Some(decoded.width() as i64);
                height_recorded = Some(decoded.height() as i64);
                let thumb = decoded.thumbnail(480, 480);
                let thumb_path = resources_dir.join(format!("{new_id}.thumb.jpg"));
                if let Err(e) = thumb.to_rgb8().save_with_format(
                    &thumb_path,
                    image::ImageFormat::Jpeg,
                ) {
                    eprintln!("keepr: thumbnail generation failed for {new_id}: {e}");
                }
            }
        }
        // Persist measured dimensions back to the attachments row.
        if let (Some(w), Some(h)) = (width_recorded, height_recorded) {
            let conn = state.db.lock();
            let _ = conn.execute(
                "UPDATE attachments SET width = ?1, height = ?2 WHERE id = ?3",
                params![w, h, new_id],
            );
        }
    }

    Ok(Attachment {
        id: new_id,
        note_id,
        kind: "image".into(),
        mime: mime.into(),
        filename: original_name,
        byte_size: metadata.len() as i64,
        width: width_recorded,
        height: height_recorded,
        position,
        created_at: now,
    })
}

/// NF-V0.5-I — companion to add_image_attachment for paste-from-
/// clipboard and drag-drop flows where the renderer has the raw bytes
/// but no on-disk file path. Bytes come over IPC as a Vec<u8> (Tauri
/// serializes via base64). `filename_hint` carries the original name
/// when known (e.g. dropped File.name); otherwise we infer from MIME.
#[tauri::command]
pub fn add_image_attachment_bytes(
    state: State<'_, AppState>,
    note_id: String,
    bytes: Vec<u8>,
    mime: String,
    filename_hint: Option<String>,
) -> Result<Attachment, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "image exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            bytes.len()
        ));
    }
    // Stage bytes in a temp file so add_image_attachment's existing flow
    // (file copy + thumbnail + DB insert + rollback) is reused. The temp
    // file is dropped right after the call regardless of outcome.
    let ext = match mime.as_str() {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        _ => return Err(format!("unsupported mime: {mime}")),
    };
    let tmp = std::env::temp_dir().join(format!(
        "keepr-paste-{}.{ext}",
        Uuid::new_v4()
    ));
    if let Err(e) = std::fs::write(&tmp, &bytes) {
        return Err(format!("could not stage clipboard bytes: {e}"));
    }
    let original_name = filename_hint.unwrap_or_else(|| format!("pasted.{ext}"));
    // Reuse the file-path flow with a faked-but-real path.
    let tmp_path_str = tmp.to_string_lossy().to_string();
    let result = add_image_attachment(state, note_id, tmp_path_str);
    let _ = std::fs::remove_file(&tmp);
    // Override filename on success — add_image_attachment derives it
    // from the temp file's name; for paste we want the hint.
    result.map(|mut a| {
        a.filename = original_name;
        a
    })
}

/// v0.20.3 — audio voice note attachment. The bytes come from a
/// MediaRecorder blob in the renderer (webm/opus or mp4/m4a depending
/// on platform). We bypass `add_image_attachment` because that one
/// runs the bytes through the `image` crate for thumbnail generation,
/// which would fail on audio. Audio attachments don't get thumbnails;
/// the renderer shows an `<audio controls>` element instead.
#[tauri::command]
pub fn add_audio_attachment_bytes(
    state: State<'_, AppState>,
    note_id: String,
    bytes: Vec<u8>,
    mime: String,
    filename_hint: Option<String>,
) -> Result<Attachment, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "audio exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            bytes.len()
        ));
    }
    let ext = match mime.as_str() {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/mp4" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        _ => return Err(format!("unsupported audio mime: {mime}")),
    };
    let new_id = Uuid::new_v4().to_string();
    let stored_name = format!("{new_id}.{ext}");
    let resources_dir = state.data_dir.join(RESOURCES_DIR);
    std::fs::create_dir_all(&resources_dir).map_err(err)?;
    let dest = resources_dir.join(&stored_name);
    let original_name = filename_hint.unwrap_or_else(|| format!("voice-note.{ext}"));

    let mut conn = state.db.lock();
    let now = now_iso();
    let position: i64 = {
        let tx = conn.transaction().map_err(err)?;
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE id = ?1",
                params![note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if exists == 0 {
            return Err(format!("note {note_id} not found"));
        }
        let pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE note_id = ?1",
                params![note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        tx.execute(
            "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at)
             VALUES (?1, ?2, 'audio', ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id,
                note_id,
                mime,
                original_name,
                bytes.len() as i64,
                pos,
                now,
            ],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        pos
    };
    drop(conn);

    if let Err(write_err) = std::fs::write(&dest, &bytes) {
        let conn = state.db.lock();
        let _ = conn.execute("DELETE FROM attachments WHERE id = ?1", params![new_id]);
        return Err(format!("could not write audio blob: {write_err}"));
    }

    Ok(Attachment {
        id: new_id,
        note_id,
        kind: "audio".into(),
        mime,
        filename: original_name,
        byte_size: bytes.len() as i64,
        width: None,
        height: None,
        position,
        created_at: now,
    })
}

#[tauri::command]
pub fn delete_attachment(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let conn = state.db.lock();
    // Read filename suffix so we can delete the blob.
    let (note_id, ext): (String, String) = conn
        .query_row(
            "SELECT note_id, mime FROM attachments WHERE id = ?1",
            params![id],
            |r| {
                let note_id: String = r.get(0)?;
                let mime: String = r.get(1)?;
                let ext = match mime.as_str() {
                    "image/png" => "png",
                    "image/jpeg" => "jpg",
                    "image/gif" => "gif",
                    "image/webp" => "webp",
                    "image/svg+xml" => "svg",
                    "audio/webm" => "webm",
                    "audio/ogg" => "ogg",
                    "audio/mp4" => "m4a",
                    "audio/mpeg" => "mp3",
                    "audio/wav" => "wav",
                    _ => "bin",
                };
                Ok((note_id, ext.to_string()))
            },
        )
        .map_err(|_| format!("attachment {id} not found"))?;
    conn.execute("DELETE FROM attachments WHERE id = ?1", params![id])
        .map_err(err)?;
    // Best-effort: bump updated_at so cards re-sort.
    let now = now_iso();
    let _ = conn.execute(
        "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
        params![now, note_id],
    );
    drop(conn);
    let resources = state.data_dir.join(RESOURCES_DIR);
    let _ = std::fs::remove_file(resources.join(format!("{id}.{ext}")));
    // NF-V0.5-B — also remove the sibling thumbnail if present. Always
    // .jpg regardless of source extension (see add_image_attachment).
    let _ = std::fs::remove_file(resources.join(format!("{id}.thumb.jpg")));
    Ok(())
}

/// NF-05 — drag-reorder support for Custom sort mode. Writes 0..N-1 into
/// `notes.position` according to the order of `ids` so a subsequent
/// `list_notes` sorted by position returns them in this order. Ids not in
/// the current notes table are ignored (e.g. a stale client view).
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

#[tauri::command]
pub fn delete_note_permanent(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM notes WHERE id = ?1", params![id])
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn set_archived(
    state: State<'_, AppState>,
    id: String,
    archived: bool,
) -> Result<(), String> {
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
pub fn set_trashed(
    state: State<'_, AppState>,
    id: String,
    trashed: bool,
) -> Result<(), String> {
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
pub fn list_labels(state: State<'_, AppState>) -> Result<Vec<Label>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare("SELECT id, name FROM labels ORDER BY name COLLATE NOCASE ASC")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(err)?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

#[tauri::command]
pub fn create_label(state: State<'_, AppState>, name: String) -> Result<Label, String> {
    let conn = state.db.lock();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("label name cannot be empty".into());
    }
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO labels (id, name) VALUES (?1, ?2)
         ON CONFLICT(name) DO NOTHING",
        params![id, trimmed],
    )
    .map_err(err)?;
    let mut stmt = conn
        .prepare("SELECT id, name FROM labels WHERE name = ?1 COLLATE NOCASE")
        .map_err(err)?;
    let label = stmt
        .query_row(params![trimmed], |row| {
            Ok(Label {
                id: row.get(0)?,
                name: row.get(1)?,
            })
        })
        .map_err(err)?;
    Ok(label)
}

#[tauri::command]
pub fn rename_label(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<(), String> {
    let conn = state.db.lock();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("label name cannot be empty".into());
    }
    conn.execute("UPDATE labels SET name = ?1 WHERE id = ?2", params![trimmed, id])
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn delete_label(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM labels WHERE id = ?1", params![id])
        .map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn set_note_labels(
    state: State<'_, AppState>,
    note_id: String,
    label_ids: Vec<String>,
) -> Result<(), String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    tx.execute("DELETE FROM note_labels WHERE note_id = ?1", params![note_id])
        .map_err(err)?;
    for lid in label_ids {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![note_id, lid],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn empty_trash(state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM notes WHERE trashed = 1", []).map_err(err)?;
    Ok(())
}

#[tauri::command]
pub fn get_data_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(state.data_dir.to_string_lossy().to_string())
}

/// NF-V0.5-J — return the OS-conventional log directory Tauri's logger
/// is writing into. The renderer surfaces this in Settings so a user
/// reporting a bug can attach the file. Uses the same path resolution
/// `tauri-plugin-log`'s `LogDir` target uses, via `app_log_dir()`.
#[tauri::command]
pub fn get_log_dir(app: tauri::AppHandle) -> Result<String, String> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| format!("could not resolve log dir: {e}"))?;
    Ok(dir.to_string_lossy().to_string())
}

/// Open one of Keepr's own directories (data or log) in the OS file
/// manager. Whitelisted — callers can't pass arbitrary paths — so we
/// don't add a generic `open_path` to the IPC surface. Uses
/// tauri-plugin-opener under the hood; on Windows that's `explorer.exe
/// <path>`, on macOS `open <path>`, on Linux `xdg-open <path>`.
#[tauri::command]
pub fn open_app_dir(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    kind: String,
) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    let path = match kind.as_str() {
        "data" => state.data_dir.clone(),
        "log" => app
            .path()
            .app_log_dir()
            .map_err(|e| format!("could not resolve log dir: {e}"))?,
        other => return Err(format!("unknown app dir kind: {other}")),
    };
    if !path.exists() {
        // Log dir may not exist yet on a fresh install with no logs
        // written. Create it so the explorer window has something to
        // land on rather than failing with "path not found".
        let _ = std::fs::create_dir_all(&path);
    }
    app.opener()
        .open_path(path.to_string_lossy().to_string(), None::<&str>)
        .map_err(|e| format!("could not open path: {e}"))
}

// --- App Lock (NF-V0.5-C) ---------------------------------------------------
//
// Stores the Argon2id PHC string in `app_settings.app_lock_pin_phc` and
// the idle timeout in `app_settings.app_lock_after_minutes`. PHC absence
// (or NULL) means the lock is disabled. Hashing is the slow step
// (~150-300 ms) and runs on the Tauri command worker — we deliberately
// don't spawn_blocking because the renderer is already waiting on the
// invoke promise and async-blocking would just add overhead.

const KEY_APP_LOCK_PHC: &str = "app_lock_pin_phc";
const KEY_APP_LOCK_MINUTES: &str = "app_lock_after_minutes";
const DEFAULT_LOCK_MINUTES: u32 = 5;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppLockSettings {
    pub enabled: bool,
    pub lock_after_minutes: u32,
}

fn read_app_setting(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>, String> {
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

fn write_app_setting(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(err)?;
    Ok(())
}

fn delete_app_setting(conn: &rusqlite::Connection, key: &str) -> Result<(), String> {
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

const KEY_VAULT_SALT: &str = "vault_kdf_salt";
const KEY_VAULT_NONCE: &str = "vault_dek_nonce";
const KEY_VAULT_WRAPPED: &str = "vault_dek_wrapped";
// v0.21.1 — opt-in BIP39 recovery seed envelope. Wraps the SAME DEK,
// just derived from the seed entropy instead of a user password.
const KEY_VAULT_SEED_SALT: &str = "vault_seed_salt";
const KEY_VAULT_SEED_NONCE: &str = "vault_seed_nonce";
const KEY_VAULT_SEED_WRAPPED: &str = "vault_seed_dek_wrapped";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    pub initialized: bool,
    pub unlocked: bool,
}

fn read_vault_material(
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
fn read_vault_seed_material(
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

fn require_unlocked_dek<'a>(
    guard: &'a parking_lot::MutexGuard<'_, Option<crate::vault::Dek>>,
) -> Result<&'a crate::vault::Dek, String> {
    guard
        .as_ref()
        .ok_or_else(|| "vault is locked".to_string())
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
    write_app_setting(&conn, KEY_VAULT_SALT, &crate::vault::to_hex(&init_data.salt))?;
    write_app_setting(&conn, KEY_VAULT_NONCE, &crate::vault::to_hex(&init_data.dek_nonce))?;
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
    write_app_setting(&conn, KEY_VAULT_SALT, &crate::vault::to_hex(&rewrapped.salt))?;
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
    write_app_setting(&conn, KEY_VAULT_SEED_SALT, &crate::vault::to_hex(&envelope.salt))?;
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
        params![KEY_VAULT_SEED_SALT, KEY_VAULT_SEED_NONCE, KEY_VAULT_SEED_WRAPPED],
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
    write_app_setting(&conn, KEY_VAULT_SALT, &crate::vault::to_hex(&rewrapped.salt))?;
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
    get_note(state.clone(), id)
        .and_then(|opt| opt.ok_or_else(|| "note vanished after vaulting".into()))
}

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
fn snapshot_current_note(
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
            let checklist: Vec<ChecklistItem> = serde_json::from_str(&checklist_json)
                .unwrap_or_default();
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
pub fn restore_snapshot(
    state: State<'_, AppState>,
    snapshot_id: String,
) -> Result<Note, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let dek_guard = state.vault_dek.lock();
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let now = now_iso();
    let (
        note_id,
        kind,
        title,
        body,
        color,
        pinned,
        checklist_json,
        snap_vault,
        snap_ciphertext,
    ): (
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
        let items: Vec<ChecklistItem> =
            serde_json::from_str(&checklist_json).unwrap_or_default();
        for item in &items {
            tx.execute(
                "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![item.id, note_id, item.text, item.checked as i64, item.position, item.parent_id],
            )
            .map_err(err)?;
        }
    }
    tx.commit().map_err(err)?;
    drop(conn);
    let conn = state.db.lock();
    let note = load_note_with_vault(&conn, &note_id, dek_guard.as_ref())
        .map_err(err)?
        .ok_or_else(|| "note vanished after restore".to_string())?;
    Ok(note)
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
    let bundle_hex = ciphertext_hex
        .ok_or_else(|| "vault note missing ciphertext".to_string())?;
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
    get_note(state.clone(), id)
        .and_then(|opt| opt.ok_or_else(|| "note vanished after unvaulting".into()))
}

// --- Smart Labels (v0.22.2, D4) ---------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SmartLabel {
    pub id: String,
    pub name: String,
    /// Opaque JSON blob — the renderer parses + applies. Server only
    /// stores it so the schema doesn't need to know the SearchFilters
    /// shape.
    pub query_json: String,
    pub position: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[tauri::command]
pub fn list_smart_labels(state: State<'_, AppState>) -> Result<Vec<SmartLabel>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id, name, query_json, position, created_at, updated_at \
             FROM smart_labels ORDER BY position ASC, created_at ASC",
        )
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SmartLabel {
                id: r.get(0)?,
                name: r.get(1)?,
                query_json: r.get(2)?,
                position: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        })
        .map_err(err)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(err)?);
    }
    Ok(out)
}

#[tauri::command]
pub fn create_smart_label(
    state: State<'_, AppState>,
    name: String,
    query_json: String,
) -> Result<SmartLabel, String> {
    let name = name.trim().to_string();
    if name.is_empty() {
        return Err("name must not be empty".into());
    }
    if name.len() > 80 {
        return Err("name too long (max 80 chars)".into());
    }
    if query_json.len() > 4096 {
        return Err("query payload too large".into());
    }
    let conn = state.db.lock();
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    let position: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(position) + 1, 0) FROM smart_labels",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    conn.execute(
        "INSERT INTO smart_labels (id, name, query_json, position, created_at, updated_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
        params![id, name, query_json, position, now],
    )
    .map_err(err)?;
    Ok(SmartLabel {
        id,
        name,
        query_json,
        position,
        created_at: now.clone(),
        updated_at: now,
    })
}

#[tauri::command]
pub fn update_smart_label(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    query_json: Option<String>,
) -> Result<SmartLabel, String> {
    let conn = state.db.lock();
    let now = now_iso();
    if let Some(n) = &name {
        let trimmed = n.trim();
        if trimmed.is_empty() {
            return Err("name must not be empty".into());
        }
        conn.execute(
            "UPDATE smart_labels SET name = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, trimmed, now],
        )
        .map_err(err)?;
    }
    if let Some(q) = &query_json {
        if q.len() > 4096 {
            return Err("query payload too large".into());
        }
        conn.execute(
            "UPDATE smart_labels SET query_json = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, q, now],
        )
        .map_err(err)?;
    }
    conn.query_row(
        "SELECT id, name, query_json, position, created_at, updated_at \
         FROM smart_labels WHERE id = ?1",
        params![id],
        |r| {
            Ok(SmartLabel {
                id: r.get(0)?,
                name: r.get(1)?,
                query_json: r.get(2)?,
                position: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        },
    )
    .map_err(err)
}

#[tauri::command]
pub fn delete_smart_label(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute("DELETE FROM smart_labels WHERE id = ?1", params![id])
        .map_err(err)?;
    Ok(())
}

/// v0.21.0 — prune auto-backup ZIPs in a folder, keeping the latest
/// `keep` by filename order. Filenames are `keepr-autobackup-<ISO>.zip`
/// so a lexical sort is equivalent to chronological. Only files
/// matching that prefix are considered — other files in the folder are
/// left alone. Returns the count deleted.
#[tauri::command]
pub fn prune_auto_backups(folder: String, keep: u32) -> Result<u32, String> {
    if keep == 0 {
        return Ok(0);
    }
    let path = PathBuf::from(&folder);
    if !path.is_dir() {
        return Err(format!("not a directory: {folder}"));
    }
    let mut ours: Vec<PathBuf> = std::fs::read_dir(&path)
        .map_err(err)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|s| s.to_str())
                .map(|n| n.starts_with("keepr-autobackup-") && n.ends_with(".zip"))
                .unwrap_or(false)
        })
        .collect();
    ours.sort(); // ISO timestamp filenames sort chronologically.
    let keep = keep as usize;
    if ours.len() <= keep {
        return Ok(0);
    }
    let prune_count = ours.len() - keep;
    let mut deleted: u32 = 0;
    for p in ours.iter().take(prune_count) {
        if std::fs::remove_file(p).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

/// Bulk-vault wrapper for NF-V0.5-C. Calls `move_note_to_vault` per
/// id in sequence; stops + returns on first failure (so partial state
/// is acceptable — each per-note call commits its own transaction
/// already). Returns the count successfully moved. v0.20.2.
#[tauri::command]
pub fn move_notes_to_vault(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> Result<u32, String> {
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

// --- Backup / restore -------------------------------------------------------
//
// EI-01: Validate every zip entry before writing it. Even though zip's
//   `enclosed_name()` already protects against `..` traversal and absolute
//   paths, we double-check that the resolved write path stays under the
//   staging directory after canonicalization. Also cap entry count and
//   total uncompressed size so a zip-bomb can't fill the disk.
// EI-02: Run `PRAGMA wal_checkpoint(TRUNCATE)` before zipping so committed
//   WAL pages land in keepr.db, and fsync the zip file before reporting
//   success.
// EI-03: Snapshot the live keepr.db to keepr.db.prev before swap; restore
//   from .prev on any error after the swap; reject parallel mutating
//   commands while import is in progress via AppState.importing.

const MAX_ENTRY_COUNT: usize = 10_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB
const MAX_PER_FILE_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

/// Validate a candidate restore archive against EI-01's caps and EI-01's
/// path-safety rules without writing anything to disk. Pure function so it
/// can be unit tested.
fn validate_zip_archive<R: std::io::Read + std::io::Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<(), String> {
    if archive.len() > MAX_ENTRY_COUNT {
        return Err(format!(
            "backup has {} entries (max {})",
            archive.len(),
            MAX_ENTRY_COUNT
        ));
    }
    let mut total_uncompressed: u64 = 0;
    for i in 0..archive.len() {
        let f = archive.by_index(i).map_err(err)?;
        if f.enclosed_name().is_none() {
            return Err(format!("backup entry '{}' has an unsafe path", f.name()));
        }
        if f.size() > MAX_PER_FILE_BYTES {
            return Err(format!(
                "backup entry '{}' exceeds {} bytes",
                f.name(),
                MAX_PER_FILE_BYTES
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(f.size());
        if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err(format!(
                "backup uncompressed size exceeds {} bytes",
                MAX_UNCOMPRESSED_BYTES
            ));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn export_zip(
    state: State<'_, AppState>,
    dest: String,
) -> Result<String, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let data_dir: PathBuf = state.data_dir.clone();
    let dest_path = PathBuf::from(&dest);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }

    // EI-02: flush WAL into the main DB before zipping (otherwise recent
    // committed writes are silently absent from the backup).
    {
        let conn = state.db.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(err)?;
    }

    let file = File::create(&dest_path).map_err(err)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // EI-V0.5-13 — mirror the import-side caps on the export so a user
    // with > 2 GiB of data doesn't write a backup they can never restore.
    let mut total_uncompressed: u64 = 0;
    let mut entry_count: usize = 0;

    for entry in walkdir::WalkDir::new(&data_dir)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path
            .strip_prefix(&data_dir)
            .map_err(err)?
            .to_string_lossy()
            .replace('\\', "/");
        // Skip SQLite sidecars (covered by the WAL checkpoint above) and our
        // own backup sentinels.
        if name.ends_with("-journal")
            || name.ends_with("-wal")
            || name.ends_with("-shm")
            || name.ends_with(".prev")
            || name.starts_with("__restore_tmp/")
        {
            continue;
        }
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size > MAX_PER_FILE_BYTES {
            return Err(format!(
                "backup entry '{name}' would exceed {} bytes — delete the attachment first",
                MAX_PER_FILE_BYTES
            ));
        }
        total_uncompressed = total_uncompressed.saturating_add(size);
        if total_uncompressed > MAX_UNCOMPRESSED_BYTES {
            return Err(format!(
                "backup would exceed {} uncompressed bytes — delete some attachments first",
                MAX_UNCOMPRESSED_BYTES
            ));
        }
        entry_count += 1;
        if entry_count > MAX_ENTRY_COUNT {
            return Err(format!(
                "backup would contain more than {} entries",
                MAX_ENTRY_COUNT
            ));
        }
        zip.start_file(name, opts).map_err(err)?;
        // EI-V0.5-13 — stream the file into the zip instead of loading
        // it into a Vec<u8> first. Saves the per-file RAM spike on
        // multi-MiB attachments.
        let mut f = File::open(path).map_err(err)?;
        std::io::copy(&mut f, &mut zip).map_err(err)?;
    }

    // EI-02: fsync the zip so a crash within milliseconds of the success
    // toast doesn't leave a zero-byte or truncated backup on disk.
    let file = zip.finish().map_err(err)?;
    file.sync_all().map_err(err)?;
    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn import_zip(
    state: State<'_, AppState>,
    src: String,
) -> Result<(), String> {
    // EI-03 — busy gate. swap() returns the previous value; if it was already
    // true, another import is in flight and we must refuse rather than race.
    if state
        .importing
        .swap(true, std::sync::atomic::Ordering::SeqCst)
    {
        return Err("a restore is already in progress".into());
    }
    // Always clear the gate on any exit path.
    let _gate = ImportGate {
        flag: state.importing.clone(),
    };

    do_import_zip(&state, &src)
}

struct ImportGate {
    flag: Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for ImportGate {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn do_import_zip(state: &State<'_, AppState>, src: &str) -> Result<(), String> {
    let data_dir: PathBuf = state.data_dir.clone();
    std::fs::create_dir_all(&data_dir).map_err(err)?;
    let staging = data_dir.join("__restore_tmp");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(err)?;
    }
    std::fs::create_dir_all(&staging).map_err(err)?;
    // Canonical staging dir for the under-prefix check below.
    let staging_canon = std::fs::canonicalize(&staging).map_err(err)?;

    let file = File::open(src).map_err(err)?;
    let mut archive = zip::ZipArchive::new(file).map_err(err)?;
    validate_zip_archive(&mut archive)?;

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(err)?;
        // enclosed_name returns None for absolute paths or any path with
        // `..` traversal — this is the first line of zip-slip defense.
        let safe = match f.enclosed_name() {
            Some(p) => p,
            None => return Err(format!("backup entry '{}' has an unsafe path", f.name())),
        };
        let outpath = staging.join(&safe);

        // Belt-and-braces: ensure the resolved write path still sits under
        // the staging directory after the join. We canonicalize the *parent*
        // (which exists once we create it) rather than the file (which
        // doesn't yet) to do the prefix check.
        if let Some(parent) = outpath.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
            let parent_canon = std::fs::canonicalize(parent).map_err(err)?;
            if !parent_canon.starts_with(&staging_canon) {
                return Err(format!(
                    "backup entry '{}' resolves outside staging directory",
                    f.name()
                ));
            }
        }

        if f.is_dir() {
            std::fs::create_dir_all(&outpath).map_err(err)?;
        } else {
            let mut out = File::create(&outpath).map_err(err)?;
            std::io::copy(&mut f, &mut out).map_err(err)?;
        }
    }

    // The archive must contain keepr.db at the root (matches what export_zip
    // writes). Reject otherwise.
    let restored_db = staging.join("keepr.db");
    if !restored_db.exists() {
        let _ = std::fs::remove_dir_all(&staging);
        return Err("backup is missing keepr.db".into());
    }

    let target_db = data_dir.join("keepr.db");
    let prev_db = data_dir.join("keepr.db.prev");

    // --- EI-03 safe swap ---
    let mut conn_guard = state.db.lock();

    // Step 1: drop the live connection so we can move the file out from
    // under it. Use a throwaway in-memory connection only until we either
    // succeed (replaced with the new DB) or fail (we restore from .prev
    // before unlocking, so no caller ever observes the throwaway).
    let throwaway = rusqlite::Connection::open_in_memory().map_err(err)?;
    let _old = std::mem::replace(&mut *conn_guard, throwaway);
    drop(_old);

    // Remove stale WAL/SHM/journal sidecars left over from previous opens.
    for sidecar in ["keepr.db-journal", "keepr.db-wal", "keepr.db-shm"] {
        let _ = std::fs::remove_file(data_dir.join(sidecar));
    }

    // Step 2: snapshot the current DB to .prev so we can restore on failure.
    let had_prior_db = target_db.exists();
    if had_prior_db {
        // remove any stale .prev from a previous failed import
        let _ = std::fs::remove_file(&prev_db);
        std::fs::rename(&target_db, &prev_db).map_err(err)?;
    }

    // Step 3: install the restored DB. Helper so we can unify error
    // recovery — on any failure between here and the successful open() we
    // restore from .prev and bail.
    let install_then_open = || -> Result<rusqlite::Connection, String> {
        std::fs::copy(&restored_db, &target_db).map_err(err)?;
        crate::db::open(&target_db).map_err(err)
    };

    match install_then_open() {
        Ok(new_conn) => {
            *conn_guard = new_conn;
            // Successful — drop the .prev snapshot and the staging dir.
            let _ = std::fs::remove_file(&prev_db);
            let _ = std::fs::remove_dir_all(&staging);
            Ok(())
        }
        Err(install_err) => {
            // Roll back. Best-effort — if even the rollback fails we leave
            // the in-memory throwaway in place so the next operation errors
            // loudly rather than silently writing to memory.
            let _ = std::fs::remove_file(&target_db);
            if had_prior_db {
                if let Err(rename_err) = std::fs::rename(&prev_db, &target_db) {
                    return Err(format!(
                        "restore failed ({install_err}); rollback also failed ({rename_err}); \
                         your previous database is at {}",
                        prev_db.display()
                    ));
                }
                match crate::db::open(&target_db) {
                    Ok(restored_conn) => {
                        *conn_guard = restored_conn;
                    }
                    Err(reopen_err) => {
                        return Err(format!(
                            "restore failed ({install_err}); rolled back to previous DB but \
                             could not reopen it ({reopen_err})"
                        ));
                    }
                }
            }
            let _ = std::fs::remove_dir_all(&staging);
            Err(install_err)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Cursor, Write as _};
    use zip::write::SimpleFileOptions;

    fn build_zip<F: FnOnce(&mut zip::ZipWriter<Cursor<Vec<u8>>>)>(build: F) -> Vec<u8> {
        let buf = Cursor::new(Vec::<u8>::new());
        let mut zw = zip::ZipWriter::new(buf);
        build(&mut zw);
        zw.finish().unwrap().into_inner()
    }

    #[test]
    fn validate_accepts_a_normal_backup() {
        let bytes = build_zip(|zw| {
            let opts = SimpleFileOptions::default();
            zw.start_file("keepr.db", opts).unwrap();
            zw.write_all(b"SQLite format 3\0").unwrap();
            zw.start_file("resources/abc.png", opts).unwrap();
            zw.write_all(b"PNGDATA").unwrap();
        });
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        validate_zip_archive(&mut archive).expect("should accept normal backup");
    }

    #[test]
    fn validate_rejects_too_many_entries() {
        let bytes = build_zip(|zw| {
            let opts = SimpleFileOptions::default();
            for i in 0..(MAX_ENTRY_COUNT + 1) {
                zw.start_file(format!("entry-{i}"), opts).unwrap();
            }
        });
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        let err = validate_zip_archive(&mut archive).unwrap_err();
        assert!(err.contains("max"), "got: {err}");
    }

    #[test]
    fn validate_rejects_path_traversal() {
        // A zip with `..\..\evil.txt` cannot be created via start_file's
        // sanitization, but raw zip parsers will accept it. We construct
        // such a malicious zip by hand.
        let mut raw = Vec::<u8>::new();
        // Use the zip crate's raw API: start_file_from_path with a literal
        // name that includes `..`. zip-rs will pass it through; the validator
        // must catch it via enclosed_name().
        let mut zw = zip::ZipWriter::new(Cursor::new(&mut raw));
        let opts = SimpleFileOptions::default();
        // The crate sanitizes via mangle on read, so enclosed_name will be
        // None for `../escape.txt` because it resolves outside the root.
        zw.start_file("../escape.txt", opts).unwrap();
        zw.write_all(b"hi").unwrap();
        zw.finish().unwrap();

        let mut archive = zip::ZipArchive::new(Cursor::new(raw)).unwrap();
        let err = validate_zip_archive(&mut archive).unwrap_err();
        assert!(err.contains("unsafe path"), "got: {err}");
    }

    // --- Pure helpers ---

    #[test]
    fn sanitize_extension_handles_uppercase_and_specials() {
        use std::path::Path;
        assert_eq!(sanitize_extension(Path::new("foo.PNG")), "png");
        assert_eq!(sanitize_extension(Path::new("foo.JPEG")), "jpeg");
        // No extension → default
        assert_eq!(sanitize_extension(Path::new("README")), "bin");
        // Non-alphanumeric characters dropped
        assert_eq!(sanitize_extension(Path::new("foo.t!@#xt")), "txt");
        // Truncated at 8 chars
        assert_eq!(
            sanitize_extension(Path::new("foo.abcdefghij")),
            "abcdefgh"
        );
    }

    #[test]
    fn sanitize_vault_filename_strips_unsafe_chars() {
        assert_eq!(
            sanitize_vault_filename("Hello / World: <test>", "abc12345"),
            "Hello - World- -test-",
        );
        // Pure-unsafe input that collapses to nothing falls back to
        // "note-<short id>". (Slashes/asterisks become dashes, not
        // empty — so we need actual nothings: empty input.)
        assert_eq!(
            sanitize_vault_filename("", "abc12345-rest-of-uuid"),
            "note-abc12345",
        );
        // Trims leading dots/spaces
        assert_eq!(
            sanitize_vault_filename("  .hidden  ", "xyz"),
            "hidden",
        );
    }

    #[test]
    fn yaml_quote_if_needed_quotes_special() {
        assert_eq!(yaml_quote_if_needed("safe"), "safe");
        assert_eq!(yaml_quote_if_needed("with: colon"), "\"with: colon\"");
        assert_eq!(yaml_quote_if_needed("- starts-with-dash"), "\"- starts-with-dash\"");
        assert_eq!(yaml_quote_if_needed(""), "\"\"");
        // Backslash + quote escape
        assert_eq!(yaml_quote_if_needed("she said \"hi\""), "\"she said \\\"hi\\\"\"");
    }

    #[test]
    fn map_keep_color_covers_full_enum() {
        for (k, v) in [
            ("RED", "red"),
            ("ORANGE", "orange"),
            ("YELLOW", "yellow"),
            ("GREEN", "green"),
            ("TEAL", "teal"),
            ("BLUE", "blue"),
            ("DARK_BLUE", "darkblue"),
            ("PURPLE", "purple"),
            ("PINK", "pink"),
            ("BROWN", "brown"),
            ("GRAY", "gray"),
            ("UNKNOWN", "default"),
            ("", "default"),
        ] {
            assert_eq!(map_keep_color(k), v, "for {k}");
        }
    }

    #[test]
    fn takeout_usec_to_rfc3339_round_trips() {
        // 2024-01-01T00:00:00Z = 1704067200 seconds = 1704067200_000_000 µs
        let usec = serde_json::json!(1704067200u64 * 1_000_000);
        let out = takeout_usec_to_rfc3339(Some(&usec)).unwrap();
        assert!(out.starts_with("2024-01-01T00:00:00"), "got: {out}");
        // Missing input → None
        assert_eq!(takeout_usec_to_rfc3339(None), None);
        // Non-number → None
        assert_eq!(
            takeout_usec_to_rfc3339(Some(&serde_json::json!("not a number"))),
            None
        );
    }

    #[test]
    fn takeout_reminder_fire_at_handles_multiple_shapes() {
        let fire_on = serde_json::json!({ "fireOn": "2024-06-15T08:00:00Z" });
        assert_eq!(
            takeout_reminder_fire_at(&fire_on),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let snake = serde_json::json!({ "fire_on": "2024-06-15T08:00:00Z" });
        assert_eq!(
            takeout_reminder_fire_at(&snake),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let usec = serde_json::json!({
            "reminderTimeUsec": 1718438400u64 * 1_000_000u64,
        });
        let result = takeout_reminder_fire_at(&usec).unwrap();
        assert!(result.starts_with("2024-06-15"), "got: {result}");
        let nested = serde_json::json!({
            "time": { "formattedDate": "2024-06-15T08:00:00Z" }
        });
        assert_eq!(
            takeout_reminder_fire_at(&nested),
            Some("2024-06-15T08:00:00Z".to_string())
        );
        let empty = serde_json::json!({});
        assert_eq!(takeout_reminder_fire_at(&empty), None);
        // Garbage timestamp → None
        let garbage = serde_json::json!({ "fireOn": "not-a-date" });
        assert_eq!(takeout_reminder_fire_at(&garbage), None);
    }

    #[test]
    fn is_keep_note_shape_accepts_canonical_takeout_note() {
        // Exact field set seen in a 2026 Google Takeout (Keep-only export).
        let note = serde_json::json!({
            "color": "DEFAULT",
            "isTrashed": false,
            "isPinned": true,
            "isArchived": false,
            "textContent": "Hello world",
            "title": "Test",
            "userEditedTimestampUsec": 1704067200000000u64,
            "createdTimestampUsec": 1704067200000000u64,
            "textContentHtml": "<p>Hello world</p>",
        });
        assert!(is_keep_note_shape(&note));
    }

    #[test]
    fn is_keep_note_shape_accepts_list_only_note() {
        // Checklist note without textContent — still a Keep note.
        let list = serde_json::json!({
            "isPinned": false,
            "isTrashed": false,
            "listContent": [{"text": "buy milk", "isChecked": false}],
            "createdTimestampUsec": 1u64,
        });
        assert!(is_keep_note_shape(&list));
    }

    #[test]
    fn is_keep_note_shape_rejects_takeout_labels_array() {
        // Takeout's `Labels.json` is a top-level array, not an object.
        let labels = serde_json::json!([{"name": "Work"}, {"name": "Personal"}]);
        assert!(!is_keep_note_shape(&labels));
    }

    #[test]
    fn is_keep_note_shape_rejects_other_product_json() {
        // Drive/Photos metadata in a multi-product Takeout — has no
        // `isPinned`, no Keep timestamps, no Keep content fields.
        let drive = serde_json::json!({
            "name": "Some doc",
            "lastModifiedTime": "2024-01-01T00:00:00Z",
            "mimeType": "application/pdf",
        });
        assert!(!is_keep_note_shape(&drive));
    }

    #[test]
    fn is_keep_note_shape_rejects_partial_match() {
        // Has `isPinned` but no content + no timestamps — not a note.
        let partial = serde_json::json!({"isPinned": false});
        assert!(!is_keep_note_shape(&partial));
        // Has content but no `isPinned` — also rejected.
        let no_pinned = serde_json::json!({
            "textContent": "x",
            "createdTimestampUsec": 1u64,
        });
        assert!(!is_keep_note_shape(&no_pinned));
    }

    #[test]
    fn guess_mime_for_ext_handles_known_and_unknown() {
        assert_eq!(guess_mime_for_ext("png"), "image/png");
        assert_eq!(guess_mime_for_ext("jpg"), "image/jpeg");
        assert_eq!(guess_mime_for_ext("jpeg"), "image/jpeg");
        assert_eq!(guess_mime_for_ext("gif"), "image/gif");
        assert_eq!(guess_mime_for_ext("webp"), "image/webp");
        assert_eq!(guess_mime_for_ext("svg"), "image/svg+xml");
        assert_eq!(guess_mime_for_ext("unknown"), "application/octet-stream");
    }

    // --- Direct-AppState integration tests ---
    //
    // These construct an AppState manually with an in-memory SQLite
    // connection so we can call commands' inner logic without going
    // through Tauri's State extractor. The commands wrapped in
    // #[tauri::command] still take State<'_, AppState>, so we duplicate
    // the body of the smaller ones into test-local helpers.

    fn test_state() -> AppState {
        use parking_lot::Mutex;
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("test.db");
        let conn = crate::db::open(&db_path).unwrap();
        // Leak the tempdir so it lives for the test's lifetime; we never
        // delete it explicitly. Test processes are short-lived; OS cleans
        // up %TEMP% eventually.
        let data_dir = tmp.into_path();
        AppState {
            db: Arc::new(Mutex::new(conn)),
            importing: Arc::new(AtomicBool::new(false)),
            data_dir,
            vault_dek: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }

    fn insert_test_note(state: &AppState, id: &str, title: &str) {
        let conn = state.db.lock();
        let now = "2026-01-01T00:00:00Z";
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, position, created_at, updated_at)
             VALUES (?1, 'text', ?2, '', 'default', 0, 0, 0, 0, ?3, ?3)",
            params![id, title, now],
        )
        .unwrap();
    }

    #[test]
    fn peek_due_reminders_returns_only_pending_and_due() {
        let state = test_state();
        insert_test_note(&state, "n1", "due note");
        insert_test_note(&state, "n2", "future note");
        insert_test_note(&state, "n3", "fired note");
        let now = "2026-05-26T12:00:00Z";
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n2', '2026-05-26T13:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, fired_at, created_at) VALUES ('n3', '2026-05-26T11:00:00Z', ?1, ?1)",
            params![now],
        ).unwrap();
        drop(conn);
        let due = peek_due_reminders(&state, now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0.note_id, "n1");
        assert_eq!(due[0].1, "due note");
    }

    #[test]
    fn peek_due_reminders_excludes_trashed_notes() {
        let state = test_state();
        insert_test_note(&state, "n_trash", "trashed");
        let conn = state.db.lock();
        conn.execute(
            "UPDATE notes SET trashed = 1 WHERE id = 'n_trash'",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n_trash', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        let due = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        assert_eq!(due.len(), 0);
    }

    #[test]
    fn mark_reminder_fired_sets_the_column() {
        let state = test_state();
        insert_test_note(&state, "n1", "x");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T12:00:00Z").unwrap();
        let conn = state.db.lock();
        let fired: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fired.as_deref(), Some("2026-05-26T12:00:00Z"));
    }

    #[test]
    fn peek_does_not_write_fired_at() {
        // EI-V0.5-2 regression test: peek_due_reminders must NEVER mark
        // fired_at; only mark_reminder_fired does. Otherwise a failed
        // notification permanently loses the reminder.
        let state = test_state();
        insert_test_note(&state, "n1", "x");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let _ = peek_due_reminders(&state, "2026-05-26T12:00:00Z").unwrap();
        let conn = state.db.lock();
        let fired: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fired.is_none(), "fired_at should still be NULL after peek");
    }

    #[test]
    fn format_ics_utc_rounds_offsets_to_zulu() {
        let out = format_ics_utc("2026-05-26T08:00:00-04:00").unwrap();
        assert_eq!(out, "20260526T120000Z");
    }

    #[test]
    fn build_fts5_query_quotes_and_prefixes_each_token() {
        // Plain text → phrase-quote per token, prefix wildcard, joined.
        assert_eq!(build_fts5_query("milk"), "\"milk\"*");
        assert_eq!(build_fts5_query("buy milk"), "\"buy\"* \"milk\"*");
        // Empty / whitespace-only → empty.
        assert_eq!(build_fts5_query(""), "");
        assert_eq!(build_fts5_query("   "), "");
        // FTS5-meaningful chars survive because the whole token is
        // wrapped in double quotes; embedded quotes are escaped.
        assert_eq!(build_fts5_query("foo(bar)"), "\"foo(bar)\"*");
        assert_eq!(build_fts5_query("ab\"cd"), "\"ab\"\"cd\"*");
        // AND / OR / NEAR — FTS5 keywords. Quoting neutralizes them.
        assert_eq!(build_fts5_query("milk OR eggs"), "\"milk\"* \"OR\"* \"eggs\"*");
    }

    #[test]
    fn escape_ics_handles_special_characters() {
        // Backslash first so the order matters.
        let out = escape_ics("a\\b,c;d\ne");
        assert_eq!(out, "a\\\\b\\,c\\;d\\ne");
    }

    #[test]
    fn next_fire_at_handles_supported_rrules() {
        // Daily
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=DAILY"))
                .unwrap()
                .starts_with("2026-05-27T08:00:00"),
            true,
        );
        // Weekly
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=WEEKLY"))
                .unwrap()
                .starts_with("2026-06-02T08:00:00"),
            true,
        );
        // Monthly
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=MONTHLY"))
                .unwrap()
                .starts_with("2026-06-26T08:00:00"),
            true,
        );
        // Yearly (with leap-day clamp)
        assert!(next_fire_at("2024-02-29T08:00:00+00:00", Some("FREQ=YEARLY"))
            .unwrap()
            .starts_with("2025-02-28"));
        // None for single-shot
        assert_eq!(next_fire_at("2026-05-26T08:00:00+00:00", None), None);
        // Unsupported rrule
        assert_eq!(
            next_fire_at("2026-05-26T08:00:00+00:00", Some("FREQ=HOURLY")),
            None
        );
    }

    #[test]
    fn validate_rrule_rejects_unknown() {
        assert!(validate_rrule(None).is_ok());
        assert!(validate_rrule(Some("FREQ=DAILY")).is_ok());
        assert!(validate_rrule(Some("FREQ=WEEKLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=MONTHLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=YEARLY")).is_ok());
        assert!(validate_rrule(Some("FREQ=HOURLY")).is_err());
        assert!(validate_rrule(Some("garbage")).is_err());
    }

    #[test]
    fn mark_reminder_fired_advances_recurring() {
        // NF-V0.5-A regression test — recurring reminders re-arm to the
        // next occurrence after a successful fire; fired_at stays NULL
        // so the row is still pending in the next sweep.
        let state = test_state();
        insert_test_note(&state, "n1", "daily standup");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, rrule, created_at) VALUES ('n1', '2026-05-26T08:00:00+00:00', 'FREQ=DAILY', '2026-05-26T08:00:00+00:00')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T08:05:00+00:00").unwrap();
        let conn = state.db.lock();
        let (fire_at, fired_at): (String, Option<String>) = conn
            .query_row(
                "SELECT fire_at, fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert!(fire_at.starts_with("2026-05-27T08:00:00"), "got fire_at: {fire_at}");
        assert!(fired_at.is_none(), "fired_at should be NULL after recurring advance");
    }

    #[test]
    fn mark_reminder_fired_single_shot_sets_fired_at() {
        let state = test_state();
        insert_test_note(&state, "n1", "single");
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '2026-05-26T08:00:00+00:00', '2026-05-26T08:00:00+00:00')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "n1", "2026-05-26T08:05:00+00:00").unwrap();
        let conn = state.db.lock();
        let fired_at: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE note_id = 'n1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fired_at.is_some(), "fired_at should be set for single-shot");
    }

    #[test]
    fn migration_v4_backfills_positions() {
        // EI-V0.5-1 regression test: every note after a fresh migrate
        // should have a unique position (0..N-1).
        let state = test_state();
        insert_test_note(&state, "a", "first");
        insert_test_note(&state, "b", "second");
        insert_test_note(&state, "c", "third");
        // Mutate updated_at so the ROW_NUMBER OVER (ORDER BY updated_at DESC) sees
        // a deterministic order.
        let conn = state.db.lock();
        conn.execute("UPDATE notes SET updated_at = '2026-05-01' WHERE id = 'a'", []).unwrap();
        conn.execute("UPDATE notes SET updated_at = '2026-05-03' WHERE id = 'b'", []).unwrap();
        conn.execute("UPDATE notes SET updated_at = '2026-05-02' WHERE id = 'c'", []).unwrap();
        // Re-run the v4 migration body directly.
        conn.execute_batch(
            "WITH ordered AS (
                SELECT id,
                       ROW_NUMBER() OVER (ORDER BY pinned DESC, updated_at DESC) - 1 AS rn
                FROM notes
            )
            UPDATE notes
            SET position = (SELECT rn FROM ordered WHERE ordered.id = notes.id);",
        )
        .unwrap();
        let mut stmt = conn
            .prepare("SELECT id, position FROM notes ORDER BY position ASC")
            .unwrap();
        let rows: Vec<(String, i64)> = stmt
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].1, 0);
        assert_eq!(rows[1].1, 1);
        assert_eq!(rows[2].1, 2);
        // First (most recent) should be b.
        assert_eq!(rows[0].0, "b");
    }
}
