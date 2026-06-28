mod commands;
mod db;
mod lock;
// v0.22.1 — vault module is `pub` so the standalone `keepr-verify`
// binary in src/bin/keepr-verify.rs can re-derive the KEK and decrypt
// vault notes from outside the Tauri runtime. No private fields are
// exposed beyond what was already accessible to commands.rs.
pub mod transcribe;
pub mod vault;
pub mod web_clipper;

use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::http::{Request, Response, StatusCode};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, UriSchemeContext, UriSchemeResponder};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

/// Sentinel filename that, when present next to the running EXE, switches
/// Keepr into portable mode — the DB is written next to the EXE instead of
/// into the per-user `app_data_dir` (EI-11). Touch `portable.flag` on a USB
/// stick alongside `keepr.exe` and the whole app travels with the drive.
const PORTABLE_SENTINEL: &str = "portable.flag";

/// Resolve the directory Keepr will store `keepr.db` in. Precedence:
///   1. `--data-dir <path>` CLI flag (v0.24.1+) — wins over everything.
///      Path is created if absent. Used for explicit relocation without
///      having to drop a portable.flag (CI test rigs, multi-profile
///      power users, BYO-cloud-folder workflows).
///   2. `portable.flag` next to the running EXE — portable mode (USB
///      stick, network share). DB lives next to the EXE.
///   3. Tauri's per-user `app_data_dir()` — the default.
fn resolve_data_dir(app: &tauri::AppHandle) -> std::io::Result<PathBuf> {
    if let Some(override_path) = data_dir_override_from_args() {
        std::fs::create_dir_all(&override_path)?;
        return Ok(override_path);
    }
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

/// Parse `std::env::args()` looking for `--data-dir <path>` or
/// `--data-dir=<path>`. Returns the path if found and non-empty. Quiet
/// on every form of malformed input (no panics during startup); the
/// caller falls through to the default resolution if `None` is
/// returned.
fn data_dir_override_from_args() -> Option<PathBuf> {
    parse_data_dir_arg(std::env::args().skip(1).collect::<Vec<_>>().as_slice())
}

fn parse_data_dir_arg(args: &[String]) -> Option<PathBuf> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if let Some(rest) = arg.strip_prefix("--data-dir=") {
            if !rest.is_empty() {
                return Some(PathBuf::from(rest));
            }
            return None;
        }
        if arg == "--data-dir" {
            if let Some(next) = iter.next() {
                if !next.is_empty() {
                    return Some(PathBuf::from(next));
                }
            }
            return None;
        }
    }
    None
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
    /// v0.24.0 — runtime metadata for the Web Clipper localhost server
    /// (port + bearer token). `None` until the server has bound on
    /// startup. Exposed to the renderer via `get_web_clipper_info` so
    /// Settings → Web Clipper can display the connection info the
    /// user needs to paste into their browser extension.
    pub web_clipper: web_clipper::WebClipperState,
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
        if let Ok(v) = content_type.parse() {
            resp.headers_mut().insert("Content-Type", v);
        }
        if let Ok(v) = "*".parse() {
            resp.headers_mut().insert("Access-Control-Allow-Origin", v);
        }
        if let Ok(v) = "bytes".parse() {
            resp.headers_mut().insert("Accept-Ranges", v);
        }
        resp
    };

    let id_part = req.uri().path().trim_start_matches('/').to_string();
    // Reject anything that smells like a traversal attempt. Keepr never
    // writes percent signs into resource names, so encoded payloads such as
    // `%2e%2e%2f`, `%5c`, or `%00` are rejected before filesystem access.
    if !is_safe_resource_id(&id_part) {
        return make(
            StatusCode::BAD_REQUEST,
            b"bad resource id".to_vec(),
            "text/plain",
        );
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
        Some(p) if p.starts_with(&resources_dir) => {}
        _ => {
            return make(
                StatusCode::BAD_REQUEST,
                b"bad resource id".to_vec(),
                "text/plain",
            )
        }
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
            if let Ok(v) = cr.parse() {
                resp.headers_mut().insert("Content-Range", v);
            }
            let len = (end - start + 1).to_string();
            if let Ok(v) = len.parse() {
                resp.headers_mut().insert("Content-Length", v);
            }
            return resp;
        }
        // Malformed range header — fall through to a full 200 response.
    }

    let mut resp = make(StatusCode::OK, bytes, ct);
    let len = total.to_string();
    if let Ok(v) = len.parse() {
        resp.headers_mut().insert("Content-Length", v);
    }
    resp
}

