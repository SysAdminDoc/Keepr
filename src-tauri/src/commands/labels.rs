use super::*;

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
pub fn rename_label(state: State<'_, AppState>, id: String, name: String) -> Result<(), String> {
    let conn = state.db.lock();
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("label name cannot be empty".into());
    }
    conn.execute(
        "UPDATE labels SET name = ?1 WHERE id = ?2",
        params![trimmed, id],
    )
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
    tx.execute(
        "DELETE FROM note_labels WHERE note_id = ?1",
        params![note_id],
    )
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
