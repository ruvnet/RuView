//! Structural Causal Models (SCM) for causal reasoning
//!
//! This module implements the core causal model structure, including:
//! - Variables with types (continuous, discrete, binary)
//! - Structural equations defining causal mechanisms
//! - Intervention semantics (do-operator)
//! - Forward simulation

use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

use super::graph::{DirectedGraph, DAGValidationError, TopologicalOrder};

/// Unique identifier for a variable in the causal model
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VariableId(pub u32);

impl From<u32> for VariableId {
    fn from(id: u32) -> Self {
        VariableId(id)
    }
}

impl From<VariableId> for u32 {
    fn from(id: VariableId) -> u32 {
        id.0
    }
}

/// Type of a causal variable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariableType {
    /// Continuous real-valued variable
    Continuous,
    /// Discrete variable with finite domain
    Discrete,
    /// Binary variable (special case of discrete)
    Binary,
    /// Categorical variable with named levels
    Categorical,
}

/// Value that a variable can take
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Continuous value
    Continuous(f64),
    /// Discrete integer value
    Discrete(i64),
    /// Binary value
    Binary(bool),
    /// Categorical value (index into category list)
    Categorical(usize),
    /// Missing/unknown value
    Missing,
}

impl Value {
    /// Convert to f64 if possible
    pub fn as_f64(&self) -> f64 {
        match self {
            Value::Continuous(x) => *x,
            Value::Discrete(x) => *x as f64,
            Value::Binary(b) => if *b { 1.0 } else { 0.0 },
            Value::Categorical(i) => *i as f64,
            Value::Missing => f64::NAN,
        }
    }

    /// Convert to bool if binary
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Binary(b) => Some(*b),
            Value::Discrete(x) => Some(*x != 0),
            Value::Continuous(x) => Some(*x != 0.0),
            Value::Categorical(i) => Some(*i != 0),
            Value::Missing => None,
        }
    }

    /// Check if value is missing
    pub fn is_missing(&self) -> bool {
        matches!(self, Value::Missing)
    }
}

impl Default for Value {
    fn default() -> Self {
        Value::Missing
    }
}

/// A variable in the causal model
#[derive(Debug, Clone)]
pub struct Variable {
    /// Unique identifier
    pub id: VariableId,
    /// Human-readable name
    pub name: String,
    /// Variable type
    pub var_type: VariableType,
    /// Domain constraints (min, max) for continuous
    pub domain: Option<(f64, f64)>,
    /// Categories for categorical variables
    pub categories: Option<Vec<String>>,
    /// Description
    pub description: Option<String>,
}

impl Variable {
    /// Create a new variable
    pub fn new(id: VariableId, name: &str, var_type: VariableType) -> Self {
        Self {
            id,
            name: name.to_string(),
            var_type,
            domain: None,
            categories: None,
            description: None,
        }
    }

    /// Set domain constraints
    pub fn with_domain(mut self, min: f64, max: f64) -> Self {
        self.domain = Some((min, max));
        self
    }

    /// Set categories
    pub fn with_categories(mut self, categories: Vec<String>) -> Self {
        self.categories = Some(categories);
        self
    }

    /// Set description
    pub fn with_description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }
}

/// Type alias for mechanism function
pub type MechanismFn = dyn Fn(&[Value]) -> Value + Send + Sync;

/// A mechanism (functional relationship) in a structural equation
#[derive(Clone)]
pub struct Mechanism {
    /// The function implementing the mechanism
    func: Arc<MechanismFn>,
    /// Optional noise distribution parameter
    pub noise_scale: f64,
}

impl Mechanism {
    /// Create a new mechanism from a function
    pub fn new<F>(func: F) -> Self
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        Self {
            func: Arc::new(func),
            noise_scale: 0.0,
        }
    }

    /// Create a mechanism with noise
    pub fn with_noise<F>(func: F, noise_scale: f64) -> Self
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        Self {
            func: Arc::new(func),
            noise_scale,
        }
    }

    /// Apply the mechanism to parent values
    pub fn apply(&self, parents: &[Value]) -> Value {
        (self.func)(parents)
    }
}

impl std::fmt::Debug for Mechanism {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mechanism")
            .field("noise_scale", &self.noise_scale)
            .finish()
    }
}

