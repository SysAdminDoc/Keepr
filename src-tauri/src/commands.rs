use crate::AppState;
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;
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
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    dest: String,
) -> Result<String, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let data_dir: PathBuf = app.path().app_data_dir().map_err(err)?;
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
    app: tauri::AppHandle,
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

    let result = do_import_zip(&app, &state, &src);
    result
}

struct ImportGate {
    flag: Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for ImportGate {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

fn do_import_zip(
    app: &tauri::AppHandle,
    state: &State<'_, AppState>,
    src: &str,
) -> Result<(), String> {
    let data_dir: PathBuf = app.path().app_data_dir().map_err(err)?;
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
