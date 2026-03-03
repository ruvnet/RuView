//! Linear Temporal Logic (LTL) verification
//!
//! Provides LTL formula parsing and basic verification

use crate::errors::AnalysisResult;
use std::collections::HashMap;

/// LTL formula representation
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum LTLFormula {
    /// Atomic proposition
    Atom(String),
    /// Negation (¬φ)
    Not(Box<LTLFormula>),
    /// Conjunction (φ ∧ ψ)
    And(Box<LTLFormula>, Box<LTLFormula>),
    /// Disjunction (φ ∨ ψ)
    Or(Box<LTLFormula>, Box<LTLFormula>),
    /// Globally (Gφ)
    Globally(Box<LTLFormula>),
    /// Finally (Fφ)
    Finally(Box<LTLFormula>),
}

impl LTLFormula {
    /// Parse LTL formula from string (simplified)
    pub fn parse(s: &str) -> AnalysisResult<Self> {
        let s = s.trim();

        if let Some(stripped) = s.strip_prefix("G ") {
            let inner = Self::parse(stripped)?;
            return Ok(LTLFormula::Globally(Box::new(inner)));
        }

        if let Some(stripped) = s.strip_prefix("F ") {
            let inner = Self::parse(stripped)?;
            return Ok(LTLFormula::Finally(Box::new(inner)));
        }

        // Atomic proposition
        Ok(LTLFormula::Atom(s.to_string()))
    }
}

/// Execution trace for LTL verification
#[derive(Debug, Clone)]
pub struct Trace {
    /// Sequence of propositions
    pub propositions: Vec<HashMap<String, bool>>,
}

impl Trace {
    /// Create new empty trace
    pub fn new() -> Self {
        Self {
            propositions: Vec::new(),
        }
    }

    /// Add state to trace
    pub fn add_state(&mut self, props: HashMap<String, bool>) {
        self.propositions.push(props);
    }

    /// Get length of trace
    pub fn len(&self) -> usize {
        self.propositions.len()
    }

    /// Check if trace is empty
    pub fn is_empty(&self) -> bool {
        self.propositions.is_empty()
    }
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

/// LTL model checker
pub struct LTLChecker {
    #[allow(dead_code)]
    max_depth: usize,
}

impl LTLChecker {
    /// Create new LTL checker
    pub fn new() -> Self {
        Self {
            max_depth: 100,
        }
    }

    /// Check if formula holds on trace
    pub fn check_formula(&self, formula: &LTLFormula, trace: &Trace) -> bool {
        if trace.is_empty() {
            return false;
        }

        self.check_at_position(formula, trace, 0)
    }

    #[allow(clippy::only_used_in_recursion)]
    fn check_at_position(&self, formula: &LTLFormula, trace: &Trace, pos: usize) -> bool {
        if pos >= trace.len() {
            return false;
        }

        match formula {
            LTLFormula::Atom(prop) => {
                trace.propositions[pos].get(prop).copied().unwrap_or(false)
            }
            LTLFormula::Not(f) => {
                !self.check_at_position(f, trace, pos)
            }
            LTLFormula::And(l, r) => {
                self.check_at_position(l, trace, pos) && self.check_at_position(r, trace, pos)
            }
            LTLFormula::Or(l, r) => {
                self.check_at_position(l, trace, pos) || self.check_at_position(r, trace, pos)
            }
            LTLFormula::Globally(f) => {
                (pos..trace.len()).all(|i| self.check_at_position(f, trace, i))
            }
            LTLFormula::Finally(f) => {
                (pos..trace.len()).any(|i| self.check_at_position(f, trace, i))
            }
        }
    }

    /// Generate counterexample if formula doesn't hold
    pub fn generate_counterexample(&self, formula: &LTLFormula, trace: &Trace) -> Option<Trace> {
        if self.check_formula(formula, trace) {
            return None;
        }

        // Return minimal counterexample
        let mut counterexample = Trace::new();

        for i in 0..trace.len() {
            counterexample.add_state(trace.propositions[i].clone());

            if !self.check_formula(formula, &counterexample) {
                return Some(counterexample);
            }
        }

        Some(trace.clone())
    }
}

impl Default for LTLChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_globally() {
        let formula = LTLFormula::parse("G authenticated").unwrap();
        assert!(matches!(formula, LTLFormula::Globally(_)));
    }

    #[test]
    fn test_check_atom() {
        let checker = LTLChecker::new();
        let mut trace = Trace::new();

        let mut props = HashMap::new();
        props.insert("authenticated".to_string(), true);
        trace.add_state(props);

        let formula = LTLFormula::Atom("authenticated".to_string());
        assert!(checker.check_formula(&formula, &trace));
    }
}