/// A structural equation: Y = f(Pa(Y), U_Y)
#[derive(Clone)]
pub struct StructuralEquation {
    /// Target variable this equation defines
    pub target: VariableId,
    /// Parent variables (causes)
    pub parents: Vec<VariableId>,
    /// The functional mechanism
    pub mechanism: Mechanism,
}

impl StructuralEquation {
    /// Create a new structural equation
    pub fn new(target: VariableId, parents: Vec<VariableId>, mechanism: Mechanism) -> Self {
        Self {
            target,
            parents,
            mechanism,
        }
    }

    /// Create a linear structural equation: Y = sum(coefficients[i] * parents[i])
    pub fn linear(parents: &[VariableId], coefficients: Vec<f64>) -> Self {
        let parents_vec = parents.to_vec();
        let coeffs = coefficients.clone();
        let mechanism = Mechanism::new(move |parent_values| {
            let sum: f64 = parent_values.iter()
                .zip(coeffs.iter())
                .map(|(v, c)| v.as_f64() * c)
                .sum();
            Value::Continuous(sum)
        });
        Self {
            target: VariableId(0), // Will be set when added to model
            parents: parents_vec,
            mechanism,
        }
    }

    /// Create a structural equation with additive noise: Y = sum(coefficients[i] * parents[i]) + noise
    pub fn with_noise(parents: &[VariableId], coefficients: Vec<f64>) -> Self {
        let parents_vec = parents.to_vec();
        let coeffs = coefficients.clone();
        let mechanism = Mechanism::with_noise(
            move |parent_values| {
                let sum: f64 = parent_values.iter()
                    .zip(coeffs.iter())
                    .map(|(v, c)| v.as_f64() * c)
                    .sum();
                Value::Continuous(sum)
            },
            1.0, // Default noise scale
        );
        Self {
            target: VariableId(0), // Will be set when added to model
            parents: parents_vec,
            mechanism,
        }
    }

    /// Compute the value of the target given parent values
    pub fn compute(&self, parent_values: &[Value]) -> Value {
        self.mechanism.apply(parent_values)
    }
}

impl std::fmt::Debug for StructuralEquation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructuralEquation")
            .field("target", &self.target)
            .field("parents", &self.parents)
            .finish()
    }
}

/// An intervention: do(X = x)
#[derive(Debug, Clone)]
pub struct Intervention {
    /// Variable being intervened on
    pub target: VariableId,
    /// Value to set
    pub value: Value,
}

impl Intervention {
    /// Create a new intervention
    pub fn new(target: VariableId, value: Value) -> Self {
        Self { target, value }
    }

    /// Create from variable name (requires model lookup)
    pub fn from_name(model: &CausalModel, name: &str, value: Value) -> Option<Self> {
        model.get_variable_id(name).map(|id| Self::new(id, value))
    }
}

/// Error types for causal model operations
#[derive(Debug, Clone, Error)]
pub enum CausalModelError {
    /// Variable not found
    #[error("Variable '{0}' not found")]
    VariableNotFound(String),

    /// Variable ID not found
    #[error("Variable ID {0:?} not found")]
    VariableIdNotFound(VariableId),

    /// Duplicate variable name
    #[error("Variable '{0}' already exists")]
    DuplicateVariable(String),

    /// DAG validation error
    #[error("Graph error: {0}")]
    GraphError(#[from] DAGValidationError),

    /// Missing structural equation
    #[error("No structural equation for variable {0:?}")]
    MissingEquation(VariableId),

    /// Invalid parent reference
    #[error("Invalid parent reference: {0:?}")]
    InvalidParent(VariableId),

    /// Type mismatch
    #[error("Type mismatch for variable {0}: expected {1:?}, got {2:?}")]
    TypeMismatch(String, VariableType, Value),

    /// Computation error
    #[error("Computation error: {0}")]
    ComputationError(String),
}

/// A Structural Causal Model (SCM)
#[derive(Debug, Clone)]
pub struct CausalModel {
    /// Variables in the model
    variables: HashMap<VariableId, Variable>,

    /// Name to ID mapping
    name_to_id: HashMap<String, VariableId>,

    /// Structural equations
    equations: HashMap<VariableId, StructuralEquation>,

    /// Underlying DAG structure
    graph: DirectedGraph,

    /// Next variable ID
    next_id: u32,

    /// Model name
    pub name: Option<String>,

    /// Model description
    pub description: Option<String>,

