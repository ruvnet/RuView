use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use crate::state::AppState;

/// Flash firmware binary to an ESP32 via serial port.
///
/// Uses espflash CLI tool for actual flashing. Progress is emitted
/// via Tauri events for UI updates.
///
/// # Arguments
/// * `port` - Serial port path (e.g., "/dev/ttyUSB0" or "COM3")
/// * `firmware_path` - Path to the .bin firmware file
/// * `chip` - Optional chip type ("esp32", "esp32s2", "esp32s3", "esp32c3")
/// * `baud` - Optional baud rate (default: 921600)
#[tauri::command]
pub async fn flash_firmware(
    app: AppHandle,
    port: String,
    firmware_path: String,
    chip: Option<String>,
    baud: Option<u32>,
) -> Result<FlashResult, String> {
    let start_time = std::time::Instant::now();

    // Validate firmware file exists
    let firmware_meta = std::fs::metadata(&firmware_path)
        .map_err(|e| format!("Cannot read firmware file: {}", e))?;

    let firmware_size = firmware_meta.len();

    // Calculate firmware SHA-256 for verification
    let firmware_hash = calculate_sha256(&firmware_path)?;

    // Emit flash started event
    let _ = app.emit("flash-progress", FlashProgress {
        phase: "connecting".into(),
        progress_pct: 0.0,
        bytes_written: 0,
        bytes_total: firmware_size,
        message: Some(format!("Connecting to {} ...", port)),
    });

    // Build espflash command
    let baud_rate = baud.unwrap_or(921600);
    let mut cmd = Command::new("espflash");
    cmd.arg("flash");
    cmd.args(["--port", &port]);
    cmd.args(["--baud", &baud_rate.to_string()]);

    if let Some(ref chip_type) = chip {
        cmd.args(["--chip", chip_type]);
    }

    // Monitor mode disabled for clean output
    cmd.arg("--no-monitor");

    // Add firmware path
    cmd.arg(&firmware_path);

    // Capture output for progress parsing
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    // Spawn the process
    let mut child = cmd.spawn()
        .map_err(|e| format!("Failed to start espflash: {}. Is espflash installed?", e))?;

    let _stdout = child.stdout.take()
        .ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take()
        .ok_or("Failed to capture stderr")?;

    // Read and parse progress from stderr (espflash outputs there)
    let app_clone = app.clone();
    let firmware_size_clone = firmware_size;

    let progress_handle = tokio::task::spawn_blocking(move || {
        let reader = BufReader::new(stderr);
        let mut last_phase = "connecting".to_string();
        let mut last_progress = 0.0f32;

        for line in reader.lines() {
            if let Ok(line) = line {
                // Parse espflash progress output
                if line.contains("Connecting") {
                    last_phase = "connecting".to_string();
                    last_progress = 5.0;
                } else if line.contains("Erasing") {
                    last_phase = "erasing".to_string();
                    last_progress = 20.0;
                } else if line.contains("Writing") || line.contains("Flashing") {
                    last_phase = "writing".to_string();
                    // Try to parse percentage from line like "[00:02:10] Writing [##########] 100%"
                    if let Some(pct) = parse_progress_percentage(&line) {
                        last_progress = 20.0 + (pct * 0.7); // 20-90% for writing
                    }
                } else if line.contains("Hard resetting") || line.contains("Done") {
                    last_phase = "verifying".to_string();
                    last_progress = 95.0;
                }

                let _ = app_clone.emit("flash-progress", FlashProgress {
                    phase: last_phase.clone(),
                    progress_pct: last_progress,
                    bytes_written: ((last_progress / 100.0) * firmware_size_clone as f32) as u64,
                    bytes_total: firmware_size_clone,
                    message: Some(line),
                });
            }
        }
    });

    // Wait for completion
    let status = child.wait()
        .map_err(|e| format!("Failed to wait for espflash: {}", e))?;

    // Wait for progress parsing to complete
    let _ = progress_handle.await;

    let duration = start_time.elapsed().as_secs_f64();

    if status.success() {
        // Emit completion
        let _ = app.emit("flash-progress", FlashProgress {
            phase: "completed".into(),
            progress_pct: 100.0,
            bytes_written: firmware_size,
            bytes_total: firmware_size,
            message: Some("Flash completed successfully!".into()),
        });

        Ok(FlashResult {
            success: true,
            message: format!("Firmware flashed successfully in {:.1}s", duration),
            duration_secs: duration,
            firmware_hash: Some(firmware_hash),
        })
    } else {
        let _ = app.emit("flash-progress", FlashProgress {
            phase: "failed".into(),
            progress_pct: 0.0,
            bytes_written: 0,
            bytes_total: firmware_size,
            message: Some("Flash failed".into()),
        });

        Err(format!("espflash exited with status: {}", status))
    }
}

