//! Intelligence Metrics Module
//!
//! Measures cognitive capabilities, reasoning quality, and learning indicators
//! for agent evaluation based on established AI benchmarking methodologies.
//!
//! Key metrics tracked:
//! - Reasoning quality (logical coherence, constraint satisfaction)
//! - Learning efficiency (regret curves, sample efficiency)
//! - Working memory (context utilization, information integration)
//! - Tool use proficiency (appropriate selection, effective utilization)
//! - Meta-cognitive awareness (self-correction, uncertainty estimation)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Intelligence assessment result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IntelligenceAssessment {
    /// Overall intelligence score (0-100)
    pub overall_score: f64,
    /// Individual capability scores
    pub capabilities: CapabilityScores,
    /// Reasoning quality metrics
    pub reasoning: ReasoningMetrics,
    /// Learning efficiency metrics
    pub learning: LearningMetrics,
    /// Tool use proficiency
    pub tool_use: ToolUseMetrics,
    /// Meta-cognitive indicators
    pub meta_cognition: MetaCognitiveMetrics,
    /// Cost efficiency metrics
    pub cost: CostMetrics,
    /// Robustness under noise
    pub robustness: RobustnessMetrics,
    /// Raw performance data
    pub raw_data: RawMetrics,
}

/// Capability scores across dimensions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CapabilityScores {
    /// Temporal reasoning (date inference, calendar math)
    pub temporal_reasoning: f64,
    /// Constraint satisfaction (multi-constraint solving)
    pub constraint_satisfaction: f64,
    /// Information retrieval (semantic search, recall)
    pub information_retrieval: f64,
    /// Pattern recognition (learning from examples)
    pub pattern_recognition: f64,
    /// Planning and sequencing
    pub planning: f64,
    /// Error recovery and adaptation
    pub adaptation: f64,
}

impl Default for CapabilityScores {
    fn default() -> Self {
        Self {
            temporal_reasoning: 0.0,
            constraint_satisfaction: 0.0,
            information_retrieval: 0.0,
            pattern_recognition: 0.0,
            planning: 0.0,
            adaptation: 0.0,
        }
    }
}

impl CapabilityScores {
    /// Compute weighted average
    pub fn weighted_average(&self, weights: &[f64; 6]) -> f64 {
        let scores = [
            self.temporal_reasoning,
            self.constraint_satisfaction,
            self.information_retrieval,
            self.pattern_recognition,
            self.planning,
            self.adaptation,
        ];
        let total_weight: f64 = weights.iter().sum();
        if total_weight == 0.0 {
            return 0.0;
        }
        scores
            .iter()
            .zip(weights.iter())
            .map(|(s, w)| s * w)
            .sum::<f64>()
            / total_weight
    }
}

/// Reasoning quality metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReasoningMetrics {
    /// Logical coherence (steps follow logically)
    pub logical_coherence: f64,
    /// Constraint satisfaction rate
    pub constraint_satisfaction_rate: f64,
    /// Solution optimality (vs. best possible)
    pub solution_optimality: f64,
    /// Reasoning efficiency (steps to solution)
    pub reasoning_efficiency: f64,
    /// Error rate in logical steps
    pub error_rate: f64,
}

impl Default for ReasoningMetrics {
    fn default() -> Self {
        Self {
            logical_coherence: 0.0,
            constraint_satisfaction_rate: 0.0,
            solution_optimality: 0.0,
            reasoning_efficiency: 0.0,
            error_rate: 0.0,
        }
    }
}

/// Learning efficiency metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LearningMetrics {
    /// Sample efficiency (performance vs. examples seen)
    pub sample_efficiency: f64,
    /// Regret trajectory (sublinear indicator)
    pub regret_sublinearity: f64,
    /// Transfer learning capability
    pub transfer_capability: f64,
    /// Learning rate (improvement per episode)
    pub learning_rate: f64,
    /// Generalization ability
    pub generalization: f64,
}

impl Default for LearningMetrics {
    fn default() -> Self {
        Self {
            sample_efficiency: 0.0,
            regret_sublinearity: 0.0,
            transfer_capability: 0.0,
            learning_rate: 0.0,
            generalization: 0.0,
        }
    }
}

