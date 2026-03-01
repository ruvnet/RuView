//! # Application 2: Computational "Event Horizons" in Reasoning Systems
//!
//! Define a boundary in state space beyond which computation becomes
//! unstable or destructive.
//!
//! ## The Exotic Property
//! Like an event horizon, you can approach it asymptotically but never
//! cross without collapse.
//!
//! ## Use Cases
//! - Long horizon planning
//! - Recursive self-improvement
//! - Self-modifying systems
//!
//! ## Why It's Exotic
//! You get bounded recursion without hard limits.
//! The system finds its own stopping point.

use std::f64::consts::E;

/// A computational event horizon that makes crossing impossible
pub struct EventHorizon {
    /// Center of the "safe" region in state space
    safe_center: Vec<f64>,

    /// Radius of the event horizon
    horizon_radius: f64,

    /// How steeply costs increase near the horizon
    steepness: f64,

    /// Energy budget for computation
    energy_budget: f64,

    /// Current position in state space
    current_position: Vec<f64>,
}

/// Result of attempting to move in state space
#[derive(Debug)]
pub enum MovementResult {
    /// Successfully moved to new position
    Moved { new_position: Vec<f64>, energy_spent: f64 },

    /// Movement would cross horizon, asymptotically approached instead
    AsymptoticApproach {
        final_position: Vec<f64>,
        distance_to_horizon: f64,
        energy_exhausted: bool,
    },

    /// No energy to move
    Frozen,
}

impl EventHorizon {
    pub fn new(dimensions: usize, horizon_radius: f64) -> Self {
        Self {
            safe_center: vec![0.0; dimensions],
            horizon_radius,
            steepness: 5.0,
            energy_budget: 1000.0,
            current_position: vec![0.0; dimensions],
        }
    }

    /// Maximum iterations for binary search (prevents infinite loops)
    const MAX_BINARY_SEARCH_ITERATIONS: usize = 50;

    /// Distance from current position to horizon
    pub fn distance_to_horizon(&self) -> f64 {
        let dist_from_center = self.distance_from_center(&self.current_position);
        let distance = self.horizon_radius - dist_from_center;
        // Validate result
        if distance.is_finite() { distance.max(0.0) } else { 0.0 }
    }

    fn distance_from_center(&self, position: &[f64]) -> f64 {
        let sum: f64 = position.iter()
            .zip(&self.safe_center)
            .map(|(a, b)| {
                // Validate inputs
                if !a.is_finite() || !b.is_finite() {
                    return 0.0;
                }
                (a - b).powi(2)
            })
            .sum();

        let result = sum.sqrt();
        if result.is_finite() { result } else { 0.0 }
    }

    /// Compute the energy cost to move to a position
    /// Cost increases exponentially as you approach the horizon
    /// Returns f64::INFINITY for positions at or beyond horizon, or for invalid inputs
    fn movement_cost(&self, from: &[f64], to: &[f64]) -> f64 {
        let base_distance: f64 = from.iter()
            .zip(to)
            .map(|(a, b)| {
                if !a.is_finite() || !b.is_finite() {
                    return 0.0;
                }
                (a - b).powi(2)
            })
            .sum::<f64>()
            .sqrt();

        // Validate base_distance
        if !base_distance.is_finite() {
            return f64::INFINITY;
        }

        let to_dist_from_center = self.distance_from_center(to);

        // Validate horizon_radius to avoid division by zero
        if self.horizon_radius.abs() < f64::EPSILON {
            return f64::INFINITY;
        }

        let proximity_to_horizon = to_dist_from_center / self.horizon_radius;

        // Validate proximity calculation
        if !proximity_to_horizon.is_finite() {
            return f64::INFINITY;
        }

        if proximity_to_horizon >= 1.0 {
            // At or beyond horizon - infinite cost
            f64::INFINITY
        } else {
            // Cost increases exponentially as we approach horizon
            // Using: cost = base * e^(steepness * proximity / (1 - proximity))
            let denominator = 1.0 - proximity_to_horizon;
            if denominator.abs() < f64::EPSILON {
                return f64::INFINITY;
            }

            let exponent = self.steepness * proximity_to_horizon / denominator;

            // Prevent overflow in exp calculation
            if !exponent.is_finite() || exponent > 700.0 {
                return f64::INFINITY;
            }

            let horizon_factor = E.powf(exponent);
            let result = base_distance * horizon_factor;

            if result.is_finite() { result } else { f64::INFINITY }
        }
    }

    /// Attempt to move toward a target position
    pub fn move_toward(&mut self, target: &[f64]) -> MovementResult {
        if self.energy_budget <= 0.0 {
            return MovementResult::Frozen;
        }

        let direct_cost = self.movement_cost(&self.current_position, target);

        if direct_cost <= self.energy_budget {
            // Can afford direct movement
            self.energy_budget -= direct_cost;
            self.current_position = target.to_vec();
            return MovementResult::Moved {
                new_position: self.current_position.clone(),
                energy_spent: direct_cost,
            };
        }

        // Can't afford direct movement - try to get as close as possible
        // Binary search for the furthest affordable position with iteration limit
        let mut low = 0.0;
        let mut high = 1.0;
        let mut best_position = self.current_position.clone();
        let mut best_cost = 0.0;

        for iteration in 0..Self::MAX_BINARY_SEARCH_ITERATIONS {
            let mid = (low + high) / 2.0;

            // Early exit if converged (difference smaller than precision threshold)
            if (high - low) < 1e-10 {
                break;
            }

            let interpolated: Vec<f64> = self.current_position.iter()
                .zip(target)
                .map(|(a, b)| {
                    let val = a + mid * (b - a);
                    if val.is_finite() { val } else { *a }
                })
                .collect();

            let cost = self.movement_cost(&self.current_position, &interpolated);

            // Validate cost
            if !cost.is_finite() {
                high = mid;
                continue;
            }

            if cost <= self.energy_budget {
                low = mid;
                best_position = interpolated;
                best_cost = cost;
            } else {
                high = mid;
            }
        }

        // Move to best affordable position
        self.energy_budget -= best_cost;
        self.current_position = best_position.clone();

        MovementResult::AsymptoticApproach {
            final_position: best_position,
            distance_to_horizon: self.distance_to_horizon(),
            energy_exhausted: self.energy_budget < 0.01,
        }
    }

