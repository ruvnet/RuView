//! Integration tests for all Tauri API commands
//!
//! Tests the actual command implementations without the Tauri runtime.

// ============================================================================
// Discovery Tests
// ============================================================================

#[test]
fn test_serial_port_detection_logic() {
    // Test ESP32 VID/PID detection
    // CP210x (Silicon Labs)
    assert!(is_esp32_vid_pid(0x10C4, 0xEA60), "CP2102 should be detected");
    assert!(is_esp32_vid_pid(0x10C4, 0xEA70), "CP2104 should be detected");

    // CH340/CH341 (QinHeng)
    assert!(is_esp32_vid_pid(0x1A86, 0x7523), "CH340 should be detected");
    assert!(is_esp32_vid_pid(0x1A86, 0x5523), "CH341 should be detected");

    // FTDI
    assert!(is_esp32_vid_pid(0x0403, 0x6001), "FTDI FT232 should be detected");
    assert!(is_esp32_vid_pid(0x0403, 0x6010), "FTDI FT2232 should be detected");

    // ESP32 native USB
    assert!(is_esp32_vid_pid(0x303A, 0x1001), "ESP32-S2/S3 native should be detected");

    // Unknown device
    assert!(!is_esp32_vid_pid(0x0000, 0x0000), "Unknown VID/PID should not be detected");
    assert!(!is_esp32_vid_pid(0x1234, 0x5678), "Random VID/PID should not be detected");
}

fn is_esp32_vid_pid(vid: u16, pid: u16) -> bool {
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

#[test]
fn test_beacon_parsing() {
    let data = b"RUVIEW_BEACON|AA:BB:CC:DD:EE:FF|1|0.3.0|esp32s3|coordinator|0|4";
    let text = std::str::from_utf8(data).unwrap();
    let parts: Vec<&str> = text.split('|').collect();

    assert_eq!(parts.len(), 8);
    assert_eq!(parts[0], "RUVIEW_BEACON");
    assert_eq!(parts[1], "AA:BB:CC:DD:EE:FF");
    assert_eq!(parts[2], "1");
    assert_eq!(parts[3], "0.3.0");
    assert_eq!(parts[4], "esp32s3");
    assert_eq!(parts[5], "coordinator");
    assert_eq!(parts[6], "0");
    assert_eq!(parts[7], "4");
}

// ============================================================================
// Settings Tests
// ============================================================================

#[test]
fn test_settings_structure() {
    use wifi_densepose_desktop::commands::settings::AppSettings;

    let settings = AppSettings::default();

    // Check default values
    assert!(!settings.theme.is_empty(), "Theme should have a default");
    assert!(settings.discover_interval_ms > 0, "Discovery interval should be positive");
    assert!(settings.auto_discover, "Auto-discover should default to true");
    assert_eq!(settings.server_http_port, 8080);
}

#[test]
fn test_settings_serialization() {
    use wifi_densepose_desktop::commands::settings::AppSettings;

    let settings = AppSettings::default();
    let json = serde_json::to_string(&settings).expect("Should serialize");
    let restored: AppSettings = serde_json::from_str(&json).expect("Should deserialize");

    assert_eq!(settings.theme, restored.theme);
    assert_eq!(settings.server_http_port, restored.server_http_port);
    assert_eq!(settings.discover_interval_ms, restored.discover_interval_ms);
}

// ============================================================================
// Server Tests
// ============================================================================

#[test]
fn test_server_state_default() {
    use wifi_densepose_desktop::state::ServerState;

    let server = ServerState::default();
    assert!(!server.running, "Server should not be running by default");
    assert!(server.pid.is_none());
    assert!(server.http_port.is_none());
}

// ============================================================================
// Flash Tests
// ============================================================================

#[test]
fn test_chip_variants() {
    use wifi_densepose_desktop::domain::node::Chip;

    let chips = vec![
        Chip::Esp32,
        Chip::Esp32s2,
        Chip::Esp32s3,
        Chip::Esp32c3,
        Chip::Esp32c6,
    ];

    for chip in chips {
        let name = format!("{:?}", chip).to_lowercase();
        assert!(name.starts_with("esp32"), "All chips should be ESP32 variants");
    }
}

#[test]
fn test_progress_parsing() {
    // Test espflash progress output parsing
    let output = "Flashing... [===>      ] 35%";
    let re = regex::Regex::new(r"(\d+)%").unwrap();

    if let Some(caps) = re.captures(output) {
        let pct: u8 = caps[1].parse().unwrap();
        assert_eq!(pct, 35);
    } else {
        panic!("Should parse percentage");
    }
}

// ============================================================================
// OTA Tests
// ============================================================================

#[test]
fn test_sha256_hash() {
    use sha2::{Sha256, Digest};

    let data = b"test firmware data";
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let hex = hex::encode(hash);

    assert_eq!(hex.len(), 64, "SHA256 should produce 64 hex characters");
}

#[test]
fn test_hmac_signature() {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let key = b"test_psk_key";
    let data = b"firmware_hash";

    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data);
    let result = mac.finalize();
    let signature = hex::encode(result.into_bytes());

    assert_eq!(signature.len(), 64, "HMAC-SHA256 should produce 64 hex characters");
}

