mod db;
mod commands;

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Manager, UriSchemeContext, UriSchemeResponder};
use tauri::http::{Request, Response, StatusCode};

/// Sentinel filename that, when present next to the running EXE, switches
/// Keepr into portable mode — the DB is written next to the EXE instead of
/// into the per-user `app_data_dir` (EI-11). Touch `portable.flag` on a USB
/// stick alongside `keepr.exe` and the whole app travels with the drive.
const PORTABLE_SENTINEL: &str = "portable.flag";

/// Resolve the directory Keepr will store `keepr.db` in. Portable when the
/// sentinel file exists, per-user otherwise.
fn resolve_data_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join(PORTABLE_SENTINEL).exists() {
                return Ok(parent.to_path_buf());
            }
        }
    }
    app.path()
        .app_data_dir()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::NotFound, e.to_string()))
}

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// Exclusive flag held during `import_zip`. Every mutating command should
    /// `if state.importing.load(...) { return Err(...) }` early so a parallel
    /// invoke can't write into a transient throwaway connection (EI-03).
    pub importing: Arc<AtomicBool>,
    /// Resolved on startup, may be either `app_data_dir()` (per-user) or
    /// the EXE's parent (portable mode — see `PORTABLE_SENTINEL`). Used by
    /// every command that needs to read or write keepr.db / attachments.
    pub data_dir: PathBuf,
}

/// Subdirectory under the data dir where the `keepr-resource://` protocol
/// looks for attachment blobs. v0.2 only scaffolds the protocol; the
/// commands that write into this directory (NF-01 image attachments) land
/// in v0.4.
const RESOURCES_SUBDIR: &str = "resources";

/// Tauri custom-protocol handler for `keepr-resource://<id>` URLs. Resolves
/// the requested id to a file under `<data_dir>/resources/<id>` (with an
/// optional extension after the id, e.g. `keepr-resource://abc123.png`).
///
/// Refuses any path containing `/` or `..` so renderer-side
/// `<img src="keepr-resource://abc/../keepr.db">` can't read the DB file.
/// Returns 404 for unknown ids, 200 with detected content-type for known
/// blobs.
fn handle_resource_request<R: tauri::Runtime>(
    ctx: UriSchemeContext<'_, R>,
    req: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let respond = |status: StatusCode, body: Vec<u8>, content_type: &str| {
        let mut resp = Response::new(body);
        *resp.status_mut() = status;
        if let Ok(value) = content_type.parse() {
            resp.headers_mut().insert("Content-Type", value);
        }
        // Allow the renderer's CSP to consume the response.
        if let Ok(value) = "*".parse() {
            resp.headers_mut().insert("Access-Control-Allow-Origin", value);
        }
        responder.respond(resp);
    };

    let id_part = req
        .uri()
        .path()
        .trim_start_matches('/')
        .to_string();

    if id_part.is_empty()
        || id_part.contains("..")
        || id_part.contains('/')
        || id_part.contains('\\')
    {
        respond(StatusCode::BAD_REQUEST, b"bad resource id".to_vec(), "text/plain");
        return;
    }

    let state: tauri::State<'_, AppState> = ctx.app_handle().state();
    let target: PathBuf = state.data_dir.join(RESOURCES_SUBDIR).join(&id_part);
    match std::fs::read(&target) {
        Ok(bytes) => {
            let ct = guess_content_type(&target);
            respond(StatusCode::OK, bytes, ct);
        }
        Err(_) => respond(StatusCode::NOT_FOUND, b"not found".to_vec(), "text/plain"),
    }
}

fn guess_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|s| s.to_str()).map(|s| s.to_ascii_lowercase()) {
        Some(ref e) if e == "png" => "image/png",
        Some(ref e) if e == "jpg" || e == "jpeg" => "image/jpeg",
        Some(ref e) if e == "gif" => "image/gif",
        Some(ref e) if e == "webp" => "image/webp",
        Some(ref e) if e == "svg" => "image/svg+xml",
        Some(ref e) if e == "mp3" => "audio/mpeg",
        Some(ref e) if e == "wav" => "audio/wav",
        Some(ref e) if e == "webm" => "audio/webm",
        Some(ref e) if e == "m4a" => "audio/mp4",
        Some(ref e) if e == "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .register_asynchronous_uri_scheme_protocol(
            "keepr-resource",
            handle_resource_request,
        )
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = match resolve_data_dir(&handle) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("keepr: failed to resolve data dir: {e}");
                    return Err(Box::new(e));
                }
            };
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("keepr.db");
            let conn = match db::open(&db_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("keepr: failed to open db at {}: {e}", db_path.display());
                    return Err(format!("failed to open db: {e}").into());
                }
            };
            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                importing: Arc::new(AtomicBool::new(false)),
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::get_note,
            commands::create_note,
            commands::update_note,
            commands::duplicate_note,
            commands::delete_note_permanent,
            commands::set_archived,
            commands::set_trashed,
            commands::set_pinned,
            commands::set_color,
            commands::list_labels,
            commands::create_label,
            commands::rename_label,
            commands::delete_label,
            commands::set_note_labels,
            commands::empty_trash,
            commands::export_zip,
            commands::import_zip,
            commands::get_data_dir,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
