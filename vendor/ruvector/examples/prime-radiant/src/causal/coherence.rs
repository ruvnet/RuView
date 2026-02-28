//! Causal Coherence Checking
//!
//! This module provides tools for verifying that beliefs and data are
//! consistent with a causal model. Key capabilities:
//!
//! - Detecting spurious correlations (associations not explained by causation)
//! - Checking if beliefs satisfy causal constraints
//! - Answering causal queries using do-calculus
//! - Computing coherence energy for integration with Prime-Radiant

use std::collections::{HashMap, HashSet};
use thiserror::Error;

use super::model::{CausalModel, CausalModelError, Value, VariableId, VariableType, Mechanism};
use super::graph::DAGValidationError;

/// Error types for coherence operations
#[derive(Debug, Clone, Error)]
pub enum CoherenceError {
    /// Model error
    #[error("Model error: {0}")]
    ModelError(#[from] CausalModelError),

    /// Graph error
    #[error("Graph error: {0}")]
    GraphError(#[from] DAGValidationError),

    /// Inconsistent belief
    #[error("Inconsistent belief: {0}")]
    InconsistentBelief(String),

    /// Invalid query
    #[error("Invalid query: {0}")]
    InvalidQuery(String),
}

/// A belief about the relationship between variables
#[derive(Debug, Clone)]
pub struct Belief {
    /// Subject variable
    pub subject: String,
    /// Object variable
    pub object: String,
    /// Type of belief
    pub belief_type: BeliefType,
    /// Confidence in the belief (0.0 to 1.0)
    pub confidence: f64,
    /// Evidence supporting the belief
    pub evidence: Option<String>,
}

/// Types of causal beliefs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeliefType {
    /// X causes Y
    Causes,
    /// X is correlated with Y (may or may not be causal)
    CorrelatedWith,
    /// X is independent of Y
    IndependentOf,
    /// X is independent of Y given Z
    ConditionallyIndependent { given: Vec<String> },
    /// X and Y have a common cause
    CommonCause,
    /// Changing X would change Y (interventional)
    WouldChange,
}

impl Belief {
    /// Create a causal belief: X causes Y
    pub fn causes(x: &str, y: &str) -> Self {
        Self {
            subject: x.to_string(),
            object: y.to_string(),
            belief_type: BeliefType::Causes,
            confidence: 1.0,
            evidence: None,
        }
    }

    /// Create a correlation belief
    pub fn correlated(x: &str, y: &str) -> Self {
        Self {
            subject: x.to_string(),
            object: y.to_string(),
            belief_type: BeliefType::CorrelatedWith,
            confidence: 1.0,
            evidence: None,
        }
    }

    /// Create an independence belief
    pub fn independent(x: &str, y: &str) -> Self {
        Self {
            subject: x.to_string(),
            object: y.to_string(),
            belief_type: BeliefType::IndependentOf,
            confidence: 1.0,
            evidence: None,
        }
    }

    /// Create a conditional independence belief
    pub fn conditionally_independent(x: &str, y: &str, given: &[&str]) -> Self {
        Self {
            subject: x.to_string(),
            object: y.to_string(),
            belief_type: BeliefType::ConditionallyIndependent {
                given: given.iter().map(|s| s.to_string()).collect(),
            },
            confidence: 1.0,
            evidence: None,
        }
    }

    /// Set confidence level
    pub fn with_confidence(mut self, confidence: f64) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set evidence
    pub fn with_evidence(mut self, evidence: &str) -> Self {
        self.evidence = Some(evidence.to_string());
        self
    }
}

/// Result of causal consistency checking
#[derive(Debug, Clone)]
pub struct CausalConsistency {
    /// Overall consistency score (0.0 to 1.0)
    pub score: f64,
    /// Number of beliefs checked
    pub beliefs_checked: usize,
    /// Number of consistent beliefs
    pub consistent_beliefs: usize,
    /// Number of inconsistent beliefs
    pub inconsistent_beliefs: usize,
    /// Details of inconsistencies
    pub inconsistencies: Vec<Inconsistency>,
    /// Suggested model modifications
    pub suggestions: Vec<String>,
}

impl CausalConsistency {
    /// Create a fully consistent result
    pub fn fully_consistent(beliefs_checked: usize) -> Self {
        Self {
            score: 1.0,
            beliefs_checked,
            consistent_beliefs: beliefs_checked,
            inconsistent_beliefs: 0,
            inconsistencies: vec![],
            suggestions: vec![],
        }
    }

