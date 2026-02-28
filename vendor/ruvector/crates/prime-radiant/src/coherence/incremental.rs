//! Incremental Coherence Computation
//!
//! This module provides efficient incremental updates to coherence energy
//! when only a subset of nodes or edges change. Instead of recomputing
//! the entire graph, we:
//!
//! 1. Track which edges are affected by each node update
//! 2. Recompute only those edge residuals
//! 3. Update the aggregate energy incrementally
//!
//! # Algorithm
//!
//! For a node update at node v:
//! 1. Find all edges incident to v: E_v = {(u,v) | (u,v) in E}
//! 2. For each edge e in E_v, recompute residual r_e
//! 3. Update total energy: E' = E - sum(old_e) + sum(new_e) for e in E_v
//!
//! # Complexity
//!
//! - Full computation: O(|E|) where E is the edge set
//! - Incremental update: O(deg(v)) where deg(v) is the degree of updated node
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::coherence::{IncrementalEngine, IncrementalConfig};
//!
//! let engine = IncrementalEngine::new(IncrementalConfig::default());
//!
//! // Full computation first
//! let energy = engine.compute_full();
//!
//! // Subsequent updates are incremental
//! engine.node_updated("fact_1");
//! let delta = engine.compute_incremental();
//!
//! println!("Energy changed by: {}", delta.energy_delta);
//! ```

use super::energy::{CoherenceEnergy, EdgeEnergy, EdgeId};
use super::engine::{CoherenceEngine, NodeId};
use chrono::{DateTime, Utc};
#[cfg(feature = "parallel")]
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Configuration for incremental computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalConfig {
    /// Whether to use incremental mode
    pub enabled: bool,
    /// Threshold for switching to full recomputation (percentage of edges affected)
    pub full_recompute_threshold: f32,
    /// Whether to batch multiple node updates
    pub batch_updates: bool,
    /// Maximum batch size before forcing computation
    pub max_batch_size: usize,
    /// Whether to track energy history for trend analysis
    pub track_history: bool,
    /// Maximum history entries to keep
    pub history_size: usize,
}

impl Default for IncrementalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            full_recompute_threshold: 0.3, // 30% of edges affected -> full recompute
            batch_updates: true,
            max_batch_size: 100,
            track_history: true,
            history_size: 1000,
        }
    }
}

/// Result of an incremental computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeltaResult {
    /// Change in total energy
    pub energy_delta: f32,
    /// New total energy
    pub new_energy: f32,
    /// Previous total energy
    pub old_energy: f32,
    /// Number of edges recomputed
    pub edges_recomputed: usize,
    /// Total edges in graph
    pub total_edges: usize,
    /// Whether full recomputation was used
    pub was_full_recompute: bool,
    /// Computation time in microseconds
    pub compute_time_us: u64,
    /// Timestamp
    pub timestamp: DateTime<Utc>,
}

impl DeltaResult {
    /// Get the relative energy change
    pub fn relative_change(&self) -> f32 {
        if self.old_energy > 1e-10 {
            self.energy_delta / self.old_energy
        } else {
            if self.new_energy > 1e-10 {
                1.0
            } else {
                0.0
            }
        }
    }

    /// Check if energy increased
    #[inline]
    pub fn energy_increased(&self) -> bool {
        self.energy_delta > 0.0
    }

    /// Check if energy decreased
    #[inline]
    pub fn energy_decreased(&self) -> bool {
        self.energy_delta < 0.0
    }
}

/// Update event for tracking changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateEvent {
    /// A node's state was updated
    NodeUpdated {
        node_id: NodeId,
        affected_edges: Vec<EdgeId>,
        timestamp: DateTime<Utc>,
    },
    /// An edge was added
    EdgeAdded {
        edge_id: EdgeId,
        timestamp: DateTime<Utc>,
    },
    /// An edge was removed
    EdgeRemoved {
        edge_id: EdgeId,
        old_energy: f32,
        timestamp: DateTime<Utc>,
    },
    /// A node was added
    NodeAdded {
        node_id: NodeId,
        timestamp: DateTime<Utc>,
    },
    /// A node was removed
    NodeRemoved {
        node_id: NodeId,
        removed_edges: Vec<EdgeId>,
        removed_energy: f32,
        timestamp: DateTime<Utc>,
    },
}

