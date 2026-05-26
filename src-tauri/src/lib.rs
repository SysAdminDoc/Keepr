mod db;
mod commands;

use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
