use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::commands::{Attachment, ChecklistItem, Label, Note};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateVector {
    pub device_id: String,
    pub notes: HashMap<String, String>,
    pub tombstones: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileRequest {
    pub state_vector: StateVector,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReconcileResponse {
    pub pull: Vec<SyncNote>,
    pub push_ids: Vec<String>,
    pub labels: Vec<Label>,
    pub tombstones: Vec<Tombstone>,
    pub attachment_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncNote {
    pub note: Note,
    pub labels_names: Vec<String>,
    pub reminder: Option<SyncReminder>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncReminder {
    pub fire_at: String,
    pub rrule: Option<String>,
    pub snooze_until: Option<String>,
    pub fired_at: Option<String>,
    pub dismissed_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tombstone {
    pub note_id: String,
    pub deleted_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PushRequest {
    pub notes: Vec<SyncNote>,
    pub labels: Vec<Label>,
}

pub fn build_state_vector(conn: &Connection, device_id: &str) -> Result<StateVector, String> {
    let mut notes = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT id, updated_at FROM notes")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, updated_at) = row.map_err(|e| e.to_string())?;
        notes.insert(id, updated_at);
    }
    let mut tombstones = HashMap::new();
    let mut stmt = conn
        .prepare("SELECT note_id, deleted_at FROM sync_tombstones")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))
        .map_err(|e| e.to_string())?;
    for row in rows {
        let (id, deleted_at) = row.map_err(|e| e.to_string())?;
        tombstones.insert(id, deleted_at);
    }
    Ok(StateVector {
        device_id: device_id.to_string(),
        notes,
        tombstones,
    })
}

pub fn reconcile(
    conn: &Connection,
    resources_dir: &Path,
    local_device_id: &str,
    remote: &StateVector,
) -> Result<ReconcileResponse, String> {
    let local = build_state_vector(conn, local_device_id)?;
    let mut pull = Vec::new();
    let mut push_ids = Vec::new();

    for (note_id, local_updated) in &local.notes {
        if remote.tombstones.get(note_id).is_some_and(|d| d > local_updated) {
            continue;
        }
        match remote.notes.get(note_id) {
            Some(remote_updated) if remote_updated >= local_updated => {}
            _ => {
                if let Ok(Some(sn)) = load_sync_note(conn, note_id) {
                    if sn.note.vault != "vault" {
                        pull.push(sn);
                    }
                }
            }
        }
    }

    for (note_id, remote_updated) in &remote.notes {
        if local.tombstones.get(note_id).is_some_and(|d| d > remote_updated) {
            continue;
        }
        match local.notes.get(note_id) {
            Some(local_updated) if local_updated >= remote_updated => {}
            _ => {
                push_ids.push(note_id.clone());
            }
        }
    }

    let labels = load_all_labels(conn)?;
    let tombstones: Vec<Tombstone> = local
        .tombstones
        .into_iter()
        .map(|(note_id, deleted_at)| Tombstone {
            note_id,
            deleted_at,
        })
        .collect();

    let attachment_hashes = list_attachment_hashes(conn, resources_dir)?;

    Ok(ReconcileResponse {
        pull,
        push_ids,
        labels,
        tombstones,
        attachment_hashes,
    })
}

pub fn apply_pushed_notes(
    conn: &Connection,
    notes: &[SyncNote],
    labels: &[Label],
) -> Result<(usize, usize), String> {
    let mut notes_applied = 0;
    let mut labels_merged = 0;

    for label in labels {
        let existing: Option<String> = conn
            .query_row(
                "SELECT id FROM labels WHERE name = ?1 COLLATE NOCASE",
                params![label.name],
                |r| r.get(0),
            )
            .ok();
        if existing.is_none() {
            conn.execute(
                "INSERT OR IGNORE INTO labels (id, name) VALUES (?1, ?2)",
                params![label.id, label.name],
            )
            .map_err(|e| e.to_string())?;
            labels_merged += 1;
        }
    }

    for sn in notes {
        let n = &sn.note;
        if n.vault == "vault" {
            continue;
        }
        let existing_updated: Option<String> = conn
            .query_row(
                "SELECT updated_at FROM notes WHERE id = ?1",
                params![n.id],
                |r| r.get(0),
            )
            .ok();
        if existing_updated.as_ref().is_some_and(|eu| eu >= &n.updated_at) {
            continue;
        }
        let local_vault: Option<String> = conn
            .query_row(
                "SELECT vault FROM notes WHERE id = ?1",
                params![n.id],
                |r| r.get(0),
            )
            .ok();
        if local_vault.as_deref() == Some("vault") {
            continue;
        }

        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
             position, created_at, updated_at, trashed_at, vault, background_pattern) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, 'plain', ?13) \
             ON CONFLICT(id) DO UPDATE SET \
             kind=excluded.kind, title=excluded.title, body=excluded.body, \
             color=excluded.color, pinned=excluded.pinned, archived=excluded.archived, \
             trashed=excluded.trashed, position=excluded.position, \
             updated_at=excluded.updated_at, trashed_at=excluded.trashed_at, \
             background_pattern=excluded.background_pattern",
            params![
                n.id,
                n.kind,
                n.title,
                n.body,
                n.color,
                n.pinned,
                n.archived,
                n.trashed,
                n.position,
                n.created_at,
                n.updated_at,
                n.trashed_at,
                n.background_pattern,
            ],
        )
        .map_err(|e| e.to_string())?;

        conn.execute(
            "DELETE FROM checklist_items WHERE note_id = ?1",
            params![n.id],
        )
        .map_err(|e| e.to_string())?;
        for ci in &n.checklist {
            conn.execute(
                "INSERT INTO checklist_items (id, note_id, text, checked, position, parent_id) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![ci.id, n.id, ci.text, ci.checked, ci.position, ci.parent_id],
            )
            .map_err(|e| e.to_string())?;
        }

        conn.execute(
            "DELETE FROM note_labels WHERE note_id = ?1",
            params![n.id],
        )
        .map_err(|e| e.to_string())?;
        for label_name in &sn.labels_names {
            let label_id: Option<String> = conn
                .query_row(
                    "SELECT id FROM labels WHERE name = ?1 COLLATE NOCASE",
                    params![label_name],
                    |r| r.get(0),
                )
                .ok();
            let lid = match label_id {
                Some(id) => id,
                None => {
                    let new_id = uuid::Uuid::new_v4().to_string();
                    conn.execute(
                        "INSERT OR IGNORE INTO labels (id, name) VALUES (?1, ?2)",
                        params![new_id, label_name],
                    )
                    .map_err(|e| e.to_string())?;
                    new_id
                }
            };
            conn.execute(
                "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
                params![n.id, lid],
            )
            .map_err(|e| e.to_string())?;
        }

        if let Some(rem) = &sn.reminder {
            conn.execute(
                "INSERT INTO reminders (note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) \
                 ON CONFLICT(note_id) DO UPDATE SET \
                 fire_at=excluded.fire_at, rrule=excluded.rrule, \
                 snooze_until=excluded.snooze_until, fired_at=excluded.fired_at, \
                 dismissed_at=excluded.dismissed_at",
                params![
                    n.id,
                    rem.fire_at,
                    rem.rrule,
                    rem.snooze_until,
                    rem.fired_at,
                    rem.dismissed_at,
                    rem.created_at,
                ],
            )
            .map_err(|e| e.to_string())?;
        }

        notes_applied += 1;
    }

    Ok((notes_applied, labels_merged))
}

