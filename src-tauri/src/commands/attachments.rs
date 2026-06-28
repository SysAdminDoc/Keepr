use super::*;

/// Canonical mime→ext mapping used by every attachment-related command.
/// Legacy attachments use `<id>.<ext>` filenames; v0.25+ writes
/// content-addressed `ab/cd/<hash>.<ext>` paths. Keep this in sync with
/// `AttachmentGrid.mimeToExt` (frontend) and `guess_content_type` (lib.rs).
/// `bin` is the fallback so unknown mimes still get a deterministic suffix.
pub(crate) fn mime_to_ext(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/mp4" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        _ => "bin",
    }
}

/// Delete a single attachment's blob + sibling thumbnail from the resources
/// dir. Best-effort: errors are logged but not surfaced (the DB row is already
/// gone by the time we get here, so the file is orphan regardless). Used by
/// `delete_attachment`, `delete_note_permanent`, and `empty_trash` to keep
/// the on-disk resources dir in sync with the `attachments` table.
#[derive(Clone, Debug)]
pub(super) struct AttachmentFiles {
    pub(super) id: String,
    pub(super) mime: String,
    pub(super) resource_path: Option<String>,
    pub(super) thumb_path: Option<String>,
}

pub(super) fn legacy_resource_path(id: &str, mime: &str) -> String {
    format!("{}.{ext}", id, ext = mime_to_ext(mime))
}

pub(super) fn legacy_thumb_path(id: &str) -> String {
    format!("{id}.thumb.jpg")
}

pub(super) fn attachment_resource_path(files: &AttachmentFiles) -> String {
    files
        .resource_path
        .clone()
        .unwrap_or_else(|| legacy_resource_path(&files.id, &files.mime))
}

pub(super) fn attachment_thumb_path(files: &AttachmentFiles) -> String {
    files
        .thumb_path
        .clone()
        .unwrap_or_else(|| legacy_thumb_path(&files.id))
}

pub(super) fn safe_resource_path(resources_dir: &Path, rel: &str) -> Option<PathBuf> {
    if !is_safe_resource_rel_path(rel) {
        return None;
    }
    Some(resources_dir.join(rel))
}

pub(super) fn is_safe_resource_rel_path(rel: &str) -> bool {
    if rel.is_empty()
        || rel.len() > 1024
        || rel.contains('\0')
        || rel.contains('\\')
        || rel.contains('%')
    {
        return false;
    }
    for segment in rel.split('/') {
        if segment.is_empty()
            || segment == "."
            || segment == ".."
            || segment.contains("..")
            || segment.contains(':')
            || !segment
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return false;
        }
        let lower = segment.to_ascii_lowercase();
        let stem = lower.split('.').next().unwrap_or(&lower);
        if stem == "con"
            || stem == "prn"
            || stem == "aux"
            || stem == "nul"
            || matches!(
                stem,
                "com1" | "com2" | "com3" | "com4" | "com5" | "com6" | "com7" | "com8" | "com9"
            )
            || matches!(
                stem,
                "lpt1" | "lpt2" | "lpt3" | "lpt4" | "lpt5" | "lpt6" | "lpt7" | "lpt8" | "lpt9"
            )
        {
            return false;
        }
    }
    true
}

pub(super) fn remove_resource_file(path: &Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            log::warn!("could not remove attachment file {}: {e}", path.display());
        }
    }
}

pub(super) fn referenced_resource_count(
    conn: &Connection,
    column: &str,
    rel: &str,
) -> rusqlite::Result<i64> {
    match column {
        "resource_path" => conn.query_row(
            "SELECT COUNT(*) FROM attachments WHERE resource_path = ?1",
            params![rel],
            |r| r.get(0),
        ),
        "thumb_path" => conn.query_row(
            "SELECT COUNT(*) FROM attachments WHERE thumb_path = ?1",
            params![rel],
            |r| r.get(0),
        ),
        _ => Ok(0),
    }
}

pub(super) fn remove_if_unreferenced(
    conn: &Connection,
    resources_dir: &Path,
    rel: &str,
    column: &str,
) {
    let Ok(refs) = referenced_resource_count(conn, column, rel) else {
        return;
    };
    if refs > 0 {
        return;
    }
    let Some(path) = safe_resource_path(resources_dir, rel) else {
        log::warn!("skipping unsafe attachment resource path '{rel}'");
        return;
    };
    remove_resource_file(&path);
}

