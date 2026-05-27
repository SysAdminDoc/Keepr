//! v0.24.0 — Web Clipper.
//!
//! Runs a tiny localhost HTTP server (random port, 127.0.0.1 only) so
//! a browser extension can save the current page / selection / URL as
//! a new note in Keepr. The token is a per-install 256-bit secret
//! generated on first launch and persisted in `app_settings`. The
//! extension MUST present it as `Authorization: Bearer <token>`.
//!
//! Hard rules:
//!   - No internet binding. EVER. We bind 127.0.0.1:0 (OS picks the
//!     port) and the server never has to reach out.
//!   - No port file. The token + port live in `app_settings`; the user
//!     pastes both into the extension's Options page manually. An
//!     auto-discovery handshake would let any local process steal both.
//!   - Constant-time token comparison via `subtle::ConstantTimeEq`.
//!   - CORS only for `chrome-extension://`, `moz-extension://`, and
//!     `http://127.0.0.1:*` (last one so the browser can preflight).
//!   - Content-Length cap: ~4 MB. A typical Readability'd page is
//!     20-80 KB; we cap at 4 MB so a malicious page can't OOM us via
//!     a 500 MB body.

use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use subtle::ConstantTimeEq;
use tower_http::cors::{AllowOrigin, CorsLayer};

const TOKEN_SETTING_KEY: &str = "web_clipper_token";
const PORT_SETTING_KEY: &str = "web_clipper_port";
const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
/// Header used by the extension to present the bearer token.
const AUTH_HEADER: &str = "authorization";

/// Live runtime state. Stored in AppState so the renderer can read the
/// port + token via Tauri commands. `None` until the server has bound
/// successfully on startup.
#[derive(Clone, Debug, Default)]
pub struct WebClipperInfo {
    pub port: Option<u16>,
    pub token: Option<String>,
}

pub type WebClipperState = Arc<Mutex<WebClipperInfo>>;

/// Resolve the current bearer token, generating + persisting one if
/// none exists. 32 random bytes hex-encoded → 64-char string.
pub fn ensure_token(conn: &rusqlite::Connection) -> Result<String, String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT value FROM app_settings WHERE key = ?1",
            rusqlite::params![TOKEN_SETTING_KEY],
            |r| r.get::<_, String>(0),
        )
        .ok();
    if let Some(t) = existing {
        if t.len() == 64 {
            return Ok(t);
        }
        // Old / malformed token — regenerate.
    }
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("getrandom failed: {e}"))?;
    let token = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![TOKEN_SETTING_KEY, token],
    )
    .map_err(|e| e.to_string())?;
    Ok(token)
}

/// Force-generate a new token, invalidating any previously paired
/// extensions. Settings UI exposes this via the "Regenerate" button.
pub fn regenerate_token(conn: &rusqlite::Connection) -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("getrandom failed: {e}"))?;
    let token = bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();
    conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![TOKEN_SETTING_KEY, token],
    )
    .map_err(|e| e.to_string())?;
    Ok(token)
}

fn persist_port(conn: &rusqlite::Connection, port: u16) {
    let _ = conn.execute(
        "INSERT INTO app_settings(key, value) VALUES(?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![PORT_SETTING_KEY, port.to_string()],
    );
}

#[derive(Clone)]
struct ServerState {
    db: Arc<Mutex<rusqlite::Connection>>,
    token: Arc<String>,
}

/// Payload for `/clip` (full-page article extracted on the extension
/// side via Readability + Turndown) and `/clip/selection` (current
/// selection only, also rendered to markdown). `/clip/url` uses the
/// same shape with `markdown = ""`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClipPayload {
    url: String,
    title: String,
    #[serde(default)]
    markdown: String,
    #[serde(default)]
    excerpt: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClipResponse {
    ok: bool,
    note_id: String,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    error: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

fn unauthorized() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody { error: "unauthorized", detail: None }),
    )
        .into_response()
}

fn check_auth(headers: &HeaderMap, expected: &str) -> bool {
    let Some(raw) = headers.get(AUTH_HEADER) else {
        return false;
    };
    let Ok(s) = raw.to_str() else {
        return false;
    };
    let Some(token) = s.strip_prefix("Bearer ").or_else(|| s.strip_prefix("bearer ")) else {
        return false;
    };
    // Constant-time compare prevents timing oracle on the 64-char token.
    token.as_bytes().ct_eq(expected.as_bytes()).into()
}

