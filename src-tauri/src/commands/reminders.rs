use super::*;
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
pub(super) const ALLOWED_RRULES: &[&str] =
    &["FREQ=DAILY", "FREQ=WEEKLY", "FREQ=MONTHLY", "FREQ=YEARLY"];
pub(super) fn validate_rrule(rrule: Option<&str>) -> Result<(), String> {
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
            prev_utc.with_year(y).or_else(|| {
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
    conn.execute("DELETE FROM reminders WHERE note_id = ?1", params![note_id])
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
pub(super) fn format_ics_utc(rfc3339: &str) -> Result<String, String> {
    let parsed = chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map_err(|e| format!("invalid timestamp {rfc3339}: {e}"))?
        .with_timezone(&chrono::Utc);
    Ok(parsed.format("%Y%m%dT%H%M%SZ").to_string())
}
/// Escape ICS-special characters per RFC 5545 §3.3.11. Backslash MUST
/// be replaced first to avoid double-escaping.
pub(super) fn escape_ics(input: &str) -> String {
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
pub(super) fn reminder_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Reminder> {
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
