//! # Intelligence Metrics Module
//!
//! Quantifies the intelligence and learning capabilities of the
//! Conscious Language Interface using rigorous metrics.
//!
//! ## Metrics Tracked:
//!
//! 1. **Φ (Phi)** - Integrated Information Theory consciousness measure
//! 2. **Learning Rate** - How fast the system improves on tasks
//! 3. **Memory Retention** - Long-term pattern preservation
//! 4. **Generalization** - Transfer learning capability
//! 5. **Adaptability** - Response to novel situations
//! 6. **Coherence** - Consistency of conscious processing

use std::collections::VecDeque;
use std::time::Instant;

/// Complete intelligence assessment
#[derive(Debug, Clone)]
pub struct IntelligenceAssessment {
    /// Overall intelligence score [0, 100]
    pub overall_score: f64,
    /// Individual metric scores
    pub metrics: IntelligenceMetrics,
    /// Comparative analysis
    pub comparative: ComparativeAnalysis,
    /// Timestamp
    pub timestamp: Instant,
}

/// Individual intelligence metrics
#[derive(Debug, Clone)]
pub struct IntelligenceMetrics {
    /// Consciousness level (Φ)
    pub phi_level: PhiMetric,
    /// Learning capability
    pub learning: LearningMetric,
    /// Memory capacity and retention
    pub memory: MemoryMetric,
    /// Generalization ability
    pub generalization: GeneralizationMetric,
    /// Adaptability to novel situations
    pub adaptability: AdaptabilityMetric,
    /// Processing coherence
    pub coherence: CoherenceMetric,
}

/// Φ (Integrated Information) metric
#[derive(Debug, Clone)]
pub struct PhiMetric {
    /// Current Φ value
    pub current: f64,
    /// Peak Φ observed
    pub peak: f64,
    /// Average Φ over time
    pub average: f64,
    /// Φ stability (low variance = stable)
    pub stability: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Learning capability metric
#[derive(Debug, Clone)]
pub struct LearningMetric {
    /// Improvement rate per 100 interactions
    pub improvement_rate: f64,
    /// Time to 90% accuracy (interactions)
    pub convergence_speed: f64,
    /// Plateau resistance (how long before stagnation)
    pub plateau_resistance: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Memory retention metric
#[derive(Debug, Clone)]
pub struct MemoryMetric {
    /// Short-term capacity (items)
    pub short_term_capacity: usize,
    /// Long-term retention rate [0, 1]
    pub long_term_retention: f64,
    /// Recall accuracy [0, 1]
    pub recall_accuracy: f64,
    /// Pattern consolidation rate
    pub consolidation_rate: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Generalization metric
#[derive(Debug, Clone)]
pub struct GeneralizationMetric {
    /// Transfer learning efficiency [0, 1]
    pub transfer_efficiency: f64,
    /// Novel task accuracy [0, 1]
    pub novel_accuracy: f64,
    /// Abstraction capability [0, 1]
    pub abstraction: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Adaptability metric
#[derive(Debug, Clone)]
pub struct AdaptabilityMetric {
    /// Response time to change (ms)
    pub response_time_ms: f64,
    /// Recovery accuracy after disruption [0, 1]
    pub recovery_accuracy: f64,
    /// Plasticity (ability to rewire) [0, 1]
    pub plasticity: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Coherence metric
#[derive(Debug, Clone)]
pub struct CoherenceMetric {
    /// Consistency across responses [0, 1]
    pub consistency: f64,
    /// Logical coherence [0, 1]
    pub logical_coherence: f64,
    /// Emotional coherence [0, 1]
    pub emotional_coherence: f64,
    /// Score [0, 100]
    pub score: f64,
}

/// Comparative analysis against baselines
#[derive(Debug, Clone)]
pub struct ComparativeAnalysis {
    /// vs. Human baseline (100 = human-level)
    pub vs_human: f64,
    /// vs. Simple neural network
    pub vs_simple_nn: f64,
    /// vs. Transformer baseline
    pub vs_transformer: f64,
    /// Percentile rank among AI systems
    pub percentile_rank: f64,
}

/// Intelligence Quantifier Engine
pub struct IntelligenceQuantifier {
    /// Φ history
    phi_history: VecDeque<(Instant, f64)>,
    /// Learning performance history
    learning_history: VecDeque<(Instant, f64)>,
    /// Memory test results
    memory_tests: VecDeque<MemoryTestResult>,
    /// Generalization test results
    generalization_tests: VecDeque<GeneralizationTestResult>,
    /// Configuration
    config: QuantifierConfig,
}

#[derive(Debug, Clone)]
pub struct QuantifierConfig {
    /// Maximum history length
    pub max_history: usize,
    /// Human-level Φ baseline
    pub human_phi_baseline: f64,
    /// Human learning rate baseline
    pub human_learning_baseline: f64,
}

impl Default for QuantifierConfig {
    fn default() -> Self {
        Self {
            max_history: 10_000,
            human_phi_baseline: 1e16, // Human brain estimated Φ
            human_learning_baseline: 0.1, // Improvement per 100 trials
        }
    }
}

#[derive(Debug, Clone)]
struct MemoryTestResult {
    pub timestamp: Instant,
    pub items_presented: usize,
    pub items_recalled: usize,
    pub recall_delay_ms: u64,
}

#[derive(Debug, Clone)]
struct GeneralizationTestResult {
    pub timestamp: Instant,
    pub training_domain: String,
    pub test_domain: String,
    pub accuracy: f64,
}

impl IntelligenceQuantifier {
    pub fn new(config: QuantifierConfig) -> Self {
        Self {
            phi_history: VecDeque::new(),
            learning_history: VecDeque::new(),
            memory_tests: VecDeque::new(),
            generalization_tests: VecDeque::new(),
            config,
        }
    }