impl UpdateEvent {
    /// Get the timestamp of this event
    pub fn timestamp(&self) -> DateTime<Utc> {
        match self {
            UpdateEvent::NodeUpdated { timestamp, .. } => *timestamp,
            UpdateEvent::EdgeAdded { timestamp, .. } => *timestamp,
            UpdateEvent::EdgeRemoved { timestamp, .. } => *timestamp,
            UpdateEvent::NodeAdded { timestamp, .. } => *timestamp,
            UpdateEvent::NodeRemoved { timestamp, .. } => *timestamp,
        }
    }

    /// Check if this event affects the given edge
    pub fn affects_edge(&self, edge_id: &str) -> bool {
        match self {
            UpdateEvent::NodeUpdated { affected_edges, .. } => {
                affected_edges.contains(&edge_id.to_string())
            }
            UpdateEvent::EdgeAdded { edge_id: eid, .. } => eid == edge_id,
            UpdateEvent::EdgeRemoved { edge_id: eid, .. } => eid == edge_id,
            UpdateEvent::NodeAdded { .. } => false,
            UpdateEvent::NodeRemoved { removed_edges, .. } => {
                removed_edges.contains(&edge_id.to_string())
            }
        }
    }
}

/// Cache for incremental computation
#[derive(Debug, Default)]
pub struct IncrementalCache {
    /// Cached edge energies (edge_id -> energy value)
    edge_energies: HashMap<EdgeId, f32>,
    /// Cached edge residuals (edge_id -> residual vector)
    edge_residuals: HashMap<EdgeId, Vec<f32>>,
    /// Total cached energy
    total_energy: f32,
    /// Fingerprint when cache was last valid
    last_fingerprint: String,
    /// Dirty edges that need recomputation
    dirty_edges: HashSet<EdgeId>,
    /// Removed edge energies (for delta calculation)
    removed_energies: HashMap<EdgeId, f32>,
}

impl IncrementalCache {
    /// Create a new empty cache
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if the cache is valid for the given fingerprint
    #[inline]
    pub fn is_valid(&self, fingerprint: &str) -> bool {
        self.last_fingerprint == fingerprint && self.dirty_edges.is_empty()
    }

    /// Mark an edge as dirty (needs recomputation)
    pub fn mark_dirty(&mut self, edge_id: impl Into<EdgeId>) {
        self.dirty_edges.insert(edge_id.into());
    }

    /// Mark all edges incident to a node as dirty
    pub fn mark_node_dirty(&mut self, incident_edges: &[EdgeId]) {
        for edge_id in incident_edges {
            self.dirty_edges.insert(edge_id.clone());
        }
    }

    /// Update the cache with new edge energy
    pub fn update_edge(&mut self, edge_id: impl Into<EdgeId>, energy: f32, residual: Vec<f32>) {
        let edge_id = edge_id.into();

        // Remove from dirty set
        self.dirty_edges.remove(&edge_id);

        // Update energy tracking
        if let Some(old_energy) = self.edge_energies.get(&edge_id) {
            self.total_energy -= old_energy;
        }
        self.total_energy += energy;

        self.edge_energies.insert(edge_id.clone(), energy);
        self.edge_residuals.insert(edge_id, residual);
    }

    /// Remove an edge from the cache
    pub fn remove_edge(&mut self, edge_id: &str) {
        if let Some(energy) = self.edge_energies.remove(edge_id) {
            self.total_energy -= energy;
            self.removed_energies.insert(edge_id.to_string(), energy);
        }
        self.edge_residuals.remove(edge_id);
        self.dirty_edges.remove(edge_id);
    }

    /// Get cached energy for an edge
    pub fn get_energy(&self, edge_id: &str) -> Option<f32> {
        self.edge_energies.get(edge_id).copied()
    }

