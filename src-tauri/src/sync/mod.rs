pub mod discovery;
pub mod protocol;
pub mod server;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPeer {
    pub device_id: String,
    pub device_name: String,
    pub host: String,
    pub port: u16,
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncSettings {
    pub enabled: bool,
    pub device_id: String,
    pub device_name: String,
    pub port: Option<u16>,
    pub last_sync: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SyncStatus {
    Disabled,
    Idle,
    Syncing,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub notes_pulled: usize,
    pub notes_pushed: usize,
    pub labels_merged: usize,
    pub attachments_transferred: usize,
    pub peer_name: String,
}

pub struct SyncState {
    pub peers: Arc<Mutex<HashMap<String, SyncPeer>>>,
    pub status: Arc<Mutex<SyncStatus>>,
    pub port: Arc<Mutex<Option<u16>>>,
}

impl Default for SyncState {
    fn default() -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            status: Arc::new(Mutex::new(SyncStatus::Disabled)),
            port: Arc::new(Mutex::new(None)),
        }
    }
}
