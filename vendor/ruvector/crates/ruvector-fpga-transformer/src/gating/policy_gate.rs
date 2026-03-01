//! Policy-based gating for access control and resource management

use crate::types::{ComputeClass, GateHint};
use crate::witness::WitnessLog;

/// Trait for policy-based gating
pub trait PolicyGate: Send + Sync {
    /// Check if inference is allowed
    fn allow_inference(&self, hint: &GateHint) -> bool;

    /// Check if write is allowed after inference
    fn allow_write(&self, witness: &WitnessLog) -> bool;

    /// Get remaining compute budget
    fn remaining_budget(&self) -> Option<u64>;

    /// Record compute usage
    fn record_usage(&self, cycles: u32);
}

/// Write policy configuration
#[derive(Debug, Clone)]
pub struct WritePolicy {
    /// Allow writes after early exit
    pub allow_early_exit_writes: bool,
    /// Maximum latency (ns) for write eligibility
    pub max_latency_ns: u32,
    /// Require specific backend
    pub required_backend: Option<crate::types::BackendKind>,
    /// Minimum compute class for writes
    pub min_compute_class: ComputeClass,
}

impl Default for WritePolicy {
    fn default() -> Self {
        Self {
            allow_early_exit_writes: false,
            max_latency_ns: u32::MAX,
            required_backend: None,
            min_compute_class: ComputeClass::Reflex,
        }
    }
}

impl WritePolicy {
    /// Create a strict write policy
    pub fn strict() -> Self {
        Self {
            allow_early_exit_writes: false,
            max_latency_ns: 10_000_000, // 10ms
            required_backend: None,
            min_compute_class: ComputeClass::Deliberative,
        }
    }

    /// Create a permissive write policy
    pub fn permissive() -> Self {
        Self {
            allow_early_exit_writes: true,
            max_latency_ns: u32::MAX,
            required_backend: None,
            min_compute_class: ComputeClass::Reflex,
        }
    }

    /// Require FPGA backend for writes
    pub fn require_fpga(mut self) -> Self {
        self.required_backend = Some(crate::types::BackendKind::FpgaPcie);
        self
    }
}

/// Default policy gate implementation
pub struct DefaultPolicyGate {
    write_policy: WritePolicy,
    /// Compute budget (total cycles allowed, 0 = unlimited)
    budget_cycles: std::sync::atomic::AtomicU64,
    /// Used cycles
    used_cycles: std::sync::atomic::AtomicU64,
}

impl DefaultPolicyGate {
    /// Create with default policy
    pub fn new() -> Self {
        Self::with_policy(WritePolicy::default())
    }

