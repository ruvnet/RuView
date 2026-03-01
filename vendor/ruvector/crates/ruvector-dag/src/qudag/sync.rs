//! Pattern Synchronization

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedPattern {
    pub id: String,
    pub pattern_vector: Vec<f32>,
    pub quality_score: f64,
    pub source_node: String,
    pub round_accepted: u64,
    pub signature: Vec<u8>,
}

pub struct PatternSync {
    last_synced_round: u64,
    pending_patterns: Vec<SyncedPattern>,
}

impl PatternSync {
    pub fn new() -> Self {
        Self {
            last_synced_round: 0,
            pending_patterns: Vec::new(),
        }
    }

    pub fn last_round(&self) -> u64 {
        self.last_synced_round
    }

    pub fn add_pattern(&mut self, pattern: SyncedPattern) {
        if pattern.round_accepted > self.last_synced_round {
            self.last_synced_round = pattern.round_accepted;
        }
        self.pending_patterns.push(pattern);
    }

    pub fn drain_pending(&mut self) -> Vec<SyncedPattern> {
        std::mem::take(&mut self.pending_patterns)
    }

    pub fn pending_count(&self) -> usize {
        self.pending_patterns.len()
    }
}

impl Default for PatternSync {
    fn default() -> Self {
        Self::new()
    }
}