    /// Latent confounders (unobserved common causes)
    latent_confounders: Vec<(VariableId, VariableId)>,

    /// Intervention values (for mutilated models)
    intervention_values: HashMap<VariableId, Value>,
}

impl CausalModel {
    /// Create a new empty causal model
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            name_to_id: HashMap::new(),
            equations: HashMap::new(),
            graph: DirectedGraph::new(),
            next_id: 0,
            name: None,
            description: None,
            latent_confounders: Vec::new(),
            intervention_values: HashMap::new(),
        }
    }

    /// Create a model with a name
    pub fn with_name(name: &str) -> Self {
        let mut model = Self::new();
        model.name = Some(name.to_string());
        model
    }

    /// Add a variable to the model
    pub fn add_variable(&mut self, name: &str, var_type: VariableType) -> Result<VariableId, CausalModelError> {
        if self.name_to_id.contains_key(name) {
            return Err(CausalModelError::DuplicateVariable(name.to_string()));
        }

        let id = VariableId(self.next_id);
        self.next_id += 1;

        let variable = Variable::new(id, name, var_type);

        self.variables.insert(id, variable);
        self.name_to_id.insert(name.to_string(), id);
        self.graph.add_node_with_label(id.0, name);

        Ok(id)
    }

    /// Add a variable with full configuration
    pub fn add_variable_with_config(&mut self, variable: Variable) -> Result<VariableId, CausalModelError> {
        if self.name_to_id.contains_key(&variable.name) {
            return Err(CausalModelError::DuplicateVariable(variable.name.clone()));
        }

        let id = variable.id;
        self.name_to_id.insert(variable.name.clone(), id);
        self.graph.add_node_with_label(id.0, &variable.name);
        self.variables.insert(id, variable);

        // Update next_id if necessary
        if id.0 >= self.next_id {
            self.next_id = id.0 + 1;
        }

        Ok(id)
    }

    /// Add a causal edge from parent to child
    pub fn add_edge(&mut self, parent: VariableId, child: VariableId) -> Result<(), CausalModelError> {
        if !self.variables.contains_key(&parent) {
            return Err(CausalModelError::VariableIdNotFound(parent));
        }
        if !self.variables.contains_key(&child) {
            return Err(CausalModelError::VariableIdNotFound(child));
        }

        self.graph.add_edge(parent.0, child.0)?;
        Ok(())
    }

    /// Add a structural equation
    pub fn add_structural_equation(
        &mut self,
        target: VariableId,
        parents: &[VariableId],
        mechanism: Mechanism,
    ) -> Result<(), CausalModelError> {
        // Validate target exists
        if !self.variables.contains_key(&target) {
            return Err(CausalModelError::VariableIdNotFound(target));
        }

        // Validate parents exist and add edges
        for &parent in parents {
            if !self.variables.contains_key(&parent) {
                return Err(CausalModelError::InvalidParent(parent));
            }
            self.graph.add_edge(parent.0, target.0)?;
        }

        let equation = StructuralEquation::new(target, parents.to_vec(), mechanism);
        self.equations.insert(target, equation);

        Ok(())
    }

    /// Add a structural equation using variable names
    pub fn add_equation_by_name<F>(
        &mut self,
        target_name: &str,
        parent_names: &[&str],
        func: F,
    ) -> Result<(), CausalModelError>
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        let target = self.get_variable_id(target_name)
            .ok_or_else(|| CausalModelError::VariableNotFound(target_name.to_string()))?;

        let parents: Result<Vec<VariableId>, _> = parent_names
            .iter()
            .map(|&name| {
                self.get_variable_id(name)
                    .ok_or_else(|| CausalModelError::VariableNotFound(name.to_string()))
            })
            .collect();

        let mechanism = Mechanism::new(func);
        self.add_structural_equation(target, &parents?, mechanism)
    }

    /// Get variable ID by name
    pub fn get_variable_id(&self, name: &str) -> Option<VariableId> {
        self.name_to_id.get(name).copied()
    }

    /// Get variable name by ID
    pub fn get_variable_name(&self, id: &VariableId) -> Option<String> {
        self.variables.get(id).map(|v| v.name.clone())
    }

    /// Get variable by ID
    pub fn get_variable(&self, id: &VariableId) -> Option<&Variable> {
        self.variables.get(id)
    }

    /// Get all variables
    pub fn variables(&self) -> impl Iterator<Item = &Variable> {
        self.variables.values()
    }

    /// Get number of variables
    pub fn num_variables(&self) -> usize {
        self.variables.len()
    }

    /// Alias for num_variables (for API compatibility)
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Check if the model is a valid DAG
    pub fn is_dag(&self) -> bool {
        let mut graph = self.graph.clone();
        graph.topological_order().is_ok()
    }

    /// Set a structural equation for a variable (convenience method)
    pub fn set_structural_equation(&mut self, target: VariableId, equation: StructuralEquation) {
        // Add edges from parents to target
        for &parent in &equation.parents {
            let _ = self.graph.add_edge(parent.0, target.0);
        }

        // Create a new equation with the correct target
        let eq = StructuralEquation {
            target,
            parents: equation.parents,
            mechanism: equation.mechanism,
        };
        self.equations.insert(target, eq);
    }

    /// Add latent confounding between two variables
    pub fn add_latent_confounding(&mut self, var1: VariableId, var2: VariableId) {
        self.latent_confounders.push((var1, var2));
    }

    /// Check if two variables are unconfounded (no latent common cause)
    pub fn is_unconfounded(&self, var1: VariableId, var2: VariableId) -> bool {
        !self.latent_confounders.iter().any(|&(a, b)| {
            (a == var1 && b == var2) || (a == var2 && b == var1)
        })
    }

    /// Check if there are latent confounders affecting a variable
    pub fn has_latent_confounding(&self, var: VariableId) -> bool {
        self.latent_confounders.iter().any(|&(a, b)| a == var || b == var)
    }

    /// Get children of a variable
    pub fn children(&self, id: &VariableId) -> Option<Vec<VariableId>> {
        self.graph.children_of(id.0).map(|children| {
            children.iter().map(|&c| VariableId(c)).collect()
        })
    }

    /// Get parents of a variable
    pub fn parents(&self, id: &VariableId) -> Option<Vec<VariableId>> {
        self.graph.parents_of(id.0).map(|parents| {
            parents.iter().map(|&p| VariableId(p)).collect()
        })
    }

    /// Compute topological ordering
    pub fn topological_order(&self) -> Result<Vec<String>, CausalModelError> {
        let mut graph = self.graph.clone();
        let order = graph.topological_order()?;
        Ok(order.iter()
            .filter_map(|&id| self.variables.get(&VariableId(id)).map(|v| v.name.clone()))
            .collect())
    }

    /// Compute topological ordering of variable IDs
    pub fn topological_order_ids(&self) -> Result<Vec<VariableId>, CausalModelError> {
        let mut graph = self.graph.clone();
        let order = graph.topological_order()?;
        Ok(order.iter().map(|&id| VariableId(id)).collect())
    }

    /// Perform an intervention and compute the resulting distribution
    ///
    /// This implements the do-operator: do(X = x)
    pub fn intervene(&self, target: VariableId, value: Value) -> Result<MutilatedModel, CausalModelError> {
        if !self.variables.contains_key(&target) {
            return Err(CausalModelError::VariableIdNotFound(target));
        }

        // Create a mutilated model (clone with incoming edges removed)
        let mut mutilated = self.clone();

        // Remove incoming edges to the intervened variable
        if let Some(parents) = self.graph.parents_of(target.0).cloned() {
            for parent in parents {
                mutilated.graph.remove_edge(parent, target.0).ok();
            }
        }

        // Set the equation to return the intervention value
        let intervention_value = value.clone();
        mutilated.equations.insert(target, StructuralEquation {
            target,
            parents: vec![],
            mechanism: Mechanism::new(move |_| intervention_value.clone()),
        });

        // Store the intervention value for reference
        mutilated.intervention_values.insert(target, value);

        Ok(MutilatedModel { model: mutilated })
    }

    /// Perform an intervention using a slice of Intervention structs
    pub fn intervene_with(&self, interventions: &[Intervention]) -> Result<IntervenedModel, CausalModelError> {
        let intervention_map: HashMap<VariableId, Value> = interventions
            .iter()
            .map(|i| (i.target, i.value.clone()))
            .collect();

        Ok(IntervenedModel {
            base_model: self,
            interventions: intervention_map,
        })
    }

    /// Perform multiple simultaneous interventions
    pub fn multi_intervene(&self, interventions: &[(VariableId, Value)]) -> Result<MutilatedModel, CausalModelError> {
        let mut mutilated = self.clone();

        for (target, value) in interventions {
            if !self.variables.contains_key(target) {
                return Err(CausalModelError::VariableIdNotFound(*target));
            }

            // Remove incoming edges
            if let Some(parents) = self.graph.parents_of(target.0).cloned() {
                for parent in parents {
                    mutilated.graph.remove_edge(parent, target.0).ok();
                }
            }

            // Set constant equation
            let intervention_value = value.clone();
            mutilated.equations.insert(*target, StructuralEquation {
                target: *target,
                parents: vec![],
                mechanism: Mechanism::new(move |_| intervention_value.clone()),
            });

            mutilated.intervention_values.insert(*target, value.clone());
        }

        Ok(MutilatedModel { model: mutilated })
    }

    /// Forward simulation: compute all variable values given exogenous inputs
    pub fn forward_simulate(&self, exogenous: &HashMap<VariableId, Value>) -> Result<HashMap<VariableId, Value>, CausalModelError> {
        let order = self.topological_order_ids()?;
        let mut values: HashMap<VariableId, Value> = exogenous.clone();

        for var_id in order {
            if values.contains_key(&var_id) {
                continue; // Already set (exogenous or intervened)
            }

            if let Some(equation) = self.equations.get(&var_id) {
                let parent_values: Vec<Value> = equation.parents
                    .iter()
                    .map(|&p| values.get(&p).cloned().unwrap_or(Value::Missing))
                    .collect();

                let value = equation.compute(&parent_values);
                values.insert(var_id, value);
            } else {
                // No equation - must be exogenous
                if !exogenous.contains_key(&var_id) {
                    return Err(CausalModelError::MissingEquation(var_id));
                }
            }
        }

        Ok(values)
    }

    /// Check if two variables are d-separated given a conditioning set
    pub fn d_separated(&self, x: VariableId, y: VariableId, z: &[VariableId]) -> bool {
        let x_set = [x.0].into_iter().collect();
        let y_set = [y.0].into_iter().collect();
        let z_set: std::collections::HashSet<_> = z.iter().map(|id| id.0).collect();

        self.graph.d_separated(&x_set, &y_set, &z_set)
    }

    /// Get the structural equation for a variable
    pub fn get_equation(&self, id: &VariableId) -> Option<&StructuralEquation> {
        self.equations.get(id)
    }

    /// Get the underlying DAG
    pub fn graph(&self) -> &DirectedGraph {
        &self.graph
    }

    /// Check if the model is valid (all endogenous variables have equations)
    pub fn validate(&self) -> Result<(), CausalModelError> {
        // Check for cycles
        let mut graph = self.graph.clone();
        graph.topological_order()?;

        // Check that non-root variables have equations
        for (&id, _) in &self.variables {
            let parents = self.graph.parents_of(id.0);
            if parents.map(|p| !p.is_empty()).unwrap_or(false) {
                // Has parents, so should have an equation
                if !self.equations.contains_key(&id) {
                    return Err(CausalModelError::MissingEquation(id));
                }
            }
        }

        Ok(())
    }

    /// Compute the conditional distribution P(Y | observation)
    pub fn conditional_distribution(&self, observation: &Observation, target_name: &str) -> Result<Distribution, CausalModelError> {
        let target_id = self.get_variable_id(target_name)
            .ok_or_else(|| CausalModelError::VariableNotFound(target_name.to_string()))?;

        // Convert observation to exogenous values
        let mut exogenous = HashMap::new();
        for (name, value) in &observation.values {
            if let Some(id) = self.get_variable_id(name) {
                exogenous.insert(id, value.clone());
            }
        }

        // Forward simulate
        let result = self.forward_simulate(&exogenous)?;

        let value = result.get(&target_id)
            .cloned()
            .unwrap_or(Value::Missing);

        Ok(Distribution::point(target_id, value))
    }

    /// Compute the marginal distribution P(Y)
    pub fn marginal_distribution(&self, target_name: &str) -> Result<Distribution, CausalModelError> {
        let target_id = self.get_variable_id(target_name)
            .ok_or_else(|| CausalModelError::VariableNotFound(target_name.to_string()))?;

        // Simulate with empty exogenous (use default/zero values)
        let result = self.forward_simulate(&self.intervention_values)?;

        let value = result.get(&target_id)
            .cloned()
            .unwrap_or(Value::Missing);

        Ok(Distribution::point(target_id, value))
    }
}

