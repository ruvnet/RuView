//! # Artificial Instincts
//!
//! Encode instincts instead of goals.
//!
//! Instincts like:
//! - Avoid fragmentation
//! - Preserve causal continuity
//! - Minimize delayed consequences
//! - Prefer reversible actions under uncertainty
//!
//! These are not rules. They are biases enforced by mincut, attention, and healing.
//! This is closer to evolution than training.

/// An instinctive bias that shapes behavior without explicit rules
pub trait Instinct: Send + Sync {
    /// Name of this instinct
    fn name(&self) -> &str;

    /// Evaluate how well an action aligns with this instinct
    /// Returns bias: negative = suppress, positive = encourage
    fn evaluate(&self, context: &InstinctContext, action: &ProposedAction) -> f64;

    /// The strength of this instinct (0-1)
    fn strength(&self) -> f64;
}

/// Context for instinct evaluation
pub struct InstinctContext {
    /// Current mincut tension (0-1)
    pub mincut_tension: f64,
    /// Graph fragmentation level (0-1)
    pub fragmentation: f64,
    /// Causal chain depth from root
    pub causal_depth: usize,
    /// Uncertainty in current state
    pub uncertainty: f64,
    /// Recent action history
    pub recent_actions: Vec<ActionOutcome>,
}

/// A proposed action to evaluate
pub struct ProposedAction {
    pub name: String,
    pub reversible: bool,
    pub affects_structure: bool,
    pub delayed_effects: bool,
    pub estimated_fragmentation_delta: f64,
    pub causal_chain_additions: usize,
}

/// Outcome of a past action
pub struct ActionOutcome {
    pub action_name: String,
    pub tension_before: f64,
    pub tension_after: f64,
    pub fragmentation_delta: f64,
}

// =============================================================================
// Core Instincts
// =============================================================================

/// Instinct: Avoid fragmentation
/// Suppresses actions that would split coherent structures
pub struct AvoidFragmentation {
    strength: f64,
}

impl AvoidFragmentation {
    pub fn new(strength: f64) -> Self {
        Self { strength }
    }
}

impl Instinct for AvoidFragmentation {
    fn name(&self) -> &str {
        "AvoidFragmentation"
    }

    fn evaluate(&self, context: &InstinctContext, action: &ProposedAction) -> f64 {
        // Strong negative bias if action increases fragmentation
        if action.estimated_fragmentation_delta > 0.0 {
            -action.estimated_fragmentation_delta * 2.0 * self.strength
        } else {
            // Slight positive bias for actions that reduce fragmentation
            -action.estimated_fragmentation_delta * 0.5 * self.strength
        }
    }

    fn strength(&self) -> f64 {
        self.strength
    }
}

/// Instinct: Preserve causal continuity
/// Prefers actions that maintain clear cause-effect chains
pub struct PreserveCausality {
    strength: f64,
    max_chain_depth: usize,
}

impl PreserveCausality {
    pub fn new(strength: f64, max_chain_depth: usize) -> Self {
        Self {
            strength,
            max_chain_depth,
        }
    }
}

impl Instinct for PreserveCausality {
    fn name(&self) -> &str {
        "PreserveCausality"
    }

    fn evaluate(&self, context: &InstinctContext, action: &ProposedAction) -> f64 {
        let new_depth = context.causal_depth + action.causal_chain_additions;

        if new_depth > self.max_chain_depth {
            // Suppress actions that extend causal chains too far
            let overshoot = (new_depth - self.max_chain_depth) as f64;
            -overshoot * 0.3 * self.strength
        } else if action.affects_structure && action.causal_chain_additions == 0 {
            // Structural changes without causal extension = potential discontinuity
            -0.2 * self.strength
        } else {
            0.0
        }
    }

    fn strength(&self) -> f64 {
        self.strength
    }
}

/// Instinct: Minimize delayed consequences
/// Prefers actions with immediate, observable effects
pub struct MinimizeDelayedEffects {
    strength: f64,
}

impl MinimizeDelayedEffects {
    pub fn new(strength: f64) -> Self {
        Self { strength }
    }
}

impl Instinct for MinimizeDelayedEffects {
    fn name(&self) -> &str {
        "MinimizeDelayedEffects"
    }

