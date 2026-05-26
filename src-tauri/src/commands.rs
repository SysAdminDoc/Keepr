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
/// where each note lives at `Takeout/Keep/<title>.json` (+ a sibling
/// HTML rendering we ignore + binary attachments alongside the JSON).
/// We iterate every `.json` entry, parse with serde_json's untyped
/// `Value` to be forgiving of schema drift, and call the existing
/// `create_note` path to insert.
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
        if name.ends_with(".json") && name.contains("/Keep/") {
            let mut text = String::new();
            entry
                .read_to_string(&mut text)
                .map_err(err)?;
            // Folder for resolving sibling attachments.
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
                    let dest = resources_dir.join(&stored_name);
                    if std::fs::write(&dest, bytes).is_err() {
                        continue;
                    }
                    let now = now_iso();
                    let conn = state.db.lock();
                    let _ = conn.execute(
                        "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at)
                         VALUES (?1, ?2, 'image', ?3, ?4, ?5, 0, ?6)",
                        params![new_id, created.id, mime, rel, bytes.len() as i64, now],
                    );
                }
            }
        }

        imported += 1;
    }
    Ok(imported)
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
    pub id: String,
    pub note_id: String,
    pub fire_at: String,
    pub rrule: Option<String>,
    pub snooze_until: Option<String>,
    pub fired_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub created_at: String,
}

#[tauri::command]
pub fn set_reminder(
    state: State<'_, AppState>,
    note_id: String,
    fire_at: String,
) -> Result<Reminder, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    // Basic validation: fire_at must be parseable RFC3339.
    if chrono::DateTime::parse_from_rfc3339(&fire_at).is_err() {
        return Err(format!("fire_at not a valid RFC3339 timestamp: {fire_at}"));
    }
    let conn = state.db.lock();
    let id = Uuid::new_v4().to_string();
    let now = now_iso();
    // UPSERT keyed on the UNIQUE note_id — replacing an existing
    // reminder rather than appending.
    conn.execute(
        "INSERT INTO reminders (id, note_id, fire_at, created_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(note_id) DO UPDATE SET
            fire_at = excluded.fire_at,
            fired_at = NULL,
            dismissed_at = NULL,
            snooze_until = NULL",
        params![id, note_id, fire_at, now],
    )
    .map_err(err)?;
    let r = conn
        .query_row(
            "SELECT id, note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
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
            "SELECT id, note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
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
            "SELECT r.id, r.note_id, r.fire_at, r.rrule, r.snooze_until,
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
                id: row.get(0)?,
                note_id: row.get(1)?,
                fire_at: row.get(2)?,
                rrule: row.get(3)?,
                snooze_until: row.get(4)?,
                fired_at: row.get(5)?,
                dismissed_at: row.get(6)?,
                created_at: row.get(7)?,
            };
            let title: String = row.get(8)?;
            let body: String = row.get(9)?;
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
pub fn mark_reminder_fired(
    state: &AppState,
    reminder_id: &str,
    fired_at_rfc3339: &str,
) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "UPDATE reminders SET fired_at = ?1 WHERE id = ?2",
        params![fired_at_rfc3339, reminder_id],
    )
    .map_err(err)?;
    Ok(())
}

fn reminder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reminder> {
    Ok(Reminder {
        id: row.get(0)?,
        note_id: row.get(1)?,
        fire_at: row.get(2)?,
        rrule: row.get(3)?,
        snooze_until: row.get(4)?,
        fired_at: row.get(5)?,
        dismissed_at: row.get(6)?,
        created_at: row.get(7)?,
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
            "INSERT INTO reminders (id, note_id, fire_at, created_at) VALUES ('r1', 'n1', '2026-05-26T11:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (id, note_id, fire_at, created_at) VALUES ('r2', 'n2', '2026-05-26T13:00:00Z', ?1)",
            params![now],
        ).unwrap();
        conn.execute(
            "INSERT INTO reminders (id, note_id, fire_at, fired_at, created_at) VALUES ('r3', 'n3', '2026-05-26T11:00:00Z', ?1, ?1)",
            params![now],
        ).unwrap();
        drop(conn);
        let due = peek_due_reminders(&state, now).unwrap();
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].0.id, "r1");
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
            "INSERT INTO reminders (id, note_id, fire_at, created_at) VALUES ('r1', 'n_trash', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
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
            "INSERT INTO reminders (id, note_id, fire_at, created_at) VALUES ('r1', 'n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
            [],
        )
        .unwrap();
        drop(conn);
        mark_reminder_fired(&state, "r1", "2026-05-26T12:00:00Z").unwrap();
        let conn = state.db.lock();
        let fired: Option<String> = conn
            .query_row(
                "SELECT fired_at FROM reminders WHERE id = 'r1'",
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
            "INSERT INTO reminders (id, note_id, fire_at, created_at) VALUES ('r1', 'n1', '2026-05-26T11:00:00Z', '2026-05-26T11:00:00Z')",
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
                "SELECT fired_at FROM reminders WHERE id = 'r1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(fired.is_none(), "fired_at should still be NULL after peek");
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
