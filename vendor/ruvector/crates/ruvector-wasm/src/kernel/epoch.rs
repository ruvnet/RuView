//! Epoch-Based Interruption
//!
//! Provides execution budget management using Wasmtime's epoch mechanism.
//! This allows coarse-grained interruption of WASM execution with minimal overhead.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Epoch controller for managing execution budgets
///
/// The epoch mechanism works by periodically incrementing a counter.
/// WASM code checks this counter at certain points (function calls, loops)
/// and traps if the deadline has been exceeded.
#[derive(Debug, Clone)]
pub struct EpochController {
    /// Current epoch value
    current_epoch: Arc<AtomicU64>,
    /// Tick interval
    tick_interval: Duration,
    /// Whether the controller is running
    running: Arc<std::sync::atomic::AtomicBool>,
}

impl EpochController {
    /// Create a new epoch controller
    ///
    /// # Arguments
    /// * `tick_interval` - How often to increment the epoch (e.g., 10ms)
    pub fn new(tick_interval: Duration) -> Self {
        EpochController {
            current_epoch: Arc::new(AtomicU64::new(0)),
            tick_interval,
            running: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        }
    }

    /// Create with default 10ms tick interval
    pub fn default_interval() -> Self {
        Self::new(Duration::from_millis(10))
    }

    /// Get current epoch value
    pub fn current(&self) -> u64 {
        self.current_epoch.load(Ordering::Relaxed)
    }

    /// Manually increment the epoch
    pub fn increment(&self) {
        self.current_epoch.fetch_add(1, Ordering::Relaxed);
    }

    /// Reset epoch to zero
    pub fn reset(&self) {
        self.current_epoch.store(0, Ordering::Relaxed);
    }

    /// Get tick interval
    pub fn tick_interval(&self) -> Duration {
        self.tick_interval
    }

    /// Check if the controller is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::Relaxed)
    }

    /// Get a clone of the epoch counter for sharing
    pub fn epoch_counter(&self) -> Arc<AtomicU64> {
        Arc::clone(&self.current_epoch)
    }

    /// Calculate deadline epoch for a given budget
    ///
    /// # Arguments
    /// * `budget_ticks` - Number of ticks before timeout
    ///
    /// # Returns
    /// The epoch value that represents the deadline
    pub fn deadline_for_budget(&self, budget_ticks: u64) -> u64 {
        self.current() + budget_ticks
    }

    /// Check if an epoch deadline has been exceeded
    pub fn is_deadline_exceeded(&self, deadline: u64) -> bool {
        self.current() >= deadline
    }

    /// Convert epoch ticks to approximate duration
    pub fn ticks_to_duration(&self, ticks: u64) -> Duration {
        self.tick_interval * ticks as u32
    }

    /// Convert duration to approximate epoch ticks
    pub fn duration_to_ticks(&self, duration: Duration) -> u64 {
        (duration.as_nanos() / self.tick_interval.as_nanos()) as u64
    }
}

impl Default for EpochController {
    fn default() -> Self {
        Self::default_interval()
    }
}

/// Configuration for epoch-based execution limits
#[derive(Debug, Clone, Copy)]
pub struct EpochConfig {
    /// Enable epoch interruption
    pub enabled: bool,

    /// Tick interval in milliseconds
    pub tick_interval_ms: u64,

    /// Default budget in ticks
    pub default_budget: u64,

    /// Maximum allowed budget (prevents abuse)
    pub max_budget: u64,
}

impl EpochConfig {
    /// Create a new epoch configuration
    pub fn new(tick_interval_ms: u64, default_budget: u64) -> Self {
        EpochConfig {
            enabled: true,
            tick_interval_ms,
            default_budget,
            max_budget: default_budget * 10, // 10x default as max
        }
    }

    /// Create configuration for server workloads (longer budgets)
    pub fn server() -> Self {
        EpochConfig {
            enabled: true,
            tick_interval_ms: 10,
            default_budget: 1000, // 10 seconds
            max_budget: 6000,     // 60 seconds max
        }
    }

    /// Create configuration for embedded/constrained workloads
    pub fn embedded() -> Self {
        EpochConfig {
            enabled: true,
            tick_interval_ms: 1,
            default_budget: 100, // 100ms
            max_budget: 1000,    // 1 second max
        }
    }