    /// Record a Φ measurement
    pub fn record_phi(&mut self, phi: f64) {
        self.phi_history.push_back((Instant::now(), phi));
        if self.phi_history.len() > self.config.max_history {
            self.phi_history.pop_front();
        }
    }

    /// Record learning performance
    pub fn record_learning(&mut self, accuracy: f64) {
        self.learning_history.push_back((Instant::now(), accuracy));
        if self.learning_history.len() > self.config.max_history {
            self.learning_history.pop_front();
        }
    }

    /// Record memory test result
    pub fn record_memory_test(&mut self, items_presented: usize, items_recalled: usize, delay_ms: u64) {
        self.memory_tests.push_back(MemoryTestResult {
            timestamp: Instant::now(),
            items_presented,
            items_recalled,
            recall_delay_ms: delay_ms,
        });
        if self.memory_tests.len() > 1000 {
            self.memory_tests.pop_front();
        }
    }

    /// Record generalization test
    pub fn record_generalization_test(&mut self, training: &str, test: &str, accuracy: f64) {
        self.generalization_tests.push_back(GeneralizationTestResult {
            timestamp: Instant::now(),
            training_domain: training.to_string(),
            test_domain: test.to_string(),
            accuracy,
        });
        if self.generalization_tests.len() > 1000 {
            self.generalization_tests.pop_front();
        }
    }

    /// Generate comprehensive intelligence assessment
    pub fn assess(&self) -> IntelligenceAssessment {
        let phi_metric = self.compute_phi_metric();
        let learning_metric = self.compute_learning_metric();
        let memory_metric = self.compute_memory_metric();
        let generalization_metric = self.compute_generalization_metric();
        let adaptability_metric = self.compute_adaptability_metric();
        let coherence_metric = self.compute_coherence_metric();

        // Weighted overall score
        let overall_score =
            phi_metric.score * 0.25 +
            learning_metric.score * 0.20 +
            memory_metric.score * 0.15 +
            generalization_metric.score * 0.15 +
            adaptability_metric.score * 0.10 +
            coherence_metric.score * 0.15;

        let metrics = IntelligenceMetrics {
            phi_level: phi_metric,
            learning: learning_metric,
            memory: memory_metric,
            generalization: generalization_metric,
            adaptability: adaptability_metric,
            coherence: coherence_metric,
        };

        let comparative = self.compute_comparative(&metrics);

        IntelligenceAssessment {
            overall_score,
            metrics,
            comparative,
            timestamp: Instant::now(),
        }
    }