    fn evaluate(&self, _context: &InstinctContext, action: &ProposedAction) -> f64 {
        if action.delayed_effects {
            -0.3 * self.strength
        } else {
            0.1 * self.strength // Slight preference for immediate feedback
        }
    }

    fn strength(&self) -> f64 {
        self.strength
    }
}

/// Instinct: Prefer reversible actions under uncertainty
/// When uncertain, choose actions that can be undone
pub struct PreferReversibility {
    strength: f64,
    uncertainty_threshold: f64,
}

impl PreferReversibility {
    pub fn new(strength: f64, uncertainty_threshold: f64) -> Self {
        Self {
            strength,
            uncertainty_threshold,
        }
    }
}

impl Instinct for PreferReversibility {
    fn name(&self) -> &str {
        "PreferReversibility"
    }

    fn evaluate(&self, context: &InstinctContext, action: &ProposedAction) -> f64 {
        if context.uncertainty > self.uncertainty_threshold {
            if action.reversible {
                0.4 * self.strength * context.uncertainty
            } else {
                -0.5 * self.strength * context.uncertainty
            }
        } else {
            // Under certainty, no preference
            0.0
        }
    }

    fn strength(&self) -> f64 {
        self.strength
    }
}

/// Instinct: Seek homeostasis
/// Prefer actions that return system to baseline tension
pub struct SeekHomeostasis {
    strength: f64,
    baseline_tension: f64,
}

impl SeekHomeostasis {
    pub fn new(strength: f64, baseline_tension: f64) -> Self {
        Self {
            strength,
            baseline_tension,
        }
    }
}

impl Instinct for SeekHomeostasis {
    fn name(&self) -> &str {
        "SeekHomeostasis"
    }

    fn evaluate(&self, context: &InstinctContext, action: &ProposedAction) -> f64 {
        // Look at recent history to predict tension change
        let avg_tension_delta: f64 = if context.recent_actions.is_empty() {
            0.0
        } else {
            context
                .recent_actions
                .iter()
                .map(|a| a.tension_after - a.tension_before)
                .sum::<f64>()
                / context.recent_actions.len() as f64
        };

        let current_deviation = (context.mincut_tension - self.baseline_tension).abs();

        // Encourage actions when far from baseline, if past similar actions reduced tension
        if current_deviation > 0.2 && avg_tension_delta < 0.0 {
            current_deviation * self.strength
        } else if current_deviation > 0.2 && avg_tension_delta > 0.0 {
            -current_deviation * 0.5 * self.strength
        } else {
            0.0
        }
    }

    fn strength(&self) -> f64 {
        self.strength
    }
}

// =============================================================================
// Instinct Engine
// =============================================================================

/// Engine that applies instincts to bias action selection
pub struct InstinctEngine {
    instincts: Vec<Box<dyn Instinct>>,
}

impl InstinctEngine {
    pub fn new() -> Self {
        Self {
            instincts: Vec::new(),
        }
    }

    /// Add a primal instinct set (recommended defaults)
    pub fn with_primal_instincts(mut self) -> Self {
        self.instincts.push(Box::new(AvoidFragmentation::new(0.8)));
        self.instincts
            .push(Box::new(PreserveCausality::new(0.7, 10)));
        self.instincts
            .push(Box::new(MinimizeDelayedEffects::new(0.5)));
        self.instincts
            .push(Box::new(PreferReversibility::new(0.9, 0.4)));
        self.instincts
            .push(Box::new(SeekHomeostasis::new(0.6, 0.2)));
        self
    }

    pub fn add_instinct(&mut self, instinct: Box<dyn Instinct>) {
        self.instincts.push(instinct);
    }

    /// Evaluate all instincts and return combined bias
    pub fn evaluate(
        &self,
        context: &InstinctContext,
        action: &ProposedAction,
    ) -> InstinctEvaluation {
        let mut contributions = Vec::new();
        let mut total_bias = 0.0;

        for instinct in &self.instincts {
            let bias = instinct.evaluate(context, action);
            contributions.push((instinct.name().to_string(), bias));
            total_bias += bias;
        }

        InstinctEvaluation {
            action_name: action.name.clone(),
            total_bias,
            contributions,
            recommendation: if total_bias > 0.3 {
                InstinctRecommendation::Encourage
            } else if total_bias < -0.3 {
                InstinctRecommendation::Suppress
            } else {
                InstinctRecommendation::Neutral
            },
        }
    }

