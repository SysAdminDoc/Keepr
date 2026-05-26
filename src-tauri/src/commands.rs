use crate::AppState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use tauri::{Manager, State};
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
        .filter_map(|r| r.ok())
        .collect();
    note.checklist = items;
    let mut lstmt =
        conn.prepare("SELECT label_id FROM note_labels WHERE note_id = ?1")?;
    let labels: Vec<String> = lstmt
        .query_map(params![id], |row| row.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    note.labels = labels;
    Ok(Some(note))
}

#[tauri::command]
pub fn list_notes(state: State<'_, AppState>) -> Result<Vec<Note>, String> {
    let conn = state.db.lock();
    let mut stmt = conn
        .prepare(
            "SELECT id FROM notes
             ORDER BY pinned DESC, updated_at DESC",
        )
        .map_err(err)?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(err)?
        .filter_map(|r| r.ok())
        .collect();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(n) = load_note(&conn, &id).map_err(err)? {
            out.push(n);
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn get_note(state: State<'_, AppState>, id: String) -> Result<Option<Note>, String> {
    let conn = state.db.lock();
    load_note(&conn, &id).map_err(err)
}

#[tauri::command]
pub fn create_note(state: State<'_, AppState>, input: NoteInput) -> Result<Note, String> {
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
    }
    for label_id in &input.labels {
        tx.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            params![id, label_id],
        )
        .map_err(err)?;
    }
    tx.commit().map_err(err)?;
    load_note(&conn, &id)
        .map_err(err)?
        .ok_or_else(|| "note vanished after insert".into())
}

#[tauri::command]
pub fn update_note(
    state: State<'_, AppState>,
    id: String,
    input: NoteInput,
) -> Result<Note, String> {
    let mut conn = state.db.lock();
    let tx = conn.transaction().map_err(err)?;
    let now = now_iso();
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
    load_note(&conn, &id)
        .map_err(err)?
        .ok_or_else(|| "note vanished after update".into())
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
pub fn get_data_dir(app: tauri::AppHandle) -> Result<String, String> {
    app.path()
        .app_data_dir()
        .map(|p| p.to_string_lossy().to_string())
        .map_err(err)
}

#[tauri::command]
pub fn export_zip(app: tauri::AppHandle, dest: String) -> Result<String, String> {
    let data_dir: PathBuf = app.path().app_data_dir().map_err(err)?;
    let dest_path = PathBuf::from(&dest);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    let file = File::create(&dest_path).map_err(err)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts: SimpleFileOptions =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    for entry in walkdir::WalkDir::new(&data_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_file() {
            let name = path
                .strip_prefix(&data_dir)
                .map_err(err)?
                .to_string_lossy()
                .replace('\\', "/");
            // skip lock/journal/wal sidecars
            if name.ends_with("-journal") || name.ends_with("-wal") || name.ends_with("-shm") {
                continue;
            }
            zip.start_file(name, opts).map_err(err)?;
            let mut f = File::open(path).map_err(err)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf).map_err(err)?;
            zip.write_all(&buf).map_err(err)?;
        }
    }
    zip.finish().map_err(err)?;
    Ok(dest_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn import_zip(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    src: String,
) -> Result<(), String> {
    let data_dir: PathBuf = app.path().app_data_dir().map_err(err)?;
    std::fs::create_dir_all(&data_dir).map_err(err)?;

    // close + drop the existing connection by replacing it after restore
    let file = File::open(&src).map_err(err)?;
    let mut archive = zip::ZipArchive::new(file).map_err(err)?;
    let staging = data_dir.join("__restore_tmp");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(err)?;
    }
    std::fs::create_dir_all(&staging).map_err(err)?;

    for i in 0..archive.len() {
        let mut f = archive.by_index(i).map_err(err)?;
        let outpath = match f.enclosed_name() {
            Some(p) => staging.join(p),
            None => continue,
        };
        if f.is_dir() {
            std::fs::create_dir_all(&outpath).map_err(err)?;
        } else {
            if let Some(parent) = outpath.parent() {
                std::fs::create_dir_all(parent).map_err(err)?;
            }
            let mut out = File::create(&outpath).map_err(err)?;
            std::io::copy(&mut f, &mut out).map_err(err)?;
        }
    }

    // swap: replace keepr.db (and any attachments dir) with restored copies
    let restored_db = staging.join("keepr.db");
    if !restored_db.exists() {
        std::fs::remove_dir_all(&staging).ok();
        return Err("backup missing keepr.db".into());
    }
    let target_db = data_dir.join("keepr.db");

    // drop & reopen connection around the swap
    {
        let mut conn_guard = state.db.lock();
        // close current conn by replacing with a throwaway in-memory one
        let throwaway = rusqlite::Connection::open_in_memory().map_err(err)?;
        let _old = std::mem::replace(&mut *conn_guard, throwaway);
        // _old is dropped here -> file handle released

        // remove WAL/SHM sidecars before overwriting
        for ext in ["-journal", "-wal", "-shm"] {
            let side = target_db.with_extension(format!("db{}", ext));
            let _ = std::fs::remove_file(side);
        }
        std::fs::copy(&restored_db, &target_db).map_err(err)?;

        // reopen the real DB
        let new_conn = crate::db::open(&target_db).map_err(err)?;
        *conn_guard = new_conn;
    }

    std::fs::remove_dir_all(&staging).ok();
    Ok(())
}