    /// Get cached residual for an edge
    pub fn get_residual(&self, edge_id: &str) -> Option<&Vec<f32>> {
        self.edge_residuals.get(edge_id)
    }

    /// Get the total cached energy
    #[inline]
    pub fn total_energy(&self) -> f32 {
        self.total_energy
    }

    /// Get the number of dirty edges
    #[inline]
    pub fn dirty_count(&self) -> usize {
        self.dirty_edges.len()
    }

    /// Get dirty edge IDs
    pub fn dirty_edges(&self) -> &HashSet<EdgeId> {
        &self.dirty_edges
    }

    /// Set the fingerprint
    pub fn set_fingerprint(&mut self, fingerprint: impl Into<String>) {
        self.last_fingerprint = fingerprint.into();
    }

    /// Clear all removed energies after processing
    pub fn clear_removed(&mut self) {
        self.removed_energies.clear();
    }

    /// Clear the entire cache
    pub fn clear(&mut self) {
        self.edge_energies.clear();
        self.edge_residuals.clear();
        self.total_energy = 0.0;
        self.last_fingerprint.clear();
        self.dirty_edges.clear();
        self.removed_energies.clear();
    }
}

/// Engine for incremental coherence computation
pub struct IncrementalEngine<'a> {
    /// Reference to the coherence engine
    engine: &'a CoherenceEngine,
    /// Configuration
    config: IncrementalConfig,
    /// Incremental cache
    cache: IncrementalCache,
    /// Pending update events
    pending_events: Vec<UpdateEvent>,
    /// Energy history for trend analysis
    energy_history: Vec<EnergyHistoryEntry>,
    /// Statistics
    stats: IncrementalStats,
}

/// Entry in energy history
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnergyHistoryEntry {
    energy: f32,
    timestamp: DateTime<Utc>,
    was_incremental: bool,
    edges_recomputed: usize,
}

/// Statistics about incremental computation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct IncrementalStats {
    total_updates: u64,
    incremental_updates: u64,
    full_recomputes: u64,
    total_edges_recomputed: u64,
    total_time_us: u64,
}

impl<'a> IncrementalEngine<'a> {
    /// Create a new incremental engine
    pub fn new(engine: &'a CoherenceEngine, config: IncrementalConfig) -> Self {
        Self {
            engine,
            config,
            cache: IncrementalCache::new(),
            pending_events: Vec::new(),
            energy_history: Vec::new(),
            stats: IncrementalStats::default(),
        }
    }

    /// Notify that a node was updated
    pub fn node_updated(&mut self, node_id: impl Into<NodeId>) {
        let node_id = node_id.into();
        let affected_edges = self.engine.edges_incident_to(&node_id);

        // Mark affected edges as dirty
        self.cache.mark_node_dirty(&affected_edges);

        // Record event
        if self.config.track_history {
            self.pending_events.push(UpdateEvent::NodeUpdated {
                node_id,
                affected_edges,
                timestamp: Utc::now(),
            });
        }
    }

    /// Notify that an edge was added
    pub fn edge_added(&mut self, edge_id: impl Into<EdgeId>) {
        let edge_id = edge_id.into();
        self.cache.mark_dirty(edge_id.clone());

        if self.config.track_history {
            self.pending_events.push(UpdateEvent::EdgeAdded {
                edge_id,
                timestamp: Utc::now(),
            });
        }
    }

    /// Notify that an edge was removed
    pub fn edge_removed(&mut self, edge_id: impl Into<EdgeId>) {
        let edge_id = edge_id.into();
        let old_energy = self.cache.get_energy(&edge_id).unwrap_or(0.0);
        self.cache.remove_edge(&edge_id);

        if self.config.track_history {
            self.pending_events.push(UpdateEvent::EdgeRemoved {
                edge_id,
                old_energy,
                timestamp: Utc::now(),
            });
        }
    }

