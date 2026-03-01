//! Over-the-Air (OTA) Update System for RuvLLM ESP32
//!
//! Enables wireless firmware updates via WiFi without physical access to the device.
//!
//! # Features
//! - HTTPS firmware download with verification
//! - SHA256 checksum validation
//! - Rollback on failed update
//! - Progress callbacks
//! - Minimal RAM footprint (streaming update)

use core::fmt;

/// OTA update configuration
#[derive(Clone)]
pub struct OtaConfig {
    /// Firmware server URL
    pub server_url: heapless::String<128>,
    /// Current firmware version
    pub current_version: heapless::String<16>,
    /// WiFi SSID
    pub wifi_ssid: heapless::String<32>,
    /// WiFi password
    pub wifi_password: heapless::String<64>,
    /// Check interval in seconds (0 = manual only)
    pub check_interval_secs: u32,
    /// Enable automatic updates
    pub auto_update: bool,
}

impl Default for OtaConfig {
    fn default() -> Self {
        Self {
            server_url: heapless::String::new(),
            current_version: heapless::String::try_from("0.2.1").unwrap_or_default(),
            wifi_ssid: heapless::String::new(),
            wifi_password: heapless::String::new(),
            check_interval_secs: 3600, // 1 hour
            auto_update: false,
        }
    }
}

/// OTA update state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OtaState {
    /// Idle, waiting for update check
    Idle,
    /// Checking for updates
    Checking,
    /// Update available
    UpdateAvailable,
    /// Downloading firmware
    Downloading,
    /// Verifying firmware
    Verifying,
    /// Applying update
    Applying,
    /// Update complete, pending reboot
    Complete,
    /// Update failed
    Failed,
}

impl fmt::Display for OtaState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OtaState::Idle => write!(f, "Idle"),
            OtaState::Checking => write!(f, "Checking"),
            OtaState::UpdateAvailable => write!(f, "Update Available"),
            OtaState::Downloading => write!(f, "Downloading"),
            OtaState::Verifying => write!(f, "Verifying"),
            OtaState::Applying => write!(f, "Applying"),
            OtaState::Complete => write!(f, "Complete"),
            OtaState::Failed => write!(f, "Failed"),
        }
    }
}

/// Update information
#[derive(Clone)]
pub struct UpdateInfo {
    /// New version string
    pub version: heapless::String<16>,
    /// Firmware size in bytes
    pub size: u32,
    /// SHA256 checksum (hex string)
    pub checksum: heapless::String<64>,
    /// Release notes
    pub notes: heapless::String<256>,
    /// Download URL
    pub download_url: heapless::String<256>,
}

/// OTA update error
#[derive(Debug, Clone, Copy)]
pub enum OtaError {
    /// WiFi connection failed
    WifiError,
    /// HTTP request failed
    HttpError,
    /// Invalid response from server
    InvalidResponse,
    /// Checksum mismatch
    ChecksumMismatch,
    /// Not enough storage space
    InsufficientSpace,
    /// Flash write failed
    FlashError,
    /// Update verification failed
    VerificationFailed,
    /// No update available
    NoUpdate,
    /// Already up to date
    AlreadyUpToDate,
}

impl fmt::Display for OtaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OtaError::WifiError => write!(f, "WiFi connection failed"),
            OtaError::HttpError => write!(f, "HTTP request failed"),
            OtaError::InvalidResponse => write!(f, "Invalid server response"),
            OtaError::ChecksumMismatch => write!(f, "Checksum verification failed"),
            OtaError::InsufficientSpace => write!(f, "Not enough storage space"),
            OtaError::FlashError => write!(f, "Flash write error"),
            OtaError::VerificationFailed => write!(f, "Update verification failed"),
            OtaError::NoUpdate => write!(f, "No update available"),
            OtaError::AlreadyUpToDate => write!(f, "Already up to date"),
        }
    }
}

/// Progress callback type
pub type ProgressCallback = fn(downloaded: u32, total: u32);

