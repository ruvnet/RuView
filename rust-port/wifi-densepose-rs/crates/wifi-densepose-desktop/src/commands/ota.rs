use serde::{Deserialize, Serialize};

/// Push firmware to a single node via HTTP OTA (port 8032).
#[tauri::command]
pub async fn ota_update(
    node_ip: String,
    firmware_path: String,
    psk: Option<String>,
) -> Result<OtaResult, String> {
    let _ = (node_ip, firmware_path, psk);
    Ok(OtaResult {
        success: true,
        node_ip: "stub".into(),
        message: "Stub: OTA not yet implemented".into(),
    })
}

/// Push firmware to multiple nodes with rolling update strategy.
#[tauri::command]
pub async fn batch_ota_update(
    node_ips: Vec<String>,
    firmware_path: String,
    psk: Option<String>,
) -> Result<Vec<OtaResult>, String> {
    let _ = (firmware_path, psk);
    Ok(node_ips
        .into_iter()
        .map(|ip| OtaResult {
            success: true,
            node_ip: ip,
            message: "Stub: batch OTA not yet implemented".into(),
        })
        .collect())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaResult {
    pub success: bool,
    pub node_ip: String,
    pub message: String,
}
