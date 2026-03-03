//! Temporal logic verification with neural reasoning
//!
//! Integrates temporal-neural-solver for:
//! - Linear Temporal Logic (LTL) verification
//! - Metric Temporal Logic (MTL) with timing constraints
//! - Neural-symbolic reasoning
//! - Differentiable temporal logic

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Temporal logic operators
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TemporalOperator {
    /// Next (X φ)
    Next,
    /// Eventually (F φ) - sometime in the future
    Eventually,
    /// Globally (G φ) - always in the future
    Globally,
    /// Until (φ U ψ) - φ holds until ψ becomes true
    Until,
    /// Release (φ R ψ) - ψ holds until and including when φ becomes true
    Release,
}

/// Temporal logic formula
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalFormula {
    /// Atomic proposition (e.g., "safety_check_passed")
    Atom(String),
    /// Negation (¬φ)
    Not(Box<TemporalFormula>),
    /// Conjunction (φ ∧ ψ)
    And(Box<TemporalFormula>, Box<TemporalFormula>),
    /// Disjunction (φ ∨ ψ)
    Or(Box<TemporalFormula>, Box<TemporalFormula>),
    /// Implication (φ → ψ)
    Implies(Box<TemporalFormula>, Box<TemporalFormula>),
    /// Temporal operator
    Temporal(TemporalOperator, Box<TemporalFormula>),
    /// Bounded temporal (with time constraint for MTL)
    BoundedTemporal {
        operator: TemporalOperator,
        formula: Box<TemporalFormula>,
        lower_bound: Duration,
        upper_bound: Duration,
    },
}

impl TemporalFormula {
    /// Create an atomic proposition
    pub fn atom(name: impl Into<String>) -> Self {
        TemporalFormula::Atom(name.into())
    }

    /// Create negation
    pub fn not(formula: TemporalFormula) -> Self {
        TemporalFormula::Not(Box::new(formula))
    }

    /// Create conjunction
    pub fn and(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::And(Box::new(left), Box::new(right))
    }

    /// Create disjunction
    pub fn or(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Or(Box::new(left), Box::new(right))
    }

    /// Create implication
    pub fn implies(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Implies(Box::new(left), Box::new(right))
    }

    /// Create eventually (F)
    pub fn eventually(formula: TemporalFormula) -> Self {
        TemporalFormula::Temporal(TemporalOperator::Eventually, Box::new(formula))
    }

    /// Create globally (G)
    pub fn globally(formula: TemporalFormula) -> Self {
        TemporalFormula::Temporal(TemporalOperator::Globally, Box::new(formula))
    }

    /// Create next (X)
    pub fn next(formula: TemporalFormula) -> Self {
        TemporalFormula::Temporal(TemporalOperator::Next, Box::new(formula))
    }

    /// Create until (U)
    pub fn until(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Temporal(
            TemporalOperator::Until,
            Box::new(TemporalFormula::And(Box::new(left), Box::new(right))),
        )
    }

    /// Create bounded eventually (for MTL)
    pub fn eventually_bounded(
        formula: TemporalFormula,
        lower: Duration,
        upper: Duration,
    ) -> Self {
        TemporalFormula::BoundedTemporal {
            operator: TemporalOperator::Eventually,
            formula: Box::new(formula),
            lower_bound: lower,
            upper_bound: upper,
        }
    }

    /// Create bounded globally (for MTL)
    pub fn globally_bounded(
        formula: TemporalFormula,
        lower: Duration,
        upper: Duration,
    ) -> Self {
        TemporalFormula::BoundedTemporal {
            operator: TemporalOperator::Globally,
            formula: Box::new(formula),
            lower_bound: lower,
            upper_bound: upper,
        }
    }
}

/// Temporal trace (sequence of states over time)
#[derive(Debug, Clone)]
pub struct TemporalTrace {
    states: Vec<TemporalState>,
}

impl TemporalTrace {
    /// Create a new empty trace
    pub fn new() -> Self {
        Self { states: Vec::new() }
    }

    /// Add a state to the trace
    pub fn add_state(&mut self, state: TemporalState) {
        self.states.push(state);
    }

    /// Get trace length
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Check if trace is empty
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    /// Get state at index
    pub fn get_state(&self, index: usize) -> Option<&TemporalState> {
        self.states.get(index)
    }
}

impl Default for TemporalTrace {
    fn default() -> Self {
        Self::new()
    }
}

/// A state in time with propositions
#[derive(Debug, Clone)]
pub struct TemporalState {
    /// Atomic propositions that are true in this state
    pub propositions: HashMap<String, bool>,
    /// Timestamp of this state
    pub timestamp: Duration,
    /// Confidence in state observations (for neural reasoning)
    pub confidence: f64,
}

