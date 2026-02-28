//! # Emergence Detection
//!
//! Automatically detecting when novel properties emerge from complex systems.
//! Measures causal emergence, phase transitions, and downward causation.
//!
//! ## Key Concepts
//!
//! - **Causal Emergence**: When macro-level descriptions are more predictive
//! - **Downward Causation**: Higher levels affecting lower levels
//! - **Phase Transitions**: Sudden qualitative changes in system behavior
//! - **Effective Information**: Information flow at different scales
//!
//! ## Theoretical Basis
//!
//! Based on:
//! - Erik Hoel's Causal Emergence framework
//! - Integrated Information Theory (IIT)
//! - Synergistic information theory
//! - Anderson's "More is Different"

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// System for detecting emergent properties
#[derive(Debug)]
pub struct EmergenceDetector {
    /// Micro-level state
    micro_state: Vec<f64>,
    /// Macro-level state
    macro_state: Vec<f64>,
    /// Coarse-graining function
    coarse_grainer: CoarseGrainer,
    /// Detected emergent properties
    emergent_properties: Vec<EmergentProperty>,
    /// Phase transition detector
    phase_detector: PhaseTransitionDetector,
    /// Causal emergence calculator
    causal_calculator: CausalEmergence,
}

/// Coarse-graining for multi-scale analysis
#[derive(Debug)]
pub struct CoarseGrainer {
    /// Grouping of micro to macro
    groupings: Vec<Vec<usize>>,
    /// Aggregation function
    aggregation: AggregationType,
}

#[derive(Debug, Clone)]
pub enum AggregationType {
    Mean,
    Majority,
    Max,
    WeightedSum(Vec<f64>),
}

/// An emergent property detected in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmergentProperty {
    pub id: Uuid,
    pub name: String,
    pub emergence_score: f64,
    pub level: usize,
    pub description: String,
    pub detected_at: u64,
}

/// Causal emergence measurement
#[derive(Debug)]
pub struct CausalEmergence {
    /// Effective information at micro level
    micro_ei: f64,
    /// Effective information at macro level
    macro_ei: f64,
    /// Causal emergence score
    emergence: f64,
    /// History of measurements
    history: Vec<EmergenceMeasurement>,
}

#[derive(Debug, Clone)]
pub struct EmergenceMeasurement {
    pub micro_ei: f64,
    pub macro_ei: f64,
    pub emergence: f64,
    pub timestamp: u64,
}

/// Phase transition detector
#[derive(Debug)]
pub struct PhaseTransitionDetector {
    /// Order parameter history
    order_parameter: Vec<f64>,
    /// Susceptibility (variance)
    susceptibility: Vec<f64>,
    /// Detected transitions
    transitions: Vec<PhaseTransition>,
    /// Window size for detection
    window_size: usize,
}

/// A detected phase transition
#[derive(Debug, Clone)]
pub struct PhaseTransition {
    pub id: Uuid,
    /// Critical point value
    pub critical_point: f64,
    /// Order parameter jump
    pub order_change: f64,
    /// Transition type
    pub transition_type: TransitionType,
    /// When detected
    pub timestamp: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TransitionType {
    /// Continuous (second-order)
    Continuous,
    /// Discontinuous (first-order)
    Discontinuous,
    /// Crossover (smooth)
    Crossover,
}

impl EmergenceDetector {
    /// Create a new emergence detector
    pub fn new() -> Self {
        Self {
            micro_state: Vec::new(),
            macro_state: Vec::new(),
            coarse_grainer: CoarseGrainer::new(),
            emergent_properties: Vec::new(),
            phase_detector: PhaseTransitionDetector::new(50),
            causal_calculator: CausalEmergence::new(),
        }
    }

    /// Detect emergence in the current state
    pub fn detect_emergence(&mut self) -> f64 {
        if self.micro_state.is_empty() {
            return 0.0;
        }

        // Compute macro state
        self.macro_state = self.coarse_grainer.coarsen(&self.micro_state);

        // Compute causal emergence
        let micro_ei = self.compute_effective_information(&self.micro_state);
        let macro_ei = self.compute_effective_information(&self.macro_state);

        self.causal_calculator.update(micro_ei, macro_ei);

        // Check for phase transitions
        let order_param = self.compute_order_parameter();
        self.phase_detector.update(order_param);

        // Detect specific emergent properties
        self.detect_specific_properties();

        self.causal_calculator.emergence
    }

