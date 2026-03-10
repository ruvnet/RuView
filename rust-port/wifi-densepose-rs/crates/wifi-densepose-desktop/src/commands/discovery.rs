use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use mdns_sd::{ServiceDaemon, ServiceEvent};
use serde::Serialize;
use tauri::State;
use tokio::time::timeout;
use tokio_serial::available_ports;
use flume::RecvTimeoutError;

use crate::domain::node::{
    Chip, DiscoveredNode, DiscoveryMethod, HealthStatus, MacAddress, MeshRole,
    NodeCapabilities, NodeRegistry,
};
use crate::state::AppState;

/// Service type for RuView ESP32 nodes using mDNS.
const MDNS_SERVICE_TYPE: &str = "_ruview._udp.local.";

/// UDP broadcast port for node discovery.
const UDP_DISCOVERY_PORT: u16 = 5006;

/// Discovery beacon magic bytes.
const BEACON_MAGIC: &[u8] = b"RUVIEW_BEACON";

/// Discover ESP32 CSI nodes on the local network via mDNS + UDP broadcast.
///
/// Discovery strategy:
/// 1. Start mDNS browser for `_ruview._udp.local.`
/// 2. Send UDP broadcast on port 5006
/// 3. Collect responses for `timeout_ms` milliseconds
/// 4. Deduplicate by MAC address and return merged results
#[tauri::command]
pub async fn discover_nodes(
    timeout_ms: Option<u64>,
    state: State<'_, AppState>,
) -> Result<Vec<DiscoveredNode>, String> {
    let timeout_duration = Duration::from_millis(timeout_ms.unwrap_or(3000));

    // Run mDNS and UDP discovery concurrently
    let (mdns_nodes, udp_nodes) = tokio::join!(
        discover_via_mdns(timeout_duration),
        discover_via_udp(timeout_duration),
    );

    // Merge results, deduplicating by MAC address
    let mut registry = NodeRegistry::new();

    for node in mdns_nodes.unwrap_or_default() {
        if let Some(ref mac) = node.mac {
            registry.upsert(MacAddress::new(mac), node);
        }
    }

    for node in udp_nodes.unwrap_or_default() {
        if let Some(ref mac) = node.mac {
            registry.upsert(MacAddress::new(mac), node);
        }
    }

    let nodes: Vec<DiscoveredNode> = registry.all().into_iter().cloned().collect();

    // Update global state
    {
        let mut discovery = state.discovery.lock().map_err(|e| e.to_string())?;
        discovery.nodes = nodes.clone();
    }

    Ok(nodes)
}

/// Discover nodes via mDNS (Bonjour/Avahi).
async fn discover_via_mdns(timeout_duration: Duration) -> Result<Vec<DiscoveredNode>, String> {
    let discovery_task = tokio::task::spawn_blocking(move || {
        let mdns = match ServiceDaemon::new() {
            Ok(daemon) => daemon,
            Err(e) => {
                tracing::warn!("Failed to create mDNS daemon: {}", e);
                return Vec::new();
            }
        };

        let receiver = match mdns.browse(MDNS_SERVICE_TYPE) {
            Ok(rx) => rx,
            Err(e) => {
                tracing::warn!("Failed to browse mDNS services: {}", e);
                return Vec::new();
            }
        };

        let mut discovered = Vec::new();
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(ServiceEvent::ServiceResolved(info)) => {
                    let props = info.get_properties();
                    let chip_str = props.get("chip").map(|v| v.val_str());
                    let chip = match chip_str {
                        Some("esp32s2") => Chip::Esp32s2,
                        Some("esp32s3") => Chip::Esp32s3,
                        Some("esp32c3") => Chip::Esp32c3,
                        Some("esp32c6") => Chip::Esp32c6,
                        _ => Chip::Esp32,
                    };
                    let role_str = props.get("role").map(|v| v.val_str());
                    let mesh_role = match role_str {
                        Some("coordinator") => MeshRole::Coordinator,
                        Some("aggregator") => MeshRole::Aggregator,
                        _ => MeshRole::Node,
                    };
                    let node = DiscoveredNode {
                        ip: info.get_addresses()
                            .iter()
                            .next()
                            .map(|a| a.to_string())
                            .unwrap_or_default(),
                        mac: props.get("mac").map(|v| v.val_str().to_string()),
                        hostname: Some(info.get_hostname().to_string()),
                        node_id: props.get("node_id")
                            .and_then(|v| v.val_str().parse().ok())
                            .unwrap_or(0),
                        firmware_version: props.get("version").map(|v| v.val_str().to_string()),
                        health: HealthStatus::Online,
                        last_seen: chrono::Utc::now().to_rfc3339(),
                        chip,
                        mesh_role,
                        discovery_method: DiscoveryMethod::Mdns,
                        tdm_slot: props.get("tdm_slot").and_then(|v| v.val_str().parse().ok()),
                        tdm_total: props.get("tdm_total").and_then(|v| v.val_str().parse().ok()),
                        edge_tier: props.get("edge_tier").and_then(|v| v.val_str().parse().ok()),
                        uptime_secs: props.get("uptime").and_then(|v| v.val_str().parse().ok()),
                        capabilities: Some(NodeCapabilities {
                            wasm: props.get("wasm").map(|v| v.val_str() == "1").unwrap_or(false),
                            ota: props.get("ota").map(|v| v.val_str() == "1").unwrap_or(true),
                            csi: props.get("csi").map(|v| v.val_str() == "1").unwrap_or(true),
                        }),
                        friendly_name: props.get("name").map(|v| v.val_str().to_string()),
                        notes: None,
                    };
                    discovered.push(node);
                }
                Ok(ServiceEvent::SearchStarted(_)) => {}
                Ok(_) => {}
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        // Stop browsing
        let _ = mdns.stop_browse(MDNS_SERVICE_TYPE);

        discovered
    });

    match timeout(timeout_duration + Duration::from_millis(500), discovery_task).await {
        Ok(Ok(nodes)) => Ok(nodes),
        Ok(Err(e)) => Err(format!("mDNS discovery task failed: {}", e)),
        Err(_) => Ok(Vec::new()), // Timeout, return empty
    }
}

