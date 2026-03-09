use serde::{Deserialize, Serialize};

use crate::domain::config::ProvisioningConfig;

/// Provision NVS configuration to an ESP32 via serial port.
#[tauri::command]
pub async fn provision_node(
    port: String,
    config: ProvisioningConfig,
) -> Result<ProvisionResult, String> {
    let _ = (port, config);
    Ok(ProvisionResult {
        success: true,
        message: "Stub: provisioning not yet implemented".into(),
    })
}

/// Read current NVS configuration from a connected ESP32.
#[tauri::command]
pub async fn read_nvs(port: String) -> Result<ProvisioningConfig, String> {
    let _ = port;
    Ok(ProvisioningConfig::default())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvisionResult {
    pub success: bool,
    pub message: String,
}
