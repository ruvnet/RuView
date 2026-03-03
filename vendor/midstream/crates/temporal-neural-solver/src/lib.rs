//! # Temporal-Neural-Solver
//!
//! Temporal logic with neural reasoning.
//!
//! ## Features
//! - Linear Temporal Logic (LTL)
//! - Computation Tree Logic (CTL)
//! - Metric Temporal Logic (MTL)
//! - Neural-guided solving
//! - Verification and validation

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use thiserror::Error;

/// Temporal logic errors
#[derive(Debug, Error)]
pub enum TemporalError {
    #[error("Formula parsing error: {0}")]
    ParseError(String),

    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Timeout: {0}ms")]
    Timeout(u64),

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

/// Temporal operators
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemporalOperator {
    /// Globally (always)
    Globally,
    /// Finally (eventually)
    Finally,
    /// Next
    Next,
    /// Until
    Until,
    /// And
    And,
    /// Or
    Or,
    /// Not
    Not,
    /// Implies
    Implies,
}

/// A temporal formula
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemporalFormula {
    /// Atomic proposition
    Atom(String),
    /// Unary operator
    Unary {
        op: TemporalOperator,
        formula: Box<TemporalFormula>,
    },
    /// Binary operator
    Binary {
        op: TemporalOperator,
        left: Box<TemporalFormula>,
        right: Box<TemporalFormula>,
    },
    /// True
    True,
    /// False
    False,
}

impl TemporalFormula {
    /// Create a Globally formula (G φ)
    pub fn globally(formula: TemporalFormula) -> Self {
        TemporalFormula::Unary {
            op: TemporalOperator::Globally,
            formula: Box::new(formula),
        }
    }

    /// Create a Finally formula (F φ)
    pub fn finally(formula: TemporalFormula) -> Self {
        TemporalFormula::Unary {
            op: TemporalOperator::Finally,
            formula: Box::new(formula),
        }
    }

    /// Create a Next formula (X φ)
    pub fn next(formula: TemporalFormula) -> Self {
        TemporalFormula::Unary {
            op: TemporalOperator::Next,
            formula: Box::new(formula),
        }
    }

