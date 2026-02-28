//! Ergodic Consciousness Theory
//!
//! Explores the deep connection between ergodicity and integrated experience.
//!
//! # Central Hypothesis
//!
//! For ergodic cognitive systems, the property that time averages equal
//! ensemble averages may create a form of temporal integration that
//! constitutes or enables consciousness.
//!
//! # Mathematical Framework
//!
//! A system is ergodic if:
//!   lim (1/T) ∫₀ᵀ f(x(t)) dt = ∫ f(x) dμ(x)
//!   T→∞
//!
//! For consciousness:
//!   - Temporal integration: System's history integrated into steady state
//!   - Perspective invariance: Same statistics from any starting point
//!   - Self-similarity: Structure preserved across time scales

/// Ergodicity tester for cognitive systems
pub struct ErgodicityAnalyzer {
    /// Number of time steps for temporal average
    time_steps: usize,
    /// Number of initial conditions for ensemble average
    ensemble_size: usize,
    /// Tolerance for ergodicity test
    tolerance: f64,
}

impl Default for ErgodicityAnalyzer {
    fn default() -> Self {
        Self {
            time_steps: 10000,
            ensemble_size: 100,
            tolerance: 0.01,
        }
    }
}

impl ErgodicityAnalyzer {
    /// Create new analyzer with custom parameters
    pub fn new(time_steps: usize, ensemble_size: usize, tolerance: f64) -> Self {
        Self {
            time_steps,
            ensemble_size,
            tolerance,
        }
    }

    /// Test if system is ergodic
    ///
    /// Returns: (is_ergodic, mixing_time, ergodicity_score)
    pub fn test_ergodicity(
        &self,
        transition_matrix: &[Vec<f64>],
        observable: impl Fn(&[f64]) -> f64,
    ) -> ErgodicityResult {
        let n = transition_matrix.len();
        if n == 0 {
            return ErgodicityResult::non_ergodic();
        }

        // Step 1: Compute temporal average from random initial state
        let temporal_avg = self.temporal_average(transition_matrix, &observable);

        // Step 2: Compute ensemble average from many initial conditions
        let ensemble_avg = self.ensemble_average(transition_matrix, &observable);

        // Step 3: Compare
        let difference = (temporal_avg - ensemble_avg).abs();
        let ergodicity_score = 1.0 - (difference / temporal_avg.abs().max(1.0));

        // Step 4: Estimate mixing time
        let mixing_time = self.estimate_mixing_time(transition_matrix);

        ErgodicityResult {
            is_ergodic: difference < self.tolerance,
            temporal_average: temporal_avg,
            ensemble_average: ensemble_avg,
            difference,
            ergodicity_score,
            mixing_time,
        }
    }

    /// Compute temporal average: (1/T) Σ f(x(t))
    fn temporal_average(
        &self,
        transition_matrix: &[Vec<f64>],
        observable: &impl Fn(&[f64]) -> f64,
    ) -> f64 {
        let n = transition_matrix.len();

        // Random initial state
        let mut state = vec![0.0; n];
        state[0] = 1.0; // Start at first state

        let mut sum = 0.0;

        for _ in 0..self.time_steps {
            sum += observable(&state);
            state = self.evolve_state(transition_matrix, &state);
        }

        sum / self.time_steps as f64
    }

    /// Compute ensemble average: ∫ f(x) dμ(x)
    fn ensemble_average(
        &self,
        transition_matrix: &[Vec<f64>],
        observable: &impl Fn(&[f64]) -> f64,
    ) -> f64 {
        let n = transition_matrix.len();
        let mut sum = 0.0;

        // Average over random initial conditions
        for i in 0..self.ensemble_size {
            let mut state = vec![0.0; n];
            state[i % n] = 1.0;

            // Evolve to approximate steady state
            for _ in 0..1000 {
                state = self.evolve_state(transition_matrix, &state);
            }

            sum += observable(&state);
        }

        sum / self.ensemble_size as f64
    }

    /// Evolve state one time step
    fn evolve_state(&self, transition_matrix: &[Vec<f64>], state: &[f64]) -> Vec<f64> {
        let n = transition_matrix.len();
        let mut next_state = vec![0.0; n];

        for i in 0..n {
            for j in 0..n {
                next_state[i] += transition_matrix[j][i] * state[j];
            }
        }

        // Normalize
        let sum: f64 = next_state.iter().sum();
        if sum > 1e-10 {
            for x in &mut next_state {
                *x /= sum;
            }
        }

        next_state
    }