/// Tool use proficiency metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolUseMetrics {
    /// Tool selection appropriateness
    pub selection_appropriateness: f64,
    /// Tool utilization effectiveness
    pub utilization_effectiveness: f64,
    /// Tool composition (combining tools)
    pub composition_ability: f64,
    /// Tool discovery (finding needed tools)
    pub discovery_ability: f64,
}

impl Default for ToolUseMetrics {
    fn default() -> Self {
        Self {
            selection_appropriateness: 0.0,
            utilization_effectiveness: 0.0,
            composition_ability: 0.0,
            discovery_ability: 0.0,
        }
    }
}

/// Meta-cognitive metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaCognitiveMetrics {
    /// Self-correction rate
    pub self_correction_rate: f64,
    /// Uncertainty calibration (confidence vs. accuracy)
    pub uncertainty_calibration: f64,
    /// Strategy adaptation
    pub strategy_adaptation: f64,
    /// Progress monitoring accuracy
    pub progress_monitoring: f64,
}

impl Default for MetaCognitiveMetrics {
    fn default() -> Self {
        Self {
            self_correction_rate: 0.0,
            uncertainty_calibration: 0.0,
            strategy_adaptation: 0.0,
            progress_monitoring: 0.0,
        }
    }
}

/// Cost efficiency metrics — first-class IQ dimension
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CostMetrics {
    /// Steps per correct solve (lower = better)
    pub steps_per_solve: f64,
    /// Tool calls per correct solve (lower = better)
    pub tools_per_solve: f64,
    /// Cost efficiency score (0-1, higher = cheaper)
    pub cost_efficiency: f64,
    /// Cost trend over episodes (positive = improving)
    pub cost_trend: f64,
}

impl Default for CostMetrics {
    fn default() -> Self {
        Self {
            steps_per_solve: 100.0,
            tools_per_solve: 10.0,
            cost_efficiency: 0.0,
            cost_trend: 0.0,
        }
    }
}

/// Robustness under adversarial conditions — first-class IQ dimension
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RobustnessMetrics {
    /// Accuracy on noise-injected tasks
    pub noise_accuracy: f64,
    /// Accuracy drop from clean to noisy (lower = more robust)
    pub noise_degradation: f64,
    /// Per-episode accuracy consistency (higher = steadier)
    pub consistency: f64,
    /// Composite robustness score (0-1)
    pub robustness_score: f64,
}

impl Default for RobustnessMetrics {
    fn default() -> Self {
        Self {
            noise_accuracy: 0.0,
            noise_degradation: 1.0,
            consistency: 0.0,
            robustness_score: 0.0,
        }
    }
}

/// Raw metrics from benchmarks
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawMetrics {
    /// Total tasks attempted
    pub tasks_attempted: usize,
    /// Tasks completed successfully
    pub tasks_completed: usize,
    /// Tasks with correct solutions
    pub tasks_correct: usize,
    /// Total steps taken
    pub total_steps: usize,
    /// Total tool calls
    pub total_tool_calls: usize,
    /// Total latency in ms
    pub total_latency_ms: u64,
    /// Performance by difficulty
    pub by_difficulty: HashMap<u8, DifficultyStats>,
    /// Episode-level metrics
    pub episodes: Vec<EpisodeMetrics>,
    /// Tasks attempted under noise injection
    pub noise_tasks_attempted: usize,
    /// Tasks correct under noise injection
    pub noise_tasks_correct: usize,
    /// Policy violations (contradictions, budget overruns)
    pub policy_violations: usize,
    /// Solved-but-incorrect count (contradiction rate numerator)
    pub contradictions: usize,
    /// Successful rollbacks from noisy to clean
    pub rollback_successes: usize,
    /// Attempted rollbacks from noisy to clean
    pub rollback_attempts: usize,
}