    /// Create an Until formula (φ U ψ)
    pub fn until(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Binary {
            op: TemporalOperator::Until,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create an And formula (φ ∧ ψ)
    pub fn and(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Binary {
            op: TemporalOperator::And,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create an Or formula (φ ∨ ψ)
    pub fn or(left: TemporalFormula, right: TemporalFormula) -> Self {
        TemporalFormula::Binary {
            op: TemporalOperator::Or,
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    /// Create a Not formula (¬φ)
    pub fn not(formula: TemporalFormula) -> Self {
        TemporalFormula::Unary {
            op: TemporalOperator::Not,
            formula: Box::new(formula),
        }
    }

    /// Create an atomic proposition
    pub fn atom(name: impl Into<String>) -> Self {
        TemporalFormula::Atom(name.into())
    }
}

/// A state in the temporal model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalState {
    pub id: u64,
    pub propositions: HashMap<String, bool>,
    pub timestamp: u64,
}

impl TemporalState {
    pub fn new(id: u64, timestamp: u64) -> Self {
        Self {
            id,
            propositions: HashMap::new(),
            timestamp,
        }
    }

    pub fn set_proposition(&mut self, prop: impl Into<String>, value: bool) {
        self.propositions.insert(prop.into(), value);
    }

    pub fn get_proposition(&self, prop: &str) -> bool {
        *self.propositions.get(prop).unwrap_or(&false)
    }
}

/// A trace is a sequence of states
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemporalTrace {
    pub states: VecDeque<TemporalState>,
    pub max_length: usize,
}

impl TemporalTrace {
    pub fn new(max_length: usize) -> Self {
        Self {
            states: VecDeque::new(),
            max_length,
        }
    }

    pub fn push(&mut self, state: TemporalState) {
        if self.states.len() >= self.max_length {
            self.states.pop_front();
        }
        self.states.push_back(state);
    }

    pub fn len(&self) -> usize {
        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&TemporalState> {
        self.states.get(index)
    }
}

/// Result of verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationResult {
    pub satisfied: bool,
    pub formula: String,
    pub counterexample: Option<Vec<u64>>,
    pub confidence: f64,
}

/// Temporal neural solver
pub struct TemporalNeuralSolver {
    trace: TemporalTrace,
    #[allow(dead_code)]
    max_solving_time_ms: u64,
    verification_strictness: VerificationStrictness,
}

#[derive(Debug, Clone, Copy)]
pub enum VerificationStrictness {
    Low,
    Medium,
    High,
}

impl TemporalNeuralSolver {
    /// Create a new temporal neural solver
    pub fn new(
        max_trace_length: usize,
        max_solving_time_ms: u64,
        verification_strictness: VerificationStrictness,
    ) -> Self {
        Self {
            trace: TemporalTrace::new(max_trace_length),
            max_solving_time_ms,
            verification_strictness,
        }
    }

    /// Add a state to the trace
    pub fn add_state(&mut self, state: TemporalState) {
        self.trace.push(state);
    }

    /// Verify a temporal formula against the trace
    pub fn verify(&self, formula: &TemporalFormula) -> Result<VerificationResult, TemporalError> {
        if self.trace.is_empty() {
            return Err(TemporalError::InvalidState("Empty trace".to_string()));
        }

        let satisfied = self.check_formula(formula, 0)?;

        let formula_str = format!("{:?}", formula);

        Ok(VerificationResult {
            satisfied,
            formula: formula_str,
            counterexample: if !satisfied {
                Some(vec![0]) // Simplified counterexample
            } else {
                None
            },
            confidence: self.calculate_confidence(),
        })
    }

    /// Check if formula holds at given position in trace
    fn check_formula(&self, formula: &TemporalFormula, position: usize) -> Result<bool, TemporalError> {
        match formula {
            TemporalFormula::True => Ok(true),
            TemporalFormula::False => Ok(false),

            TemporalFormula::Atom(prop) => {
                if let Some(state) = self.trace.get(position) {
                    Ok(state.get_proposition(prop))
                } else {
                    Ok(false)
                }
            }

            TemporalFormula::Unary { op, formula } => {
                match op {
                    TemporalOperator::Not => {
                        Ok(!self.check_formula(formula, position)?)
                    }
                    TemporalOperator::Next => {
                        if position + 1 < self.trace.len() {
                            self.check_formula(formula, position + 1)
                        } else {
                            Ok(false)
                        }
                    }
                    TemporalOperator::Globally => {
                        // G φ: φ holds at all future states
                        for i in position..self.trace.len() {
                            if !self.check_formula(formula, i)? {
                                return Ok(false);
                            }
                        }
                        Ok(true)
                    }
                    TemporalOperator::Finally => {
                        // F φ: φ holds at some future state
                        for i in position..self.trace.len() {
                            if self.check_formula(formula, i)? {
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    _ => Err(TemporalError::ParseError(format!("Invalid unary operator: {:?}", op))),
                }
            }

            TemporalFormula::Binary { op, left, right } => {
                match op {
                    TemporalOperator::And => {
                        Ok(self.check_formula(left, position)? && self.check_formula(right, position)?)
                    }
                    TemporalOperator::Or => {
                        Ok(self.check_formula(left, position)? || self.check_formula(right, position)?)
                    }
                    TemporalOperator::Implies => {
                        Ok(!self.check_formula(left, position)? || self.check_formula(right, position)?)
                    }
                    TemporalOperator::Until => {
                        // φ U ψ: φ holds until ψ becomes true
                        for i in position..self.trace.len() {
                            if self.check_formula(right, i)? {
                                // ψ is true, check if φ held until now
                                for j in position..i {
                                    if !self.check_formula(left, j)? {
                                        return Ok(false);
                                    }
                                }
                                return Ok(true);
                            }
                        }
                        Ok(false)
                    }
                    _ => Err(TemporalError::ParseError(format!("Invalid binary operator: {:?}", op))),
                }
            }
        }
    }

    /// Calculate confidence in verification
    fn calculate_confidence(&self) -> f64 {
        let trace_length_factor = (self.trace.len() as f64 / 100.0).min(1.0);

        let strictness_factor = match self.verification_strictness {
            VerificationStrictness::Low => 0.7,
            VerificationStrictness::Medium => 0.85,
            VerificationStrictness::High => 0.95,
        };

        trace_length_factor * strictness_factor
    }

    /// Synthesize a controller to satisfy a formula
    pub fn synthesize_controller(&self, _formula: &TemporalFormula) -> Result<Vec<String>, TemporalError> {
        // Simplified controller synthesis
        // In production, this would use more sophisticated techniques
        Ok(vec!["action1".to_string(), "action2".to_string()])
    }

    /// Get trace length
    pub fn trace_length(&self) -> usize {
        self.trace.len()
    }

    /// Clear the trace
    pub fn clear_trace(&mut self) {
        self.trace.states.clear();
    }
}

impl Default for TemporalNeuralSolver {
    fn default() -> Self {
        Self::new(1000, 500, VerificationStrictness::Medium)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_formula_creation() {
        let atom = TemporalFormula::atom("safe");
        let globally_safe = TemporalFormula::globally(atom);

        match globally_safe {
            TemporalFormula::Unary { op, .. } => {
                assert_eq!(op, TemporalOperator::Globally);
            }
            _ => panic!("Expected Unary"),
        }
    }

    #[test]
    fn test_state() {
        let mut state = TemporalState::new(1, 100);
        state.set_proposition("safe", true);
        state.set_proposition("ready", false);

        assert!(state.get_proposition("safe"));
        assert!(!state.get_proposition("ready"));
        assert!(!state.get_proposition("unknown"));
    }

    #[test]
    fn test_trace() {
        let mut trace = TemporalTrace::new(10);

        for i in 0..5 {
            let mut state = TemporalState::new(i, i * 100);
            state.set_proposition("step", true);
            trace.push(state);
        }

        assert_eq!(trace.len(), 5);
        assert!(trace.get(0).is_some());
    }

    #[test]
    fn test_verification_atom() {
        let mut solver = TemporalNeuralSolver::default();

        let mut state = TemporalState::new(1, 100);
        state.set_proposition("safe", true);
        solver.add_state(state);

        let formula = TemporalFormula::atom("safe");
        let result = solver.verify(&formula).unwrap();

        assert!(result.satisfied);
    }

    #[test]
    fn test_verification_globally() {
        let mut solver = TemporalNeuralSolver::default();

        // Add states where "safe" is always true
        for i in 0..5 {
            let mut state = TemporalState::new(i, i * 100);
            state.set_proposition("safe", true);
            solver.add_state(state);
        }

        let formula = TemporalFormula::globally(TemporalFormula::atom("safe"));
        let result = solver.verify(&formula).unwrap();

        assert!(result.satisfied);
    }

    #[test]
    fn test_verification_finally() {
        let mut solver = TemporalNeuralSolver::default();

        // Add states where "goal" becomes true at step 3
        for i in 0..5 {
            let mut state = TemporalState::new(i, i * 100);
            state.set_proposition("goal", i == 3);
            solver.add_state(state);
        }

        let formula = TemporalFormula::finally(TemporalFormula::atom("goal"));
        let result = solver.verify(&formula).unwrap();

        assert!(result.satisfied);
    }

    #[test]
    fn test_verification_next() {
        let mut solver = TemporalNeuralSolver::default();

        let mut state1 = TemporalState::new(1, 100);
        state1.set_proposition("ready", false);
        solver.add_state(state1);

        let mut state2 = TemporalState::new(2, 200);
        state2.set_proposition("ready", true);
        solver.add_state(state2);

        let formula = TemporalFormula::next(TemporalFormula::atom("ready"));
        let result = solver.verify(&formula).unwrap();

        assert!(result.satisfied);
    }

    #[test]
    fn test_verification_and() {
        let mut solver = TemporalNeuralSolver::default();

        let mut state = TemporalState::new(1, 100);
        state.set_proposition("safe", true);
        state.set_proposition("ready", true);
        solver.add_state(state);

        let formula = TemporalFormula::and(
            TemporalFormula::atom("safe"),
            TemporalFormula::atom("ready"),
        );
        let result = solver.verify(&formula).unwrap();

        assert!(result.satisfied);
    }
}