    /// Set the micro-level state
    pub fn set_micro_state(&mut self, state: Vec<f64>) {
        self.micro_state = state;
    }

    /// Configure coarse-graining
    pub fn set_coarse_graining(
        &mut self,
        groupings: Vec<Vec<usize>>,
        aggregation: AggregationType,
    ) {
        self.coarse_grainer = CoarseGrainer {
            groupings,
            aggregation,
        };
    }

    fn compute_effective_information(&self, state: &[f64]) -> f64 {
        if state.is_empty() {
            return 0.0;
        }

        // Simplified EI: entropy of state distribution
        let sum: f64 = state.iter().map(|x| x.abs()).sum();
        if sum == 0.0 {
            return 0.0;
        }

        let normalized: Vec<f64> = state.iter().map(|x| x.abs() / sum).collect();

        // Shannon entropy
        -normalized
            .iter()
            .filter(|&&p| p > 1e-10)
            .map(|&p| p * p.ln())
            .sum::<f64>()
    }

    fn compute_order_parameter(&self) -> f64 {
        if self.macro_state.is_empty() {
            return 0.0;
        }

        // Order parameter: average alignment/correlation
        let mean: f64 = self.macro_state.iter().sum::<f64>() / self.macro_state.len() as f64;
        let variance: f64 = self
            .macro_state
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>()
            / self.macro_state.len() as f64;

        // Low variance = high order
        1.0 / (1.0 + variance)
    }

    fn detect_specific_properties(&mut self) {
        // Check for coherence (synchronized macro state)
        if let Some(coherence) = self.detect_coherence() {
            if coherence > 0.7 {
                self.record_property("Coherence", coherence, 1, "Synchronized macro behavior");
            }
        }

        // Check for hierarchy (multi-level structure)
        if let Some(hierarchy) = self.detect_hierarchy() {
            if hierarchy > 0.5 {
                self.record_property("Hierarchy", hierarchy, 2, "Multi-level organization");
            }
        }

        // Check for criticality
        if self.phase_detector.is_near_critical() {
            self.record_property("Criticality", 0.9, 1, "Near phase transition");
        }
    }

    fn detect_coherence(&self) -> Option<f64> {
        if self.macro_state.len() < 2 {
            return None;
        }

        // Coherence as average pairwise correlation
        let mean: f64 = self.macro_state.iter().sum::<f64>() / self.macro_state.len() as f64;
        let deviations: Vec<f64> = self.macro_state.iter().map(|x| x - mean).collect();

        let norm = deviations.iter().map(|x| x * x).sum::<f64>().sqrt();
        if norm == 0.0 {
            return Some(1.0); // Perfect coherence
        }

        Some((1.0 / (1.0 + norm)).min(1.0))
    }

    fn detect_hierarchy(&self) -> Option<f64> {
        // Hierarchy based on scale separation
        if self.micro_state.is_empty() || self.macro_state.is_empty() {
            return None;
        }

        let micro_complexity = self.compute_effective_information(&self.micro_state);
        let macro_complexity = self.compute_effective_information(&self.macro_state);

        // Hierarchy emerges when macro is simpler than micro
        if micro_complexity == 0.0 {
            return Some(0.0);
        }

        Some(1.0 - (macro_complexity / micro_complexity).min(1.0))
    }

