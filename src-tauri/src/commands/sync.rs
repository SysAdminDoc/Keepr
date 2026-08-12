use crate::sync::{SyncPeer, SyncResult, SyncSettings, SyncState, SyncStatus};
use crate::AppState;
use chrono::Utc;
use rusqlite::params;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::time::Duration;
use tauri::State;
use tokio::sync::oneshot;
use uuid::Uuid;

use crate::sync::protocol;

fn get_setting(state: &AppState, key: &str) -> Option<String> {
    let conn = state.db.lock();
    conn.query_row(
        "SELECT value FROM app_settings WHERE key = ?1",
        params![key],
        |r| r.get(0),
    )
    .ok()
}

fn set_setting(state: &AppState, key: &str, value: &str) -> Result<(), String> {
    let conn = state.db.lock();
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn ensure_device_id(state: &AppState) -> String {
    if let Some(id) = get_setting(state, "sync_device_id") {
        return id;
    }
    let id = Uuid::new_v4().to_string();
    let _ = set_setting(state, "sync_device_id", &id);
    id
}

pub fn ensure_sync_token(state: &AppState) -> String {
    if let Some(tok) = get_setting(state, "sync_token") {
        if tok.len() >= 32 {
            return tok;
        }
    }
    let tok = Uuid::new_v4().to_string().replace('-', "")
        + &Uuid::new_v4().to_string().replace('-', "");
    let _ = set_setting(state, "sync_token", &tok);
    tok
}

fn device_name() -> String {
    hostname::get()
        .ok()
        .and_then(|h| h.into_string().ok())
        .unwrap_or_else(|| "Keepr".to_string())
}

/// Start the LAN sync server and mDNS discovery on a dedicated runtime.
/// Keeping the runtime handle in `SyncState` makes the Settings toggle a
/// real lifecycle control instead of a setting that only takes effect after
/// the next process launch.
pub fn start_sync_runtime(state: &AppState, sync: &SyncState) -> Result<(), String> {
    let mut runtime_guard = sync.runtime.lock();
    if let Some(runtime) = runtime_guard.as_ref() {
        if !runtime
            .thread
            .as_ref()
            .is_some_and(std::thread::JoinHandle::is_finished)
        {
            return Ok(());
        }
    }
    if let Some(mut stale) = runtime_guard.take() {
        stale.stop_browser.store(true, Ordering::SeqCst);
        if let Some(sender) = stale.server_shutdown.take() {
            let _ = sender.send(());
        }
        if let Some(thread) = stale.thread.take() {
            let _ = thread.join();
        }
    }

    let device_id = ensure_device_id(state);
    let sync_token = ensure_sync_token(state);
    let device_name = device_name();
    let db = state.db.clone();
    let resources_dir = state.data_dir.join("resources");
    let port_holder = sync.port.clone();
    let peers_holder = sync.peers.clone();
    let status_holder = sync.status.clone();
    let stop_browser = Arc::new(AtomicBool::new(false));
    let stop_for_thread = stop_browser.clone();
    let (server_shutdown, server_shutdown_rx) = oneshot::channel();
    let (ready_tx, ready_rx) = sync_channel::<Result<u16, String>>(1);
    let did = device_id.clone();
    let dname = device_name.clone();

    let thread = std::thread::spawn(move || {
        let runtime = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(runtime) => runtime,
            Err(e) => {
                let message = format!("could not build sync runtime: {e}");
                let _ = ready_tx.send(Err(message.clone()));
                *status_holder.lock() = SyncStatus::Error;
                log::error!("sync: {message}");
                return;
            }
        };

        runtime.block_on(async move {
            let (port, server_task) = match crate::sync::server::start(
                db,
                resources_dir,
                did.clone(),
                dname.clone(),
                sync_token,
                server_shutdown_rx,
            )
            .await
            {
                Ok(started) => started,
                Err(e) => {
                    let _ = ready_tx.send(Err(e.clone()));
                    *status_holder.lock() = SyncStatus::Error;
                    log::error!("sync server start failed: {e}");
                    return;
                }
            };

            *port_holder.lock() = Some(port);
            *status_holder.lock() = SyncStatus::Idle;
            let _ = ready_tx.send(Ok(port));
            log::info!("sync server: ready on 0.0.0.0:{port}");

            let service_daemon = match crate::sync::discovery::register_service(
                &did, &dname, port,
            ) {
                Ok(daemon) => Some(daemon),
                Err(e) => {
                    log::warn!("sync mDNS register: {e}");
                    None
                }
            };
            let browser = match crate::sync::discovery::start_browser(
                peers_holder.clone(),
                did,
                stop_for_thread.clone(),
            ) {
                Ok(handles) => Some(handles),
                Err(e) => {
                    log::warn!("sync mDNS browse: {e}");
                    None
                }
            };

            if let Err(e) = server_task.await {
                log::error!("sync server task failed: {e}");
            }

            let requested_stop = stop_for_thread.load(Ordering::SeqCst);
            stop_for_thread.store(true, Ordering::SeqCst);
            if let Some(daemon) = service_daemon {
                if let Ok(done) = daemon.shutdown() {
                    let _ = done.recv_timeout(Duration::from_secs(2));
                }
            }
            if let Some((daemon, browser_thread)) = browser {
                if let Ok(done) = daemon.shutdown() {
                    let _ = done.recv_timeout(Duration::from_secs(2));
                }
                let _ = browser_thread.join();
            }

            peers_holder.lock().clear();
            *port_holder.lock() = None;
            *status_holder.lock() = if requested_stop {
                SyncStatus::Disabled
            } else {
                SyncStatus::Error
            };
        });
    });

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(_port)) => {
            *runtime_guard = Some(crate::sync::SyncRuntime {
                stop_browser,
                server_shutdown: Some(server_shutdown),
                thread: Some(thread),
            });
            Ok(())
        }
        Ok(Err(e)) => {
            let _ = server_shutdown.send(());
            let _ = thread.join();
            Err(e)
        }
        Err(e) => {
            let _ = server_shutdown.send(());
            let _ = thread.join();
            Err(format!("sync server did not start: {e}"))
        }
    }
}