impl Default for RawMetrics {
    fn default() -> Self {
        Self {
            tasks_attempted: 0,
            tasks_completed: 0,
            tasks_correct: 0,
            total_steps: 0,
            total_tool_calls: 0,
            total_latency_ms: 0,
            by_difficulty: HashMap::new(),
            episodes: Vec::new(),
            noise_tasks_attempted: 0,
            noise_tasks_correct: 0,
            policy_violations: 0,
            contradictions: 0,
            rollback_successes: 0,
            rollback_attempts: 0,
        }
    }
}

/// Stats per difficulty level
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DifficultyStats {
    pub attempted: usize,
    pub completed: usize,
    pub correct: usize,
    pub avg_steps: f64,
}

/// Per-episode metrics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeMetrics {
    pub episode: usize,
    pub accuracy: f64,
    pub reward: f64,
    pub regret: f64,
    pub cumulative_regret: f64,
}

/// Intelligence metrics calculator
pub struct IntelligenceCalculator {
    /// Weights for capability scoring
    pub capability_weights: [f64; 6],
    /// Baseline for comparison
    pub baseline_accuracy: f64,
    /// Oracle performance for regret calculation
    pub oracle_reward: f64,
}

impl Default for IntelligenceCalculator {
    fn default() -> Self {
        Self {
            capability_weights: [1.0, 1.0, 1.0, 1.0, 1.0, 1.0],
            baseline_accuracy: 0.5,
            oracle_reward: 100.0,
        }
    }
}

impl IntelligenceCalculator {
    /// Calculate intelligence assessment from raw metrics
    pub fn calculate(&self, raw: &RawMetrics) -> IntelligenceAssessment {
        let capabilities = self.calculate_capabilities(raw);
        let reasoning = self.calculate_reasoning(raw);
        let learning = self.calculate_learning(raw);
        let tool_use = self.calculate_tool_use(raw);
        let meta_cognition = self.calculate_meta_cognition(raw);
        let cost = self.calculate_cost(raw);
        let robustness = self.calculate_robustness(raw);

        // Overall score: three equal pillars — graded outcomes, cost, robustness
        let overall_score = self.calculate_overall_score(
            &capabilities,
            &reasoning,
            &learning,
            &tool_use,
            &meta_cognition,
            &cost,
            &robustness,
        );

        IntelligenceAssessment {
            overall_score,
            capabilities,
            reasoning,
            learning,
            tool_use,
            meta_cognition,
            cost,
            robustness,
            raw_data: raw.clone(),
        }
    }