/// Delete a single attachment's blob + sibling thumbnail from the resources
/// dir. Content-addressed rows are ref-counted by `resource_path` and
/// `thumb_path` so duplicate attachments can share one blob. Legacy rows have
/// null paths and keep the old `<id>.<ext>` / `<id>.thumb.jpg` filenames.
pub(super) fn delete_attachment_files(
    conn: &Connection,
    resources_dir: &Path,
    files: &AttachmentFiles,
) {
    let main = attachment_resource_path(files);
    if files.resource_path.is_some() {
        remove_if_unreferenced(conn, resources_dir, &main, "resource_path");
    } else if let Some(path) = safe_resource_path(resources_dir, &main) {
        remove_resource_file(&path);
    }

    let thumb = attachment_thumb_path(files);
    if files.thumb_path.is_some() {
        remove_if_unreferenced(conn, resources_dir, &thumb, "thumb_path");
    } else if let Some(path) = safe_resource_path(resources_dir, &thumb) {
        remove_resource_file(&path);
    }
}

/// Collect every attachment row for the given note ids so the
/// caller can clean up resource files after the DB rows are gone (cascading
/// FK delete drops the `attachments` rows when the parent `notes` row is
/// removed, but the files on disk are not the DB's concern).
pub(super) fn collect_attachment_files(
    conn: &Connection,
    note_ids: &[String],
) -> rusqlite::Result<Vec<AttachmentFiles>> {
    if note_ids.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders = std::iter::repeat("?")
        .take(note_ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "SELECT id, mime, resource_path, thumb_path FROM attachments WHERE note_id IN ({placeholders})"
    );
    let mut stmt = conn.prepare(&sql)?;
    let params_iter: Vec<&dyn rusqlite::ToSql> =
        note_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let rows = stmt
        .query_map(params_iter.as_slice(), |r| {
            Ok(AttachmentFiles {
                id: r.get(0)?,
                mime: r.get(1)?,
                resource_path: r.get(2)?,
                thumb_path: r.get(3)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

// --- NF-01 attachments ---
//
// File model: bytes live under <data_dir>/resources/<id>.<ext>, served
// to the renderer through the keepr-resource://<id>.<ext> protocol
// (registered in lib.rs). The attachments table holds metadata. We
// resolve the filename suffix from the source file's extension so the
// protocol's content-type whitelist (guess_content_type) picks the
// right MIME.

pub(super) const RESOURCES_DIR: &str = "resources";

// Mirror of MAX_PER_FILE_BYTES but lower for in-app uploads, matching
// the spirit of Keep's ~10 MB-per-image cap.
pub(super) const MAX_ATTACHMENT_BYTES: u64 = 32 * 1024 * 1024; // 32 MiB

pub(super) fn sanitize_extension(src: &Path) -> String {
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

pub(super) fn guess_mime_for_ext(ext: &str) -> &'static str {
    match ext {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

#[derive(Clone, Debug)]
pub(super) struct StoredResource {
    pub(super) hash: String,
    pub(super) rel_path: String,
    pub(super) created: bool,
}

pub(super) fn content_addressed_rel_path(hash: &str, ext: &str) -> String {
    format!("{}/{}/{}.{}", &hash[0..2], &hash[2..4], hash, ext)
}

pub(super) fn content_addressed_thumb_path(hash: &str) -> String {
    format!("{}/{}/{}.thumb.jpg", &hash[0..2], &hash[2..4], hash)
}

pub(super) fn store_content_addressed_bytes(
    resources_dir: &Path,
    bytes: &[u8],
    ext: &str,
) -> Result<StoredResource, String> {
    let hash = blake3::hash(bytes).to_hex().to_string();
    let rel_path = content_addressed_rel_path(&hash, ext);
    let target = safe_resource_path(resources_dir, &rel_path)
        .ok_or_else(|| format!("unsafe generated resource path: {rel_path}"))?;
    if target.exists() {
        return Ok(StoredResource {
            hash,
            rel_path,
            created: false,
        });
    }
    let parent = target
        .parent()
        .ok_or_else(|| format!("resource path has no parent: {}", target.display()))?;
    std::fs::create_dir_all(parent).map_err(err)?;
    let tmp_name = format!(
        ".{}.{}.tmp",
        target
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("resource"),
        Uuid::new_v4()
    );
    let tmp = parent.join(tmp_name);
    std::fs::write(&tmp, bytes).map_err(err)?;
    match std::fs::rename(&tmp, &target) {
        Ok(_) => Ok(StoredResource {
            hash,
            rel_path,
            created: true,
        }),
        Err(_) if target.exists() => {
            let _ = std::fs::remove_file(&tmp);
            log::info!(
                "attachment resource already existed after write race: {}",
                target.display()
            );
            Ok(StoredResource {
                hash,
                rel_path,
                created: false,
            })
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(err(e))
        }
    }
}

pub(super) fn write_content_addressed_thumbnail(
    resources_dir: &Path,
    hash: &str,
    decoded: &image::DynamicImage,
) -> Option<String> {
    let rel_path = content_addressed_thumb_path(hash);
    let target = safe_resource_path(resources_dir, &rel_path)?;
    if target.exists() {
        return Some(rel_path);
    }
    if let Some(parent) = target.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            log::warn!("thumbnail dir create failed for {}: {e}", parent.display());
            return None;
        }
    }
    let thumb = decoded.thumbnail(480, 480);
    if let Err(e) = thumb
        .to_rgb8()
        .save_with_format(&target, image::ImageFormat::Jpeg)
    {
        log::warn!("thumbnail generation failed for {}: {e}", target.display());
        return None;
    }
    Some(rel_path)
}

pub(super) fn cleanup_new_resource_on_insert_failure(
    conn: &Connection,
    resources_dir: &Path,
    resource: &StoredResource,
    thumb_path: Option<&str>,
) {
    if resource.created {
        remove_if_unreferenced(conn, resources_dir, &resource.rel_path, "resource_path");
    }
    if let Some(thumb) = thumb_path {
        remove_if_unreferenced(conn, resources_dir, thumb, "thumb_path");
    }
}

pub(super) const RESOURCE_ORPHAN_GRACE: std::time::Duration =
    std::time::Duration::from_secs(24 * 60 * 60);
pub(super) const RESOURCE_TRASH_RETENTION: std::time::Duration =
    std::time::Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Default)]
pub struct OrphanSweepStats {
    pub moved_to_trash: usize,
    pub purged: usize,
}

pub fn sweep_orphaned_resources(
    conn: &Connection,
    resources_dir: &Path,
) -> Result<OrphanSweepStats, String> {
    sweep_orphaned_resources_with_clock(
        conn,
        resources_dir,
        std::time::SystemTime::now(),
        RESOURCE_ORPHAN_GRACE,
        RESOURCE_TRASH_RETENTION,
    )
}

pub(super) fn referenced_resource_paths(
    conn: &Connection,
) -> Result<std::collections::HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT id, mime, resource_path, thumb_path FROM attachments")
        .map_err(err)?;
    let rows = stmt
        .query_map([], |r| {
            Ok(AttachmentFiles {
                id: r.get(0)?,
                mime: r.get(1)?,
                resource_path: r.get(2)?,
                thumb_path: r.get(3)?,
            })
        })
        .map_err(err)?;
    let mut paths = std::collections::HashSet::new();
    for row in rows {
        let files = row.map_err(err)?;
        paths.insert(attachment_resource_path(&files));
        paths.insert(attachment_thumb_path(&files));
    }
    Ok(paths)
}

pub(super) fn path_age(path: &Path, now: std::time::SystemTime) -> Option<std::time::Duration> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|modified| now.duration_since(modified).ok())
}