    /// Compute energy incrementally or fully based on dirty state
    pub fn compute(&mut self) -> DeltaResult {
        let start = std::time::Instant::now();
        let old_energy = self.cache.total_energy();
        let total_edges = self.engine.edge_count();
        let dirty_count = self.cache.dirty_count();

        // Decide whether to do incremental or full recompute
        let ratio = if total_edges > 0 {
            dirty_count as f32 / total_edges as f32
        } else {
            1.0
        };

        let (new_energy, edges_recomputed, was_full) = if !self.config.enabled
            || ratio > self.config.full_recompute_threshold
            || self.cache.last_fingerprint.is_empty()
        {
            // Full recompute
            let energy = self.compute_full_internal();
            (energy.total_energy, energy.edge_count, true)
        } else {
            // Incremental
            let result = self.compute_incremental_internal();
            (result, dirty_count, false)
        };

        let compute_time_us = start.elapsed().as_micros() as u64;
        let energy_delta = new_energy - old_energy;

        // Update stats
        self.stats.total_updates += 1;
        if was_full {
            self.stats.full_recomputes += 1;
        } else {
            self.stats.incremental_updates += 1;
        }
        self.stats.total_edges_recomputed += edges_recomputed as u64;
        self.stats.total_time_us += compute_time_us;

        // Update history
        if self.config.track_history {
            self.energy_history.push(EnergyHistoryEntry {
                energy: new_energy,
                timestamp: Utc::now(),
                was_incremental: !was_full,
                edges_recomputed,
            });

            // Trim history
            while self.energy_history.len() > self.config.history_size {
                self.energy_history.remove(0);
            }
        }

        // Clear pending events
        self.pending_events.clear();
        self.cache.clear_removed();

        DeltaResult {
            energy_delta,
            new_energy,
            old_energy,
            edges_recomputed,
            total_edges,
            was_full_recompute: was_full,
            compute_time_us,
            timestamp: Utc::now(),
        }
    }

    /// Force a full recomputation
    pub fn compute_full(&mut self) -> CoherenceEnergy {
        self.compute_full_internal()
    }

    /// Get the current cached energy
    #[inline]
    pub fn cached_energy(&self) -> f32 {
        self.cache.total_energy()
    }

    /// Get the number of pending dirty edges
    #[inline]
    pub fn dirty_count(&self) -> usize {
        self.cache.dirty_count()
    }

    /// Check if incremental mode is effective
    pub fn incremental_ratio(&self) -> f32 {
        if self.stats.total_updates > 0 {
            self.stats.incremental_updates as f32 / self.stats.total_updates as f32
        } else {
            0.0
        }
    }

    /// Get energy trend over recent history
    pub fn energy_trend(&self, window: usize) -> Option<f32> {
        if self.energy_history.len() < window {
            return None;
        }

        let recent: Vec<_> = self.energy_history.iter().rev().take(window).collect();

        // Linear regression slope
        let n = recent.len() as f32;
        let sum_x: f32 = (0..recent.len()).map(|i| i as f32).sum();
        let sum_y: f32 = recent.iter().map(|e| e.energy).sum();
        let sum_xy: f32 = recent
            .iter()
            .enumerate()
            .map(|(i, e)| i as f32 * e.energy)
            .sum();
        let sum_xx: f32 = (0..recent.len()).map(|i| (i as f32).powi(2)).sum();

        let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
        Some(slope)
    }

    // Private methods

    fn compute_full_internal(&mut self) -> CoherenceEnergy {
        let energy = self.engine.compute_energy();

        // Rebuild cache from full computation
        self.cache.clear();
        for (edge_id, edge_energy) in &energy.edge_energies {
            self.cache.update_edge(
                edge_id.clone(),
                edge_energy.energy,
                edge_energy.residual.clone(),
            );
        }
        self.cache.set_fingerprint(&energy.fingerprint);

        energy
    }

