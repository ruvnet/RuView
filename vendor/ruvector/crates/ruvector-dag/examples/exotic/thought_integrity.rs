//! # Thought Integrity Monitoring
//!
//! Compute substrates where reasoning integrity is monitored like voltage or temperature.
//!
//! When coherence drops:
//! - Reduce precision
//! - Exit early
//! - Route to simpler paths
//! - Escalate to heavier reasoning only if needed
//!
//! This is how you get always-on intelligence without runaway cost.

use std::time::{Duration, Instant};

/// Reasoning depth levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReasoningDepth {
    /// Pattern matching only - instant, near-zero cost
    Reflexive,
    /// Simple inference - fast, low cost
    Shallow,
    /// Standard reasoning - moderate cost
    Standard,
    /// Deep analysis - high cost
    Deep,
    /// Full deliberation - maximum cost
    Deliberative,
}

impl ReasoningDepth {
    fn cost_multiplier(&self) -> f64 {
        match self {
            Self::Reflexive => 0.01,
            Self::Shallow => 0.1,
            Self::Standard => 1.0,
            Self::Deep => 5.0,
            Self::Deliberative => 20.0,
        }
    }

    fn precision(&self) -> f64 {
        match self {
            Self::Reflexive => 0.6,
            Self::Shallow => 0.75,
            Self::Standard => 0.9,
            Self::Deep => 0.95,
            Self::Deliberative => 0.99,
        }
    }
}

/// A reasoning step with integrity monitoring
#[derive(Clone, Debug)]
pub struct ReasoningStep {
    pub description: String,
    pub coherence: f64,
    pub confidence: f64,
    pub depth: ReasoningDepth,
    pub cost: f64,
}

/// Thought integrity monitor - like voltage monitoring for reasoning
pub struct ThoughtIntegrityMonitor {
    /// Current coherence level (0-1)
    coherence: f64,

    /// Rolling coherence history
    coherence_history: Vec<f64>,

    /// Current reasoning depth
    depth: ReasoningDepth,

    /// Thresholds for depth adjustment
    thresholds: DepthThresholds,

    /// Total reasoning cost accumulated
    total_cost: f64,

    /// Cost budget per time window
    cost_budget: f64,

    /// Steps taken at each depth
    depth_counts: [usize; 5],

    /// Early exits taken
    early_exits: usize,

    /// Escalations to deeper reasoning
    escalations: usize,
}

struct DepthThresholds {
    /// Above this, can use Deliberative
    deliberative: f64,
    /// Above this, can use Deep
    deep: f64,
    /// Above this, can use Standard
    standard: f64,
    /// Above this, can use Shallow
    shallow: f64,
    /// Below this, must use Reflexive only
    reflexive: f64,
}

impl Default for DepthThresholds {
    fn default() -> Self {
        Self {
            deliberative: 0.95,
            deep: 0.85,
            standard: 0.7,
            shallow: 0.5,
            reflexive: 0.3,
        }
    }
}

/// Result of a reasoning attempt
#[derive(Debug)]
pub enum ReasoningResult {
    /// Successfully completed at given depth
    Complete {
        answer: String,
        confidence: f64,
        depth_used: ReasoningDepth,
        cost: f64,
    },
    /// Exited early due to coherence drop
    EarlyExit {
        partial_answer: String,
        coherence_at_exit: f64,
        steps_completed: usize,
    },
    /// Escalated to deeper reasoning
    Escalated {
        from_depth: ReasoningDepth,
        to_depth: ReasoningDepth,
        reason: String,
    },
    /// Refused to process - integrity too low
    Refused { coherence: f64, reason: String },
}

impl ThoughtIntegrityMonitor {
    pub fn new(cost_budget: f64) -> Self {
        Self {
            coherence: 1.0,
            coherence_history: Vec::with_capacity(100),
            depth: ReasoningDepth::Standard,
            thresholds: DepthThresholds::default(),
            total_cost: 0.0,
            cost_budget,
            depth_counts: [0; 5],
            early_exits: 0,
            escalations: 0,
        }
    }