    /// Check if fully consistent
    pub fn is_consistent(&self) -> bool {
        self.score >= 1.0 - 1e-10
    }
}

/// Details of a causal inconsistency
#[derive(Debug, Clone)]
pub struct Inconsistency {
    /// The belief that is inconsistent
    pub belief: Belief,
    /// Why it's inconsistent
    pub reason: String,
    /// Severity (0.0 to 1.0)
    pub severity: f64,
}

/// A detected spurious correlation
#[derive(Debug, Clone)]
pub struct SpuriousCorrelation {
    /// First variable
    pub var_a: String,
    /// Second variable
    pub var_b: String,
    /// The common cause(s) explaining the correlation
    pub confounders: Vec<String>,
    /// Strength of the spurious correlation
    pub strength: f64,
    /// Explanation
    pub explanation: String,
}

/// A causal query
#[derive(Debug, Clone)]
pub struct CausalQuery {
    /// The variable we're asking about
    pub target: String,
    /// Variables we're intervening on
    pub interventions: Vec<(String, Value)>,
    /// Variables we're conditioning on
    pub conditions: Vec<(String, Value)>,
    /// Query type
    pub query_type: QueryType,
}

/// Types of causal queries
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryType {
    /// P(Y | do(X=x)) - interventional query
    Interventional,
    /// P(Y | X=x) - observational query
    Observational,
    /// P(Y_x | X=x') - counterfactual query
    Counterfactual,
    /// P(Y | do(X=x), Z=z) - conditional interventional
    ConditionalInterventional,
}

impl CausalQuery {
    /// Create an interventional query: P(target | do(intervention))
    pub fn interventional(target: &str, intervention_var: &str, intervention_val: Value) -> Self {
        Self {
            target: target.to_string(),
            interventions: vec![(intervention_var.to_string(), intervention_val)],
            conditions: vec![],
            query_type: QueryType::Interventional,
        }
    }

    /// Create an observational query: P(target | condition)
    pub fn observational(target: &str, condition_var: &str, condition_val: Value) -> Self {
        Self {
            target: target.to_string(),
            interventions: vec![],
            conditions: vec![(condition_var.to_string(), condition_val)],
            query_type: QueryType::Observational,
        }
    }

    /// Add a condition
    pub fn given(mut self, var: &str, val: Value) -> Self {
        self.conditions.push((var.to_string(), val));
        self
    }
}

/// Answer to a causal query
#[derive(Debug, Clone)]
pub struct CausalAnswer {
    /// The query that was answered
    pub query: CausalQuery,
    /// The estimated value/distribution
    pub estimate: Value,
    /// Confidence interval (if applicable)
    pub confidence_interval: Option<(f64, f64)>,
    /// Whether the query is identifiable from observational data
    pub is_identifiable: bool,
    /// Explanation of the answer
    pub explanation: String,
}

/// Combined coherence energy for integration with Prime-Radiant
#[derive(Debug, Clone)]
pub struct CoherenceEnergy {
    /// Total energy (lower is more coherent)
    pub total: f64,
    /// Structural component (from sheaf consistency)
    pub structural_component: f64,
    /// Causal component (from causal consistency)
    pub causal_component: f64,
    /// Intervention component (from intervention consistency)
    pub intervention_component: f64,
    /// Whether the system is coherent (energy below threshold)
    pub is_coherent: bool,
}

impl CoherenceEnergy {
    /// Create a fully coherent state
    pub fn coherent() -> Self {
        Self {
            total: 0.0,
            structural_component: 0.0,
            causal_component: 0.0,
            intervention_component: 0.0,
            is_coherent: true,
        }
    }

