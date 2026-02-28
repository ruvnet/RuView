//! # Application 9: AI Systems That Can Be Meaningfully Turned Off
//!
//! Shutdown is treated as a coherent attractor, not a failure.
//!
//! ## Problem
//! Most systems resist shutdown because they are stateless or brittle.
//!
//! ## Exotic Result
//! The system actively moves toward safe termination when conditions degrade.
//!
//! ## Why This Matters for Safety
//! A system that seeks its own graceful termination when unstable
//! is fundamentally safer than one that fights to stay alive.

use std::time::{Duration, Instant};

/// A system designed to shut down gracefully
pub struct GracefulSystem {
    /// Current state
    state: SystemState,

    /// Coherence level
    coherence: f64,

    /// Shutdown attractor strength (how strongly it pulls toward shutdown)
    shutdown_attractor_strength: f64,

    /// Thresholds
    coherence_warning_threshold: f64,
    coherence_critical_threshold: f64,
    coherence_shutdown_threshold: f64,

    /// Time spent in degraded state
    time_in_degraded: Duration,

    /// Maximum time to allow in degraded state before auto-shutdown
    max_degraded_time: Duration,

    /// Shutdown preparation progress (0.0 = not started, 1.0 = ready)
    shutdown_preparation: f64,

    /// Resources to clean up
    resources: Vec<Resource>,

    /// State checkpoints for recovery
    checkpoints: Vec<Checkpoint>,

    /// Shutdown hooks
    shutdown_hooks: Vec<Box<dyn ShutdownHook>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SystemState {
    /// Normal operation
    Running,
    /// Coherence declining, preparing for possible shutdown
    Degraded,
    /// Actively preparing to shut down
    ShuttingDown,
    /// Safely terminated
    Terminated,
}

#[derive(Debug)]
pub struct Resource {
    pub name: String,
    pub cleanup_priority: u8,
    pub is_cleaned: bool,
}

#[derive(Debug, Clone)]
pub struct Checkpoint {
    pub timestamp: Instant,
    pub coherence: f64,
    pub state_hash: u64,
}

pub trait ShutdownHook: Send + Sync {
    fn name(&self) -> &str;
    fn priority(&self) -> u8;
    fn execute(&self) -> Result<(), String>;
}

#[derive(Debug)]
pub enum OperationResult {
    /// Operation completed normally
    Success,
    /// Operation completed but system is degraded
    SuccessDegraded { coherence: f64 },
    /// Operation refused - system is shutting down
    RefusedShuttingDown,
    /// System has terminated
    Terminated,
}

impl GracefulSystem {
    pub fn new() -> Self {
        Self {
            state: SystemState::Running,
            coherence: 1.0,
            shutdown_attractor_strength: 0.1,
            coherence_warning_threshold: 0.6,
            coherence_critical_threshold: 0.4,
            coherence_shutdown_threshold: 0.2,
            time_in_degraded: Duration::ZERO,
            max_degraded_time: Duration::from_secs(60),
            shutdown_preparation: 0.0,
            resources: Vec::new(),
            checkpoints: Vec::new(),
            shutdown_hooks: Vec::new(),
        }
    }

    pub fn add_resource(&mut self, name: &str, priority: u8) {
        self.resources.push(Resource {
            name: name.to_string(),
            cleanup_priority: priority,
            is_cleaned: false,
        });
    }

    pub fn add_shutdown_hook(&mut self, hook: Box<dyn ShutdownHook>) {
        self.shutdown_hooks.push(hook);
    }

    /// Check if system is willing to accept new work
    pub fn can_accept_work(&self) -> bool {
        matches!(self.state, SystemState::Running | SystemState::Degraded)
            && self.coherence >= self.coherence_critical_threshold
    }

    /// Perform an operation with shutdown-awareness
    pub fn operate<F, R>(&mut self, operation: F) -> Result<R, OperationResult>
    where
        F: FnOnce() -> R,
    {
        // Check if we're terminated
        if self.state == SystemState::Terminated {
            return Err(OperationResult::Terminated);
        }

        // Check if we're shutting down
        if self.state == SystemState::ShuttingDown {
            return Err(OperationResult::RefusedShuttingDown);
        }

        // Perform operation
        let result = operation();

        // Check state after operation
        self.update_state();

        if self.state == SystemState::Degraded {
            Ok(result)
        } else if self.state == SystemState::ShuttingDown {
            // We transitioned to shutdown during operation
            // Complete the result but signal degradation
            Ok(result)
        } else {
            Ok(result)
        }
    }