async fn health(State(_): State<ServerState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "ok": true,
        "app": "keepr",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

/// Insert a note built from the clip payload. Title comes from the
/// payload; body is a markdown blob with the URL as the first line
/// (so the editor "Mentions" footer renders an opener), followed by
/// the excerpt (if any) and the page markdown. Tags are added as
/// labels.
fn insert_clipped_note(
    conn: &rusqlite::Connection,
    payload: &ClipPayload,
) -> Result<String, rusqlite::Error> {
    let new_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();
    let mut body = String::new();
    // First line: URL marker the user can click in the editor. Also
    // surfaces in the card preview as the source.
    body.push_str(&format!("Source: {}\n", payload.url));
    if let Some(excerpt) = &payload.excerpt {
        if !excerpt.is_empty() {
            body.push_str("\n");
            body.push_str(excerpt.trim());
            body.push_str("\n");
        }
    }
    if !payload.markdown.is_empty() {
        body.push_str("\n---\n\n");
        body.push_str(&payload.markdown);
    }
    // Truncate to the 64 KiB body cap (matches MAX_BODY_BYTES in
    // commands.rs::validate_note_input). Clipped pages occasionally
    // exceed this — better to truncate than reject.
    const BODY_CAP: usize = 64 * 1024;
    if body.len() > BODY_CAP {
        body.truncate(BODY_CAP);
        body.push_str("\n\n[... truncated; original page is longer than Keepr's per-note cap]");
    }

    conn.execute(
        "INSERT INTO notes (id, kind, title, body, color, pinned, archived, trashed,
                            position, created_at, updated_at)
         VALUES (?1, 'text', ?2, ?3, 'default', 0, 0, 0, 0, ?4, ?4)",
        rusqlite::params![new_id, payload.title.trim(), body, now],
    )?;

    // Apply tags as labels (creating them if missing). This matches
    // the extension's stated "tags" array contract; user-provided tag
    // strings are trimmed and case-folded against existing labels.
    for tag in &payload.tags {
        let tag = tag.trim();
        if tag.is_empty() { continue; }
        // Find or create label.
        let label_id: String = conn
            .query_row(
                "SELECT id FROM labels WHERE LOWER(name) = LOWER(?1)",
                rusqlite::params![tag],
                |r| r.get::<_, String>(0),
            )
            .unwrap_or_else(|_| {
                let id = uuid::Uuid::new_v4().to_string();
                let _ = conn.execute(
                    "INSERT OR IGNORE INTO labels (id, name) VALUES (?1, ?2)",
                    rusqlite::params![id, tag],
                );
                id
            });
        let _ = conn.execute(
            "INSERT OR IGNORE INTO note_labels (note_id, label_id) VALUES (?1, ?2)",
            rusqlite::params![new_id, label_id],
        );
    }
    Ok(new_id)
}

async fn clip_url(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(payload): Json<ClipPayload>,
) -> axum::response::Response {
    if !check_auth(&headers, &state.token) {
        return unauthorized();
    }
    if payload.url.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody { error: "invalid_payload", detail: Some("url required".into()) }),
        )
            .into_response();
    }
    let id = {
        let conn = state.db.lock();
        match insert_clipped_note(&conn, &payload) {
            Ok(id) => id,
            Err(e) => {
                log::error!("web_clipper: insert failed: {e}");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody { error: "db_write_failed", detail: Some(e.to_string()) }),
                )
                    .into_response();
            }
        }
    };
    log::info!("web_clipper: clipped {} -> note {}", payload.url, id);
    Json(ClipResponse { ok: true, note_id: id }).into_response()
}

/// Build the axum router. Same handler for `/clip`, `/clip/selection`,
/// and `/clip/url` for v1 — the extension picks what to send.
fn router(state: ServerState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _req_parts| {
            origin
                .to_str()
                .map(|s| {
                    s.starts_with("chrome-extension://")
                        || s.starts_with("moz-extension://")
                        || s.starts_with("http://127.0.0.1:")
                        || s == "http://127.0.0.1"
                })
                .unwrap_or(false)
        }))
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ]);

    Router::new()
        .route("/health", get(health))
        .route("/clip", post(clip_url))
        .route("/clip/url", post(clip_url))
        .route("/clip/selection", post(clip_url))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(cors)
        .with_state(state)
}

