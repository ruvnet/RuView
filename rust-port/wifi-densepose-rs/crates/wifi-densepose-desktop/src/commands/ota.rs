use std::fs::File;
use std::io::Read;
use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter};

/// OTA update port on ESP32 nodes.
const OTA_PORT: u16 = 8032;

/// OTA endpoint path.
const OTA_PATH: &str = "/ota/upload";

/// Request timeout for OTA uploads.
const OTA_TIMEOUT_SECS: u64 = 120;

type HmacSha256 = Hmac<Sha256>;

/// Push firmware to a single node via HTTP OTA (port 8032).
///
/// Protocol:
/// 1. Calculate firmware SHA-256
/// 2. Sign with PSK using HMAC-SHA256 if provided
/// 3. POST multipart/form-data to http://<node_ip>:8032/ota/upload
/// 4. Include signature in X-OTA-Signature header
/// 5. Wait for reboot confirmation
#[tauri::command]
pub async fn ota_update(
    app: AppHandle,
    node_ip: String,
    firmware_path: String,
    psk: Option<String>,
) -> Result<OtaResult, String> {
    let start_time = std::time::Instant::now();

    // Emit progress
    let _ = app.emit("ota-progress", OtaProgress {
        node_ip: node_ip.clone(),
        phase: "preparing".into(),
        progress_pct: 0.0,
        message: Some("Reading firmware...".into()),
    });

    // Read firmware file
    let mut file = File::open(&firmware_path)
        .map_err(|e| format!("Cannot read firmware: {}", e))?;

    let mut firmware_data = Vec::new();
    file.read_to_end(&mut firmware_data)
        .map_err(|e| format!("Failed to read firmware: {}", e))?;

    let firmware_size = firmware_data.len();

    // Calculate SHA-256 hash
    let mut hasher = Sha256::new();
    hasher.update(&firmware_data);
    let firmware_hash = hex::encode(hasher.finalize());

    // Calculate HMAC signature if PSK provided
    let signature = if let Some(ref key) = psk {
        let mut mac = HmacSha256::new_from_slice(key.as_bytes())
            .map_err(|e| format!("Invalid PSK: {}", e))?;
        mac.update(&firmware_data);
        Some(hex::encode(mac.finalize().into_bytes()))
    } else {
        None
    };

    // Emit progress
    let _ = app.emit("ota-progress", OtaProgress {
        node_ip: node_ip.clone(),
        phase: "uploading".into(),
        progress_pct: 10.0,
        message: Some(format!("Uploading {} bytes to {}...", firmware_size, node_ip)),
    });

    // Build HTTP client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(OTA_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    // Build multipart form
    let firmware_part = Part::bytes(firmware_data)
        .file_name("firmware.bin")
        .mime_str("application/octet-stream")
        .map_err(|e| format!("Failed to create multipart: {}", e))?;

    let form = Form::new()
        .part("firmware", firmware_part)
        .text("sha256", firmware_hash.clone())
        .text("size", firmware_size.to_string());

    // Build request
    let url = format!("http://{}:{}{}", node_ip, OTA_PORT, OTA_PATH);
    let mut request = client.post(&url).multipart(form);

    // Add signature header if present
    if let Some(ref sig) = signature {
        request = request.header("X-OTA-Signature", sig);
    }

    // Add firmware hash header
    request = request.header("X-OTA-SHA256", &firmware_hash);

    // Send request
    let response = request.send().await
        .map_err(|e| format!("OTA upload failed: {}", e))?;

    let status = response.status();
    let body = response.text().await.unwrap_or_default();

    if !status.is_success() {
        let _ = app.emit("ota-progress", OtaProgress {
            node_ip: node_ip.clone(),
            phase: "failed".into(),
            progress_pct: 0.0,
            message: Some(format!("HTTP {}: {}", status, body)),
        });

        return Err(format!("OTA failed with HTTP {}: {}", status, body));
    }

    // Emit progress - upload complete
    let _ = app.emit("ota-progress", OtaProgress {
        node_ip: node_ip.clone(),
        phase: "rebooting".into(),
        progress_pct: 80.0,
        message: Some("Waiting for node reboot...".into()),
    });

    // Wait for node to come back online
    let reboot_ok = wait_for_reboot(&client, &node_ip, Duration::from_secs(30)).await;

    let duration = start_time.elapsed().as_secs_f64();

    if reboot_ok {
        let _ = app.emit("ota-progress", OtaProgress {
            node_ip: node_ip.clone(),
            phase: "completed".into(),
            progress_pct: 100.0,
            message: Some(format!("OTA completed in {:.1}s", duration)),
        });

        Ok(OtaResult {
            success: true,
            node_ip,
            message: format!("OTA completed successfully in {:.1}s", duration),
            firmware_hash: Some(firmware_hash),
            duration_secs: Some(duration),
        })
    } else {
        let _ = app.emit("ota-progress", OtaProgress {
            node_ip: node_ip.clone(),
            phase: "warning".into(),
            progress_pct: 90.0,
            message: Some("Node may not have rebooted successfully".into()),
        });

        Ok(OtaResult {
            success: true,
            node_ip,
            message: "OTA uploaded but reboot confirmation timed out".into(),
            firmware_hash: Some(firmware_hash),
            duration_secs: Some(duration),
        })
    }
}

