use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::domain::config::ProvisioningConfig;

/// Serial baud rate for provisioning communication.
const PROVISION_BAUD: u32 = 115200;

/// Timeout for serial operations.
const SERIAL_TIMEOUT_MS: u64 = 5000;

/// NVS partition name (reserved for future use).
#[allow(dead_code)]
const NVS_PARTITION: &str = "nvs";

/// Magic bytes for provisioning protocol.
const PROVISION_MAGIC: &[u8] = b"RUVIEW_NVS";

/// Provision NVS configuration to an ESP32 via serial port.
///
/// Protocol:
/// 1. Open serial port at 115200 baud
/// 2. Send provisioning magic bytes
/// 3. Wait for acknowledgment
/// 4. Send NVS binary blob
/// 5. Wait for checksum confirmation
#[tauri::command]
pub async fn provision_node(
    port: String,
    config: ProvisioningConfig,
) -> Result<ProvisionResult, String> {
    // Validate configuration
    config.validate()?;

    // Serialize config to NVS binary format
    let nvs_data = serialize_nvs_config(&config)?;
    let nvs_size = nvs_data.len();

    // Calculate checksum
    let mut hasher = Sha256::new();
    hasher.update(&nvs_data);
    let checksum = hex::encode(&hasher.finalize()[..8]); // First 8 bytes

    // Open serial port
    let port_settings = tokio_serial::SerialPortBuilderExt::open_native_async(
        tokio_serial::new(&port, PROVISION_BAUD)
            .timeout(Duration::from_millis(SERIAL_TIMEOUT_MS))
    ).map_err(|e| format!("Failed to open serial port: {}", e))?;

    let (mut reader, mut writer) = tokio::io::split(port_settings);

    // Send magic bytes + size header
    let header = ProvisionHeader {
        magic: PROVISION_MAGIC.try_into().unwrap(),
        version: 1,
        size: nvs_size as u32,
    };

    let header_bytes = bincode_header(&header);
    tokio::io::AsyncWriteExt::write_all(&mut writer, &header_bytes).await
        .map_err(|e| format!("Failed to send header: {}", e))?;

    // Wait for ACK
    let mut ack_buf = [0u8; 4];
    tokio::time::timeout(
        Duration::from_millis(SERIAL_TIMEOUT_MS),
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut ack_buf)
    ).await
        .map_err(|_| "Timeout waiting for device acknowledgment")?
        .map_err(|e| format!("Failed to read ACK: {}", e))?;

    if &ack_buf != b"ACK\n" {
        return Err(format!("Invalid ACK response: {:?}", ack_buf));
    }

    // Send NVS data in chunks
    const CHUNK_SIZE: usize = 256;
    for chunk in nvs_data.chunks(CHUNK_SIZE) {
        tokio::io::AsyncWriteExt::write_all(&mut writer, chunk).await
            .map_err(|e| format!("Failed to send data chunk: {}", e))?;

        // Small delay between chunks for device processing
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // Send checksum
    tokio::io::AsyncWriteExt::write_all(&mut writer, checksum.as_bytes()).await
        .map_err(|e| format!("Failed to send checksum: {}", e))?;

    tokio::io::AsyncWriteExt::write_all(&mut writer, b"\n").await
        .map_err(|e| format!("Failed to send newline: {}", e))?;

    // Wait for confirmation
    let mut confirm_buf = [0u8; 32];
    let confirm_len = tokio::time::timeout(
        Duration::from_millis(SERIAL_TIMEOUT_MS * 2),
        tokio::io::AsyncReadExt::read(&mut reader, &mut confirm_buf)
    ).await
        .map_err(|_| "Timeout waiting for confirmation")?
        .map_err(|e| format!("Failed to read confirmation: {}", e))?;

    let confirm_str = String::from_utf8_lossy(&confirm_buf[..confirm_len]);

    if confirm_str.contains("OK") {
        Ok(ProvisionResult {
            success: true,
            message: format!("Provisioned {} bytes to NVS successfully", nvs_size),
            checksum: Some(checksum),
        })
    } else if confirm_str.contains("ERR") {
        Err(format!("Device reported error: {}", confirm_str.trim()))
    } else {
        Err(format!("Unexpected response: {}", confirm_str.trim()))
    }
}

