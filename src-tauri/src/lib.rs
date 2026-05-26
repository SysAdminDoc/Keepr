mod db;
mod commands;

use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub db: Arc<Mutex<rusqlite::Connection>>,
    /// Exclusive flag held during `import_zip`. Every mutating command should
    /// `if state.importing.load(...) { return Err(...) }` early so a parallel
    /// invoke can't write into a transient throwaway connection (EI-03).
    pub importing: Arc<AtomicBool>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app data dir");
            std::fs::create_dir_all(&data_dir).ok();
            let db_path = data_dir.join("keepr.db");
            let conn = db::open(&db_path).expect("failed to open db");
            app.manage(AppState {
                db: Arc::new(Mutex::new(conn)),
                importing: Arc::new(AtomicBool::new(false)),
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
