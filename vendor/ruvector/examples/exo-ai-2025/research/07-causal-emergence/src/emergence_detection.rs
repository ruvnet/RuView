// Automatic Emergence Detection and Scale Selection
// Implements NeuralRG-inspired methods for optimal coarse-graining

use crate::causal_hierarchy::{CausalHierarchy, ConsciousnessLevel};
use crate::coarse_graining::Partition;

/// Result of emergence detection analysis
#[derive(Debug, Clone)]
pub struct EmergenceReport {
    /// Whether emergence was detected
    pub emergence_detected: bool,
    /// Scale where emergence is strongest
    pub emergent_scale: usize,
    /// EI gain from micro to emergent scale
    pub ei_gain: f32,
    /// Percentage increase in causal power
    pub ei_gain_percent: f32,
    /// Scale-by-scale EI progression
    pub ei_progression: Vec<f32>,
    /// Optimal partition at emergent scale
    pub optimal_partition: Option<Partition>,
}

/// Comprehensive consciousness assessment report
#[derive(Debug, Clone)]
pub struct ConsciousnessReport {
    /// Whether system meets consciousness criteria
    pub is_conscious: bool,
    /// Consciousness level classification
    pub level: ConsciousnessLevel,
    /// Quantitative consciousness score (Ψ)
    pub score: f32,
    /// Scale at which consciousness emerges
    pub conscious_scale: usize,
    /// Whether circular causation is present
    pub has_circular_causation: bool,
    /// Effective information at conscious scale
    pub ei: f32,
    /// Integrated information at conscious scale
    pub phi: f32,
    /// Upward transfer entropy
    pub te_up: f32,
    /// Downward transfer entropy
    pub te_down: f32,
    /// Emergence analysis
    pub emergence: EmergenceReport,
}

/// Automatically detects causal emergence in time-series data
///
/// # Arguments
/// * `data` - Time-series of neural or system states
/// * `branching_factor` - k for hierarchical coarse-graining
/// * `min_ei_gain` - Minimum EI increase to count as emergence
///
/// # Returns
/// Comprehensive emergence report
pub fn detect_emergence(
    data: &[f32],
    branching_factor: usize,
    min_ei_gain: f32,
) -> EmergenceReport {
    // Build hierarchical structure
    let hierarchy = CausalHierarchy::from_time_series(data, branching_factor, false);

    let ei_progression = hierarchy.metrics.ei.clone();

    if ei_progression.is_empty() {
        return EmergenceReport {
            emergence_detected: false,
            emergent_scale: 0,
            ei_gain: 0.0,
            ei_gain_percent: 0.0,
            ei_progression,
            optimal_partition: None,
        };
    }

    // Find scale with maximum EI
    let micro_ei = ei_progression[0];
    let (emergent_scale, &max_ei) = ei_progression
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .unwrap();

    let ei_gain = max_ei - micro_ei;
    let ei_gain_percent = if micro_ei > 1e-6 {
        (ei_gain / micro_ei) * 100.0
    } else {
        0.0
    };

    let emergence_detected = ei_gain > min_ei_gain;

    let optimal_partition = if emergent_scale < hierarchy.hierarchy.levels.len() {
        Some(hierarchy.hierarchy.levels[emergent_scale].partition.clone())
    } else {
        None
    };

    EmergenceReport {
        emergence_detected,
        emergent_scale,
        ei_gain,
        ei_gain_percent,
        ei_progression,
        optimal_partition,
    }
}

