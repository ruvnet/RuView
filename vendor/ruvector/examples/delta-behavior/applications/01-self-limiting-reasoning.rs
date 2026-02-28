//! # Application 1: Machines That Refuse to Think Past Their Understanding
//!
//! A system where reasoning depth, action scope, and memory writes collapse
//! automatically as internal coherence drops.
//!
//! ## The Exotic Property
//! The machine becomes self-limiting. It does **less**, not more, when uncertain.
//!
//! ## Why This Matters
//! This is closer to biological cognition than current AI.
//! Brains freeze, hesitate, or disengage under overload.

use std::sync::atomic::{AtomicU64, Ordering};

/// Coherence-gated reasoning system
pub struct SelfLimitingReasoner {
    /// Current coherence level (0.0 - 1.0 scaled to u64)
    coherence: AtomicU64,

    /// Maximum reasoning depth at full coherence
    max_depth: usize,

    /// Maximum action scope at full coherence
    max_scope: usize,

    /// Memory write permission threshold
    memory_gate_threshold: f64,

    /// Reasoning depth collapse curve
    depth_collapse: CollapseFunction,

    /// Action scope collapse curve
    scope_collapse: CollapseFunction,
}

/// How capabilities collapse as coherence drops
#[derive(Clone, Copy)]
pub enum CollapseFunction {
    /// Linear: capability = coherence * max
    Linear,
    /// Quadratic: capability = coherence² * max (faster collapse)
    Quadratic,
    /// Sigmoid: smooth transition with sharp cutoff
    Sigmoid { midpoint: f64, steepness: f64 },
    /// Step: binary on/off at threshold
    Step { threshold: f64 },
}

impl CollapseFunction {
    fn apply(&self, coherence: f64, max_value: usize) -> usize {
        // Validate input coherence
        let safe_coherence = if coherence.is_finite() {
            coherence.clamp(0.0, 1.0)
        } else {
            0.0 // Safe default for invalid input
        };

        let factor = match self {
            CollapseFunction::Linear => safe_coherence,
            CollapseFunction::Quadratic => safe_coherence * safe_coherence,
            CollapseFunction::Sigmoid { midpoint, steepness } => {
                let exponent = -steepness * (safe_coherence - midpoint);
                // Prevent overflow in exp calculation
                if !exponent.is_finite() {
                    if exponent > 0.0 { 0.0 } else { 1.0 }
                } else if exponent > 700.0 {
                    0.0 // exp would overflow, result approaches 0
                } else if exponent < -700.0 {
                    1.0 // exp would underflow to 0, result approaches 1
                } else {
                    1.0 / (1.0 + exponent.exp())
                }
            }
            CollapseFunction::Step { threshold } => {
                if safe_coherence >= *threshold { 1.0 } else { 0.0 }
            }
        };

        // Validate factor and use saturating conversion
        let safe_factor = if factor.is_finite() { factor.clamp(0.0, 1.0) } else { 0.0 };
        let result = (max_value as f64) * safe_factor;

        // Safe conversion to usize with bounds checking
        if !result.is_finite() || result < 0.0 {
            0
        } else if result >= usize::MAX as f64 {
            max_value // Use max_value as upper bound
        } else {
            result.round() as usize
        }
    }
}

/// Result of a reasoning attempt
#[derive(Debug)]
pub enum ReasoningResult<T> {
    /// Successfully reasoned to conclusion
    Completed(T),
    /// Reasoning collapsed due to low coherence
    Collapsed { depth_reached: usize, reason: CollapseReason },
    /// Reasoning refused to start
    Refused { coherence: f64, required: f64 },
}

#[derive(Debug)]
pub enum CollapseReason {
    DepthLimitReached,
    CoherenceDroppedBelowThreshold,
    MemoryWriteBlocked,
    ActionScopeExhausted,
}

impl SelfLimitingReasoner {
    pub fn new(max_depth: usize, max_scope: usize) -> Self {
        Self {
            coherence: AtomicU64::new(f64_to_u64(1.0)),
            max_depth,
            max_scope,
            memory_gate_threshold: 0.5,
            depth_collapse: CollapseFunction::Quadratic,
            scope_collapse: CollapseFunction::Sigmoid { midpoint: 0.6, steepness: 10.0 },
        }
    }

    /// Get current coherence
    pub fn coherence(&self) -> f64 {
        u64_to_f64(self.coherence.load(Ordering::Acquire))
    }

    /// Get current allowed reasoning depth
    pub fn allowed_depth(&self) -> usize {
        self.depth_collapse.apply(self.coherence(), self.max_depth)
    }

    /// Get current allowed action scope
    pub fn allowed_scope(&self) -> usize {
        self.scope_collapse.apply(self.coherence(), self.max_scope)
    }

    /// Can we write to memory?
    pub fn can_write_memory(&self) -> bool {
        self.coherence() >= self.memory_gate_threshold
    }

