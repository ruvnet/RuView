//! Persisted runtime configuration (ADR-044 §5.3).
//!
//! Stores operator-tunable parameters across server restarts in `<data_dir>/config.json`.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Runtime configuration that persists across server restarts via `data/config.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Divisor for multi-node person-count deduplication (sum / factor).
    pub dedup_factor: f64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self { dedup_factor: 3.0 }
    }
}

/// Load persisted runtime config from `<data_dir>/config.json`.
/// Falls back to [`RuntimeConfig::default`] if the file is absent or malformed.
pub fn load_runtime_config(data_dir: &Path) -> RuntimeConfig {
    let path = data_dir.join("config.json");
    match std::fs::read_to_string(&path) {
        Ok(json) => serde_json::from_str(&json).unwrap_or_default(),
        Err(_) => RuntimeConfig::default(),
    }
}

/// Persist runtime config to `<data_dir>/config.json`.
pub fn save_runtime_config(data_dir: &Path, config: &RuntimeConfig) {
    let path = data_dir.join("config.json");
    if let Ok(json) = serde_json::to_string_pretty(config) {
        let _ = std::fs::write(&path, json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_dedup_factor_three() {
        let cfg = RuntimeConfig::default();
        assert!((cfg.dedup_factor - 3.0).abs() < 1e-6);
    }

    #[test]
    fn roundtrip_serialization() {
        let original = RuntimeConfig { dedup_factor: 2.5 };
        let serialized = serde_json::to_string(&original).expect("serialize");
        let deserialized: RuntimeConfig = serde_json::from_str(&serialized).expect("deserialize");
        assert!((deserialized.dedup_factor - 2.5).abs() < 1e-6);
    }
}