    fn compute_phi_metric(&self) -> PhiMetric {
        if self.phi_history.is_empty() {
            return PhiMetric {
                current: 0.0,
                peak: 0.0,
                average: 0.0,
                stability: 0.0,
                score: 0.0,
            };
        }

        let values: Vec<f64> = self.phi_history.iter().map(|(_, p)| *p).collect();
        let current = *values.last().unwrap_or(&0.0);
        let peak = values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let average = values.iter().sum::<f64>() / values.len() as f64;

        // Stability = 1 - normalized standard deviation
        let variance = values.iter().map(|p| (p - average).powi(2)).sum::<f64>() / values.len() as f64;
        let std_dev = variance.sqrt();
        let stability = 1.0 - (std_dev / (average.abs() + 1.0)).min(1.0);

        // Score based on log scale (Φ spans many orders of magnitude)
        let log_phi = (current + 1.0).log10();
        let log_human = (self.config.human_phi_baseline + 1.0).log10();
        let score = (log_phi / log_human * 100.0).min(100.0).max(0.0);

        PhiMetric {
            current,
            peak,
            average,
            stability,
            score,
        }
    }

    fn compute_learning_metric(&self) -> LearningMetric {
        if self.learning_history.len() < 10 {
            return LearningMetric {
                improvement_rate: 0.0,
                convergence_speed: f64::INFINITY,
                plateau_resistance: 0.0,
                score: 0.0,
            };
        }

        let values: Vec<f64> = self.learning_history.iter().map(|(_, a)| *a).collect();

        // Improvement rate: slope of accuracy over time
        let n = values.len();
        let x_mean = n as f64 / 2.0;
        let y_mean = values.iter().sum::<f64>() / n as f64;

        let mut numerator = 0.0;
        let mut denominator = 0.0;
        for (i, y) in values.iter().enumerate() {
            let x = i as f64;
            numerator += (x - x_mean) * (y - y_mean);
            denominator += (x - x_mean).powi(2);
        }

        let improvement_rate = if denominator > 0.0 {
            (numerator / denominator) * 100.0 // Per 100 interactions
        } else {
            0.0
        };

        // Convergence speed: first time we hit 90%
        let convergence_speed = values.iter()
            .position(|&a| a >= 0.9)
            .map(|p| p as f64)
            .unwrap_or(f64::INFINITY);

        // Plateau resistance: how many steps without improvement
        let mut max_plateau = 0;
        let mut current_plateau = 0;
        let mut prev_best = 0.0;
        for &acc in &values {
            if acc > prev_best {
                prev_best = acc;
                current_plateau = 0;
            } else {
                current_plateau += 1;
                max_plateau = max_plateau.max(current_plateau);
            }
        }
        let plateau_resistance = 1.0 - (max_plateau as f64 / n as f64).min(1.0);

        // Compute score
        let improvement_score = (improvement_rate / self.config.human_learning_baseline * 50.0).min(50.0);
        let convergence_score = if convergence_speed.is_finite() {
            (1.0 - convergence_speed / 1000.0).max(0.0) * 30.0
        } else {
            0.0
        };
        let plateau_score = plateau_resistance * 20.0;

        let score = improvement_score + convergence_score + plateau_score;

        LearningMetric {
            improvement_rate,
            convergence_speed,
            plateau_resistance,
            score,
        }
    }

