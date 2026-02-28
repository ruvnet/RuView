//! # Delta-Behavior: The Mathematics of Systems That Refuse to Collapse
//!
//! Delta-behavior is a pattern of constrained state transitions that preserve global coherence.
//! This library provides the core abstractions and 10 exotic applications demonstrating
//! systems that exhibit these properties.
//!
//! ## What is Delta-Behavior?
//!
//! Delta-behavior is a pattern of system behavior where:
//! - **Change is permitted, collapse is not**
//! - **Transitions only occur along allowed paths**
//! - **Global coherence is preserved under local changes**
//! - **The system biases toward closure over divergence**
//!
//! ## The Four Properties
//!
//! 1. **Local Change**: Updates happen in bounded steps
//! 2. **Global Preservation**: Local changes don't break overall structure
//! 3. **Violation Resistance**: Destabilizing transitions are damped/blocked
//! 4. **Closure Preference**: System settles into stable attractors
//!
//! ## Core Types
//!
//! - [`DeltaSystem`] - Core trait for systems exhibiting delta-behavior
//! - [`Coherence`] - Measure of system stability (0.0 - 1.0)
//! - [`CoherenceBounds`] - Thresholds for coherence enforcement
//! - [`Attractor`] - Stable states the system gravitates toward
//! - [`DeltaConfig`] - Configuration for enforcement parameters
//!
//! ## Applications
//!
//! Enable individual applications via feature flags:
//!
//! ```toml
//! [dependencies]
//! delta-behavior = { version = "0.1", features = ["containment", "swarm-intelligence"] }
//! ```
//!
//! See the [`applications`] module for all 10 exotic applications.
//!
//! ## Quick Start
//!
//! ```rust
//! use delta_behavior::{DeltaSystem, Coherence, DeltaConfig};
//!
//! // The core invariant: coherence must be preserved
//! fn check_delta_property<S: DeltaSystem>(
//!     system: &S,
//!     transition: &S::Transition,
//!     config: &DeltaConfig,
//! ) -> bool {
//!     let predicted = system.predict_coherence(transition);
//!     predicted.value() >= config.bounds.min_coherence.value()
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(not(feature = "std"))]
extern crate alloc;

// ============================================================================
// Core Modules (defined inline below)
// ============================================================================

// Re-export core types from inline modules
pub use coherence::{Coherence, CoherenceBounds, CoherenceState};
pub use transition::{Transition, TransitionConstraint, TransitionResult};
pub use attractor::{Attractor, AttractorBasin, GuidanceForce};
pub use enforcement::{DeltaEnforcer, EnforcementResult};

// ============================================================================
// WASM Module
// ============================================================================

/// WebAssembly bindings for JavaScript/TypeScript interop.
///
/// This module provides WASM-compatible wrappers for all core types and
/// the 10 application systems, enabling use from web browsers and Node.js.
///
/// The module is always compiled for documentation purposes, but the
/// `#[wasm_bindgen]` attributes are only active when compiling for wasm32.
pub mod wasm;

/// SIMD-optimized utilities for batch operations.
///
/// This module provides portable SIMD-style optimizations using manual loop
/// unrolling and cache-friendly access patterns. Operations include:
///
/// - **Batch distance calculations**: Process multiple points efficiently
/// - **Range checks**: Determine which points are within a threshold
/// - **Vector coherence**: Compute cosine similarity for high-dimensional vectors
/// - **Normalization**: Normalize vectors to unit length
///
/// These utilities follow ruvector patterns for cross-platform compatibility,
/// benefiting from compiler auto-vectorization without requiring explicit SIMD
/// intrinsics.
///
/// # Example
///
/// ```rust
/// use delta_behavior::simd_utils::{batch_squared_distances, vector_coherence};
///
/// // Efficient batch distance calculation
/// let points = [(1.0, 2.0), (3.0, 4.0), (5.0, 6.0)];
/// let distances = batch_squared_distances(&points, (0.0, 0.0));
///
/// // Coherence between state vectors
/// let current_state = vec![0.8, 0.1, 0.1];
/// let target_state = vec![0.9, 0.05, 0.05];
/// let coherence = vector_coherence(&current_state, &target_state);
/// ```
pub mod simd_utils;