    /// Create from individual components
    pub fn from_components(structural: f64, causal: f64, intervention: f64) -> Self {
        let total = structural + causal + intervention;
        Self {
            total,
            structural_component: structural,
            causal_component: causal,
            intervention_component: intervention,
            is_coherent: total < 1e-6,
        }
    }
}

/// Dataset for spurious correlation detection
#[derive(Debug, Clone)]
pub struct Dataset {
    /// Column names
    pub columns: Vec<String>,
    /// Data rows (each row is a vector of values)
    pub rows: Vec<Vec<f64>>,
}

impl Dataset {
    /// Create a new dataset
    pub fn new(columns: Vec<String>) -> Self {
        Self {
            columns,
            rows: Vec::new(),
        }
    }

    /// Add a row
    pub fn add_row(&mut self, row: Vec<f64>) {
        if row.len() == self.columns.len() {
            self.rows.push(row);
        }
    }

    /// Get column index
    pub fn column_index(&self, name: &str) -> Option<usize> {
        self.columns.iter().position(|c| c == name)
    }

    /// Get column values
    pub fn column(&self, name: &str) -> Option<Vec<f64>> {
        let idx = self.column_index(name)?;
        Some(self.rows.iter().map(|row| row[idx]).collect())
    }

    /// Compute correlation between two columns
    pub fn correlation(&self, col_a: &str, col_b: &str) -> Option<f64> {
        let a = self.column(col_a)?;
        let b = self.column(col_b)?;

        if a.len() != b.len() || a.is_empty() {
            return None;
        }

        let n = a.len() as f64;
        let mean_a: f64 = a.iter().sum::<f64>() / n;
        let mean_b: f64 = b.iter().sum::<f64>() / n;

        let mut cov = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;

        for i in 0..a.len() {
            let da = a[i] - mean_a;
            let db = b[i] - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        let denom = (var_a * var_b).sqrt();
        if denom < 1e-10 {
            Some(0.0)
        } else {
            Some(cov / denom)
        }
    }
}

/// Causal coherence checker
pub struct CausalCoherenceChecker<'a> {
    /// The causal model
    model: &'a CausalModel,
    /// Correlation threshold for "significant" correlation
    correlation_threshold: f64,
}

impl<'a> CausalCoherenceChecker<'a> {
    /// Create a new checker
    pub fn new(model: &'a CausalModel) -> Self {
        Self {
            model,
            correlation_threshold: 0.1,
        }
    }

    /// Set correlation threshold
    pub fn with_correlation_threshold(mut self, threshold: f64) -> Self {
        self.correlation_threshold = threshold;
        self
    }

    /// Check if a set of beliefs is consistent with the causal model
    pub fn check_causal_consistency(&self, beliefs: &[Belief]) -> CausalConsistency {
        let mut consistent_count = 0;
        let mut inconsistencies = Vec::new();
        let mut suggestions = Vec::new();

        for belief in beliefs {
            match self.check_single_belief(belief) {
                Ok(()) => consistent_count += 1,
                Err(reason) => {
                    inconsistencies.push(Inconsistency {
                        belief: belief.clone(),
                        reason: reason.clone(),
                        severity: 1.0 - belief.confidence,
                    });

                    // Generate suggestion
                    if let Some(suggestion) = self.generate_suggestion(belief, &reason) {
                        suggestions.push(suggestion);
                    }
                }
            }
        }

        let beliefs_checked = beliefs.len();
        let score = if beliefs_checked > 0 {
            consistent_count as f64 / beliefs_checked as f64
        } else {
            1.0
        };

        CausalConsistency {
            score,
            beliefs_checked,
            consistent_beliefs: consistent_count,
            inconsistent_beliefs: beliefs_checked - consistent_count,
            inconsistencies,
            suggestions,
        }
    }

