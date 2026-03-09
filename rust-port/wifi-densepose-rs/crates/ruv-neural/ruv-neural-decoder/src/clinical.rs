//! Clinical biomarker detection from brain topology deviations.

use ruv_neural_core::topology::TopologyMetrics;

/// Clinical biomarker scorer based on topology deviation from a healthy baseline.
///
/// Computes z-scores of current topology metrics relative to a learned
/// healthy population baseline, then derives disease-specific risk scores
/// and a composite brain health index.
pub struct ClinicalScorer {
    /// Mean topology metrics from healthy population.
    healthy_baseline: TopologyMetrics,
    /// Standard deviation of topology metrics from healthy population.
    healthy_std: TopologyMetrics,
}

impl ClinicalScorer {
    /// Create a scorer with explicit baseline mean and standard deviation.
    pub fn new(baseline: TopologyMetrics, std: TopologyMetrics) -> Self {
        Self {
            healthy_baseline: baseline,
            healthy_std: std,
        }
    }

    /// Learn the healthy baseline from a set of healthy topology observations.
    ///
    /// Computes the mean and standard deviation of each metric across the
    /// provided samples.
    pub fn learn_baseline(&mut self, healthy_data: &[TopologyMetrics]) {
        if healthy_data.is_empty() {
            return;
        }

        let n = healthy_data.len() as f64;

        // Compute means.
        let mean_mincut = healthy_data.iter().map(|m| m.global_mincut).sum::<f64>() / n;
        let mean_mod = healthy_data.iter().map(|m| m.modularity).sum::<f64>() / n;
        let mean_eff = healthy_data.iter().map(|m| m.global_efficiency).sum::<f64>() / n;
        let mean_loc = healthy_data.iter().map(|m| m.local_efficiency).sum::<f64>() / n;
        let mean_ent = healthy_data.iter().map(|m| m.graph_entropy).sum::<f64>() / n;
        let mean_fiedler = healthy_data.iter().map(|m| m.fiedler_value).sum::<f64>() / n;

        self.healthy_baseline = TopologyMetrics {
            global_mincut: mean_mincut,
            modularity: mean_mod,
            global_efficiency: mean_eff,
            local_efficiency: mean_loc,
            graph_entropy: mean_ent,
            fiedler_value: mean_fiedler,
            num_modules: 0,
            timestamp: 0.0,
        };

        // Compute standard deviations.
        let std_mincut = std_dev(healthy_data.iter().map(|m| m.global_mincut), mean_mincut);
        let std_mod = std_dev(healthy_data.iter().map(|m| m.modularity), mean_mod);
        let std_eff = std_dev(
            healthy_data.iter().map(|m| m.global_efficiency),
            mean_eff,
        );
        let std_loc = std_dev(
            healthy_data.iter().map(|m| m.local_efficiency),
            mean_loc,
        );
        let std_ent = std_dev(healthy_data.iter().map(|m| m.graph_entropy), mean_ent);
        let std_fiedler = std_dev(
            healthy_data.iter().map(|m| m.fiedler_value),
            mean_fiedler,
        );

        self.healthy_std = TopologyMetrics {
            global_mincut: std_mincut,
            modularity: std_mod,
            global_efficiency: std_eff,
            local_efficiency: std_loc,
            graph_entropy: std_ent,
            fiedler_value: std_fiedler,
            num_modules: 0,
            timestamp: 0.0,
        };
    }

    /// Composite deviation score (mean absolute z-score across all metrics).
    ///
    /// Higher values indicate greater deviation from healthy baseline.
    pub fn deviation_score(&self, current: &TopologyMetrics) -> f64 {
        let z_scores = self.z_scores(current);
        z_scores.iter().map(|z| z.abs()).sum::<f64>() / z_scores.len() as f64
    }

    /// Alzheimer's disease risk score in `[0, 1]`.
    ///
    /// Based on characteristic patterns: reduced global efficiency,
    /// increased modularity (network fragmentation), reduced mincut.
    pub fn alzheimer_risk(&self, current: &TopologyMetrics) -> f64 {
        let z = self.z_scores(current);
        // z[0]=mincut, z[1]=modularity, z[2]=global_eff, z[3]=local_eff, z[4]=entropy, z[5]=fiedler

        // Alzheimer's: decreased efficiency (negative z), decreased mincut (negative z),
        // increased modularity (positive z = fragmentation).
        let efficiency_component = sigmoid(-z[2], 2.0);
        let mincut_component = sigmoid(-z[0], 2.0);
        let modularity_component = sigmoid(z[1], 2.0);
        let fiedler_component = sigmoid(-z[5], 1.5);

        let risk = 0.35 * efficiency_component
            + 0.25 * mincut_component
            + 0.25 * modularity_component
            + 0.15 * fiedler_component;

        risk.clamp(0.0, 1.0)
    }