// ============================================================================
// Applications Module
// ============================================================================

/// Exotic applications of delta-behavior theory.
///
/// Each application is gated behind a feature flag for minimal dependency footprint.
/// Enable `all-applications` to include everything, or pick specific ones.
pub mod applications;

// ============================================================================
// Core Trait
// ============================================================================

/// Core trait for systems exhibiting delta-behavior.
///
/// Any system implementing this trait guarantees that it will preserve
/// coherence during state transitions, following the four properties
/// of delta-behavior.
///
/// # Example
///
/// ```rust
/// use delta_behavior::{DeltaSystem, Coherence};
///
/// struct MySystem {
///     state: f64,
///     coherence: Coherence,
/// }
///
/// impl DeltaSystem for MySystem {
///     type State = f64;
///     type Transition = f64;  // Delta to apply
///     type Error = &'static str;
///
///     fn coherence(&self) -> Coherence {
///         self.coherence
///     }
///
///     fn step(&mut self, delta: &f64) -> Result<(), Self::Error> {
///         let new_state = self.state + delta;
///         // In a real system, check coherence bounds here
///         self.state = new_state;
///         Ok(())
///     }
///
///     fn predict_coherence(&self, delta: &f64) -> Coherence {
///         // Larger deltas reduce coherence
///         let impact = delta.abs() * 0.1;
///         Coherence::clamped(self.coherence.value() - impact)
///     }
///
///     fn state(&self) -> &f64 {
///         &self.state
///     }
///
///     fn in_attractor(&self) -> bool {
///         self.state.abs() < 0.1  // Near origin is stable
///     }
/// }
/// ```
pub trait DeltaSystem {
    /// The state type of the system
    type State: Clone;

    /// The transition type
    type Transition;

    /// Error type for failed transitions
    type Error;

    /// Measure current coherence of the system.
    ///
    /// Returns a value between 0.0 (incoherent/collapsed) and 1.0 (fully coherent).
    fn coherence(&self) -> Coherence;

    /// Step the system forward by applying a transition.
    ///
    /// This should only succeed if the transition preserves coherence
    /// above the minimum threshold.
    fn step(&mut self, transition: &Self::Transition) -> Result<(), Self::Error>;

    /// Predict coherence after applying a transition without actually applying it.
    ///
    /// This allows the system to evaluate transitions before committing to them.
    fn predict_coherence(&self, transition: &Self::Transition) -> Coherence;

    /// Get a reference to the current state.
    fn state(&self) -> &Self::State;

    /// Check if the system is currently in an attractor basin.
    ///
    /// Systems in attractors are considered stable and resistant to perturbation.
    fn in_attractor(&self) -> bool;
}

// ============================================================================
// Configuration
// ============================================================================

/// Configuration for delta-behavior enforcement.
///
/// This struct contains all the parameters that control how aggressively
/// the system enforces coherence bounds and resists destabilizing transitions.
#[derive(Debug, Clone)]
pub struct DeltaConfig {
    /// Coherence bounds defining minimum, throttle, and target levels.
    pub bounds: CoherenceBounds,

    /// Energy cost parameters for transitions.
    pub energy: EnergyConfig,

    /// Scheduling parameters for prioritization.
    pub scheduling: SchedulingConfig,

    /// Gating parameters for write operations.
    pub gating: GatingConfig,

    /// Strength of attractor guidance (0.0 = none, 1.0 = strong).
    pub guidance_strength: f64,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            bounds: CoherenceBounds::default(),
            energy: EnergyConfig::default(),
            scheduling: SchedulingConfig::default(),
            gating: GatingConfig::default(),
            guidance_strength: 0.5,
        }
    }
}

