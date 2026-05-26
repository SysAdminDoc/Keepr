use anyhow::{bail, Result};
use rusqlite::Connection;
use std::path::Path;

/// Current schema version. Bump and add a new arm to `apply_migration` for every
/// schema change. Migrations are forward-only and ordered.
pub const SCHEMA_VERSION: i32 = 8;

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
        5 => tx.execute_batch(MIGRATION_V5)?,
        6 => tx.execute_batch(MIGRATION_V6)?,
        7 => tx.execute_batch(MIGRATION_V7)?,
        8 => tx.execute_batch(MIGRATION_V8)?,
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

/// v5 — `app_settings` key/value table (NF-V0.5-C App Lock). Keys
/// currently used: `app_lock_pin_phc` (Argon2id PHC string, NULL row
/// when lock is disabled) and `app_lock_after_minutes` (idle minutes
/// before the UI auto-locks). The table stays generic so future
/// preferences can land without a migration.
const MIGRATION_V5: &str = r#"
CREATE TABLE IF NOT EXISTS app_settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"#;

/// v6 — Private Vault (NF-V0.5-C / 2 of 2).
///
/// `notes.vault` is one of:
///   - 'plain'  — title/body/checklist columns are authoritative (default).
///   - 'vault'  — title/body are empty strings; checklist_items rows for
///                this note are deleted; the real payload lives in
///                `vault_ciphertext` (hex-encoded `nonce || aead`).
///
/// The vault DEK is wrapped under a password-derived KEK; the wrap
/// material lives in `app_settings` under three hex-string keys:
///   - `vault_kdf_salt`     — 16 bytes
///   - `vault_dek_nonce`    — 24 bytes
///   - `vault_dek_wrapped`  — DEK bundle (XChaCha20-Poly1305 ciphertext)
const MIGRATION_V6: &str = r#"
ALTER TABLE notes ADD COLUMN vault TEXT NOT NULL DEFAULT 'plain'
    CHECK (vault IN ('plain','vault'));
ALTER TABLE notes ADD COLUMN vault_ciphertext TEXT;
CREATE INDEX IF NOT EXISTS idx_notes_vault ON notes(vault);
"#;