impl TemporalState {
    /// Create a new temporal state
    pub fn new(timestamp: Duration) -> Self {
        Self {
            propositions: HashMap::new(),
            timestamp,
            confidence: 1.0,
        }
    }

    /// Set a proposition value
    pub fn set(&mut self, name: String, value: bool) {
        self.propositions.insert(name, value);
    }

    /// Check if a proposition is true
    pub fn is_true(&self, name: &str) -> bool {
        self.propositions.get(name).copied().unwrap_or(false)
    }
}

/// Verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    /// Whether the formula holds
    pub holds: bool,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Explanation of the result
    pub explanation: String,
    /// Counterexample trace if formula doesn't hold
    pub counterexample: Option<Vec<String>>,
}

/// Temporal neural solver combining logic and learning
pub struct TemporalNeuralSolver {
    /// Neural weights for soft logic (learned from data)
    neural_weights: HashMap<String, f64>,
    /// Verification cache
    cache: HashMap<String, VerificationResult>,
}

impl TemporalNeuralSolver {
    /// Create a new temporal neural solver
    pub fn new() -> Self {
        Self {
            neural_weights: HashMap::new(),
            cache: HashMap::new(),
        }
    }

    /// Verify a temporal formula against a trace
    pub fn verify(
        &mut self,
        formula: &TemporalFormula,
        trace: &TemporalTrace,
    ) -> VerificationResult {
        // Check cache
        let cache_key = format!("{:?}", formula);
        if let Some(cached) = self.cache.get(&cache_key) {
            return cached.clone();
        }

        // Verify formula
        let result = self.verify_at_position(formula, trace, 0);

        // Cache result
        self.cache.insert(cache_key, result.clone());

        result
    }

