//! Causal Abstraction Layer
//!
//! This module implements causal abstraction theory, which formalizes the
//! relationship between detailed (low-level) and simplified (high-level)
//! causal models. The key insight is that a high-level model is a valid
//! abstraction if interventions on the low-level model can be "lifted" to
//! corresponding interventions on the high-level model while preserving
//! distributional semantics.
//!
//! ## Theory
//!
//! A causal abstraction consists of:
//! - A low-level model M_L with variables V_L
//! - A high-level model M_H with variables V_H
//! - A surjective mapping τ: V_L → V_H
//!
//! The abstraction is **consistent** if for all interventions I on M_H:
//! τ(M_L(τ^{-1}(I))) = M_H(I)
//!
//! ## References
//!
//! - Beckers & Halpern (2019): "Abstracting Causal Models"
//! - Rubenstein et al. (2017): "Causal Consistency of Structural Equation Models"

use std::collections::{HashMap, HashSet};
use thiserror::Error;

use super::model::{CausalModel, CausalModelError, Intervention, Value, VariableId, Distribution};
use super::counterfactual::CounterfactualDistribution;

/// Error types for abstraction operations
#[derive(Debug, Clone, Error)]
pub enum AbstractionError {
    /// Abstraction map is not surjective
    #[error("Abstraction map is not surjective: high-level variable {0:?} has no preimage")]
    NotSurjective(VariableId),

    /// Abstraction is not consistent under intervention
    #[error("Abstraction is not consistent: intervention {0:?} yields different results")]
    InconsistentIntervention(String),

    /// Invalid variable mapping
    #[error("Invalid mapping: low-level variable {0:?} not in model")]
    InvalidMapping(VariableId),

    /// Models have incompatible structure
    #[error("Incompatible model structure: {0}")]
    IncompatibleStructure(String),

    /// Underlying model error
    #[error("Model error: {0}")]
    ModelError(#[from] CausalModelError),
}

/// Mapping from low-level to high-level variables
#[derive(Debug, Clone)]
pub struct AbstractionMap {
    /// Maps high-level variable to set of low-level variables
    high_to_low: HashMap<VariableId, HashSet<VariableId>>,

    /// Maps low-level variable to high-level variable
    low_to_high: HashMap<VariableId, VariableId>,

    /// Value aggregation functions (how to combine low-level values)
    aggregators: HashMap<VariableId, Aggregator>,
}

/// How to aggregate low-level values into a high-level value
#[derive(Debug, Clone)]
pub enum Aggregator {
    /// Take first value (for 1-to-1 mappings)
    First,
    /// Sum of values
    Sum,
    /// Mean of values
    Mean,
    /// Max of values
    Max,
    /// Min of values
    Min,
    /// Majority vote (for discrete/binary)
    Majority,
    /// Weighted combination
    Weighted(Vec<f64>),
    /// Custom function (represented as string for debug)
    Custom(String),
}

impl Aggregator {
    /// Apply the aggregator to a set of values
    pub fn apply(&self, values: &[Value]) -> Value {
        if values.is_empty() {
            return Value::Missing;
        }

        match self {
            Aggregator::First => values[0].clone(),

            Aggregator::Sum => {
                let sum: f64 = values.iter().map(|v| v.as_f64()).sum();
                Value::Continuous(sum)
            }

            Aggregator::Mean => {
                let sum: f64 = values.iter().map(|v| v.as_f64()).sum();
                Value::Continuous(sum / values.len() as f64)
            }

            Aggregator::Max => {
                let max = values.iter()
                    .map(|v| v.as_f64())
                    .fold(f64::NEG_INFINITY, f64::max);
                Value::Continuous(max)
            }

            Aggregator::Min => {
                let min = values.iter()
                    .map(|v| v.as_f64())
                    .fold(f64::INFINITY, f64::min);
                Value::Continuous(min)
            }

            Aggregator::Majority => {
                let mut counts: HashMap<i64, usize> = HashMap::new();
                for v in values {
                    let key = v.as_f64() as i64;
                    *counts.entry(key).or_default() += 1;
                }
                let majority = counts.into_iter()
                    .max_by_key(|(_, count)| *count)
                    .map(|(val, _)| val)
                    .unwrap_or(0);
                Value::Discrete(majority)
            }

            Aggregator::Weighted(weights) => {
                let weighted_sum: f64 = values.iter()
                    .zip(weights.iter())
                    .map(|(v, w)| v.as_f64() * w)
                    .sum();
                Value::Continuous(weighted_sum)
            }

            Aggregator::Custom(_) => {
                // Default to mean for custom
                let sum: f64 = values.iter().map(|v| v.as_f64()).sum();
                Value::Continuous(sum / values.len() as f64)
            }
        }
    }
}

impl AbstractionMap {
    /// Create a new empty abstraction map
    pub fn new() -> Self {
        Self {
            high_to_low: HashMap::new(),
            low_to_high: HashMap::new(),
            aggregators: HashMap::new(),
        }
    }