pub(super) fn resource_rel_from_path(resources_dir: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(resources_dir)
        .ok()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
}

pub(super) fn move_orphan_to_trash(
    resources_dir: &Path,
    trash_dir: &Path,
    rel: &str,
) -> Result<(), String> {
    let src = safe_resource_path(resources_dir, rel)
        .ok_or_else(|| format!("unsafe orphan resource path: {rel}"))?;
    let dst = trash_dir.join(rel);
    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    if dst.exists() {
        std::fs::remove_file(&dst).map_err(err)?;
    }
    match std::fs::rename(&src, &dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            std::fs::copy(&src, &dst).map_err(|copy_err| {
                format!("could not move orphan resource ({rename_err}); copy failed ({copy_err})")
            })?;
            std::fs::remove_file(&src).map_err(err)?;
            Ok(())
        }
    }
}

pub(super) fn sweep_orphaned_resources_with_clock(
    conn: &Connection,
    resources_dir: &Path,
    now: std::time::SystemTime,
    orphan_grace: std::time::Duration,
    trash_retention: std::time::Duration,
) -> Result<OrphanSweepStats, String> {
    if !resources_dir.exists() {
        return Ok(OrphanSweepStats::default());
    }
    let referenced = referenced_resource_paths(conn)?;
    let trash_dir = resources_dir.join(".trash");
    let mut stats = OrphanSweepStats::default();

    for entry in walkdir::WalkDir::new(resources_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() || path.starts_with(&trash_dir) {
            continue;
        }
        let Some(rel) = resource_rel_from_path(resources_dir, path) else {
            continue;
        };
        if referenced.contains(&rel) {
            continue;
        }
        let Some(age) = path_age(path, now) else {
            continue;
        };
        if age < orphan_grace {
            continue;
        }
        move_orphan_to_trash(resources_dir, &trash_dir, &rel)?;
        stats.moved_to_trash += 1;
    }

    if trash_dir.exists() {
        for entry in walkdir::WalkDir::new(&trash_dir)
            .min_depth(1)
            .contents_first(true)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                if path_age(path, now).is_some_and(|age| age >= trash_retention) {
                    std::fs::remove_file(path).map_err(err)?;
                    stats.purged += 1;
                }
            } else if path.is_dir() {
                let _ = std::fs::remove_dir(path);
            }
        }
    }

    Ok(stats)
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
    let bytes = std::fs::read(&src).map_err(err)?;
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "image exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            bytes.len()
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

    add_image_attachment_bytes_inner(
        state.inner(),
        note_id,
        bytes,
        mime.to_string(),
        original_name,
    )
}