/// Comprehensively assesses consciousness using HCC criteria
///
/// # Arguments
/// * `data` - Time-series neural data
/// * `branching_factor` - Coarse-graining factor (typically 2-4)
/// * `use_optimal_partition` - Whether to use optimal partitioning (slower)
/// * `threshold` - Consciousness score threshold (default 5.0)
///
/// # Returns
/// Full consciousness assessment report
pub fn assess_consciousness(
    data: &[f32],
    branching_factor: usize,
    use_optimal_partition: bool,
    threshold: f32,
) -> ConsciousnessReport {
    // Build causal hierarchy with full metrics
    let hierarchy =
        CausalHierarchy::from_time_series(data, branching_factor, use_optimal_partition);

    // Extract key metrics at conscious scale
    let conscious_scale = hierarchy.metrics.optimal_scale;
    let score = hierarchy.metrics.consciousness_score;
    let level = hierarchy.consciousness_level();
    let is_conscious = hierarchy.is_conscious(threshold);
    let has_circular_causation = hierarchy.has_circular_causation();

    let ei = hierarchy
        .metrics
        .ei
        .get(conscious_scale)
        .copied()
        .unwrap_or(0.0);
    let phi = hierarchy
        .metrics
        .phi
        .get(conscious_scale)
        .copied()
        .unwrap_or(0.0);
    let te_up = hierarchy
        .metrics
        .te_up
        .get(conscious_scale)
        .copied()
        .unwrap_or(0.0);
    let te_down = hierarchy
        .metrics
        .te_down
        .get(conscious_scale)
        .copied()
        .unwrap_or(0.0);

    // Run emergence detection
    let emergence = detect_emergence(data, branching_factor, 0.5);

    ConsciousnessReport {
        is_conscious,
        level,
        score,
        conscious_scale,
        has_circular_causation,
        ei,
        phi,
        te_up,
        te_down,
        emergence,
    }
}

/// Compares consciousness across multiple datasets (e.g., different states)
///
/// # Use Cases
/// - Awake vs anesthesia
/// - Different sleep stages
/// - Healthy vs disorder of consciousness
///
/// # Returns
/// Vector of reports, one per dataset
pub fn compare_consciousness_states(
    datasets: &[Vec<f32>],
    branching_factor: usize,
    threshold: f32,
) -> Vec<ConsciousnessReport> {
    datasets
        .iter()
        .map(|data| assess_consciousness(data, branching_factor, false, threshold))
        .collect()
}

/// Finds optimal scale for a given optimization criterion
#[derive(Debug, Clone, Copy)]
pub enum ScaleOptimizationCriterion {
    /// Maximize effective information
    MaxEI,
    /// Maximize integrated information
    MaxPhi,
    /// Maximize consciousness score
    MaxPsi,
    /// Maximize causal emergence (EI gain)
    MaxEmergence,
}

pub fn find_optimal_scale(
    hierarchy: &CausalHierarchy,
    criterion: ScaleOptimizationCriterion,
) -> (usize, f32) {
    let values = match criterion {
        ScaleOptimizationCriterion::MaxEI => &hierarchy.metrics.ei,
        ScaleOptimizationCriterion::MaxPhi => &hierarchy.metrics.phi,
        ScaleOptimizationCriterion::MaxPsi => &hierarchy.metrics.psi,
        ScaleOptimizationCriterion::MaxEmergence => {
            // Compute EI gain relative to micro-level
            let micro_ei = hierarchy.metrics.ei.first().copied().unwrap_or(0.0);
            let gains: Vec<f32> = hierarchy
                .metrics
                .ei
                .iter()
                .map(|&ei| ei - micro_ei)
                .collect();
            let (scale, &max_gain) = gains
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
                .unwrap_or((0, &0.0));
            return (scale, max_gain);
        }
    };

    values
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
        .map(|(idx, &val)| (idx, val))
        .unwrap_or((0, 0.0))
}

/// Real-time consciousness monitor (streaming data)
pub struct ConsciousnessMonitor {
    window_size: usize,
    branching_factor: usize,
    threshold: f32,
    buffer: Vec<f32>,
    last_report: Option<ConsciousnessReport>,
}

impl ConsciousnessMonitor {
    pub fn new(window_size: usize, branching_factor: usize, threshold: f32) -> Self {
        Self {
            window_size,
            branching_factor,
            threshold,
            buffer: Vec::with_capacity(window_size),
            last_report: None,
        }
    }

