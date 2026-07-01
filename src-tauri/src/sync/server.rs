use axum::extract::State as AxumState;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;

use super::protocol;

#[derive(Clone)]
pub struct SyncServerState {
    pub db: Arc<Mutex<Connection>>,
    pub resources_dir: PathBuf,
    pub device_id: String,
    pub device_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    device_id: String,
    device_name: String,
    note_count: i64,
}

async fn info(AxumState(state): AxumState<SyncServerState>) -> impl IntoResponse {
    let count: i64 = {
        let conn = state.db.lock();
        conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap_or(0)
    };
    Json(InfoResponse {
        device_id: state.device_id.clone(),
        device_name: state.device_name.clone(),
        note_count: count,
    })
}

async fn reconcile_handler(
    AxumState(state): AxumState<SyncServerState>,
    Json(req): Json<protocol::ReconcileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = state.db.lock();
    let resp =
        protocol::reconcile(&conn, &state.resources_dir, &state.device_id, &req.state_vector)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(resp))
}

async fn push_handler(
    AxumState(state): AxumState<SyncServerState>,
    Json(req): Json<protocol::PushRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let conn = state.db.lock();
    let (notes_applied, labels_merged) =
        protocol::apply_pushed_notes(&conn, &req.notes, &req.labels)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(PushResponse {
        notes_applied,
        labels_merged,
    }))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PushResponse {
    notes_applied: usize,
    labels_merged: usize,
}

#[derive(Deserialize)]
struct AttachmentQuery {
    path: String,
}

async fn get_attachment(
    AxumState(state): AxumState<SyncServerState>,
    axum::extract::Query(q): axum::extract::Query<AttachmentQuery>,
) -> impl IntoResponse {
    let file_path = state.resources_dir.join(&q.path);
    if !file_path.starts_with(&state.resources_dir) || q.path.contains("..") {
        return (StatusCode::BAD_REQUEST, Vec::new()).into_response();
    }
    match tokio::fs::read(&file_path).await {
        Ok(bytes) => (StatusCode::OK, bytes).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Vec::new()).into_response(),
    }
}

pub async fn start(
    db: Arc<Mutex<Connection>>,
    resources_dir: PathBuf,
    device_id: String,
    device_name: String,
) -> Result<u16, String> {
    let state = SyncServerState {
        db,
        resources_dir,
        device_id,
        device_name,
    };

    let app = Router::new()
        .route("/sync/info", get(info))
        .route("/sync/reconcile", post(reconcile_handler))
        .route("/sync/push", post(push_handler))
        .route("/sync/attachment", get(get_attachment))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .map_err(|e| format!("bind failed: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("local_addr: {e}"))?
        .port();

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("sync server error: {e}");
        }
    });

    Ok(port)
}