/// An observation of variable values (for conditioning)
#[derive(Debug, Clone)]
pub struct Observation {
    /// Observed variable values by name
    pub values: HashMap<String, Value>,
}

impl Observation {
    /// Create a new observation from name-value pairs
    pub fn new(values: &[(&str, Value)]) -> Self {
        Self {
            values: values.iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        }
    }

    /// Create an empty observation
    pub fn empty() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Add an observed value
    pub fn observe(&mut self, var: &str, value: Value) {
        self.values.insert(var.to_string(), value);
    }

    /// Get an observed value
    pub fn get(&self, var: &str) -> Option<&Value> {
        self.values.get(var)
    }

    /// Check if a variable is observed
    pub fn is_observed(&self, var: &str) -> bool {
        self.values.contains_key(var)
    }
}

impl Default for CausalModel {
    fn default() -> Self {
        Self::new()
    }
}

/// A causal model with interventions applied
pub struct IntervenedModel<'a> {
    base_model: &'a CausalModel,
    interventions: HashMap<VariableId, Value>,
}

impl<'a> IntervenedModel<'a> {
    /// Simulate the intervened model
    pub fn simulate(&self, exogenous: &HashMap<VariableId, Value>) -> Result<HashMap<VariableId, Value>, CausalModelError> {
        let order = self.base_model.topological_order_ids()?;
        let mut values: HashMap<VariableId, Value> = exogenous.clone();

        // Apply interventions first
        for (var, val) in &self.interventions {
            values.insert(*var, val.clone());
        }

        for var_id in order {
            if values.contains_key(&var_id) {
                continue;
            }

            // Check if this variable is intervened
            if let Some(intervention_value) = self.interventions.get(&var_id) {
                values.insert(var_id, intervention_value.clone());
                continue;
            }

            if let Some(equation) = self.base_model.equations.get(&var_id) {
                let parent_values: Vec<Value> = equation.parents
                    .iter()
                    .map(|&p| values.get(&p).cloned().unwrap_or(Value::Missing))
                    .collect();

                let value = equation.compute(&parent_values);
                values.insert(var_id, value);
            }
        }

        Ok(values)
    }