pub(super) fn add_image_attachment_bytes_inner(
    state: &AppState,
    note_id: String,
    bytes: Vec<u8>,
    mime: String,
    original_name: String,
) -> Result<Attachment, String> {
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "image exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            bytes.len()
        ));
    }
    let ext = mime_to_ext(&mime);
    let resources_dir = state.data_dir.join(RESOURCES_DIR);
    std::fs::create_dir_all(&resources_dir).map_err(err)?;
    let stored = store_content_addressed_bytes(&resources_dir, &bytes, ext)?;

    let mut width_recorded: Option<i64> = None;
    let mut height_recorded: Option<i64> = None;
    let mut thumb_path: Option<String> = None;
    if mime.starts_with("image/") && mime != "image/svg+xml" {
        if let Ok(decoded) = image::load_from_memory(&bytes) {
            width_recorded = Some(decoded.width() as i64);
            height_recorded = Some(decoded.height() as i64);
            thumb_path = write_content_addressed_thumbnail(&resources_dir, &stored.hash, &decoded);
        }
    }

    let new_id = Uuid::new_v4().to_string();
    let mut conn = state.db.lock();
    let now = now_iso();
    let position_result: Result<i64, String> = (|| {
        let tx = conn.transaction().map_err(err)?;
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE id = ?1",
                params![&note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if exists == 0 {
            return Err(format!("note {note_id} not found"));
        }
        let pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE note_id = ?1",
                params![&note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        tx.execute(
            "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, width, height, position, created_at, resource_path, thumb_path)
             VALUES (?1, ?2, 'image', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                &new_id,
                &note_id,
                &mime,
                &original_name,
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
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![&now, &note_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(pos)
    })();
    let position = match position_result {
        Ok(pos) => pos,
        Err(e) => {
            cleanup_new_resource_on_insert_failure(
                &conn,
                &resources_dir,
                &stored,
                thumb_path.as_deref(),
            );
            return Err(e);
        }
    };
    drop(conn);

    Ok(Attachment {
        id: new_id,
        note_id,
        kind: "image".into(),
        mime,
        filename: original_name,
        byte_size: bytes.len() as i64,
        width: width_recorded,
        height: height_recorded,
        position,
        created_at: now,
        resource_path: Some(stored.rel_path),
        thumb_path,
    })
}

pub(super) fn image_ext_for_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/svg+xml" => Some("svg"),
        _ => None,
    }
}

/// NF-V0.5-I — companion to add_image_attachment for paste-from-
/// clipboard and drag-drop flows where the renderer has the raw bytes
/// but no on-disk file path. Bytes come over IPC as a Vec<u8> (Tauri
/// serializes via base64). `filename_hint` carries the original name
/// when known (e.g. dropped File.name); otherwise we infer from MIME.
#[tauri::command]
pub fn add_image_attachment_bytes(
    state: State<'_, AppState>,
    note_id: String,
    bytes: Vec<u8>,
    mime: String,
    filename_hint: Option<String>,
) -> Result<Attachment, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let ext = image_ext_for_mime(&mime).ok_or_else(|| format!("unsupported mime: {mime}"))?;
    let original_name = filename_hint.unwrap_or_else(|| format!("pasted.{ext}"));
    add_image_attachment_bytes_inner(state.inner(), note_id, bytes, mime, original_name)
}

/// v0.20.3 — audio voice note attachment. The bytes come from a
/// MediaRecorder blob in the renderer (webm/opus or mp4/m4a depending
/// on platform). We bypass `add_image_attachment` because that one
/// runs the bytes through the `image` crate for thumbnail generation,
/// which would fail on audio. Audio attachments don't get thumbnails;
/// the renderer shows an `<audio controls>` element instead.
#[tauri::command]
pub fn add_audio_attachment_bytes(
    state: State<'_, AppState>,
    note_id: String,
    bytes: Vec<u8>,
    mime: String,
    filename_hint: Option<String>,
) -> Result<Attachment, String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    if bytes.len() as u64 > MAX_ATTACHMENT_BYTES {
        return Err(format!(
            "audio exceeds {} bytes (got {})",
            MAX_ATTACHMENT_BYTES,
            bytes.len()
        ));
    }
    let ext = match mime.as_str() {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/mp4" => "m4a",
        "audio/mpeg" => "mp3",
        "audio/wav" => "wav",
        _ => return Err(format!("unsupported audio mime: {mime}")),
    };
    let resources_dir = state.data_dir.join(RESOURCES_DIR);
    std::fs::create_dir_all(&resources_dir).map_err(err)?;
    let stored = store_content_addressed_bytes(&resources_dir, &bytes, ext)?;
    let new_id = Uuid::new_v4().to_string();
    let original_name = filename_hint.unwrap_or_else(|| format!("voice-note.{ext}"));

    let mut conn = state.db.lock();
    let now = now_iso();
    let position_result: Result<i64, String> = (|| {
        let tx = conn.transaction().map_err(err)?;
        let exists: i64 = tx
            .query_row(
                "SELECT COUNT(*) FROM notes WHERE id = ?1",
                params![&note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        if exists == 0 {
            return Err(format!("note {note_id} not found"));
        }
        let pos: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(position) + 1, 0) FROM attachments WHERE note_id = ?1",
                params![&note_id],
                |r| r.get(0),
            )
            .map_err(err)?;
        tx.execute(
            "INSERT INTO attachments (id, note_id, kind, mime, filename, byte_size, position, created_at, resource_path)
             VALUES (?1, ?2, 'audio', ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                &new_id,
                &note_id,
                &mime,
                &original_name,
                bytes.len() as i64,
                pos,
                &now,
                &stored.rel_path,
            ],
        )
        .map_err(err)?;
        tx.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![&now, &note_id],
        )
        .map_err(err)?;
        tx.commit().map_err(err)?;
        Ok(pos)
    })();
    let position = match position_result {
        Ok(pos) => pos,
        Err(e) => {
            cleanup_new_resource_on_insert_failure(&conn, &resources_dir, &stored, None);
            return Err(e);
        }
    };
    drop(conn);

    Ok(Attachment {
        id: new_id,
        note_id,
        kind: "audio".into(),
        mime,
        filename: original_name,
        byte_size: bytes.len() as i64,
        width: None,
        height: None,
        position,
        created_at: now,
        resource_path: Some(stored.rel_path),
        thumb_path: None,
    })
}