/// v7 — Note version history (NF-V0.5-D).
///
/// Every `update_note` snapshots the pre-update state of the row into
/// `note_snapshots`. A trigger trims each note's history to the most
/// recent 20 snapshots so storage growth is bounded. Vault notes
/// snapshot their `vault_ciphertext` directly — no DEK needed for the
/// history path; restore swaps the ciphertext back into place.
const MIGRATION_V7: &str = r#"
CREATE TABLE IF NOT EXISTS note_snapshots (
    id TEXT PRIMARY KEY,
    note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    title TEXT NOT NULL DEFAULT '',
    body TEXT NOT NULL DEFAULT '',
    color TEXT NOT NULL DEFAULT 'default',
    pinned INTEGER NOT NULL DEFAULT 0,
    checklist_json TEXT NOT NULL DEFAULT '[]',
    vault TEXT NOT NULL DEFAULT 'plain' CHECK (vault IN ('plain','vault')),
    vault_ciphertext TEXT,
    taken_at TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snapshots_note_taken
    ON note_snapshots(note_id, taken_at DESC);

-- Trim to last 20 snapshots per note after every insert. The subselect
-- finds the cutoff row's taken_at; anything older is deleted.
CREATE TRIGGER IF NOT EXISTS note_snapshots_trim_to_20
AFTER INSERT ON note_snapshots
BEGIN
    DELETE FROM note_snapshots
    WHERE note_id = NEW.note_id
      AND id NOT IN (
          SELECT id FROM note_snapshots
           WHERE note_id = NEW.note_id
           ORDER BY taken_at DESC, id DESC
           LIMIT 20
      );
END;
"#;

/// v8 — Reminder schema cleanup (EI-V0.5-14).
///
/// The original `reminders` table carried a redundant `id TEXT PRIMARY
/// KEY` alongside a `note_id TEXT NOT NULL UNIQUE` — one reminder per
/// note made `note_id` the natural key. v8 rebuilds the table with
/// `note_id` as the PK directly + a CHECK on `fire_at` so we can't
/// land an empty timestamp from a future bug. The migration is a
/// classic SQLite table-rebuild (no DROP COLUMN PK support).
const MIGRATION_V8: &str = r#"
CREATE TABLE reminders_new (
    note_id TEXT PRIMARY KEY REFERENCES notes(id) ON DELETE CASCADE,
    fire_at TEXT NOT NULL CHECK (length(fire_at) > 0),
    rrule TEXT,
    snooze_until TEXT,
    fired_at TEXT,
    dismissed_at TEXT,
    created_at TEXT NOT NULL
);
INSERT INTO reminders_new
    (note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at)
SELECT note_id, fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at
FROM reminders;
DROP TABLE reminders;
ALTER TABLE reminders_new RENAME TO reminders;
CREATE INDEX IF NOT EXISTS idx_reminders_pending
    ON reminders(fired_at, fire_at)
    WHERE fired_at IS NULL;
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
    fn migration_v8_rebuilds_reminders_with_note_id_pk() {
        let conn = fresh_conn();
        // PK is now note_id (no separate id column).
        let cols: Vec<(String, i32)> = conn
            .prepare("PRAGMA table_info(reminders)")
            .unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(1)?, r.get::<_, i32>(5)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        let names: Vec<&str> = cols.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"id"), "v8 should drop reminders.id; got {names:?}");
        let note_id_pk = cols.iter().find(|(n, _)| n == "note_id").unwrap().1;
        assert_eq!(note_id_pk, 1, "note_id should be the PK");
        // CHECK on fire_at rejects empty strings.
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
                                position, created_at, updated_at) \
             VALUES ('n1', 'text', '', '', 'default', 0, 0, 0, 0, \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let bad = conn.execute(
            "INSERT INTO reminders (note_id, fire_at, created_at) VALUES ('n1', '', '2026-01-01T00:00:00Z')",
            [],
        );
        assert!(bad.is_err(), "CHECK should reject empty fire_at");
    }

    #[test]
    fn migration_v7_creates_snapshots_table_and_trim_trigger() {
        let conn = fresh_conn();
        // Table exists.
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='note_snapshots'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        // Trigger exists.
        let tcount: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='trigger' AND name='note_snapshots_trim_to_20'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tcount, 1);
        // The trigger trims to last 20. Seed a parent note then insert
        // 25 snapshots and confirm we end at 20.
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
                                position, created_at, updated_at) \
             VALUES ('parent', 'text', '', '', 'default', 0, 0, 0, 0, \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        for i in 0..25 {
            conn.execute(
                "INSERT INTO note_snapshots (id, note_id, kind, taken_at) \
                 VALUES (?1, 'parent', 'text', ?2)",
                rusqlite::params![format!("s{i:02}"), format!("2026-01-{:02}T00:00:00Z", i + 1)],
            )
            .unwrap();
        }
        let remaining: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM note_snapshots WHERE note_id = 'parent'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 20, "trigger should cap to most-recent 20");
        // The oldest survivors should be s05..s24 (descending), i.e. s05 stays.
        let oldest_id: String = conn
            .query_row(
                "SELECT id FROM note_snapshots WHERE note_id = 'parent' \
                 ORDER BY taken_at ASC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(oldest_id, "s05");
    }

    #[test]
    fn migration_v6_adds_vault_columns_to_notes() {
        let conn = fresh_conn();
        // Insert a plain note and confirm the new columns default sanely.
        conn.execute(
            "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed, \
                                position, created_at, updated_at) \
             VALUES ('n1', 'text', 't', 'b', 'default', 0, 0, 0, 0, \
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .unwrap();
        let vault: String = conn
            .query_row("SELECT vault FROM notes WHERE id = 'n1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(vault, "plain");
        let ct: Option<String> = conn
            .query_row("SELECT vault_ciphertext FROM notes WHERE id = 'n1'", [], |r| r.get(0))
            .unwrap();
        assert!(ct.is_none());
        // CHECK constraint should reject bogus values.
        let bad = conn.execute(
            "UPDATE notes SET vault = 'whatever' WHERE id = 'n1'",
            [],
        );
        assert!(bad.is_err(), "CHECK should reject unknown vault state");
    }

    #[test]
    fn migration_v5_creates_app_settings_table() {
        let conn = fresh_conn();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type='table' AND name='app_settings'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "app_settings table should exist after migration");
        // Roundtrip a key to prove the schema is actually usable.
        conn.execute(
            "INSERT INTO app_settings(key, value) VALUES('k', 'v')",
            [],
        )
        .unwrap();
        let v: String = conn
            .query_row(
                "SELECT value FROM app_settings WHERE key = 'k'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(v, "v");
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
