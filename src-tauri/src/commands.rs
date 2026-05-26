use crate::AppState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;
use zip::write::SimpleFileOptions;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChecklistItem {
    pub id: String,
    pub text: String,
    pub checked: bool,
    pub position: i64,
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
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Label {
    pub id: String,
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NoteInput {
    pub kind: String,
    pub title: String,
    pub body: String,
    pub color: String,
    pub pinned: bool,
    pub checklist: Vec<ChecklistItemInput>,
    pub labels: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChecklistItemInput {
    pub id: Option<String>,
    pub text: String,
    pub checked: bool,
    pub position: i64,
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
    Ok(())
}

fn load_note(conn: &Connection, id: &str) -> Result<Option<Note>, rusqlite::Error> {
    let mut stmt = conn.prepare(
        "SELECT id, kind, title, body, color, pinned, archived, trashed, position,
                created_at, updated_at, trashed_at
         FROM notes WHERE id = ?1",
    )?;
    let note_opt = stmt
        .query_row(params![id], |row| {
            Ok(Note {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                color: row.get(4)?,
                pinned: row.get::<_, i64>(5)? != 0,
                archived: row.get::<_, i64>(6)? != 0,
                trashed: row.get::<_, i64>(7)? != 0,
                position: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                trashed_at: row.get(11)?,
                checklist: vec![],
                labels: vec![],
                attachments: vec![],
            })
        })
        .optional()?;
    let Some(mut note) = note_opt else {
        return Ok(None);
    };
    let mut cstmt = conn.prepare(
        "SELECT id, text, checked, position FROM checklist_items
         WHERE note_id = ?1 ORDER BY position ASC, rowid ASC",
    )?;
    let items = cstmt
        .query_map(params![id], |row| {
            Ok(ChecklistItem {
                id: row.get(0)?,
                text: row.get(1)?,
                checked: row.get::<_, i64>(2)? != 0,
                position: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    note.checklist = items;
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
                    created_at, updated_at, trashed_at
             FROM notes
             ORDER BY pinned DESC, updated_at DESC",
        )
        .map_err(err)?;
    let mut notes: Vec<Note> = nstmt
        .query_map([], |row| {
            Ok(Note {
                id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                body: row.get(3)?,
                color: row.get(4)?,
                pinned: row.get::<_, i64>(5)? != 0,
                archived: row.get::<_, i64>(6)? != 0,
                trashed: row.get::<_, i64>(7)? != 0,
                position: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
                trashed_at: row.get(11)?,
                checklist: Vec::new(),
                labels: Vec::new(),
                attachments: Vec::new(),
            })
        })
        .map_err(err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(err)?;

    // Build an id -> Vec index for in-place stitching.
    use std::collections::HashMap;
    let mut idx: HashMap<String, usize> = HashMap::with_capacity(notes.len());
    for (i, n) in notes.iter().enumerate() {
        idx.insert(n.id.clone(), i);
    }

    let mut cstmt = conn
        .prepare(
            "SELECT note_id, id, text, checked, position
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
    let conn = state.db.lock();
    load_note(&conn, &id).map_err(err)
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
            "INSERT INTO checklist_items (id, note_id, text, checked, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item_id, id, item.text, item.checked as i64, item.position],
        )
        .map_err(err)?;
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
        });
    }
    for label_id in &input.labels {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![id, label_id],
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
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let now = now_iso();
    // Read created_at + archived/trashed flags so the returned Note
    // accurately reflects what's on disk (we don't change those fields here).
    let (created_at, archived, trashed, trashed_at, position): (String, i64, i64, Option<String>, i64) =
        tx.query_row(
            "SELECT created_at, archived, trashed, trashed_at, position FROM notes WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .map_err(|_| format!("note {id} not found"))?;
    tx.execute(
        "UPDATE notes
           SET kind = ?1, title = ?2, body = ?3, color = ?4, pinned = ?5, updated_at = ?6
         WHERE id = ?7",
        params![
            input.kind,
            input.title,
            input.body,
            input.color,
            input.pinned as i64,
            now,
            id,
        ],
    )
    .map_err(err)?;
    tx.execute("DELETE FROM checklist_items WHERE note_id = ?1", params![id])
        .map_err(err)?;
    let mut checklist_out: Vec<ChecklistItem> = Vec::with_capacity(input.checklist.len());
    for item in &input.checklist {
        let item_id = item
            .id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        tx.execute(
            "INSERT INTO checklist_items (id, note_id, text, checked, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item_id, id, item.text, item.checked as i64, item.position],
        )
        .map_err(err)?;
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
        });
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
    let mut checklist_out: Vec<ChecklistItem> = Vec::with_capacity(source.checklist.len());
    for item in &source.checklist {
        let item_id = Uuid::new_v4().to_string();
        tx.execute(
            "INSERT INTO checklist_items (id, note_id, text, checked, position)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![item_id, new_id, item.text, item.checked as i64, item.position],
        )
        .map_err(err)?;
        checklist_out.push(ChecklistItem {
            id: item_id,
            text: item.text.clone(),
            checked: item.checked,
            position: item.position,
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

    Ok(Attachment {
        id: new_id,
        note_id,
        kind: "image".into(),
        mime: mime.into(),
        filename: original_name,
        byte_size: metadata.len() as i64,
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
    let path = state.data_dir.join(RESOURCES_DIR).join(format!("{id}.{ext}"));
    let _ = std::fs::remove_file(path);
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
        zip.start_file(name, opts).map_err(err)?;
        let mut f = File::open(path).map_err(err)?;
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(err)?;
        zip.write_all(&buf).map_err(err)?;
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
}