    /// Estimate mixing time (time to reach stationary distribution)
    fn estimate_mixing_time(&self, transition_matrix: &[Vec<f64>]) -> usize {
        let n = transition_matrix.len();

        // Start from peaked distribution
        let mut state = vec![0.0; n];
        state[0] = 1.0;

        // Target: uniform distribution (for symmetric systems)
        let target = vec![1.0 / n as f64; n];

        for t in 0..self.time_steps {
            // Check if close to stationary
            let distance: f64 = state
                .iter()
                .zip(target.iter())
                .map(|(a, b)| (a - b).abs())
                .sum();

            if distance < self.tolerance {
                return t;
            }

            state = self.evolve_state(transition_matrix, &state);
        }

        self.time_steps // Didn't converge
    }

    /// Test if mixing time is in optimal range for consciousness
    ///
    /// Hypothesis: Conscious systems have τ_mix ≈ 100-1000 steps
    /// (corresponding to ~100-1000ms in biological time)
    pub fn is_optimal_mixing_time(&self, mixing_time: usize) -> bool {
        mixing_time >= 100 && mixing_time <= 1000
    }
}

/// Result of ergodicity analysis
#[derive(Debug, Clone)]
pub struct ErgodicityResult {
    /// Whether system is ergodic (time avg ≈ ensemble avg)
    pub is_ergodic: bool,
    /// Temporal average value
    pub temporal_average: f64,
    /// Ensemble average value
    pub ensemble_average: f64,
    /// Absolute difference
    pub difference: f64,
    /// Ergodicity score (0-1, higher = more ergodic)
    pub ergodicity_score: f64,
    /// Mixing time (steps to reach stationary)
    pub mixing_time: usize,
}

impl ErgodicityResult {
    fn non_ergodic() -> Self {
        Self {
            is_ergodic: false,
            temporal_average: 0.0,
            ensemble_average: 0.0,
            difference: f64::INFINITY,
            ergodicity_score: 0.0,
            mixing_time: 0,
        }
    }

    /// Get consciousness compatibility score
    /// Combines ergodicity + optimal mixing time
    pub fn consciousness_score(&self) -> f64 {
        let ergodic_component = self.ergodicity_score;

        // Optimal mixing time: 100-1000 steps
        let mixing_component = if self.mixing_time >= 100 && self.mixing_time <= 1000 {
            1.0
        } else if self.mixing_time < 100 {
            self.mixing_time as f64 / 100.0
        } else {
            1000.0 / self.mixing_time as f64
        };

        (ergodic_component + mixing_component) / 2.0
    }
}

/// Consciousness-specific ergodicity metrics
pub struct ConsciousnessErgodicityMetrics {
    /// Temporal integration strength (how much history matters)
    pub temporal_integration: f64,
    /// Perspective invariance (similarity across initial conditions)
    pub perspective_invariance: f64,
    /// Self-similarity across time scales
    pub self_similarity: f64,
    /// Overall ergodic consciousness index
    pub ergodic_consciousness_index: f64,
}

impl ConsciousnessErgodicityMetrics {
    /// Compute from ergodicity result and system dynamics
    pub fn from_ergodicity(result: &ErgodicityResult, phi: f64) -> Self {
        // Temporal integration: how much mixing time vs total time
        let temporal_integration = (result.mixing_time as f64 / 10000.0).min(1.0);

        // Perspective invariance: ergodicity score
        let perspective_invariance = result.ergodicity_score;

        // Self-similarity: inverse of mixing time variance (stub for now)
        let self_similarity = 1.0 / (1.0 + result.mixing_time as f64 / 1000.0);

        // Overall index: weighted combination + Φ
        let ergodic_consciousness_index = (temporal_integration * 0.3
            + perspective_invariance * 0.3
            + self_similarity * 0.2
            + phi * 0.2)
            .min(1.0);

        Self {
            temporal_integration,
            perspective_invariance,
            self_similarity,
            ergodic_consciousness_index,
        }
    }

    /// Interpret consciousness level
    pub fn consciousness_level(&self) -> &str {
        if self.ergodic_consciousness_index > 0.8 {
            "High"
        } else if self.ergodic_consciousness_index > 0.5 {
            "Moderate"
        } else if self.ergodic_consciousness_index > 0.2 {
            "Low"
        } else {
            "Minimal"
        }
    }
}