#[tauri::command]
pub fn delete_attachment(state: State<'_, AppState>, id: String) -> Result<(), String> {
    if state.importing.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("a restore is currently in progress".into());
    }
    let conn = state.db.lock();
    let (note_id, files): (String, AttachmentFiles) = conn
        .query_row(
            "SELECT note_id, id, mime, resource_path, thumb_path FROM attachments WHERE id = ?1",
            params![&id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    AttachmentFiles {
                        id: r.get(1)?,
                        mime: r.get(2)?,
                        resource_path: r.get(3)?,
                        thumb_path: r.get(4)?,
                    },
                ))
            },
        )
        .map_err(|_| format!("attachment {id} not found"))?;
    conn.execute("DELETE FROM attachments WHERE id = ?1", params![&id])
        .map_err(err)?;
    // Best-effort: bump updated_at so cards re-sort.
    let now = now_iso();
    let _ = conn.execute(
        "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
        params![now, note_id],
    );
    let resources = state.data_dir.join(RESOURCES_DIR);
    delete_attachment_files(&conn, &resources, &files);
    drop(conn);
    Ok(())
}

// ---------------------------------------------------------------------------
// v0.23.0 — offline speech transcription via whisper.cpp
// ---------------------------------------------------------------------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeechModelStatus {
    pub downloaded: bool,
    pub model_id: String,
    pub model_filename: String,
    pub model_size_bytes: u64,
    pub model_url: String,
    pub on_disk_path: String,
}

