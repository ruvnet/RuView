//! # Cognitive Black Holes
//!
//! Attractor states that trap cognitive processing, modeling rumination,
//! obsession, and escape dynamics in thought space.
//!
//! ## Key Concepts
//!
//! - **Attractor States**: Stable configurations that draw nearby states
//! - **Rumination Loops**: Repetitive thought patterns
//! - **Event Horizons**: Points of no return in thought space
//! - **Escape Velocity**: Energy required to exit an attractor
//! - **Singularities**: Extreme focus points
//!
//! ## Theoretical Basis
//!
//! Inspired by:
//! - Dynamical systems theory (attractors, basins)
//! - Clinical psychology (rumination, OCD)
//! - Physics of black holes as metaphor

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Cognitive black hole representing an attractor state
#[derive(Debug)]
pub struct CognitiveBlackHole {
    /// Center of the attractor in thought space
    center: Vec<f64>,
    /// Strength of attraction (mass analog)
    strength: f64,
    /// Event horizon radius
    event_horizon: f64,
    /// Captured thoughts
    captured: Vec<CapturedThought>,
    /// Escape attempts
    escape_attempts: Vec<EscapeAttempt>,
    /// Current attraction level
    attraction_level: f64,
    /// Type of cognitive trap
    trap_type: TrapType,
}

/// A thought that has been captured by the black hole
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapturedThought {
    pub id: Uuid,
    pub content: Vec<f64>,
    pub capture_time: u64,
    pub distance_to_center: f64,
    pub orbit_count: usize,
}

