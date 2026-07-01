use super::SyncPeer;
use chrono::Utc;
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

const SERVICE_TYPE: &str = "_keepr-sync._tcp.local.";

pub fn register_service(
    device_id: &str,
    device_name: &str,
    port: u16,
) -> Result<ServiceDaemon, String> {
    let daemon = ServiceDaemon::new()
        .map_err(|e| format!("mDNS daemon failed: {e}"))?;
    let mut props = HashMap::new();
    props.insert("device_id".to_string(), device_id.to_string());
    props.insert("device_name".to_string(), device_name.to_string());
    let instance = format!("Keepr-{}", &device_id[..8.min(device_id.len())]);
    let info = ServiceInfo::new(
        SERVICE_TYPE,
        &instance,
        &format!("{instance}.local."),
        "",
        port,
        props,
    )
    .map_err(|e| format!("service info: {e}"))?;
    daemon
        .register(info)
        .map_err(|e| format!("register: {e}"))?;
    Ok(daemon)
}

pub fn start_browser(
    peers: Arc<Mutex<HashMap<String, SyncPeer>>>,
    own_device_id: String,
) -> Result<ServiceDaemon, String> {
    let daemon = ServiceDaemon::new()
        .map_err(|e| format!("mDNS browser daemon failed: {e}"))?;
    let receiver = daemon
        .browse(SERVICE_TYPE)
        .map_err(|e| format!("browse: {e}"))?;
    std::thread::spawn(move || {
        while let Ok(event) = receiver.recv() {
            match event {
                ServiceEvent::ServiceResolved(info) => {
                    let device_id = info
                        .get_properties()
                        .get("device_id")
                        .map(|v| v.val_str().to_string())
                        .unwrap_or_default();
                    if device_id.is_empty() || device_id == own_device_id {
                        continue;
                    }
                    let device_name = info
                        .get_properties()
                        .get("device_name")
                        .map(|v| v.val_str().to_string())
                        .unwrap_or_else(|| "Unknown".to_string());
                    let host = info
                        .get_addresses()
                        .iter()
                        .next()
                        .map(|a| a.to_string())
                        .unwrap_or_default();
                    if host.is_empty() {
                        continue;
                    }
                    let peer = SyncPeer {
                        device_id: device_id.clone(),
                        device_name,
                        host,
                        port: info.get_port(),
                        last_seen: Utc::now().to_rfc3339(),
                    };
                    peers.lock().insert(device_id, peer);
                }
                ServiceEvent::ServiceRemoved(_, fullname) => {
                    let mut map = peers.lock();
                    map.retain(|_, p| {
                        !fullname.contains(&p.device_id[..8.min(p.device_id.len())])
                    });
                }
                _ => {}
            }
        }
    });
    Ok(daemon)
}