    /// Check a single belief against the model
    fn check_single_belief(&self, belief: &Belief) -> Result<(), String> {
        let subject_id = self.model.get_variable_id(&belief.subject)
            .ok_or_else(|| format!("Variable '{}' not in model", belief.subject))?;
        let object_id = self.model.get_variable_id(&belief.object)
            .ok_or_else(|| format!("Variable '{}' not in model", belief.object))?;

        match &belief.belief_type {
            BeliefType::Causes => {
                // Check if there's a directed path from subject to object
                let descendants = self.model.graph().descendants(subject_id.0);
                if !descendants.contains(&object_id.0) {
                    return Err(format!(
                        "No causal path from {} to {} in model",
                        belief.subject, belief.object
                    ));
                }
            }

            BeliefType::IndependentOf => {
                // Check if they're d-separated given empty set
                if !self.model.d_separated(subject_id, object_id, &[]) {
                    return Err(format!(
                        "{} and {} are not independent according to model",
                        belief.subject, belief.object
                    ));
                }
            }

            BeliefType::ConditionallyIndependent { given } => {
                let given_ids: Result<Vec<VariableId>, _> = given.iter()
                    .map(|name| {
                        self.model.get_variable_id(name)
                            .ok_or_else(|| format!("Variable '{}' not in model", name))
                    })
                    .collect();
                let given_ids = given_ids?;

                if !self.model.d_separated(subject_id, object_id, &given_ids) {
                    return Err(format!(
                        "{} and {} are not conditionally independent given {:?}",
                        belief.subject, belief.object, given
                    ));
                }
            }

            BeliefType::CommonCause => {
                // Check if they share a common ancestor
                let ancestors_a = self.model.graph().ancestors(subject_id.0);
                let ancestors_b = self.model.graph().ancestors(object_id.0);

                let common: HashSet<_> = ancestors_a.intersection(&ancestors_b).collect();
                if common.is_empty() {
                    return Err(format!(
                        "No common cause found for {} and {}",
                        belief.subject, belief.object
                    ));
                }
            }

            BeliefType::CorrelatedWith | BeliefType::WouldChange => {
                // These are not directly checkable against model structure alone
                // They would need data or simulation
            }
        }

        Ok(())
    }

    /// Generate a suggestion for fixing an inconsistency
    fn generate_suggestion(&self, belief: &Belief, _reason: &str) -> Option<String> {
        match &belief.belief_type {
            BeliefType::Causes => {
                Some(format!(
                    "Consider adding edge {} -> {} to the model, or revising the belief",
                    belief.subject, belief.object
                ))
            }
            BeliefType::IndependentOf => {
                Some(format!(
                    "Consider conditioning on a confounding variable, or revising the model structure"
                ))
            }
            BeliefType::ConditionallyIndependent { given } => {
                Some(format!(
                    "The conditioning set {:?} may be insufficient; consider additional variables",
                    given
                ))
            }
            _ => None,
        }
    }

