//! Neural routing mechanisms for the nervous system
//!
//! This module implements three complementary routing strategies inspired by
//! computational neuroscience:
//!
//! 1. **Predictive Coding** (`predictive`) - Bandwidth reduction through residual transmission
//! 2. **Communication Through Coherence** (`coherence`) - Phase-locked oscillatory routing
//! 3. **Global Workspace** (`workspace`) - Limited-capacity broadcast with competition
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────┐
//! │              CoherenceGatedSystem                       │
//! ├─────────────────────────────────────────────────────────┤
//! │                                                         │
//! │  ┌──────────────────┐      ┌──────────────────┐        │
//! │  │  Predictive      │      │  Oscillatory     │        │
//! │  │  Layers          │─────▶│  Router          │        │
//! │  │                  │      │  (Kuramoto)      │        │
//! │  └──────────────────┘      └──────────────────┘        │
//! │         │                          │                    │
//! │         │                          ▼                    │
//! │         │                  ┌──────────────────┐        │
//! │         └─────────────────▶│  Global          │        │
//! │                            │  Workspace       │        │
//! │                            └──────────────────┘        │
//! └─────────────────────────────────────────────────────────┘
//! ```
//!
//! # Performance Characteristics
//!
//! - **Predictive coding**: 90-99% bandwidth reduction on stable signals
//! - **Oscillator step**: <1μs per module (tested up to 100 modules)
//! - **Communication gain**: <100ns per pair computation
//! - **Workspace capacity**: 4-7 items (Miller's Law)
//!
//! # Examples
//!
//! ## Basic Coherence Routing
//!
//! ```rust
//! use ruvector_nervous_system::routing::{OscillatoryRouter, Representation, GlobalWorkspace};
//!
//! // Create 40Hz gamma-band router
//! let mut router = OscillatoryRouter::new(5, 40.0);
//!
//! // Advance oscillator dynamics
//! for _ in 0..1000 {
//!     router.step(0.001); // 1ms time steps
//! }
//!
//! // Route message based on phase coherence
//! let message = vec![1.0, 2.0, 3.0];
//! let receivers = vec![1, 2, 3];
//! let routed = router.route(&message, 0, &receivers);
//! ```
//!
//! ## Predictive Bandwidth Reduction
//!
//! ```rust
//! use ruvector_nervous_system::routing::PredictiveLayer;
//!
//! let mut layer = PredictiveLayer::new(128, 0.2);
//!
//! // Only transmits when prediction error exceeds 20%
//! let signal = vec![0.5; 128];
//! if let Some(residual) = layer.residual_gated_write(&signal) {
//!     // Transmit residual (surprise)
//!     println!("Transmitting residual");
//! } else {
//!     // No transmission needed (predictable)
//!     println!("Signal predicted - no transmission");
//! }
//! ```
//!
//! ## Global Workspace Broadcast
//!
//! ```rust
//! use ruvector_nervous_system::routing::{GlobalWorkspace, Representation};
//!
//! let mut workspace = GlobalWorkspace::new(7); // 7-item capacity
//!
//! // Compete for broadcast access
//! let rep1 = Representation::new(vec![1.0], 0.8, 0u16, 0);
//! let rep2 = Representation::new(vec![2.0], 0.3, 1u16, 0);
//!
//! workspace.broadcast(rep1); // High salience - accepted
//! workspace.broadcast(rep2); // Low salience - may be rejected
//!
//! // Run competitive dynamics
//! workspace.compete();
//!
//! // Retrieve winning representations
//! let winners = workspace.retrieve_top_k(3);
//! ```

pub mod circadian;
pub mod coherence;
pub mod predictive;
pub mod workspace;

pub use circadian::{
    BudgetGuardrail, CircadianController, CircadianPhase, CircadianScheduler, HysteresisTracker,
    NervousSystemMetrics, NervousSystemScorecard, PhaseModulation, ScorecardTargets,
};
pub use coherence::OscillatoryRouter;
pub use predictive::PredictiveLayer;
pub use workspace::{GlobalWorkspace, Representation};

/// Integrated coherence-gated system combining all routing mechanisms
#[derive(Debug, Clone)]
pub struct CoherenceGatedSystem {
    /// Oscillatory router for phase-based communication
    router: OscillatoryRouter,
    /// Global workspace for broadcast
    workspace: GlobalWorkspace,
    /// Predictive layers for each module
    predictive: Vec<PredictiveLayer>,
}