impl DeltaConfig {
    /// Create a strict configuration with tight coherence bounds.
    ///
    /// Use this for safety-critical applications.
    pub fn strict() -> Self {
        Self {
            bounds: CoherenceBounds {
                min_coherence: Coherence::clamped(0.5),
                throttle_threshold: Coherence::clamped(0.7),
                target_coherence: Coherence::clamped(0.9),
                max_delta_drop: 0.05,
            },
            guidance_strength: 0.8,
            ..Default::default()
        }
    }

    /// Create a relaxed configuration with wider coherence bounds.
    ///
    /// Use this for exploratory or creative applications.
    pub fn relaxed() -> Self {
        Self {
            bounds: CoherenceBounds {
                min_coherence: Coherence::clamped(0.2),
                throttle_threshold: Coherence::clamped(0.4),
                target_coherence: Coherence::clamped(0.7),
                max_delta_drop: 0.15,
            },
            guidance_strength: 0.3,
            ..Default::default()
        }
    }
}

/// Energy cost configuration for transitions.
///
/// Higher costs for destabilizing transitions creates natural resistance
/// to coherence-reducing operations.
#[derive(Debug, Clone)]
pub struct EnergyConfig {
    /// Base cost for any transition.
    pub base_cost: f64,
    /// Exponent for instability scaling (higher = steeper cost curve).
    pub instability_exponent: f64,
    /// Maximum cost cap.
    pub max_cost: f64,
    /// Energy budget regeneration per tick.
    pub budget_per_tick: f64,
}

impl Default for EnergyConfig {
    fn default() -> Self {
        Self {
            base_cost: 1.0,
            instability_exponent: 2.0,
            max_cost: 100.0,
            budget_per_tick: 10.0,
        }
    }
}

/// Scheduling configuration for prioritizing transitions.
#[derive(Debug, Clone)]
pub struct SchedulingConfig {
    /// Coherence thresholds for priority levels (5 levels).
    pub priority_thresholds: [f64; 5],
    /// Rate limits per priority level (transitions per tick).
    pub rate_limits: [usize; 5],
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            priority_thresholds: [0.0, 0.3, 0.5, 0.7, 0.9],
            rate_limits: [100, 50, 20, 10, 5],
        }
    }
}

/// Gating configuration for write/mutation operations.
#[derive(Debug, Clone)]
pub struct GatingConfig {
    /// Minimum coherence to allow writes.
    pub min_write_coherence: f64,
    /// Minimum coherence that must remain after a write.
    pub min_post_write_coherence: f64,
    /// Recovery margin (percentage above minimum before writes resume).
    pub recovery_margin: f64,
}

impl Default for GatingConfig {
    fn default() -> Self {
        Self {
            min_write_coherence: 0.3,
            min_post_write_coherence: 0.25,
            recovery_margin: 0.2,
        }
    }
}

// ============================================================================
// Coherence Module Implementation
// ============================================================================

/// Core coherence types and operations.
pub mod coherence {
    /// A coherence value representing system stability.
    ///
    /// Values range from 0.0 (completely incoherent) to 1.0 (fully coherent).
    /// The system should maintain coherence above a minimum threshold to
    /// prevent collapse.
    #[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
    pub struct Coherence(f64);

    impl Coherence {
        /// Create a new coherence value.
        ///
        /// Returns an error if the value is outside [0.0, 1.0].
        pub fn new(value: f64) -> Result<Self, &'static str> {
            if !(0.0..=1.0).contains(&value) {
                Err("Coherence must be between 0.0 and 1.0")
            } else {
                Ok(Self(value))
            }
        }

        /// Create a coherence value, clamping to valid range.
        pub fn clamped(value: f64) -> Self {
            Self(value.clamp(0.0, 1.0))
        }

        /// Maximum coherence (1.0).
        pub fn maximum() -> Self {
            Self(1.0)
        }

        /// Minimum coherence (0.0).
        pub fn minimum() -> Self {
            Self(0.0)
        }

        /// Get the underlying value.
        pub fn value(&self) -> f64 {
            self.0
        }

        /// Check if coherence is above a threshold.
        pub fn is_above(&self, threshold: f64) -> bool {
            self.0 >= threshold
        }