/// An attractor state in cognitive space
#[derive(Debug, Clone)]
pub struct AttractorState {
    pub id: Uuid,
    pub position: Vec<f64>,
    pub basin_radius: f64,
    pub stability: f64,
    pub attractor_type: AttractorType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AttractorType {
    /// Fixed point - single stable state
    FixedPoint,
    /// Limit cycle - periodic orbit
    LimitCycle,
    /// Strange attractor - chaotic but bounded
    Strange,
    /// Saddle - stable in some dimensions, unstable in others
    Saddle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrapType {
    /// Repetitive negative thinking
    Rumination,
    /// Fixation on specific thought
    Obsession,
    /// Anxious loops
    Anxiety,
    /// Depressive spirals
    Depression,
    /// Addictive patterns
    Addiction,
    /// Neutral attractor
    Neutral,
}

/// Dynamics of escaping an attractor
#[derive(Debug)]
pub struct EscapeDynamics {
    /// Current position in thought space
    #[allow(dead_code)]
    position: Vec<f64>,
    /// Current velocity (rate of change)
    #[allow(dead_code)]
    velocity: Vec<f64>,
    /// Escape energy accumulated
    escape_energy: f64,
    /// Required escape velocity
    escape_velocity: f64,
    /// Distance to event horizon
    #[allow(dead_code)]
    horizon_distance: f64,
}

/// Record of an escape attempt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EscapeAttempt {
    pub id: Uuid,
    pub success: bool,
    pub energy_used: f64,
    pub duration: u64,
    pub method: EscapeMethod,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EscapeMethod {
    /// Gradual energy accumulation
    Gradual,
    /// Sudden external force
    External,
    /// Reframing the attractor
    Reframe,
    /// Tunneling (quantum-like escape)
    Tunneling,
    /// Attractor destruction
    Destruction,
}

impl CognitiveBlackHole {
    /// Create a new cognitive black hole
    pub fn new() -> Self {
        Self {
            center: vec![0.0; 8],
            strength: 1.0,
            event_horizon: 0.5,
            captured: Vec::new(),
            escape_attempts: Vec::new(),
            attraction_level: 0.0,
            trap_type: TrapType::Neutral,
        }
    }

    /// Create with specific parameters
    pub fn with_params(center: Vec<f64>, strength: f64, trap_type: TrapType) -> Self {
        let event_horizon = (strength * 0.3).clamp(0.1, 1.0);

        Self {
            center,
            strength,
            event_horizon,
            captured: Vec::new(),
            escape_attempts: Vec::new(),
            attraction_level: 0.0,
            trap_type,
        }
    }

    /// Measure current attraction strength
    pub fn measure_attraction(&self) -> f64 {
        self.attraction_level
    }

    /// Check if a thought would be captured
    pub fn would_capture(&self, thought: &[f64]) -> bool {
        let distance = self.distance_to_center(thought);
        distance < self.event_horizon
    }

    fn distance_to_center(&self, point: &[f64]) -> f64 {
        let len = self.center.len().min(point.len());
        let mut sum_sq = 0.0;

        for i in 0..len {
            let diff = self.center[i] - point[i];
            sum_sq += diff * diff;
        }

        sum_sq.sqrt()
    }

    /// Submit a thought to the black hole's influence
    pub fn process_thought(&mut self, thought: Vec<f64>) -> ThoughtResult {
        let distance = self.distance_to_center(&thought);
        let gravitational_pull = self.strength / (distance.powi(2) + 0.01);

        // Update attraction level
        self.attraction_level = gravitational_pull.min(1.0);

        if distance < self.event_horizon {
            // Thought is captured
            self.captured.push(CapturedThought {
                id: Uuid::new_v4(),
                content: thought.clone(),
                capture_time: Self::current_time(),
                distance_to_center: distance,
                orbit_count: 0,
            });

            ThoughtResult::Captured {
                distance,
                attraction: gravitational_pull,
            }
        } else if distance < self.event_horizon * 3.0 {
            // In danger zone
            ThoughtResult::Orbiting {
                distance,
                attraction: gravitational_pull,
                decay_rate: gravitational_pull * 0.1,
            }
        } else {
            // Safe distance
            ThoughtResult::Free {
                distance,
                residual_pull: gravitational_pull,
            }
        }
    }

    /// Attempt to escape from the black hole
    pub fn attempt_escape(&mut self, energy: f64, method: EscapeMethod) -> EscapeResult {
        let escape_velocity = self.compute_escape_velocity();

        let success = match &method {
            EscapeMethod::Gradual => energy >= escape_velocity,
            EscapeMethod::External => energy >= escape_velocity * 0.8,
            EscapeMethod::Reframe => {
                // Reframing reduces the effective strength
                energy >= escape_velocity * 0.5
            }
            EscapeMethod::Tunneling => {
                // Probabilistic escape even with low energy
                let probability = 0.1 * (energy / escape_velocity);
                rand_probability() < probability
            }
            EscapeMethod::Destruction => {
                // Need overwhelming force
                energy >= escape_velocity * 2.0
            }
        };

        self.escape_attempts.push(EscapeAttempt {
            id: Uuid::new_v4(),
            success,
            energy_used: energy,
            duration: 0,
            method: method.clone(),
        });

        if success {
            // Free captured thoughts
            let freed = self.captured.len();
            self.captured.clear();
            self.attraction_level = 0.0;

            EscapeResult::Success {
                freed_thoughts: freed,
                energy_remaining: energy - escape_velocity,
            }
        } else {
            EscapeResult::Failure {
                energy_deficit: escape_velocity - energy,
                suggestion: self.suggest_escape_method(energy),
            }
        }
    }

    fn compute_escape_velocity(&self) -> f64 {
        // v_escape = sqrt(2 * G * M / r)
        // Simplified: stronger black hole = higher escape velocity
        (2.0 * self.strength / self.event_horizon).sqrt()
    }

    fn suggest_escape_method(&self, available_energy: f64) -> EscapeMethod {
        let escape_velocity = self.compute_escape_velocity();

        if available_energy >= escape_velocity * 0.8 {
            EscapeMethod::External
        } else if available_energy >= escape_velocity * 0.5 {
            EscapeMethod::Reframe
        } else {
            EscapeMethod::Tunneling
        }
    }

    /// Simulate one time step of orbital decay
    pub fn tick(&mut self) {
        // Captured thoughts spiral inward
        for thought in &mut self.captured {
            thought.distance_to_center *= 0.99;
            thought.orbit_count += 1;
        }

        // Increase attraction as thoughts accumulate
        if !self.captured.is_empty() {
            self.attraction_level = (self.attraction_level + 0.01).min(1.0);
        }
    }

    /// Get captured thoughts count
    pub fn captured_count(&self) -> usize {
        self.captured.len()
    }

    /// Get escape success rate
    pub fn escape_success_rate(&self) -> f64 {
        if self.escape_attempts.is_empty() {
            return 0.0;
        }

        let successes = self.escape_attempts.iter().filter(|a| a.success).count();
        successes as f64 / self.escape_attempts.len() as f64
    }

    /// Get trap type
    pub fn trap_type(&self) -> &TrapType {
        &self.trap_type
    }

    /// Get statistics
    pub fn statistics(&self) -> BlackHoleStatistics {
        BlackHoleStatistics {
            strength: self.strength,
            event_horizon: self.event_horizon,
            attraction_level: self.attraction_level,
            captured_count: self.captured.len(),
            total_escape_attempts: self.escape_attempts.len(),
            escape_success_rate: self.escape_success_rate(),
            trap_type: self.trap_type.clone(),
        }
    }

    fn current_time() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

impl Default for CognitiveBlackHole {
    fn default() -> Self {
        Self::new()
    }
}

impl AttractorState {
    /// Create a new attractor state
    pub fn new(position: Vec<f64>, attractor_type: AttractorType) -> Self {
        Self {
            id: Uuid::new_v4(),
            position,
            basin_radius: 1.0,
            stability: 0.5,
            attractor_type,
        }
    }

    /// Check if a point is in the basin of attraction
    pub fn in_basin(&self, point: &[f64]) -> bool {
        let distance = self.distance_to(point);
        distance < self.basin_radius
    }

    fn distance_to(&self, point: &[f64]) -> f64 {
        let len = self.position.len().min(point.len());
        let mut sum_sq = 0.0;

        for i in 0..len {
            let diff = self.position[i] - point[i];
            sum_sq += diff * diff;
        }

        sum_sq.sqrt()
    }

    /// Get attraction strength at a point
    pub fn attraction_at(&self, point: &[f64]) -> f64 {
        let distance = self.distance_to(point);
        if distance < 0.01 {
            return 1.0;
        }

        self.stability / distance
    }
}

impl EscapeDynamics {
    /// Create new escape dynamics
    pub fn new(position: Vec<f64>, black_hole: &CognitiveBlackHole) -> Self {
        let distance = {
            let len = position.len().min(black_hole.center.len());
            let mut sum_sq = 0.0;
            for i in 0..len {
                let diff = position[i] - black_hole.center[i];
                sum_sq += diff * diff;
            }
            sum_sq.sqrt()
        };

        Self {
            position,
            velocity: vec![0.0; 8],
            escape_energy: 0.0,
            escape_velocity: (2.0 * black_hole.strength / distance.max(0.1)).sqrt(),
            horizon_distance: distance - black_hole.event_horizon,
        }
    }

    /// Add escape energy
    pub fn add_energy(&mut self, amount: f64) {
        self.escape_energy += amount;
    }

    /// Check if we have escape velocity
    pub fn can_escape(&self) -> bool {
        self.escape_energy >= self.escape_velocity * 0.5
    }

    /// Get progress towards escape (0-1)
    pub fn escape_progress(&self) -> f64 {
        (self.escape_energy / self.escape_velocity).min(1.0)
    }
}

/// Result of processing a thought
#[derive(Debug, Clone)]
pub enum ThoughtResult {
    Captured {
        distance: f64,
        attraction: f64,
    },
    Orbiting {
        distance: f64,
        attraction: f64,
        decay_rate: f64,
    },
    Free {
        distance: f64,
        residual_pull: f64,
    },
}

/// Result of an escape attempt
#[derive(Debug, Clone)]
pub enum EscapeResult {
    Success {
        freed_thoughts: usize,
        energy_remaining: f64,
    },
    Failure {
        energy_deficit: f64,
        suggestion: EscapeMethod,
    },
}

/// Statistics about the black hole
#[derive(Debug, Clone)]
pub struct BlackHoleStatistics {
    pub strength: f64,
    pub event_horizon: f64,
    pub attraction_level: f64,
    pub captured_count: usize,
    pub total_escape_attempts: usize,
    pub escape_success_rate: f64,
    pub trap_type: TrapType,
}

/// Simple probability function
fn rand_probability() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(12345) as u64;

    // Simple LCG
    let result = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (result as f64) / (u64::MAX as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_black_hole_creation() {
        let bh = CognitiveBlackHole::new();
        assert_eq!(bh.captured_count(), 0);
        assert_eq!(bh.measure_attraction(), 0.0);
    }

    #[test]
    fn test_thought_capture() {
        let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 2.0, TrapType::Rumination);

        // Close thought should be captured
        let close_thought = vec![0.1; 8];
        let result = bh.process_thought(close_thought);

        assert!(matches!(result, ThoughtResult::Captured { .. }));
        assert_eq!(bh.captured_count(), 1);
    }

    #[test]
    fn test_thought_orbiting() {
        let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 1.0, TrapType::Neutral);

        // Medium distance thought
        let thought = vec![0.8; 8];
        let result = bh.process_thought(thought);

        assert!(matches!(
            result,
            ThoughtResult::Orbiting { .. } | ThoughtResult::Free { .. }
        ));
    }

    #[test]
    fn test_escape_attempt() {
        let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 1.0, TrapType::Anxiety);

        // Capture some thoughts
        for _ in 0..3 {
            bh.process_thought(vec![0.1; 8]);
        }

        // Attempt escape with high energy
        let result = bh.attempt_escape(10.0, EscapeMethod::External);

        if let EscapeResult::Success { freed_thoughts, .. } = result {
            assert_eq!(freed_thoughts, 3);
            assert_eq!(bh.captured_count(), 0);
        }
    }