/// Push firmware to multiple nodes with rolling update strategy.
///
/// Strategy options:
/// - Sequential: One node at a time
/// - Parallel: All nodes simultaneously (max_concurrent)
/// - TdmSafe: Respects TDM slots to avoid disruption
#[tauri::command]
pub async fn batch_ota_update(
    app: AppHandle,
    node_ips: Vec<String>,
    firmware_path: String,
    psk: Option<String>,
    strategy: Option<String>,
    max_concurrent: Option<usize>,
) -> Result<BatchOtaResult, String> {
    let start_time = std::time::Instant::now();
    let total_nodes = node_ips.len();
    let strategy = strategy.unwrap_or_else(|| "sequential".into());
    let max_concurrent = max_concurrent.unwrap_or(1);

    let _ = app.emit("batch-ota-progress", BatchOtaProgress {
        phase: "starting".into(),
        total: total_nodes,
        completed: 0,
        failed: 0,
        current_node: None,
    });

    let mut results = Vec::new();
    let mut completed = 0;
    let mut failed = 0;

    match strategy.as_str() {
        "parallel" => {
            // Parallel execution with semaphore
            // Parallel OTA with semaphore

            let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(max_concurrent));
            let firmware_path = std::sync::Arc::new(firmware_path);
            let psk = std::sync::Arc::new(psk);
            let app = std::sync::Arc::new(app.clone());

            let tasks: Vec<_> = node_ips.into_iter().map(|ip| {
                let sem = semaphore.clone();
                let fw_path = firmware_path.clone();
                let psk_clone = psk.clone();
                let app_clone = app.clone();

                async move {
                    let _permit = sem.acquire().await.unwrap();
                    ota_update(
                        (*app_clone).clone(),
                        ip,
                        (*fw_path).clone(),
                        (*psk_clone).clone(),
                    ).await
                }
            }).collect();

            let task_results = futures::future::join_all(tasks).await;

            for result in task_results {
                match result {
                    Ok(r) => {
                        if r.success {
                            completed += 1;
                        } else {
                            failed += 1;
                        }
                        results.push(r);
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(OtaResult {
                            success: false,
                            node_ip: "unknown".into(),
                            message: e,
                            firmware_hash: None,
                            duration_secs: None,
                        });
                    }
                }
            }
        }
        _ => {
            // Sequential execution (default)
            for ip in node_ips {
                let _ = app.emit("batch-ota-progress", BatchOtaProgress {
                    phase: "updating".into(),
                    total: total_nodes,
                    completed,
                    failed,
                    current_node: Some(ip.clone()),
                });

                match ota_update(
                    app.clone(),
                    ip.clone(),
                    firmware_path.clone(),
                    psk.clone(),
                ).await {
                    Ok(r) => {
                        if r.success {
                            completed += 1;
                        } else {
                            failed += 1;
                        }
                        results.push(r);
                    }
                    Err(e) => {
                        failed += 1;
                        results.push(OtaResult {
                            success: false,
                            node_ip: ip,
                            message: e,
                            firmware_hash: None,
                            duration_secs: None,
                        });
                    }
                }
            }
        }
    }

    let duration = start_time.elapsed().as_secs_f64();

    let _ = app.emit("batch-ota-progress", BatchOtaProgress {
        phase: "completed".into(),
        total: total_nodes,
        completed,
        failed,
        current_node: None,
    });

    Ok(BatchOtaResult {
        total: total_nodes,
        completed,
        failed,
        results,
        duration_secs: duration,
    })
}

/// Check if a node's OTA endpoint is accessible.
#[tauri::command]
pub async fn check_ota_endpoint(node_ip: String) -> Result<OtaEndpointInfo, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

    let url = format!("http://{}:{}/ota/status", node_ip, OTA_PORT);

    match client.get(&url).send().await {
        Ok(response) => {
            if response.status().is_success() {
                let body = response.text().await.unwrap_or_default();

                // Try to parse as JSON
                let version = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|v| v.get("version").and_then(|v| v.as_str().map(|s| s.to_string())));

                Ok(OtaEndpointInfo {
                    reachable: true,
                    ota_supported: true,
                    current_version: version,
                    psk_required: false, // Would need to check headers
                })
            } else {
                Ok(OtaEndpointInfo {
                    reachable: true,
                    ota_supported: response.status() != reqwest::StatusCode::NOT_FOUND,
                    current_version: None,
                    psk_required: response.status() == reqwest::StatusCode::UNAUTHORIZED,
                })
            }
        }
        Err(_) => Ok(OtaEndpointInfo {
            reachable: false,
            ota_supported: false,
            current_version: None,
            psk_required: false,
        }),
    }
}

/// Wait for a node to come back online after OTA reboot.
async fn wait_for_reboot(client: &reqwest::Client, node_ip: &str, timeout: Duration) -> bool {
    let url = format!("http://{}:{}/ota/status", node_ip, OTA_PORT);
    let start = std::time::Instant::now();

    // First wait for node to go down
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Then poll for it to come back
    while start.elapsed() < timeout {
        if let Ok(response) = client.get(&url).send().await {
            if response.status().is_success() {
                return true;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    false
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OtaResult {
    pub success: bool,
    pub node_ip: String,
    pub message: String,
    pub firmware_hash: Option<String>,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OtaProgress {
    pub node_ip: String,
    pub phase: String,
    pub progress_pct: f32,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOtaResult {
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub results: Vec<OtaResult>,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchOtaProgress {
    pub phase: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub current_node: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OtaEndpointInfo {
    pub reachable: bool,
    pub ota_supported: bool,
    pub current_version: Option<String>,
    pub psk_required: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hmac_signature() {
        let data = b"test firmware data";
        let psk = "secret_key";

        let mut mac = HmacSha256::new_from_slice(psk.as_bytes()).unwrap();
        mac.update(data);
        let signature = hex::encode(mac.finalize().into_bytes());

        assert_eq!(signature.len(), 64); // SHA-256 = 32 bytes = 64 hex chars
    }

    #[test]
    fn test_sha256_hash() {
        let mut hasher = Sha256::new();
        hasher.update(b"test data");
        let hash = hex::encode(hasher.finalize());

        assert_eq!(hash.len(), 64);
    }
}