/// Bind on 127.0.0.1:0 (OS picks the port), persist the chosen port +
/// the bearer token in `app_settings`, and `tokio::spawn` the axum
/// server. Returns the port so the renderer can display it.
///
/// Caller is responsible for keeping the spawned task alive — we
/// don't return a join handle because Tauri's main loop owns the
/// app lifetime; the OS will reap the listener on process exit.
pub async fn start_server(
    db: Arc<Mutex<rusqlite::Connection>>,
    info: WebClipperState,
) -> Result<u16, String> {
    let token = {
        let conn = db.lock();
        ensure_token(&conn)?
    };
    let addr: SocketAddr = "127.0.0.1:0".parse().unwrap();
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("could not bind web clipper port: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();
    {
        let conn = db.lock();
        persist_port(&conn, port);
    }
    {
        let mut guard = info.lock();
        guard.port = Some(port);
        guard.token = Some(token.clone());
    }
    let state = ServerState {
        db,
        token: Arc::new(token),
    };
    let app = router(state);
    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            log::error!("web_clipper: server crashed: {e}");
        }
    });
    log::info!("web_clipper: serving on http://127.0.0.1:{port}");
    Ok(port)
}

// --- unit tests -----------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh_db() -> Connection {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        crate::db::migrate(&mut conn).unwrap();
        conn
    }

    #[test]
    fn ensure_token_generates_64_hex_chars() {
        let conn = fresh_db();
        let t = ensure_token(&conn).unwrap();
        assert_eq!(t.len(), 64);
        assert!(t.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn ensure_token_returns_same_token_on_second_call() {
        let conn = fresh_db();
        let t1 = ensure_token(&conn).unwrap();
        let t2 = ensure_token(&conn).unwrap();
        assert_eq!(t1, t2);
    }

    #[test]
    fn regenerate_returns_different_token() {
        let conn = fresh_db();
        let t1 = ensure_token(&conn).unwrap();
        let t2 = regenerate_token(&conn).unwrap();
        assert_ne!(t1, t2);
        // ensure_token now returns the regenerated one.
        let t3 = ensure_token(&conn).unwrap();
        assert_eq!(t2, t3);
    }

    #[test]
    fn insert_clipped_note_creates_text_note_with_source_line() {
        let conn = fresh_db();
        let payload = ClipPayload {
            url: "https://example.com/article".into(),
            title: "Example Article".into(),
            markdown: "# Heading\n\nBody.".into(),
            excerpt: Some("Short summary.".into()),
            tags: vec!["clipped".into(), "web".into()],
        };
        let id = insert_clipped_note(&conn, &payload).unwrap();
        let (title, body, kind): (String, String, String) = conn
            .query_row(
                "SELECT title, body, kind FROM notes WHERE id = ?1",
                rusqlite::params![id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(title, "Example Article");
        assert_eq!(kind, "text");
        assert!(body.starts_with("Source: https://example.com/article"));
        assert!(body.contains("Short summary."));
        assert!(body.contains("# Heading"));

        // Labels were created + attached.
        let label_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM note_labels WHERE note_id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(label_count, 2);
    }

    #[test]
    fn insert_clipped_note_truncates_oversize_bodies() {
        let conn = fresh_db();
        let huge = "x".repeat(80 * 1024);
        let payload = ClipPayload {
            url: "https://example.com/".into(),
            title: "Big".into(),
            markdown: huge,
            excerpt: None,
            tags: vec![],
        };
        let id = insert_clipped_note(&conn, &payload).unwrap();
        let body: String = conn
            .query_row(
                "SELECT body FROM notes WHERE id = ?1",
                rusqlite::params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert!(body.ends_with("[... truncated; original page is longer than Keepr's per-note cap]"));
        assert!(body.len() <= 64 * 1024 + 100);
    }

    #[test]
    fn check_auth_rejects_missing_header() {
        let headers = HeaderMap::new();
        assert!(!check_auth(&headers, "abc"));
    }

    #[test]
    fn check_auth_rejects_wrong_token() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, "Bearer xyz".parse().unwrap());
        assert!(!check_auth(&headers, "abc"));
    }

    #[test]
    fn check_auth_accepts_correct_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, "Bearer abc".parse().unwrap());
        assert!(check_auth(&headers, "abc"));
    }

    #[test]
    fn check_auth_case_insensitive_on_bearer_prefix() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, "bearer abc".parse().unwrap());
        assert!(check_auth(&headers, "abc"));
    }

    #[test]
    fn check_auth_rejects_non_bearer_scheme() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTH_HEADER, "Basic abc".parse().unwrap());
        assert!(!check_auth(&headers, "abc"));
    }
}
