use serde::{Deserialize, Serialize};

/// NVS provisioning configuration for a single ESP32 node.
/// Maps to the firmware's nvs_config_t struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProvisioningConfig {
    pub wifi_ssid: Option<String>,
    pub wifi_password: Option<String>,
    pub target_ip: Option<String>,
    pub target_port: Option<u16>,
    pub node_id: Option<u8>,
    pub tdm_slot: Option<u8>,
    pub tdm_total: Option<u8>,
    pub edge_tier: Option<u8>,
    pub presence_thresh: Option<u16>,
    pub fall_thresh: Option<u16>,
    pub vital_window: Option<u16>,
    pub vital_interval_ms: Option<u16>,
    pub top_k_count: Option<u8>,
    pub hop_count: Option<u8>,
    pub channel_list: Option<Vec<u8>>,
    pub dwell_ms: Option<u32>,
    pub power_duty: Option<u8>,
    pub wasm_max_modules: Option<u8>,
    pub wasm_verify: Option<bool>,
    pub ota_psk: Option<String>,
}

impl ProvisioningConfig {
    /// Validate invariants:
    /// - tdm_slot < tdm_total when both set
    /// - channel_list.len() == hop_count when both set
    /// - 10 <= power_duty <= 100
    pub fn validate(&self) -> Result<(), String> {
        if let (Some(slot), Some(total)) = (self.tdm_slot, self.tdm_total) {
            if slot >= total {
                return Err(format!(
                    "tdm_slot ({}) must be less than tdm_total ({})",
                    slot, total
                ));
            }
        }
        if let (Some(ref channels), Some(hops)) = (&self.channel_list, self.hop_count) {
            if channels.len() != hops as usize {
                return Err(format!(
                    "channel_list length ({}) must equal hop_count ({})",
                    channels.len(),
                    hops
                ));
            }
        }
        if let Some(duty) = self.power_duty {
            if !(10..=100).contains(&duty) {
                return Err(format!(
                    "power_duty ({}) must be between 10 and 100",
                    duty
                ));
            }
        }
        Ok(())
    }
}

/// Mesh-level configuration that generates per-node ProvisioningConfig instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshConfig {
    pub common: ProvisioningConfig,
    pub nodes: Vec<MeshNodeEntry>,
}

/// Per-node override within a mesh configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeshNodeEntry {
    pub port: String,
    pub node_id: u8,
    pub tdm_slot: u8,
}

impl MeshConfig {
    /// Generate a ProvisioningConfig for a specific mesh node,
    /// merging common settings with per-node overrides.
    pub fn config_for_node(&self, entry: &MeshNodeEntry) -> ProvisioningConfig {
        let mut cfg = self.common.clone();
        cfg.node_id = Some(entry.node_id);
        cfg.tdm_slot = Some(entry.tdm_slot);
        cfg.tdm_total = Some(self.nodes.len() as u8);
        cfg
    }
}