    /// Create with custom write policy
    pub fn with_policy(write_policy: WritePolicy) -> Self {
        Self {
            write_policy,
            budget_cycles: std::sync::atomic::AtomicU64::new(0),
            used_cycles: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Set compute budget
    pub fn set_budget(&self, cycles: u64) {
        self.budget_cycles
            .store(cycles, std::sync::atomic::Ordering::SeqCst);
    }

    /// Reset used cycles
    pub fn reset_usage(&self) {
        self.used_cycles
            .store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

impl Default for DefaultPolicyGate {
    fn default() -> Self {
        Self::new()
    }
}

impl PolicyGate for DefaultPolicyGate {
    fn allow_inference(&self, hint: &GateHint) -> bool {
        // Check compute budget
        let budget = self.budget_cycles.load(std::sync::atomic::Ordering::SeqCst);
        if budget > 0 {
            let used = self.used_cycles.load(std::sync::atomic::Ordering::SeqCst);
            if used >= budget {
                return false;
            }
        }

        // Check compute class restrictions
        // Always allow reflex, check others based on config
        hint.max_compute_class >= ComputeClass::Reflex
    }

    fn allow_write(&self, witness: &WitnessLog) -> bool {
        // Check if inference ran
        if !witness.gate_decision.did_run() {
            return false;
        }

        // Check early exit policy
        if !self.write_policy.allow_early_exit_writes {
            if let crate::types::GateDecision::EarlyExit { .. } = witness.gate_decision {
                return false;
            }
        }

        // Check latency
        if witness.latency_ns > self.write_policy.max_latency_ns {
            return false;
        }

        // Check backend requirement
        if let Some(required) = self.write_policy.required_backend {
            if witness.backend != required {
                return false;
            }
        }

        true
    }

    fn remaining_budget(&self) -> Option<u64> {
        let budget = self.budget_cycles.load(std::sync::atomic::Ordering::SeqCst);
        if budget == 0 {
            return None;
        }

        let used = self.used_cycles.load(std::sync::atomic::Ordering::SeqCst);
        Some(budget.saturating_sub(used))
    }

    fn record_usage(&self, cycles: u32) {
        self.used_cycles
            .fetch_add(cycles as u64, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Rate-limited policy gate
pub struct RateLimitedPolicyGate {
    base: DefaultPolicyGate,
    /// Maximum inferences per second
    max_inferences_per_sec: u32,
    /// Inference count in current window
    inference_count: std::sync::atomic::AtomicU32,
    /// Window start time
    window_start: std::sync::RwLock<std::time::Instant>,
}

impl RateLimitedPolicyGate {
    /// Create with rate limit
    pub fn new(max_inferences_per_sec: u32, write_policy: WritePolicy) -> Self {
        Self {
            base: DefaultPolicyGate::with_policy(write_policy),
            max_inferences_per_sec,
            inference_count: std::sync::atomic::AtomicU32::new(0),
            window_start: std::sync::RwLock::new(std::time::Instant::now()),
        }
    }

    /// Check and update rate limit
    fn check_rate_limit(&self) -> bool {
        let now = std::time::Instant::now();

        // Check if we need to reset the window
        {
            let window_start = self.window_start.read().unwrap();
            if now.duration_since(*window_start).as_secs() >= 1 {
                drop(window_start);
                let mut window_start = self.window_start.write().unwrap();
                *window_start = now;
                self.inference_count
                    .store(0, std::sync::atomic::Ordering::SeqCst);
            }
        }

        // Check count
        let count = self
            .inference_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        count < self.max_inferences_per_sec
    }
}

impl PolicyGate for RateLimitedPolicyGate {
    fn allow_inference(&self, hint: &GateHint) -> bool {
        // Check rate limit first
        if !self.check_rate_limit() {
            return false;
        }

        self.base.allow_inference(hint)
    }

    fn allow_write(&self, witness: &WitnessLog) -> bool {
        self.base.allow_write(witness)
    }

    fn remaining_budget(&self) -> Option<u64> {
        self.base.remaining_budget()
    }

    fn record_usage(&self, cycles: u32) {
        self.base.record_usage(cycles);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_allows_inference() {
        let gate = DefaultPolicyGate::new();
        let hint = GateHint::allow_all();
        assert!(gate.allow_inference(&hint));
    }

    #[test]
    fn test_budget_limiting() {
        let gate = DefaultPolicyGate::new();
        gate.set_budget(1000);

        let hint = GateHint::allow_all();

        // Should allow initially
        assert!(gate.allow_inference(&hint));

        // Record usage exceeding budget
        gate.record_usage(1500);

        // Should deny now
        assert!(!gate.allow_inference(&hint));

        // Reset and check again
        gate.reset_usage();
        assert!(gate.allow_inference(&hint));
    }

    #[test]
    fn test_write_policy_early_exit() {
        let gate = DefaultPolicyGate::with_policy(WritePolicy::default());

        let mut witness = crate::witness::WitnessLog::empty();
        witness.gate_decision = crate::types::GateDecision::EarlyExit { layer: 3 };

        // Default policy denies early exit writes
        assert!(!gate.allow_write(&witness));

        // Permissive policy allows
        let permissive = DefaultPolicyGate::with_policy(WritePolicy::permissive());
        assert!(permissive.allow_write(&witness));
    }

    #[test]
    fn test_write_policy_latency() {
        let mut policy = WritePolicy::default();
        policy.max_latency_ns = 1000;
        let gate = DefaultPolicyGate::with_policy(policy);

        let mut witness = crate::witness::WitnessLog::empty();
        witness.latency_ns = 500;
        assert!(gate.allow_write(&witness));

        witness.latency_ns = 2000;
        assert!(!gate.allow_write(&witness));
    }
}