/// Stop the server, mDNS daemons, and their worker threads. This is safe to
/// call repeatedly, including from the app's exit callback.
pub fn stop_sync_runtime(sync: &SyncState) {
    let runtime = sync.runtime.lock().take();
    if let Some(mut runtime) = runtime {
        runtime.stop_browser.store(true, Ordering::SeqCst);
        if let Some(sender) = runtime.server_shutdown.take() {
            let _ = sender.send(());
        }
        if let Some(thread) = runtime.thread.take() {
            let _ = thread.join();
        }
    }
    sync.peers.lock().clear();
    *sync.port.lock() = None;
    *sync.status.lock() = SyncStatus::Disabled;
}

#[tauri::command]
pub fn get_sync_settings(state: State<'_, AppState>, sync: State<'_, SyncState>) -> SyncSettings {
    let device_id = ensure_device_id(&state);
    let enabled = get_setting(&state, "sync_enabled")
        .map(|v| v == "true")
        .unwrap_or(false);
    let last_sync = get_setting(&state, "sync_last_at");
    let port = *sync.port.lock();
    SyncSettings {
        enabled,
        device_id,
        device_name: device_name(),
        port,
        last_sync,
    }
}

#[tauri::command]
pub fn get_sync_peers(sync: State<'_, SyncState>) -> Vec<SyncPeer> {
    sync.peers.lock().values().cloned().collect()
}

#[tauri::command]
pub fn get_sync_status(sync: State<'_, SyncState>) -> SyncStatus {
    *sync.status.lock()
}

#[tauri::command]
pub async fn set_sync_enabled(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
    enabled: bool,
) -> Result<(), String> {
    set_setting(&state, "sync_enabled", if enabled { "true" } else { "false" })?;
    if enabled {
        if let Err(e) = start_sync_runtime(state.inner(), sync.inner()) {
            let _ = set_setting(&state, "sync_enabled", "false");
            *sync.status.lock() = SyncStatus::Error;
            return Err(e);
        }
    } else {
        stop_sync_runtime(sync.inner());
    }
    Ok(())
}