    /// Create configuration with interruption disabled (for benchmarking)
    ///
    /// # Warning
    /// Only use this for controlled benchmarking scenarios.
    pub fn disabled() -> Self {
        EpochConfig {
            enabled: false,
            tick_interval_ms: 10,
            default_budget: u64::MAX,
            max_budget: u64::MAX,
        }
    }

    /// Get tick interval as Duration
    pub fn tick_interval(&self) -> Duration {
        Duration::from_millis(self.tick_interval_ms)
    }

    /// Clamp a requested budget to the allowed maximum
    pub fn clamp_budget(&self, requested: u64) -> u64 {
        requested.min(self.max_budget)
    }

    /// Convert budget ticks to approximate duration
    pub fn budget_duration(&self, budget: u64) -> Duration {
        Duration::from_millis(budget * self.tick_interval_ms)
    }
}

impl Default for EpochConfig {
    fn default() -> Self {
        Self::server()
    }
}

/// Epoch deadline tracker for a single kernel invocation
#[derive(Debug, Clone, Copy)]
pub struct EpochDeadline {
    /// The epoch value at which execution should stop
    pub deadline: u64,
    /// The budget that was allocated
    pub budget: u64,
    /// When the execution started (epoch value)
    pub start_epoch: u64,
}

impl EpochDeadline {
    /// Create a new deadline
    pub fn new(start_epoch: u64, budget: u64) -> Self {
        EpochDeadline {
            deadline: start_epoch + budget,
            budget,
            start_epoch,
        }
    }

    /// Calculate elapsed ticks
    pub fn elapsed(&self, current: u64) -> u64 {
        current.saturating_sub(self.start_epoch)
    }

    /// Calculate remaining ticks
    pub fn remaining(&self, current: u64) -> u64 {
        self.deadline.saturating_sub(current)
    }

    /// Check if deadline is exceeded
    pub fn is_exceeded(&self, current: u64) -> bool {
        current >= self.deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_controller() {
        let controller = EpochController::default_interval();
        assert_eq!(controller.current(), 0);

        controller.increment();
        assert_eq!(controller.current(), 1);

        controller.increment();
        assert_eq!(controller.current(), 2);

        controller.reset();
        assert_eq!(controller.current(), 0);
    }

    #[test]
    fn test_deadline_calculation() {
        let controller = EpochController::default_interval();

        let deadline = controller.deadline_for_budget(100);
        assert_eq!(deadline, 100);

        assert!(!controller.is_deadline_exceeded(deadline));

        // Simulate time passing
        for _ in 0..100 {
            controller.increment();
        }

        assert!(controller.is_deadline_exceeded(deadline));
    }

    #[test]
    fn test_duration_conversion() {
        let config = EpochConfig::new(10, 1000);

        assert_eq!(config.budget_duration(100), Duration::from_secs(1));

        let controller = EpochController::new(Duration::from_millis(10));
        assert_eq!(controller.ticks_to_duration(100), Duration::from_secs(1));
        assert_eq!(controller.duration_to_ticks(Duration::from_secs(1)), 100);
    }

    #[test]
    fn test_epoch_config_clamp() {
        let config = EpochConfig::new(10, 1000);
        assert_eq!(config.max_budget, 10000);

        assert_eq!(config.clamp_budget(500), 500);
        assert_eq!(config.clamp_budget(20000), 10000);
    }

    #[test]
    fn test_epoch_deadline() {
        let deadline = EpochDeadline::new(10, 100);

        assert_eq!(deadline.deadline, 110);
        assert_eq!(deadline.elapsed(50), 40);
        assert_eq!(deadline.remaining(50), 60);
        assert!(!deadline.is_exceeded(50));
        assert!(deadline.is_exceeded(110));
        assert!(deadline.is_exceeded(200));
    }

    #[test]
    fn test_server_config() {
        let config = EpochConfig::server();
        assert!(config.enabled);
        assert_eq!(config.tick_interval_ms, 10);
        assert_eq!(config.default_budget, 1000);
    }

    #[test]
    fn test_embedded_config() {
        let config = EpochConfig::embedded();
        assert!(config.enabled);
        assert_eq!(config.tick_interval_ms, 1);
        assert_eq!(config.default_budget, 100);
    }

    #[test]
    fn test_disabled_config() {
        let config = EpochConfig::disabled();
        assert!(!config.enabled);
    }
}
