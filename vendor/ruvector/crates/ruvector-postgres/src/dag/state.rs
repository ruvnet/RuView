//! DAG neural learning state management
//!
//! This module manages the global state for the neural DAG learning system,
//! including configuration, metrics, and statistics.

use once_cell::sync::Lazy;
use serde_json::Value;
use std::sync::{Arc, Mutex};

/// Global DAG state singleton
pub static DAG_STATE: Lazy<DagState> = Lazy::new(DagState::default);

/// DAG neural learning state
pub struct DagState {
    inner: Arc<Mutex<DagStateInner>>,
}

struct DagStateInner {
    enabled: bool,
    learning_rate: f64,
    attention_mechanism: String,
    pattern_count: usize,
    trajectory_count: usize,
    cache_hit_count: u64,
    cache_miss_count: u64,
    total_improvements: f64,
    improvement_count: u64,

    // SONA configuration
    micro_lora_rank: i32,
    base_lora_rank: i32,
    ewc_lambda: f64,
    pattern_clusters: i32,

    // Attention parameters
    attention_params: std::collections::HashMap<String, Value>,
}

impl Default for DagState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(DagStateInner {
                enabled: true,
                learning_rate: 0.01,
                attention_mechanism: "auto".to_string(),
                pattern_count: 0,
                trajectory_count: 0,
                cache_hit_count: 0,
                cache_miss_count: 0,
                total_improvements: 0.0,
                improvement_count: 0,
                micro_lora_rank: 2,
                base_lora_rank: 8,
                ewc_lambda: 5000.0,
                pattern_clusters: 100,
                attention_params: std::collections::HashMap::new(),
            })),
        }
    }
}

impl DagState {
    /// Check if neural DAG learning is enabled
    pub fn is_enabled(&self) -> bool {
        self.inner.lock().unwrap().enabled
    }

    /// Enable or disable neural DAG learning
    pub fn set_enabled(&self, enabled: bool) {
        self.inner.lock().unwrap().enabled = enabled;
    }

    /// Get the learning rate
    pub fn get_learning_rate(&self) -> f64 {
        self.inner.lock().unwrap().learning_rate
    }

    /// Set the learning rate
    pub fn set_learning_rate(&self, rate: f64) {
        self.inner.lock().unwrap().learning_rate = rate;
    }

    /// Get the current attention mechanism
    pub fn get_attention_mechanism(&self) -> String {
        self.inner.lock().unwrap().attention_mechanism.clone()
    }

    /// Set the attention mechanism
    pub fn set_attention_mechanism(&self, mechanism: String) {
        self.inner.lock().unwrap().attention_mechanism = mechanism;
    }

    /// Configure SONA parameters
    pub fn configure_sona(
        &self,
        micro_lora_rank: i32,
        base_lora_rank: i32,
        ewc_lambda: f64,
        pattern_clusters: i32,
    ) {
        let mut inner = self.inner.lock().unwrap();
        inner.micro_lora_rank = micro_lora_rank;
        inner.base_lora_rank = base_lora_rank;
        inner.ewc_lambda = ewc_lambda;
        inner.pattern_clusters = pattern_clusters;
    }

    /// Get pattern count
    pub fn get_pattern_count(&self) -> usize {
        self.inner.lock().unwrap().pattern_count
    }

    /// Get trajectory count
    pub fn get_trajectory_count(&self) -> usize {
        self.inner.lock().unwrap().trajectory_count
    }

    /// Get cache hit rate
    pub fn get_cache_hit_rate(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        let total = inner.cache_hit_count + inner.cache_miss_count;
        if total == 0 {
            0.0
        } else {
            inner.cache_hit_count as f64 / total as f64
        }
    }

    /// Get average improvement
    pub fn get_avg_improvement(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        if inner.improvement_count == 0 {
            0.0
        } else {
            inner.total_improvements / inner.improvement_count as f64
        }
    }

    /// Set attention parameters for a mechanism
    pub fn set_attention_params(&self, mechanism: &str, params: Value) {
        self.inner
            .lock()
            .unwrap()
            .attention_params
            .insert(mechanism.to_string(), params);
    }

    /// Get configuration as a struct (for composite type)
    pub fn get_config(&self) -> DagConfig {
        let inner = self.inner.lock().unwrap();
        DagConfig {
            enabled: inner.enabled,
            learning_rate: inner.learning_rate,
            attention_mechanism: inner.attention_mechanism.clone(),
            micro_lora_rank: inner.micro_lora_rank,
            base_lora_rank: inner.base_lora_rank,
            ewc_lambda: inner.ewc_lambda,
            pattern_clusters: inner.pattern_clusters,
        }
    }

    /// Record a cache hit
    pub fn record_cache_hit(&self) {
        self.inner.lock().unwrap().cache_hit_count += 1;
    }

    /// Record a cache miss
    pub fn record_cache_miss(&self) {
        self.inner.lock().unwrap().cache_miss_count += 1;
    }

    /// Record an improvement
    pub fn record_improvement(&self, improvement: f64) {
        let mut inner = self.inner.lock().unwrap();
        inner.total_improvements += improvement;
        inner.improvement_count += 1;
    }

    /// Increment pattern count
    pub fn increment_pattern_count(&self) {
        self.inner.lock().unwrap().pattern_count += 1;
    }

    /// Increment trajectory count
    pub fn increment_trajectory_count(&self) {
        self.inner.lock().unwrap().trajectory_count += 1;
    }
}

/// Configuration snapshot
#[derive(Debug, Clone)]
pub struct DagConfig {
    pub enabled: bool,
    pub learning_rate: f64,
    pub attention_mechanism: String,
    pub micro_lora_rank: i32,
    pub base_lora_rank: i32,
    pub ewc_lambda: f64,
    pub pattern_clusters: i32,
}