#[tauri::command]
pub async fn sync_now(
    state: State<'_, AppState>,
    sync: State<'_, SyncState>,
) -> Result<Vec<SyncResult>, String> {
    let enabled = get_setting(&state, "sync_enabled")
        .map(|v| v == "true")
        .unwrap_or(false);
    if !enabled {
        return Err("Sync is not enabled".into());
    }

    let peers: Vec<SyncPeer> = sync.peers.lock().values().cloned().collect();
    if peers.is_empty() {
        return Err("No peers discovered on the local network".into());
    }

    *sync.status.lock() = SyncStatus::Syncing;
    let device_id = ensure_device_id(&state);
    let resources_dir = state.data_dir.join("resources");
    let mut results = Vec::new();

    for peer in &peers {
        match sync_with_peer(&state, &device_id, &resources_dir, peer).await {
            Ok(result) => results.push(result),
            Err(e) => {
                log::warn!("sync with {} failed: {e}", peer.device_name);
            }
        }
    }

    let now = Utc::now().to_rfc3339();
    let _ = set_setting(&state, "sync_last_at", &now);

    let cutoff = (Utc::now() - chrono::Duration::days(30)).to_rfc3339();
    {
        let conn = state.db.lock();
        let _ = protocol::purge_old_tombstones(&conn, &cutoff);
    }

    *sync.status.lock() = if results.is_empty() {
        SyncStatus::Error
    } else {
        SyncStatus::Idle
    };

    if results.is_empty() && !peers.is_empty() {
        return Err("All sync attempts failed".into());
    }

    Ok(results)
}