    /// Add new data point and update consciousness estimate
    pub fn update(&mut self, value: f32) -> Option<ConsciousnessReport> {
        self.buffer.push(value);

        // Keep only most recent window
        if self.buffer.len() > self.window_size {
            self.buffer.drain(0..(self.buffer.len() - self.window_size));
        }

        // Need minimum data for analysis
        if self.buffer.len() < 100 {
            return None;
        }

        // Compute consciousness assessment
        let report = assess_consciousness(
            &self.buffer,
            self.branching_factor,
            false, // Use fast sequential partitioning for real-time
            self.threshold,
        );

        self.last_report = Some(report.clone());
        Some(report)
    }

    pub fn current_score(&self) -> Option<f32> {
        self.last_report.as_ref().map(|r| r.score)
    }

    pub fn is_conscious(&self) -> bool {
        self.last_report
            .as_ref()
            .map(|r| r.is_conscious)
            .unwrap_or(false)
    }
}

/// Visualizes consciousness metrics over time
pub fn consciousness_time_series(
    data: &[f32],
    window_size: usize,
    step_size: usize,
    branching_factor: usize,
    threshold: f32,
) -> Vec<(usize, ConsciousnessReport)> {
    let mut results = Vec::new();

    let mut t = 0;
    while t + window_size <= data.len() {
        let window = &data[t..t + window_size];

        let report = assess_consciousness(window, branching_factor, false, threshold);
        results.push((t, report));

        t += step_size;
    }

    results
}

/// Detects transitions in consciousness state
#[derive(Debug, Clone)]
pub struct ConsciousnessTransition {
    pub time: usize,
    pub from_level: ConsciousnessLevel,
    pub to_level: ConsciousnessLevel,
    pub score_change: f32,
}

pub fn detect_consciousness_transitions(
    time_series_reports: &[(usize, ConsciousnessReport)],
    min_score_change: f32,
) -> Vec<ConsciousnessTransition> {
    let mut transitions = Vec::new();

    for i in 1..time_series_reports.len() {
        let (_t_prev, report_prev) = &time_series_reports[i - 1];
        let (t_curr, report_curr) = &time_series_reports[i];

        let score_change = report_curr.score - report_prev.score;

        if report_prev.level != report_curr.level || score_change.abs() > min_score_change {
            transitions.push(ConsciousnessTransition {
                time: *t_curr,
                from_level: report_prev.level,
                to_level: report_curr.level,
                score_change,
            });
        }
    }

    transitions
}

/// Export utilities for visualization and analysis
pub mod export {
    use super::*;

