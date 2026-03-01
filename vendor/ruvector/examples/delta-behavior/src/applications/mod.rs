//! # Delta-Behavior Applications
//!
//! This module contains 10 exotic applications of delta-behavior theory,
//! each demonstrating a unique property that emerges when systems are
//! constrained to preserve global coherence.
//!
//! ## The 10 Applications
//!
//! 1. **Self-Limiting Reasoning** - Machines that refuse to think past their understanding
//! 2. **Event Horizon** - Computational boundaries that cannot be crossed
//! 3. **Artificial Homeostasis** - Synthetic life with coherence as survival constraint
//! 4. **Self-Stabilizing World Model** - Models that stop learning when incoherent
//! 5. **Coherence-Bounded Creativity** - Novelty without collapse
//! 6. **Anti-Cascade Financial Systems** - Markets that cannot cascade into collapse
//! 7. **Graceful Aging** - Distributed systems that become simpler with age
//! 8. **Swarm Intelligence** - Local freedom with global coherence bounds
//! 9. **Graceful Shutdown** - Systems that seek their own safe termination
//! 10. **Pre-AGI Containment** - Intelligence growth bounded by coherence
//!
//! ## Feature Flags
//!
//! Each application can be enabled individually via feature flags:
//!
//! ```toml
//! [dependencies]
//! delta-behavior = { version = "0.1", features = ["self-limiting-reasoning", "containment"] }
//! ```
//!
//! Or enable groups of related applications:
//!
//! - `all-applications` - All 10 applications
//! - `safety-critical` - Self-limiting reasoning, graceful shutdown, containment
//! - `distributed` - Graceful aging, swarm intelligence, anti-cascade
//! - `ai-ml` - Self-limiting reasoning, world model, creativity, containment

#[cfg(feature = "self-limiting-reasoning")]
pub mod self_limiting_reasoning;
#[cfg(feature = "self-limiting-reasoning")]
pub use self_limiting_reasoning::*;

#[cfg(feature = "event-horizon")]
pub mod event_horizon;
#[cfg(feature = "event-horizon")]
pub use event_horizon::*;

#[cfg(feature = "homeostasis")]
pub mod homeostasis;
#[cfg(feature = "homeostasis")]
pub use homeostasis::*;

#[cfg(feature = "world-model")]
pub mod world_model;
#[cfg(feature = "world-model")]
pub use world_model::*;

#[cfg(feature = "coherence-creativity")]
pub mod coherence_creativity;
#[cfg(feature = "coherence-creativity")]
pub use coherence_creativity::*;

#[cfg(feature = "anti-cascade")]
pub mod anti_cascade;
#[cfg(feature = "anti-cascade")]
pub use anti_cascade::*;

#[cfg(feature = "graceful-aging")]
pub mod graceful_aging;
#[cfg(feature = "graceful-aging")]
pub use graceful_aging::*;

#[cfg(feature = "swarm-intelligence")]
pub mod swarm_intelligence;
#[cfg(feature = "swarm-intelligence")]
pub use swarm_intelligence::*;

#[cfg(feature = "graceful-shutdown")]
pub mod graceful_shutdown;
#[cfg(feature = "graceful-shutdown")]
pub use graceful_shutdown::*;

#[cfg(feature = "containment")]
pub mod containment;
#[cfg(feature = "containment")]
pub use containment::*;

// When no features are enabled, provide the core types
// that all applications depend on
#[cfg(not(any(
    feature = "self-limiting-reasoning",
    feature = "event-horizon",
    feature = "homeostasis",
    feature = "world-model",
    feature = "coherence-creativity",
    feature = "anti-cascade",
    feature = "graceful-aging",
    feature = "swarm-intelligence",
    feature = "graceful-shutdown",
    feature = "containment",
)))]
pub mod _placeholder {
    //! Placeholder module when no application features are enabled.
    //! The core types in the parent `delta_behavior` crate are always available.
}