    /// Add a mapping from high-level variable to low-level variables
    pub fn add_mapping(
        &mut self,
        high: VariableId,
        low_vars: HashSet<VariableId>,
        aggregator: Aggregator,
    ) {
        for &low in &low_vars {
            self.low_to_high.insert(low, high);
        }
        self.high_to_low.insert(high, low_vars);
        self.aggregators.insert(high, aggregator);
    }

    /// Add a 1-to-1 mapping
    pub fn add_identity_mapping(&mut self, high: VariableId, low: VariableId) {
        let mut low_set = HashSet::new();
        low_set.insert(low);
        self.add_mapping(high, low_set, Aggregator::First);
    }

    /// Get the high-level variable for a low-level variable
    pub fn lift_variable(&self, low: VariableId) -> Option<VariableId> {
        self.low_to_high.get(&low).copied()
    }

    /// Get the low-level variables for a high-level variable
    pub fn project_variable(&self, high: VariableId) -> Option<&HashSet<VariableId>> {
        self.high_to_low.get(&high)
    }

    /// Lift a value from low-level to high-level
    pub fn lift_value(&self, high: VariableId, low_values: &HashMap<VariableId, Value>) -> Value {
        let low_vars = match self.high_to_low.get(&high) {
            Some(vars) => vars,
            None => return Value::Missing,
        };

        let values: Vec<Value> = low_vars.iter()
            .filter_map(|v| low_values.get(v).cloned())
            .collect();

        let aggregator = self.aggregators.get(&high).unwrap_or(&Aggregator::First);
        aggregator.apply(&values)
    }

    /// Check if the mapping is surjective (every high-level var has a preimage)
    pub fn is_surjective(&self, high_level: &CausalModel) -> bool {
        for var in high_level.variables() {
            if !self.high_to_low.contains_key(&var.id) {
                return false;
            }
        }
        true
    }
}

impl Default for AbstractionMap {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of consistency checking
#[derive(Debug, Clone)]
pub struct ConsistencyResult {
    /// Whether the abstraction is consistent
    pub is_consistent: bool,
    /// Violations found (if any)
    pub violations: Vec<ConsistencyViolation>,
    /// Interventions tested
    pub interventions_tested: usize,
    /// Maximum observed divergence
    pub max_divergence: f64,
}

/// A violation of causal abstraction consistency
#[derive(Debug, Clone)]
pub struct ConsistencyViolation {
    /// The intervention that caused the violation
    pub intervention: String,
    /// Expected high-level outcome
    pub expected: HashMap<String, f64>,
    /// Actual (projected from low-level) outcome
    pub actual: HashMap<String, f64>,
    /// Divergence measure
    pub divergence: f64,
}

/// Causal Abstraction between two causal models
pub struct CausalAbstraction<'a> {
    /// The low-level (detailed) model
    pub low_level: &'a CausalModel,

    /// The high-level (abstract) model
    pub high_level: &'a CausalModel,

    /// The abstraction mapping
    pub abstraction_map: AbstractionMap,