impl CoherenceGatedSystem {
    /// Create a new coherence-gated system
    ///
    /// # Arguments
    /// * `num_modules` - Number of communicating modules
    /// * `vector_dim` - Dimension of vectors being transmitted
    /// * `gamma_frequency` - Base oscillation frequency (Hz, typically 30-90)
    /// * `workspace_capacity` - Global workspace capacity (typically 4-7)
    pub fn new(
        num_modules: usize,
        vector_dim: usize,
        gamma_frequency: f32,
        workspace_capacity: usize,
    ) -> Self {
        Self {
            router: OscillatoryRouter::new(num_modules, gamma_frequency),
            workspace: GlobalWorkspace::new(workspace_capacity),
            predictive: (0..num_modules)
                .map(|_| PredictiveLayer::new(vector_dim, 0.2))
                .collect(),
        }
    }

    /// Step oscillator dynamics forward in time
    pub fn step_oscillators(&mut self, dt: f32) {
        self.router.step(dt);
    }

    /// Route message with coherence gating and predictive filtering
    ///
    /// # Process
    /// 1. Compute predictive residual
    /// 2. If residual significant, apply coherence-based routing
    /// 3. Broadcast to workspace if salience high enough
    ///
    /// # Returns
    /// Vector of (receiver_id, weighted_residual) for successful routes
    pub fn route_with_coherence(
        &mut self,
        message: &[f32],
        sender: usize,
        receivers: &[usize],
        dt: f32,
    ) -> Vec<(usize, Vec<f32>)> {
        // Step 1: Advance oscillator dynamics
        self.step_oscillators(dt);

        // Step 2: Predictive filtering
        if sender >= self.predictive.len() {
            return Vec::new();
        }

        let residual = match self.predictive[sender].residual_gated_write(message) {
            Some(res) => res,
            None => return Vec::new(), // Predictable - no transmission
        };

        // Step 3: Coherence-based routing
        let routed = self.router.route(&residual, sender, receivers);

        // Step 4: Attempt global workspace broadcast for high-coherence routes
        for (receiver, weighted_msg) in &routed {
            let gain = self.router.communication_gain(sender, *receiver);

            if gain > 0.7 {
                // High coherence - try to broadcast to workspace
                let salience = gain;
                let rep = Representation::new(
                    weighted_msg.clone(),
                    salience,
                    sender as u16,
                    0, // Timestamp managed by workspace
                );
                self.workspace.broadcast(rep);
            }
        }

        routed
    }

    /// Get current oscillator phases
    pub fn phases(&self) -> &[f32] {
        self.router.phases()
    }

    /// Get workspace contents
    pub fn workspace_contents(&self) -> Vec<Representation> {
        self.workspace.retrieve()
    }

    /// Run workspace competition
    pub fn compete_workspace(&mut self) {
        self.workspace.compete();
    }

    /// Get synchronization level (order parameter)
    pub fn synchronization(&self) -> f32 {
        self.router.order_parameter()
    }