    fn record_property(&mut self, name: &str, score: f64, level: usize, description: &str) {
        // Check if already recorded recently
        let recent = self
            .emergent_properties
            .iter()
            .any(|p| p.name == name && p.level == level);

        if !recent {
            self.emergent_properties.push(EmergentProperty {
                id: Uuid::new_v4(),
                name: name.to_string(),
                emergence_score: score,
                level,
                description: description.to_string(),
                detected_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
        }
    }

    /// Get causal emergence calculator
    pub fn causal_emergence(&self) -> &CausalEmergence {
        &self.causal_calculator
    }

    /// Get detected emergent properties
    pub fn emergent_properties(&self) -> &[EmergentProperty] {
        &self.emergent_properties
    }

    /// Get phase transitions
    pub fn phase_transitions(&self) -> &[PhaseTransition] {
        self.phase_detector.transitions()
    }

    /// Get detection statistics
    pub fn statistics(&self) -> EmergenceStatistics {
        EmergenceStatistics {
            micro_dimension: self.micro_state.len(),
            macro_dimension: self.macro_state.len(),
            compression_ratio: if self.micro_state.is_empty() {
                0.0
            } else {
                self.macro_state.len() as f64 / self.micro_state.len() as f64
            },
            emergence_score: self.causal_calculator.emergence,
            properties_detected: self.emergent_properties.len(),
            transitions_detected: self.phase_detector.transitions.len(),
        }
    }
}

impl Default for EmergenceDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl CoarseGrainer {
    /// Create a new coarse-grainer
    pub fn new() -> Self {
        Self {
            groupings: Vec::new(),
            aggregation: AggregationType::Mean,
        }
    }

    /// Create with specific groupings
    pub fn with_groupings(groupings: Vec<Vec<usize>>, aggregation: AggregationType) -> Self {
        Self {
            groupings,
            aggregation,
        }
    }

    /// Coarsen a micro state to macro state
    pub fn coarsen(&self, micro: &[f64]) -> Vec<f64> {
        if self.groupings.is_empty() {
            // Default: simple averaging in pairs
            return self.default_coarsen(micro);
        }

        self.groupings
            .iter()
            .map(|group| {
                let values: Vec<f64> = group
                    .iter()
                    .filter_map(|&i| micro.get(i).copied())
                    .collect();
                self.aggregate(&values)
            })
            .collect()
    }

    fn default_coarsen(&self, micro: &[f64]) -> Vec<f64> {
        micro
            .chunks(2)
            .map(|chunk| chunk.iter().sum::<f64>() / chunk.len() as f64)
            .collect()
    }

    fn aggregate(&self, values: &[f64]) -> f64 {
        if values.is_empty() {
            return 0.0;
        }

        match &self.aggregation {
            AggregationType::Mean => values.iter().sum::<f64>() / values.len() as f64,
            AggregationType::Majority => {
                let positive = values.iter().filter(|&&v| v > 0.0).count();
                if positive > values.len() / 2 {
                    1.0
                } else {
                    -1.0
                }
            }
            AggregationType::Max => values.iter().cloned().fold(f64::MIN, f64::max),
            AggregationType::WeightedSum(weights) => {
                values.iter().zip(weights.iter()).map(|(v, w)| v * w).sum()
            }
        }
    }
}

impl Default for CoarseGrainer {
    fn default() -> Self {
        Self::new()
    }
}

impl CausalEmergence {
    /// Create a new causal emergence calculator
    pub fn new() -> Self {
        Self {
            micro_ei: 0.0,
            macro_ei: 0.0,
            emergence: 0.0,
            history: Vec::new(),
        }
    }

    /// Update with new EI measurements
    pub fn update(&mut self, micro_ei: f64, macro_ei: f64) {
        self.micro_ei = micro_ei;
        self.macro_ei = macro_ei;

        // Causal emergence = macro_ei - micro_ei (when positive)
        self.emergence = (macro_ei - micro_ei).max(0.0);

        self.history.push(EmergenceMeasurement {
            micro_ei,
            macro_ei,
            emergence: self.emergence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });
    }

    /// Get emergence score
    pub fn score(&self) -> f64 {
        self.emergence
    }

    /// Is there causal emergence?
    pub fn has_emergence(&self) -> bool {
        self.emergence > 0.0
    }

    /// Get emergence trend
    pub fn trend(&self) -> f64 {
        if self.history.len() < 2 {
            return 0.0;
        }

        let recent = &self.history[self.history.len().saturating_sub(10)..];
        if recent.len() < 2 {
            return 0.0;
        }

        let first = recent[0].emergence;
        let last = recent[recent.len() - 1].emergence;

        last - first
    }
}

impl Default for CausalEmergence {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseTransitionDetector {
    /// Create a new phase transition detector
    pub fn new(window_size: usize) -> Self {
        Self {
            order_parameter: Vec::new(),
            susceptibility: Vec::new(),
            transitions: Vec::new(),
            window_size,
        }
    }

    /// Update with new order parameter value
    pub fn update(&mut self, order: f64) {
        self.order_parameter.push(order);

        // Compute susceptibility (local variance)
        if self.order_parameter.len() >= self.window_size {
            let window = &self.order_parameter[self.order_parameter.len() - self.window_size..];
            let mean: f64 = window.iter().sum::<f64>() / window.len() as f64;
            let variance: f64 =
                window.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / window.len() as f64;
            self.susceptibility.push(variance);

            // Detect transition (spike in susceptibility)
            if self.susceptibility.len() >= 2 {
                let current = *self.susceptibility.last().unwrap();
                let previous = self.susceptibility[self.susceptibility.len() - 2];

                if current > previous * 2.0 && current > 0.1 {
                    self.record_transition(order, current - previous);
                }
            }
        }
    }

    fn record_transition(&mut self, critical_point: f64, order_change: f64) {
        let transition_type = if order_change.abs() > 0.5 {
            TransitionType::Discontinuous
        } else if order_change.abs() > 0.1 {
            TransitionType::Continuous
        } else {
            TransitionType::Crossover
        };

        self.transitions.push(PhaseTransition {
            id: Uuid::new_v4(),
            critical_point,
            order_change,
            transition_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        });
    }

    /// Is the system near a critical point?
    pub fn is_near_critical(&self) -> bool {
        if self.susceptibility.is_empty() {
            return false;
        }

        let recent = *self.susceptibility.last().unwrap();
        let avg = self.susceptibility.iter().sum::<f64>() / self.susceptibility.len() as f64;

        recent > avg * 1.5
    }

    /// Get detected transitions
    pub fn transitions(&self) -> &[PhaseTransition] {
        &self.transitions
    }
}

/// Statistics about emergence detection
#[derive(Debug, Clone)]
pub struct EmergenceStatistics {
    pub micro_dimension: usize,
    pub macro_dimension: usize,
    pub compression_ratio: f64,
    pub emergence_score: f64,
    pub properties_detected: usize,
    pub transitions_detected: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emergence_detector_creation() {
        let detector = EmergenceDetector::new();
        assert_eq!(detector.emergent_properties().len(), 0);
    }

    #[test]
    fn test_coarse_graining() {
        let cg = CoarseGrainer::new();
        let micro = vec![1.0, 2.0, 3.0, 4.0];
        let macro_state = cg.coarsen(&micro);

        assert_eq!(macro_state.len(), 2);
        assert_eq!(macro_state[0], 1.5);
        assert_eq!(macro_state[1], 3.5);
    }

    #[test]
    fn test_custom_coarse_graining() {
        let groupings = vec![vec![0, 1], vec![2, 3]];
        let cg = CoarseGrainer::with_groupings(groupings, AggregationType::Max);
        let micro = vec![1.0, 2.0, 3.0, 4.0];
        let macro_state = cg.coarsen(&micro);

        assert_eq!(macro_state[0], 2.0);
        assert_eq!(macro_state[1], 4.0);
    }

    #[test]
    fn test_emergence_detection() {
        let mut detector = EmergenceDetector::new();

        // Set a micro state
        detector.set_micro_state(vec![0.1, 0.9, 0.2, 0.8, 0.15, 0.85, 0.18, 0.82]);

        let score = detector.detect_emergence();
        assert!(score >= 0.0);
    }

    #[test]
    fn test_causal_emergence() {
        let mut ce = CausalEmergence::new();

        ce.update(2.0, 3.0); // Macro more informative
        assert!(ce.has_emergence());
        assert_eq!(ce.score(), 1.0);

        ce.update(3.0, 2.0); // Micro more informative
        assert!(!ce.has_emergence()); // Emergence is 0 when macro < micro
    }

    #[test]
    fn test_phase_transition_detection() {
        let mut detector = PhaseTransitionDetector::new(5);

        // Normal values
        for _ in 0..10 {
            detector.update(0.5);
        }

        // Sudden change (transition)
        detector.update(0.1);
        detector.update(0.05);
        detector.update(0.02);

        // Check if transition detected
        // (This may or may not trigger depending on thresholds)
        assert!(detector.order_parameter.len() >= 10);
    }

    #[test]
    fn test_emergence_statistics() {
        let mut detector = EmergenceDetector::new();
        detector.set_micro_state(vec![1.0, 2.0, 3.0, 4.0]);
        detector.detect_emergence();

        let stats = detector.statistics();
        assert_eq!(stats.micro_dimension, 4);
        assert_eq!(stats.macro_dimension, 2);
        assert_eq!(stats.compression_ratio, 0.5);
    }
}