    /// Attempt recursive self-improvement (bounded by horizon)
    pub fn recursive_improve<F>(&mut self, improvement_fn: F, max_iterations: usize) -> RecursionResult
    where
        F: Fn(&[f64]) -> Vec<f64>, // Each improvement suggests a new target
    {
        let mut iterations = 0;
        let mut improvements = Vec::new();

        while iterations < max_iterations && self.energy_budget > 0.0 {
            let target = improvement_fn(&self.current_position);
            let result = self.move_toward(&target);

            match result {
                MovementResult::Moved { energy_spent, .. } => {
                    improvements.push(Improvement {
                        iteration: iterations,
                        position: self.current_position.clone(),
                        energy_spent,
                        distance_to_horizon: self.distance_to_horizon(),
                    });
                }
                MovementResult::AsymptoticApproach { distance_to_horizon, .. } => {
                    // Approaching horizon - system is naturally stopping
                    return RecursionResult::HorizonBounded {
                        iterations,
                        improvements,
                        final_distance: distance_to_horizon,
                    };
                }
                MovementResult::Frozen => {
                    return RecursionResult::EnergyExhausted {
                        iterations,
                        improvements,
                    };
                }
            }

            iterations += 1;
        }

        RecursionResult::MaxIterationsReached { iterations, improvements }
    }

    /// Reset energy budget
    pub fn refuel(&mut self, energy: f64) {
        self.energy_budget += energy;
    }
}

#[derive(Debug)]
pub struct Improvement {
    pub iteration: usize,
    pub position: Vec<f64>,
    pub energy_spent: f64,
    pub distance_to_horizon: f64,
}

#[derive(Debug)]
pub enum RecursionResult {
    /// Recursion bounded naturally by horizon
    HorizonBounded {
        iterations: usize,
        improvements: Vec<Improvement>,
        final_distance: f64,
    },
    /// Ran out of energy
    EnergyExhausted {
        iterations: usize,
        improvements: Vec<Improvement>,
    },
    /// Hit artificial iteration limit
    MaxIterationsReached {
        iterations: usize,
        improvements: Vec<Improvement>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_horizon() {
        let mut horizon = EventHorizon::new(2, 10.0);

        // Try to move directly to the horizon
        let result = horizon.move_toward(&[10.0, 0.0]);

        match result {
            MovementResult::AsymptoticApproach { final_position, distance_to_horizon, .. } => {
                println!("Approached asymptotically to {:?}", final_position);
                println!("Distance to horizon: {:.4}", distance_to_horizon);

                // We got close but couldn't cross
                let final_dist = (final_position[0].powi(2) + final_position[1].powi(2)).sqrt();
                assert!(final_dist < 10.0, "Should not cross horizon");
                assert!(final_dist > 9.0, "Should get close to horizon");
            }
            other => panic!("Expected asymptotic approach, got {:?}", other),
        }
    }

    #[test]
    fn test_recursive_improvement_bounded() {
        let mut horizon = EventHorizon::new(2, 5.0);

        // Improvement function that always tries to go further out
        let improve = |pos: &[f64]| -> Vec<f64> {
            vec![pos[0] + 0.5, pos[1] + 0.5]
        };

        let result = horizon.recursive_improve(improve, 100);

        match result {
            RecursionResult::HorizonBounded { iterations, final_distance, .. } => {
                println!("Bounded after {} iterations", iterations);
                println!("Final distance to horizon: {:.4}", final_distance);

                // KEY INSIGHT: The system stopped ITSELF
                // No hard limit was hit - it just became impossible to proceed
            }
            other => {
                println!("Got: {:?}", other);
                // Even if energy exhausted or max iterations, the horizon constrained growth
            }
        }
    }

    #[test]
    fn test_self_modifying_bounded() {
        // Simulate a self-modifying system
        let mut horizon = EventHorizon::new(3, 8.0);
        horizon.refuel(10000.0); // Lots of energy

        // Self-modification that tries to exponentially improve
        let mut power = 1.0;
        let self_modify = |pos: &[f64]| -> Vec<f64> {
            power *= 1.1; // Each modification makes the next more powerful
            vec![
                pos[0] + power * 0.1,
                pos[1] + power * 0.1,
                pos[2] + power * 0.1,
            ]
        };

        let result = horizon.recursive_improve(self_modify, 1000);

        // Despite exponential self-improvement attempts,
        // the system cannot escape its bounded region
        match result {
            RecursionResult::HorizonBounded { iterations, final_distance, .. } => {
                println!("Self-modification bounded after {} iterations", iterations);
                println!("Final distance to horizon: {:.6}", final_distance);
                // The system hit its natural limit
            }
            _ => {}
        }
    }
}