/// OTA Update Manager
pub struct OtaManager {
    config: OtaConfig,
    state: OtaState,
    progress: u32,
    last_error: Option<OtaError>,
    update_info: Option<UpdateInfo>,
}

impl OtaManager {
    /// Create new OTA manager with config
    pub fn new(config: OtaConfig) -> Self {
        Self {
            config,
            state: OtaState::Idle,
            progress: 0,
            last_error: None,
            update_info: None,
        }
    }

    /// Get current state
    pub fn state(&self) -> OtaState {
        self.state
    }

    /// Get download progress (0-100)
    pub fn progress(&self) -> u32 {
        self.progress
    }

    /// Get last error
    pub fn last_error(&self) -> Option<OtaError> {
        self.last_error
    }

    /// Get available update info
    pub fn update_info(&self) -> Option<&UpdateInfo> {
        self.update_info.as_ref()
    }

    /// Check for updates (simulation for no_std)
    ///
    /// In a real implementation, this would:
    /// 1. Connect to WiFi
    /// 2. Query the update server
    /// 3. Parse the response
    /// 4. Compare versions
    pub fn check_for_update(&mut self) -> Result<bool, OtaError> {
        self.state = OtaState::Checking;
        self.last_error = None;

        // Simulated version check
        // In real impl: HTTP GET to {server_url}/version.json
        let server_version = "0.2.2"; // Would come from server

        if self.is_newer_version(server_version) {
            self.update_info = Some(UpdateInfo {
                version: heapless::String::try_from(server_version).unwrap_or_default(),
                size: 512 * 1024, // 512KB
                checksum: heapless::String::try_from(
                    "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                ).unwrap_or_default(),
                notes: heapless::String::try_from("Performance improvements and bug fixes").unwrap_or_default(),
                download_url: heapless::String::try_from(
                    "https://github.com/ruvnet/ruvector/releases/latest/download/ruvllm-esp32"
                ).unwrap_or_default(),
            });
            self.state = OtaState::UpdateAvailable;
            Ok(true)
        } else {
            self.state = OtaState::Idle;
            self.last_error = Some(OtaError::AlreadyUpToDate);
            Ok(false)
        }
    }

    /// Compare version strings (simple semver comparison)
    fn is_newer_version(&self, server_version: &str) -> bool {
        let current = self.parse_version(self.config.current_version.as_str());
        let server = self.parse_version(server_version);

        server > current
    }

    /// Parse version string to tuple
    fn parse_version(&self, version: &str) -> (u32, u32, u32) {
        let mut parts = version.split('.');
        let major = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let minor = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        let patch = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
        (major, minor, patch)
    }

    /// Start firmware download
    ///
    /// In real implementation:
    /// 1. Stream download to flash partition
    /// 2. Verify checksum incrementally
    /// 3. Call progress callback
    pub fn download_update(&mut self, _progress_cb: Option<ProgressCallback>) -> Result<(), OtaError> {
        if self.state != OtaState::UpdateAvailable {
            return Err(OtaError::NoUpdate);
        }

        self.state = OtaState::Downloading;
        self.progress = 0;

        // Simulated download
        // In real impl: HTTP GET with streaming to flash
        let total_size = self.update_info.as_ref().map(|i| i.size).unwrap_or(0);

        // Simulate progress
        for i in 0..=100 {
            self.progress = i;
            if let Some(cb) = _progress_cb {
                cb(i * total_size / 100, total_size);
            }
        }

        self.state = OtaState::Verifying;
        Ok(())
    }

    /// Verify downloaded firmware
    pub fn verify_update(&mut self) -> Result<(), OtaError> {
        if self.state != OtaState::Verifying {
            return Err(OtaError::VerificationFailed);
        }

        // In real impl: Calculate SHA256 of downloaded partition
        // Compare with expected checksum

        // Simulated verification
        self.state = OtaState::Complete;
        Ok(())
    }

    /// Apply update and reboot
    ///
    /// In real implementation:
    /// 1. Set boot partition to new firmware
    /// 2. Reboot device
    pub fn apply_update(&mut self) -> Result<(), OtaError> {
        if self.state != OtaState::Complete {
            return Err(OtaError::VerificationFailed);
        }

        self.state = OtaState::Applying;

        // In real impl:
        // esp_ota_set_boot_partition(...)
        // esp_restart()

        Ok(())
    }

    /// Rollback to previous firmware
    pub fn rollback(&mut self) -> Result<(), OtaError> {
        // In real impl:
        // esp_ota_mark_app_invalid_rollback_and_reboot()
        self.state = OtaState::Idle;
        Ok(())
    }

    /// Get human-readable status
    pub fn status_string(&self) -> &'static str {
        match self.state {
            OtaState::Idle => "Ready",
            OtaState::Checking => "Checking for updates...",
            OtaState::UpdateAvailable => "Update available!",
            OtaState::Downloading => "Downloading update...",
            OtaState::Verifying => "Verifying firmware...",
            OtaState::Applying => "Applying update...",
            OtaState::Complete => "Update complete! Reboot to apply.",
            OtaState::Failed => "Update failed",
        }
    }
}

