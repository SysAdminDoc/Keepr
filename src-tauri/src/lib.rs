mod db;
mod commands;
mod lock;
// v0.22.1 — vault module is `pub` so the standalone `keepr-verify`
// binary in src/bin/keepr-verify.rs can re-derive the KEK and decrypt
// vault notes from outside the Tauri runtime. No private fields are
// exposed beyond what was already accessible to commands.rs.
pub mod vault;

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
    /// NF-V0.5-C — Private Vault data-encryption-key, present only while
    /// the vault is unlocked. Zeroized on Drop; replaced with `None`
    /// when `lock_vault` is called or the app exits.
    pub vault_dek: Arc<Mutex<Option<vault::Dek>>>,
    /// EI-V0.5-12 — set to true on `RunEvent::ExitRequested` so the
    /// reminder scheduler thread breaks its sleep loop and returns. The
    /// thread checks this flag at the top of every iteration AND wakes
    /// from sleep early via a parking pair (see `run()` below).
    pub shutdown: Arc<AtomicBool>,
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
    // v0.22.8 fix: without Range support, Chromium's `<audio>` element
    // loads but can't determine the playable range and refuses to play
    // media files. Build the response in one place so the move-only
    // `responder` is consumed exactly once.
    let resp = build_resource_response(&ctx, &req);
    responder.respond(resp);
}

fn build_resource_response<R: tauri::Runtime>(
    ctx: &UriSchemeContext<'_, R>,
    req: &Request<Vec<u8>>,
) -> Response<Vec<u8>> {
    let make = |status: StatusCode, body: Vec<u8>, content_type: &str| -> Response<Vec<u8>> {
        let mut resp = Response::new(body);
        *resp.status_mut() = status;
        if let Ok(v) = content_type.parse() { resp.headers_mut().insert("Content-Type", v); }
        if let Ok(v) = "*".parse() { resp.headers_mut().insert("Access-Control-Allow-Origin", v); }
        if let Ok(v) = "bytes".parse() { resp.headers_mut().insert("Accept-Ranges", v); }
        resp
    };

    let id_part = req.uri().path().trim_start_matches('/').to_string();
    // Reject anything that smells like a traversal attempt. We check the
    // raw, the percent-decoded, and the canonicalized form to defeat
    // bypasses via `%2e%2e%2f`, `%5c` (encoded backslash), or NUL bytes.
    if !is_safe_resource_id(&id_part) {
        return make(StatusCode::BAD_REQUEST, b"bad resource id".to_vec(), "text/plain");
    }

    let state: tauri::State<'_, AppState> = ctx.app_handle().state();
    let resources_dir = state.data_dir.join(RESOURCES_SUBDIR);
    let target: PathBuf = resources_dir.join(&id_part);

    // Defense-in-depth: even after `is_safe_resource_id`, verify the
    // resolved path is still inside the resources directory. We compare
    // the parent (since the file may not exist yet) rather than the full
    // canonical path. PathBuf::join with a leading `/` or absolute path
    // would replace the prefix; the safety check above blocks that, this
    // check catches anything we missed.
    match target.parent() {
        Some(p) if p == resources_dir.as_path() => {}
        _ => return make(StatusCode::BAD_REQUEST, b"bad resource id".to_vec(), "text/plain"),
    }

    let bytes = match std::fs::read(&target) {
        Ok(b) => b,
        Err(_) => return make(StatusCode::NOT_FOUND, b"not found".to_vec(), "text/plain"),
    };
    let ct = guess_content_type(&target);
    let total = bytes.len();

    // Range header: `bytes=START-END` (END inclusive; either side may be empty).
    let range_header = req
        .headers()
        .get("range")
        .or_else(|| req.headers().get("Range"))
        .and_then(|v| v.to_str().ok());

    if let Some(raw) = range_header {
        if let Some((start, end)) = parse_byte_range(raw, total) {
            let slice = bytes[start..=end].to_vec();
            let mut resp = make(StatusCode::PARTIAL_CONTENT, slice, ct);
            let cr = format!("bytes {start}-{end}/{total}");
            if let Ok(v) = cr.parse() { resp.headers_mut().insert("Content-Range", v); }
            let len = (end - start + 1).to_string();
            if let Ok(v) = len.parse() { resp.headers_mut().insert("Content-Length", v); }
            return resp;
        }
        // Malformed range header — fall through to a full 200 response.
    }

    let mut resp = make(StatusCode::OK, bytes, ct);
    let len = total.to_string();
    if let Ok(v) = len.parse() { resp.headers_mut().insert("Content-Length", v); }
    resp
}

