use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

/// Application settings that persist across restarts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub server_http_port: u16,
    pub server_ws_port: u16,
    pub server_udp_port: u16,
    pub bind_address: String,
    pub ui_path: String,
    pub ota_psk: String,
    pub auto_discover: bool,
    pub discover_interval_ms: u32,
    pub theme: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            server_http_port: 8080,
            server_ws_port: 8765,
            server_udp_port: 5005,
            bind_address: "127.0.0.1".into(),
            ui_path: String::new(),
            ota_psk: String::new(),
            auto_discover: true,
            discover_interval_ms: 10_000,
            theme: "dark".into(),
        }
    }
}

/// Get the settings file path in the app data directory.
fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    // Ensure directory exists
    fs::create_dir_all(&app_dir)
        .map_err(|e| format!("Failed to create app data dir: {}", e))?;

    Ok(app_dir.join("settings.json"))
}

/// Load settings from disk.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Option<AppSettings>, String> {
    let path = settings_path(&app)?;

    if !path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&path)
        .map_err(|e| format!("Failed to read settings: {}", e))?;

    let settings: AppSettings = serde_json::from_str(&contents)
        .map_err(|e| format!("Failed to parse settings: {}", e))?;

    Ok(Some(settings))
}

/// Save settings to disk.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: AppSettings) -> Result<(), String> {
    let path = settings_path(&app)?;

    let contents = serde_json::to_string_pretty(&settings)
        .map_err(|e| format!("Failed to serialize settings: {}", e))?;

    fs::write(&path, contents)
        .map_err(|e| format!("Failed to write settings: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        assert_eq!(settings.server_http_port, 8080);
        assert_eq!(settings.bind_address, "127.0.0.1");
        assert!(settings.auto_discover);
    }

    #[test]
    fn test_settings_serialization() {
        let settings = AppSettings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: AppSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.server_http_port, settings.server_http_port);
    }
}