/// Renderer-facing status of the speech model. Settings → Voice
/// transcription uses this to decide whether to show "Download" or
/// "Delete + Re-download".
#[tauri::command]
pub fn get_speech_model_status(state: State<'_, AppState>) -> Result<SpeechModelStatus, String> {
    let path = crate::transcribe::model_path(&state.data_dir);
    Ok(SpeechModelStatus {
        downloaded: path.exists(),
        model_id: crate::transcribe::MODEL_ID.to_string(),
        model_filename: crate::transcribe::MODEL_FILENAME.to_string(),
        model_size_bytes: crate::transcribe::MODEL_BYTES,
        model_url: crate::transcribe::MODEL_URL.to_string(),
        on_disk_path: path.to_string_lossy().to_string(),
    })
}

/// Download the whisper model from Hugging Face. Streams ~57 MB with
/// progress events on `transcribe://model-progress`. Idempotent: if the
/// model is already on disk and its SHA-1 matches the published digest,
/// this returns without any network activity.
///
/// This is the ONLY outbound HTTP call in Keepr — explicitly opt-in via
/// the Settings → Voice transcription UI. After download, transcription
/// runs fully offline forever.
#[tauri::command]
pub async fn download_speech_model(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let data_dir = state.data_dir.clone();
    crate::transcribe::download_model(app, data_dir)
        .await
        .map_err(|e| e.to_string())
}