pub fn apply_tombstones(conn: &Connection, tombstones: &[Tombstone]) -> Result<usize, String> {
    let mut deleted = 0;
    for t in tombstones {
        let exists: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE id = ?1",
                params![t.note_id],
                |r| r.get::<_, i64>(0).map(|c| c > 0),
            )
            .unwrap_or(false);
        if !exists {
            conn.execute(
                "INSERT OR REPLACE INTO sync_tombstones (note_id, deleted_at) VALUES (?1, ?2)",
                params![t.note_id, t.deleted_at],
            )
            .map_err(|e| e.to_string())?;
            continue;
        }
        let updated_at: String = conn
            .query_row(
                "SELECT updated_at FROM notes WHERE id = ?1",
                params![t.note_id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if t.deleted_at > updated_at {
            conn.execute("DELETE FROM notes WHERE id = ?1", params![t.note_id])
                .map_err(|e| e.to_string())?;
            conn.execute(
                "INSERT OR REPLACE INTO sync_tombstones (note_id, deleted_at) VALUES (?1, ?2)",
                params![t.note_id, t.deleted_at],
            )
            .map_err(|e| e.to_string())?;
            deleted += 1;
        }
    }
    Ok(deleted)
}

pub fn record_tombstone(conn: &Connection, note_id: &str, deleted_at: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO sync_tombstones (note_id, deleted_at) VALUES (?1, ?2)",
        params![note_id, deleted_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn purge_old_tombstones(conn: &Connection, cutoff: &str) -> Result<usize, String> {
    let deleted = conn
        .execute(
            "DELETE FROM sync_tombstones WHERE deleted_at < ?1",
            params![cutoff],
        )
        .map_err(|e| e.to_string())?;
    Ok(deleted)
}

pub fn is_safe_sync_resource_path(s: &str) -> bool {
    if s.is_empty() || s.len() > 256 {
        return false;
    }
    if s.contains('\0') || s.contains('\\') || s.contains("..") || s.contains('%') {
        return false;
    }
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 3 {
        return false;
    }
    if parts[0].len() != 2
        || parts[1].len() != 2
        || !parts[0].chars().all(|c| c.is_ascii_hexdigit())
        || !parts[1].chars().all(|c| c.is_ascii_hexdigit())
    {
        return false;
    }
    let filename = parts[2];
    let stem = filename.split('.').next().unwrap_or("");
    stem.len() == 64 && stem.chars().all(|c| c.is_ascii_hexdigit())
}

fn load_sync_note(conn: &Connection, note_id: &str) -> Result<Option<SyncNote>, String> {
    let note = match load_note_row(conn, note_id)? {
        Some(n) => n,
        None => return Ok(None),
    };

    let labels_names: Vec<String> = conn
        .prepare(
            "SELECT l.name FROM labels l \
             JOIN note_labels nl ON nl.label_id = l.id \
             WHERE nl.note_id = ?1",
        )
        .map_err(|e| e.to_string())?
        .query_map(params![note_id], |r| r.get(0))
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let reminder = conn
        .query_row(
            "SELECT fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at \
             FROM reminders WHERE note_id = ?1",
            params![note_id],
            |r| {
                Ok(SyncReminder {
                    fire_at: r.get(0)?,
                    rrule: r.get(1)?,
                    snooze_until: r.get(2)?,
                    fired_at: r.get(3)?,
                    dismissed_at: r.get(4)?,
                    created_at: r.get(5)?,
                })
            },
        )
        .ok();

    Ok(Some(SyncNote {
        note,
        labels_names,
        reminder,
    }))
}

fn load_note_row(conn: &Connection, note_id: &str) -> Result<Option<Note>, String> {
    let note: Option<Note> = conn
        .query_row(
            "SELECT id, kind, title, body, color, pinned, archived, trashed, \
             position, created_at, updated_at, trashed_at, vault, background_pattern \
             FROM notes WHERE id = ?1",
            params![note_id],
            |r| {
                Ok(Note {
                    id: r.get(0)?,
                    kind: r.get(1)?,
                    title: r.get(2)?,
                    body: r.get(3)?,
                    color: r.get(4)?,
                    pinned: r.get(5)?,
                    archived: r.get(6)?,
                    trashed: r.get(7)?,
                    position: r.get(8)?,
                    created_at: r.get(9)?,
                    updated_at: r.get(10)?,
                    trashed_at: r.get(11)?,
                    checklist: Vec::new(),
                    labels: Vec::new(),
                    attachments: Vec::new(),
                    vault_attachment_count: 0,
                    vault: r.get(12)?,
                    background_pattern: r.get(13)?,
                })
            },
        )
        .ok();

    let mut note = match note {
        Some(n) => n,
        None => return Ok(None),
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, text, checked, position, parent_id \
             FROM checklist_items WHERE note_id = ?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    note.checklist = stmt
        .query_map(params![note_id], |r| {
            Ok(ChecklistItem {
                id: r.get(0)?,
                text: r.get(1)?,
                checked: r.get(2)?,
                position: r.get(3)?,
                parent_id: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut stmt = conn
        .prepare(
            "SELECT id, note_id, kind, mime, filename, byte_size, width, height, \
             position, created_at, resource_path, thumb_path \
             FROM attachments WHERE note_id = ?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    note.attachments = stmt
        .query_map(params![note_id], |r| {
            Ok(Attachment {
                id: r.get(0)?,
                note_id: r.get(1)?,
                kind: r.get(2)?,
                mime: r.get(3)?,
                filename: r.get(4)?,
                byte_size: r.get(5)?,
                width: r.get(6)?,
                height: r.get(7)?,
                position: r.get(8)?,
                created_at: r.get(9)?,
                resource_path: r.get(10)?,
                thumb_path: r.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    Ok(Some(note))
}

fn load_all_labels(conn: &Connection) -> Result<Vec<Label>, String> {
    conn.prepare("SELECT id, name FROM labels")
        .map_err(|e| e.to_string())?
        .query_map([], |r| {
            Ok(Label {
                id: r.get(0)?,
                name: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

fn list_attachment_hashes(conn: &Connection, _resources_dir: &Path) -> Result<Vec<String>, String> {
    conn.prepare("SELECT DISTINCT resource_path FROM attachments WHERE resource_path IS NOT NULL")
        .map_err(|e| e.to_string())?
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn test_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        db::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn state_vector_empty_db() {
        let conn = test_conn();
        let sv = build_state_vector(&conn, "dev1").unwrap();
        assert!(sv.notes.is_empty());
        assert!(sv.tombstones.is_empty());
        assert_eq!(sv.device_id, "dev1");
    }

    #[test]
    fn state_vector_with_notes() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
             position, created_at, updated_at) \
             VALUES ('n1', 'text', 'Hello', '', 'default', 0, 0, 0, 0, \
             '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
            [],
        )
        .unwrap();
        let sv = build_state_vector(&conn, "dev1").unwrap();
        assert_eq!(sv.notes.len(), 1);
        assert_eq!(sv.notes.get("n1").unwrap(), "2026-01-02T00:00:00Z");
    }

    #[test]
    fn tombstone_lifecycle() {
        let conn = test_conn();
        record_tombstone(&conn, "n1", "2026-07-01T00:00:00Z").unwrap();
        let sv = build_state_vector(&conn, "dev1").unwrap();
        assert_eq!(sv.tombstones.len(), 1);
        purge_old_tombstones(&conn, "2026-07-02T00:00:00Z").unwrap();
        let sv = build_state_vector(&conn, "dev1").unwrap();
        assert!(sv.tombstones.is_empty());
    }

    #[test]
    fn apply_new_note_from_push() {
        let conn = test_conn();
        let note = Note {
            id: "n1".into(),
            kind: "text".into(),
            title: "Remote".into(),
            body: "content".into(),
            color: "default".into(),
            pinned: false,
            archived: false,
            trashed: false,
            position: 0,
            created_at: "2026-01-01T00:00:00Z".into(),
            updated_at: "2026-01-02T00:00:00Z".into(),
            trashed_at: None,
            checklist: vec![],
            labels: vec![],
            attachments: vec![],
            vault_attachment_count: 0,
            vault: "plain".into(),
            background_pattern: String::new(),
        };
        let sn = SyncNote {
            note,
            labels_names: vec!["work".into()],
            reminder: None,
        };
        let (applied, labels) = apply_pushed_notes(&conn, &[sn], &[]).unwrap();
        assert_eq!(applied, 1);
        assert_eq!(labels, 0);
        let title: String = conn
            .query_row("SELECT title FROM notes WHERE id = 'n1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(title, "Remote");
    }

    #[test]
    fn tombstone_deletes_older_note() {
        let conn = test_conn();
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
             position, created_at, updated_at) \
             VALUES ('n1', 'text', 'Old', '', 'default', 0, 0, 0, 0, \
             '2026-01-01T00:00:00Z', '2026-01-02T00:00:00Z')",
            [],
        )
        .unwrap();
        let ts = vec![Tombstone {
            note_id: "n1".into(),
            deleted_at: "2026-01-03T00:00:00Z".into(),
        }];
        let deleted = apply_tombstones(&conn, &ts).unwrap();
        assert_eq!(deleted, 1);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM notes WHERE id = 'n1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(count, 0);
    }
}