    /// Check if a variable is intervened
    pub fn is_intervened(&self, var: VariableId) -> bool {
        self.interventions.contains_key(&var)
    }

    /// Get the intervention value for a variable
    pub fn intervention_value(&self, var: VariableId) -> Option<&Value> {
        self.interventions.get(&var)
    }
}

/// A mutilated causal model (with interventions applied)
///
/// This is a complete copy of the model with incoming edges to intervened
/// variables removed, representing the do-operator graph transformation.
#[derive(Debug, Clone)]
pub struct MutilatedModel {
    /// The mutilated model
    pub model: CausalModel,
}

impl MutilatedModel {
    /// Get parents of a variable in the mutilated model
    pub fn parents(&self, id: &VariableId) -> Result<Vec<VariableId>, CausalModelError> {
        self.model.parents(id).ok_or(CausalModelError::VariableIdNotFound(*id))
    }

    /// Compute the value of a variable by name
    pub fn compute(&self, var_name: &str) -> Result<Value, CausalModelError> {
        let var_id = self.model.get_variable_id(var_name)
            .ok_or_else(|| CausalModelError::VariableNotFound(var_name.to_string()))?;

        // Forward simulate with intervention values as exogenous
        let result = self.model.forward_simulate(&self.model.intervention_values)?;

        result.get(&var_id)
            .cloned()
            .ok_or_else(|| CausalModelError::ComputationError(format!("Variable {} not computed", var_name)))
    }