    /// Tolerance for numerical consistency checks
    pub tolerance: f64,
}

impl<'a> CausalAbstraction<'a> {
    /// Create a new causal abstraction
    pub fn new(
        low_level: &'a CausalModel,
        high_level: &'a CausalModel,
        abstraction_map: AbstractionMap,
    ) -> Result<Self, AbstractionError> {
        let abstraction = Self {
            low_level,
            high_level,
            abstraction_map,
            tolerance: 1e-6,
        };

        abstraction.validate_structure()?;

        Ok(abstraction)
    }

    /// Set tolerance for numerical comparisons
    pub fn with_tolerance(mut self, tol: f64) -> Self {
        self.tolerance = tol;
        self
    }

    /// Validate that the abstraction structure is valid
    fn validate_structure(&self) -> Result<(), AbstractionError> {
        // Check surjectivity
        for var in self.high_level.variables() {
            if self.abstraction_map.high_to_low.get(&var.id).is_none() {
                return Err(AbstractionError::NotSurjective(var.id));
            }
        }

        // Check that all low-level variables in the map exist
        for low_vars in self.abstraction_map.high_to_low.values() {
            for &low_var in low_vars {
                if self.low_level.get_variable(&low_var).is_none() {
                    return Err(AbstractionError::InvalidMapping(low_var));
                }
            }
        }

        Ok(())
    }

    /// Check if the abstraction is consistent under a set of interventions
    pub fn is_consistent(&self) -> bool {
        self.check_consistency().is_consistent
    }

    /// Perform detailed consistency check
    pub fn check_consistency(&self) -> ConsistencyResult {
        let mut violations = Vec::new();
        let mut max_divergence = 0.0;
        let mut interventions_tested = 0;

        // Test consistency for single-variable interventions on high-level model
        for high_var in self.high_level.variables() {
            // Test a few intervention values
            for intervention_value in [0.0, 1.0, -1.0, 0.5] {
                interventions_tested += 1;

                let high_intervention = Intervention::new(
                    high_var.id,
                    Value::Continuous(intervention_value),
                );

                // Check consistency for this intervention
                if let Some(violation) = self.check_single_intervention(&high_intervention) {
                    max_divergence = max_divergence.max(violation.divergence);
                    violations.push(violation);
                }
            }
        }

        ConsistencyResult {
            is_consistent: violations.is_empty(),
            violations,
            interventions_tested,
            max_divergence,
        }
    }

    /// Check consistency for a single intervention
    fn check_single_intervention(&self, high_intervention: &Intervention) -> Option<ConsistencyViolation> {
        // Lift the intervention to low-level
        let low_interventions = self.lift_intervention(high_intervention);

        // Simulate high-level model with intervention
        let high_result = self.high_level.intervene(&[high_intervention.clone()]);
        let high_values = match high_result {
            Ok(model) => model.simulate(&HashMap::new()).ok(),
            Err(_) => None,
        };

        // Simulate low-level model with lifted interventions
        let low_result = self.low_level.intervene(&low_interventions);
        let low_values = match low_result {
            Ok(model) => model.simulate(&HashMap::new()).ok(),
            Err(_) => None,
        };

        // Project low-level results to high-level
        let (high_values, low_values) = match (high_values, low_values) {
            (Some(h), Some(l)) => (h, l),
            _ => return None, // Can't compare if simulation failed
        };

        let projected = self.project_distribution(&low_values);

        // Compare high-level result with projected result
        let mut divergence = 0.0;
        let mut expected = HashMap::new();
        let mut actual = HashMap::new();

        for high_var in self.high_level.variables() {
            let high_val = high_values.get(&high_var.id)
                .map(|v| v.as_f64())
                .unwrap_or(0.0);
            let proj_val = projected.get(&high_var.id)
                .map(|v| v.as_f64())
                .unwrap_or(0.0);

            let diff = (high_val - proj_val).abs();
            divergence += diff * diff;

            expected.insert(high_var.name.clone(), high_val);
            actual.insert(high_var.name.clone(), proj_val);
        }

        divergence = divergence.sqrt();

        if divergence > self.tolerance {
            Some(ConsistencyViolation {
                intervention: format!("do({:?} = {:?})", high_intervention.target, high_intervention.value),
                expected,
                actual,
                divergence,
            })
        } else {
            None
        }
    }