/// Read current NVS configuration from a connected ESP32.
#[tauri::command]
pub async fn read_nvs(port: String) -> Result<ProvisioningConfig, String> {
    // Open serial port
    let port_settings = tokio_serial::SerialPortBuilderExt::open_native_async(
        tokio_serial::new(&port, PROVISION_BAUD)
            .timeout(Duration::from_millis(SERIAL_TIMEOUT_MS))
    ).map_err(|e| format!("Failed to open serial port: {}", e))?;

    let (mut reader, mut writer) = tokio::io::split(port_settings);

    // Send read command
    tokio::io::AsyncWriteExt::write_all(&mut writer, b"RUVIEW_NVS_READ\n").await
        .map_err(|e| format!("Failed to send read command: {}", e))?;

    // Read size header
    let mut size_buf = [0u8; 4];
    tokio::time::timeout(
        Duration::from_millis(SERIAL_TIMEOUT_MS),
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut size_buf)
    ).await
        .map_err(|_| "Timeout waiting for NVS size")?
        .map_err(|e| format!("Failed to read size: {}", e))?;

    let nvs_size = u32::from_le_bytes(size_buf) as usize;

    if nvs_size == 0 || nvs_size > 4096 {
        return Err(format!("Invalid NVS size: {}", nvs_size));
    }

    // Read NVS data
    let mut nvs_data = vec![0u8; nvs_size];
    tokio::time::timeout(
        Duration::from_millis(SERIAL_TIMEOUT_MS * 2),
        tokio::io::AsyncReadExt::read_exact(&mut reader, &mut nvs_data)
    ).await
        .map_err(|_| "Timeout reading NVS data")?
        .map_err(|e| format!("Failed to read NVS data: {}", e))?;

    // Parse NVS data to config
    deserialize_nvs_config(&nvs_data)
}

/// Erase NVS partition on a connected ESP32.
#[tauri::command]
pub async fn erase_nvs(port: String) -> Result<ProvisionResult, String> {
    // Open serial port
    let port_settings = tokio_serial::SerialPortBuilderExt::open_native_async(
        tokio_serial::new(&port, PROVISION_BAUD)
            .timeout(Duration::from_millis(SERIAL_TIMEOUT_MS))
    ).map_err(|e| format!("Failed to open serial port: {}", e))?;

    let (mut reader, mut writer) = tokio::io::split(port_settings);

    // Send erase command
    tokio::io::AsyncWriteExt::write_all(&mut writer, b"RUVIEW_NVS_ERASE\n").await
        .map_err(|e| format!("Failed to send erase command: {}", e))?;

    // Wait for confirmation
    let mut confirm_buf = [0u8; 32];
    let confirm_len = tokio::time::timeout(
        Duration::from_millis(SERIAL_TIMEOUT_MS * 3), // Erase takes longer
        tokio::io::AsyncReadExt::read(&mut reader, &mut confirm_buf)
    ).await
        .map_err(|_| "Timeout waiting for erase confirmation")?
        .map_err(|e| format!("Failed to read confirmation: {}", e))?;

    let confirm_str = String::from_utf8_lossy(&confirm_buf[..confirm_len]);

    if confirm_str.contains("OK") {
        Ok(ProvisionResult {
            success: true,
            message: "NVS partition erased successfully".into(),
            checksum: None,
        })
    } else {
        Err(format!("Erase failed: {}", confirm_str.trim()))
    }
}

/// Validate provisioning configuration without applying.
#[tauri::command]
pub async fn validate_config(config: ProvisioningConfig) -> Result<ValidationResult, String> {
    match config.validate() {
        Ok(()) => {
            let nvs_data = serialize_nvs_config(&config)?;
            Ok(ValidationResult {
                valid: true,
                message: None,
                estimated_size: nvs_data.len(),
            })
        }
        Err(e) => Ok(ValidationResult {
            valid: false,
            message: Some(e),
            estimated_size: 0,
        }),
    }
}

/// Generate mesh provisioning configs for multiple nodes.
#[tauri::command]
pub async fn generate_mesh_configs(
    base_config: ProvisioningConfig,
    node_count: u8,
) -> Result<Vec<MeshNodeConfig>, String> {
    if node_count == 0 || node_count > 32 {
        return Err("Node count must be 1-32".into());
    }

    let mut configs = Vec::new();

    for i in 0..node_count {
        let mut node_config = base_config.clone();
        node_config.node_id = Some(i);
        node_config.tdm_slot = Some(i);
        node_config.tdm_total = Some(node_count);

        configs.push(MeshNodeConfig {
            node_id: i,
            tdm_slot: i,
            config: node_config,
        });
    }

    Ok(configs)
}