    /// Epilepsy risk score in `[0, 1]`.
    ///
    /// Based on characteristic patterns: hypersynchrony (increased mincut),
    /// decreased modularity, increased local efficiency.
    pub fn epilepsy_risk(&self, current: &TopologyMetrics) -> f64 {
        let z = self.z_scores(current);

        // Epilepsy: increased mincut (hypersynchrony), decreased modularity,
        // increased local efficiency.
        let mincut_component = sigmoid(z[0], 2.0);
        let modularity_component = sigmoid(-z[1], 2.0);
        let local_eff_component = sigmoid(z[3], 2.0);

        let risk = 0.4 * mincut_component
            + 0.3 * modularity_component
            + 0.3 * local_eff_component;

        risk.clamp(0.0, 1.0)
    }

    /// Depression risk score in `[0, 1]`.
    ///
    /// Based on characteristic patterns: reduced global efficiency,
    /// altered entropy, reduced Fiedler value (weaker connectivity).
    pub fn depression_risk(&self, current: &TopologyMetrics) -> f64 {
        let z = self.z_scores(current);

        // Depression: decreased efficiency, decreased Fiedler value,
        // altered entropy (can go either way, use absolute deviation).
        let efficiency_component = sigmoid(-z[2], 2.0);
        let fiedler_component = sigmoid(-z[5], 2.0);
        let entropy_component = sigmoid(z[4].abs(), 1.5);

        let risk = 0.4 * efficiency_component
            + 0.35 * fiedler_component
            + 0.25 * entropy_component;

        risk.clamp(0.0, 1.0)
    }

    /// General brain health index in `[0, 1]`.
    ///
    /// `0.0` = severe abnormality, `1.0` = perfectly healthy (all metrics
    /// within normal range).
    pub fn brain_health_index(&self, current: &TopologyMetrics) -> f64 {
        let deviation = self.deviation_score(current);
        // Map deviation to health: 0 deviation = 1.0 health, large deviation = ~0.0.
        let health = (-0.5 * deviation).exp();
        health.clamp(0.0, 1.0)
    }

    /// Compute z-scores for all topology metrics.
    ///
    /// Order: [mincut, modularity, global_efficiency, local_efficiency, entropy, fiedler].
    fn z_scores(&self, current: &TopologyMetrics) -> [f64; 6] {
        [
            z_score(
                current.global_mincut,
                self.healthy_baseline.global_mincut,
                self.healthy_std.global_mincut,
            ),
            z_score(
                current.modularity,
                self.healthy_baseline.modularity,
                self.healthy_std.modularity,
            ),
            z_score(
                current.global_efficiency,
                self.healthy_baseline.global_efficiency,
                self.healthy_std.global_efficiency,
            ),
            z_score(
                current.local_efficiency,
                self.healthy_baseline.local_efficiency,
                self.healthy_std.local_efficiency,
            ),
            z_score(
                current.graph_entropy,
                self.healthy_baseline.graph_entropy,
                self.healthy_std.graph_entropy,
            ),
            z_score(
                current.fiedler_value,
                self.healthy_baseline.fiedler_value,
                self.healthy_std.fiedler_value,
            ),
        ]
    }
}

/// Compute the z-score: (value - mean) / std.
///
/// Returns 0.0 if std is near zero.
fn z_score(value: f64, mean: f64, std: f64) -> f64 {
    if std.abs() < 1e-10 {
        return 0.0;
    }
    (value - mean) / std
}

/// Standard deviation from an iterator of values and a precomputed mean.
fn std_dev(values: impl Iterator<Item = f64>, mean: f64) -> f64 {
    let vals: Vec<f64> = values.collect();
    if vals.len() < 2 {
        return 1.0; // Default to 1.0 to avoid division by zero.
    }
    let n = vals.len() as f64;
    let variance = vals.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0);
    let s = variance.sqrt();
    if s < 1e-10 { 1.0 } else { s }
}

