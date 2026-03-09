use serde::{Deserialize, Serialize};

/// Flash firmware binary to an ESP32 via serial port.
#[tauri::command]
pub async fn flash_firmware(
    port: String,
    firmware_path: String,
    chip: Option<String>,
    baud: Option<u32>,
) -> Result<FlashResult, String> {
    let _ = (port, firmware_path, chip, baud);
    // Stub: return placeholder result
    Ok(FlashResult {
        success: true,
        message: "Stub: flash not yet implemented".into(),
        duration_secs: 0.0,
    })
}

/// Get current flash progress (stub for polling-based approach).
#[tauri::command]
pub async fn flash_progress() -> Result<FlashProgress, String> {
    Ok(FlashProgress {
        phase: "idle".into(),
        progress_pct: 0.0,
        bytes_written: 0,
        bytes_total: 0,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashResult {
    pub success: bool,
    pub message: String,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlashProgress {
    pub phase: String,
    pub progress_pct: f32,
    pub bytes_written: u64,
    pub bytes_total: u64,
}