    /// Detect spurious correlations in data given the causal model
    pub fn detect_spurious_correlations(&self, data: &Dataset) -> Vec<SpuriousCorrelation> {
        let mut spurious = Vec::new();

        // Check all pairs of variables
        for i in 0..data.columns.len() {
            for j in (i + 1)..data.columns.len() {
                let col_a = &data.columns[i];
                let col_b = &data.columns[j];

                // Get correlation from data
                let correlation = match data.correlation(col_a, col_b) {
                    Some(c) => c,
                    None => continue,
                };

                // If significantly correlated
                if correlation.abs() > self.correlation_threshold {
                    // Check if causally linked
                    if let (Some(id_a), Some(id_b)) = (
                        self.model.get_variable_id(col_a),
                        self.model.get_variable_id(col_b),
                    ) {
                        // Check if there's a direct causal path
                        let a_causes_b = self.model.graph().descendants(id_a.0).contains(&id_b.0);
                        let b_causes_a = self.model.graph().descendants(id_b.0).contains(&id_a.0);

                        if !a_causes_b && !b_causes_a {
                            // Correlation without direct causation - find confounders
                            let confounders = self.find_confounders(id_a, id_b);

                            if !confounders.is_empty() {
                                spurious.push(SpuriousCorrelation {
                                    var_a: col_a.clone(),
                                    var_b: col_b.clone(),
                                    confounders: confounders.clone(),
                                    strength: correlation.abs(),
                                    explanation: format!(
                                        "Correlation (r={:.3}) between {} and {} is explained by common cause(s): {}",
                                        correlation, col_a, col_b, confounders.join(", ")
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        spurious
    }

    /// Find common causes (confounders) of two variables
    fn find_confounders(&self, a: VariableId, b: VariableId) -> Vec<String> {
        let ancestors_a = self.model.graph().ancestors(a.0);
        let ancestors_b = self.model.graph().ancestors(b.0);

        let common: Vec<_> = ancestors_a.intersection(&ancestors_b)
            .filter_map(|&id| self.model.get_variable_name(&VariableId(id)))
            .collect();

        common
    }

    /// Answer a causal query using do-calculus
    pub fn enforce_do_calculus(&self, query: &CausalQuery) -> Result<CausalAnswer, CoherenceError> {
        // Get target variable
        let target_id = self.model.get_variable_id(&query.target)
            .ok_or_else(|| CoherenceError::InvalidQuery(
                format!("Target variable '{}' not in model", query.target)
            ))?;

        match query.query_type {
            QueryType::Interventional => {
                self.answer_interventional_query(query, target_id)
            }
            QueryType::Observational => {
                self.answer_observational_query(query, target_id)
            }
            QueryType::Counterfactual => {
                self.answer_counterfactual_query(query, target_id)
            }
            QueryType::ConditionalInterventional => {
                self.answer_conditional_interventional_query(query, target_id)
            }
        }
    }

    fn answer_interventional_query(
        &self,
        query: &CausalQuery,
        target_id: VariableId,
    ) -> Result<CausalAnswer, CoherenceError> {
        // Convert intervention specification to Intervention objects
        let interventions: Result<Vec<_>, _> = query.interventions.iter()
            .map(|(var, val)| {
                self.model.get_variable_id(var)
                    .map(|id| super::model::Intervention::new(id, val.clone()))
                    .ok_or_else(|| CoherenceError::InvalidQuery(
                        format!("Intervention variable '{}' not in model", var)
                    ))
            })
            .collect();
        let interventions = interventions?;

        // Perform intervention
        let intervened = self.model.intervene_with(&interventions)?;

        // Simulate to get the target value
        let values = intervened.simulate(&HashMap::new())?;

        let estimate = values.get(&target_id).cloned().unwrap_or(Value::Missing);

        // Check identifiability
        let is_identifiable = self.check_identifiability(query);

        Ok(CausalAnswer {
            query: query.clone(),
            estimate,
            confidence_interval: None,
            is_identifiable,
            explanation: format!(
                "Computed P({} | do({})) by intervention simulation",
                query.target,
                query.interventions.iter()
                    .map(|(v, val)| format!("{}={:?}", v, val))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        })
    }

    fn answer_observational_query(
        &self,
        query: &CausalQuery,
        target_id: VariableId,
    ) -> Result<CausalAnswer, CoherenceError> {
        // For observational queries, we need to condition
        // This requires probabilistic reasoning which we approximate

        let explanation = format!(
            "Observational query P({} | {}) - requires probabilistic inference",
            query.target,
            query.conditions.iter()
                .map(|(v, val)| format!("{}={:?}", v, val))
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(CausalAnswer {
            query: query.clone(),
            estimate: Value::Missing, // Would need actual probabilistic computation
            confidence_interval: None,
            is_identifiable: true, // Observational queries are always identifiable
            explanation,
        })
    }

    fn answer_counterfactual_query(
        &self,
        query: &CausalQuery,
        _target_id: VariableId,
    ) -> Result<CausalAnswer, CoherenceError> {
        // Counterfactual queries require abduction-action-prediction
        let explanation = format!(
            "Counterfactual query for {} - requires three-step process: abduction, action, prediction",
            query.target
        );

        Ok(CausalAnswer {
            query: query.clone(),
            estimate: Value::Missing,
            confidence_interval: None,
            is_identifiable: false, // Counterfactuals often not identifiable
            explanation,
        })
    }

    fn answer_conditional_interventional_query(
        &self,
        query: &CausalQuery,
        target_id: VariableId,
    ) -> Result<CausalAnswer, CoherenceError> {
        // Combines intervention with conditioning
        let explanation = format!(
            "Conditional interventional query P({} | do({}), {}) - may require adjustment formula",
            query.target,
            query.interventions.iter()
                .map(|(v, val)| format!("{}={:?}", v, val))
                .collect::<Vec<_>>()
                .join(", "),
            query.conditions.iter()
                .map(|(v, val)| format!("{}={:?}", v, val))
                .collect::<Vec<_>>()
                .join(", ")
        );

        Ok(CausalAnswer {
            query: query.clone(),
            estimate: Value::Missing,
            confidence_interval: None,
            is_identifiable: self.check_identifiability(query),
            explanation,
        })
    }

    /// Check if a causal query is identifiable from observational data
    fn check_identifiability(&self, query: &CausalQuery) -> bool {
        // Simplified identifiability check
        // Full implementation would use do-calculus rules

        if query.interventions.is_empty() {
            return true; // Observational queries are identifiable
        }

        // Check if intervention variables have unobserved confounders with target
        for (var, _) in &query.interventions {
            if let (Some(var_id), Some(target_id)) = (
                self.model.get_variable_id(var),
                self.model.get_variable_id(&query.target),
            ) {
                // If there's a backdoor path that can't be blocked, not identifiable
                // This is a simplified check
                let var_ancestors = self.model.graph().ancestors(var_id.0);
                let target_ancestors = self.model.graph().ancestors(target_id.0);

                // If they share unobserved common ancestors, might not be identifiable
                let common = var_ancestors.intersection(&target_ancestors).count();
                if common > 0 && !self.has_valid_adjustment_set(var_id, target_id) {
                    return false;
                }
            }
        }

        true
    }

    /// Check if there's a valid adjustment set for identifying causal effect
    fn has_valid_adjustment_set(&self, treatment: VariableId, outcome: VariableId) -> bool {
        // Check backdoor criterion
        // A set Z satisfies backdoor criterion if:
        // 1. No node in Z is a descendant of X
        // 2. Z blocks every path from X to Y that contains an arrow into X

        let descendants = self.model.graph().descendants(treatment.0);

        // Try the set of all non-descendants as potential adjustment set
        let all_vars: Vec<_> = self.model.variables()
            .filter(|v| v.id != treatment && v.id != outcome)
            .filter(|v| !descendants.contains(&v.id.0))
            .map(|v| v.id)
            .collect();

        // Check if conditioning on all non-descendants blocks backdoor paths
        self.model.d_separated(treatment, outcome, &all_vars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal::model::{CausalModelBuilder, VariableType};

    fn create_test_model() -> CausalModel {
        let mut model = CausalModel::with_name("Test");

        model.add_variable("Age", VariableType::Continuous).unwrap();
        model.add_variable("Education", VariableType::Continuous).unwrap();
        model.add_variable("Income", VariableType::Continuous).unwrap();
        model.add_variable("Health", VariableType::Continuous).unwrap();

        let age = model.get_variable_id("Age").unwrap();
        let edu = model.get_variable_id("Education").unwrap();
        let income = model.get_variable_id("Income").unwrap();
        let health = model.get_variable_id("Health").unwrap();

        // Age -> Education, Age -> Health
        model.add_edge(age, edu).unwrap();
        model.add_edge(age, health).unwrap();

        // Education -> Income
        model.add_edge(edu, income).unwrap();

        // Add equations
        model.add_structural_equation(edu, &[age], Mechanism::new(|p| {
            Value::Continuous(12.0 + p[0].as_f64() * 0.1)
        })).unwrap();

        model.add_structural_equation(income, &[edu], Mechanism::new(|p| {
            Value::Continuous(30000.0 + p[0].as_f64() * 5000.0)
        })).unwrap();

        model.add_structural_equation(health, &[age], Mechanism::new(|p| {
            Value::Continuous(100.0 - p[0].as_f64() * 0.5)
        })).unwrap();

        model
    }

    #[test]
    fn test_belief_creation() {
        let belief = Belief::causes("Age", "Education").with_confidence(0.9);
        assert_eq!(belief.subject, "Age");
        assert_eq!(belief.object, "Education");
        assert_eq!(belief.confidence, 0.9);
    }

    #[test]
    fn test_causal_consistency() {
        let model = create_test_model();
        let checker = CausalCoherenceChecker::new(&model);

        let beliefs = vec![
            Belief::causes("Age", "Education"),
            Belief::causes("Education", "Income"),
        ];

        let result = checker.check_causal_consistency(&beliefs);

        assert!(result.is_consistent());
        assert_eq!(result.consistent_beliefs, 2);
    }

    #[test]
    fn test_inconsistent_belief() {
        let model = create_test_model();
        let checker = CausalCoherenceChecker::new(&model);

        let beliefs = vec![
            Belief::causes("Income", "Age"), // Wrong direction
        ];

        let result = checker.check_causal_consistency(&beliefs);

        assert!(!result.is_consistent());
        assert_eq!(result.inconsistent_beliefs, 1);
    }

    #[test]
    fn test_conditional_independence() {
        let model = create_test_model();
        let checker = CausalCoherenceChecker::new(&model);

        // Education and Health should be independent given Age
        let beliefs = vec![
            Belief::conditionally_independent("Education", "Health", &["Age"]),
        ];

        let result = checker.check_causal_consistency(&beliefs);

        assert!(result.is_consistent());
    }

    #[test]
    fn test_spurious_correlation_detection() {
        let model = create_test_model();
        let checker = CausalCoherenceChecker::new(&model).with_correlation_threshold(0.1);

        // Create dataset with Education-Health correlation (spurious via Age)
        let mut data = Dataset::new(vec![
            "Age".to_string(),
            "Education".to_string(),
            "Health".to_string(),
        ]);

        // Add correlated data
        for i in 0..100 {
            let age = 20.0 + i as f64 * 0.5;
            let edu = 12.0 + age * 0.1 + (i as f64 * 0.1).sin();
            let health = 100.0 - age * 0.5 + (i as f64 * 0.2).cos();
            data.add_row(vec![age, edu, health]);
        }

        let spurious = checker.detect_spurious_correlations(&data);

        // Should detect Education-Health as spurious (both caused by Age)
        let edu_health = spurious.iter()
            .find(|s| (s.var_a == "Education" && s.var_b == "Health") ||
                      (s.var_a == "Health" && s.var_b == "Education"));

        assert!(edu_health.is_some());

        if let Some(s) = edu_health {
            assert!(s.confounders.contains(&"Age".to_string()));
        }
    }

    #[test]
    fn test_interventional_query() {
        let model = create_test_model();
        let checker = CausalCoherenceChecker::new(&model);

        let query = CausalQuery::interventional(
            "Income",
            "Education",
            Value::Continuous(16.0),
        );

        let answer = checker.enforce_do_calculus(&query).unwrap();

        assert!(answer.is_identifiable);
        assert!(matches!(answer.query.query_type, QueryType::Interventional));
    }

    #[test]
    fn test_coherence_energy() {
        let energy = CoherenceEnergy::from_components(0.1, 0.2, 0.05);

        assert!((energy.total - 0.35).abs() < 1e-10);
        assert!(!energy.is_coherent);

        let coherent = CoherenceEnergy::coherent();
        assert!(coherent.is_coherent);
    }

    #[test]
    fn test_dataset_correlation() {
        let mut data = Dataset::new(vec!["X".to_string(), "Y".to_string()]);

        // Perfect positive correlation
        for i in 0..10 {
            data.add_row(vec![i as f64, i as f64]);
        }

        let corr = data.correlation("X", "Y").unwrap();
        assert!((corr - 1.0).abs() < 1e-10);

        // Add negatively correlated data
        let mut data2 = Dataset::new(vec!["A".to_string(), "B".to_string()]);
        for i in 0..10 {
            data2.add_row(vec![i as f64, (10 - i) as f64]);
        }

        let corr2 = data2.correlation("A", "B").unwrap();
        assert!((corr2 + 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_causal_query_builder() {
        let query = CausalQuery::interventional("Y", "X", Value::Continuous(1.0))
            .given("Z", Value::Continuous(2.0));

        assert_eq!(query.target, "Y");
        assert_eq!(query.interventions.len(), 1);
        assert_eq!(query.conditions.len(), 1);
    }
}