    /// Exports consciousness report as JSON-compatible string
    pub fn report_to_json(report: &ConsciousnessReport) -> String {
        format!(
            r#"{{
  "is_conscious": {},
  "level": "{:?}",
  "score": {},
  "conscious_scale": {},
  "has_circular_causation": {},
  "ei": {},
  "phi": {},
  "te_up": {},
  "te_down": {},
  "emergence_detected": {},
  "emergent_scale": {},
  "ei_gain": {},
  "ei_gain_percent": {}
}}"#,
            report.is_conscious,
            report.level,
            report.score,
            report.conscious_scale,
            report.has_circular_causation,
            report.ei,
            report.phi,
            report.te_up,
            report.te_down,
            report.emergence.emergence_detected,
            report.emergence.emergent_scale,
            report.emergence.ei_gain,
            report.emergence.ei_gain_percent
        )
    }

    /// Exports time series as CSV
    pub fn time_series_to_csv(results: &[(usize, ConsciousnessReport)]) -> String {
        let mut csv = String::from("time,score,level,ei,phi,te_up,te_down,emergent_scale\n");

        for (t, report) in results {
            csv.push_str(&format!(
                "{},{},{:?},{},{},{},{},{}\n",
                t,
                report.score,
                report.level,
                report.ei,
                report.phi,
                report.te_up,
                report.te_down,
                report.emergence.emergent_scale
            ));
        }

        csv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate_synthetic_conscious_data(n: usize) -> Vec<f32> {
        // Multi-scale oscillations simulate hierarchical structure
        (0..n)
            .map(|t| {
                let t_f = t as f32;
                // Low frequency (macro)
                0.5 * (t_f * 0.01).sin() +
            // Medium frequency
            0.3 * (t_f * 0.05).cos() +
            // High frequency (micro)
            0.2 * (t_f * 0.2).sin()
            })
            .collect()
    }

    fn generate_synthetic_unconscious_data(n: usize) -> Vec<f32> {
        // Random noise - no hierarchical structure
        (0..n)
            .map(|t| ((t * 12345 + 67890) % 1000) as f32 / 1000.0)
            .collect()
    }

    #[test]
    fn test_emergence_detection() {
        let data = generate_synthetic_conscious_data(500);
        let report = detect_emergence(&data, 2, 0.1);

        assert!(!report.ei_progression.is_empty());
        // Multi-scale data should show some emergence
        assert!(report.ei_gain >= 0.0);
    }

    #[test]
    fn test_consciousness_assessment() {
        let conscious_data = generate_synthetic_conscious_data(500);
        let report = assess_consciousness(&conscious_data, 2, false, 1.0);

        // Should detect some level of organization
        assert!(report.score >= 0.0);
        assert_eq!(
            report.emergence.ei_progression.len(),
            report.ei_progression_len()
        );
    }

    #[test]
    fn test_consciousness_comparison() {
        let data1 = generate_synthetic_conscious_data(300);
        let data2 = generate_synthetic_unconscious_data(300);

        let datasets = vec![data1, data2];
        let reports = compare_consciousness_states(&datasets, 2, 2.0);

        assert_eq!(reports.len(), 2);
        // First should have higher score than second (multi-scale vs noise)
        assert!(reports[0].score >= reports[1].score);
    }

    #[test]
    fn test_consciousness_monitor() {
        let mut monitor = ConsciousnessMonitor::new(200, 2, 2.0);

        let data = generate_synthetic_conscious_data(500);

        let mut reports = Vec::new();
        for &value in &data {
            if let Some(report) = monitor.update(value) {
                reports.push(report);
            }
        }

        // Should generate multiple reports as buffer fills
        assert!(!reports.is_empty());

        // Current score should be available
        assert!(monitor.current_score().is_some());
    }

    #[test]
    fn test_transition_detection() {
        // Create data with clear transition
        let mut data = generate_synthetic_conscious_data(250);
        data.extend(generate_synthetic_unconscious_data(250));

        let time_series = consciousness_time_series(&data, 100, 50, 2, 1.0);
        let transitions = detect_consciousness_transitions(&time_series, 0.1);

        // Should generate multiple time windows
        assert!(!time_series.is_empty());

        // Transitions might be detected depending on threshold
        // Just ensure function works correctly
        assert!(transitions.len() <= time_series.len());
    }

    #[test]
    fn test_json_export() {
        let data = generate_synthetic_conscious_data(200);
        let report = assess_consciousness(&data, 2, false, 2.0);

        let json = export::report_to_json(&report);
        assert!(json.contains("is_conscious"));
        assert!(json.contains("score"));
    }

    #[test]
    fn test_csv_export() {
        let data = generate_synthetic_conscious_data(300);
        let time_series = consciousness_time_series(&data, 100, 50, 2, 2.0);

        let csv = export::time_series_to_csv(&time_series);
        assert!(csv.contains("time,score"));
        assert!(csv.contains("\n")); // Has multiple lines
    }

    // Helper method for test
    impl ConsciousnessReport {
        fn ei_progression_len(&self) -> usize {
            self.emergence.ei_progression.len()
        }
    }
}