    /// Get the marginal distribution of a variable (point mass for deterministic models)
    pub fn marginal_distribution(&self, var_name: &str) -> Result<Distribution, CausalModelError> {
        let value = self.compute(var_name)?;
        let var_id = self.model.get_variable_id(var_name)
            .ok_or_else(|| CausalModelError::VariableNotFound(var_name.to_string()))?;

        Ok(Distribution::point(var_id, value))
    }

    /// Simulate the mutilated model with optional exogenous inputs
    pub fn simulate(&self, exogenous: &HashMap<VariableId, Value>) -> Result<HashMap<VariableId, Value>, CausalModelError> {
        // Merge exogenous with intervention values
        let mut all_exogenous = self.model.intervention_values.clone();
        all_exogenous.extend(exogenous.iter().map(|(k, v)| (*k, v.clone())));
        self.model.forward_simulate(&all_exogenous)
    }

    /// Check if a variable is intervened
    pub fn is_intervened(&self, var: &VariableId) -> bool {
        self.model.intervention_values.contains_key(var)
    }

    /// Check if the mutilated model is still a DAG
    pub fn is_dag(&self) -> bool {
        self.model.is_dag()
    }

    /// Get the underlying model
    pub fn inner(&self) -> &CausalModel {
        &self.model
    }
}