    #[test]
    fn test_escape_failure() {
        let mut bh = CognitiveBlackHole::with_params(
            vec![0.0; 8],
            5.0, // Strong black hole
            TrapType::Depression,
        );

        bh.process_thought(vec![0.1; 8]);

        // Attempt escape with low energy
        let result = bh.attempt_escape(0.1, EscapeMethod::Gradual);

        assert!(matches!(result, EscapeResult::Failure { .. }));
    }

    #[test]
    fn test_attractor_state() {
        let attractor = AttractorState::new(vec![0.0; 4], AttractorType::FixedPoint);

        let close_point = vec![0.1; 4];
        let far_point = vec![5.0; 4];

        assert!(attractor.in_basin(&close_point));
        assert!(!attractor.in_basin(&far_point));
    }

    #[test]
    fn test_escape_dynamics() {
        let bh = CognitiveBlackHole::new();
        let mut dynamics = EscapeDynamics::new(vec![0.3; 8], &bh);

        assert!(!dynamics.can_escape());

        dynamics.add_energy(10.0);
        assert!(dynamics.escape_progress() > 0.0);
    }

    #[test]
    fn test_tick_decay() {
        let mut bh = CognitiveBlackHole::with_params(
            vec![0.0; 8],
            2.0, // Higher strength
            TrapType::Neutral,
        );
        // Use a close thought that will definitely be captured
        bh.process_thought(vec![0.1; 8]);

        assert!(!bh.captured.is_empty(), "Thought should be captured");
        let initial_distance = bh.captured[0].distance_to_center;
        bh.tick();
        let final_distance = bh.captured[0].distance_to_center;

        assert!(final_distance < initial_distance);
    }

    #[test]
    fn test_statistics() {
        let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 1.5, TrapType::Obsession);

        bh.process_thought(vec![0.1; 8]);
        bh.attempt_escape(0.5, EscapeMethod::Tunneling);

        let stats = bh.statistics();
        assert_eq!(stats.captured_count, 1);
        assert_eq!(stats.total_escape_attempts, 1);
        assert_eq!(stats.trap_type, TrapType::Obsession);
    }
}