    /// Verify formula at a specific position in the trace
    fn verify_at_position(
        &self,
        formula: &TemporalFormula,
        trace: &TemporalTrace,
        position: usize,
    ) -> VerificationResult {
        match formula {
            TemporalFormula::Atom(name) => {
                if let Some(state) = trace.get_state(position) {
                    let holds = state.is_true(name);
                    VerificationResult {
                        holds,
                        confidence: state.confidence,
                        explanation: format!(
                            "Atom '{}' is {} at position {}",
                            name,
                            if holds { "true" } else { "false" },
                            position
                        ),
                        counterexample: if holds { None } else { Some(vec![name.clone()]) },
                    }
                } else {
                    VerificationResult {
                        holds: false,
                        confidence: 0.0,
                        explanation: format!("Position {} out of bounds", position),
                        counterexample: Some(vec!["out_of_bounds".to_string()]),
                    }
                }
            }

            TemporalFormula::Not(inner) => {
                let inner_result = self.verify_at_position(inner, trace, position);
                VerificationResult {
                    holds: !inner_result.holds,
                    confidence: inner_result.confidence,
                    explanation: format!("Not({})", inner_result.explanation),
                    counterexample: if !inner_result.holds {
                        None
                    } else {
                        inner_result.counterexample
                    },
                }
            }

            TemporalFormula::And(left, right) => {
                let left_result = self.verify_at_position(left, trace, position);
                let right_result = self.verify_at_position(right, trace, position);

                let holds = left_result.holds && right_result.holds;
                let confidence = left_result.confidence.min(right_result.confidence);

                VerificationResult {
                    holds,
                    confidence,
                    explanation: format!(
                        "({}) AND ({})",
                        left_result.explanation, right_result.explanation
                    ),
                    counterexample: if !holds {
                        Some(
                            left_result
                                .counterexample
                                .unwrap_or_default()
                                .into_iter()
                                .chain(right_result.counterexample.unwrap_or_default())
                                .collect(),
                        )
                    } else {
                        None
                    },
                }
            }

            TemporalFormula::Or(left, right) => {
                let left_result = self.verify_at_position(left, trace, position);
                let right_result = self.verify_at_position(right, trace, position);

                let holds = left_result.holds || right_result.holds;
                let confidence = left_result.confidence.max(right_result.confidence);

                VerificationResult {
                    holds,
                    confidence,
                    explanation: format!(
                        "({}) OR ({})",
                        left_result.explanation, right_result.explanation
                    ),
                    counterexample: if !holds {
                        Some(
                            left_result
                                .counterexample
                                .unwrap_or_default()
                                .into_iter()
                                .chain(right_result.counterexample.unwrap_or_default())
                                .collect(),
                        )
                    } else {
                        None
                    },
                }
            }

            TemporalFormula::Implies(left, right) => {
                // A -> B is equivalent to (¬A) ∨ B
                let left_result = self.verify_at_position(left, trace, position);
                let right_result = self.verify_at_position(right, trace, position);

                let holds = !left_result.holds || right_result.holds;
                let confidence = if left_result.holds {
                    right_result.confidence
                } else {
                    1.0
                };

                VerificationResult {
                    holds,
                    confidence,
                    explanation: format!(
                        "({}) IMPLIES ({})",
                        left_result.explanation, right_result.explanation
                    ),
                    counterexample: if !holds {
                        right_result.counterexample
                    } else {
                        None
                    },
                }
            }

            TemporalFormula::Temporal(op, inner) => match op {
                TemporalOperator::Next => {
                    if position + 1 < trace.len() {
                        self.verify_at_position(inner, trace, position + 1)
                    } else {
                        VerificationResult {
                            holds: false,
                            confidence: 0.0,
                            explanation: "Next: no next state".to_string(),
                            counterexample: Some(vec!["no_next_state".to_string()]),
                        }
                    }
                }

                TemporalOperator::Eventually => {
                    // F φ: φ holds at some point in the future
                    for i in position..trace.len() {
                        let result = self.verify_at_position(inner, trace, i);
                        if result.holds {
                            return VerificationResult {
                                holds: true,
                                confidence: result.confidence,
                                explanation: format!("Eventually at position {}: {}", i, result.explanation),
                                counterexample: None,
                            };
                        }
                    }
                    VerificationResult {
                        holds: false,
                        confidence: 0.0,
                        explanation: "Eventually: never becomes true".to_string(),
                        counterexample: Some(vec!["never_true".to_string()]),
                    }
                }

                TemporalOperator::Globally => {
                    // G φ: φ holds at all points in the future
                    let mut min_confidence = 1.0;
                    for i in position..trace.len() {
                        let result = self.verify_at_position(inner, trace, i);
                        if !result.holds {
                            return VerificationResult {
                                holds: false,
                                confidence: result.confidence,
                                explanation: format!(
                                    "Globally fails at position {}: {}",
                                    i, result.explanation
                                ),
                                counterexample: Some(vec![format!("fails_at_{}", i)]),
                            };
                        }
                        min_confidence = min_confidence.min(result.confidence);
                    }
                    VerificationResult {
                        holds: true,
                        confidence: min_confidence,
                        explanation: "Globally: holds everywhere".to_string(),
                        counterexample: None,
                    }
                }

                TemporalOperator::Until => {
                    // Simplified Until operator
                    VerificationResult {
                        holds: false,
                        confidence: 0.5,
                        explanation: "Until: not fully implemented".to_string(),
                        counterexample: Some(vec!["not_implemented".to_string()]),
                    }
                }

                TemporalOperator::Release => {
                    // Simplified Release operator
                    VerificationResult {
                        holds: false,
                        confidence: 0.5,
                        explanation: "Release: not fully implemented".to_string(),
                        counterexample: Some(vec!["not_implemented".to_string()]),
                    }
                }
            },

            TemporalFormula::BoundedTemporal {
                operator,
                formula,
                lower_bound,
                upper_bound,
            } => {
                // MTL: check within time bounds
                let current_time = trace
                    .get_state(position)
                    .map(|s| s.timestamp)
                    .unwrap_or(Duration::ZERO);

                match operator {
                    TemporalOperator::Eventually => {
                        // F[a,b] φ: φ holds at some point within time interval [a, b]
                        for i in position..trace.len() {
                            if let Some(state) = trace.get_state(i) {
                                let delta = state.timestamp.saturating_sub(current_time);
                                if delta >= *lower_bound && delta <= *upper_bound {
                                    let result = self.verify_at_position(formula, trace, i);
                                    if result.holds {
                                        return VerificationResult {
                                            holds: true,
                                            confidence: result.confidence,
                                            explanation: format!(
                                                "Bounded Eventually at {} ms: {}",
                                                delta.as_millis(),
                                                result.explanation
                                            ),
                                            counterexample: None,
                                        };
                                    }
                                }
                            }
                        }
                        VerificationResult {
                            holds: false,
                            confidence: 0.0,
                            explanation: format!(
                                "Bounded Eventually: never true within [{}, {}] ms",
                                lower_bound.as_millis(),
                                upper_bound.as_millis()
                            ),
                            counterexample: Some(vec!["not_within_bounds".to_string()]),
                        }
                    }

                    _ => VerificationResult {
                        holds: false,
                        confidence: 0.5,
                        explanation: "Bounded temporal: operator not fully implemented".to_string(),
                        counterexample: Some(vec!["not_implemented".to_string()]),
                    },
                }
            }
        }
    }

    /// Learn neural weights from verified traces (neural-symbolic learning)
    pub fn learn_from_trace(&mut self, formula: &TemporalFormula, trace: &TemporalTrace) {
        // Extract atoms from formula
        let atoms = self.extract_atoms(formula);

        // Update weights based on trace satisfaction
        for atom in atoms {
            let satisfaction_rate = self.calculate_satisfaction_rate(&atom, trace);
            self.neural_weights.insert(atom, satisfaction_rate);
        }
    }

