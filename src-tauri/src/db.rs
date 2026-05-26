use anyhow::{bail, Result};
use rusqlite::Connection;
use std::path::Path;

/// Current schema version. Bump and add a new arm to `apply_migration` for every
/// schema change. Migrations are forward-only and ordered.
pub const SCHEMA_VERSION: i32 = 4;

pub fn open(path: &Path) -> Result<Connection> {
    let mut conn = Connection::open(path)?;
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA foreign_keys = ON;
         PRAGMA synchronous = NORMAL;",
    )?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Read the current `user_version` pragma, then apply any pending migrations
/// up to `SCHEMA_VERSION` inside a single transaction. Idempotent.
pub fn migrate(conn: &mut Connection) -> Result<()> {
    let current: i32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if current > SCHEMA_VERSION {
        bail!(
            "database is at schema v{} but this binary only knows up to v{}; \
             please upgrade Keepr or restore an older backup",
            current,
            SCHEMA_VERSION
        );
    }
    if current == SCHEMA_VERSION {
        return Ok(());
    }

    let tx = conn.transaction()?;
    for v in (current + 1)..=SCHEMA_VERSION {
        apply_migration(&tx, v)?;
    }
    // `PRAGMA user_version` cannot be parameterized; SCHEMA_VERSION is a hard-coded const.
    tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    tx.commit()?;
    Ok(())
}

fn apply_migration(tx: &rusqlite::Transaction, version: i32) -> Result<()> {
    match version {
        1 => tx.execute_batch(MIGRATION_V1)?,
        2 => tx.execute_batch(MIGRATION_V2)?,
        3 => tx.execute_batch(MIGRATION_V3)?,
        4 => tx.execute_batch(MIGRATION_V4)?,
        v => bail!("no migration defined for schema v{v}"),
    }
    Ok(())
}

/// v1 — initial schema (notes, checklist_items, labels, note_labels).
const MIGRATION_V1: &str = r#"
CREATE TABLE IF NOT EXISTS notes (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL DEFAULT 'text' CHECK (kind IN ('text','list')),
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'default',
    pinned INTEGER NOT NULL DEFAULT 0,
    archived INTEGER NOT NULL DEFAULT 0,
    trashed INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    trashed_at TEXT
);
CREATE INDEX IF NOT EXISTS idx_notes_state
    ON notes(archived, trashed, pinned, updated_at DESC);

CREATE TABLE IF NOT EXISTS checklist_items (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    text TEXT NOT NULL DEFAULT '',
    checked INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX IF NOT EXISTS idx_checklist_note ON checklist_items(note_id);

CREATE TABLE IF NOT EXISTS labels (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE COLLATE NOCASE
);

CREATE TABLE IF NOT EXISTS note_labels (
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
    PRIMARY KEY (note_id, label_id)
);
CREATE INDEX IF NOT EXISTS idx_note_labels_label ON note_labels(label_id);
"#;

/// v2 — attachments table + state index for SQL-side filtering.
/// `idx_notes_state` is already created in v1's `CREATE INDEX IF NOT EXISTS`,
/// so this migration only adds the attachments table.
const MIGRATION_V2: &str = r#"
CREATE TABLE IF NOT EXISTS attachments (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL DEFAULT 'image' CHECK (kind IN ('image','drawing','audio','file')),
    mime TEXT NOT NULL DEFAULT '',
    filename TEXT NOT NULL DEFAULT '',
    byte_size INTEGER NOT NULL DEFAULT 0,
    width INTEGER,
    height INTEGER,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_attachments_note ON attachments(note_id);
"#;

/// v3 — reminders table (NF-02). One pending reminder per note. RRULE
/// is reserved for v0.5+ recurring reminders; v0.4 single-shot only.
const MIGRATION_V3: &str = r#"
CREATE TABLE IF NOT EXISTS reminders (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL UNIQUE REFERENCES notes(id) ON DELETE CASCADE,
    fire_at TEXT NOT NULL,        -- ISO 8601, UTC
    rrule TEXT,                    -- RFC 5545 (unused in v0.4)
    snooze_until TEXT,             -- ISO 8601, UTC
    fired_at TEXT,                 -- ISO 8601, UTC (NULL = pending)
    dismissed_at TEXT,             -- ISO 8601, UTC
    created_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_reminders_pending
    ON reminders(fired_at, fire_at)
    WHERE fired_at IS NULL;
"#;

/// v4 — position backfill (EI-V0.5-1). The `notes.position` column has
/// existed since v1 but was unused until v0.3's Custom sort. Users
/// upgrading from v1/v2/v3 have `position = 0` on every note, so
/// switching into Custom sort shows them in tie-break-by-updated_at
/// order — feels random. This migration assigns an initial position
/// reflecting the current sort default (Modified DESC), so Custom-sort
/// behaves as "start from where you are, then drag from there."
const MIGRATION_V4: &str = r#"
WITH ordered AS (
    SELECT id,
           ROW_NUMBER() OVER (ORDER BY pinned DESC, updated_at DESC) - 1 AS rn
    FROM notes
)
UPDATE notes
SET position = (SELECT rn FROM ordered WHERE ordered.id = notes.id)
WHERE position = 0;
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_conn() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn migrate_from_scratch_lands_on_latest() {
        let conn = fresh_conn();
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        migrate(&mut conn).unwrap();
        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_upgrades_v1_database_to_latest() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        // Simulate a v1-shaped database: apply v1 migration, set user_version = 1
        // explicitly, then call `migrate` and expect it to climb to SCHEMA_VERSION.
        let tx = conn.transaction().unwrap();
        tx.execute_batch(MIGRATION_V1).unwrap();
        tx.execute_batch("PRAGMA user_version = 1").unwrap();
        tx.commit().unwrap();

        // Before: attachments table should not exist.
        let count_attachments: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='attachments'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_attachments, 0);

        migrate(&mut conn).unwrap();

        let v: i32 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, SCHEMA_VERSION);
        let count_attachments: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='attachments'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count_attachments, 1);
    }

    #[test]
    fn migrate_rejects_future_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        // Pretend the file came from a newer Keepr.
        conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 5))
            .unwrap();
        let err = migrate(&mut conn).unwrap_err();
        assert!(err.to_string().contains("upgrade Keepr"));
    }

    #[test]
    fn wal_and_fks_actually_on_via_open() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path();
        let conn = open(path).unwrap();
        let journal_mode: String = conn
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(journal_mode.to_lowercase(), "wal");
        let fk: i32 = conn
            .query_row("PRAGMA foreign_keys", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fk, 1);
    }
}