/// Serialize ProvisioningConfig to NVS binary format.
/// Format: key-value pairs with length prefixes
fn serialize_nvs_config(config: &ProvisioningConfig) -> Result<Vec<u8>, String> {
    let mut data = Vec::new();

    // Inline helpers to avoid closure borrow issues
    fn write_str(data: &mut Vec<u8>, key: &str, value: &str) {
        // Key length (1 byte) + key + value length (2 bytes) + value
        data.push(key.len() as u8);
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(&(value.len() as u16).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
    }

    fn write_u8(data: &mut Vec<u8>, key: &str, value: u8) {
        data.push(key.len() as u8);
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(&1u16.to_le_bytes());
        data.push(value);
    }

    fn write_u16(data: &mut Vec<u8>, key: &str, value: u16) {
        data.push(key.len() as u8);
        data.extend_from_slice(key.as_bytes());
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&value.to_le_bytes());
    }

    // Serialize each field
    if let Some(ref ssid) = config.wifi_ssid {
        write_str(&mut data, "wifi_ssid", ssid);
    }
    if let Some(ref pass) = config.wifi_password {
        write_str(&mut data, "wifi_pass", pass);
    }
    if let Some(ref ip) = config.target_ip {
        write_str(&mut data, "target_ip", ip);
    }
    if let Some(port) = config.target_port {
        write_u16(&mut data, "target_port", port);
    }
    if let Some(id) = config.node_id {
        write_u8(&mut data, "node_id", id);
    }
    if let Some(slot) = config.tdm_slot {
        write_u8(&mut data, "tdm_slot", slot);
    }
    if let Some(total) = config.tdm_total {
        write_u8(&mut data, "tdm_total", total);
    }
    if let Some(tier) = config.edge_tier {
        write_u8(&mut data, "edge_tier", tier);
    }
    if let Some(thresh) = config.presence_thresh {
        write_u16(&mut data, "presence_th", thresh);
    }
    if let Some(thresh) = config.fall_thresh {
        write_u16(&mut data, "fall_th", thresh);
    }
    if let Some(window) = config.vital_window {
        write_u16(&mut data, "vital_win", window);
    }
    if let Some(interval) = config.vital_interval_ms {
        write_u16(&mut data, "vital_int", interval);
    }
    if let Some(count) = config.top_k_count {
        write_u8(&mut data, "top_k", count);
    }
    if let Some(hops) = config.hop_count {
        write_u8(&mut data, "hop_count", hops);
    }
    if let Some(ref channels) = config.channel_list {
        let ch_str: String = channels.iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(",");
        write_str(&mut data, "channels", &ch_str);
    }
    if let Some(duty) = config.power_duty {
        write_u8(&mut data, "power_duty", duty);
    }
    if let Some(max) = config.wasm_max_modules {
        write_u8(&mut data, "wasm_max", max);
    }
    if let Some(verify) = config.wasm_verify {
        write_u8(&mut data, "wasm_verify", if verify { 1 } else { 0 });
    }
    if let Some(ref psk) = config.ota_psk {
        write_str(&mut data, "ota_psk", psk);
    }

    // End marker
    data.push(0);

    Ok(data)
}

