//! Lateral Inhibition Model
//!
//! Implements inhibitory connections between neurons for winner-take-all dynamics.

/// Lateral inhibition mechanism
///
/// Models inhibitory connections between neurons, where active neurons
/// suppress nearby neurons through inhibitory synapses.
///
/// # Model
///
/// - Mexican hat connectivity (surround inhibition)
/// - Distance-based inhibition strength
/// - Exponential decay with time
///
/// # Example
///
/// ```
/// use ruvector_nervous_system::compete::LateralInhibition;
///
/// let mut inhibition = LateralInhibition::new(10, 0.5, 0.9);
/// let mut activations = vec![0.1, 0.2, 0.9, 0.3, 0.1, 0.0, 0.0, 0.0, 0.0, 0.0];
/// inhibition.apply(&mut activations, 2); // Winner at index 2
/// // Activations at indices near 2 will be suppressed
/// ```
#[derive(Debug, Clone)]
pub struct LateralInhibition {
    /// Inhibitory connection weights (sparse representation)
    size: usize,

    /// Base inhibition strength
    strength: f32,

    /// Temporal decay factor
    decay: f32,

    /// Inhibition radius (neurons within this distance are inhibited)
    radius: usize,
}

impl LateralInhibition {
    /// Create a new lateral inhibition model
    ///
    /// # Arguments
    ///
    /// * `size` - Number of neurons
    /// * `strength` - Base inhibition strength (0.0-1.0)
    /// * `decay` - Temporal decay factor (0.0-1.0)
    pub fn new(size: usize, strength: f32, decay: f32) -> Self {
        Self {
            size,
            strength: strength.clamp(0.0, 1.0),
            decay: decay.clamp(0.0, 1.0),
            radius: (size as f32).sqrt() as usize, // Default radius based on size
        }
    }

    /// Set inhibition radius
    pub fn with_radius(mut self, radius: usize) -> Self {
        self.radius = radius;
        self
    }

    /// Apply lateral inhibition from winner neuron
    ///
    /// Suppresses activations of neurons near the winner based on distance.
    ///
    /// # Arguments
    ///
    /// * `activations` - Current activation levels (modified in-place)
    /// * `winner` - Index of winning neuron
    pub fn apply(&self, activations: &mut [f32], winner: usize) {
        assert_eq!(activations.len(), self.size, "Activation size mismatch");
        assert!(winner < self.size, "Winner index out of bounds");

        let winner_activation = activations[winner];

        for (i, activation) in activations.iter_mut().enumerate() {
            if i == winner {
                continue; // Don't inhibit winner
            }

            // Calculate distance (can use topology-aware distance in future)
            let distance = if i > winner { i - winner } else { winner - i };

            if distance <= self.radius {
                // Inhibition strength decreases with distance (Mexican hat)
                let distance_factor = 1.0 - (distance as f32 / self.radius as f32);
                let inhibition = self.strength * distance_factor * winner_activation;

                // Apply inhibition (multiplicative suppression)
                *activation *= 1.0 - inhibition;
            }
        }
    }

    /// Apply global inhibition (all neurons inhibit all others)
    ///
    /// Used for sparse coding where multiple weak activations compete.
    pub fn apply_global(&self, activations: &mut [f32]) {
        let total_activation: f32 = activations.iter().sum();
        let mean_activation = total_activation / activations.len() as f32;

        for activation in activations.iter_mut() {
            // Global inhibition proportional to mean activity
            let inhibition = self.strength * mean_activation;
            *activation = (*activation - inhibition).max(0.0);
        }
    }

    /// Compute inhibitory weight between two neurons
    ///
    /// Returns inhibition strength based on distance and connectivity pattern.
    pub fn weight(&self, from: usize, to: usize) -> f32 {
        if from == to {
            return 0.0; // No self-inhibition
        }

        let distance = if to > from { to - from } else { from - to };

        if distance > self.radius {
            return 0.0;
        }

        // Mexican hat profile
        let distance_factor = 1.0 - (distance as f32 / self.radius as f32);
        self.strength * distance_factor
    }