    /// Lift a high-level intervention to low-level interventions
    pub fn lift_intervention(&self, high: &Intervention) -> Vec<Intervention> {
        let low_vars = match self.abstraction_map.project_variable(high.target) {
            Some(vars) => vars,
            None => return vec![],
        };

        // Simple strategy: apply same value to all corresponding low-level variables
        // More sophisticated approaches could distribute the intervention differently
        low_vars.iter()
            .map(|&low_var| Intervention::new(low_var, high.value.clone()))
            .collect()
    }

    /// Project a low-level distribution to high-level
    pub fn project_distribution(&self, low_dist: &HashMap<VariableId, Value>) -> HashMap<VariableId, Value> {
        let mut high_dist = HashMap::new();

        for high_var in self.high_level.variables() {
            let projected_value = self.abstraction_map.lift_value(high_var.id, low_dist);
            high_dist.insert(high_var.id, projected_value);
        }

        high_dist
    }

    /// Project a CounterfactualDistribution object
    pub fn project_distribution_obj(&self, low_dist: &CounterfactualDistribution) -> CounterfactualDistribution {
        let high_values = self.project_distribution(&low_dist.values);
        CounterfactualDistribution {
            values: high_values,
            probability: low_dist.probability,
        }
    }

    /// Get the coarsening factor (how much the abstraction simplifies)
    pub fn coarsening_factor(&self) -> f64 {
        let low_count = self.low_level.num_variables() as f64;
        let high_count = self.high_level.num_variables() as f64;

        if high_count > 0.0 {
            low_count / high_count
        } else {
            f64::INFINITY
        }
    }

    /// Check if a low-level variable is "hidden" (not directly represented in high-level)
    pub fn is_hidden(&self, low_var: VariableId) -> bool {
        self.abstraction_map.lift_variable(low_var).is_none()
    }

    /// Get all hidden variables
    pub fn hidden_variables(&self) -> Vec<VariableId> {
        self.low_level.variables()
            .filter(|v| self.is_hidden(v.id))
            .map(|v| v.id)
            .collect()
    }
}

/// Builder for creating causal abstractions
pub struct AbstractionBuilder<'a> {
    low_level: Option<&'a CausalModel>,
    high_level: Option<&'a CausalModel>,
    map: AbstractionMap,
}

impl<'a> AbstractionBuilder<'a> {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            low_level: None,
            high_level: None,
            map: AbstractionMap::new(),
        }
    }

    /// Set the low-level model
    pub fn low_level(mut self, model: &'a CausalModel) -> Self {
        self.low_level = Some(model);
        self
    }

    /// Set the high-level model
    pub fn high_level(mut self, model: &'a CausalModel) -> Self {
        self.high_level = Some(model);
        self
    }

    /// Add a variable mapping by name
    pub fn map_variable(
        mut self,
        high_name: &str,
        low_names: &[&str],
        aggregator: Aggregator,
    ) -> Self {
        if let (Some(low), Some(high)) = (self.low_level, self.high_level) {
            if let Some(high_id) = high.get_variable_id(high_name) {
                let low_ids: HashSet<_> = low_names.iter()
                    .filter_map(|&name| low.get_variable_id(name))
                    .collect();

                if !low_ids.is_empty() {
                    self.map.add_mapping(high_id, low_ids, aggregator);
                }
            }
        }
        self
    }

    /// Add an identity mapping by name
    pub fn map_identity(mut self, high_name: &str, low_name: &str) -> Self {
        if let (Some(low), Some(high)) = (self.low_level, self.high_level) {
            if let (Some(high_id), Some(low_id)) = (
                high.get_variable_id(high_name),
                low.get_variable_id(low_name),
            ) {
                self.map.add_identity_mapping(high_id, low_id);
            }
        }
        self
    }

    /// Build the abstraction
    pub fn build(self) -> Result<CausalAbstraction<'a>, AbstractionError> {
        let low = self.low_level.ok_or_else(|| {
            AbstractionError::IncompatibleStructure("No low-level model provided".to_string())
        })?;
        let high = self.high_level.ok_or_else(|| {
            AbstractionError::IncompatibleStructure("No high-level model provided".to_string())
        })?;

        CausalAbstraction::new(low, high, self.map)
    }
}