/// Delete the model file from disk. Idempotent.
#[tauri::command]
pub fn delete_speech_model(state: State<'_, AppState>) -> Result<(), String> {
    crate::transcribe::delete_model(&state.data_dir).map_err(|e| e.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptRecord {
    pub attachment_id: String,
    pub note_id: String,
    pub text: String,
    pub model: String,
    pub created_at: String,
    pub updated_at: String,
}

/// Return the persisted transcript (if any) for a given audio attachment.
/// Used by the renderer to show "Transcribed" state without re-running
/// inference.
#[tauri::command]
pub fn get_transcript(
    state: State<'_, AppState>,
    attachment_id: String,
) -> Result<Option<TranscriptRecord>, String> {
    let conn = state.db.lock();
    let row = conn
        .query_row(
            "SELECT attachment_id, note_id, text, model, created_at, updated_at
             FROM transcripts WHERE attachment_id = ?1",
            params![attachment_id],
            |r| {
                Ok(TranscriptRecord {
                    attachment_id: r.get(0)?,
                    note_id: r.get(1)?,
                    text: r.get(2)?,
                    model: r.get(3)?,
                    created_at: r.get(4)?,
                    updated_at: r.get(5)?,
                })
            },
        )
        .optional()
        .map_err(err)?;
    Ok(row)
}

/// Transcribe a saved audio attachment. The audio must already be
/// stored on disk via `add_audio_attachment_bytes`. Returns the
/// transcribed text; also persists it to the `transcripts` table so
/// subsequent reads via `get_transcript` are free.
///
/// CPU-heavy: spawns a dedicated OS thread for whisper inference so
/// the Tauri async runtime stays responsive. Cancellation is not
/// supported (whisper.cpp has no mid-inference abort hook); a typical
/// 30-second voice note takes 3-8 seconds depending on CPU.
#[tauri::command]
pub async fn transcribe_audio_attachment(
    state: State<'_, AppState>,
    attachment_id: String,
) -> Result<String, String> {
    // Resolve the audio path + verify it's an audio kind.
    let (note_id, audio_path) = {
        let conn = state.db.lock();
        let (note_id, kind, files): (String, String, AttachmentFiles) = conn
            .query_row(
                "SELECT note_id, kind, id, mime, resource_path, thumb_path FROM attachments WHERE id = ?1",
                params![&attachment_id],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        AttachmentFiles {
                            id: r.get(2)?,
                            mime: r.get(3)?,
                            resource_path: r.get(4)?,
                            thumb_path: r.get(5)?,
                        },
                    ))
                },
            )
            .map_err(|_| format!("attachment {attachment_id} not found"))?;
        if kind != "audio" {
            return Err(format!("attachment {attachment_id} is not an audio kind"));
        }
        let path = state
            .data_dir
            .join(RESOURCES_DIR)
            .join(attachment_resource_path(&files));
        (note_id, path)
    };
    if !audio_path.exists() {
        return Err(format!(
            "audio file missing on disk: {}",
            audio_path.display()
        ));
    }

    // Model must be present + valid.
    let model_path = crate::transcribe::model_path(&state.data_dir);
    if !model_path.exists() {
        return Err(
            "Speech model not downloaded. Open Settings → Voice transcription to download it (~57 MB, one time, then fully offline)."
                .into(),
        );
    }

    // Short-circuit: if we have a transcript for the same CRC32, reuse it.
    let crc = crate::transcribe::wav_crc32(&audio_path).map_err(|e| e.to_string())?;
    {
        let conn = state.db.lock();
        let cached: Option<(String, u32)> = conn
            .query_row(
                "SELECT text, source_crc32 FROM transcripts WHERE attachment_id = ?1",
                params![attachment_id],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, u32>(1)?)),
            )
            .optional()
            .map_err(err)?;
        if let Some((text, prior_crc)) = cached {
            if prior_crc == crc {
                log::info!("transcribe: cache hit for {attachment_id}");
                return Ok(text);
            }
        }
    }

    log::info!(
        "transcribe: starting whisper on {} ({crc:08x})",
        audio_path.display()
    );

    // Spawn whisper on a dedicated OS thread; bridge the result back via
    // tokio's oneshot so this async fn doesn't block the Tauri runtime.
    let (tx, rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    std::thread::spawn(move || {
        let work = (|| -> Result<String, String> {
            let samples = crate::transcribe::wav_to_whisper_samples(&audio_path)
                .map_err(|e| e.to_string())?;
            if samples.is_empty() {
                return Err("audio file decoded to zero samples".into());
            }
            crate::transcribe::transcribe_samples_blocking(&model_path, &samples)
                .map_err(|e| e.to_string())
        })();
        let _ = tx.send(work);
    });
    let text = rx
        .await
        .map_err(|e| format!("worker thread crashed: {e}"))??;

    // Persist (upsert by attachment_id).
    let now = now_iso();
    {
        let conn = state.db.lock();
        conn.execute(
            "INSERT INTO transcripts (attachment_id, note_id, text, model, source_crc32, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(attachment_id) DO UPDATE SET
                 text = excluded.text,
                 model = excluded.model,
                 source_crc32 = excluded.source_crc32,
                 updated_at = excluded.updated_at",
            params![attachment_id, note_id, text, crate::transcribe::MODEL_ID, crc, now],
        )
        .map_err(err)?;
        // Bump the parent note's updated_at so the card resorts.
        let _ = conn.execute(
            "UPDATE notes SET updated_at = ?1 WHERE id = ?2",
            params![now, note_id],
        );
    }
    log::info!(
        "transcribe: completed for {attachment_id} ({} chars)",
        text.len()
    );
    Ok(text)
}