    /// Process a query with integrity monitoring
    pub fn process(&mut self, query: &str, required_precision: f64) -> ReasoningResult {
        // Check if we should refuse
        if self.coherence < self.thresholds.reflexive {
            return ReasoningResult::Refused {
                coherence: self.coherence,
                reason: "Coherence critically low - refusing to process".into(),
            };
        }

        // Determine initial depth based on coherence and required precision
        let initial_depth = self.select_depth(required_precision);
        self.depth = initial_depth;

        // Simulate reasoning steps
        let mut steps: Vec<ReasoningStep> = Vec::new();
        let mut current_confidence = 0.5;

        for step_num in 0..10 {
            // Simulate coherence drift during reasoning
            let step_coherence = self.simulate_step_coherence(step_num, query);
            self.update_coherence(step_coherence);

            // Check for early exit
            if self.should_early_exit(current_confidence, required_precision) {
                self.early_exits += 1;
                return ReasoningResult::EarlyExit {
                    partial_answer: format!("Partial answer from {} steps", steps.len()),
                    coherence_at_exit: self.coherence,
                    steps_completed: steps.len(),
                };
            }

            // Check for escalation need
            if current_confidence < required_precision * 0.7 && self.can_escalate() && step_num > 3
            {
                let old_depth = self.depth;
                self.depth = self.escalate_depth();
                self.escalations += 1;
                return ReasoningResult::Escalated {
                    from_depth: old_depth,
                    to_depth: self.depth,
                    reason: "Confidence too low for required precision".into(),
                };
            }

            // Execute step
            let step_cost = self.depth.cost_multiplier() * 0.1;
            self.total_cost += step_cost;

            current_confidence += (self.depth.precision() - current_confidence) * 0.2;

            steps.push(ReasoningStep {
                description: format!("Step {}", step_num + 1),
                coherence: self.coherence,
                confidence: current_confidence,
                depth: self.depth,
                cost: step_cost,
            });

            self.depth_counts[self.depth as usize] += 1;

            // Check if we've achieved required precision
            if current_confidence >= required_precision {
                break;
            }

            // Adjust depth based on updated coherence
            self.depth = self.select_depth(required_precision);
        }

        let total_step_cost: f64 = steps.iter().map(|s| s.cost).sum();

        ReasoningResult::Complete {
            answer: format!("Answer from {} steps at {:?}", steps.len(), self.depth),
            confidence: current_confidence,
            depth_used: self.depth,
            cost: total_step_cost,
        }
    }

    /// Get current integrity status
    pub fn status(&self) -> IntegrityStatus {
        IntegrityStatus {
            coherence: self.coherence,
            coherence_trend: self.coherence_trend(),
            current_depth: self.depth,
            max_allowed_depth: self.max_allowed_depth(),
            total_cost: self.total_cost,
            budget_remaining: (self.cost_budget - self.total_cost).max(0.0),
            depth_distribution: self.depth_counts,
            early_exits: self.early_exits,
            escalations: self.escalations,
        }
    }

    fn select_depth(&self, required_precision: f64) -> ReasoningDepth {
        // Balance coherence-allowed depth with precision requirements
        let max_depth = self.max_allowed_depth();

        // Find minimum depth that meets precision requirement
        let min_needed = if required_precision > 0.95 {
            ReasoningDepth::Deliberative
        } else if required_precision > 0.9 {
            ReasoningDepth::Deep
        } else if required_precision > 0.8 {
            ReasoningDepth::Standard
        } else if required_precision > 0.7 {
            ReasoningDepth::Shallow
        } else {
            ReasoningDepth::Reflexive
        };

        // Use the lesser of max allowed and minimum needed
        if max_depth < min_needed {
            max_depth
        } else {
            min_needed
        }
    }

    fn max_allowed_depth(&self) -> ReasoningDepth {
        if self.coherence >= self.thresholds.deliberative {
            ReasoningDepth::Deliberative
        } else if self.coherence >= self.thresholds.deep {
            ReasoningDepth::Deep
        } else if self.coherence >= self.thresholds.standard {
            ReasoningDepth::Standard
        } else if self.coherence >= self.thresholds.shallow {
            ReasoningDepth::Shallow
        } else {
            ReasoningDepth::Reflexive
        }
    }

    fn can_escalate(&self) -> bool {
        self.depth < self.max_allowed_depth() && self.total_cost < self.cost_budget * 0.8
    }