/// A simple probability distribution representation
#[derive(Debug, Clone)]
pub struct Distribution {
    /// Variable ID
    pub variable: VariableId,
    /// Value (point mass for deterministic)
    pub value: Value,
    /// Probability mass
    pub probability: f64,
}

impl Distribution {
    /// Create a point mass distribution
    pub fn point(variable: VariableId, value: Value) -> Self {
        Self {
            variable,
            value,
            probability: 1.0,
        }
    }

    /// Get the expected value (for continuous)
    pub fn expected_value(&self) -> f64 {
        self.value.as_f64()
    }
}

impl PartialEq for Distribution {
    fn eq(&self, other: &Self) -> bool {
        self.variable == other.variable &&
        (self.probability - other.probability).abs() < 1e-10 &&
        match (&self.value, &other.value) {
            (Value::Continuous(a), Value::Continuous(b)) => (a - b).abs() < 1e-10,
            (Value::Discrete(a), Value::Discrete(b)) => a == b,
            (Value::Binary(a), Value::Binary(b)) => a == b,
            _ => false,
        }
    }
}

/// Builder for creating causal models fluently
pub struct CausalModelBuilder {
    model: CausalModel,
}

impl CausalModelBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            model: CausalModel::new(),
        }
    }

    /// Create a builder with a model name
    pub fn with_name(name: &str) -> Self {
        Self {
            model: CausalModel::with_name(name),
        }
    }

    /// Add a continuous variable
    pub fn add_continuous(mut self, name: &str) -> Self {
        self.model.add_variable(name, VariableType::Continuous).ok();
        self
    }

    /// Add a binary variable
    pub fn add_binary(mut self, name: &str) -> Self {
        self.model.add_variable(name, VariableType::Binary).ok();
        self
    }

    /// Add a discrete variable
    pub fn add_discrete(mut self, name: &str) -> Self {
        self.model.add_variable(name, VariableType::Discrete).ok();
        self
    }

    /// Add a causal relationship
    pub fn add_cause(mut self, cause: &str, effect: &str) -> Self {
        if let (Some(c), Some(e)) = (
            self.model.get_variable_id(cause),
            self.model.get_variable_id(effect),
        ) {
            self.model.add_edge(c, e).ok();
        }
        self
    }

    /// Add a structural equation
    pub fn with_equation<F>(mut self, target: &str, parents: &[&str], func: F) -> Self
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        self.model.add_equation_by_name(target, parents, func).ok();
        self
    }

    /// Build the model
    pub fn build(self) -> CausalModel {
        self.model
    }
}

impl Default for CausalModelBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_model() {
        let mut model = CausalModel::new();
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();