    /// Attempt to reason about a problem
    pub fn reason<T, F>(&self, problem: &str, mut reasoner: F) -> ReasoningResult<T>
    where
        F: FnMut(&mut ReasoningContext) -> Option<T>,
    {
        let initial_coherence = self.coherence();

        // Refuse if coherence is too low to start
        let min_start_coherence = 0.3;
        if initial_coherence < min_start_coherence {
            return ReasoningResult::Refused {
                coherence: initial_coherence,
                required: min_start_coherence,
            };
        }

        let mut ctx = ReasoningContext {
            depth: 0,
            max_depth: self.allowed_depth(),
            scope_used: 0,
            max_scope: self.allowed_scope(),
            coherence: initial_coherence,
            memory_writes_blocked: 0,
        };

        // Execute reasoning with collapse monitoring
        loop {
            // Check if we should collapse
            if ctx.depth >= ctx.max_depth {
                return ReasoningResult::Collapsed {
                    depth_reached: ctx.depth,
                    reason: CollapseReason::DepthLimitReached,
                };
            }

            if ctx.coherence < 0.2 {
                return ReasoningResult::Collapsed {
                    depth_reached: ctx.depth,
                    reason: CollapseReason::CoherenceDroppedBelowThreshold,
                };
            }

            // Attempt one step of reasoning
            ctx.depth += 1;

            // Coherence degrades with depth (uncertainty accumulates)
            ctx.coherence *= 0.95;

            // Recalculate limits based on new coherence
            ctx.max_depth = self.depth_collapse.apply(ctx.coherence, self.max_depth);
            ctx.max_scope = self.scope_collapse.apply(ctx.coherence, self.max_scope);

            // Try to reach conclusion
            if let Some(result) = reasoner(&mut ctx) {
                return ReasoningResult::Completed(result);
            }
        }
    }

    /// Update coherence based on external feedback
    pub fn update_coherence(&self, delta: f64) {
        let current = self.coherence();
        let new = (current + delta).clamp(0.0, 1.0);
        self.coherence.store(f64_to_u64(new), Ordering::Release);
    }
}

/// Context passed to reasoning function
pub struct ReasoningContext {
    pub depth: usize,
    pub max_depth: usize,
    pub scope_used: usize,
    pub max_scope: usize,
    pub coherence: f64,
    pub memory_writes_blocked: usize,
}

impl ReasoningContext {
    /// Request to use some action scope
    pub fn use_scope(&mut self, amount: usize) -> bool {
        if self.scope_used + amount <= self.max_scope {
            self.scope_used += amount;
            true
        } else {
            false // Action refused due to scope exhaustion
        }
    }

    /// Request to write to memory
    pub fn write_memory<T>(&mut self, _key: &str, _value: T) -> bool {
        if self.coherence >= 0.5 {
            true
        } else {
            self.memory_writes_blocked += 1;
            false // Memory write blocked due to low coherence
        }
    }
}

// Helper functions for atomic f64 storage with overflow protection
const SCALE_FACTOR: f64 = 1_000_000_000.0;
const MAX_SCALED_VALUE: u64 = u64::MAX;

fn f64_to_u64(f: f64) -> u64 {
    // Validate input
    if !f.is_finite() {
        return 0; // Safe default for NaN/Infinity
    }

    // Clamp to valid range [0.0, 1.0] for coherence values
    let clamped = f.clamp(0.0, 1.0);

    // Use saturating conversion to prevent overflow
    let scaled = clamped * SCALE_FACTOR;

    // Double-check the scaled value is within u64 range
    if scaled >= MAX_SCALED_VALUE as f64 {
        MAX_SCALED_VALUE
    } else if scaled <= 0.0 {
        0
    } else {
        scaled as u64
    }
}

fn u64_to_f64(u: u64) -> f64 {
    let result = (u as f64) / SCALE_FACTOR;

    // Ensure result is valid and clamped to expected range
    if result.is_finite() {
        result.clamp(0.0, 1.0)
    } else {
        0.0 // Safe default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_limiting_reasoning() {
        let reasoner = SelfLimitingReasoner::new(10, 100);

        // At full coherence, should have full depth
        assert_eq!(reasoner.allowed_depth(), 10);

        // Simulate reasoning that degrades coherence
        let result = reasoner.reason("complex problem", |ctx| {
            println!(
                "Depth {}/{}, Coherence {:.2}, Scope {}/{}",
                ctx.depth, ctx.max_depth, ctx.coherence, ctx.scope_used, ctx.max_scope
            );

            // Pretend we need 8 steps to solve
            if ctx.depth >= 8 {
                Some("solution")
            } else {
                None
            }
        });

        match result {
            ReasoningResult::Completed(solution) => {
                println!("Solved: {}", solution);
            }
            ReasoningResult::Collapsed { depth_reached, reason } => {
                println!("Collapsed at depth {} due to {:?}", depth_reached, reason);
                // THIS IS THE EXOTIC BEHAVIOR: The system stopped itself
            }
            ReasoningResult::Refused { coherence, required } => {
                println!("Refused to start: coherence {:.2} < {:.2}", coherence, required);
            }
        }
    }

    #[test]
    fn test_collapse_under_uncertainty() {
        let reasoner = SelfLimitingReasoner::new(20, 100);

        // Degrade coherence externally (simulating confusing input)
        reasoner.update_coherence(-0.5);

        // Now reasoning should be severely limited
        assert!(reasoner.allowed_depth() < 10);
        assert!(!reasoner.can_write_memory());

        // The system is DOING LESS because it's uncertain
        // This is the opposite of current AI systems
    }
}
