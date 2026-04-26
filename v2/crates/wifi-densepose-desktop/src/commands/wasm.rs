use std::fs::File;
use std::io::Read;
use std::time::Duration;

use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// WASM management port on ESP32 nodes.
const WASM_PORT: u16 = 8033;

/// Request timeout for WASM operations.
const WASM_TIMEOUT_SECS: u64 = 30;

/// List WASM modules loaded on a specific node.
#[tauri::command]
pub async fn wasm_list(node_ip: String) -> Result<Vec<WasmModuleInfo>, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(WASM_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("http://{}:{}/wasm/list", node_ip, WASM_PORT);

    let response = client.get(&url).send().await
        .map_err(|e| format!("Failed to connect to node: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Node returned HTTP {}", response.status()));
    }

    let modules: Vec<WasmModuleInfo> = response.json().await
        .map_err(|e| format!("Failed to parse response: {}", e))?;

    Ok(modules)
}

/// Upload a WASM module to a node.
///
/// Protocol:
/// 1. Read WASM file and calculate SHA-256
/// 2. POST multipart/form-data to http://<node_ip>:8033/wasm/upload
/// 3. Module is automatically validated on node side
/// 4. Return assigned module ID
#[tauri::command]
pub async fn wasm_upload(
    node_ip: String,
    wasm_path: String,
    module_name: Option<String>,
    auto_start: Option<bool>,
) -> Result<WasmUploadResult, String> {
    // Read WASM file
    let mut file = File::open(&wasm_path)
        .map_err(|e| format!("Cannot read WASM file: {}", e))?;

    let mut wasm_data = Vec::new();
    file.read_to_end(&mut wasm_data)
        .map_err(|e| format!("Failed to read WASM file: {}", e))?;

    let wasm_size = wasm_data.len();

    // Validate WASM magic bytes
    if wasm_data.len() < 4 || &wasm_data[0..4] != b"\0asm" {
        return Err("Invalid WASM file: missing magic bytes".into());
    }

    // Calculate SHA-256
    let mut hasher = Sha256::new();
    hasher.update(&wasm_data);
    let wasm_hash = hex::encode(hasher.finalize());

    // Extract filename for module name
    let name = module_name.unwrap_or_else(|| {
        std::path::Path::new(&wasm_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("module")
            .to_string()
    });

    // Build HTTP client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(WASM_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Build multipart form
    let wasm_part = Part::bytes(wasm_data)
        .file_name(format!("{}.wasm", name))
        .mime_str("application/wasm")
        .map_err(|e| format!("Failed to create multipart: {}", e))?;

    let form = Form::new()
        .part("wasm", wasm_part)
        .text("name", name.clone())
        .text("sha256", wasm_hash.clone())
        .text("size", wasm_size.to_string())
        .text("auto_start", auto_start.unwrap_or(false).to_string());

    // Send request
    let url = format!("http://{}:{}/wasm/upload", node_ip, WASM_PORT);
    let response = client.post(&url)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("WASM upload failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!("WASM upload failed with HTTP {}: {}", status, body));
    }

    // Parse response for module ID
    let upload_response: WasmUploadResponse = response.json().await
        .map_err(|e| format!("Failed to parse upload response: {}", e))?;

    Ok(WasmUploadResult {
        success: true,
        module_id: upload_response.module_id,
        message: format!("Module '{}' uploaded successfully ({} bytes)", name, wasm_size),
        sha256: Some(wasm_hash),
    })
}

/// Start, stop, or unload a WASM module on a node.
///
/// Actions:
/// - "start": Start module execution
/// - "stop": Pause module execution
/// - "unload": Remove module from memory
/// - "restart": Stop then start
#[tauri::command]
pub async fn wasm_control(
    node_ip: String,
    module_id: String,
    action: String,
) -> Result<WasmControlResult, String> {
    // Validate action
    let valid_actions = ["start", "stop", "unload", "restart"];
    if !valid_actions.contains(&action.as_str()) {
        return Err(format!(
            "Invalid action '{}'. Valid actions: {:?}",
            action, valid_actions
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(WASM_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!(
        "http://{}:{}/wasm/{}/{}",
        node_ip, WASM_PORT, module_id, action
    );

    let response = client.post(&url).send().await
        .map_err(|e| format!("WASM control failed: {}", e))?;

    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(format!(
            "WASM {} failed with HTTP {}: {}",
            action, status, body
        ));
    }

    Ok(WasmControlResult {
        success: true,
        module_id,
        action,
        message: "Operation completed successfully".into(),
    })
}

/// Get detailed info about a specific WASM module.
#[tauri::command]
pub async fn wasm_info(
    node_ip: String,
    module_id: String,
) -> Result<WasmModuleDetail, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(WASM_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("http://{}:{}/wasm/{}", node_ip, WASM_PORT, module_id);

    let response = client.get(&url).send().await
        .map_err(|e| format!("Failed to get module info: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("Module not found or HTTP {}", response.status()));
    }

    let detail: WasmModuleDetail = response.json().await
        .map_err(|e| format!("Failed to parse module info: {}", e))?;

    Ok(detail)
}

/// Get WASM runtime statistics from a node.
#[tauri::command]
pub async fn wasm_stats(node_ip: String) -> Result<WasmRuntimeStats, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(WASM_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("http://{}:{}/wasm/stats", node_ip, WASM_PORT);

    let response = client.get(&url).send().await
        .map_err(|e| format!("Failed to get WASM stats: {}", e))?;

    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status()));
    }

    let stats: WasmRuntimeStats = response.json().await
        .map_err(|e| format!("Failed to parse stats: {}", e))?;

    Ok(stats)
}

/// Check if node supports WASM modules.
#[tauri::command]
pub async fn check_wasm_support(node_ip: String) -> Result<WasmSupportInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("http://{}:{}/wasm/info", node_ip, WASM_PORT);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let body = response.text().await.unwrap_or_default();

                // Try to parse as JSON
                let info = serde_json::from_str::<serde_json::Value>(&body).ok();

                Ok(WasmSupportInfo {
                    supported: true,
                    max_modules: info.as_ref()
                        .and_then(|v| v.get("max_modules").and_then(|v| v.as_u64()))
                        .map(|v| v as u8),
                    memory_limit_kb: info.as_ref()
                        .and_then(|v| v.get("memory_limit_kb").and_then(|v| v.as_u64()))
                        .map(|v| v as u32),
                    verify_signatures: info.as_ref()
                        .and_then(|v| v.get("verify_signatures").and_then(|v| v.as_bool()))
                        .unwrap_or(false),
                })
            } else if response.status() == reqwest::StatusCode::NOT_FOUND {
                Ok(WasmSupportInfo {
                    supported: false,
                    max_modules: None,
                    memory_limit_kb: None,
                    verify_signatures: false,
                })
            } else {
                Err(format!("HTTP {}", response.status()))
            }
        }
        Err(_) => Ok(WasmSupportInfo {
            supported: false,
            max_modules: None,
            memory_limit_kb: None,
            verify_signatures: false,
        }),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleInfo {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub status: String,
    pub sha256: Option<String>,
    pub loaded_at: Option<String>,
    pub memory_used_kb: Option<u32>,
    pub cpu_usage_pct: Option<f32>,
    pub exec_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmModuleDetail {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub status: String,
    pub sha256: String,
    pub loaded_at: String,
    pub memory_used_kb: u32,
    pub exports: Vec<String>,
    pub imports: Vec<String>,
    pub execution_count: u64,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WasmUploadResponse {
    pub module_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmUploadResult {
    pub success: bool,
    pub module_id: String,
    pub message: String,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmControlResult {
    pub success: bool,
    pub module_id: String,
    pub action: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmRuntimeStats {
    pub total_modules: u8,
    pub running_modules: u8,
    pub memory_used_kb: u32,
    pub memory_limit_kb: u32,
    pub total_executions: u64,
    pub errors: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct WasmSupportInfo {
    pub supported: bool,
    pub max_modules: Option<u8>,
    pub memory_limit_kb: Option<u32>,
    pub verify_signatures: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_magic_bytes() {
        let valid_wasm = b"\0asm\x01\x00\x00\x00";
        assert_eq!(&valid_wasm[0..4], b"\0asm");

        let invalid = b"not wasm";
        assert_ne!(&invalid[0..4], b"\0asm");
    }

    #[test]
    fn test_wasm_module_info() {
        let info = WasmModuleInfo {
            id: "mod-1".into(),
            name: "test".into(),
            size_bytes: 1024,
            status: "running".into(),
            sha256: Some("abc123".into()),
            loaded_at: Some("2024-01-01T00:00:00Z".into()),
            memory_used_kb: Some(128),
            cpu_usage_pct: Some(5.2),
            exec_count: Some(42),
        };

        assert_eq!(info.id, "mod-1");
        assert_eq!(info.size_bytes, 1024);
        assert_eq!(info.memory_used_kb, Some(128));
    }
}