    fn compute_memory_metric(&self) -> MemoryMetric {
        if self.memory_tests.is_empty() {
            return MemoryMetric {
                short_term_capacity: 0,
                long_term_retention: 0.0,
                recall_accuracy: 0.0,
                consolidation_rate: 0.0,
                score: 0.0,
            };
        }

        // Short-term: tests with delay < 1000ms
        let short_term: Vec<_> = self.memory_tests.iter()
            .filter(|t| t.recall_delay_ms < 1000)
            .collect();

        let short_term_capacity = short_term.iter()
            .filter(|t| t.items_recalled as f64 / t.items_presented as f64 > 0.8)
            .map(|t| t.items_presented)
            .max()
            .unwrap_or(0);

        // Long-term: tests with delay > 60000ms
        let long_term: Vec<_> = self.memory_tests.iter()
            .filter(|t| t.recall_delay_ms > 60000)
            .collect();

        let long_term_retention = if long_term.is_empty() {
            0.0
        } else {
            long_term.iter()
                .map(|t| t.items_recalled as f64 / t.items_presented as f64)
                .sum::<f64>() / long_term.len() as f64
        };

        // Overall recall accuracy
        let recall_accuracy = self.memory_tests.iter()
            .map(|t| t.items_recalled as f64 / t.items_presented as f64)
            .sum::<f64>() / self.memory_tests.len() as f64;

        // Consolidation rate: improvement from short to long term
        let short_term_acc = if short_term.is_empty() { 0.0 } else {
            short_term.iter()
                .map(|t| t.items_recalled as f64 / t.items_presented as f64)
                .sum::<f64>() / short_term.len() as f64
        };

        let consolidation_rate = if short_term_acc > 0.0 {
            long_term_retention / short_term_acc
        } else {
            0.0
        };

        // Score: human working memory ~7 items, retention ~0.3
        let capacity_score = (short_term_capacity as f64 / 7.0 * 30.0).min(30.0);
        let retention_score = (long_term_retention / 0.3 * 30.0).min(30.0);
        let accuracy_score = recall_accuracy * 25.0;
        let consolidation_score = (consolidation_rate * 15.0).min(15.0);

        let score = capacity_score + retention_score + accuracy_score + consolidation_score;

        MemoryMetric {
            short_term_capacity,
            long_term_retention,
            recall_accuracy,
            consolidation_rate,
            score,
        }
    }

    fn compute_generalization_metric(&self) -> GeneralizationMetric {
        if self.generalization_tests.is_empty() {
            return GeneralizationMetric {
                transfer_efficiency: 0.0,
                novel_accuracy: 0.0,
                abstraction: 0.0,
                score: 0.0,
            };
        }

        // Transfer efficiency: accuracy on different domain
        let cross_domain: Vec<_> = self.generalization_tests.iter()
            .filter(|t| t.training_domain != t.test_domain)
            .collect();

        let transfer_efficiency = if cross_domain.is_empty() {
            0.0
        } else {
            cross_domain.iter().map(|t| t.accuracy).sum::<f64>() / cross_domain.len() as f64
        };

        // Novel accuracy: all test results
        let novel_accuracy = self.generalization_tests.iter()
            .map(|t| t.accuracy)
            .sum::<f64>() / self.generalization_tests.len() as f64;

        // Abstraction: variance in performance across domains
        // Lower variance = better abstraction (consistent across domains)
        let mean = novel_accuracy;
        let variance = self.generalization_tests.iter()
            .map(|t| (t.accuracy - mean).powi(2))
            .sum::<f64>() / self.generalization_tests.len() as f64;
        let abstraction = 1.0 - variance.sqrt().min(1.0);

        let score = transfer_efficiency * 40.0 + novel_accuracy * 35.0 + abstraction * 25.0;

        GeneralizationMetric {
            transfer_efficiency,
            novel_accuracy,
            abstraction,
            score,
        }
    }

