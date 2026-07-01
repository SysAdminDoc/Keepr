use axum::extract::State as AxumState;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;

use super::protocol;

#[derive(Clone)]
pub struct SyncServerState {
    pub db: Arc<Mutex<Connection>>,
    pub resources_dir: PathBuf,
    pub device_id: String,
    pub device_name: String,
    pub token: String,
}

fn check_bearer(headers: &HeaderMap, expected: &str) -> Result<(), (StatusCode, String)> {
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let provided = auth
        .strip_prefix("Bearer ")
        .or_else(|| auth.strip_prefix("bearer "))
        .unwrap_or("");
    if provided.len() != expected.len()
        || !bool::from(provided.as_bytes().ct_eq(expected.as_bytes()))
    {
        return Err((StatusCode::UNAUTHORIZED, "invalid token".into()));
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InfoResponse {
    device_id: String,
    device_name: String,
    note_count: i64,
}

async fn info(
    AxumState(state): AxumState<SyncServerState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    check_bearer(&headers, &state.token)?;
    let count: i64 = {
        let conn = state.db.lock();
        conn.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0))
            .unwrap_or(0)
    };
    Ok(Json(InfoResponse {
        device_id: state.device_id.clone(),
        device_name: state.device_name.clone(),
        note_count: count,
    }))
}

async fn reconcile_handler(
    AxumState(state): AxumState<SyncServerState>,
    headers: HeaderMap,
    Json(req): Json<protocol::ReconcileRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    check_bearer(&headers, &state.token)?;
    let conn = state.db.lock();
    let resp =
        protocol::reconcile(&conn, &state.resources_dir, &state.device_id, &req.state_vector)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(resp))
}

async fn push_handler(
    AxumState(state): AxumState<SyncServerState>,
    headers: HeaderMap,
    Json(req): Json<protocol::PushRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    check_bearer(&headers, &state.token)?;
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

use super::protocol::is_safe_sync_resource_path;

async fn get_attachment(
    AxumState(state): AxumState<SyncServerState>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<AttachmentQuery>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    check_bearer(&headers, &state.token)?;
    if !is_safe_sync_resource_path(&q.path) {
        return Ok((StatusCode::BAD_REQUEST, Vec::new()).into_response());
    }
    let file_path = state.resources_dir.join(&q.path);
    if !file_path.starts_with(&state.resources_dir) {
        return Ok((StatusCode::BAD_REQUEST, Vec::new()).into_response());
    }
    match tokio::fs::read(&file_path).await {
        Ok(bytes) => Ok((StatusCode::OK, bytes).into_response()),
        Err(_) => Ok((StatusCode::NOT_FOUND, Vec::new()).into_response()),
    }
}

pub async fn start(
    db: Arc<Mutex<Connection>>,
    resources_dir: PathBuf,
    device_id: String,
    device_name: String,
    token: String,
) -> Result<u16, String> {
    let state = SyncServerState {
        db,
        resources_dir,
        device_id,
        device_name,
        token,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_path_accepts_content_addressed() {
        let hash = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
        assert!(is_safe_sync_resource_path(&format!("ab/cd/{hash}.png")));
        assert!(is_safe_sync_resource_path(&format!(
            "ab/cd/{hash}.thumb.jpg"
        )));
    }

    #[test]
    fn safe_path_rejects_traversal() {
        assert!(!is_safe_sync_resource_path("../../etc/passwd"));
        assert!(!is_safe_sync_resource_path("..\\..\\Windows\\System32"));
    }

    #[test]
    fn safe_path_rejects_absolute() {
        assert!(!is_safe_sync_resource_path(
            "C:\\Windows\\System32\\config\\SAM"
        ));
        assert!(!is_safe_sync_resource_path("/etc/passwd"));
    }

    #[test]
    fn safe_path_rejects_flat_filename() {
        assert!(!is_safe_sync_resource_path("somefile.png"));
    }

    #[test]
    fn safe_path_rejects_empty() {
        assert!(!is_safe_sync_resource_path(""));
    }
}
