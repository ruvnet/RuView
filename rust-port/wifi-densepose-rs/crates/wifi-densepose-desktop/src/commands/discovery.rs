use serde::Serialize;

use crate::domain::node::DiscoveredNode;

/// Discover ESP32 CSI nodes on the local network via mDNS / UDP broadcast.
#[tauri::command]
pub async fn discover_nodes(timeout_ms: Option<u64>) -> Result<Vec<DiscoveredNode>, String> {
    let _timeout = timeout_ms.unwrap_or(3000);
    // Stub: return placeholder data
    Ok(vec![DiscoveredNode {
        ip: "192.168.1.100".into(),
        mac: Some("AA:BB:CC:DD:EE:FF".into()),
        hostname: Some("ruview-node-1".into()),
        node_id: 1,
        firmware_version: Some("0.3.0".into()),
        health: crate::domain::node::HealthStatus::Online,
        last_seen: chrono::Utc::now().to_rfc3339(),
    }])
}

/// List available serial ports on this machine.
#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    // Stub: return empty list
    Ok(vec![])
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub manufacturer: Option<String>,
}