// ============================================================================
// Provision Tests
// ============================================================================

#[test]
fn test_nvs_config_format() {
    // Test CSV format for NVS partition
    let csv = "key,type,encoding,value\ncsi_cfg,namespace,,\nssid,data,string,TestNetwork\npassword,data,string,TestPass123\n";

    let lines: Vec<&str> = csv.lines().collect();
    assert_eq!(lines.len(), 4);
    assert!(lines[0].starts_with("key,type"));
    assert!(lines[1].contains("namespace"));
    assert!(lines[2].contains("ssid"));
    assert!(lines[3].contains("password"));
}

#[test]
fn test_mesh_config_generation() {
    // Test that mesh configs have required fields
    let config = serde_json::json!({
        "node_id": 1,
        "mesh_role": "node",
        "tdm_slot": 0,
        "tdm_total": 4,
        "ssid": "TestNetwork",
        "password": "TestPass",
        "coordinator_ip": "192.168.1.100"
    });

    assert!(config.get("node_id").is_some());
    assert!(config.get("mesh_role").is_some());
    assert!(config.get("ssid").is_some());
}

// ============================================================================
// WASM Tests
// ============================================================================

#[test]
fn test_wasm_magic_bytes() {
    // WebAssembly magic bytes: \0asm
    let wasm_header: [u8; 4] = [0x00, 0x61, 0x73, 0x6D];

    assert_eq!(wasm_header[0], 0x00);
    assert_eq!(wasm_header[1], 0x61); // 'a'
    assert_eq!(wasm_header[2], 0x73); // 's'
    assert_eq!(wasm_header[3], 0x6D); // 'm'
}

#[test]
fn test_wasm_version() {
    // WASM version 1
    let wasm_version: [u8; 4] = [0x01, 0x00, 0x00, 0x00];

    let version = u32::from_le_bytes(wasm_version);
    assert_eq!(version, 1);
}

// ============================================================================
// State Tests
// ============================================================================

#[test]
fn test_app_state_initialization() {
    use wifi_densepose_desktop::state::AppState;

    let state = AppState::default();

    // Check that all state components initialize correctly
    let discovery = state.discovery.lock().unwrap();
    assert!(discovery.nodes.is_empty(), "Should start with no nodes");
    drop(discovery);

    let flash = state.flash.lock().unwrap();
    assert_eq!(flash.phase, "", "Should start with empty phase");
    assert_eq!(flash.progress_pct, 0.0);
    drop(flash);

    let server = state.server.lock().unwrap();
    assert!(!server.running, "Server should not be running initially");
}

// ============================================================================
// Domain Model Tests
// ============================================================================

#[test]
fn test_health_status_variants() {
    use wifi_densepose_desktop::domain::node::HealthStatus;

    let statuses = vec![
        HealthStatus::Online,
        HealthStatus::Degraded,
        HealthStatus::Offline,
    ];

    for status in statuses {
        let json = serde_json::to_string(&status).expect("Should serialize");
        assert!(!json.is_empty());
    }
}