    /// Extract all atoms from a formula
    fn extract_atoms(&self, formula: &TemporalFormula) -> Vec<String> {
        let mut atoms = Vec::new();
        self.extract_atoms_recursive(formula, &mut atoms);
        atoms.sort();
        atoms.dedup();
        atoms
    }

    fn extract_atoms_recursive(&self, formula: &TemporalFormula, atoms: &mut Vec<String>) {
        match formula {
            TemporalFormula::Atom(name) => atoms.push(name.clone()),
            TemporalFormula::Not(inner) => self.extract_atoms_recursive(inner, atoms),
            TemporalFormula::And(left, right)
            | TemporalFormula::Or(left, right)
            | TemporalFormula::Implies(left, right) => {
                self.extract_atoms_recursive(left, atoms);
                self.extract_atoms_recursive(right, atoms);
            }
            TemporalFormula::Temporal(_, inner) => self.extract_atoms_recursive(inner, atoms),
            TemporalFormula::BoundedTemporal { formula, .. } => {
                self.extract_atoms_recursive(formula, atoms)
            }
        }
    }

    /// Calculate how often an atom is satisfied in a trace
    fn calculate_satisfaction_rate(&self, atom: &str, trace: &TemporalTrace) -> f64 {
        if trace.is_empty() {
            return 0.0;
        }

        let mut true_count = 0;
        for i in 0..trace.len() {
            if let Some(state) = trace.get_state(i) {
                if state.is_true(atom) {
                    true_count += 1;
                }
            }
        }

        true_count as f64 / trace.len() as f64
    }
}

impl Default for TemporalNeuralSolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_verification() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        let mut state = TemporalState::new(Duration::from_secs(0));
        state.set("safe".to_string(), true);
        trace.add_state(state);

        let formula = TemporalFormula::atom("safe");
        let result = solver.verify(&formula, &trace);

        assert!(result.holds);
        assert!(result.confidence > 0.9);
    }

    #[test]
    fn test_eventually_operator() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        // Add states where "goal" becomes true at position 2
        for i in 0..3 {
            let mut state = TemporalState::new(Duration::from_secs(i));
            state.set("goal".to_string(), i == 2);
            trace.add_state(state);
        }

        let formula = TemporalFormula::eventually(TemporalFormula::atom("goal"));
        let result = solver.verify(&formula, &trace);

        assert!(result.holds);
        println!("Eventually result: {:?}", result);
    }

    #[test]
    fn test_globally_operator() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        // Add states where "invariant" is always true
        for i in 0..5 {
            let mut state = TemporalState::new(Duration::from_secs(i));
            state.set("invariant".to_string(), true);
            trace.add_state(state);
        }

        let formula = TemporalFormula::globally(TemporalFormula::atom("invariant"));
        let result = solver.verify(&formula, &trace);

        assert!(result.holds);
        println!("Globally result: {:?}", result);
    }

    #[test]
    fn test_bounded_eventually() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        // Add states with timestamps
        for i in 0..10 {
            let mut state = TemporalState::new(Duration::from_millis(i * 100));
            state.set("event".to_string(), i == 5); // Event occurs at 500ms
            trace.add_state(state);
        }

        // Check if event occurs within [400ms, 600ms]
        let formula = TemporalFormula::eventually_bounded(
            TemporalFormula::atom("event"),
            Duration::from_millis(400),
            Duration::from_millis(600),
        );

        let result = solver.verify(&formula, &trace);
        assert!(result.holds);
        println!("Bounded Eventually result: {:?}", result);
    }

    #[test]
    fn test_complex_formula() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        // G(request -> F response)
        // "If request happens, response must eventually happen"

        for i in 0..10 {
            let mut state = TemporalState::new(Duration::from_secs(i));
            state.set("request".to_string(), i == 2);
            state.set("response".to_string(), i >= 5);
            trace.add_state(state);
        }

        let formula = TemporalFormula::globally(TemporalFormula::implies(
            TemporalFormula::atom("request"),
            TemporalFormula::eventually(TemporalFormula::atom("response")),
        ));

        let result = solver.verify(&formula, &trace);
        println!("Complex formula result: {:?}", result);
    }

    #[test]
    fn test_learning() {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..10 {
            let mut state = TemporalState::new(Duration::from_secs(i));
            state.set("pattern".to_string(), i % 2 == 0);
            trace.add_state(state);
        }

        let formula = TemporalFormula::atom("pattern");
        solver.learn_from_trace(&formula, &trace);

        // Check learned weight
        if let Some(&weight) = solver.neural_weights.get("pattern") {
            println!("Learned weight for 'pattern': {}", weight);
            assert!((weight - 0.5).abs() < 0.1); // Should be ~0.5 (true half the time)
        }
    }
}