    /// Rank actions by instinctive preference
    pub fn rank_actions(
        &self,
        context: &InstinctContext,
        actions: &[ProposedAction],
    ) -> Vec<(String, f64)> {
        let mut rankings: Vec<(String, f64)> = actions
            .iter()
            .map(|a| {
                let eval = self.evaluate(context, a);
                (a.name.clone(), eval.total_bias)
            })
            .collect();

        rankings.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        rankings
    }
}

#[derive(Debug)]
pub struct InstinctEvaluation {
    pub action_name: String,
    pub total_bias: f64,
    pub contributions: Vec<(String, f64)>,
    pub recommendation: InstinctRecommendation,
}

#[derive(Debug)]
pub enum InstinctRecommendation {
    Encourage,
    Neutral,
    Suppress,
}

fn main() {
    println!("=== Artificial Instincts ===\n");
    println!("Not rules. Biases enforced by structure.\n");

    let engine = InstinctEngine::new().with_primal_instincts();

    // Create context
    let context = InstinctContext {
        mincut_tension: 0.5,
        fragmentation: 0.3,
        causal_depth: 5,
        uncertainty: 0.6,
        recent_actions: vec![ActionOutcome {
            action_name: "rebalance".into(),
            tension_before: 0.6,
            tension_after: 0.5,
            fragmentation_delta: -0.05,
        }],
    };

    // Possible actions
    let actions = vec![
        ProposedAction {
            name: "Split workload".into(),
            reversible: true,
            affects_structure: true,
            delayed_effects: false,
            estimated_fragmentation_delta: 0.15,
            causal_chain_additions: 2,
        },
        ProposedAction {
            name: "Merge subsystems".into(),
            reversible: false,
            affects_structure: true,
            delayed_effects: true,
            estimated_fragmentation_delta: -0.2,
            causal_chain_additions: 1,
        },
        ProposedAction {
            name: "Add monitoring".into(),
            reversible: true,
            affects_structure: false,
            delayed_effects: false,
            estimated_fragmentation_delta: 0.0,
            causal_chain_additions: 0,
        },
        ProposedAction {
            name: "Aggressive optimization".into(),
            reversible: false,
            affects_structure: true,
            delayed_effects: true,
            estimated_fragmentation_delta: 0.1,
            causal_chain_additions: 4,
        },
        ProposedAction {
            name: "Gradual rebalance".into(),
            reversible: true,
            affects_structure: true,
            delayed_effects: false,
            estimated_fragmentation_delta: -0.05,
            causal_chain_additions: 1,
        },
    ];

    println!(
        "Context: tension={:.2}, fragmentation={:.2}, uncertainty={:.2}\n",
        context.mincut_tension, context.fragmentation, context.uncertainty
    );

    println!("Action                  | Bias   | Recommendation | Top Contributors");
    println!("------------------------|--------|----------------|------------------");

    for action in &actions {
        let eval = engine.evaluate(&context, action);

        // Get top 2 contributors
        let mut contribs = eval.contributions.clone();
        contribs.sort_by(|a, b| b.1.abs().partial_cmp(&a.1.abs()).unwrap());
        let top_contribs: Vec<String> = contribs
            .iter()
            .take(2)
            .map(|(name, bias)| format!("{}:{:+.2}", &name[..3.min(name.len())], bias))
            .collect();

        println!(
            "{:23} | {:+.2}   | {:14?} | {}",
            action.name,
            eval.total_bias,
            eval.recommendation,
            top_contribs.join(", ")
        );
    }

    println!("\n=== Instinctive Ranking ===");
    let rankings = engine.rank_actions(&context, &actions);
    for (i, (name, bias)) in rankings.iter().enumerate() {
        let marker = if *bias > 0.3 {
            "+"
        } else if *bias < -0.3 {
            "-"
        } else {
            " "
        };
        println!("{}. {} {:23} ({:+.2})", i + 1, marker, name, bias);
    }

    println!("\n\"Closer to evolution than training.\"");
}