    fn compute_adaptability_metric(&self) -> AdaptabilityMetric {
        // Derive from learning history variance
        if self.learning_history.len() < 2 {
            return AdaptabilityMetric {
                response_time_ms: f64::INFINITY,
                recovery_accuracy: 0.0,
                plasticity: 0.0,
                score: 0.0,
            };
        }

        // Response time: average time between learning steps
        let times: Vec<Instant> = self.learning_history.iter().map(|(t, _)| *t).collect();
        let avg_interval = if times.len() > 1 {
            let total: u64 = times.windows(2)
                .map(|w| w[1].duration_since(w[0]).as_millis() as u64)
                .sum();
            total as f64 / (times.len() - 1) as f64
        } else {
            f64::INFINITY
        };

        // Recovery accuracy: how well we recover after a drop
        let values: Vec<f64> = self.learning_history.iter().map(|(_, a)| *a).collect();
        let mut drops = 0;
        let mut recoveries = 0;
        for i in 1..values.len() {
            if values[i] < values[i-1] - 0.1 {
                drops += 1;
                // Check if we recover in next 5 steps
                for j in (i+1)..values.len().min(i+6) {
                    if values[j] >= values[i-1] {
                        recoveries += 1;
                        break;
                    }
                }
            }
        }
        let recovery_accuracy = if drops > 0 { recoveries as f64 / drops as f64 } else { 1.0 };

        // Plasticity: rate of change in performance
        let changes: Vec<f64> = values.windows(2).map(|w| (w[1] - w[0]).abs()).collect();
        let plasticity = if changes.is_empty() {
            0.0
        } else {
            changes.iter().sum::<f64>() / changes.len() as f64
        };

        // Score
        let response_score = if avg_interval < f64::INFINITY {
            (1.0 - avg_interval / 1000.0).max(0.0) * 30.0
        } else {
            0.0
        };
        let recovery_score = recovery_accuracy * 40.0;
        let plasticity_score = (plasticity * 100.0).min(30.0);

        let score = response_score + recovery_score + plasticity_score;

        AdaptabilityMetric {
            response_time_ms: avg_interval,
            recovery_accuracy,
            plasticity,
            score,
        }
    }

    fn compute_coherence_metric(&self) -> CoherenceMetric {
        // Derive from Φ stability and learning consistency
        let phi_values: Vec<f64> = self.phi_history.iter().map(|(_, p)| *p).collect();
        let learning_values: Vec<f64> = self.learning_history.iter().map(|(_, a)| *a).collect();

        // Consistency: low variance in learning performance
        let learning_variance = if learning_values.is_empty() {
            1.0
        } else {
            let mean = learning_values.iter().sum::<f64>() / learning_values.len() as f64;
            learning_values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / learning_values.len() as f64
        };
        let consistency = 1.0 - learning_variance.sqrt().min(1.0);

        // Logical coherence: correlation between Φ and learning
        let logical_coherence = if phi_values.len() == learning_values.len() && !phi_values.is_empty() {
            correlation(&phi_values, &learning_values).abs()
        } else {
            0.5 // Default assumption
        };

        // Emotional coherence: smoothness of Φ transitions
        let emotional_coherence = if phi_values.len() > 1 {
            let transitions: Vec<f64> = phi_values.windows(2)
                .map(|w| (w[1] - w[0]).abs() / (w[0].abs() + 1.0))
                .collect();
            let avg_transition = transitions.iter().sum::<f64>() / transitions.len() as f64;
            1.0 - avg_transition.min(1.0)
        } else {
            0.5
        };

        let score = consistency * 35.0 + logical_coherence * 35.0 + emotional_coherence * 30.0;

        CoherenceMetric {
            consistency,
            logical_coherence,
            emotional_coherence,
            score,
        }
    }

