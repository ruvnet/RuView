//! Compute budget enforcement for solver operations.
//!
//! [`BudgetEnforcer`] tracks wall-clock time, iteration count, and memory
//! allocation against a [`ComputeBudget`]. Solvers call
//! [`check_iteration`](BudgetEnforcer::check_iteration) at the top of each
//! iteration loop and
//! [`check_memory`](BudgetEnforcer::check_memory) before any allocation that
//! could exceed the memory ceiling.
//!
//! Budget violations are reported as [`SolverError::BudgetExhausted`] with a
//! human-readable reason describing which limit was hit.

use std::time::Instant;

use crate::error::SolverError;
use crate::types::ComputeBudget;

/// Default memory ceiling when none is specified (256 MiB).
const DEFAULT_MEMORY_LIMIT: usize = 256 * 1024 * 1024;

/// Enforces wall-time, iteration, and memory budgets during a solve.
///
/// Create one at the start of a solve and call the `check_*` methods at each
/// iteration or before allocating scratch space. The enforcer is intentionally
/// non-`Clone` so that each solve owns exactly one.
///
/// # Example
///
/// ```
/// use ruvector_solver::budget::BudgetEnforcer;
/// use ruvector_solver::types::ComputeBudget;
///
/// let budget = ComputeBudget::default();
/// let mut enforcer = BudgetEnforcer::new(budget);
///
/// // At the top of each solver iteration:
/// enforcer.check_iteration().unwrap();
///
/// // Before allocating scratch memory:
/// enforcer.check_memory(1024).unwrap();
/// ```
pub struct BudgetEnforcer {
    /// Monotonic clock snapshot taken when the enforcer was created.
    start_time: Instant,

    /// The budget limits to enforce.
    budget: ComputeBudget,

    /// Number of iterations consumed so far.
    iterations_used: usize,

    /// Cumulative memory allocated (tracked by the caller, not measured).
    memory_used: usize,

    /// Maximum memory allowed. Defaults to [`DEFAULT_MEMORY_LIMIT`] if
    /// the `ComputeBudget` does not carry a memory field.
    memory_limit: usize,
}

impl BudgetEnforcer {
    /// Create a new enforcer with the given budget.
    ///
    /// The wall-clock timer starts immediately.
    pub fn new(budget: ComputeBudget) -> Self {
        Self {
            start_time: Instant::now(),
            budget,
            iterations_used: 0,
            memory_used: 0,
            memory_limit: DEFAULT_MEMORY_LIMIT,
        }
    }

    /// Create an enforcer with a custom memory ceiling.
    ///
    /// Use this when the caller knows the available memory and wants to
    /// enforce a tighter or looser bound than the default 256 MiB.
    pub fn with_memory_limit(budget: ComputeBudget, memory_limit: usize) -> Self {
        Self {
            start_time: Instant::now(),
            budget,
            iterations_used: 0,
            memory_used: 0,
            memory_limit,
        }
    }

    /// Check whether the next iteration is within budget.
    ///
    /// Must be called **once per iteration**, at the top of the loop body.
    /// Increments the internal iteration counter and checks both the iteration
    /// limit and the wall-clock time limit.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError::BudgetExhausted`] if either the iteration count
    /// or wall-clock time has been exceeded.
    pub fn check_iteration(&mut self) -> Result<(), SolverError> {
        self.iterations_used += 1;

        // Iteration budget
        if self.iterations_used > self.budget.max_iterations {
            return Err(SolverError::BudgetExhausted {
                reason: format!(
                    "iteration limit reached ({} > {})",
                    self.iterations_used, self.budget.max_iterations,
                ),
                elapsed: self.start_time.elapsed(),
            });
        }

        // Wall-clock budget
        let elapsed = self.start_time.elapsed();
        if elapsed > self.budget.max_time {
            return Err(SolverError::BudgetExhausted {
                reason: format!(
                    "wall-clock time limit reached ({:.2?} > {:.2?})",
                    elapsed, self.budget.max_time,
                ),
                elapsed,
            });
        }

        Ok(())
    }

    /// Check whether an additional memory allocation is within budget.
    ///
    /// Call this **before** performing the allocation. The `additional` parameter
    /// is the number of bytes the caller intends to allocate. If the allocation
    /// would push cumulative usage over the memory ceiling, the call fails
    /// without modifying the internal counter.
    ///
    /// On success the internal counter is incremented by `additional`.
    ///
    /// # Errors
    ///
    /// Returns [`SolverError::BudgetExhausted`] if the allocation would exceed
    /// the memory limit.
    pub fn check_memory(&mut self, additional: usize) -> Result<(), SolverError> {
        let new_total = self.memory_used.saturating_add(additional);
        if new_total > self.memory_limit {
            return Err(SolverError::BudgetExhausted {
                reason: format!(
                    "memory limit reached ({} + {} = {} > {} bytes)",
                    self.memory_used, additional, new_total, self.memory_limit,
                ),
                elapsed: self.start_time.elapsed(),
            });
        }
        self.memory_used = new_total;
        Ok(())
    }