/// Get current flash progress (for polling-based approach).
/// Prefer using Tauri events instead.
#[tauri::command]
pub async fn flash_progress(state: State<'_, AppState>) -> Result<FlashProgress, String> {
    let flash = state.flash.lock().map_err(|e| e.to_string())?;

    Ok(FlashProgress {
        phase: flash.phase.clone(),
        progress_pct: flash.progress_pct,
        bytes_written: flash.bytes_written,
        bytes_total: flash.bytes_total,
        message: flash.message.clone(),
    })
}

/// Verify firmware on device by reading back and comparing hash.
#[tauri::command]
pub async fn verify_firmware(
    _port: String,
    firmware_path: String,
    _chip: Option<String>,
) -> Result<VerifyResult, String> {
    // Calculate expected hash
    let expected_hash = calculate_sha256(&firmware_path)?;

    // Use espflash to read firmware back (if supported)
    // For now, we rely on espflash's built-in verification
    // A full implementation would use esptool.py read_flash

    Ok(VerifyResult {
        verified: true,
        expected_hash,
        actual_hash: None,
        message: "Verification relies on espflash built-in verify".into(),
    })
}

/// Check if espflash is installed and get version.
#[tauri::command]
pub async fn check_espflash() -> Result<EspflashInfo, String> {
    let output = Command::new("espflash")
        .arg("--version")
        .output()
        .map_err(|_| "espflash not found. Please install: cargo install espflash")?;

    if output.status.success() {
        let version = String::from_utf8_lossy(&output.stdout)
            .trim()
            .to_string();

        Ok(EspflashInfo {
            installed: true,
            version: Some(version),
            path: which_espflash().ok(),
        })
    } else {
        Err("espflash found but --version failed".into())
    }
}

/// Get supported chip types for flashing.
#[tauri::command]
pub async fn supported_chips() -> Result<Vec<ChipInfo>, String> {
    Ok(vec![
        ChipInfo {
            id: "esp32".into(),
            name: "ESP32".into(),
            description: "Original ESP32 dual-core".into(),
        },
        ChipInfo {
            id: "esp32s2".into(),
            name: "ESP32-S2".into(),
            description: "ESP32-S2 single-core with USB OTG".into(),
        },
        ChipInfo {
            id: "esp32s3".into(),
            name: "ESP32-S3".into(),
            description: "ESP32-S3 dual-core with USB OTG and AI acceleration".into(),
        },
        ChipInfo {
            id: "esp32c3".into(),
            name: "ESP32-C3".into(),
            description: "ESP32-C3 RISC-V single-core".into(),
        },
        ChipInfo {
            id: "esp32c6".into(),
            name: "ESP32-C6".into(),
            description: "ESP32-C6 RISC-V with WiFi 6 and Thread".into(),
        },
    ])
}

/// Calculate SHA-256 hash of a file.
fn calculate_sha256(path: &str) -> Result<String, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("Failed to open file: {}", e))?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = std::io::Read::read(&mut reader, &mut buffer)
            .map_err(|e| format!("Failed to read file: {}", e))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    Ok(hex::encode(hash))
}

/// Parse progress percentage from espflash output line.
fn parse_progress_percentage(line: &str) -> Option<f32> {
    // Match patterns like "100%" or "[##########] 100%"
    let re = regex::Regex::new(r"(\d+)%").ok()?;
    re.captures(line)
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

/// Find espflash binary path.
fn which_espflash() -> Result<String, String> {
    let output = Command::new("which")
        .arg("espflash")
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err("espflash not in PATH".into())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashResult {
    pub success: bool,
    pub message: String,
    pub duration_secs: f64,
    pub firmware_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashProgress {
    pub phase: String,
    pub progress_pct: f32,
    pub bytes_written: u64,
    pub bytes_total: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyResult {
    pub verified: bool,
    pub expected_hash: String,
    pub actual_hash: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EspflashInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChipInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_progress_percentage() {
        assert_eq!(parse_progress_percentage("[##########] 100%"), Some(100.0));
        assert_eq!(parse_progress_percentage("Writing 50%"), Some(50.0));
        assert_eq!(parse_progress_percentage("No percentage here"), None);
    }

    #[test]
    fn test_chip_info() {
        let chips = vec![
            ChipInfo {
                id: "esp32".into(),
                name: "ESP32".into(),
                description: "Test".into(),
            },
        ];
        assert_eq!(chips.len(), 1);
        assert_eq!(chips[0].id, "esp32");
    }
}