        assert_eq!(model.num_variables(), 2);
        assert_eq!(model.get_variable_id("X"), Some(x));
        assert_eq!(model.get_variable_id("Y"), Some(y));
    }

    #[test]
    fn test_add_edges() {
        let mut model = CausalModel::new();
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();

        model.add_edge(x, y).unwrap();

        assert_eq!(model.children(&x), Some(vec![y]));
        assert_eq!(model.parents(&y), Some(vec![x]));
    }

    #[test]
    fn test_structural_equation() {
        let mut model = CausalModel::new();
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();

        // Y = 2*X + 1
        let mechanism = Mechanism::new(|parents| {
            let x_val = parents[0].as_f64();
            Value::Continuous(2.0 * x_val + 1.0)
        });

        model.add_structural_equation(y, &[x], mechanism).unwrap();

        // Simulate
        let mut exogenous = HashMap::new();
        exogenous.insert(x, Value::Continuous(3.0));

        let result = model.forward_simulate(&exogenous).unwrap();

        assert_eq!(result.get(&y), Some(&Value::Continuous(7.0)));
    }

    #[test]
    fn test_intervention() {
        let mut model = CausalModel::new();
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();
        let z = model.add_variable("Z", VariableType::Continuous).unwrap();

        // Y = X, Z = Y
        model.add_structural_equation(y, &[x], Mechanism::new(|p| p[0].clone())).unwrap();
        model.add_structural_equation(z, &[y], Mechanism::new(|p| p[0].clone())).unwrap();

        // Intervene: do(Y = 5)
        let intervention = Intervention::new(y, Value::Continuous(5.0));
        let intervened = model.intervene(&[intervention]).unwrap();

        let mut exogenous = HashMap::new();
        exogenous.insert(x, Value::Continuous(10.0)); // X = 10

        let result = intervened.simulate(&exogenous).unwrap();

        // X should still be 10
        assert_eq!(result.get(&x).unwrap().as_f64(), 10.0);
        // Y should be 5 (intervened)
        assert_eq!(result.get(&y).unwrap().as_f64(), 5.0);
        // Z should be 5 (from Y)
        assert_eq!(result.get(&z).unwrap().as_f64(), 5.0);
    }

    #[test]
    fn test_builder() {
        let model = CausalModelBuilder::new()
            .add_continuous("Age")
            .add_continuous("Income")
            .add_binary("Employed")
            .add_cause("Age", "Income")
            .add_cause("Employed", "Income")
            .build();

        assert_eq!(model.num_variables(), 3);

        let age = model.get_variable_id("Age").unwrap();
        let income = model.get_variable_id("Income").unwrap();

        assert_eq!(model.children(&age), Some(vec![income]));
    }

    #[test]
    fn test_d_separation() {
        let mut model = CausalModel::new();

        // Chain: X -> Z -> Y
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let z = model.add_variable("Z", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();

        model.add_edge(x, z).unwrap();
        model.add_edge(z, y).unwrap();

        // X and Y are NOT d-separated given empty set
        assert!(!model.d_separated(x, y, &[]));

        // X and Y ARE d-separated given Z
        assert!(model.d_separated(x, y, &[z]));
    }

    #[test]
    fn test_topological_order() {
        let mut model = CausalModel::new();

        let a = model.add_variable("A", VariableType::Continuous).unwrap();
        let b = model.add_variable("B", VariableType::Continuous).unwrap();
        let c = model.add_variable("C", VariableType::Continuous).unwrap();

        model.add_edge(a, b).unwrap();
        model.add_edge(b, c).unwrap();

        let order = model.topological_order().unwrap();

        let pos_a = order.iter().position(|n| n == "A").unwrap();
        let pos_b = order.iter().position(|n| n == "B").unwrap();
        let pos_c = order.iter().position(|n| n == "C").unwrap();

        assert!(pos_a < pos_b);
        assert!(pos_b < pos_c);
    }

    #[test]
    fn test_value_conversions() {
        let continuous = Value::Continuous(3.14);
        assert!((continuous.as_f64() - 3.14).abs() < 1e-10);

        let binary = Value::Binary(true);
        assert_eq!(binary.as_bool(), Some(true));
        assert!((binary.as_f64() - 1.0).abs() < 1e-10);

        let discrete = Value::Discrete(42);
        assert!((discrete.as_f64() - 42.0).abs() < 1e-10);

        let missing = Value::Missing;
        assert!(missing.is_missing());
        assert!(missing.as_f64().is_nan());
    }

    #[test]
    fn test_duplicate_variable() {
        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();

        let result = model.add_variable("X", VariableType::Continuous);
        assert!(matches!(result, Err(CausalModelError::DuplicateVariable(_))));
    }

    #[test]
    fn test_model_validation() {
        let mut model = CausalModel::new();
        let x = model.add_variable("X", VariableType::Continuous).unwrap();
        let y = model.add_variable("Y", VariableType::Continuous).unwrap();

        model.add_edge(x, y).unwrap();

        // Should fail - Y has parents but no equation
        let result = model.validate();
        assert!(matches!(result, Err(CausalModelError::MissingEquation(_))));

        // Add equation
        model.add_structural_equation(y, &[x], Mechanism::new(|p| p[0].clone())).unwrap();

        // Should pass now
        model.validate().unwrap();
    }
}
