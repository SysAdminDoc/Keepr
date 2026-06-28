use super::*;

// --- NF-08 Markdown vault export + Google Takeout import ---

pub(super) const VAULT_RESOURCES_DIR: &str = "_resources";

pub(super) fn sanitize_vault_filename(stem: &str, id: &str) -> String {
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
pub fn export_vault(state: State<'_, AppState>, dest_dir: String) -> Result<String, String> {
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
    let stamp = chrono::Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
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

    let notes = notes::list_notes(state.clone())?;
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
                let stored_name = att
                    .resource_path
                    .clone()
                    .unwrap_or_else(|| attachments::legacy_resource_path(&att.id, &att.mime));
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
                    if let Some(parent) = dst.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
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

pub(super) fn build_frontmatter(
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

pub(super) fn yaml_quote_if_needed(s: &str) -> String {
    // If the value contains : # & * { } [ ] , | > ' " % @ ` or starts with
    // - we double-quote it. Conservative — over-quotes some safe values
    // but never under-quotes.
    let needs = s.chars().any(|c| {
        matches!(
            c,
            ':' | '#'
                | '&'
                | '*'
                | '{'
                | '}'
                | '['
                | ']'
                | ','
                | '|'
                | '>'
                | '\''
                | '"'
                | '%'
                | '@'
                | '`'
        )
    }) || s.starts_with('-')
        || s.is_empty();
    if needs {
        format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        s.to_string()
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
pub fn import_takeout(state: State<'_, AppState>, src: String) -> Result<u32, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    log::info!("import_takeout: opening {src}");
    let file = File::open(&src).map_err(|e| {
        log::error!("import_takeout: open failed: {e}");
        err(e)
    })?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| {
        log::error!("import_takeout: not a valid zip: {e}");
        err(e)
    })?;

    // Two-pass: first collect attachment bytes keyed by their archive
    // path so we can resolve note-relative references.
    let mut blobs: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
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
            let folder = name
                .rsplit_once('/')
                .map(|(d, _)| d.to_string())
                .unwrap_or_default();
            note_entries.push((folder, text));
        } else if entry.size() > 0 && entry.size() <= attachments::MAX_ATTACHMENT_BYTES {
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
        let title = v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let text_body = v
            .get("textContent")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let pinned = v.get("isPinned").and_then(|x| x.as_bool()).unwrap_or(false);
        let archived = v
            .get("isArchived")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        let trashed = v
            .get("isTrashed")
            .and_then(|x| x.as_bool())
            .unwrap_or(false);
        if trashed {
            continue; // Takeout-trashed notes get skipped on import.
        }
        let color = map_keep_color(v.get("color").and_then(|x| x.as_str()).unwrap_or("DEFAULT"));
        let label_names: Vec<String> = v
            .get("labels")
            .and_then(|x| x.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|l| {
                        l.get("name")
                            .and_then(|n| n.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();

        // List content (checklist) — when present overrides textContent.
        let (kind, checklist_input) =
            if let Some(arr) = v.get("listContent").and_then(|x| x.as_array()) {
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
            match labels::create_label(state.clone(), name.clone()) {
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
        let created = match notes::create_note(state.clone(), input) {
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
            let _ = notes::set_archived(state.clone(), created.id.clone(), true);
        }

        // EI-V0.5-6 — preserve Takeout reminders. Takeout's shape varies
        // by export year; we accept several common forms. Single-shot
        // only; recurring reminders (when they exist in the JSON) get
        // their fire_at imported but the rrule field is ignored.
        if let Some(reminders) = v.get("reminders").and_then(|x| x.as_array()) {
            for r in reminders {
                if let Some(fire_at) = takeout_reminder_fire_at(r) {
                    let _ = reminders::set_reminder(
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
                    let ext = attachments::mime_to_ext(&mime);
                    let new_id = Uuid::new_v4().to_string();
                    let resources_dir = state.data_dir.join("resources");
                    if std::fs::create_dir_all(&resources_dir).is_err() {
                        continue;
                    }
                    let Ok(stored) =
                        attachments::store_content_addressed_bytes(&resources_dir, bytes, ext)
                    else {
                        continue;
                    };
                    let mut width_recorded: Option<i64> = None;
                    let mut height_recorded: Option<i64> = None;
                    let mut thumb_path: Option<String> = None;
                    if mime.starts_with("image/") && mime != "image/svg+xml" {
                        if let Ok(decoded) = image::load_from_memory(bytes) {
                            width_recorded = Some(decoded.width() as i64);
                            height_recorded = Some(decoded.height() as i64);
                            thumb_path = attachments::write_content_addressed_thumbnail(
                                &resources_dir,
                                &stored.hash,
                                &decoded,
                            );
                        }
                    }
                    let now = now_iso();
                    {
                        let mut conn = state.db.lock();
                        let insert_result: Result<(), String> = (|| {
                            let tx = conn.transaction().map_err(err)?;
                            let pos: i64 = tx
                                .query_row(
                                    "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE note_id = ?1",
                                    params![&created.id],
                                    |r| r.get(0),
                                )
                                .map_err(err)?;
                            tx.execute(
                                "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, width, height, position, created_at, resource_path, thumb_path)
                                 VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                                params![
                                    &new_id,
                                    &created.id,
                                    &mime,
                                    &rel,
                                    bytes.len() as i64,
                                    width_recorded,
                                    height_recorded,
                                    pos,
                                    &now,
                                    &stored.rel_path,
                                    thumb_path.as_deref(),
                                ],
                            )
                            .map_err(err)?;
                            tx.commit().map_err(err)?;
                            Ok(())
                        })();
                        if insert_result.is_err() {
                            attachments::cleanup_new_resource_on_insert_failure(
                                &conn,
                                &resources_dir,
                                &stored,
                                thumb_path.as_deref(),
                            );
                            continue;
                        }
                    }
                }
            }
        }

        imported += 1;
    }
    log::info!("import_takeout: imported {imported} notes from {src}");
    Ok(imported)
}

/// Detect a Google Keep note by JSON shape so we don't depend on the
/// archive path (Takeout localizes the `Keep` folder name and users
/// sometimes re-zip without the `Takeout/` prefix). A Keep note has
/// an `isPinned` boolean, at least one canonical timestamp, and at
/// least one content field (text body or checklist).
pub(super) fn is_keep_note_shape(v: &serde_json::Value) -> bool {
    let obj = match v.as_object() {
        Some(o) => o,
        None => return false,
    };
    let has_pinned = obj.get("isPinned").map(|x| x.is_boolean()).unwrap_or(false);
    let has_ts =
        obj.contains_key("createdTimestampUsec") || obj.contains_key("userEditedTimestampUsec");
    let has_content = obj.contains_key("textContent") || obj.contains_key("listContent");
    has_pinned && has_ts && has_content
}

pub(super) fn map_keep_color(c: &str) -> String {
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
pub(super) fn takeout_usec_to_rfc3339(v: Option<&serde_json::Value>) -> Option<String> {
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
pub(super) fn takeout_reminder_fire_at(r: &serde_json::Value) -> Option<String> {
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
    if let Some(usec_value) = r
        .get("reminderTimeUsec")
        .or_else(|| r.get("reminder_time_usec"))
    {
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

#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
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

pub(super) const MAX_ENTRY_COUNT: usize = 10_000;
pub(super) const MAX_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GiB
pub(super) const MAX_PER_FILE_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

/// Validate a candidate restore archive against EI-01's caps and EI-01's
/// path-safety rules without writing anything to disk. Pure function so it
/// can be unit tested.
pub(super) fn validate_zip_archive<R: std::io::Read + std::io::Seek>(
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
pub fn export_zip(state: State<'_, AppState>, dest: String) -> Result<String, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let data_dir: PathBuf = state.data_dir.clone();
    let dest_path = PathBuf::from(&dest);
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            log::error!(
                "export_zip: could not create parent dir {}: {e}",
                parent.display()
            );
            err(e)
        })?;
    }
    log::info!("export_zip: writing backup to {}", dest_path.display());

    // EI-02: flush WAL into the main DB before zipping (otherwise recent
    // committed writes are silently absent from the backup).
    {
        let conn = state.db.lock();
        conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
            .map_err(|e| {
                log::error!("export_zip: wal_checkpoint failed: {e}");
                err(e)
            })?;
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
pub fn import_zip(state: State<'_, AppState>, src: String) -> Result<(), String> {
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

    log::info!("import_zip: restoring from {src}");
    let result = do_import_zip(&state, &src);
    match &result {
        Ok(_) => log::info!("import_zip: restore complete"),
        Err(e) => log::error!("import_zip: restore failed: {e}"),
    }
    result
}

pub(super) struct ImportGate {
    flag: Arc<std::sync::atomic::AtomicBool>,
}
impl Drop for ImportGate {
    fn drop(&mut self) {
        self.flag.store(false, std::sync::atomic::Ordering::SeqCst);
    }
}

pub(super) fn do_import_zip(state: &State<'_, AppState>, src: &str) -> Result<(), String> {
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

// ---------------------------------------------------------------------------
// v0.24.0 — Web Clipper (localhost HTTP server + MV3 extension)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebClipperInfoOut {
    /// Localhost port the server is bound on. `None` if startup hasn't
    /// completed yet (rare — startup is sub-second).
    pub port: Option<u16>,
    /// 64-char hex bearer token. Persisted per install.
    pub token: Option<String>,
}

/// Return the current Web Clipper port + bearer token so the user can
/// paste them into the browser extension's Options page.
#[tauri::command]
pub fn get_web_clipper_info(state: State<'_, AppState>) -> Result<WebClipperInfoOut, String> {
    let info = state.web_clipper.lock().clone();
    Ok(WebClipperInfoOut {
        port: info.port,
        token: info.token,
    })
}

/// Generate a fresh 256-bit bearer token. Invalidates any previously
/// paired extensions — user must re-paste the new token.
#[tauri::command]
pub fn regenerate_web_clipper_token(state: State<'_, AppState>) -> Result<String, String> {
    let new_token = {
        let conn = state.db.lock();
        crate::web_clipper::regenerate_token(&conn)?
    };
    let mut guard = state.web_clipper.lock();
    guard.token = Some(new_token.clone());
    log::info!("web_clipper: bearer token regenerated");
    Ok(new_token)
}