/// Validate a `keepr-resource://` path component before joining it onto the
/// resources directory. Rejects path-traversal attempts (literal `..`, `/`,
/// `\\`, NUL bytes), URL-encoded equivalents (`%2e%2e`, `%2f`, `%5c`, `%00`),
/// and anything that looks absolute or drive-prefixed on Windows (`C:`,
/// `\\?\`). The renderer-side `convertFileSrc(<id>.<ext>, "keepr-resource")`
/// only ever produces filenames of the form `<uuid>.<ext>` or `<uuid>.thumb.jpg`,
/// so a strict allow-list (alphanumeric + `-`, `_`, `.`) would also work; we
/// stay slightly looser to tolerate future filename schemes while still
/// rejecting traversal vectors.
fn is_safe_resource_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 1024 {
        return false;
    }
    if s.contains('\0') || s.contains('/') || s.contains('\\') || s.contains("..") {
        return false;
    }
    // Reject ANY percent-encoded character. Filenames Keepr writes never
    // contain percent signs, so this only blocks attacker payloads.
    if s.contains('%') {
        return false;
    }
    // Reject Windows reserved device names: CON, PRN, AUX, NUL, COM1-9,
    // LPT1-9. Opening any of these as a file resolves to the actual
    // device on Windows even from non-Windows-aware code paths.
    let lower = s.to_ascii_lowercase();
    let stem: &str = lower.split('.').next().unwrap_or(&lower);
    let stem = stem.split(':').next().unwrap_or(stem);
    let is_reserved = matches!(stem, "con" | "prn" | "aux" | "nul")
        || (stem.len() == 4
            && (stem.starts_with("com") || stem.starts_with("lpt"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if is_reserved {
        return false;
    }
    if s.len() >= 2 && s.as_bytes()[1] == b':' {
        // `C:foo` would resolve to "current dir on C:" — reject.
        return false;
    }
    true
}

/// Parse an HTTP `Range: bytes=...` header. Returns `(start, end)` inclusive
/// bounds clamped to `[0, total - 1]`. Returns `None` for the unsupported
/// multi-range form (`bytes=0-99,200-299`) and for any malformed input.
fn parse_byte_range(raw: &str, total: usize) -> Option<(usize, usize)> {
    if total == 0 {
        return None;
    }
    let raw = raw.trim();
    let spec = raw.strip_prefix("bytes=")?;
    // Reject multi-range — we don't bother implementing the multipart
    // boundary response.
    if spec.contains(',') {
        return None;
    }
    let (lo, hi) = spec.split_once('-')?;
    let lo = lo.trim();
    let hi = hi.trim();
    let max_idx = total - 1;
    if lo.is_empty() {
        // Suffix range: `bytes=-500` → last 500 bytes.
        let n: usize = hi.parse().ok()?;
        if n == 0 {
            return None;
        }
        let n = n.min(total);
        return Some((total - n, max_idx));
    }
    let start: usize = lo.parse().ok()?;
    if start > max_idx {
        return None;
    }
    let end = if hi.is_empty() {
        max_idx
    } else {
        let e: usize = hi.parse().ok()?;
        e.min(max_idx)
    };
    if end < start {
        return None;
    }
    Some((start, end))
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
        // EI-V0.5-4 — register single-instance BEFORE any other plugin so
        // the second-launch callback gets the chance to refocus the
        // running window and bail before duplicate AppState init.
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            // Show + focus the existing main window. Best-effort.
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.unminimize();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        // Persist window position + size across launches. Without this
        // the window resets to the tauri.conf.json default (1280x800)
        // on every start, which is hostile on multi-monitor setups.
        // State file lives next to keepr.db in the app data dir (or
        // next to keepr.exe in portable mode — the plugin uses the
        // same path::app_config_dir resolution).
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // NF-V0.5-J — file + stdout logging via tauri-plugin-log. Writes
        // to <app_log_dir>/Keepr.log (per OS convention). Rotation kicks
        // in at 1 MiB and keeps the previous file around as .old; stderr
        // / stdout are mirrored in dev so println! and eprintln! still
        // surface in `tauri dev`.
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .targets([
                    tauri_plugin_log::Target::new(
                        tauri_plugin_log::TargetKind::LogDir { file_name: Some("Keepr".to_string()) },
                    ),
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::Stdout),
                ])
                .max_file_size(1024 * 1024)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepOne)
                .build(),
        )
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
                vault_dek: Arc::new(Mutex::new(None)),
                shutdown: Arc::new(AtomicBool::new(false)),
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
            // window is up. EI-V0.5-7 — surface failure to the renderer
            // (not just stderr) so users see a toast instead of silently
            // wondering why nothing happens when they press the hotkey.
            // Common cause: another app has Ctrl+Alt+N grabbed already.
            match app.global_shortcut().register(quick_shortcut) {
                Ok(_) => {
                    let _ = app.emit("keepr://hotkey-status", "ok");
                }
                Err(e) => {
                    eprintln!("keepr: failed to register Ctrl+Alt+N: {e}");
                    let _ = app.emit("keepr://hotkey-status", e.to_string());
                }
            }

            // NF-02 — reminder scheduler thread. Polls peek_due_reminders
            // every 30s, fires a native notification per due reminder via
            // tauri-plugin-notification, and emits keepr://reminder-fired
            // so the renderer can refresh the bell badge.
            //
            // EI-V0.5-2 — two-phase fire-then-mark: peek (no write), fire
            // each toast, mark_reminder_fired only on success. If
            // notification.show() fails, the reminder stays pending and
            // the next 30s tick retries.
            let app_handle = app.handle().clone();
            std::thread::spawn(move || {
                use std::thread::sleep;
                use std::time::Duration;
                loop {
                    let state: tauri::State<'_, AppState> = app_handle.state();
                    // EI-V0.5-12 — exit cleanly when the run loop signals
                    // shutdown. Checked before AND between sleep slices
                    // so we never wait the full 30 s after Exit.
                    if state.shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                        log::info!("reminder scheduler shutting down");
                        break;
                    }
                    let now = chrono::Utc::now().to_rfc3339();
                    match commands::peek_due_reminders(&state, &now) {
                        Ok(items) if !items.is_empty() => {
                            for (rem, preview) in &items {
                                let show_result = tauri_plugin_notification::NotificationExt::notification(
                                    &app_handle,
                                )
                                .builder()
                                .title("Keepr reminder")
                                .body(preview)
                                .show();
                                match show_result {
                                    Ok(_) => {
                                        // Toast surfaced — safe to mark fired.
                                        if let Err(e) = commands::mark_reminder_fired(
                                            &state,
                                            &rem.note_id,
                                            &now,
                                        ) {
                                            log::warn!(
                                                "failed to mark reminder for note {} fired: {e}",
                                                rem.note_id
                                            );
                                        }
                                        // Payload is the note id — the renderer opens
                                        // the editor on it via the View-note toast.
                                        let _ = app_handle
                                            .emit("keepr://reminder-fired", &rem.note_id);
                                    }
                                    Err(e) => {
                                        // Permission denied / COM error / Focus Assist —
                                        // leave fired_at NULL so the next sweep retries.
                                        log::warn!(
                                            "notification.show() failed for note {}: {e}",
                                            rem.note_id
                                        );
                                    }
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(e) => log::warn!("reminder sweep failed: {e}"),
                    }
                    // Sleep in 1-second slices so a shutdown request takes
                    // at most one second to propagate. 30 one-second checks
                    // is cheaper than the per-sweep work.
                    for _ in 0..30 {
                        sleep(Duration::from_secs(1));
                        if state.shutdown.load(std::sync::atomic::Ordering::SeqCst) {
                            break;
                        }
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_notes,
            commands::get_note,
            commands::search_notes,
            commands::create_note,
            commands::update_note,
            commands::duplicate_note,
            commands::reorder_notes,
            commands::add_image_attachment,
            commands::add_image_attachment_bytes,
            commands::delete_attachment,
            commands::set_reminder,
            commands::snooze_reminder,
            commands::clear_reminder,
            commands::list_reminders,
            commands::export_vault,
            commands::import_takeout,
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
            commands::get_log_dir,
            commands::open_app_dir,
            commands::get_app_lock_settings,
            commands::enable_app_lock,
            commands::disable_app_lock,
            commands::verify_app_lock_pin,
            commands::set_app_lock_minutes,
            commands::get_vault_status,
            commands::init_vault,
            commands::unlock_vault,
            commands::lock_vault,
            commands::change_vault_password,
            commands::move_note_to_vault,
            commands::move_note_out_of_vault,
            commands::move_notes_to_vault,
            commands::move_notes_out_of_vault,
            commands::vault_has_recovery_seed,
            commands::setup_vault_recovery_seed,
            commands::remove_vault_recovery_seed,
            commands::recover_vault_with_seed,
            commands::add_audio_attachment_bytes,
            commands::prune_auto_backups,
            commands::list_smart_labels,
            commands::create_smart_label,
            commands::update_smart_label,
            commands::delete_smart_label,
            commands::list_snapshots,
            commands::restore_snapshot,
            commands::export_reminders_ics,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {
            // EI-V0.5-12 — flip the shutdown flag on ExitRequested so the
            // reminder scheduler thread wakes from its 1-second sleep,
            // sees the flag, and returns. Tauri kills the process after
            // this callback returns; the explicit signal lets the
            // scheduler log a clean shutdown rather than vanishing.
            if let tauri::RunEvent::ExitRequested { .. } = event {
                if let Some(state) = app_handle.try_state::<AppState>() {
                    state
                        .shutdown
                        .store(true, std::sync::atomic::Ordering::SeqCst);
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::{is_safe_resource_id, parse_byte_range};

    // is_safe_resource_id —

    #[test]
    fn safe_id_accepts_uuid_filename() {
        assert!(is_safe_resource_id("550e8400-e29b-41d4-a716-446655440000.png"));
        assert!(is_safe_resource_id("550e8400-e29b-41d4-a716-446655440000.thumb.jpg"));
        assert!(is_safe_resource_id("550e8400-e29b-41d4-a716-446655440000.wav"));
    }

    #[test]
    fn safe_id_rejects_empty() {
        assert!(!is_safe_resource_id(""));
    }

    #[test]
    fn safe_id_rejects_dot_dot() {
        assert!(!is_safe_resource_id(".."));
        assert!(!is_safe_resource_id("..foo"));
        assert!(!is_safe_resource_id("foo..bar"));
    }

    #[test]
    fn safe_id_rejects_separators() {
        assert!(!is_safe_resource_id("a/b"));
        assert!(!is_safe_resource_id("a\\b"));
    }

    #[test]
    fn safe_id_rejects_nul_byte() {
        assert!(!is_safe_resource_id("foo\0bar"));
    }

    #[test]
    fn safe_id_rejects_percent_encoded_traversal() {
        // The previous handler accepted these because the literal-string
        // checks for `..`/`/`/`\\` don't catch the encoded forms; the
        // filesystem also doesn't decode them, so they'd land as weird
        // filenames — but defense-in-depth: reject outright.
        assert!(!is_safe_resource_id("%2e%2e%2fkeepr.db"));
        assert!(!is_safe_resource_id("%2E%2E%5Ckeepr.db"));
        assert!(!is_safe_resource_id("foo%00.png"));
    }

    #[test]
    fn safe_id_rejects_windows_drive_prefix() {
        assert!(!is_safe_resource_id("C:keepr.db"));
        assert!(!is_safe_resource_id("Z:\\absolute"));
    }

    #[test]
    fn safe_id_rejects_windows_reserved_names() {
        assert!(!is_safe_resource_id("con"));
        assert!(!is_safe_resource_id("con.png"));
        assert!(!is_safe_resource_id("CON.png"));
        assert!(!is_safe_resource_id("nul"));
        assert!(!is_safe_resource_id("com1.txt"));
    }

    #[test]
    fn safe_id_allows_legitimate_names_starting_with_reserved_prefix() {
        // "container.png" starts with "con" but isn't the CON reserved
        // device — should be allowed.
        assert!(is_safe_resource_id("container.png"));
        assert!(is_safe_resource_id("computer-keys.jpg"));
        assert!(is_safe_resource_id("lptable-photo.png"));
    }

    #[test]
    fn safe_id_rejects_overly_long_input() {
        let too_long = "a".repeat(2000);
        assert!(!is_safe_resource_id(&too_long));
    }

    // parse_byte_range —


    #[test]
    fn parse_full_range() {
        assert_eq!(parse_byte_range("bytes=0-99", 1000), Some((0, 99)));
    }

    #[test]
    fn parse_open_ended_range() {
        // `bytes=500-` → from 500 to EOF.
        assert_eq!(parse_byte_range("bytes=500-", 1000), Some((500, 999)));
    }

    #[test]
    fn parse_suffix_range() {
        // `bytes=-100` → last 100 bytes.
        assert_eq!(parse_byte_range("bytes=-100", 1000), Some((900, 999)));
    }

    #[test]
    fn parse_suffix_range_larger_than_file() {
        // Suffix larger than file → entire file.
        assert_eq!(parse_byte_range("bytes=-5000", 1000), Some((0, 999)));
    }

    #[test]
    fn parse_clamps_end_to_eof() {
        assert_eq!(parse_byte_range("bytes=900-9999", 1000), Some((900, 999)));
    }

    #[test]
    fn parse_rejects_start_past_eof() {
        assert_eq!(parse_byte_range("bytes=2000-3000", 1000), None);
    }

    #[test]
    fn parse_rejects_inverted_range() {
        assert_eq!(parse_byte_range("bytes=500-100", 1000), None);
    }

    #[test]
    fn parse_rejects_multi_range() {
        // We don't support `multipart/byteranges` responses.
        assert_eq!(parse_byte_range("bytes=0-99,200-299", 1000), None);
    }

    #[test]
    fn parse_rejects_missing_prefix() {
        assert_eq!(parse_byte_range("0-99", 1000), None);
    }

    #[test]
    fn parse_rejects_empty_file() {
        assert_eq!(parse_byte_range("bytes=0-99", 0), None);
    }
}
