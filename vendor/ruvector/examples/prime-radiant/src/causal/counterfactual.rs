//! Counterfactual Reasoning
//!
//! This module implements counterfactual inference based on Pearl's three-step
//! procedure: Abduction, Action, Prediction.
//!
//! ## Counterfactual Semantics
//!
//! Given a structural causal model M = (U, V, F), a counterfactual query asks:
//! "What would Y have been if X had been x, given that we observed E = e?"
//!
//! Written as: P(Y_x | E = e) or Y_{X=x}(u) where u is the exogenous state.
//!
//! ## Three-Step Procedure
//!
//! 1. **Abduction**: Update P(U) given evidence E = e to get P(U | E = e)
//! 2. **Action**: Modify the model by intervention do(X = x)
//! 3. **Prediction**: Compute Y in the modified model using updated U
//!
//! ## References
//!
//! - Pearl (2009): "Causality" Chapter 7
//! - Halpern (2016): "Actual Causality"

use std::collections::HashMap;
use thiserror::Error;

use super::model::{
    CausalModel, CausalModelError, Intervention, Value, VariableId, Mechanism,
    Observation,
};

/// Error types for counterfactual reasoning
#[derive(Debug, Clone, Error)]
pub enum CounterfactualError {
    /// Model error
    #[error("Model error: {0}")]
    ModelError(#[from] CausalModelError),

    /// Invalid observation
    #[error("Invalid observation: variable '{0}' not in model")]
    InvalidObservation(String),

    /// Abduction failed
    #[error("Abduction failed: {0}")]
    AbductionFailed(String),

    /// Counterfactual not well-defined
    #[error("Counterfactual not well-defined: {0}")]
    NotWellDefined(String),
}

/// Extended Distribution type for counterfactual results
#[derive(Debug, Clone)]
pub struct CounterfactualDistribution {
    /// Point estimate values (for deterministic models)
    pub values: HashMap<VariableId, Value>,
    /// Probability mass (for discrete) or density (for continuous)
    pub probability: f64,
}

impl CounterfactualDistribution {
    /// Create a point mass distribution
    pub fn point_mass(values: HashMap<VariableId, Value>) -> Self {
        Self {
            values,
            probability: 1.0,
        }
    }

    /// Create from a simulation result
    pub fn from_simulation(values: HashMap<VariableId, Value>) -> Self {
        Self::point_mass(values)
    }

    /// Get value for a variable
    pub fn get(&self, var: VariableId) -> Option<&Value> {
        self.values.get(&var)
    }

    /// Get mean value (for continuous distributions)
    pub fn mean(&self, var: VariableId) -> f64 {
        self.values.get(&var)
            .map(|v| v.as_f64())
            .unwrap_or(0.0)
    }
}

/// A counterfactual query
#[derive(Debug, Clone)]
pub struct CounterfactualQuery {
    /// Target variable (what we want to know)
    pub target: String,
    /// Interventions (what we're hypothetically changing)
    pub interventions: Vec<(String, Value)>,
    /// Evidence (what we observed)
    pub evidence: Observation,
}

impl CounterfactualQuery {
    /// Create a new counterfactual query
    ///
    /// Asking: "What would `target` have been if we had done `interventions`,
    /// given that we observed `evidence`?"
    pub fn new(target: &str, interventions: Vec<(&str, Value)>, evidence: Observation) -> Self {
        Self {
            target: target.to_string(),
            interventions: interventions.into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            evidence,
        }
    }

    /// Simple counterfactual: "What would Y have been if X had been x?"
    pub fn simple(target: &str, intervention_var: &str, intervention_val: Value) -> Self {
        Self {
            target: target.to_string(),
            interventions: vec![(intervention_var.to_string(), intervention_val)],
            evidence: Observation::empty(),
        }
    }

