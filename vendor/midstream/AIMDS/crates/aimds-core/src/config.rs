//! Configuration management for AIMDS

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Main AIMDS configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AimdsConfig {
    #[serde(default)]
    pub detection: DetectionConfig,
    #[serde(default)]
    pub analysis: AnalysisConfig,
    #[serde(default)]
    pub response: ResponseConfig,
    #[serde(default)]
    pub system: SystemConfig,
}

/// Detection layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectionConfig {
    pub pattern_matching_enabled: bool,
    pub sanitization_enabled: bool,
    pub confidence_threshold: f64,
    pub max_pattern_complexity: usize,
    pub cache_size: usize,
}

impl Default for DetectionConfig {
    fn default() -> Self {
        Self {
            pattern_matching_enabled: true,
            sanitization_enabled: true,
            confidence_threshold: 0.75,
            max_pattern_complexity: 1000,
            cache_size: 10000,
        }
    }
}

/// Analysis layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalysisConfig {
    pub behavioral_analysis_enabled: bool,
    pub policy_verification_enabled: bool,
    pub ltl_checking_enabled: bool,
    pub threat_score_threshold: f64,
    pub max_temporal_window: Duration,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            behavioral_analysis_enabled: true,
            policy_verification_enabled: true,
            ltl_checking_enabled: true,
            threat_score_threshold: 0.8,
            max_temporal_window: Duration::from_secs(3600),
        }
    }
}

/// Response layer configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseConfig {
    pub meta_learning_enabled: bool,
    pub adaptive_responses_enabled: bool,
    pub auto_mitigation_enabled: bool,
    pub learning_rate: f64,
    pub response_timeout: Duration,
}

impl Default for ResponseConfig {
    fn default() -> Self {
        Self {
            meta_learning_enabled: true,
            adaptive_responses_enabled: true,
            auto_mitigation_enabled: true,
            learning_rate: 0.01,
            response_timeout: Duration::from_secs(5),
        }
    }
}

/// System-wide configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemConfig {
    pub max_concurrent_requests: usize,
    pub request_timeout: Duration,
    pub enable_metrics: bool,
    pub enable_tracing: bool,
    pub log_level: String,
}

impl Default for SystemConfig {
    fn default() -> Self {
        Self {
            max_concurrent_requests: 1000,
            request_timeout: Duration::from_secs(30),
            enable_metrics: true,
            enable_tracing: true,
            log_level: "info".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AimdsConfig::default();
        assert!(config.detection.pattern_matching_enabled);
        assert!(config.analysis.behavioral_analysis_enabled);
        assert!(config.response.meta_learning_enabled);
    }

    #[test]
    fn test_config_serialization() {
        let config = AimdsConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: AimdsConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(
            config.detection.confidence_threshold,
            deserialized.detection.confidence_threshold
        );
    }
}