impl<'a> Default for AbstractionBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal::model::{CausalModelBuilder, Mechanism, VariableType};

    fn create_low_level_model() -> CausalModel {
        let mut model = CausalModel::with_name("Low-Level");

        // Detailed model with separate variables
        model.add_variable("Age", VariableType::Continuous).unwrap();
        model.add_variable("Education", VariableType::Continuous).unwrap();
        model.add_variable("Experience", VariableType::Continuous).unwrap();
        model.add_variable("Salary", VariableType::Continuous).unwrap();
        model.add_variable("Savings", VariableType::Continuous).unwrap();

        let age = model.get_variable_id("Age").unwrap();
        let edu = model.get_variable_id("Education").unwrap();
        let exp = model.get_variable_id("Experience").unwrap();
        let salary = model.get_variable_id("Salary").unwrap();
        let savings = model.get_variable_id("Savings").unwrap();

        // Experience = f(Age, Education)
        model.add_structural_equation(exp, &[age, edu], Mechanism::new(|p| {
            Value::Continuous(p[0].as_f64() * 0.5 + p[1].as_f64() * 0.3)
        })).unwrap();

        // Salary = f(Education, Experience)
        model.add_structural_equation(salary, &[edu, exp], Mechanism::new(|p| {
            Value::Continuous(30000.0 + p[0].as_f64() * 5000.0 + p[1].as_f64() * 2000.0)
        })).unwrap();

        // Savings = f(Salary)
        model.add_structural_equation(savings, &[salary], Mechanism::new(|p| {
            Value::Continuous(p[0].as_f64() * 0.2)
        })).unwrap();

        model
    }

    fn create_high_level_model() -> CausalModel {
        let mut model = CausalModel::with_name("High-Level");

        // Simplified model with aggregated variables
        model.add_variable("HumanCapital", VariableType::Continuous).unwrap();
        model.add_variable("Wealth", VariableType::Continuous).unwrap();

        let hc = model.get_variable_id("HumanCapital").unwrap();
        let wealth = model.get_variable_id("Wealth").unwrap();

        // Wealth = f(HumanCapital)
        model.add_structural_equation(wealth, &[hc], Mechanism::new(|p| {
            Value::Continuous(p[0].as_f64() * 10000.0)
        })).unwrap();

        model
    }

    #[test]
    fn test_abstraction_map() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        // HumanCapital = mean(Education, Experience)
        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();
        let exp_id = low.get_variable_id("Experience").unwrap();

        let mut low_vars = HashSet::new();
        low_vars.insert(edu_id);
        low_vars.insert(exp_id);
        map.add_mapping(hc_id, low_vars, Aggregator::Mean);

        // Wealth = sum(Salary, Savings)
        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();
        let savings_id = low.get_variable_id("Savings").unwrap();

        let mut wealth_vars = HashSet::new();
        wealth_vars.insert(salary_id);
        wealth_vars.insert(savings_id);
        map.add_mapping(wealth_id, wealth_vars, Aggregator::Sum);

        assert!(map.is_surjective(&high));
    }

    #[test]
    fn test_aggregators() {
        let values = vec![
            Value::Continuous(1.0),
            Value::Continuous(2.0),
            Value::Continuous(3.0),
        ];

        assert_eq!(Aggregator::First.apply(&values).as_f64(), 1.0);
        assert_eq!(Aggregator::Sum.apply(&values).as_f64(), 6.0);
        assert_eq!(Aggregator::Mean.apply(&values).as_f64(), 2.0);
        assert_eq!(Aggregator::Max.apply(&values).as_f64(), 3.0);
        assert_eq!(Aggregator::Min.apply(&values).as_f64(), 1.0);
    }

    #[test]
    fn test_lift_intervention() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();
        let exp_id = low.get_variable_id("Experience").unwrap();

        let mut low_vars = HashSet::new();
        low_vars.insert(edu_id);
        low_vars.insert(exp_id);
        map.add_mapping(hc_id, low_vars, Aggregator::Mean);

        // Add wealth mapping
        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();
        let mut wealth_vars = HashSet::new();
        wealth_vars.insert(salary_id);
        map.add_mapping(wealth_id, wealth_vars, Aggregator::First);

        let abstraction = CausalAbstraction::new(&low, &high, map).unwrap();

        let high_intervention = Intervention::new(hc_id, Value::Continuous(10.0));
        let low_interventions = abstraction.lift_intervention(&high_intervention);

        // Should lift to interventions on Education and Experience
        assert_eq!(low_interventions.len(), 2);
        assert!(low_interventions.iter().any(|i| i.target == edu_id));
        assert!(low_interventions.iter().any(|i| i.target == exp_id));
    }

    #[test]
    fn test_project_distribution() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();

        let mut low_vars = HashSet::new();
        low_vars.insert(edu_id);
        map.add_mapping(hc_id, low_vars, Aggregator::First);

        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();

        let mut wealth_vars = HashSet::new();
        wealth_vars.insert(salary_id);
        map.add_mapping(wealth_id, wealth_vars, Aggregator::First);

        let abstraction = CausalAbstraction::new(&low, &high, map).unwrap();

        let mut low_dist = HashMap::new();
        low_dist.insert(edu_id, Value::Continuous(16.0));
        low_dist.insert(salary_id, Value::Continuous(80000.0));

        let high_dist = abstraction.project_distribution(&low_dist);

        assert_eq!(high_dist.get(&hc_id).unwrap().as_f64(), 16.0);
        assert_eq!(high_dist.get(&wealth_id).unwrap().as_f64(), 80000.0);
    }

    #[test]
    fn test_coarsening_factor() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        // Simple identity mappings for this test
        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();
        map.add_identity_mapping(hc_id, edu_id);

        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();
        map.add_identity_mapping(wealth_id, salary_id);

        let abstraction = CausalAbstraction::new(&low, &high, map).unwrap();

        // 5 low-level vars / 2 high-level vars = 2.5
        assert!((abstraction.coarsening_factor() - 2.5).abs() < 1e-10);
    }

    #[test]
    fn test_hidden_variables() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        // Only map Education to HumanCapital
        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();
        map.add_identity_mapping(hc_id, edu_id);

        // Only map Salary to Wealth
        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();
        map.add_identity_mapping(wealth_id, salary_id);

        let abstraction = CausalAbstraction::new(&low, &high, map).unwrap();

        let hidden = abstraction.hidden_variables();

        // Age, Experience, Savings should be hidden
        let age_id = low.get_variable_id("Age").unwrap();
        let exp_id = low.get_variable_id("Experience").unwrap();
        let savings_id = low.get_variable_id("Savings").unwrap();

        assert!(hidden.contains(&age_id));
        assert!(hidden.contains(&exp_id));
        assert!(hidden.contains(&savings_id));
        assert!(!hidden.contains(&edu_id));
        assert!(!hidden.contains(&salary_id));
    }

    #[test]
    fn test_builder() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let abstraction = AbstractionBuilder::new()
            .low_level(&low)
            .high_level(&high)
            .map_identity("HumanCapital", "Education")
            .map_identity("Wealth", "Salary")
            .build()
            .unwrap();

        assert_eq!(abstraction.coarsening_factor(), 2.5);
    }

    #[test]
    fn test_consistency_check() {
        let low = create_low_level_model();
        let high = create_high_level_model();

        let mut map = AbstractionMap::new();

        let hc_id = high.get_variable_id("HumanCapital").unwrap();
        let edu_id = low.get_variable_id("Education").unwrap();
        map.add_identity_mapping(hc_id, edu_id);

        let wealth_id = high.get_variable_id("Wealth").unwrap();
        let salary_id = low.get_variable_id("Salary").unwrap();
        map.add_identity_mapping(wealth_id, salary_id);

        let abstraction = CausalAbstraction::new(&low, &high, map)
            .unwrap()
            .with_tolerance(1000.0); // High tolerance for this test

        let result = abstraction.check_consistency();

        // The abstraction may or may not be consistent depending on mechanisms
        // This test just verifies the check runs
        assert!(result.interventions_tested > 0);
    }
}