    /// Add evidence to the query
    pub fn given(mut self, var: &str, value: Value) -> Self {
        self.evidence.observe(var, value);
        self
    }
}

/// Result of a counterfactual query
#[derive(Debug, Clone)]
pub struct CounterfactualResult {
    /// The query that was answered
    pub query: CounterfactualQuery,
    /// The counterfactual distribution
    pub distribution: CounterfactualDistribution,
    /// The abduced exogenous values
    pub exogenous: HashMap<VariableId, Value>,
    /// Explanation of the reasoning
    pub explanation: String,
}

/// Average Treatment Effect computation
#[derive(Debug, Clone)]
pub struct AverageTreatmentEffect {
    /// Treatment variable
    pub treatment: String,
    /// Outcome variable
    pub outcome: String,
    /// Treatment value
    pub treatment_value: Value,
    /// Control value
    pub control_value: Value,
    /// Estimated ATE
    pub ate: f64,
    /// Standard error (if available)
    pub standard_error: Option<f64>,
}

impl AverageTreatmentEffect {
    /// Create a new ATE result
    pub fn new(
        treatment: &str,
        outcome: &str,
        treatment_value: Value,
        control_value: Value,
        ate: f64,
    ) -> Self {
        Self {
            treatment: treatment.to_string(),
            outcome: outcome.to_string(),
            treatment_value,
            control_value,
            ate,
            standard_error: None,
        }
    }