/// Discover nodes via UDP broadcast beacon.
async fn discover_via_udp(timeout_duration: Duration) -> Result<Vec<DiscoveredNode>, String> {
    let discovery_task = tokio::task::spawn_blocking(move || -> Vec<DiscoveredNode> {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("Failed to bind UDP socket: {}", e);
                return Vec::new();
            }
        };

        if let Err(e) = socket.set_broadcast(true) {
            tracing::warn!("Failed to enable broadcast: {}", e);
            return Vec::new();
        }

        if let Err(e) = socket.set_read_timeout(Some(Duration::from_millis(100))) {
            tracing::warn!("Failed to set read timeout: {}", e);
            return Vec::new();
        }

        // Send discovery beacon
        let broadcast_addr = format!("255.255.255.255:{}", UDP_DISCOVERY_PORT);
        if let Err(e) = socket.send_to(b"RUVIEW_DISCOVER", &broadcast_addr) {
            tracing::warn!("Failed to send discovery beacon: {}", e);
        }

        let mut discovered = Vec::new();
        let mut buf = [0u8; 256];
        let start = std::time::Instant::now();

        while start.elapsed() < timeout_duration {
            match socket.recv_from(&mut buf) {
                Ok((len, addr)) => {
                    if len >= BEACON_MAGIC.len() && &buf[..BEACON_MAGIC.len()] == BEACON_MAGIC {
                        // Parse beacon response: RUVIEW_BEACON|mac|node_id|version
                        if let Some(node) = parse_beacon_response(&buf[..len], addr) {
                            discovered.push(node);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(ref e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
                Err(_) => break,
            }
        }

        discovered
    });

    match timeout(timeout_duration + Duration::from_millis(500), discovery_task).await {
        Ok(Ok(nodes)) => Ok(nodes),
        Ok(Err(e)) => Err(format!("UDP discovery task failed: {}", e)),
        Err(_) => Ok(Vec::new()),
    }
}

/// Parse a UDP beacon response into a DiscoveredNode.
/// Format: RUVIEW_BEACON|<mac>|<node_id>|<version>|<chip>|<role>|<tdm_slot>|<tdm_total>
fn parse_beacon_response(data: &[u8], addr: SocketAddr) -> Option<DiscoveredNode> {
    let text = std::str::from_utf8(data).ok()?;
    let parts: Vec<&str> = text.split('|').collect();

    if parts.len() < 2 || parts[0] != "RUVIEW_BEACON" {
        return None;
    }

    let mac = parts.get(1).map(|s| s.to_string());
    let node_id = parts.get(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let version = parts.get(3).map(|s| s.to_string());
    let chip_str = parts.get(4).copied();
    let chip = match chip_str {
        Some("esp32s2") => Chip::Esp32s2,
        Some("esp32s3") => Chip::Esp32s3,
        Some("esp32c3") => Chip::Esp32c3,
        Some("esp32c6") => Chip::Esp32c6,
        _ => Chip::Esp32,
    };
    let role_str = parts.get(5).copied();
    let mesh_role = match role_str {
        Some("coordinator") => MeshRole::Coordinator,
        Some("aggregator") => MeshRole::Aggregator,
        _ => MeshRole::Node,
    };
    let tdm_slot = parts.get(6).and_then(|s| s.parse().ok());
    let tdm_total = parts.get(7).and_then(|s| s.parse().ok());

    Some(DiscoveredNode {
        ip: addr.ip().to_string(),
        mac,
        hostname: None,
        node_id,
        firmware_version: version,
        health: HealthStatus::Online,
        last_seen: chrono::Utc::now().to_rfc3339(),
        chip,
        mesh_role,
        discovery_method: DiscoveryMethod::UdpProbe,
        tdm_slot,
        tdm_total,
        edge_tier: None,
        uptime_secs: None,
        capabilities: Some(NodeCapabilities {
            wasm: false,
            ota: true,
            csi: true,
        }),
        friendly_name: None,
        notes: None,
    })
}

/// List available serial ports on this machine.
/// Filters for known ESP32 USB-to-serial chips (CP2102, CH340, FTDI).
#[tauri::command]
pub async fn list_serial_ports() -> Result<Vec<SerialPortInfo>, String> {
    let ports = available_ports().map_err(|e| format!("Failed to enumerate ports: {}", e))?;

    let mut result = Vec::new();

    for port in ports {
        let info = match port.port_type {
            tokio_serial::SerialPortType::UsbPort(usb_info) => {
                SerialPortInfo {
                    name: port.port_name,
                    vid: Some(usb_info.vid),
                    pid: Some(usb_info.pid),
                    manufacturer: usb_info.manufacturer,
                    serial_number: usb_info.serial_number,
                    is_esp32_compatible: is_esp32_compatible(usb_info.vid, usb_info.pid),
                }
            }
            _ => {
                SerialPortInfo {
                    name: port.port_name,
                    vid: None,
                    pid: None,
                    manufacturer: None,
                    serial_number: None,
                    is_esp32_compatible: false,
                }
            }
        };

        result.push(info);
    }

    // Sort ESP32-compatible ports first
    result.sort_by(|a, b| b.is_esp32_compatible.cmp(&a.is_esp32_compatible));

    Ok(result)
}

/// Check if a USB VID/PID is from a known ESP32 USB-to-serial chip.
fn is_esp32_compatible(vid: u16, pid: u16) -> bool {
    // CP210x (Silicon Labs)
    if vid == 0x10C4 && (pid == 0xEA60 || pid == 0xEA70) {
        return true;
    }
    // CH340/CH341 (QinHeng)
    if vid == 0x1A86 && (pid == 0x7523 || pid == 0x5523) {
        return true;
    }
    // FTDI
    if vid == 0x0403 && (pid == 0x6001 || pid == 0x6010 || pid == 0x6011 || pid == 0x6014 || pid == 0x6015) {
        return true;
    }
    // ESP32-S2/S3 native USB
    if vid == 0x303A {
        return true;
    }
    false
}

#[derive(Debug, Clone, Serialize)]
pub struct SerialPortInfo {
    pub name: String,
    pub vid: Option<u16>,
    pub pid: Option<u16>,
    pub manufacturer: Option<String>,
    pub serial_number: Option<String>,
    pub is_esp32_compatible: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_beacon_response() {
        let data = b"RUVIEW_BEACON|AA:BB:CC:DD:EE:FF|1|0.3.0|esp32s3|coordinator|0|4";
        let addr: SocketAddr = "192.168.1.100:5006".parse().unwrap();

        let node = parse_beacon_response(data, addr).unwrap();
        assert_eq!(node.ip, "192.168.1.100");
        assert_eq!(node.mac, Some("AA:BB:CC:DD:EE:FF".to_string()));
        assert_eq!(node.node_id, 1);
        assert_eq!(node.firmware_version, Some("0.3.0".to_string()));
        assert_eq!(node.chip, Chip::Esp32s3);
        assert_eq!(node.mesh_role, MeshRole::Coordinator);
        assert_eq!(node.tdm_slot, Some(0));
        assert_eq!(node.tdm_total, Some(4));
    }

    #[test]
    fn test_is_esp32_compatible() {
        // CP2102
        assert!(is_esp32_compatible(0x10C4, 0xEA60));
        // CH340
        assert!(is_esp32_compatible(0x1A86, 0x7523));
        // ESP32-S3 native
        assert!(is_esp32_compatible(0x303A, 0x1001));
        // Unknown
        assert!(!is_esp32_compatible(0x0000, 0x0000));
    }
}