    fn compute_incremental_internal(&mut self) -> f32 {
        let dirty_edges: Vec<_> = self.cache.dirty_edges().iter().cloned().collect();

        // Recompute dirty edges (parallel when feature enabled)
        #[cfg(feature = "parallel")]
        let new_energies: Vec<(EdgeId, EdgeEnergy)> = dirty_edges
            .par_iter()
            .filter_map(|edge_id| {
                self.engine
                    .compute_edge_energy(edge_id)
                    .ok()
                    .map(|e| (edge_id.clone(), e))
            })
            .collect();

        #[cfg(not(feature = "parallel"))]
        let new_energies: Vec<(EdgeId, EdgeEnergy)> = dirty_edges
            .iter()
            .filter_map(|edge_id| {
                self.engine
                    .compute_edge_energy(edge_id)
                    .ok()
                    .map(|e| (edge_id.clone(), e))
            })
            .collect();

        // Update cache
        for (edge_id, edge_energy) in new_energies {
            self.cache
                .update_edge(edge_id, edge_energy.energy, edge_energy.residual);
        }

        // Update fingerprint
        self.cache
            .set_fingerprint(self.engine.current_fingerprint());

        self.cache.total_energy()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coherence::engine::CoherenceConfig;

    #[test]
    fn test_incremental_cache() {
        let mut cache = IncrementalCache::new();

        cache.update_edge("e1", 1.0, vec![1.0]);
        cache.update_edge("e2", 2.0, vec![1.4]);

        assert_eq!(cache.total_energy(), 3.0);
        assert_eq!(cache.get_energy("e1"), Some(1.0));

        cache.remove_edge("e1");
        assert_eq!(cache.total_energy(), 2.0);
        assert_eq!(cache.get_energy("e1"), None);
    }

    #[test]
    fn test_dirty_tracking() {
        let mut cache = IncrementalCache::new();

        cache.update_edge("e1", 1.0, vec![]);
        cache.set_fingerprint("fp1");

        assert_eq!(cache.dirty_count(), 0);

        cache.mark_dirty("e1");
        assert_eq!(cache.dirty_count(), 1);
        assert!(!cache.is_valid("fp1"));

        cache.update_edge("e1", 1.5, vec![]);
        assert_eq!(cache.dirty_count(), 0);
    }

    #[test]
    fn test_incremental_engine() {
        let engine = CoherenceEngine::new(CoherenceConfig::default());

        engine.add_node("n1", vec![1.0, 0.0]).unwrap();
        engine.add_node("n2", vec![0.0, 1.0]).unwrap();
        engine.add_edge("n1", "n2", 1.0, None).unwrap();

        let mut inc = IncrementalEngine::new(&engine, IncrementalConfig::default());

        // First compute is full
        let result = inc.compute();
        assert!(result.was_full_recompute);
        assert_eq!(result.new_energy, 2.0); // |[1,-1]|^2 = 2

        // No changes -> no dirty edges
        assert_eq!(inc.dirty_count(), 0);
    }

    #[test]
    fn test_delta_result() {
        let result = DeltaResult {
            energy_delta: 0.5,
            new_energy: 2.5,
            old_energy: 2.0,
            edges_recomputed: 1,
            total_edges: 10,
            was_full_recompute: false,
            compute_time_us: 100,
            timestamp: Utc::now(),
        };

        assert!(result.energy_increased());
        assert!(!result.energy_decreased());
        assert!((result.relative_change() - 0.25).abs() < 1e-6);
    }

    #[test]
    fn test_update_events() {
        let event = UpdateEvent::NodeUpdated {
            node_id: "n1".to_string(),
            affected_edges: vec!["e1".to_string(), "e2".to_string()],
            timestamp: Utc::now(),
        };

        assert!(event.affects_edge("e1"));
        assert!(event.affects_edge("e2"));
        assert!(!event.affects_edge("e3"));
    }

    #[test]
    fn test_energy_trend() {
        let engine = CoherenceEngine::default();
        let mut inc = IncrementalEngine::new(
            &engine,
            IncrementalConfig {
                track_history: true,
                history_size: 10,
                ..Default::default()
            },
        );

        // Manually populate history for testing
        for i in 0..5 {
            inc.energy_history.push(EnergyHistoryEntry {
                energy: i as f32 * 0.5,
                timestamp: Utc::now(),
                was_incremental: true,
                edges_recomputed: 1,
            });
        }

        let trend = inc.energy_trend(4);
        assert!(trend.is_some());
        assert!(trend.unwrap() > 0.0); // Increasing trend
    }
}