async fn sync_with_peer(
    state: &AppState,
    device_id: &str,
    resources_dir: &std::path::Path,
    peer: &SyncPeer,
) -> Result<SyncResult, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let base_url = format!("http://{}:{}", peer.host, peer.port);
    let token = get_setting(state, "sync_peer_token")
        .unwrap_or_default();

    let sv = {
        let conn = state.db.lock();
        protocol::build_state_vector(&conn, device_id)?
    };

    let resp = client
        .post(format!("{base_url}/sync/reconcile"))
        .bearer_auth(&token)
        .json(&protocol::ReconcileRequest {
            state_vector: sv,
        })
        .send()
        .await
        .map_err(|e| format!("reconcile request: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("reconcile failed: {}", resp.status()));
    }

    let reconcile: protocol::ReconcileResponse =
        resp.json().await.map_err(|e| format!("parse: {e}"))?;

    let notes_pulled = reconcile.pull.len();
    {
        let conn = state.db.lock();
        protocol::apply_pushed_notes(&conn, &reconcile.pull, &reconcile.labels)?;
        protocol::apply_tombstones(&conn, &reconcile.tombstones)?;
    }

    let mut notes_to_push = Vec::new();
    {
        let conn = state.db.lock();
        for note_id in &reconcile.push_ids {
            if let Ok(Some(sn)) = load_sync_note_from_conn(&conn, note_id) {
                if sn.note.vault != "vault" {
                    notes_to_push.push(sn);
                }
            }
        }
    }
    let notes_pushed = notes_to_push.len();

    if !notes_to_push.is_empty() {
        let all_labels: Vec<crate::commands::Label> = {
            let conn = state.db.lock();
            let mut stmt = conn
                .prepare("SELECT id, name FROM labels")
                .map_err(|e| e.to_string())?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(crate::commands::Label {
                        id: r.get(0)?,
                        name: r.get(1)?,
                    })
                })
                .map_err(|e| e.to_string())?;
            rows.filter_map(|r| r.ok()).collect()
        };

        let push_req = protocol::PushRequest {
            notes: notes_to_push,
            labels: all_labels,
        };

        let resp = client
            .post(format!("{base_url}/sync/push"))
            .bearer_auth(&token)
            .json(&push_req)
            .send()
            .await
            .map_err(|e| format!("push request: {e}"))?;

        if !resp.status().is_success() {
            log::warn!("push to {} failed: {}", peer.device_name, resp.status());
        }
    }

    let local_hashes: Vec<String> = {
        let conn = state.db.lock();
        let mut stmt = conn
            .prepare(
                "SELECT DISTINCT resource_path FROM attachments WHERE resource_path IS NOT NULL",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let mut attachments_transferred = 0;
    for remote_path in &reconcile.attachment_hashes {
        if !protocol::is_safe_sync_resource_path(remote_path) {
            log::warn!("sync: skipping unsafe attachment path from peer: {remote_path}");
            continue;
        }
        if local_hashes.contains(remote_path) {
            continue;
        }
        let full_path = resources_dir.join(remote_path);
        if !full_path.starts_with(resources_dir) {
            continue;
        }
        if full_path.exists() {
            continue;
        }
        let resp = client
            .get(format!("{base_url}/sync/attachment"))
            .bearer_auth(&token)
            .query(&[("path", remote_path.as_str())])
            .send()
            .await;
        match resp {
            Ok(r) if r.status().is_success() => {
                if let Ok(bytes) = r.bytes().await {
                    if bytes.len() > 50 * 1024 * 1024 {
                        log::warn!("sync: skipping oversized attachment ({} bytes)", bytes.len());
                        continue;
                    }
                    if let Some(parent) = full_path.parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    if std::fs::write(&full_path, &bytes).is_ok() {
                        attachments_transferred += 1;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(SyncResult {
        notes_pulled,
        notes_pushed,
        labels_merged: reconcile.labels.len(),
        attachments_transferred,
        peer_name: peer.device_name.clone(),
    })
}

fn load_sync_note_from_conn(
    conn: &rusqlite::Connection,
    note_id: &str,
) -> Result<Option<protocol::SyncNote>, String> {
    let note = match conn.query_row(
        "SELECT id, kind, title, body, color, pinned, archived, trashed, \
         position, created_at, updated_at, trashed_at, vault, background_pattern \
         FROM notes WHERE id = ?1",
        params![note_id],
        |r| {
            Ok(crate::commands::Note {
                id: r.get(0)?,
                kind: r.get(1)?,
                title: r.get(2)?,
                body: r.get(3)?,
                color: r.get(4)?,
                pinned: r.get(5)?,
                archived: r.get(6)?,
                trashed: r.get(7)?,
                position: r.get(8)?,
                created_at: r.get(9)?,
                updated_at: r.get(10)?,
                trashed_at: r.get(11)?,
                checklist: Vec::new(),
                labels: Vec::new(),
                attachments: Vec::new(),
                vault_attachment_count: 0,
                vault: r.get(12)?,
                background_pattern: r.get(13)?,
            })
        },
    ) {
        Ok(n) => n,
        Err(_) => return Ok(None),
    };

    let mut stmt = conn
        .prepare(
            "SELECT id, text, checked, position, parent_id \
             FROM checklist_items WHERE note_id = ?1 ORDER BY position",
        )
        .map_err(|e| e.to_string())?;
    let checklist: Vec<crate::commands::ChecklistItem> = stmt
        .query_map(params![note_id], |r| {
            Ok(crate::commands::ChecklistItem {
                id: r.get(0)?,
                text: r.get(1)?,
                checked: r.get(2)?,
                position: r.get(3)?,
                parent_id: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .filter_map(|r| r.ok())
        .collect();

    let mut note = note;
    note.checklist = checklist;

    let labels_names: Vec<String> = {
        let mut stmt = conn
            .prepare(
                "SELECT l.name FROM labels l \
                 JOIN note_labels nl ON nl.label_id = l.id \
                 WHERE nl.note_id = ?1",
            )
            .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![note_id], |r| r.get(0))
            .map_err(|e| e.to_string())?;
        rows.filter_map(|r| r.ok()).collect()
    };

    let reminder = conn
        .query_row(
            "SELECT fire_at, rrule, snooze_until, fired_at, dismissed_at, created_at \
             FROM reminders WHERE note_id = ?1",
            params![note_id],
            |r| {
                Ok(protocol::SyncReminder {
                    fire_at: r.get(0)?,
                    rrule: r.get(1)?,
                    snooze_until: r.get(2)?,
                    fired_at: r.get(3)?,
                    dismissed_at: r.get(4)?,
                    created_at: r.get(5)?,
                })
            },
        )
        .ok();

    Ok(Some(protocol::SyncNote {
        note,
        labels_names,
        reminder,
    }))
}
