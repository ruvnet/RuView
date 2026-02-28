//! Adapter for connecting RuvLLM engine to Prime-Radiant.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

use super::error::{Result, RuvLlmIntegrationError};

/// Adapter for bridging RuvLLM engine to Prime-Radiant coherence.
///
/// This adapter wraps a RuvLLM engine and provides coherence-aware
/// inference capabilities.
#[derive(Debug)]
pub struct RuvLlmAdapter {
    /// Configuration
    config: AdapterConfig,

    /// Statistics
    stats: AdapterStats,
}

/// Configuration for the RuvLLM adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    /// Storage path for shared data
    pub storage_path: String,

    /// Embedding dimension for coherence vectors
    pub embedding_dim: usize,

    /// Enable async operations
    pub async_enabled: bool,

    /// Connection timeout in milliseconds
    pub connection_timeout_ms: u64,

    /// Maximum retry attempts
    pub max_retries: u32,

    /// Enable caching of coherence checks
    pub cache_coherence: bool,

    /// Cache TTL in seconds
    pub cache_ttl_secs: u64,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            storage_path: ".prime-radiant/ruvllm".to_string(),
            embedding_dim: 768,
            async_enabled: true,
            connection_timeout_ms: 5000,
            max_retries: 3,
            cache_coherence: true,
            cache_ttl_secs: 300,
        }
    }
}

/// Statistics for the RuvLLM adapter.
#[derive(Debug, Default)]
pub struct AdapterStats {
    /// Total requests processed
    pub requests: AtomicU64,

    /// Requests that passed coherence check
    pub passed: AtomicU64,

    /// Requests that failed coherence check
    pub failed: AtomicU64,

    /// Requests escalated to human review
    pub escalated: AtomicU64,

    /// Cache hits
    pub cache_hits: AtomicU64,

    /// Cache misses
    pub cache_misses: AtomicU64,

    /// Total processing time (microseconds)
    pub total_time_us: AtomicU64,
}

impl AdapterStats {
    /// Get the pass rate (0.0-1.0).
    pub fn pass_rate(&self) -> f64 {
        let total = self.requests.load(Ordering::Relaxed);
        if total == 0 {
            return 1.0;
        }
        let passed = self.passed.load(Ordering::Relaxed);
        passed as f64 / total as f64
    }

    /// Get the cache hit rate (0.0-1.0).
    pub fn cache_hit_rate(&self) -> f64 {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let total = hits + misses;
        if total == 0 {
            return 0.0;
        }
        hits as f64 / total as f64
    }

    /// Get average processing time in microseconds.
    pub fn avg_time_us(&self) -> f64 {
        let total = self.requests.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let time = self.total_time_us.load(Ordering::Relaxed);
        time as f64 / total as f64
    }

    /// Create a snapshot of current stats.
    pub fn snapshot(&self) -> AdapterStatsSnapshot {
        AdapterStatsSnapshot {
            requests: self.requests.load(Ordering::Relaxed),
            passed: self.passed.load(Ordering::Relaxed),
            failed: self.failed.load(Ordering::Relaxed),
            escalated: self.escalated.load(Ordering::Relaxed),
            cache_hits: self.cache_hits.load(Ordering::Relaxed),
            cache_misses: self.cache_misses.load(Ordering::Relaxed),
            total_time_us: self.total_time_us.load(Ordering::Relaxed),
        }
    }
}

/// Snapshot of adapter statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterStatsSnapshot {
    /// Total requests processed
    pub requests: u64,
    /// Requests that passed coherence check
    pub passed: u64,
    /// Requests that failed coherence check
    pub failed: u64,
    /// Requests escalated to human review
    pub escalated: u64,
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Total processing time (microseconds)
    pub total_time_us: u64,
}

impl RuvLlmAdapter {
    /// Create a new RuvLLM adapter with the given configuration.
    pub fn new(config: AdapterConfig) -> Result<Self> {
        Ok(Self {
            config,
            stats: AdapterStats::default(),
        })
    }

    /// Get the adapter configuration.
    pub fn config(&self) -> &AdapterConfig {
        &self.config
    }

    /// Get adapter statistics.
    pub fn stats(&self) -> &AdapterStats {
        &self.stats
    }

    /// Record a successful coherence check.
    pub fn record_pass(&self, time_us: u64) {
        self.stats.requests.fetch_add(1, Ordering::Relaxed);
        self.stats.passed.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_time_us
            .fetch_add(time_us, Ordering::Relaxed);
    }

    /// Record a failed coherence check.
    pub fn record_fail(&self, time_us: u64) {
        self.stats.requests.fetch_add(1, Ordering::Relaxed);
        self.stats.failed.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_time_us
            .fetch_add(time_us, Ordering::Relaxed);
    }

    /// Record an escalation.
    pub fn record_escalation(&self, time_us: u64) {
        self.stats.requests.fetch_add(1, Ordering::Relaxed);
        self.stats.escalated.fetch_add(1, Ordering::Relaxed);
        self.stats
            .total_time_us
            .fetch_add(time_us, Ordering::Relaxed);
    }

    /// Record a cache hit.
    pub fn record_cache_hit(&self) {
        self.stats.cache_hits.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a cache miss.
    pub fn record_cache_miss(&self) {
        self.stats.cache_misses.fetch_add(1, Ordering::Relaxed);
    }

    /// Validate that the adapter is properly configured.
    pub fn validate(&self) -> Result<()> {
        if self.config.embedding_dim == 0 {
            return Err(RuvLlmIntegrationError::Config(
                "Embedding dimension must be > 0".to_string(),
            ));
        }

        if self.config.connection_timeout_ms == 0 {
            return Err(RuvLlmIntegrationError::Config(
                "Connection timeout must be > 0".to_string(),
            ));
        }

        Ok(())
    }
}