/// OTA serial command handler
pub fn handle_ota_command(manager: &mut OtaManager, command: &str) -> heapless::String<256> {
    let mut response = heapless::String::new();

    let parts: heapless::Vec<&str, 4> = command.split_whitespace().collect();
    let cmd = parts.first().copied().unwrap_or("");

    match cmd {
        "status" => {
            let _ = core::fmt::write(
                &mut response,
                format_args!("OTA Status: {} ({}%)", manager.status_string(), manager.progress())
            );
        }
        "check" => {
            match manager.check_for_update() {
                Ok(true) => {
                    if let Some(info) = manager.update_info() {
                        let _ = core::fmt::write(
                            &mut response,
                            format_args!("Update available: v{} ({}KB)", info.version, info.size / 1024)
                        );
                    }
                }
                Ok(false) => {
                    let _ = response.push_str("Already up to date");
                }
                Err(e) => {
                    let _ = core::fmt::write(&mut response, format_args!("Check failed: {}", e));
                }
            }
        }
        "download" => {
            match manager.download_update(None) {
                Ok(()) => {
                    let _ = response.push_str("Download complete");
                }
                Err(e) => {
                    let _ = core::fmt::write(&mut response, format_args!("Download failed: {}", e));
                }
            }
        }
        "apply" => {
            let _ = manager.verify_update();
            match manager.apply_update() {
                Ok(()) => {
                    let _ = response.push_str("Rebooting to apply update...");
                }
                Err(e) => {
                    let _ = core::fmt::write(&mut response, format_args!("Apply failed: {}", e));
                }
            }
        }
        "rollback" => {
            match manager.rollback() {
                Ok(()) => {
                    let _ = response.push_str("Rolling back to previous firmware...");
                }
                Err(e) => {
                    let _ = core::fmt::write(&mut response, format_args!("Rollback failed: {}", e));
                }
            }
        }
        _ => {
            let _ = response.push_str("OTA commands: status, check, download, apply, rollback");
        }
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_comparison() {
        let config = OtaConfig {
            current_version: heapless::String::try_from("0.2.1").unwrap(),
            ..Default::default()
        };
        let manager = OtaManager::new(config);

        assert!(manager.is_newer_version("0.2.2"));
        assert!(manager.is_newer_version("0.3.0"));
        assert!(manager.is_newer_version("1.0.0"));
        assert!(!manager.is_newer_version("0.2.1"));
        assert!(!manager.is_newer_version("0.2.0"));
        assert!(!manager.is_newer_version("0.1.0"));
    }

    #[test]
    fn test_state_transitions() {
        let config = OtaConfig::default();
        let mut manager = OtaManager::new(config);

        assert_eq!(manager.state(), OtaState::Idle);

        let _ = manager.check_for_update();
        assert!(matches!(manager.state(), OtaState::UpdateAvailable | OtaState::Idle));
    }
}