/// Validate a `keepr-resource://` relative path before joining it onto the
/// resources directory. Legacy attachments use one filename
/// (`<uuid>.<ext>`). v0.25+ content-addressed attachments use exactly
/// `ab/cd/<64-hex-hash>.<ext>` or `ab/cd/<64-hex-hash>.thumb.jpg`.
/// Everything else is rejected before filesystem access.
fn is_safe_resource_id(s: &str) -> bool {
    if s.is_empty() || s.len() > 1024 {
        return false;
    }
    if s.contains('\0') || s.contains('\\') || s.contains("..") || s.contains('%') {
        return false;
    }
    let parts: Vec<&str> = s.split('/').collect();
    if parts.len() != 1 && parts.len() != 3 {
        return false;
    }
    // Reject Windows reserved device names: CON, PRN, AUX, NUL, COM1-9,
    // LPT1-9. Opening any of these as a file resolves to the actual
    // device on Windows even from non-Windows-aware code paths.
    for part in &parts {
        if part.is_empty()
            || part.contains(':')
            || !part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        {
            return false;
        }
        let lower = part.to_ascii_lowercase();
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
    }
    if parts.len() == 3 {
        if !is_two_hex(parts[0]) || !is_two_hex(parts[1]) {
            return false;
        }
        let stem = parts[2].split('.').next().unwrap_or("");
        if stem.len() != 64 || !stem.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
        let expected = format!(
            "{}{}",
            parts[0].to_ascii_lowercase(),
            parts[1].to_ascii_lowercase()
        );
        if !stem.to_ascii_lowercase().starts_with(&expected) {
            return false;
        }
    }
    true
}