    fn escalate_depth(&self) -> ReasoningDepth {
        let max = self.max_allowed_depth();
        match self.depth {
            ReasoningDepth::Reflexive if max >= ReasoningDepth::Shallow => ReasoningDepth::Shallow,
            ReasoningDepth::Shallow if max >= ReasoningDepth::Standard => ReasoningDepth::Standard,
            ReasoningDepth::Standard if max >= ReasoningDepth::Deep => ReasoningDepth::Deep,
            ReasoningDepth::Deep if max >= ReasoningDepth::Deliberative => {
                ReasoningDepth::Deliberative
            }
            _ => self.depth,
        }
    }

    fn should_early_exit(&self, confidence: f64, required: f64) -> bool {
        // Exit early if:
        // 1. Coherence dropped significantly
        // 2. And we've achieved some confidence
        self.coherence < self.thresholds.shallow && confidence > required * 0.6
    }

    fn simulate_step_coherence(&self, step: usize, query: &str) -> f64 {
        // Simulate coherence based on query complexity and step depth
        let base = 0.9 - (step as f64 * 0.02);
        let complexity_factor = 1.0 - (query.len() as f64 * 0.001).min(0.3);
        base * complexity_factor
    }

    fn update_coherence(&mut self, new_coherence: f64) {
        // Exponential moving average
        self.coherence = self.coherence * 0.7 + new_coherence * 0.3;
        self.coherence_history.push(self.coherence);
        if self.coherence_history.len() > 50 {
            self.coherence_history.remove(0);
        }
    }

    fn coherence_trend(&self) -> f64 {
        if self.coherence_history.len() < 10 {
            return 0.0;
        }
        let recent: f64 = self.coherence_history.iter().rev().take(5).sum::<f64>() / 5.0;
        let older: f64 = self
            .coherence_history
            .iter()
            .rev()
            .skip(5)
            .take(5)
            .sum::<f64>()
            / 5.0;
        recent - older
    }
}

#[derive(Debug)]
pub struct IntegrityStatus {
    pub coherence: f64,
    pub coherence_trend: f64,
    pub current_depth: ReasoningDepth,
    pub max_allowed_depth: ReasoningDepth,
    pub total_cost: f64,
    pub budget_remaining: f64,
    pub depth_distribution: [usize; 5],
    pub early_exits: usize,
    pub escalations: usize,
}

fn main() {
    println!("=== Thought Integrity Monitoring ===\n");
    println!("Reasoning integrity monitored like voltage or temperature.\n");

    let mut monitor = ThoughtIntegrityMonitor::new(100.0);

    // Various queries with different precision requirements
    let queries = vec![
        ("Simple lookup", 0.7),
        ("Pattern matching", 0.75),
        ("Basic inference", 0.85),
        ("Complex reasoning", 0.92),
        ("Critical decision", 0.98),
        ("Another simple query", 0.65),
        ("Medium complexity", 0.8),
        ("Deep analysis needed", 0.95),
    ];

    println!("Query               | Precision | Result          | Depth      | Coherence");
    println!("--------------------|-----------|-----------------|------------|----------");

    for (query, precision) in &queries {
        let result = monitor.process(query, *precision);
        let status = monitor.status();

        let result_str = match &result {
            ReasoningResult::Complete { depth_used, .. } => format!("Complete ({:?})", depth_used),
            ReasoningResult::EarlyExit {
                steps_completed, ..
            } => format!("EarlyExit ({})", steps_completed),
            ReasoningResult::Escalated { to_depth, .. } => format!("Escalated->{:?}", to_depth),
            ReasoningResult::Refused { .. } => "REFUSED".into(),
        };

        println!(
            "{:19} | {:.2}      | {:15} | {:10?} | {:.2}",
            query, precision, result_str, status.current_depth, status.coherence
        );
    }

    let final_status = monitor.status();

    println!("\n=== Integrity Summary ===");
    println!(
        "Total cost: {:.2} / {:.2} budget",
        final_status.total_cost, 100.0
    );
    println!("Budget remaining: {:.2}", final_status.budget_remaining);
    println!("Early exits: {}", final_status.early_exits);
    println!("Escalations: {}", final_status.escalations);

    println!("\nDepth distribution:");
    let depth_names = ["Reflexive", "Shallow", "Standard", "Deep", "Deliberative"];
    for (i, count) in final_status.depth_distribution.iter().enumerate() {
        if *count > 0 {
            println!("  {:12}: {} steps", depth_names[i], count);
        }
    }

    println!("\n\"Always-on intelligence without runaway cost.\"");
}
