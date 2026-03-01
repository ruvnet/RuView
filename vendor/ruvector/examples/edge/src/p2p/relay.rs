//! Relay Manager - GUN Relay Health Monitoring
//!
//! Tracks relay health, latency, and provides automatic failover

use std::collections::HashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Relay health status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayStatus {
    pub url: String,
    pub healthy: bool,
    pub last_check: u64,
    pub latency_ms: u32,
    pub failures: u8,
}

impl RelayStatus {
    fn new(url: String) -> Self {
        Self {
            url,
            healthy: true, // Assume healthy initially
            last_check: 0,
            latency_ms: 0,
            failures: 0,
        }
    }
}

/// Bootstrap GUN relays (public infrastructure)
pub const BOOTSTRAP_RELAYS: &[&str] = &[
    "https://gun-manhattan.herokuapp.com/gun",
    "https://gun-us.herokuapp.com/gun",
    "https://gun-eu.herokuapp.com/gun",
];

/// Relay Manager for health tracking and failover
pub struct RelayManager {
    relays: Arc<RwLock<HashMap<String, RelayStatus>>>,
    max_failures: u8,
}

impl RelayManager {
    /// Create with bootstrap relays
    pub fn new() -> Self {
        Self::with_relays(BOOTSTRAP_RELAYS.iter().map(|s| s.to_string()).collect())
    }

    /// Create with custom relays
    pub fn with_relays(relay_urls: Vec<String>) -> Self {
        let mut relays = HashMap::new();
        for url in relay_urls {
            relays.insert(url.clone(), RelayStatus::new(url));
        }

        Self {
            relays: Arc::new(RwLock::new(relays)),
            max_failures: 3,
        }
    }

    /// Get list of healthy relays
    pub fn get_healthy_relays(&self) -> Vec<String> {
        self.relays.read()
            .values()
            .filter(|r| r.healthy)
            .map(|r| r.url.clone())
            .collect()
    }

    /// Get all relays (for initial connection attempts)
    pub fn get_all_relays(&self) -> Vec<String> {
        self.relays.read()
            .keys()
            .cloned()
            .collect()
    }

    /// Mark relay as successful
    pub fn mark_success(&self, url: &str, latency_ms: u32) {
        let mut relays = self.relays.write();
        if let Some(relay) = relays.get_mut(url) {
            relay.healthy = true;
            relay.latency_ms = latency_ms;
            relay.failures = 0;
            relay.last_check = chrono::Utc::now().timestamp_millis() as u64;
        }
    }

    /// Mark relay as failed
    pub fn mark_failed(&self, url: &str) {
        let mut relays = self.relays.write();
        if let Some(relay) = relays.get_mut(url) {
            relay.failures += 1;
            relay.last_check = chrono::Utc::now().timestamp_millis() as u64;

            if relay.failures >= self.max_failures {
                relay.healthy = false;
                tracing::warn!("Relay marked unhealthy: {} (failures: {})", url, relay.failures);
            }
        }
    }

    /// Add new relay
    pub fn add_relay(&self, url: String) {
        let mut relays = self.relays.write();
        if !relays.contains_key(&url) {
            relays.insert(url.clone(), RelayStatus::new(url));
        }
    }

    /// Remove relay
    pub fn remove_relay(&self, url: &str) {
        self.relays.write().remove(url);
    }

    /// Get relay metrics
    pub fn get_metrics(&self) -> RelayMetrics {
        let relays = self.relays.read();
        let healthy_relays: Vec<_> = relays.values().filter(|r| r.healthy).collect();

        let avg_latency = if healthy_relays.is_empty() {
            0
        } else {
            healthy_relays.iter().map(|r| r.latency_ms as u64).sum::<u64>() / healthy_relays.len() as u64
        };

        RelayMetrics {
            total: relays.len(),
            healthy: healthy_relays.len(),
            avg_latency_ms: avg_latency as u32,
        }
    }

    /// Export working relay set (for persistence)
    pub fn export_working_set(&self) -> Vec<String> {
        self.get_healthy_relays()
    }

    /// Import relay set
    pub fn import_relays(&self, urls: Vec<String>) {
        for url in urls {
            self.add_relay(url);
        }
    }

    /// Reset failed relay (give it another chance)
    pub fn reset_relay(&self, url: &str) {
        let mut relays = self.relays.write();
        if let Some(relay) = relays.get_mut(url) {
            relay.healthy = true;
            relay.failures = 0;
        }
    }

    /// Get relay by URL
    pub fn get_relay(&self, url: &str) -> Option<RelayStatus> {
        self.relays.read().get(url).cloned()
    }
}

impl Default for RelayManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Relay health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayMetrics {
    pub total: usize,
    pub healthy: usize,
    pub avg_latency_ms: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relay_manager() {
        let manager = RelayManager::new();

        // Should start with bootstrap relays
        let relays = manager.get_all_relays();
        assert!(!relays.is_empty());

        // All should be healthy initially
        let healthy = manager.get_healthy_relays();
        assert_eq!(relays.len(), healthy.len());
    }

    #[test]
    fn test_relay_failure_tracking() {
        let manager = RelayManager::with_relays(vec!["http://test:8080".to_string()]);

        // Mark failed 3 times
        manager.mark_failed("http://test:8080");
        manager.mark_failed("http://test:8080");
        assert!(manager.get_healthy_relays().len() == 1); // Still healthy

        manager.mark_failed("http://test:8080");
        assert!(manager.get_healthy_relays().is_empty()); // Now unhealthy
    }

    #[test]
    fn test_relay_success_resets_failures() {
        let manager = RelayManager::with_relays(vec!["http://test:8080".to_string()]);

        manager.mark_failed("http://test:8080");
        manager.mark_failed("http://test:8080");
        manager.mark_success("http://test:8080", 50);

        // Should still be healthy after success
        assert!(!manager.get_healthy_relays().is_empty());
    }
}