    fn calculate_capabilities(&self, raw: &RawMetrics) -> CapabilityScores {
        let base_accuracy = if raw.tasks_attempted > 0 {
            raw.tasks_correct as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        // Temporal reasoning: accuracy on time-based tasks
        let temporal_reasoning = base_accuracy * 100.0;

        // Constraint satisfaction: correct solutions
        let constraint_satisfaction = base_accuracy * 100.0;

        // Information retrieval: based on steps to solution
        let avg_steps = if raw.tasks_attempted > 0 {
            raw.total_steps as f64 / raw.tasks_attempted as f64
        } else {
            100.0
        };
        let information_retrieval = (100.0 - avg_steps).max(0.0).min(100.0);

        // Pattern recognition: performance improvement across difficulties
        let pattern_recognition = self.calculate_pattern_recognition(raw);

        // Planning: efficiency of tool use
        let avg_tools = if raw.tasks_attempted > 0 {
            raw.total_tool_calls as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };
        let planning = if avg_tools > 0.0 && avg_tools <= 2.0 {
            100.0 * (1.0 - (avg_tools - 1.0).abs() / 2.0)
        } else {
            50.0
        };

        // Adaptation: improvement over episodes
        let adaptation = self.calculate_adaptation(raw);

        CapabilityScores {
            temporal_reasoning,
            constraint_satisfaction,
            information_retrieval,
            pattern_recognition,
            planning,
            adaptation,
        }
    }

    fn calculate_pattern_recognition(&self, raw: &RawMetrics) -> f64 {
        if raw.by_difficulty.len() < 2 {
            return 50.0;
        }

        // Check if harder problems are still solvable
        let mut difficulties: Vec<_> = raw.by_difficulty.keys().copied().collect();
        difficulties.sort();

        let mut scores = Vec::new();
        for d in &difficulties {
            if let Some(stats) = raw.by_difficulty.get(d) {
                if stats.attempted > 0 {
                    scores.push(stats.correct as f64 / stats.attempted as f64);
                }
            }
        }

        if scores.is_empty() {
            return 50.0;
        }

        // Average accuracy across difficulties
        let avg: f64 = scores.iter().sum::<f64>() / scores.len() as f64;
        avg * 100.0
    }

    fn calculate_adaptation(&self, raw: &RawMetrics) -> f64 {
        if raw.episodes.len() < 3 {
            return 50.0;
        }

        // Check if accuracy improves over episodes
        let first_half: f64 = raw.episodes[..raw.episodes.len() / 2]
            .iter()
            .map(|e| e.accuracy)
            .sum::<f64>()
            / (raw.episodes.len() / 2) as f64;

        let second_half: f64 = raw.episodes[raw.episodes.len() / 2..]
            .iter()
            .map(|e| e.accuracy)
            .sum::<f64>()
            / (raw.episodes.len() - raw.episodes.len() / 2) as f64;

        let improvement = second_half - first_half;

        // Scale: -0.2 to +0.2 improvement maps to 0-100
        ((improvement + 0.2) / 0.4 * 100.0).max(0.0).min(100.0)
    }

    fn calculate_reasoning(&self, raw: &RawMetrics) -> ReasoningMetrics {
        let constraint_satisfaction_rate = if raw.tasks_attempted > 0 {
            raw.tasks_correct as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        let avg_steps = if raw.tasks_attempted > 0 {
            raw.total_steps as f64 / raw.tasks_attempted as f64
        } else {
            100.0
        };

        // Reasoning efficiency: inverse of steps (normalized)
        let reasoning_efficiency = (100.0 - avg_steps).max(0.0).min(100.0) / 100.0;

        // Logical coherence: based on completion rate vs correct rate
        let completion_rate = if raw.tasks_attempted > 0 {
            raw.tasks_completed as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };
        let logical_coherence = if completion_rate > 0.0 {
            constraint_satisfaction_rate / completion_rate
        } else {
            0.0
        };

        ReasoningMetrics {
            logical_coherence,
            constraint_satisfaction_rate,
            solution_optimality: constraint_satisfaction_rate,
            reasoning_efficiency,
            error_rate: 1.0 - constraint_satisfaction_rate,
        }
    }

    fn calculate_learning(&self, raw: &RawMetrics) -> LearningMetrics {
        let mut learning = LearningMetrics::default();

        if raw.episodes.is_empty() {
            return learning;
        }

        // Sample efficiency: accuracy per episode
        learning.sample_efficiency =
            raw.episodes.iter().map(|e| e.accuracy).sum::<f64>() / raw.episodes.len() as f64;

        // Regret sublinearity: check if cumulative regret grows sublinearly
        // True sublinearity means R_k/k → 0 as k → ∞ (regret per episode decreasing)
        if raw.episodes.len() >= 5 {
            // Calculate regret trend using linear regression
            let n = raw.episodes.len() as f64;
            let mut sum_x = 0.0;
            let mut sum_y = 0.0;
            let mut sum_xy = 0.0;
            let mut sum_xx = 0.0;

            for (i, ep) in raw.episodes.iter().enumerate() {
                let x = (i + 1) as f64;
                let y = ep.regret;
                sum_x += x;
                sum_y += y;
                sum_xy += x * y;
                sum_xx += x * x;
            }

            let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);

            // Negative slope = decreasing regret = sublinear
            // Transform: slope < 0 → sublinearity > 0
            if slope < 0.0 {
                // Stronger negative slope = better sublinearity (cap at 1.0)
                learning.regret_sublinearity = (-slope / 10.0).min(1.0);
            }

            // Also check cumulative average
            let last = raw.episodes.last().unwrap();
            let avg_regret = last.cumulative_regret / n;
            let first_half_avg = raw
                .episodes
                .iter()
                .take(raw.episodes.len() / 2)
                .map(|e| e.regret)
                .sum::<f64>()
                / (n / 2.0);

            // If second half has lower per-episode regret, that's sublinear
            if avg_regret < first_half_avg && learning.regret_sublinearity == 0.0 {
                learning.regret_sublinearity =
                    ((first_half_avg - avg_regret) / first_half_avg).max(0.0);
            }
        }

        // Learning rate: improvement in accuracy over episodes
        if raw.episodes.len() >= 2 {
            let first_acc = raw.episodes[0].accuracy;
            let last_acc = raw.episodes.last().unwrap().accuracy;
            learning.learning_rate = (last_acc - first_acc + 1.0) / 2.0;
        }

        // Generalization: consistency across difficulties
        if raw.by_difficulty.len() >= 2 {
            let accuracies: Vec<f64> = raw
                .by_difficulty
                .values()
                .filter(|s| s.attempted > 0)
                .map(|s| s.correct as f64 / s.attempted as f64)
                .collect();

            if !accuracies.is_empty() {
                let mean = accuracies.iter().sum::<f64>() / accuracies.len() as f64;
                let variance = accuracies.iter().map(|a| (a - mean).powi(2)).sum::<f64>()
                    / accuracies.len() as f64;
                let std_dev = variance.sqrt();

                // Lower variance = better generalization
                learning.generalization = (1.0 - std_dev).max(0.0);
            }
        }

        learning
    }

    fn calculate_tool_use(&self, raw: &RawMetrics) -> ToolUseMetrics {
        let avg_tools = if raw.tasks_attempted > 0 {
            raw.total_tool_calls as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        // Selection appropriateness: using tools when helpful
        let accuracy = if raw.tasks_attempted > 0 {
            raw.tasks_correct as f64 / raw.tasks_attempted as f64
        } else {
            0.0
        };

        // Effectiveness: accuracy when tools are used
        let utilization_effectiveness = accuracy;

        // Appropriateness: not overusing tools
        let selection_appropriateness = if avg_tools > 0.0 {
            (accuracy / avg_tools.min(2.0)).min(1.0)
        } else {
            0.5
        };

        ToolUseMetrics {
            selection_appropriateness,
            utilization_effectiveness,
            composition_ability: avg_tools.min(1.0), // Using multiple tools
            discovery_ability: accuracy,             // Finding solutions
        }
    }

    fn calculate_meta_cognition(&self, raw: &RawMetrics) -> MetaCognitiveMetrics {
        // Self-correction: completed but not correct -> corrected
        let completed_but_wrong = raw.tasks_completed.saturating_sub(raw.tasks_correct);
        let self_correction_rate = if completed_but_wrong > 0 {
            0.0 // No self-correction if still wrong
        } else if raw.tasks_completed > 0 {
            1.0 // All completed are correct
        } else {
            0.5
        };

        // Strategy adaptation: improvement over episodes
        let strategy_adaptation = if raw.episodes.len() >= 3 {
            let trend: f64 = raw
                .episodes
                .windows(2)
                .map(|w| {
                    if w[1].accuracy > w[0].accuracy {
                        1.0
                    } else {
                        0.0
                    }
                })
                .sum::<f64>();
            trend / (raw.episodes.len() - 1) as f64
        } else {
            0.5
        };

        MetaCognitiveMetrics {
            self_correction_rate,
            uncertainty_calibration: 0.5, // Would need confidence scores
            strategy_adaptation,
            progress_monitoring: strategy_adaptation, // Similar metric
        }
    }

    fn calculate_cost(&self, raw: &RawMetrics) -> CostMetrics {
        let steps_per_solve = if raw.tasks_correct > 0 {
            raw.total_steps as f64 / raw.tasks_correct as f64
        } else if raw.tasks_attempted > 0 {
            raw.total_steps as f64
        } else {
            100.0
        };

        let tools_per_solve = if raw.tasks_correct > 0 {
            raw.total_tool_calls as f64 / raw.tasks_correct as f64
        } else {
            10.0
        };

        // Efficiency: 1.0 at <=5 steps/solve, 0.0 at >=100 steps/solve
        let cost_efficiency = (1.0 - (steps_per_solve - 5.0) / 95.0).clamp(0.0, 1.0);

        // Cost trend: compare early vs late episode accuracy per step
        let cost_trend = if raw.episodes.len() >= 4 {
            let half = raw.episodes.len() / 2;
            let early_acc: f64 =
                raw.episodes[..half].iter().map(|e| e.accuracy).sum::<f64>() / half as f64;
            let late_acc: f64 = raw.episodes[half..].iter().map(|e| e.accuracy).sum::<f64>()
                / (raw.episodes.len() - half) as f64;
            // If accuracy improves, effective cost per solve drops
            if early_acc > 0.01 {
                (late_acc - early_acc) / early_acc
            } else {
                0.0
            }
        } else {
            0.0
        };

        CostMetrics {
            steps_per_solve,
            tools_per_solve,
            cost_efficiency,
            cost_trend,
        }
    }

    fn calculate_robustness(&self, raw: &RawMetrics) -> RobustnessMetrics {
        let noise_accuracy = if raw.noise_tasks_attempted > 0 {
            raw.noise_tasks_correct as f64 / raw.noise_tasks_attempted as f64
        } else {
            0.5 // no noise data -> neutral prior
        };

        let clean_attempted = raw
            .tasks_attempted
            .saturating_sub(raw.noise_tasks_attempted);
        let clean_correct = raw.tasks_correct.saturating_sub(raw.noise_tasks_correct);
        let clean_accuracy = if clean_attempted > 0 {
            clean_correct as f64 / clean_attempted as f64
        } else {
            0.0
        };

        let noise_degradation = (clean_accuracy - noise_accuracy).max(0.0);

        let consistency = if raw.episodes.len() >= 2 {
            let mean =
                raw.episodes.iter().map(|e| e.accuracy).sum::<f64>() / raw.episodes.len() as f64;
            let variance = raw
                .episodes
                .iter()
                .map(|e| (e.accuracy - mean).powi(2))
                .sum::<f64>()
                / raw.episodes.len() as f64;
            (1.0 - variance.sqrt()).max(0.0)
        } else {
            0.5
        };

        let robustness_score =
            noise_accuracy * 0.4 + (1.0 - noise_degradation.min(1.0)) * 0.3 + consistency * 0.3;

        RobustnessMetrics {
            noise_accuracy,
            noise_degradation,
            consistency,
            robustness_score,
        }
    }

    fn calculate_overall_score(
        &self,
        capabilities: &CapabilityScores,
        reasoning: &ReasoningMetrics,
        learning: &LearningMetrics,
        tool_use: &ToolUseMetrics,
        meta_cognition: &MetaCognitiveMetrics,
        cost: &CostMetrics,
        robustness: &RobustnessMetrics,
    ) -> f64 {
        // Sub-scores (0-100 scale)
        let cap_score = capabilities.weighted_average(&self.capability_weights);

        let reasoning_score = (reasoning.logical_coherence
            + reasoning.constraint_satisfaction_rate
            + reasoning.solution_optimality
            + reasoning.reasoning_efficiency)
            / 4.0
            * 100.0;

        let learning_score = (learning.sample_efficiency
            + learning.regret_sublinearity
            + learning.learning_rate
            + learning.generalization)
            / 4.0
            * 100.0;

        let tool_score = (tool_use.selection_appropriateness
            + tool_use.utilization_effectiveness
            + tool_use.composition_ability
            + tool_use.discovery_ability)
            / 4.0
            * 100.0;

        let meta_score = (meta_cognition.self_correction_rate
            + meta_cognition.strategy_adaptation
            + meta_cognition.progress_monitoring)
            / 3.0
            * 100.0;

        let cost_score = cost.cost_efficiency * 100.0;
        let robustness_score = robustness.robustness_score * 100.0;

        // Three equal pillars: graded outcomes (~0.34), cost (~0.33), robustness (~0.33)
        // Graded outcomes = capabilities + reasoning + learning + tool + meta
        cap_score * 0.12
            + reasoning_score * 0.10
            + learning_score * 0.06
            + tool_score * 0.03
            + meta_score * 0.03
            + cost_score * 0.33
            + robustness_score * 0.33
    }
}

/// Print a formatted intelligence report
pub fn print_intelligence_report(assessment: &IntelligenceAssessment) {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              Intelligence Assessment Report                   ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "🧠 Overall Intelligence Score: {:.1}/100",
        assessment.overall_score
    );
    println!();

    println!("📊 Capability Scores:");
    println!(
        "   Temporal Reasoning:     {:5.1}",
        assessment.capabilities.temporal_reasoning
    );
    println!(
        "   Constraint Satisfaction:{:5.1}",
        assessment.capabilities.constraint_satisfaction
    );
    println!(
        "   Information Retrieval:  {:5.1}",
        assessment.capabilities.information_retrieval
    );
    println!(
        "   Pattern Recognition:    {:5.1}",
        assessment.capabilities.pattern_recognition
    );
    println!(
        "   Planning:               {:5.1}",
        assessment.capabilities.planning
    );
    println!(
        "   Adaptation:             {:5.1}",
        assessment.capabilities.adaptation
    );
    println!();

    println!("🔍 Reasoning Quality:");
    println!(
        "   Logical Coherence:      {:.2}",
        assessment.reasoning.logical_coherence
    );
    println!(
        "   Constraint Satisfaction:{:.2}",
        assessment.reasoning.constraint_satisfaction_rate
    );
    println!(
        "   Solution Optimality:    {:.2}",
        assessment.reasoning.solution_optimality
    );
    println!(
        "   Reasoning Efficiency:   {:.2}",
        assessment.reasoning.reasoning_efficiency
    );
    println!(
        "   Error Rate:             {:.2}",
        assessment.reasoning.error_rate
    );
    println!();

    println!("📈 Learning Metrics:");
    println!(
        "   Sample Efficiency:      {:.2}",
        assessment.learning.sample_efficiency
    );
    println!(
        "   Regret Sublinearity:    {:.2}",
        assessment.learning.regret_sublinearity
    );
    println!(
        "   Learning Rate:          {:.2}",
        assessment.learning.learning_rate
    );
    println!(
        "   Generalization:         {:.2}",
        assessment.learning.generalization
    );
    println!();

    println!("🔧 Tool Use Proficiency:");
    println!(
        "   Selection:              {:.2}",
        assessment.tool_use.selection_appropriateness
    );
    println!(
        "   Effectiveness:          {:.2}",
        assessment.tool_use.utilization_effectiveness
    );
    println!(
        "   Composition:            {:.2}",
        assessment.tool_use.composition_ability
    );
    println!();

    println!("🪞 Meta-Cognitive Indicators:");
    println!(
        "   Self-Correction:        {:.2}",
        assessment.meta_cognition.self_correction_rate
    );
    println!(
        "   Strategy Adaptation:    {:.2}",
        assessment.meta_cognition.strategy_adaptation
    );
    println!(
        "   Progress Monitoring:    {:.2}",
        assessment.meta_cognition.progress_monitoring
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intelligence_calculation() {
        let mut raw = RawMetrics::default();
        raw.tasks_attempted = 100;
        raw.tasks_completed = 90;
        raw.tasks_correct = 80;
        raw.total_steps = 500;
        raw.total_tool_calls = 100;

        let calculator = IntelligenceCalculator::default();
        let assessment = calculator.calculate(&raw);

        assert!(assessment.overall_score > 0.0);
        assert!(assessment.capabilities.temporal_reasoning > 0.0);
    }

    #[test]
    fn test_learning_metrics() {
        let mut raw = RawMetrics::default();
        raw.tasks_attempted = 50;
        raw.tasks_correct = 40;

        // Add episodes showing improvement
        for i in 0..10 {
            raw.episodes.push(EpisodeMetrics {
                episode: i + 1,
                accuracy: 0.5 + 0.04 * i as f64,
                reward: 50.0 + 4.0 * i as f64,
                regret: 50.0 - 4.0 * i as f64,
                cumulative_regret: (0..=i).map(|j| 50.0 - 4.0 * j as f64).sum(),
            });
        }

        let calculator = IntelligenceCalculator::default();
        let assessment = calculator.calculate(&raw);

        // Should show learning (improvement over time)
        assert!(assessment.learning.learning_rate > 0.5);
    }
}