    /// Set standard error
    pub fn with_standard_error(mut self, se: f64) -> Self {
        self.standard_error = Some(se);
        self
    }
}

/// Compute a counterfactual: "What would Y have been if X had been x, given observation?"
///
/// This implements Pearl's three-step procedure:
/// 1. Abduction: Infer exogenous variables from observation
/// 2. Action: Apply intervention do(X = x)
/// 3. Prediction: Compute Y under intervention with abduced exogenous values
///
/// # Arguments
/// * `model` - The causal model
/// * `observation` - The observed evidence
/// * `intervention_var` - The variable to intervene on
/// * `intervention_value` - The counterfactual value for the intervention
/// * `target_name` - The target variable to compute the counterfactual for
pub fn counterfactual(
    model: &CausalModel,
    observation: &Observation,
    intervention_var: VariableId,
    intervention_value: Value,
    target_name: &str,
) -> Result<Value, CounterfactualError> {
    // Step 1: Abduction - infer exogenous variables
    let exogenous = abduce(model, observation)?;

    // Step 2: Action - create intervened model
    let intervention = Intervention::new(intervention_var, intervention_value);
    let intervened = model.intervene_with(&[intervention])?;

    // Step 3: Prediction - simulate with abduced exogenous and intervention
    let result = intervened.simulate(&exogenous)?;

    // Get the target variable
    let target_id = model.get_variable_id(target_name)
        .ok_or_else(|| CounterfactualError::InvalidObservation(target_name.to_string()))?;

    result.get(&target_id)
        .cloned()
        .ok_or_else(|| CounterfactualError::AbductionFailed(
            format!("Target variable {} not computed", target_name)
        ))
}

/// Compute a counterfactual with an Intervention struct (alternative API)
pub fn counterfactual_with_intervention(
    model: &CausalModel,
    observation: &Observation,
    intervention: &Intervention,
) -> Result<CounterfactualDistribution, CounterfactualError> {
    // Step 1: Abduction - infer exogenous variables
    let exogenous = abduce(model, observation)?;

    // Step 2: Action - create intervened model
    let intervened = model.intervene_with(&[intervention.clone()])?;

    // Step 3: Prediction - simulate with abduced exogenous and intervention
    let result = intervened.simulate(&exogenous)?;

    Ok(CounterfactualDistribution::from_simulation(result))
}

/// Compute counterfactual from a query object
pub fn counterfactual_query(
    model: &CausalModel,
    query: &CounterfactualQuery,
) -> Result<CounterfactualResult, CounterfactualError> {
    // Convert interventions
    let interventions: Result<Vec<Intervention>, _> = query.interventions.iter()
        .map(|(var, val)| {
            model.get_variable_id(var)
                .map(|id| Intervention::new(id, val.clone()))
                .ok_or_else(|| CounterfactualError::InvalidObservation(var.clone()))
        })
        .collect();
    let interventions = interventions?;

    // Step 1: Abduction
    let exogenous = abduce(model, &query.evidence)?;

    // Step 2 & 3: Action and Prediction
    let intervened = model.intervene_with(&interventions)?;
    let result = intervened.simulate(&exogenous)?;

    let target_id = model.get_variable_id(&query.target)
        .ok_or_else(|| CounterfactualError::InvalidObservation(query.target.clone()))?;

    let explanation = format!(
        "Counterfactual computed via three-step procedure:\n\
         1. Abduced exogenous values from evidence\n\
         2. Applied intervention(s): {}\n\
         3. Predicted {} under intervention",
        query.interventions.iter()
            .map(|(v, val)| format!("do({}={:?})", v, val))
            .collect::<Vec<_>>()
            .join(", "),
        query.target
    );

    Ok(CounterfactualResult {
        query: query.clone(),
        distribution: CounterfactualDistribution::from_simulation(result),
        exogenous,
        explanation,
    })
}

/// Abduction: Infer exogenous variable values from observations
///
/// For deterministic models, this inverts the structural equations
fn abduce(
    model: &CausalModel,
    observation: &Observation,
) -> Result<HashMap<VariableId, Value>, CounterfactualError> {
    let mut exogenous = HashMap::new();

    // Get topological order
    let topo_order = model.topological_order_ids()?;

    // For each variable in topological order
    for var_id in topo_order {
        let var = model.get_variable(var_id.as_ref())
            .ok_or_else(|| CounterfactualError::AbductionFailed(
                format!("Variable {:?} not found", var_id)
            ))?;

        // Check if this variable is observed
        if let Some(observed_value) = observation.values.get(&var.name) {
            // If this is a root variable (no parents), it's exogenous
            if model.parents(&var_id).map(|p| p.is_empty()).unwrap_or(true) {
                exogenous.insert(var_id, observed_value.clone());
            } else {
                // For endogenous variables, we might need to compute the residual
                // For now, we use the observed value as the exogenous noise
                exogenous.insert(var_id, observed_value.clone());
            }
        }
    }

    Ok(exogenous)
}

/// Compute the Average Treatment Effect (ATE)
///
/// ATE = E[Y | do(X=treatment_value)] - E[Y | do(X=control_value)]
///
/// # Arguments
/// * `model` - The causal model
/// * `treatment` - Treatment variable ID
/// * `outcome` - Outcome variable ID
/// * `treatment_value` - The treatment value
/// * `control_value` - The control value
pub fn causal_effect(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    treatment_value: Value,
    control_value: Value,
) -> Result<f64, CounterfactualError> {
    causal_effect_at_values(
        model,
        treatment,
        outcome,
        treatment_value,
        control_value,
    )
}

/// Compute the Average Treatment Effect with default binary values
///
/// ATE = E[Y | do(X=1)] - E[Y | do(X=0)]
pub fn causal_effect_binary(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
) -> Result<f64, CounterfactualError> {
    causal_effect_at_values(
        model,
        treatment,
        outcome,
        Value::Continuous(1.0),
        Value::Continuous(0.0),
    )
}

/// Compute causal effect at specific treatment values
pub fn causal_effect_at_values(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    treatment_value: Value,
    control_value: Value,
) -> Result<f64, CounterfactualError> {
    // E[Y | do(X = treatment)]
    let intervention_treat = Intervention::new(treatment, treatment_value);
    let intervened_treat = model.intervene_with(&[intervention_treat])?;
    let result_treat = intervened_treat.simulate(&HashMap::new())?;
    let y_treat = result_treat.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    // E[Y | do(X = control)]
    let intervention_ctrl = Intervention::new(treatment, control_value);
    let intervened_ctrl = model.intervene_with(&[intervention_ctrl])?;
    let result_ctrl = intervened_ctrl.simulate(&HashMap::new())?;
    let y_ctrl = result_ctrl.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(y_treat - y_ctrl)
}

/// Compute ATE with full result structure
pub fn average_treatment_effect(
    model: &CausalModel,
    treatment_name: &str,
    outcome_name: &str,
    treatment_value: Value,
    control_value: Value,
) -> Result<AverageTreatmentEffect, CounterfactualError> {
    let treatment_id = model.get_variable_id(treatment_name)
        .ok_or_else(|| CounterfactualError::InvalidObservation(treatment_name.to_string()))?;
    let outcome_id = model.get_variable_id(outcome_name)
        .ok_or_else(|| CounterfactualError::InvalidObservation(outcome_name.to_string()))?;

    let ate = causal_effect_at_values(
        model,
        treatment_id,
        outcome_id,
        treatment_value.clone(),
        control_value.clone(),
    )?;

    Ok(AverageTreatmentEffect::new(
        treatment_name,
        outcome_name,
        treatment_value,
        control_value,
        ate,
    ))
}

/// Compute Individual Treatment Effect (ITE) for a specific unit
///
/// ITE_i = Y_i(X=1) - Y_i(X=0)
///
/// This is a counterfactual quantity: what would have happened to unit i
/// under different treatment assignments.
pub fn individual_treatment_effect(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    unit_observation: &Observation,
    treatment_value: Value,
    control_value: Value,
) -> Result<f64, CounterfactualError> {
    // Get the outcome variable name
    let outcome_name = model.get_variable_name(&outcome)
        .ok_or_else(|| CounterfactualError::InvalidObservation(format!("Outcome variable {:?} not found", outcome)))?;

    // Counterfactual: Y(X=treatment) for this unit
    let y_treat = counterfactual(model, unit_observation, treatment, treatment_value, &outcome_name)?;
    let y_treat_val = y_treat.as_f64();

    // Counterfactual: Y(X=control) for this unit
    let y_ctrl = counterfactual(model, unit_observation, treatment, control_value, &outcome_name)?;
    let y_ctrl_val = y_ctrl.as_f64();

    Ok(y_treat_val - y_ctrl_val)
}

/// Natural Direct Effect (NDE)
///
/// NDE = E[Y(x, M(x'))] - E[Y(x', M(x'))]
///
/// The effect of X on Y that would remain if the mediator were held at the
/// value it would have taken under X = x'.
pub fn natural_direct_effect(
    model: &CausalModel,
    treatment: VariableId,
    mediator: VariableId,
    outcome: VariableId,
    treatment_value: Value,
    control_value: Value,
) -> Result<f64, CounterfactualError> {
    // E[Y(x', M(x'))] - baseline
    let ctrl_intervention = Intervention::new(treatment, control_value.clone());
    let intervened = model.intervene_with(&[ctrl_intervention.clone()])?;
    let baseline_result = intervened.simulate(&HashMap::new())?;
    let m_ctrl = baseline_result.get(&mediator).cloned().unwrap_or(Value::Missing);
    let y_baseline = baseline_result.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    // E[Y(x, M(x'))] - intervene on X but keep M at control level
    let treat_intervention = Intervention::new(treatment, treatment_value);
    let m_intervention = Intervention::new(mediator, m_ctrl);
    let intervened = model.intervene_with(&[treat_intervention, m_intervention])?;
    let nde_result = intervened.simulate(&HashMap::new())?;
    let y_nde = nde_result.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(y_nde - y_baseline)
}

/// Natural Indirect Effect (NIE)
///
/// NIE = E[Y(x, M(x))] - E[Y(x, M(x'))]
///
/// The effect of X on Y that is mediated through M.
pub fn natural_indirect_effect(
    model: &CausalModel,
    treatment: VariableId,
    mediator: VariableId,
    outcome: VariableId,
    treatment_value: Value,
    control_value: Value,
) -> Result<f64, CounterfactualError> {
    // E[Y(x, M(x))] - full treatment effect
    let treat_intervention = Intervention::new(treatment, treatment_value.clone());
    let intervened = model.intervene_with(&[treat_intervention.clone()])?;
    let full_result = intervened.simulate(&HashMap::new())?;
    let y_full = full_result.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    // E[Y(x, M(x'))] - treatment but mediator at control level
    let ctrl_intervention = Intervention::new(treatment, control_value);
    let ctrl_intervened = model.intervene_with(&[ctrl_intervention])?;
    let ctrl_result = ctrl_intervened.simulate(&HashMap::new())?;
    let m_ctrl = ctrl_result.get(&mediator).cloned().unwrap_or(Value::Missing);

    let m_intervention = Intervention::new(mediator, m_ctrl);
    let intervened = model.intervene_with(&[treat_intervention, m_intervention])?;
    let indirect_result = intervened.simulate(&HashMap::new())?;
    let y_indirect = indirect_result.get(&outcome)
        .map(|v| v.as_f64())
        .unwrap_or(0.0);

    Ok(y_full - y_indirect)
}

/// Probability of Necessity (PN)
///
/// PN = P(Y_x' = 0 | X = x, Y = 1)
///
/// Given that X=x and Y=1 occurred, what is the probability that Y would
/// have been 0 if X had been x' instead?
pub fn probability_of_necessity(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    observation: &Observation,
    counterfactual_treatment: Value,
) -> Result<f64, CounterfactualError> {
    // Get outcome variable name
    let outcome_name = model.get_variable_name(&outcome)
        .ok_or_else(|| CounterfactualError::InvalidObservation(format!("Outcome variable {:?} not found", outcome)))?;

    // Compute counterfactual outcome
    let cf_value = counterfactual(model, observation, treatment, counterfactual_treatment, &outcome_name)?;

    let cf_outcome = cf_value.as_f64();

    // PN is probability that outcome would be 0 (negative)
    // For continuous outcomes, we check if it crosses the threshold
    let observed_outcome = observation.values.iter()
        .find_map(|(name, val)| {
            model.get_variable_id(name)
                .filter(|id| *id == outcome)
                .map(|_| val.as_f64())
        })
        .unwrap_or(0.0);

    // Simple heuristic: if counterfactual outcome is significantly different
    if observed_outcome > 0.0 && cf_outcome <= 0.0 {
        Ok(1.0) // Necessary
    } else if (observed_outcome - cf_outcome).abs() < 1e-6 {
        Ok(0.0) // Not necessary
    } else {
        Ok(0.5) // Uncertain
    }
}

/// Probability of Sufficiency (PS)
///
/// PS = P(Y_x = 1 | X = x', Y = 0)
///
/// Given that X=x' and Y=0 occurred, what is the probability that Y would
/// have been 1 if X had been x instead?
pub fn probability_of_sufficiency(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    observation: &Observation,
    counterfactual_treatment: Value,
) -> Result<f64, CounterfactualError> {
    // Get outcome variable name
    let outcome_name = model.get_variable_name(&outcome)
        .ok_or_else(|| CounterfactualError::InvalidObservation(format!("Outcome variable {:?} not found", outcome)))?;

    // Compute counterfactual outcome
    let cf_value = counterfactual(model, observation, treatment, counterfactual_treatment, &outcome_name)?;

    let cf_outcome = cf_value.as_f64();

    let observed_outcome = observation.values.iter()
        .find_map(|(name, val)| {
            model.get_variable_id(name)
                .filter(|id| *id == outcome)
                .map(|_| val.as_f64())
        })
        .unwrap_or(1.0);

    // PS: would the outcome have been positive if treatment were different?
    if observed_outcome <= 0.0 && cf_outcome > 0.0 {
        Ok(1.0) // Sufficient
    } else if (observed_outcome - cf_outcome).abs() < 1e-6 {
        Ok(0.0) // Not sufficient
    } else {
        Ok(0.5) // Uncertain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::causal::model::{CausalModelBuilder, VariableType, Mechanism};

    fn create_simple_model() -> CausalModel {
        let mut model = CausalModel::with_name("Simple");

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        // Y = 2*X + 1
        model.add_structural_equation(y, &[x], Mechanism::new(|p| {
            Value::Continuous(2.0 * p[0].as_f64() + 1.0)
        })).unwrap();

        model
    }

    fn create_mediation_model() -> CausalModel {
        let mut model = CausalModel::with_name("Mediation");

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("M", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let m = model.get_variable_id("M").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        // M = X
        model.add_structural_equation(m, &[x], Mechanism::new(|p| {
            p[0].clone()
        })).unwrap();

        // Y = M + 0.5*X
        model.add_structural_equation(y, &[m, x], Mechanism::new(|p| {
            Value::Continuous(p[0].as_f64() + 0.5 * p[1].as_f64())
        })).unwrap();

        model
    }

    #[test]
    fn test_observation() {
        let mut obs = Observation::new(&[("X", Value::Continuous(5.0))]);
        obs.observe("Y", Value::Continuous(11.0));

        assert!(obs.is_observed("X"));
        assert!(obs.is_observed("Y"));
        assert!(!obs.is_observed("Z"));
    }

    #[test]
    fn test_counterfactual_simple() {
        let model = create_simple_model();

        let x_id = model.get_variable_id("X").unwrap();

        // Observation: X=3, Y=7 (since Y = 2*3 + 1)
        let observation = Observation::new(&[
            ("X", Value::Continuous(3.0)),
            ("Y", Value::Continuous(7.0)),
        ]);

        // Counterfactual: What would Y have been if X had been 5?
        let intervention = Intervention::new(x_id, Value::Continuous(5.0));

        let result = counterfactual(&model, &observation, &intervention).unwrap();

        // Y should be 2*5 + 1 = 11
        let y_id = model.get_variable_id("Y").unwrap();
        let y_value = result.get(y_id).unwrap().as_f64();

        assert!((y_value - 11.0).abs() < 1e-10);
    }

    #[test]
    fn test_causal_effect() {
        let model = create_simple_model();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        // ATE = E[Y|do(X=1)] - E[Y|do(X=0)]
        // = (2*1 + 1) - (2*0 + 1) = 3 - 1 = 2
        let ate = causal_effect(&model, x, y).unwrap();

        assert!((ate - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_average_treatment_effect() {
        let model = create_simple_model();

        let ate_result = average_treatment_effect(
            &model,
            "X", "Y",
            Value::Continuous(1.0),
            Value::Continuous(0.0),
        ).unwrap();

        assert_eq!(ate_result.treatment, "X");
        assert_eq!(ate_result.outcome, "Y");
        assert!((ate_result.ate - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_mediation_effects() {
        let model = create_mediation_model();

        let x = model.get_variable_id("X").unwrap();
        let m = model.get_variable_id("M").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        let treat = Value::Continuous(1.0);
        let ctrl = Value::Continuous(0.0);

        // Total effect should be:
        // E[Y|do(X=1)] - E[Y|do(X=0)]
        // = (M(1) + 0.5*1) - (M(0) + 0.5*0)
        // = (1 + 0.5) - (0 + 0) = 1.5
        let total = causal_effect_at_values(&model, x, y, treat.clone(), ctrl.clone()).unwrap();
        assert!((total - 1.5).abs() < 1e-10);

        // NDE should be the direct effect = 0.5 (coefficient of X in Y equation)
        let nde = natural_direct_effect(&model, x, m, y, treat.clone(), ctrl.clone()).unwrap();
        assert!((nde - 0.5).abs() < 1e-10);

        // NIE should be the indirect effect = 1.0 (coefficient of M in Y, times effect of X on M)
        let nie = natural_indirect_effect(&model, x, m, y, treat, ctrl).unwrap();
        assert!((nie - 1.0).abs() < 1e-10);

        // NDE + NIE should equal total effect
        assert!((nde + nie - total).abs() < 1e-10);
    }

    #[test]
    fn test_counterfactual_query() {
        let model = create_simple_model();

        let query = CounterfactualQuery::new(
            "Y",
            vec![("X", Value::Continuous(10.0))],
            Observation::new(&[("X", Value::Continuous(3.0))]),
        );

        let result = counterfactual_query(&model, &query).unwrap();

        // Y = 2*10 + 1 = 21
        let y_id = model.get_variable_id("Y").unwrap();
        assert!((result.distribution.mean(y_id) - 21.0).abs() < 1e-10);
    }

    #[test]
    fn test_distribution() {
        let mut values = HashMap::new();
        let x_id = VariableId(0);
        let y_id = VariableId(1);

        values.insert(x_id, Value::Continuous(5.0));
        values.insert(y_id, Value::Continuous(10.0));

        let dist = CounterfactualDistribution::point_mass(values);

        assert_eq!(dist.mean(x_id), 5.0);
        assert_eq!(dist.mean(y_id), 10.0);
        assert_eq!(dist.probability, 1.0);
    }

    #[test]
    fn test_individual_treatment_effect() {
        let model = create_simple_model();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        // Unit-specific observation
        let unit_obs = Observation::new(&[
            ("X", Value::Continuous(3.0)),
            ("Y", Value::Continuous(7.0)),
        ]);

        let ite = individual_treatment_effect(
            &model,
            x, y,
            &unit_obs,
            Value::Continuous(5.0),
            Value::Continuous(3.0),
        ).unwrap();

        // ITE = Y(X=5) - Y(X=3) = 11 - 7 = 4
        assert!((ite - 4.0).abs() < 1e-10);
    }
}