/// Ergodic phase transition detector
///
/// Detects transitions between:
/// - Sub-ergodic (frozen, unconscious)
/// - Ergodic (critical, conscious)
/// - Super-ergodic (chaotic, fragmented)
pub struct ErgodicPhaseDetector {
    threshold_lower: f64,
    threshold_upper: f64,
}

impl Default for ErgodicPhaseDetector {
    fn default() -> Self {
        Self {
            threshold_lower: 0.95,
            threshold_upper: 1.05,
        }
    }
}

impl ErgodicPhaseDetector {
    /// Detect phase from dominant eigenvalue
    pub fn detect_phase(&self, dominant_eigenvalue: f64) -> ErgodicPhase {
        if dominant_eigenvalue < self.threshold_lower {
            ErgodicPhase::SubErgodic {
                eigenvalue: dominant_eigenvalue,
                description: "Frozen/sub-critical - may lack consciousness".to_string(),
            }
        } else if dominant_eigenvalue > self.threshold_upper {
            ErgodicPhase::SuperErgodic {
                eigenvalue: dominant_eigenvalue,
                description: "Chaotic/super-critical - fragmented consciousness".to_string(),
            }
        } else {
            ErgodicPhase::Ergodic {
                eigenvalue: dominant_eigenvalue,
                description: "Critical/ergodic - optimal for consciousness".to_string(),
            }
        }
    }
}

/// Ergodic phase of system
#[derive(Debug, Clone)]
pub enum ErgodicPhase {
    SubErgodic {
        eigenvalue: f64,
        description: String,
    },
    Ergodic {
        eigenvalue: f64,
        description: String,
    },
    SuperErgodic {
        eigenvalue: f64,
        description: String,
    },
}

impl ErgodicPhase {
    pub fn is_conscious_compatible(&self) -> bool {
        matches!(self, ErgodicPhase::Ergodic { .. })
    }

    pub fn eigenvalue(&self) -> f64 {
        match self {
            ErgodicPhase::SubErgodic { eigenvalue, .. } => *eigenvalue,
            ErgodicPhase::Ergodic { eigenvalue, .. } => *eigenvalue,
            ErgodicPhase::SuperErgodic { eigenvalue, .. } => *eigenvalue,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ergodic_cycle() {
        let analyzer = ErgodicityAnalyzer::new(1000, 50, 0.05);

        // Symmetric 4-cycle
        let mut transition = vec![vec![0.0; 4]; 4];
        transition[0][1] = 1.0;
        transition[1][2] = 1.0;
        transition[2][3] = 1.0;
        transition[3][0] = 1.0;

        // Observable: first component
        let observable = |state: &[f64]| state[0];

        let result = analyzer.test_ergodicity(&transition, observable);

        // Check ergodicity (may not converge due to deterministic cycle)
        // Deterministic cycles have special behavior
        assert!(result.mixing_time > 0);
        assert!(result.ergodicity_score >= 0.0 && result.ergodicity_score <= 1.0);
    }

    #[test]
    fn test_consciousness_score() {
        let result = ErgodicityResult {
            is_ergodic: true,
            temporal_average: 0.5,
            ensemble_average: 0.51,
            difference: 0.01,
            ergodicity_score: 0.98,
            mixing_time: 300, // Optimal range
        };

        let score = result.consciousness_score();
        assert!(score > 0.9); // Should be high
    }

    #[test]
    fn test_phase_detection() {
        let detector = ErgodicPhaseDetector::default();

        let sub = detector.detect_phase(0.5);
        assert!(matches!(sub, ErgodicPhase::SubErgodic { .. }));

        let ergodic = detector.detect_phase(1.0);
        assert!(matches!(ergodic, ErgodicPhase::Ergodic { .. }));
        assert!(ergodic.is_conscious_compatible());

        let super_ = detector.detect_phase(1.5);
        assert!(matches!(super_, ErgodicPhase::SuperErgodic { .. }));
    }

    #[test]
    fn test_consciousness_metrics() {
        let result = ErgodicityResult {
            is_ergodic: true,
            temporal_average: 0.5,
            ensemble_average: 0.5,
            difference: 0.0,
            ergodicity_score: 1.0,
            mixing_time: 500,
        };

        let metrics = ConsciousnessErgodicityMetrics::from_ergodicity(&result, 5.0);

        assert!(metrics.ergodic_consciousness_index > 0.5);
        // Check that consciousness level is computed correctly
        let level = metrics.consciousness_level();
        assert!(level == "High" || level == "Moderate");
    }
}