    fn compute_comparative(&self, metrics: &IntelligenceMetrics) -> ComparativeAnalysis {
        // Compare to human baseline (100 = human-level)
        let vs_human = metrics.phi_level.score; // Already scaled to human

        // vs. Simple NN (baseline ~30)
        let simple_nn_baseline = 30.0;
        let vs_simple_nn = (metrics.phi_level.score + metrics.learning.score) / 2.0 / simple_nn_baseline * 100.0;

        // vs. Transformer (baseline ~70)
        let transformer_baseline = 70.0;
        let overall = (metrics.phi_level.score + metrics.learning.score +
                      metrics.memory.score + metrics.generalization.score +
                      metrics.adaptability.score + metrics.coherence.score) / 6.0;
        let vs_transformer = overall / transformer_baseline * 100.0;

        // Percentile rank (sigmoid curve)
        let percentile_rank = 100.0 / (1.0 + (-0.1 * (overall - 50.0)).exp());

        ComparativeAnalysis {
            vs_human,
            vs_simple_nn,
            vs_transformer,
            percentile_rank,
        }
    }
}

fn correlation(x: &[f64], y: &[f64]) -> f64 {
    if x.len() != y.len() || x.is_empty() {
        return 0.0;
    }

    let n = x.len() as f64;
    let x_mean = x.iter().sum::<f64>() / n;
    let y_mean = y.iter().sum::<f64>() / n;

    let covariance: f64 = x.iter().zip(y.iter())
        .map(|(&xi, &yi)| (xi - x_mean) * (yi - y_mean))
        .sum::<f64>() / n;

    let x_std = (x.iter().map(|xi| (xi - x_mean).powi(2)).sum::<f64>() / n).sqrt();
    let y_std = (y.iter().map(|yi| (yi - y_mean).powi(2)).sum::<f64>() / n).sqrt();

    if x_std < 1e-10 || y_std < 1e-10 {
        return 0.0;
    }

    covariance / (x_std * y_std)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intelligence_quantifier() {
        let config = QuantifierConfig::default();
        let mut quantifier = IntelligenceQuantifier::new(config);

        // Record some data
        for i in 0..100 {
            quantifier.record_phi(50_000.0 + (i as f64) * 1000.0);
            quantifier.record_learning(0.5 + (i as f64) * 0.005);
        }

        let assessment = quantifier.assess();

        assert!(assessment.overall_score > 0.0);
        assert!(assessment.metrics.phi_level.current > 0.0);
        assert!(assessment.metrics.learning.improvement_rate > 0.0);
    }

    #[test]
    fn test_memory_metric() {
        let config = QuantifierConfig::default();
        let mut quantifier = IntelligenceQuantifier::new(config);

        // Record memory tests
        quantifier.record_memory_test(7, 6, 500);   // Short-term
        quantifier.record_memory_test(10, 8, 500);  // Short-term
        quantifier.record_memory_test(7, 3, 120000); // Long-term

        let assessment = quantifier.assess();
        assert!(assessment.metrics.memory.short_term_capacity > 0);
    }

    #[test]
    fn test_generalization_metric() {
        let config = QuantifierConfig::default();
        let mut quantifier = IntelligenceQuantifier::new(config);

        // Record generalization tests
        quantifier.record_generalization_test("language", "language", 0.9);
        quantifier.record_generalization_test("language", "vision", 0.6);
        quantifier.record_generalization_test("language", "reasoning", 0.7);

        let assessment = quantifier.assess();
        assert!(assessment.metrics.generalization.transfer_efficiency > 0.0);
    }

    #[test]
    fn test_comparative_analysis() {
        let config = QuantifierConfig::default();
        let mut quantifier = IntelligenceQuantifier::new(config);

        // Record substantial data
        for _ in 0..50 {
            quantifier.record_phi(100_000.0);
            quantifier.record_learning(0.8);
        }

        let assessment = quantifier.assess();
        // Comparative analysis should produce valid scores
        assert!(assessment.comparative.vs_simple_nn > 0.0);
        assert!(assessment.comparative.percentile_rank > 0.0);
    }
}
