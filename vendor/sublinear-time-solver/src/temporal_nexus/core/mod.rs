//! Temporal Nexus Core - Nanosecond Scheduler for Temporal Consciousness
//!
//! This module implements the core nanosecond scheduler that enables temporal consciousness
//! by managing high-precision timing, temporal windows, and identity continuity.
//!
//! The scheduler operates at nanosecond precision using hardware Time Stamp Counter (TSC)
//! and maintains temporal windows with 50-100 tick overlap to ensure consciousness continuity.

pub mod scheduler;
pub mod temporal_window;
pub mod strange_loop;
pub mod identity;

pub use scheduler::NanosecondScheduler;
pub use temporal_window::{TemporalWindow, WindowOverlapManager};
pub use strange_loop::{StrangeLoopOperator, ContractionMetrics};
pub use identity::{IdentityContinuityTracker, ContinuityMetrics};

/// Core temporal consciousness configuration
#[derive(Debug, Clone)]
pub struct TemporalConfig {
    /// Target temporal window overlap percentage (50-100%)
    pub window_overlap_percent: f64,
    /// Maximum scheduling overhead in nanoseconds
    pub max_scheduling_overhead_ns: u64,
    /// Lipschitz constant upper bound for contraction (< 1.0)
    pub lipschitz_bound: f64,
    /// Maximum iterations for contraction convergence
    pub max_contraction_iterations: usize,
    /// TSC frequency for high-precision timing
    pub tsc_frequency_hz: u64,
}

impl Default for TemporalConfig {
    fn default() -> Self {
        Self {
            window_overlap_percent: 75.0,
            max_scheduling_overhead_ns: 1_000, // 1 microsecond
            lipschitz_bound: 0.95,
            max_contraction_iterations: 10,
            tsc_frequency_hz: 3_000_000_000, // 3 GHz default
        }
    }
}

/// Temporal consciousness task types
#[derive(Debug, Clone, PartialEq)]
pub enum ConsciousnessTask {
    /// Perception processing task
    Perception { priority: u8, data: Vec<u8> },
    /// Memory integration task
    MemoryIntegration { session_id: String, state: Vec<u8> },
    /// Identity preservation task
    IdentityPreservation { continuity_check: bool },
    /// Strange loop processing task
    StrangeLoopProcessing { iteration: usize, state: Vec<f64> },
    /// Temporal window management task
    WindowManagement { window_id: u64, overlap_target: f64 },
}

/// Error types for temporal consciousness operations
#[derive(Debug, thiserror::Error)]
pub enum TemporalError {
    #[error("Scheduling overhead exceeded limit: {actual_ns}ns > {limit_ns}ns")]
    SchedulingOverhead { actual_ns: u64, limit_ns: u64 },
    
    #[error("Window overlap below threshold: {actual}% < {required}%")]
    WindowOverlapTooLow { actual: f64, required: f64 },
    
    #[error("Contraction failed to converge in {iterations} iterations")]
    ContractionNoConvergence { iterations: usize },
    
    #[error("Identity continuity broken: gap = {gap_ns}ns")]
    IdentityContinuityBreak { gap_ns: u64 },
    
    #[error("TSC timing error: {message}")]
    TscTimingError { message: String },
    
    #[error("Task queue overflow: {current_size}/{max_size}")]
    TaskQueueOverflow { current_size: usize, max_size: usize },
}

pub type TemporalResult<T> = Result<T, TemporalError>;

/// High-precision timestamp using TSC
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TscTimestamp(pub u64);

impl TscTimestamp {
    /// Read current TSC timestamp
    #[inline]
    pub fn now() -> Self {
        Self(unsafe { core::arch::x86_64::_rdtsc() })
    }
    
    /// Calculate nanoseconds since another timestamp
    pub fn nanos_since(&self, other: TscTimestamp, tsc_freq_hz: u64) -> u64 {
        let tsc_diff = self.0.saturating_sub(other.0);
        (tsc_diff * 1_000_000_000) / tsc_freq_hz
    }
    
    /// Add nanoseconds to timestamp
    pub fn add_nanos(&self, nanos: u64, tsc_freq_hz: u64) -> Self {
        let tsc_increment = (nanos * tsc_freq_hz) / 1_000_000_000;
        Self(self.0 + tsc_increment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_temporal_config_default() {
        let config = TemporalConfig::default();
        assert!(config.window_overlap_percent >= 50.0);
        assert!(config.window_overlap_percent <= 100.0);
        assert!(config.lipschitz_bound < 1.0);
        assert!(config.max_scheduling_overhead_ns <= 1_000);
    }
    
    #[test]
    fn test_tsc_timestamp() {
        let ts1 = TscTimestamp::now();
        std::thread::sleep(std::time::Duration::from_nanos(1000));
        let ts2 = TscTimestamp::now();
        assert!(ts2 > ts1);
        
        let nanos = ts2.nanos_since(ts1, 3_000_000_000);
        assert!(nanos > 0);
    }
}