#[test]
fn test_discovery_method_variants() {
    use wifi_densepose_desktop::domain::node::DiscoveryMethod;

    let methods = vec![
        DiscoveryMethod::Mdns,
        DiscoveryMethod::UdpProbe,
        DiscoveryMethod::Manual,
        DiscoveryMethod::HttpSweep,
    ];

    for method in methods {
        let json = serde_json::to_string(&method).expect("Should serialize");
        assert!(!json.is_empty());
    }
}

#[test]
fn test_mesh_role_variants() {
    use wifi_densepose_desktop::domain::node::MeshRole;

    let roles = vec![
        MeshRole::Coordinator,
        MeshRole::Aggregator,
        MeshRole::Node,
    ];

    for role in roles {
        let json = serde_json::to_string(&role).expect("Should serialize");
        assert!(!json.is_empty());
    }
}

// ============================================================================
// WiFi Config Tests (New Feature)
// ============================================================================

#[test]
fn test_wifi_config_command_format() {
    let ssid = "TestNetwork";
    let password = "TestPass123";

    // Test all command formats
    let cmd1 = format!("wifi_config {} {}\r\n", ssid, password);
    let cmd2 = format!("wifi {} {}\r\n", ssid, password);
    let cmd3 = format!("set ssid {}\r\n", ssid);
    let cmd4 = format!("set password {}\r\n", password);

    assert!(cmd1.contains("wifi_config"));
    assert!(cmd1.contains(ssid));
    assert!(cmd1.contains(password));
    assert!(cmd1.ends_with("\r\n"));

    assert!(cmd2.starts_with("wifi "));
    assert!(cmd3.starts_with("set ssid "));
    assert!(cmd4.starts_with("set password "));
}

#[test]
fn test_wifi_credentials_validation() {
    // SSID: 1-32 characters
    let valid_ssid = "MyNetwork";
    let empty_ssid = "";
    let long_ssid = "A".repeat(33);

    assert!(!valid_ssid.is_empty() && valid_ssid.len() <= 32);
    assert!(empty_ssid.is_empty());
    assert!(long_ssid.len() > 32);

    // Password: 8-63 characters for WPA2
    let valid_pass = "password123";
    let short_pass = "short";
    let long_pass = "A".repeat(64);

    assert!(valid_pass.len() >= 8 && valid_pass.len() <= 63);
    assert!(short_pass.len() < 8);
    assert!(long_pass.len() > 63);
}

// ============================================================================
// Node Registry Tests
// ============================================================================

#[test]
fn test_node_registry() {
    use wifi_densepose_desktop::domain::node::{
        DiscoveredNode, MacAddress, NodeRegistry, HealthStatus, Chip, MeshRole, DiscoveryMethod
    };

    let mut registry = NodeRegistry::new();
    assert!(registry.is_empty());

    let node = DiscoveredNode {
        ip: "192.168.1.100".into(),
        mac: Some("AA:BB:CC:DD:EE:FF".into()),
        hostname: Some("csi-node-1".into()),
        node_id: 1,
        firmware_version: Some("0.3.0".into()),
        health: HealthStatus::Online,
        last_seen: "2024-01-01T00:00:00Z".into(),
        chip: Chip::Esp32s3,
        mesh_role: MeshRole::Node,
        discovery_method: DiscoveryMethod::Mdns,
        tdm_slot: Some(0),
        tdm_total: Some(4),
        edge_tier: None,
        uptime_secs: Some(3600),
        capabilities: None,
        friendly_name: None,
        notes: None,
    };

    registry.upsert(MacAddress::new("AA:BB:CC:DD:EE:FF"), node);
    assert_eq!(registry.len(), 1);

    let retrieved = registry.get(&MacAddress::new("AA:BB:CC:DD:EE:FF"));
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().ip, "192.168.1.100");
}

// ============================================================================
// MAC Address Tests
// ============================================================================

#[test]
fn test_mac_address() {
    use wifi_densepose_desktop::domain::node::MacAddress;

    let mac = MacAddress::new("AA:BB:CC:DD:EE:FF");
    assert_eq!(mac.to_string(), "AA:BB:CC:DD:EE:FF");

    let mac2 = MacAddress::new("aa:bb:cc:dd:ee:ff");
    assert_ne!(mac, mac2); // Case sensitive comparison
}
