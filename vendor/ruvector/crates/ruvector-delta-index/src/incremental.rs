//! Incremental index updates
//!
//! Provides efficient strategies for updating the index without full rebuild.

use std::collections::HashMap;

use ruvector_delta_core::{Delta, VectorDelta};

use crate::{DeltaHnsw, Result, SearchResult};

/// Configuration for incremental updates
#[derive(Debug, Clone)]
pub struct IncrementalConfig {
    /// Minimum delta magnitude to trigger reconnection
    pub reconnect_threshold: f32,
    /// Maximum pending updates before batch processing
    pub batch_threshold: usize,
    /// Whether to use lazy reconnection
    pub lazy_reconnect: bool,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            reconnect_threshold: 0.1,
            batch_threshold: 100,
            lazy_reconnect: true,
        }
    }
}

/// Handles incremental updates to the HNSW index
pub struct IncrementalUpdater {
    config: IncrementalConfig,
    pending_updates: HashMap<String, VectorDelta>,
    total_updates: usize,
}

impl IncrementalUpdater {
    /// Create a new incremental updater
    pub fn new(config: IncrementalConfig) -> Self {
        Self {
            config,
            pending_updates: HashMap::new(),
            total_updates: 0,
        }
    }

    /// Queue an update for batch processing
    pub fn queue_update(&mut self, id: String, delta: VectorDelta) {
        self.pending_updates
            .entry(id)
            .and_modify(|existing| {
                *existing = existing.clone().compose(delta.clone());
            })
            .or_insert(delta);

        self.total_updates += 1;
    }

    /// Check if batch processing is needed
    pub fn needs_flush(&self) -> bool {
        self.pending_updates.len() >= self.config.batch_threshold
    }

    /// Flush pending updates to the index
    pub fn flush(&mut self, index: &mut DeltaHnsw) -> Result<FlushResult> {
        let mut applied = 0;
        let mut reconnected = 0;
        let mut errors = Vec::new();

        let updates: Vec<_> = self.pending_updates.drain().collect();

        for (id, delta) in updates {
            match index.apply_delta(&id, &delta) {
                Ok(()) => {
                    applied += 1;

                    // Check if reconnection is needed
                    if delta.l2_norm() > self.config.reconnect_threshold {
                        reconnected += 1;
                    }
                }
                Err(e) => {
                    errors.push((id, e.to_string()));
                }
            }
        }

        Ok(FlushResult {
            applied,
            reconnected,
            errors,
        })
    }

    /// Get number of pending updates
    pub fn pending_count(&self) -> usize {
        self.pending_updates.len()
    }

    /// Get total updates processed
    pub fn total_updates(&self) -> usize {
        self.total_updates
    }

    /// Clear pending updates without applying
    pub fn clear_pending(&mut self) {
        self.pending_updates.clear();
    }
}

/// Result of flushing updates
#[derive(Debug)]
pub struct FlushResult {
    /// Number of updates applied
    pub applied: usize,
    /// Number of nodes reconnected
    pub reconnected: usize,
    /// Errors encountered
    pub errors: Vec<(String, String)>,
}

/// Strategies for handling vector updates
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UpdateStrategy {
    /// Apply delta without graph modification
    DeltaOnly,
    /// Apply delta and update local neighbors
    LocalRepair,
    /// Apply delta and full reconnection
    FullReconnect,
    /// Queue for batch processing
    Deferred,
}

/// Determine the best update strategy based on delta magnitude
pub fn select_strategy(delta: &VectorDelta, config: &IncrementalConfig) -> UpdateStrategy {
    let magnitude = delta.l2_norm();

    if magnitude < config.reconnect_threshold * 0.1 {
        UpdateStrategy::DeltaOnly
    } else if magnitude < config.reconnect_threshold {
        if config.lazy_reconnect {
            UpdateStrategy::DeltaOnly
        } else {
            UpdateStrategy::LocalRepair
        }
    } else if magnitude < config.reconnect_threshold * 5.0 {
        UpdateStrategy::LocalRepair
    } else {
        UpdateStrategy::FullReconnect
    }
}

/// Statistics about incremental updates
#[derive(Debug, Clone, Default)]
pub struct UpdateStats {
    /// Total updates applied
    pub total_applied: usize,
    /// Updates that triggered reconnection
    pub reconnections: usize,
    /// Updates that were delta-only
    pub delta_only: usize,
    /// Average delta magnitude
    pub avg_magnitude: f32,
    /// Maximum delta magnitude
    pub max_magnitude: f32,
}

impl UpdateStats {
    /// Record an update
    pub fn record(&mut self, delta: &VectorDelta, reconnected: bool) {
        let mag = delta.l2_norm();

        self.total_applied += 1;
        if reconnected {
            self.reconnections += 1;
        } else {
            self.delta_only += 1;
        }

        // Update running average
        let n = self.total_applied as f32;
        self.avg_magnitude = self.avg_magnitude * ((n - 1.0) / n) + mag / n;
        self.max_magnitude = self.max_magnitude.max(mag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_updater() {
        let mut updater = IncrementalUpdater::new(IncrementalConfig::default());

        let delta = VectorDelta::from_dense(vec![0.1, 0.2, 0.3]);
        updater.queue_update("test".to_string(), delta);

        assert_eq!(updater.pending_count(), 1);
        assert_eq!(updater.total_updates(), 1);
    }

    #[test]
    fn test_delta_composition() {
        let mut updater = IncrementalUpdater::new(IncrementalConfig::default());

        let delta1 = VectorDelta::from_dense(vec![1.0, 0.0, 0.0]);
        let delta2 = VectorDelta::from_dense(vec![0.0, 1.0, 0.0]);

        updater.queue_update("test".to_string(), delta1);
        updater.queue_update("test".to_string(), delta2);

        // Should compose into single update
        assert_eq!(updater.pending_count(), 1);
    }

    #[test]
    fn test_strategy_selection() {
        let config = IncrementalConfig {
            reconnect_threshold: 0.5,
            ..Default::default()
        };

        // Small delta -> DeltaOnly
        let small = VectorDelta::from_dense(vec![0.01, 0.01, 0.01]);
        assert_eq!(select_strategy(&small, &config), UpdateStrategy::DeltaOnly);

        // Large delta -> FullReconnect
        let large = VectorDelta::from_dense(vec![10.0, 10.0, 10.0]);
        assert_eq!(
            select_strategy(&large, &config),
            UpdateStrategy::FullReconnect
        );
    }
}