    /// Get full inhibition matrix (for visualization/analysis)
    ///
    /// Returns a size × size matrix of inhibitory weights.
    /// Note: This is expensive and should only be used for debugging.
    pub fn weight_matrix(&self) -> Vec<Vec<f32>> {
        let mut matrix = vec![vec![0.0; self.size]; self.size];

        for i in 0..self.size {
            for j in 0..self.size {
                matrix[i][j] = self.weight(i, j);
            }
        }

        matrix
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inhibition_basic() {
        let inhibition = LateralInhibition::new(10, 0.8, 0.9);
        let mut activations = vec![0.0; 10];
        activations[5] = 1.0; // Strong activation at index 5
        activations[4] = 0.5; // Weak activation nearby
        activations[6] = 0.5; // Weak activation nearby

        inhibition.apply(&mut activations, 5);

        // Winner should remain unchanged
        assert_eq!(activations[5], 1.0);

        // Nearby neurons should be suppressed
        assert!(activations[4] < 0.5, "Nearby neuron should be inhibited");
        assert!(activations[6] < 0.5, "Nearby neuron should be inhibited");
    }

    #[test]
    fn test_inhibition_radius() {
        let inhibition = LateralInhibition::new(20, 0.8, 0.9).with_radius(2);
        let mut activations = vec![0.5; 20];
        activations[10] = 1.0; // Winner

        inhibition.apply(&mut activations, 10);

        // Within radius should be inhibited
        assert!(activations[9] < 0.5);
        assert!(activations[11] < 0.5);

        // Outside radius should be less affected
        assert!(activations[7] >= activations[9]);
        assert!(activations[13] >= activations[11]);
    }

    #[test]
    fn test_inhibition_no_self_inhibition() {
        let inhibition = LateralInhibition::new(10, 1.0, 0.9);
        assert_eq!(inhibition.weight(5, 5), 0.0, "No self-inhibition");
    }

    #[test]
    fn test_inhibition_symmetric() {
        let inhibition = LateralInhibition::new(10, 0.8, 0.9);

        // Inhibition should be symmetric
        assert_eq!(
            inhibition.weight(3, 7),
            inhibition.weight(7, 3),
            "Inhibition should be symmetric"
        );
    }

    #[test]
    fn test_global_inhibition() {
        let inhibition = LateralInhibition::new(10, 0.5, 0.9);
        let mut activations = vec![0.8; 10];

        inhibition.apply_global(&mut activations);

        // All activations should be suppressed equally
        assert!(activations.iter().all(|&x| x < 0.8));
        assert!(activations.windows(2).all(|w| (w[0] - w[1]).abs() < 1e-6));
    }

    #[test]
    fn test_inhibition_strength_bounds() {
        let inhibition1 = LateralInhibition::new(10, -0.5, 0.9);
        let inhibition2 = LateralInhibition::new(10, 1.5, 0.9);

        // Strength should be clamped to [0, 1]
        assert_eq!(inhibition1.strength, 0.0);
        assert_eq!(inhibition2.strength, 1.0);
    }

    #[test]
    fn test_weight_matrix_structure() {
        let inhibition = LateralInhibition::new(5, 0.8, 0.9).with_radius(1);
        let matrix = inhibition.weight_matrix();

        // Matrix should be square
        assert_eq!(matrix.len(), 5);
        assert!(matrix.iter().all(|row| row.len() == 5));

        // Diagonal should be zero (no self-inhibition)
        for i in 0..5 {
            assert_eq!(matrix[i][i], 0.0);
        }

        // Should be symmetric
        for i in 0..5 {
            for j in 0..5 {
                assert_eq!(matrix[i][j], matrix[j][i]);
            }
        }
    }

    #[test]
    fn test_mexican_hat_profile() {
        let inhibition = LateralInhibition::new(10, 0.8, 0.9).with_radius(3);

        // Inhibition should decrease with distance
        let w1 = inhibition.weight(5, 6); // Distance 1
        let w2 = inhibition.weight(5, 7); // Distance 2
        let w3 = inhibition.weight(5, 8); // Distance 3

        assert!(w1 > w2, "Inhibition decreases with distance");
        assert!(w2 > w3, "Inhibition decreases with distance");
        assert_eq!(inhibition.weight(5, 9), 0.0, "Beyond radius");
    }
}