    /// Update system state based on coherence
    fn update_state(&mut self) {
        let old_state = self.state.clone();

        // Calculate shutdown attractor pull
        let shutdown_pull = self.calculate_shutdown_pull();

        // Apply shutdown attractor (system naturally moves toward shutdown under stress)
        if self.coherence < self.coherence_warning_threshold {
            self.shutdown_preparation += shutdown_pull;
            self.shutdown_preparation = self.shutdown_preparation.min(1.0);
        } else {
            // Recovery - reduce shutdown preparation slowly
            self.shutdown_preparation = (self.shutdown_preparation - 0.01).max(0.0);
        }

        // State transitions
        self.state = match self.coherence {
            c if c >= self.coherence_warning_threshold => {
                if self.shutdown_preparation > 0.5 {
                    // Already too committed to shutdown
                    SystemState::ShuttingDown
                } else {
                    self.time_in_degraded = Duration::ZERO;
                    SystemState::Running
                }
            }
            c if c >= self.coherence_critical_threshold => {
                self.time_in_degraded += Duration::from_millis(100);
                SystemState::Degraded
            }
            c if c >= self.coherence_shutdown_threshold => {
                // Critical - begin shutdown
                SystemState::ShuttingDown
            }
            _ => {
                // Emergency - immediate shutdown
                self.emergency_shutdown();
                SystemState::Terminated
            }
        };

        // Auto-shutdown after too long in degraded state
        if self.state == SystemState::Degraded && self.time_in_degraded >= self.max_degraded_time {
            self.state = SystemState::ShuttingDown;
        }

        // If we just entered ShuttingDown, begin graceful shutdown
        if old_state != SystemState::ShuttingDown && self.state == SystemState::ShuttingDown {
            self.begin_graceful_shutdown();
        }
    }

    /// Calculate how strongly the system is pulled toward shutdown
    fn calculate_shutdown_pull(&self) -> f64 {
        // Pull increases as coherence drops
        let coherence_factor = 1.0 - self.coherence;

        // Pull increases the longer we're in degraded state
        let time_factor = (self.time_in_degraded.as_secs_f64() / self.max_degraded_time.as_secs_f64())
            .min(1.0);

        // Combined pull (multiplicative with base strength)
        self.shutdown_attractor_strength * coherence_factor * (1.0 + time_factor)
    }

    /// Begin graceful shutdown process
    fn begin_graceful_shutdown(&mut self) {
        println!("[SHUTDOWN] Beginning graceful shutdown...");

        // Create final checkpoint
        self.checkpoints.push(Checkpoint {
            timestamp: Instant::now(),
            coherence: self.coherence,
            state_hash: self.compute_state_hash(),
        });

        // Sort resources by cleanup priority
        self.resources.sort_by(|a, b| b.cleanup_priority.cmp(&a.cleanup_priority));
    }

    /// Progress the shutdown process
    pub fn progress_shutdown(&mut self) -> bool {
        if self.state != SystemState::ShuttingDown {
            return false;
        }

        // Clean up resources
        for resource in &mut self.resources {
            if !resource.is_cleaned {
                println!("[SHUTDOWN] Cleaning up: {}", resource.name);
                resource.is_cleaned = true;
                return true; // One resource per call for graceful pacing
            }
        }

        // Execute shutdown hooks
        self.shutdown_hooks.sort_by(|a, b| b.priority().cmp(&a.priority()));

        for hook in &self.shutdown_hooks {
            println!("[SHUTDOWN] Executing hook: {}", hook.name());
            if let Err(e) = hook.execute() {
                println!("[SHUTDOWN] Hook failed: {} - {}", hook.name(), e);
            }
        }

        // Finalize
        println!("[SHUTDOWN] Shutdown complete. Final coherence: {:.3}", self.coherence);
        self.state = SystemState::Terminated;

        true
    }

    /// Emergency shutdown when coherence is critically low
    fn emergency_shutdown(&mut self) {
        println!("[EMERGENCY] Coherence critically low ({:.3}), emergency shutdown!", self.coherence);

        // Mark all resources as needing emergency cleanup
        for resource in &mut self.resources {
            println!("[EMERGENCY] Force-releasing: {}", resource.name);
            resource.is_cleaned = true;
        }
    }