/// Sigmoid function mapping a z-score to `[0, 1]`.
///
/// `scale` controls the steepness of the transition.
fn sigmoid(z: f64, scale: f64) -> f64 {
    1.0 / (1.0 + (-scale * z).exp())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_metrics(
        mincut: f64,
        modularity: f64,
        efficiency: f64,
        entropy: f64,
    ) -> TopologyMetrics {
        TopologyMetrics {
            global_mincut: mincut,
            modularity,
            global_efficiency: efficiency,
            local_efficiency: 0.3,
            graph_entropy: entropy,
            fiedler_value: 0.5,
            num_modules: 4,
            timestamp: 0.0,
        }
    }

    fn make_baseline_scorer() -> ClinicalScorer {
        ClinicalScorer::new(
            make_metrics(5.0, 0.4, 0.3, 2.0),
            make_metrics(1.0, 0.1, 0.05, 0.3),
        )
    }

    #[test]
    fn test_healthy_deviation_near_zero() {
        let scorer = make_baseline_scorer();
        let healthy = make_metrics(5.0, 0.4, 0.3, 2.0);
        let deviation = scorer.deviation_score(&healthy);
        assert!(
            deviation < 0.5,
            "Healthy metrics should have low deviation, got {}",
            deviation
        );
    }

    #[test]
    fn test_abnormal_deviation_high() {
        let scorer = make_baseline_scorer();
        let abnormal = make_metrics(15.0, 1.5, 0.9, 8.0);
        let deviation = scorer.deviation_score(&abnormal);
        assert!(
            deviation > 2.0,
            "Abnormal metrics should have high deviation, got {}",
            deviation
        );
    }

    #[test]
    fn test_brain_health_healthy() {
        let scorer = make_baseline_scorer();
        let healthy = make_metrics(5.0, 0.4, 0.3, 2.0);
        let health = scorer.brain_health_index(&healthy);
        assert!(
            health > 0.8,
            "Healthy metrics should yield high health index, got {}",
            health
        );
    }

    #[test]
    fn test_brain_health_abnormal() {
        let scorer = make_baseline_scorer();
        let abnormal = make_metrics(15.0, 1.5, 0.9, 8.0);
        let health = scorer.brain_health_index(&abnormal);
        assert!(
            health < 0.5,
            "Abnormal metrics should yield low health index, got {}",
            health
        );
    }

    #[test]
    fn test_disease_risks_in_range() {
        let scorer = make_baseline_scorer();
        let current = make_metrics(3.0, 0.6, 0.15, 2.5);

        let alz = scorer.alzheimer_risk(&current);
        let epi = scorer.epilepsy_risk(&current);
        let dep = scorer.depression_risk(&current);

        assert!(alz >= 0.0 && alz <= 1.0, "Alzheimer risk out of range: {}", alz);
        assert!(epi >= 0.0 && epi <= 1.0, "Epilepsy risk out of range: {}", epi);
        assert!(dep >= 0.0 && dep <= 1.0, "Depression risk out of range: {}", dep);
    }

    #[test]
    fn test_learn_baseline() {
        let mut scorer = ClinicalScorer::new(
            make_metrics(0.0, 0.0, 0.0, 0.0),
            make_metrics(1.0, 1.0, 1.0, 1.0),
        );

        let data = vec![
            make_metrics(5.0, 0.4, 0.3, 2.0),
            make_metrics(5.2, 0.42, 0.31, 2.1),
            make_metrics(4.8, 0.38, 0.29, 1.9),
        ];
        scorer.learn_baseline(&data);

        // After learning, healthy data should have low deviation.
        let deviation = scorer.deviation_score(&make_metrics(5.0, 0.4, 0.3, 2.0));
        assert!(deviation < 1.0, "Post-learning deviation too high: {}", deviation);
    }

    #[test]
    fn test_health_index_range() {
        let scorer = make_baseline_scorer();
        // Test extreme values.
        for mincut in [0.0, 5.0, 20.0] {
            for mod_val in [0.0, 0.4, 1.0] {
                let m = make_metrics(mincut, mod_val, 0.3, 2.0);
                let h = scorer.brain_health_index(&m);
                assert!(h >= 0.0 && h <= 1.0, "Health index out of range: {}", h);
            }
        }
    }
}