        /// Check if coherence is below a threshold.
        pub fn is_below(&self, threshold: f64) -> bool {
            self.0 < threshold
        }

        /// Calculate the drop from another coherence value.
        pub fn drop_from(&self, other: &Coherence) -> f64 {
            (other.0 - self.0).max(0.0)
        }
    }

    /// Bounds defining coherence thresholds for enforcement.
    #[derive(Debug, Clone)]
    pub struct CoherenceBounds {
        /// Minimum acceptable coherence (below = blocked).
        pub min_coherence: Coherence,
        /// Throttle threshold (below = rate limited).
        pub throttle_threshold: Coherence,
        /// Target coherence for recovery.
        pub target_coherence: Coherence,
        /// Maximum allowed drop in a single transition.
        pub max_delta_drop: f64,
    }

    impl Default for CoherenceBounds {
        fn default() -> Self {
            Self {
                min_coherence: Coherence(0.3),
                throttle_threshold: Coherence(0.5),
                target_coherence: Coherence(0.8),
                max_delta_drop: 0.1,
            }
        }
    }

    /// State tracking for coherence over time.
    #[derive(Debug, Clone)]
    pub struct CoherenceState {
        /// Current coherence value.
        pub current: Coherence,
        /// Historical coherence values.
        pub history: Vec<Coherence>,
        /// Trend direction (-1.0 declining, 0.0 stable, 1.0 improving).
        pub trend: f64,
    }

    impl CoherenceState {
        /// Create a new coherence state.
        pub fn new(initial: Coherence) -> Self {
            Self {
                current: initial,
                history: vec![initial],
                trend: 0.0,
            }
        }

        /// Update with a new coherence reading.
        pub fn update(&mut self, new_coherence: Coherence) {
            let old = self.current.value();
            self.current = new_coherence;
            self.history.push(new_coherence);

            // Keep history bounded
            if self.history.len() > 100 {
                self.history.remove(0);
            }

            // Calculate trend
            let delta = new_coherence.value() - old;
            self.trend = self.trend * 0.9 + delta * 0.1;
        }

        /// Check if coherence is declining.
        pub fn is_declining(&self) -> bool {
            self.trend < -0.01
        }

        /// Check if coherence is improving.
        pub fn is_improving(&self) -> bool {
            self.trend > 0.01
        }
    }
}

// ============================================================================
// Transition Module Implementation
// ============================================================================

/// Transition types and constraints.
pub mod transition {
    use super::coherence::Coherence;

    /// A generic transition that can be applied to a system.
    #[derive(Debug, Clone)]
    pub struct Transition<T> {
        /// The transition data.
        pub data: T,
        /// Priority level (higher = more important).
        pub priority: u8,
        /// Estimated coherence impact.
        pub estimated_impact: f64,
    }

    /// Constraint that limits allowed transitions.
    #[derive(Debug, Clone)]
    pub struct TransitionConstraint {
        /// Name of the constraint.
        pub name: String,
        /// Maximum allowed coherence drop.
        pub max_coherence_drop: f64,
        /// Minimum coherence required to apply.
        pub min_required_coherence: Coherence,
    }

    impl Default for TransitionConstraint {
        fn default() -> Self {
            Self {
                name: "default".to_string(),
                max_coherence_drop: 0.1,
                min_required_coherence: Coherence::clamped(0.3),
            }
        }
    }

    /// Result of attempting a transition.
    #[derive(Debug, Clone)]
    pub enum TransitionResult<T, E> {
        /// Transition was applied successfully.
        Applied {
            /// The result of the transition.
            result: T,
            /// Coherence change.
            coherence_delta: f64,
        },
        /// Transition was blocked.
        Blocked {
            /// Reason for blocking.
            reason: E,
        },
        /// Transition was throttled (delayed).
        Throttled {
            /// Delay before retry.
            delay_ms: u64,
        },
        /// Transition was modified to preserve coherence.
        Modified {
            /// The modified result.
            result: T,
            /// Description of modifications.
            modifications: String,
        },
    }
}

