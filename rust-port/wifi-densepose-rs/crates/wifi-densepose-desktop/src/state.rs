use std::process::Child;
use std::sync::Mutex;
use std::time::Instant;

use crate::domain::node::DiscoveredNode;
use crate::domain::training::{
    CheckpointInfo, DatasetInfo, EpochMetrics, EvaluationMetrics,
    GpuInfo, JointAccuracy, RuVectorConfig, TrainingJob,
};

/// Sub-state for discovered nodes.
#[derive(Default)]
pub struct DiscoveryState {
    pub nodes: Vec<DiscoveredNode>,
    pub last_discovery: Option<Instant>,
}

/// Sub-state for the managed sensing server process.
pub struct ServerState {
    pub running: bool,
    pub pid: Option<u32>,
    pub http_port: Option<u16>,
    pub ws_port: Option<u16>,
    pub udp_port: Option<u16>,
    pub child: Option<Child>,
    pub start_time: Option<Instant>,
}

impl Default for ServerState {
    fn default() -> Self {
        Self {
            running: false,
            pid: None,
            http_port: None,
            ws_port: None,
            udp_port: None,
            child: None,
            start_time: None,
        }
    }
}

/// Sub-state for flash progress tracking.
#[derive(Default)]
pub struct FlashState {
    pub phase: String,
    pub progress_pct: f32,
    pub bytes_written: u64,
    pub bytes_total: u64,
    pub message: Option<String>,
    pub session_id: Option<String>,
}

/// Sub-state for OTA progress tracking.
#[derive(Default)]
pub struct OtaState {
    pub active_updates: Vec<OtaUpdateTracker>,
}

/// Tracks a single OTA update in progress.
pub struct OtaUpdateTracker {
    pub node_ip: String,
    pub phase: String,
    pub progress_pct: f32,
    pub started_at: Instant,
}

impl Default for OtaUpdateTracker {
    fn default() -> Self {
        Self {
            node_ip: String::new(),
            phase: "idle".into(),
            progress_pct: 0.0,
            started_at: Instant::now(),
        }
    }
}

/// Sub-state for application settings cache.
pub struct SettingsState {
    pub loaded: bool,
    pub dirty: bool,
}

impl Default for SettingsState {
    fn default() -> Self {
        Self {
            loaded: false,
            dirty: false,
        }
    }
}

/// Sub-state for training operations.
pub struct TrainingState {
    pub gpu_info: Option<GpuInfo>,
    pub datasets: Vec<DatasetInfo>,
    pub checkpoints: Vec<CheckpointInfo>,
    pub current_job: Option<TrainingJob>,
    pub ruvector_config: RuVectorConfig,
    pub training_history: Vec<EpochMetrics>,
    pub evaluation_metrics: Option<EvaluationMetrics>,
    pub joint_accuracies: Vec<JointAccuracy>,
}

impl Default for TrainingState {
    fn default() -> Self {
        Self {
            gpu_info: None,
            datasets: Vec::new(),
            checkpoints: Vec::new(),
            current_job: None,
            ruvector_config: RuVectorConfig::default(),
            training_history: Vec::new(),
            evaluation_metrics: None,
            joint_accuracies: Vec::new(),
        }
    }
}

/// Top-level application state managed by Tauri.
pub struct AppState {
    pub discovery: Mutex<DiscoveryState>,
    pub server: Mutex<ServerState>,
    pub flash: Mutex<FlashState>,
    pub ota: Mutex<OtaState>,
    pub settings: Mutex<SettingsState>,
    pub training: Mutex<TrainingState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            discovery: Mutex::new(DiscoveryState::default()),
            server: Mutex::new(ServerState::default()),
            flash: Mutex::new(FlashState::default()),
            ota: Mutex::new(OtaState::default()),
            settings: Mutex::new(SettingsState::default()),
            training: Mutex::new(TrainingState::default()),
        }
    }
}

impl AppState {
    /// Create a new AppState instance.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset all state to defaults.
    pub fn reset(&self) {
        if let Ok(mut discovery) = self.discovery.lock() {
            *discovery = DiscoveryState::default();
        }
        if let Ok(mut server) = self.server.lock() {
            // Kill child process if running
            if let Some(ref mut child) = server.child {
                let _ = child.kill();
            }
            *server = ServerState::default();
        }
        if let Ok(mut flash) = self.flash.lock() {
            *flash = FlashState::default();
        }
        if let Ok(mut ota) = self.ota.lock() {
            *ota = OtaState::default();
        }
        if let Ok(mut settings) = self.settings.lock() {
            *settings = SettingsState::default();
        }
        if let Ok(mut training) = self.training.lock() {
            *training = TrainingState::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_default() {
        let state = AppState::default();

        let discovery = state.discovery.lock().unwrap();
        assert!(discovery.nodes.is_empty());

        let server = state.server.lock().unwrap();
        assert!(!server.running);
        assert!(server.pid.is_none());
    }

    #[test]
    fn test_app_state_reset() {
        let state = AppState::new();

        // Modify state
        {
            let mut discovery = state.discovery.lock().unwrap();
            discovery.nodes.push(DiscoveredNode {
                ip: "192.168.1.100".into(),
                mac: Some("AA:BB:CC:DD:EE:FF".into()),
                hostname: None,
                node_id: 1,
                firmware_version: None,
                health: crate::domain::node::HealthStatus::Online,
                last_seen: chrono::Utc::now().to_rfc3339(),
                chip: crate::domain::node::Chip::default(),
                mesh_role: crate::domain::node::MeshRole::default(),
                discovery_method: crate::domain::node::DiscoveryMethod::default(),
                tdm_slot: None,
                tdm_total: None,
                edge_tier: None,
                uptime_secs: None,
                capabilities: None,
                friendly_name: None,
                notes: None,
            });
        }

        // Reset
        state.reset();

        // Verify reset
        let discovery = state.discovery.lock().unwrap();
        assert!(discovery.nodes.is_empty());
    }

    #[test]
    fn test_server_state() {
        let server = ServerState::default();
        assert!(!server.running);
        assert!(server.child.is_none());
        assert!(server.start_time.is_none());
    }

    #[test]
    fn test_flash_state() {
        let flash = FlashState::default();
        assert_eq!(flash.phase, "");
        assert_eq!(flash.progress_pct, 0.0);
    }
}
