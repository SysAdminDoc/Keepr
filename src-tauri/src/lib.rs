mod db;
mod commands;

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::{Emitter, Manager, UriSchemeContext, UriSchemeResponder};
use tauri::http::{Request, Response, StatusCode};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

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

// NF-06 — show / hide the main window. Tray click + tray menu both go
// through this so the visibility state stays consistent.
fn toggle_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let visible = win.is_visible().unwrap_or(false);
        let focused = win.is_focused().unwrap_or(false);
        if visible && focused {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
    }
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // NF-06 — register the global "new quick note" shortcut. The handler
    // shows the main window and emits a `keepr://quick-capture` event the
    // renderer subscribes to (App.tsx) to open a blank editor.
    let quick_shortcut = Shortcut::new(
        Some(Modifiers::CONTROL | Modifiers::ALT),
        Code::KeyN,
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &quick_shortcut && event.state() == ShortcutState::Pressed {
                        show_main_window(app);
                        let _ = app.emit("keepr://quick-capture", ());
                    }
                })
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol(
            "keepr-resource",
            handle_resource_request,
        )
        .on_window_event(|window, event| {
            // NF-06 — closing the window minimizes to tray instead of
            // killing the process so the app continues to receive global
            // hotkey events and run the auto-backup tick.
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(move |app| {
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

            // NF-06 — tray icon + menu. "Show / Hide Keepr" toggles the
            // main window; "New note" focuses the window and emits the
            // quick-capture event; "Quit" actually exits the process
            // (this is the only path that bypasses the "minimize to tray"
            // on close behavior above).
            let show_item = MenuItem::with_id(app, "show", "Show / hide Keepr", true, None::<&str>)?;
            let new_item = MenuItem::with_id(app, "new", "New note", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Keepr", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &new_item, &quit_item])?;

            let _tray = TrayIconBuilder::with_id("main-tray")
                .icon(app.default_window_icon().cloned().unwrap_or_else(|| {
                    // Fallback to a 1x1 transparent icon if the bundled
                    // default isn't available (shouldn't happen in
                    // release builds, but keeps tests / dev happy).
                    tauri::image::Image::new_owned(vec![0, 0, 0, 0], 1, 1)
                }))
                .tooltip("Keepr")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => toggle_main_window(app),
                    "new" => {
                        show_main_window(app);
                        let _ = app.emit("keepr://quick-capture", ());
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    // Left click on the tray icon toggles the window (so
                    // users don't have to right-click for the menu every
                    // time).
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_main_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // NF-06 — register the Ctrl+Alt+N global hotkey now that the
            // window is up. Failure is logged but not fatal — the rest of
            // the app still works without the shortcut.
            if let Err(e) = app.global_shortcut().register(quick_shortcut) {
                eprintln!("keepr: failed to register Ctrl+Alt+N: {e}");
            }

            // NF-02 — reminder scheduler thread. Polls take_due_reminders
            // every 30s, fires a native notification per due reminder via
            // tauri-plugin-notification, and emits keepr://reminder-fired
            // so the renderer can refresh the bell badge.
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                use std::thread::sleep;
                use std::time::Duration;
                loop {
                    let now = chrono::Utc::now().to_rfc3339();
                    let state: tauri::State<'_, AppState> = app_handle.state();
                    match commands::take_due_reminders(&state, &now) {
                        Ok(items) if !items.is_empty() => {
                            for (rem, preview) in &items {
                                let _ = tauri_plugin_notification::NotificationExt::notification(
                                    &app_handle,
                                )
                                .builder()
                                .title("Keepr reminder")
                                .body(preview)
                                .show();
                                let _ = app_handle
                                    .emit("keepr://reminder-fired", &rem.id);
                            }
                        }
                        Ok(_) => {}
                        Err(e) => eprintln!("keepr: reminder sweep failed: {e}"),
                    }
                    sleep(Duration::from_secs(30));
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::get_note,
            commands::create_note,
            commands::update_note,
            commands::duplicate_note,
            commands::reorder_notes,
            commands::add_image_attachment,
            commands::delete_attachment,
            commands::set_reminder,
            commands::clear_reminder,
            commands::list_reminders,
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