/// Deserialize NVS binary data to ProvisioningConfig.
fn deserialize_nvs_config(data: &[u8]) -> Result<ProvisioningConfig, String> {
    let mut config = ProvisioningConfig::default();
    let mut pos = 0;

    while pos < data.len() {
        // Read key length
        let key_len = data[pos] as usize;
        pos += 1;

        if key_len == 0 {
            break; // End marker
        }

        if pos + key_len > data.len() {
            return Err("Invalid NVS data: truncated key".into());
        }

        let key = std::str::from_utf8(&data[pos..pos + key_len])
            .map_err(|_| "Invalid key encoding")?;
        pos += key_len;

        if pos + 2 > data.len() {
            return Err("Invalid NVS data: truncated value length".into());
        }

        let value_len = u16::from_le_bytes([data[pos], data[pos + 1]]) as usize;
        pos += 2;

        if pos + value_len > data.len() {
            return Err("Invalid NVS data: truncated value".into());
        }

        let value_bytes = &data[pos..pos + value_len];
        pos += value_len;

        // Parse based on key
        match key {
            "wifi_ssid" => config.wifi_ssid = Some(String::from_utf8_lossy(value_bytes).to_string()),
            "wifi_pass" => config.wifi_password = Some(String::from_utf8_lossy(value_bytes).to_string()),
            "target_ip" => config.target_ip = Some(String::from_utf8_lossy(value_bytes).to_string()),
            "target_port" if value_len == 2 => {
                config.target_port = Some(u16::from_le_bytes([value_bytes[0], value_bytes[1]]));
            }
            "node_id" if value_len == 1 => config.node_id = Some(value_bytes[0]),
            "tdm_slot" if value_len == 1 => config.tdm_slot = Some(value_bytes[0]),
            "tdm_total" if value_len == 1 => config.tdm_total = Some(value_bytes[0]),
            "edge_tier" if value_len == 1 => config.edge_tier = Some(value_bytes[0]),
            "presence_th" if value_len == 2 => {
                config.presence_thresh = Some(u16::from_le_bytes([value_bytes[0], value_bytes[1]]));
            }
            "fall_th" if value_len == 2 => {
                config.fall_thresh = Some(u16::from_le_bytes([value_bytes[0], value_bytes[1]]));
            }
            "vital_win" if value_len == 2 => {
                config.vital_window = Some(u16::from_le_bytes([value_bytes[0], value_bytes[1]]));
            }
            "vital_int" if value_len == 2 => {
                config.vital_interval_ms = Some(u16::from_le_bytes([value_bytes[0], value_bytes[1]]));
            }
            "top_k" if value_len == 1 => config.top_k_count = Some(value_bytes[0]),
            "hop_count" if value_len == 1 => config.hop_count = Some(value_bytes[0]),
            "channels" => {
                let ch_str = String::from_utf8_lossy(value_bytes);
                config.channel_list = Some(
                    ch_str.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect()
                );
            }
            "power_duty" if value_len == 1 => config.power_duty = Some(value_bytes[0]),
            "wasm_max" if value_len == 1 => config.wasm_max_modules = Some(value_bytes[0]),
            "wasm_verify" if value_len == 1 => config.wasm_verify = Some(value_bytes[0] != 0),
            "ota_psk" => config.ota_psk = Some(String::from_utf8_lossy(value_bytes).to_string()),
            _ => {} // Ignore unknown keys
        }
    }

    Ok(config)
}

/// Binary header for provisioning protocol.
#[repr(C, packed)]
struct ProvisionHeader {
    magic: [u8; 10],
    version: u8,
    size: u32,
}

fn bincode_header(header: &ProvisionHeader) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(15);
    bytes.extend_from_slice(&header.magic);
    bytes.push(header.version);
    bytes.extend_from_slice(&header.size.to_le_bytes());
    bytes
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResult {
    pub success: bool,
    pub message: String,
    pub checksum: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub message: Option<String>,
    pub estimated_size: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct MeshNodeConfig {
    pub node_id: u8,
    pub tdm_slot: u8,
    pub config: ProvisioningConfig,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serialize_deserialize_config() {
        let config = ProvisioningConfig {
            wifi_ssid: Some("TestNetwork".into()),
            wifi_password: Some("password123".into()),
            node_id: Some(1),
            tdm_slot: Some(0),
            tdm_total: Some(4),
            ..Default::default()
        };

        let serialized = serialize_nvs_config(&config).unwrap();
        let deserialized = deserialize_nvs_config(&serialized).unwrap();

        assert_eq!(deserialized.wifi_ssid, config.wifi_ssid);
        assert_eq!(deserialized.node_id, config.node_id);
        assert_eq!(deserialized.tdm_slot, config.tdm_slot);
    }

    #[test]
    fn test_config_validation() {
        let mut config = ProvisioningConfig::default();
        config.tdm_slot = Some(5);
        config.tdm_total = Some(4);

        let result = config.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_provision_header() {
        let header = ProvisionHeader {
            magic: *b"RUVIEW_NVS",
            version: 1,
            size: 256,
        };

        let bytes = bincode_header(&header);
        assert_eq!(bytes.len(), 15);
        assert_eq!(&bytes[0..10], b"RUVIEW_NVS");
    }
}