    /// Get workspace occupancy (0.0 to 1.0)
    pub fn workspace_occupancy(&self) -> f32 {
        self.workspace.len() as f32 / self.workspace.capacity() as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integrated_system() {
        let mut system = CoherenceGatedSystem::new(
            5,    // 5 modules
            128,  // 128-dim vectors
            40.0, // 40Hz gamma
            7,    // 7-item workspace
        );

        assert_eq!(system.phases().len(), 5);
        assert_eq!(system.workspace_contents().len(), 0);
    }

    #[test]
    fn test_route_with_coherence() {
        let mut system = CoherenceGatedSystem::new(3, 16, 40.0, 5);

        // Synchronize oscillators first
        for _ in 0..1000 {
            system.step_oscillators(0.001);
        }

        let message = vec![1.0; 16];
        let receivers = vec![1, 2];

        // Should transmit first time (no prediction yet)
        let routed = system.route_with_coherence(&message, 0, &receivers, 0.001);
        assert!(!routed.is_empty());
    }

    #[test]
    fn test_predictive_suppression() {
        let mut system = CoherenceGatedSystem::new(2, 16, 40.0, 5);

        let stable_message = vec![1.0; 16];
        let receivers = vec![1];

        // First transmission should go through
        let first = system.route_with_coherence(&stable_message, 0, &receivers, 0.001);
        assert!(!first.is_empty());

        // After learning, stable message should be suppressed
        for _ in 0..50 {
            system.route_with_coherence(&stable_message, 0, &receivers, 0.001);
        }

        // Should eventually suppress (prediction learned)
        let mut suppressed_count = 0;
        for _ in 0..20 {
            let result = system.route_with_coherence(&stable_message, 0, &receivers, 0.001);
            if result.is_empty() {
                suppressed_count += 1;
            }
        }

        assert!(suppressed_count > 10, "Should suppress predictable signals");
    }

    #[test]
    fn test_workspace_integration() {
        let mut system = CoherenceGatedSystem::new(3, 8, 40.0, 3);

        // Synchronize for high coherence
        for _ in 0..2000 {
            system.step_oscillators(0.001);
        }

        let message = vec![1.0; 8];
        let receivers = vec![1, 2];

        // Route with high coherence
        system.route_with_coherence(&message, 0, &receivers, 0.001);

        // Workspace should receive broadcast
        let workspace_items = system.workspace_contents();
        assert!(!workspace_items.is_empty(), "Workspace should have items");
    }

    #[test]
    fn test_synchronization_metric() {
        let mut system = CoherenceGatedSystem::new(10, 16, 40.0, 7);

        let initial_sync = system.synchronization();

        // Run dynamics with oscillators
        for _ in 0..5000 {
            system.step_oscillators(0.001);
        }

        let final_sync = system.synchronization();

        // Synchronization should be a valid metric in [0, 1] range
        assert!(
            final_sync >= 0.0 && final_sync <= 1.0,
            "Synchronization should be in valid range: {}",
            final_sync
        );
        // Verify the metric works correctly
        assert!(
            initial_sync >= 0.0 && initial_sync <= 1.0,
            "Initial sync should be valid: {}",
            initial_sync
        );
    }

    #[test]
    fn test_workspace_occupancy() {
        let mut system = CoherenceGatedSystem::new(3, 8, 40.0, 4);

        assert_eq!(system.workspace_occupancy(), 0.0);

        // Fill workspace manually
        for i in 0..3 {
            let rep = Representation::new(vec![1.0; 8], 0.8, i as u16, 0);
            system.workspace.broadcast(rep);
        }

        assert_eq!(system.workspace_occupancy(), 0.75); // 3/4
    }

    #[test]
    fn test_workspace_competition() {
        let mut system = CoherenceGatedSystem::new(2, 8, 40.0, 3);

        // Add weak representation
        let rep = Representation::new(vec![1.0; 8], 0.3, 0_u16, 0);
        system.workspace.broadcast(rep);

        system.compete_workspace();

        // Salience should decay
        let contents = system.workspace_contents();
        if !contents.is_empty() {
            assert!(contents[0].salience < 0.3, "Salience should decay");
        }
    }

    #[test]
    fn test_end_to_end_routing() {
        let mut system = CoherenceGatedSystem::new(4, 32, 40.0, 5);

        // Synchronize oscillators
        for _ in 0..1000 {
            system.step_oscillators(0.0001);
        }

        // Send varying signal
        let mut routed_count = 0;
        for i in 0..100 {
            let signal_strength = (i as f32 * 0.1).sin();
            let message: Vec<f32> = (0..32).map(|_| signal_strength).collect();
            let receivers = vec![1, 2, 3];

            let routed = system.route_with_coherence(&message, 0, &receivers, 0.0001);

            // Count successful routes
            if !routed.is_empty() {
                routed_count += 1;
            }
        }

        // Should have some successful routes (predictive coding may suppress some)
        assert!(
            routed_count > 0,
            "Should have at least some successful routes, got {}",
            routed_count
        );

        // Workspace should have accumulated some representations
        system.compete_workspace();

        // Expect valid workspace state
        assert!(system.workspace_occupancy() <= 1.0);
    }

    #[test]
    fn test_performance_integrated() {
        let mut system = CoherenceGatedSystem::new(50, 128, 40.0, 7);

        let message = vec![1.0; 128];
        let receivers: Vec<usize> = (1..50).collect();

        let start = std::time::Instant::now();

        for _ in 0..100 {
            system.route_with_coherence(&message, 0, &receivers, 0.001);
        }

        let elapsed = start.elapsed();
        let avg_route = elapsed.as_micros() / 100;

        println!("Average route time: {}μs (50 modules, 128-dim)", avg_route);

        // Should be reasonably fast (<1ms per route)
        assert!(avg_route < 1000, "Routing should be fast");
    }
}