fn is_two_hex(s: &str) -> bool {
    s.len() == 2 && s.chars().all(|c| c.is_ascii_hexdigit())
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
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
    {
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
    let quick_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::ALT), Code::KeyN);

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
                    tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir {
                        file_name: Some("Keepr".to_string()),
                    }),
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
        .register_asynchronous_uri_scheme_protocol("keepr-resource", handle_resource_request)
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
            let db_arc = Arc::new(Mutex::new(conn));
            let shutdown = Arc::new(AtomicBool::new(false));
            let web_clipper_info: web_clipper::WebClipperState =
                Arc::new(Mutex::new(web_clipper::WebClipperInfo::default()));

            // v0.24.0 — spin up the Web Clipper localhost server. Binds
            // 127.0.0.1:0 (OS picks the port); persists port + token
            // into app_settings. We tokio::spawn the bind itself onto
            // a dedicated runtime so we don't block app init if a
            // network namespace quirk delays bind.
            let db_for_clipper = db_arc.clone();
            let info_for_clipper = web_clipper_info.clone();
            std::thread::spawn(move || {
                let rt = match tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                {
                    Ok(r) => r,
                    Err(e) => {
                        log::error!("web_clipper: could not build tokio runtime: {e}");
                        return;
                    }
                };
                rt.block_on(async move {
                    match web_clipper::start_server(db_for_clipper, info_for_clipper).await {
                        Ok(port) => log::info!("web_clipper: ready on 127.0.0.1:{port}"),
                        Err(e) => log::error!("web_clipper: start failed: {e}"),
                    }
                    // Keep the runtime alive so the spawned server task
                    // keeps running for the lifetime of the app.
                    std::future::pending::<()>().await;
                });
            });

            // v0.25.0 — content-addressed resources can leave harmless
            // zero-reference files behind if a crash lands between file
            // write and DB insert. Sweep once on startup, then daily.
            let db_for_sweep = db_arc.clone();
            let resources_for_sweep = data_dir.join("resources");
            let shutdown_for_sweep = shutdown.clone();
            std::thread::spawn(move || {
                use std::thread::sleep;
                use std::time::Duration;
                loop {
                    {
                        let conn = db_for_sweep.lock();
                        match commands::attachments::sweep_orphaned_resources(
                            &conn,
                            &resources_for_sweep,
                        ) {
                            Ok(stats) if stats.moved_to_trash > 0 || stats.purged > 0 => {
                                log::info!(
                                    "resource sweep: moved {} orphan(s), purged {} trash file(s)",
                                    stats.moved_to_trash,
                                    stats.purged
                                )
                            }
                            Ok(_) => {}
                            Err(e) => log::warn!("resource sweep failed: {e}"),
                        }
                    }
                    for _ in 0..(24 * 60 * 60) {
                        if shutdown_for_sweep.load(std::sync::atomic::Ordering::SeqCst) {
                            log::info!("resource sweep shutting down");
                            return;
                        }
                        sleep(Duration::from_secs(1));
                    }
                }
            });

            app.manage(AppState {
                db: db_arc,
                importing: Arc::new(AtomicBool::new(false)),
                data_dir,
                vault_dek: Arc::new(Mutex::new(None)),
                shutdown,
                web_clipper: web_clipper_info,
            });

            // NF-06 — tray icon + menu. "Show / Hide Keepr" toggles the
            // main window; "New note" focuses the window and emits the
            // quick-capture event; "Quit" actually exits the process
            // (this is the only path that bypasses the "minimize to tray"
            // on close behavior above).
            let show_item =
                MenuItem::with_id(app, "show", "Show / hide Keepr", true, None::<&str>)?;
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
                    match commands::reminders::peek_due_reminders(&state, &now) {
                        Ok(items) if !items.is_empty() => {
                            for (rem, preview) in &items {
                                let show_result =
                                    tauri_plugin_notification::NotificationExt::notification(
                                        &app_handle,
                                    )
                                    .builder()
                                    .title("Keepr reminder")
                                    .body(preview)
                                    .show();
                                match show_result {
                                    Ok(_) => {
                                        // Toast surfaced — safe to mark fired.
                                        if let Err(e) = commands::reminders::mark_reminder_fired(
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
                                        let _ =
                                            app_handle.emit("keepr://reminder-fired", &rem.note_id);
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
            commands::notes::list_notes,
            commands::notes::get_note,
            commands::notes::search_notes,
            commands::notes::create_note,
            commands::notes::update_note,
            commands::notes::duplicate_note,
            commands::notes::reorder_notes,
            commands::attachments::add_image_attachment,
            commands::attachments::add_image_attachment_bytes,
            commands::attachments::delete_attachment,
            commands::reminders::set_reminder,
            commands::reminders::snooze_reminder,
            commands::reminders::clear_reminder,
            commands::reminders::list_reminders,
            commands::io::export_vault,
            commands::io::import_takeout,
            commands::notes::delete_note_permanent,
            commands::notes::set_archived,
            commands::notes::set_trashed,
            commands::notes::set_pinned,
            commands::notes::set_color,
            commands::labels::list_labels,
            commands::labels::create_label,
            commands::labels::rename_label,
            commands::labels::delete_label,
            commands::labels::set_note_labels,
            commands::notes::empty_trash,
            commands::io::export_zip,
            commands::io::import_zip,
            commands::io::get_app_version,
            commands::io::get_data_dir,
            commands::io::get_log_dir,
            commands::io::open_app_dir,
            commands::security::get_app_lock_settings,
            commands::security::enable_app_lock,
            commands::security::disable_app_lock,
            commands::security::verify_app_lock_pin,
            commands::security::set_app_lock_minutes,
            commands::security::get_vault_status,
            commands::security::init_vault,
            commands::security::unlock_vault,
            commands::security::lock_vault,
            commands::security::change_vault_password,
            commands::security::move_note_to_vault,
            commands::security::move_note_out_of_vault,
            commands::security::move_notes_to_vault,
            commands::security::move_notes_out_of_vault,
            commands::security::vault_has_recovery_seed,
            commands::security::setup_vault_recovery_seed,
            commands::security::remove_vault_recovery_seed,
            commands::security::recover_vault_with_seed,
            commands::attachments::add_audio_attachment_bytes,
            commands::io::prune_auto_backups,
            commands::labels::list_smart_labels,
            commands::labels::create_smart_label,
            commands::labels::update_smart_label,
            commands::labels::delete_smart_label,
            commands::history::list_snapshots,
            commands::history::restore_snapshot,
            commands::reminders::export_reminders_ics,
            // v0.23.0 — opt-in offline speech transcription via whisper.cpp.
            commands::attachments::get_speech_model_status,
            commands::attachments::download_speech_model,
            commands::attachments::delete_speech_model,
            commands::attachments::get_transcript,
            commands::attachments::transcribe_audio_attachment,
            // v0.24.0 — Web Clipper (localhost server + MV3 extension).
            commands::io::get_web_clipper_info,
            commands::io::regenerate_web_clipper_token,
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
    use super::{is_safe_resource_id, parse_byte_range, parse_data_dir_arg};
    use std::path::PathBuf;

    // is_safe_resource_id —

    #[test]
    fn safe_id_accepts_uuid_filename() {
        assert!(is_safe_resource_id(
            "550e8400-e29b-41d4-a716-446655440000.png"
        ));
        assert!(is_safe_resource_id(
            "550e8400-e29b-41d4-a716-446655440000.thumb.jpg"
        ));
        assert!(is_safe_resource_id(
            "550e8400-e29b-41d4-a716-446655440000.wav"
        ));
    }

    #[test]
    fn safe_id_accepts_content_addressed_paths() {
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert!(is_safe_resource_id(&format!("ab/cd/{hash}.png")));
        assert!(is_safe_resource_id(&format!("ab/cd/{hash}.thumb.jpg")));
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
        assert!(!is_safe_resource_id("aa/bb/not-a-hash.png"));
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

    // parse_data_dir_arg — v0.24.1 CLI flag

    fn s(args: &[&str]) -> Vec<String> {
        args.iter().map(|a| a.to_string()).collect()
    }

    #[test]
    fn data_dir_arg_none_when_unset() {
        assert_eq!(parse_data_dir_arg(&s(&[])), None);
        assert_eq!(parse_data_dir_arg(&s(&["--other", "foo"])), None);
    }

    #[test]
    fn data_dir_arg_space_form() {
        assert_eq!(
            parse_data_dir_arg(&s(&["--data-dir", "C:/Notes/Keepr"])),
            Some(PathBuf::from("C:/Notes/Keepr"))
        );
    }

    #[test]
    fn data_dir_arg_equals_form() {
        assert_eq!(
            parse_data_dir_arg(&s(&["--data-dir=/var/lib/keepr"])),
            Some(PathBuf::from("/var/lib/keepr"))
        );
    }

    #[test]
    fn data_dir_arg_ignores_unrelated_args() {
        assert_eq!(
            parse_data_dir_arg(&s(&["--quiet", "--data-dir", "x", "--later"])),
            Some(PathBuf::from("x"))
        );
    }

    #[test]
    fn data_dir_arg_returns_none_when_flag_has_no_value() {
        // --data-dir at end of argv with no value -> None, NOT a panic.
        assert_eq!(parse_data_dir_arg(&s(&["--data-dir"])), None);
        // --data-dir= with empty value -> None.
        assert_eq!(parse_data_dir_arg(&s(&["--data-dir="])), None);
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