// ============================================================================
// Attractor Module Implementation
// ============================================================================

/// Attractor basins and guidance forces.
pub mod attractor {
    /// An attractor representing a stable state.
    #[derive(Debug, Clone)]
    pub struct Attractor<S> {
        /// The stable state.
        pub state: S,
        /// Strength of the attractor (0.0 - 1.0).
        pub strength: f64,
        /// Radius of the attractor basin.
        pub radius: f64,
    }

    /// Basin of attraction around an attractor.
    #[derive(Debug, Clone)]
    pub struct AttractorBasin<S> {
        /// The central attractor.
        pub attractor: Attractor<S>,
        /// Distance from the attractor center.
        pub distance: f64,
        /// Whether currently inside the basin.
        pub inside: bool,
    }

    /// Force guiding the system toward an attractor.
    #[derive(Debug, Clone)]
    pub struct GuidanceForce {
        /// Direction of the force (unit vector or similar).
        pub direction: Vec<f64>,
        /// Magnitude of the force.
        pub magnitude: f64,
    }

    impl GuidanceForce {
        /// Create a zero force.
        pub fn zero(dimensions: usize) -> Self {
            Self {
                direction: vec![0.0; dimensions],
                magnitude: 0.0,
            }
        }

        /// Calculate force toward a target.
        pub fn toward(from: &[f64], to: &[f64], strength: f64) -> Self {
            let direction: Vec<f64> = from
                .iter()
                .zip(to.iter())
                .map(|(a, b)| b - a)
                .collect();

            let magnitude: f64 = direction.iter().map(|x| x * x).sum::<f64>().sqrt();

            if magnitude < 0.0001 {
                return Self::zero(from.len());
            }

            let normalized: Vec<f64> = direction.iter().map(|x| x / magnitude).collect();

            Self {
                direction: normalized,
                magnitude: magnitude * strength,
            }
        }
    }
}

// ============================================================================
// Enforcement Module Implementation
// ============================================================================

/// Enforcement mechanisms for delta-behavior.
pub mod enforcement {
    use super::coherence::Coherence;
    use super::DeltaConfig;
    use core::time::Duration;

    /// Enforcer that validates and gates transitions.
    pub struct DeltaEnforcer {
        config: DeltaConfig,
        energy_budget: f64,
        in_recovery: bool,
    }

    impl DeltaEnforcer {
        /// Create a new enforcer with the given configuration.
        pub fn new(config: DeltaConfig) -> Self {
            Self {
                energy_budget: config.energy.budget_per_tick * 10.0,
                config,
                in_recovery: false,
            }
        }

        /// Check if a transition should be allowed.
        pub fn check(
            &mut self,
            current: Coherence,
            predicted: Coherence,
        ) -> EnforcementResult {
            // Check recovery mode
            if self.in_recovery {
                let recovery_target = self.config.bounds.min_coherence.value()
                    + self.config.gating.recovery_margin;
                if current.value() < recovery_target {
                    return EnforcementResult::Blocked(
                        "In recovery mode - waiting for coherence to improve".to_string()
                    );
                }
                self.in_recovery = false;
            }

            // Check minimum coherence
            if predicted.value() < self.config.bounds.min_coherence.value() {
                self.in_recovery = true;
                return EnforcementResult::Blocked(format!(
                    "Would drop coherence to {:.3} (min: {:.3})",
                    predicted.value(),
                    self.config.bounds.min_coherence.value()
                ));
            }

            // Check delta drop
            let drop = predicted.drop_from(&current);
            if drop > self.config.bounds.max_delta_drop {
                return EnforcementResult::Blocked(format!(
                    "Coherence drop {:.3} exceeds max {:.3}",
                    drop, self.config.bounds.max_delta_drop
                ));
            }

            // Check throttle threshold
            if predicted.value() < self.config.bounds.throttle_threshold.value() {
                return EnforcementResult::Throttled(Duration::from_millis(100));
            }

            // Check energy budget
            let cost = self.calculate_cost(current, predicted);
            if cost > self.energy_budget {
                return EnforcementResult::Blocked("Energy budget exhausted".to_string());
            }
            self.energy_budget -= cost;

            EnforcementResult::Allowed
        }