    /// Wall-clock microseconds elapsed since the enforcer was created.
    #[inline]
    pub fn elapsed_us(&self) -> u64 {
        self.start_time.elapsed().as_micros() as u64
    }

    /// Wall-clock duration elapsed since the enforcer was created.
    #[inline]
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Number of iterations consumed so far.
    #[inline]
    pub fn iterations_used(&self) -> usize {
        self.iterations_used
    }

    /// Cumulative memory tracked so far (in bytes).
    #[inline]
    pub fn memory_used(&self) -> usize {
        self.memory_used
    }

    /// The tolerance target from the budget (convenience accessor).
    #[inline]
    pub fn tolerance(&self) -> f64 {
        self.budget.tolerance
    }

    /// A reference to the underlying budget configuration.
    #[inline]
    pub fn budget(&self) -> &ComputeBudget {
        &self.budget
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ComputeBudget;
    use std::time::Duration;

    fn tiny_budget() -> ComputeBudget {
        ComputeBudget {
            max_time: Duration::from_secs(60),
            max_iterations: 5,
            tolerance: 1e-6,
        }
    }

    #[test]
    fn iterations_within_budget() {
        let mut enforcer = BudgetEnforcer::new(tiny_budget());
        for _ in 0..5 {
            enforcer.check_iteration().unwrap();
        }
        assert_eq!(enforcer.iterations_used(), 5);
    }

    #[test]
    fn iteration_limit_exceeded() {
        let mut enforcer = BudgetEnforcer::new(tiny_budget());
        for _ in 0..5 {
            enforcer.check_iteration().unwrap();
        }
        // 6th iteration should fail
        let err = enforcer.check_iteration().unwrap_err();
        match err {
            SolverError::BudgetExhausted { ref reason, .. } => {
                assert!(reason.contains("iteration"), "reason: {reason}");
            }
            other => panic!("expected BudgetExhausted, got {other:?}"),
        }
    }

    #[test]
    fn wall_clock_limit_exceeded() {
        let budget = ComputeBudget {
            max_time: Duration::from_nanos(1), // Impossibly short
            max_iterations: 1_000_000,
            tolerance: 1e-6,
        };
        let mut enforcer = BudgetEnforcer::new(budget);

        // Burn a tiny bit of time so Instant::now() moves forward
        std::thread::sleep(Duration::from_micros(10));

        let err = enforcer.check_iteration().unwrap_err();
        match err {
            SolverError::BudgetExhausted { ref reason, .. } => {
                assert!(reason.contains("wall-clock"), "reason: {reason}");
            }
            other => panic!("expected BudgetExhausted for time, got {other:?}"),
        }
    }

    #[test]
    fn memory_within_budget() {
        let mut enforcer = BudgetEnforcer::with_memory_limit(tiny_budget(), 1024);
        enforcer.check_memory(512).unwrap();
        enforcer.check_memory(512).unwrap();
        assert_eq!(enforcer.memory_used(), 1024);
    }

    #[test]
    fn memory_limit_exceeded() {
        let mut enforcer = BudgetEnforcer::with_memory_limit(tiny_budget(), 1024);
        enforcer.check_memory(800).unwrap();

        let err = enforcer.check_memory(300).unwrap_err();
        match err {
            SolverError::BudgetExhausted { ref reason, .. } => {
                assert!(reason.contains("memory"), "reason: {reason}");
            }
            other => panic!("expected BudgetExhausted for memory, got {other:?}"),
        }
        // Memory should not have been incremented on failure
        assert_eq!(enforcer.memory_used(), 800);
    }

    #[test]
    fn memory_saturating_add_no_panic() {
        // Use a limit smaller than usize::MAX so that saturation triggers an error.
        let limit = usize::MAX / 2;
        let mut enforcer = BudgetEnforcer::with_memory_limit(tiny_budget(), limit);
        enforcer.check_memory(limit - 1).unwrap();
        // Adding another large amount should saturate to usize::MAX which exceeds the limit.
        let err = enforcer.check_memory(usize::MAX).unwrap_err();
        assert!(matches!(err, SolverError::BudgetExhausted { .. }));
    }

    #[test]
    fn elapsed_us_positive() {
        let enforcer = BudgetEnforcer::new(tiny_budget());
        // Just ensure it does not panic; the value may be 0 on fast machines.
        let _ = enforcer.elapsed_us();
    }

    #[test]
    fn tolerance_accessor() {
        let enforcer = BudgetEnforcer::new(tiny_budget());
        assert!((enforcer.tolerance() - 1e-6).abs() < f64::EPSILON);
    }

    #[test]
    fn budget_accessor() {
        let budget = tiny_budget();
        let enforcer = BudgetEnforcer::new(budget.clone());
        assert_eq!(enforcer.budget().max_iterations, 5);
    }
}
