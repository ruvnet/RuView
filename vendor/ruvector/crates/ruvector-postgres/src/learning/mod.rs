//! Self-Learning and ReasoningBank Module
//!
//! This module implements adaptive query optimization using trajectory tracking,
//! pattern extraction, and learned parameter optimization.

pub mod operators;
pub mod optimizer;
pub mod patterns;
pub mod reasoning_bank;
pub mod trajectory;

pub use optimizer::{OptimizationTarget, SearchOptimizer, SearchParams};
pub use patterns::{LearnedPattern, PatternExtractor};
pub use reasoning_bank::ReasoningBank;
pub use trajectory::{QueryTrajectory, TrajectoryTracker};

use dashmap::DashMap;
use std::sync::Arc;

/// Global learning state manager
pub struct LearningManager {
    /// Trajectory trackers per table
    trackers: DashMap<String, Arc<TrajectoryTracker>>,
    /// ReasoningBank instances per table
    reasoning_banks: DashMap<String, Arc<ReasoningBank>>,
    /// Search optimizers per table
    optimizers: DashMap<String, Arc<SearchOptimizer>>,
}

impl LearningManager {
    /// Create a new learning manager
    pub fn new() -> Self {
        Self {
            trackers: DashMap::new(),
            reasoning_banks: DashMap::new(),
            optimizers: DashMap::new(),
        }
    }

    /// Enable learning for a table
    pub fn enable_for_table(&self, table_name: &str, max_trajectories: usize) {
        let tracker = Arc::new(TrajectoryTracker::new(max_trajectories));
        let bank = Arc::new(ReasoningBank::new());
        let optimizer = Arc::new(SearchOptimizer::new(bank.clone()));

        self.trackers.insert(table_name.to_string(), tracker);
        self.reasoning_banks.insert(table_name.to_string(), bank);
        self.optimizers.insert(table_name.to_string(), optimizer);
    }

    /// Get tracker for a table
    pub fn get_tracker(&self, table_name: &str) -> Option<Arc<TrajectoryTracker>> {
        self.trackers.get(table_name).map(|r| r.value().clone())
    }

    /// Get reasoning bank for a table
    pub fn get_reasoning_bank(&self, table_name: &str) -> Option<Arc<ReasoningBank>> {
        self.reasoning_banks
            .get(table_name)
            .map(|r| r.value().clone())
    }

    /// Get optimizer for a table
    pub fn get_optimizer(&self, table_name: &str) -> Option<Arc<SearchOptimizer>> {
        self.optimizers.get(table_name).map(|r| r.value().clone())
    }

    /// Extract and store patterns for a table
    pub fn extract_patterns(&self, table_name: &str, num_clusters: usize) -> Result<usize, String> {
        let tracker = self
            .get_tracker(table_name)
            .ok_or_else(|| format!("Learning not enabled for table: {}", table_name))?;
        let bank = self
            .get_reasoning_bank(table_name)
            .ok_or_else(|| format!("ReasoningBank not found for table: {}", table_name))?;

        let trajectories = tracker.get_all();
        if trajectories.is_empty() {
            return Ok(0);
        }

        let extractor = PatternExtractor::new(num_clusters);
        let patterns = extractor.extract_patterns(&trajectories);

        let count = patterns.len();
        for pattern in patterns {
            bank.store(pattern);
        }

        Ok(count)
    }
}

impl Default for LearningManager {
    fn default() -> Self {
        Self::new()
    }
}

lazy_static::lazy_static! {
    /// Global learning manager instance
    pub static ref LEARNING_MANAGER: LearningManager = LearningManager::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_learning_manager_lifecycle() {
        let manager = LearningManager::new();

        manager.enable_for_table("test_table", 1000);

        assert!(manager.get_tracker("test_table").is_some());
        assert!(manager.get_reasoning_bank("test_table").is_some());
        assert!(manager.get_optimizer("test_table").is_some());
    }
}