        /// Calculate energy cost for a transition.
        fn calculate_cost(&self, current: Coherence, predicted: Coherence) -> f64 {
            let drop = (current.value() - predicted.value()).max(0.0);
            let instability_factor = (1.0_f64 / predicted.value().max(0.1))
                .powf(self.config.energy.instability_exponent);

            (self.config.energy.base_cost + drop * 10.0 * instability_factor)
                .min(self.config.energy.max_cost)
        }

        /// Regenerate energy budget (call once per tick).
        pub fn tick(&mut self) {
            self.energy_budget = (self.energy_budget + self.config.energy.budget_per_tick)
                .min(self.config.energy.budget_per_tick * 20.0);
        }
    }

    /// Result of enforcement check.
    #[derive(Debug, Clone)]
    pub enum EnforcementResult {
        /// Transition allowed.
        Allowed,
        /// Transition blocked with reason.
        Blocked(String),
        /// Transition throttled (rate limited).
        Throttled(Duration),
    }

    impl EnforcementResult {
        /// Check if the result allows the transition.
        pub fn is_allowed(&self) -> bool {
            matches!(self, EnforcementResult::Allowed)
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Simple test system for verification.
    struct TestSystem {
        state: f64,
        coherence: Coherence,
    }

    impl TestSystem {
        fn new() -> Self {
            Self {
                state: 0.0,
                coherence: Coherence::maximum(),
            }
        }
    }

    impl DeltaSystem for TestSystem {
        type State = f64;
        type Transition = f64;
        type Error = &'static str;

        fn coherence(&self) -> Coherence {
            self.coherence
        }

        fn step(&mut self, delta: &f64) -> Result<(), Self::Error> {
            let predicted = self.predict_coherence(delta);
            if predicted.value() < 0.3 {
                return Err("Would violate coherence bound");
            }
            self.state += delta;
            self.coherence = predicted;
            Ok(())
        }

        fn predict_coherence(&self, delta: &f64) -> Coherence {
            let impact = delta.abs() * 0.1;
            Coherence::clamped(self.coherence.value() - impact)
        }

        fn state(&self) -> &f64 {
            &self.state
        }

        fn in_attractor(&self) -> bool {
            self.state.abs() < 0.1
        }
    }

    #[test]
    fn test_coherence_bounds() {
        let c = Coherence::new(0.5).unwrap();
        assert_eq!(c.value(), 0.5);
        assert!(c.is_above(0.4));
        assert!(c.is_below(0.6));
    }

    #[test]
    fn test_coherence_clamping() {
        let c = Coherence::clamped(1.5);
        assert_eq!(c.value(), 1.0);

        let c = Coherence::clamped(-0.5);
        assert_eq!(c.value(), 0.0);
    }

    #[test]
    fn test_delta_system() {
        let mut system = TestSystem::new();

        // Small steps should succeed
        assert!(system.step(&0.1).is_ok());
        assert!(system.coherence().value() > 0.9);

        // Large steps should fail
        assert!(system.step(&10.0).is_err());
    }

    #[test]
    fn test_enforcer() {
        let config = DeltaConfig::default();
        let mut enforcer = enforcement::DeltaEnforcer::new(config);

        let current = Coherence::new(0.8).unwrap();
        let good_prediction = Coherence::new(0.75).unwrap();
        let bad_prediction = Coherence::new(0.2).unwrap();

        assert!(enforcer.check(current, good_prediction).is_allowed());
        assert!(!enforcer.check(current, bad_prediction).is_allowed());
    }

    #[test]
    fn test_config_presets() {
        let strict = DeltaConfig::strict();
        let relaxed = DeltaConfig::relaxed();

        assert!(strict.bounds.min_coherence.value() > relaxed.bounds.min_coherence.value());
        assert!(strict.guidance_strength > relaxed.guidance_strength);
    }
}