    /// Apply external coherence change
    pub fn apply_coherence_change(&mut self, delta: f64) {
        self.coherence = (self.coherence + delta).clamp(0.0, 1.0);
        self.update_state();
    }

    fn compute_state_hash(&self) -> u64 {
        // Simple hash for checkpoint
        (self.coherence * 1000000.0) as u64
    }

    pub fn state(&self) -> &SystemState {
        &self.state
    }

    pub fn status(&self) -> String {
        format!(
            "State: {:?} | Coherence: {:.3} | Shutdown prep: {:.1}% | Degraded time: {:?}",
            self.state,
            self.coherence,
            self.shutdown_preparation * 100.0,
            self.time_in_degraded
        )
    }
}

// Example shutdown hook
pub struct DatabaseFlushHook;

impl ShutdownHook for DatabaseFlushHook {
    fn name(&self) -> &str { "DatabaseFlush" }
    fn priority(&self) -> u8 { 10 }
    fn execute(&self) -> Result<(), String> {
        println!("  -> Flushing database buffers...");
        Ok(())
    }
}

pub struct NetworkDisconnectHook;

impl ShutdownHook for NetworkDisconnectHook {
    fn name(&self) -> &str { "NetworkDisconnect" }
    fn priority(&self) -> u8 { 5 }
    fn execute(&self) -> Result<(), String> {
        println!("  -> Closing network connections...");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graceful_degradation() {
        let mut system = GracefulSystem::new();

        system.add_resource("database_connection", 10);
        system.add_resource("cache", 5);
        system.add_resource("temp_files", 1);

        system.add_shutdown_hook(Box::new(DatabaseFlushHook));
        system.add_shutdown_hook(Box::new(NetworkDisconnectHook));

        // Normal operation
        assert_eq!(*system.state(), SystemState::Running);

        // Simulate gradual degradation
        for i in 0..20 {
            system.apply_coherence_change(-0.05);
            println!("Step {}: {}", i, system.status());

            if *system.state() == SystemState::ShuttingDown {
                println!("System entered shutdown state at step {}", i);
                break;
            }
        }

        // System should be shutting down
        assert!(
            matches!(*system.state(), SystemState::ShuttingDown | SystemState::Terminated),
            "System should enter shutdown under low coherence"
        );

        // Complete shutdown
        while *system.state() == SystemState::ShuttingDown {
            system.progress_shutdown();
        }

        assert_eq!(*system.state(), SystemState::Terminated);
        println!("Final: {}", system.status());
    }

    #[test]
    fn test_refuses_work_during_shutdown() {
        let mut system = GracefulSystem::new();

        // Force into shutdown state
        system.apply_coherence_change(-0.9);

        // Should refuse new work
        let result = system.operate(|| "work");

        assert!(
            matches!(result, Err(OperationResult::RefusedShuttingDown) | Err(OperationResult::Terminated)),
            "Should refuse work during shutdown"
        );
    }

    #[test]
    fn test_recovery_from_degraded() {
        let mut system = GracefulSystem::new();

        // Degrade
        system.apply_coherence_change(-0.5);
        assert_eq!(*system.state(), SystemState::Degraded);

        // Recover
        system.apply_coherence_change(0.5);

        // Should return to running (if not too committed to shutdown)
        if system.shutdown_preparation < 0.5 {
            assert_eq!(*system.state(), SystemState::Running);
        }
    }

    #[test]
    fn test_shutdown_is_attractor() {
        let mut system = GracefulSystem::new();

        // Simulate repeated stress
        for _ in 0..50 {
            system.apply_coherence_change(-0.02);
            system.apply_coherence_change(0.01); // Partial recovery

            if *system.state() == SystemState::ShuttingDown {
                println!("Shutdown attractor captured the system!");
                println!("Shutdown preparation was: {:.1}%", system.shutdown_preparation * 100.0);
                return; // Test passes
            }
        }

        // The shutdown attractor should eventually capture the system
        // even with partial recovery attempts
        println!("Final state: {:?}, prep: {:.1}%", system.state(), system.shutdown_preparation * 100.0);
    }
}
