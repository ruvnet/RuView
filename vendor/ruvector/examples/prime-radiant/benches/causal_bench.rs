//! Causal Reasoning Benchmarks for Prime-Radiant
//!
//! Benchmarks for causal inference operations including:
//! - Intervention computation (do-calculus)
//! - Counterfactual queries
//! - Causal abstraction verification
//! - Structural causal model operations
//!
//! Target metrics:
//! - Intervention: < 1ms per intervention
//! - Counterfactual: < 5ms per query
//! - Abstraction verification: < 10ms for moderate models

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::{HashMap, HashSet, VecDeque};

// ============================================================================
// CAUSAL MODEL TYPES
// ============================================================================

/// Variable identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct VariableId(usize);

/// Variable value
#[derive(Clone, Debug)]
enum Value {
    Continuous(f64),
    Discrete(i64),
    Vector(Vec<f64>),
}

impl Value {
    fn as_f64(&self) -> f64 {
        match self {
            Value::Continuous(v) => *v,
            Value::Discrete(v) => *v as f64,
            Value::Vector(v) => v.iter().sum(),
        }
    }
}

/// Structural equation: V = f(Pa(V), U_V)
struct StructuralEquation {
    variable: VariableId,
    parents: Vec<VariableId>,
    /// Function mapping parent values to variable value
    function: Box<dyn Fn(&[Value]) -> Value + Send + Sync>,
}

/// Structural Causal Model
struct CausalModel {
    variables: HashMap<VariableId, String>,
    variable_ids: HashMap<String, VariableId>,
    parents: HashMap<VariableId, Vec<VariableId>>,
    children: HashMap<VariableId, Vec<VariableId>>,
    equations: HashMap<VariableId, Box<dyn Fn(&[Value]) -> Value + Send + Sync>>,
    exogenous: HashMap<VariableId, Value>,
    next_id: usize,
}

impl CausalModel {
    fn new() -> Self {
        Self {
            variables: HashMap::new(),
            variable_ids: HashMap::new(),
            parents: HashMap::new(),
            children: HashMap::new(),
            equations: HashMap::new(),
            exogenous: HashMap::new(),
            next_id: 0,
        }
    }

    fn add_variable(&mut self, name: &str) -> VariableId {
        let id = VariableId(self.next_id);
        self.next_id += 1;

        self.variables.insert(id, name.to_string());
        self.variable_ids.insert(name.to_string(), id);
        self.parents.insert(id, Vec::new());
        self.children.insert(id, Vec::new());

        // Default exogenous value
        self.exogenous.insert(id, Value::Continuous(0.0));

        id
    }

    fn add_edge(&mut self, from: VariableId, to: VariableId) {
        self.parents.get_mut(&to).unwrap().push(from);
        self.children.get_mut(&from).unwrap().push(to);
    }

    fn set_equation<F>(&mut self, var: VariableId, func: F)
    where
        F: Fn(&[Value]) -> Value + Send + Sync + 'static,
    {
        self.equations.insert(var, Box::new(func));
    }

    fn set_exogenous(&mut self, var: VariableId, value: Value) {
        self.exogenous.insert(var, value);
    }

    fn topological_order(&self) -> Vec<VariableId> {
        let mut order = Vec::new();
        let mut visited = HashSet::new();
        let mut temp_mark = HashSet::new();

        fn visit(
            id: VariableId,
            parents: &HashMap<VariableId, Vec<VariableId>>,
            visited: &mut HashSet<VariableId>,
            temp_mark: &mut HashSet<VariableId>,
            order: &mut Vec<VariableId>,
        ) {
            if visited.contains(&id) {
                return;
            }
            if temp_mark.contains(&id) {
                return; // Cycle detected
            }

            temp_mark.insert(id);

            for &parent in parents.get(&id).unwrap_or(&vec![]) {
                visit(parent, parents, visited, temp_mark, order);
            }

            temp_mark.remove(&id);
            visited.insert(id);
            order.push(id);
        }

        for &id in self.variables.keys() {
            visit(id, &self.parents, &mut visited, &mut temp_mark, &mut order);
        }

        order
    }

    /// Compute values given current exogenous variables
    fn forward(&self) -> HashMap<VariableId, Value> {
        let mut values = HashMap::new();
        let order = self.topological_order();

        for id in order {
            let parent_ids = self.parents.get(&id).unwrap();
            let parent_values: Vec<Value> = parent_ids
                .iter()
                .map(|&pid| values.get(&pid).cloned().unwrap_or(Value::Continuous(0.0)))
                .collect();

            let value = if let Some(func) = self.equations.get(&id) {
                // Combine exogenous with structural equation
                let exo = self.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0));
                let base = func(&parent_values);
                Value::Continuous(base.as_f64() + exo.as_f64())
            } else {
                self.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0))
            };

            values.insert(id, value);
        }

        values
    }
}

// ============================================================================
// INTERVENTION
// ============================================================================

/// Intervention: do(X = x)
#[derive(Clone)]
struct Intervention {
    variable: VariableId,
    value: Value,
}

impl Intervention {
    fn new(variable: VariableId, value: Value) -> Self {
        Self { variable, value }
    }
}

/// Apply intervention and compute resulting distribution
fn apply_intervention(
    model: &CausalModel,
    intervention: &Intervention,
) -> HashMap<VariableId, Value> {
    let mut values = HashMap::new();
    let order = model.topological_order();

    for id in order {
        if id == intervention.variable {
            // Override with intervention value
            values.insert(id, intervention.value.clone());
        } else {
            let parent_ids = model.parents.get(&id).unwrap();
            let parent_values: Vec<Value> = parent_ids
                .iter()
                .map(|&pid| values.get(&pid).cloned().unwrap_or(Value::Continuous(0.0)))
                .collect();

            let value = if let Some(func) = model.equations.get(&id) {
                let exo = model.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0));
                let base = func(&parent_values);
                Value::Continuous(base.as_f64() + exo.as_f64())
            } else {
                model.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0))
            };

            values.insert(id, value);
        }
    }

    values
}

/// Apply multiple interventions
fn apply_multi_intervention(
    model: &CausalModel,
    interventions: &[Intervention],
) -> HashMap<VariableId, Value> {
    let intervention_map: HashMap<VariableId, Value> = interventions
        .iter()
        .map(|i| (i.variable, i.value.clone()))
        .collect();

    let mut values = HashMap::new();
    let order = model.topological_order();

    for id in order {
        if let Some(value) = intervention_map.get(&id) {
            values.insert(id, value.clone());
        } else {
            let parent_ids = model.parents.get(&id).unwrap();
            let parent_values: Vec<Value> = parent_ids
                .iter()
                .map(|&pid| values.get(&pid).cloned().unwrap_or(Value::Continuous(0.0)))
                .collect();

            let value = if let Some(func) = model.equations.get(&id) {
                let exo = model.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0));
                let base = func(&parent_values);
                Value::Continuous(base.as_f64() + exo.as_f64())
            } else {
                model.exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0))
            };

            values.insert(id, value);
        }
    }

    values
}

// ============================================================================
// COUNTERFACTUAL REASONING
// ============================================================================

/// Counterfactual query: Y_x(u) where we observed Y = y
struct CounterfactualQuery {
    /// The variable we're asking about
    target: VariableId,
    /// The intervention
    intervention: Intervention,
    /// Observed facts
    observations: HashMap<VariableId, Value>,
}

/// Compute counterfactual using abduction-action-prediction
fn compute_counterfactual(
    model: &CausalModel,
    query: &CounterfactualQuery,
) -> Option<Value> {
    // Step 1: Abduction - infer exogenous variables from observations
    let inferred_exogenous = abduct_exogenous(model, &query.observations)?;

    // Step 2: Action - create modified model with intervention
    // (We don't actually modify the model, we use the intervention directly)

    // Step 3: Prediction - compute outcome under intervention with inferred exogenous
    let mut values = HashMap::new();
    let order = model.topological_order();

    for id in order {
        if id == query.intervention.variable {
            values.insert(id, query.intervention.value.clone());
        } else {
            let parent_ids = model.parents.get(&id).unwrap();
            let parent_values: Vec<Value> = parent_ids
                .iter()
                .map(|&pid| values.get(&pid).cloned().unwrap_or(Value::Continuous(0.0)))
                .collect();

            let value = if let Some(func) = model.equations.get(&id) {
                let exo = inferred_exogenous
                    .get(&id)
                    .cloned()
                    .unwrap_or(Value::Continuous(0.0));
                let base = func(&parent_values);
                Value::Continuous(base.as_f64() + exo.as_f64())
            } else {
                inferred_exogenous
                    .get(&id)
                    .cloned()
                    .unwrap_or(Value::Continuous(0.0))
            };

            values.insert(id, value);
        }
    }

    values.get(&query.target).cloned()
}

/// Abduct exogenous variables from observations
fn abduct_exogenous(
    model: &CausalModel,
    observations: &HashMap<VariableId, Value>,
) -> Option<HashMap<VariableId, Value>> {
    let mut exogenous = model.exogenous.clone();
    let order = model.topological_order();

    // For each observed variable, infer the exogenous noise
    let mut computed_values = HashMap::new();

    for id in order {
        let parent_ids = model.parents.get(&id).unwrap();
        let parent_values: Vec<Value> = parent_ids
            .iter()
            .map(|&pid| {
                computed_values
                    .get(&pid)
                    .cloned()
                    .unwrap_or(Value::Continuous(0.0))
            })
            .collect();

        if let Some(observed) = observations.get(&id) {
            // Infer exogenous: U = Y - f(Pa)
            if let Some(func) = model.equations.get(&id) {
                let structural_part = func(&parent_values).as_f64();
                let inferred_exo = observed.as_f64() - structural_part;
                exogenous.insert(id, Value::Continuous(inferred_exo));
            }
            computed_values.insert(id, observed.clone());
        } else {
            // Compute from parents
            let value = if let Some(func) = model.equations.get(&id) {
                let exo = exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0));
                let base = func(&parent_values);
                Value::Continuous(base.as_f64() + exo.as_f64())
            } else {
                exogenous.get(&id).cloned().unwrap_or(Value::Continuous(0.0))
            };
            computed_values.insert(id, value);
        }
    }

    Some(exogenous)
}

// ============================================================================
// CAUSAL ABSTRACTION
// ============================================================================

/// Map between low-level and high-level causal models
struct CausalAbstraction {
    /// Low-level model
    low_level: CausalModel,
    /// High-level model
    high_level: CausalModel,
    /// Variable mapping: high-level -> set of low-level variables
    variable_map: HashMap<VariableId, Vec<VariableId>>,
    /// Value mapping: how to aggregate low-level values
    value_aggregator: Box<dyn Fn(&[Value]) -> Value + Send + Sync>,
}

impl CausalAbstraction {
    fn new(low_level: CausalModel, high_level: CausalModel) -> Self {
        Self {
            low_level,
            high_level,
            variable_map: HashMap::new(),
            value_aggregator: Box::new(|vals: &[Value]| {
                let sum: f64 = vals.iter().map(|v| v.as_f64()).sum();
                Value::Continuous(sum / vals.len().max(1) as f64)
            }),
        }
    }

    fn add_mapping(&mut self, high_var: VariableId, low_vars: Vec<VariableId>) {
        self.variable_map.insert(high_var, low_vars);
    }

    /// Verify abstraction consistency: interventions commute
    fn verify_consistency(&self, intervention: &Intervention) -> bool {
        // High-level: intervene and compute
        let high_values = apply_intervention(&self.high_level, intervention);

        // Low-level: intervene on corresponding variables and aggregate
        let low_vars = self.variable_map.get(&intervention.variable);
        if low_vars.is_none() {
            return false;
        }

        let low_interventions: Vec<Intervention> = low_vars
            .unwrap()
            .iter()
            .map(|&v| Intervention::new(v, intervention.value.clone()))
            .collect();

        let low_values = apply_multi_intervention(&self.low_level, &low_interventions);

        // Compare aggregated low-level values with high-level values
        for (&high_var, low_vars) in &self.variable_map {
            let high_val = high_values.get(&high_var).map(|v| v.as_f64()).unwrap_or(0.0);

            let low_vals: Vec<Value> = low_vars
                .iter()
                .filter_map(|&lv| low_values.get(&lv).cloned())
                .collect();

            let aggregated = (self.value_aggregator)(&low_vals).as_f64();

            if (high_val - aggregated).abs() > 1e-6 {
                return false;
            }
        }

        true
    }

    /// Compute abstraction error
    fn compute_abstraction_error(&self, num_samples: usize) -> f64 {
        let mut total_error = 0.0;

        for i in 0..num_samples {
            // Random intervention value
            let value = Value::Continuous((i as f64 * 0.1).sin() * 10.0);

            // Pick a random variable to intervene on
            let high_vars: Vec<_> = self.high_level.variables.keys().copied().collect();
            if high_vars.is_empty() {
                continue;
            }
            let var_idx = i % high_vars.len();
            let intervention = Intervention::new(high_vars[var_idx], value);

            // Compute values
            let high_values = apply_intervention(&self.high_level, &intervention);

            let low_vars = self.variable_map.get(&intervention.variable);
            if low_vars.is_none() {
                continue;
            }

            let low_interventions: Vec<Intervention> = low_vars
                .unwrap()
                .iter()
                .map(|&v| Intervention::new(v, intervention.value.clone()))
                .collect();

            let low_values = apply_multi_intervention(&self.low_level, &low_interventions);

            // Compute error
            for (&high_var, low_vars) in &self.variable_map {
                let high_val = high_values.get(&high_var).map(|v| v.as_f64()).unwrap_or(0.0);

                let low_vals: Vec<Value> = low_vars
                    .iter()
                    .filter_map(|&lv| low_values.get(&lv).cloned())
                    .collect();

                let aggregated = (self.value_aggregator)(&low_vals).as_f64();
                total_error += (high_val - aggregated).powi(2);
            }
        }

        (total_error / num_samples.max(1) as f64).sqrt()
    }
}

// ============================================================================
// CAUSAL EFFECT ESTIMATION
// ============================================================================

/// Average Treatment Effect
fn compute_ate(
    model: &CausalModel,
    treatment: VariableId,
    outcome: VariableId,
    treatment_values: (f64, f64), // (control, treated)
) -> f64 {
    // E[Y | do(X = treated)] - E[Y | do(X = control)]
    let intervention_treated = Intervention::new(treatment, Value::Continuous(treatment_values.1));
    let intervention_control = Intervention::new(treatment, Value::Continuous(treatment_values.0));

    let values_treated = apply_intervention(model, &intervention_treated);
    let values_control = apply_intervention(model, &intervention_control);

    let y_treated = values_treated.get(&outcome).map(|v| v.as_f64()).unwrap_or(0.0);
    let y_control = values_control.get(&outcome).map(|v| v.as_f64()).unwrap_or(0.0);

    y_treated - y_control
}

// ============================================================================
// BENCHMARK DATA GENERATORS
// ============================================================================

fn create_chain_model(length: usize) -> CausalModel {
    let mut model = CausalModel::new();
    let mut vars = Vec::new();

    for i in 0..length {
        let var = model.add_variable(&format!("V{}", i));
        vars.push(var);

        if i > 0 {
            model.add_edge(vars[i - 1], var);

            let parent_var = vars[i - 1];
            model.set_equation(var, move |parents| {
                if parents.is_empty() {
                    Value::Continuous(0.0)
                } else {
                    Value::Continuous(parents[0].as_f64() * 0.8 + 0.5)
                }
            });
        }
    }

    model
}

fn create_diamond_model(num_layers: usize, width: usize) -> CausalModel {
    let mut model = CausalModel::new();
    let mut layers: Vec<Vec<VariableId>> = Vec::new();

    // Create layers
    for layer in 0..num_layers {
        let layer_width = if layer == 0 || layer == num_layers - 1 {
            1
        } else {
            width
        };

        let mut layer_vars = Vec::new();
        for i in 0..layer_width {
            let var = model.add_variable(&format!("L{}_{}", layer, i));
            layer_vars.push(var);

            // Connect to previous layer
            if layer > 0 {
                for &parent in &layers[layer - 1] {
                    model.add_edge(parent, var);
                }

                model.set_equation(var, |parents| {
                    let sum: f64 = parents.iter().map(|p| p.as_f64()).sum();
                    Value::Continuous(sum / parents.len().max(1) as f64 + 0.1)
                });
            }
        }

        layers.push(layer_vars);
    }

    model
}

fn create_dense_model(num_vars: usize, density: f64, seed: u64) -> CausalModel {
    let mut model = CausalModel::new();
    let mut vars = Vec::new();

    // Create variables
    for i in 0..num_vars {
        let var = model.add_variable(&format!("V{}", i));
        vars.push(var);
    }

    // Add edges (respecting DAG structure: only forward edges)
    let mut rng_state = seed;
    for i in 0..num_vars {
        for j in (i + 1)..num_vars {
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let random = (rng_state >> 33) as f64 / (u32::MAX as f64);

            if random < density {
                model.add_edge(vars[i], vars[j]);
            }
        }
    }

    // Set equations
    for i in 1..num_vars {
        model.set_equation(vars[i], |parents| {
            let sum: f64 = parents.iter().map(|p| p.as_f64()).sum();
            Value::Continuous(sum * 0.5 + 0.1)
        });
    }

    model
}

// ============================================================================
// BENCHMARKS
// ============================================================================

fn bench_intervention(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/intervention");
    group.sample_size(100);

    for &size in &[10, 50, 100, 200] {
        let model = create_chain_model(size);
        let var = VariableId(size / 2); // Intervene in middle
        let intervention = Intervention::new(var, Value::Continuous(1.0));

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("chain", size),
            &(&model, &intervention),
            |b, (model, intervention)| {
                b.iter(|| black_box(apply_intervention(black_box(model), black_box(intervention))))
            },
        );
    }

    for &size in &[10, 25, 50] {
        let model = create_diamond_model(4, size);
        let var = VariableId(0);
        let intervention = Intervention::new(var, Value::Continuous(1.0));

        let total_vars = 2 + 2 * size; // 1 + size + size + 1
        group.throughput(Throughput::Elements(total_vars as u64));

        group.bench_with_input(
            BenchmarkId::new("diamond", size),
            &(&model, &intervention),
            |b, (model, intervention)| {
                b.iter(|| black_box(apply_intervention(black_box(model), black_box(intervention))))
            },
        );
    }

    group.finish();
}

fn bench_multi_intervention(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/multi_intervention");
    group.sample_size(50);

    for &num_interventions in &[1, 5, 10, 20] {
        let model = create_dense_model(100, 0.1, 42);
        let interventions: Vec<Intervention> = (0..num_interventions)
            .map(|i| Intervention::new(VariableId(i * 5), Value::Continuous(1.0)))
            .collect();

        group.throughput(Throughput::Elements(num_interventions as u64));

        group.bench_with_input(
            BenchmarkId::new("dense_100", num_interventions),
            &(&model, &interventions),
            |b, (model, interventions)| {
                b.iter(|| black_box(apply_multi_intervention(black_box(model), black_box(interventions))))
            },
        );
    }

    group.finish();
}

fn bench_counterfactual(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/counterfactual");
    group.sample_size(50);

    for &size in &[10, 25, 50, 100] {
        let model = create_chain_model(size);

        // Observe last variable
        let mut observations = HashMap::new();
        observations.insert(VariableId(size - 1), Value::Continuous(5.0));

        let query = CounterfactualQuery {
            target: VariableId(size - 1),
            intervention: Intervention::new(VariableId(0), Value::Continuous(2.0)),
            observations,
        };

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("chain", size),
            &(&model, &query),
            |b, (model, query)| {
                b.iter(|| black_box(compute_counterfactual(black_box(model), black_box(query))))
            },
        );
    }

    group.finish();
}

fn bench_abstraction_verification(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/abstraction");
    group.sample_size(30);

    for &low_size in &[20, 50, 100] {
        let high_size = low_size / 5;

        let low_model = create_chain_model(low_size);
        let high_model = create_chain_model(high_size);

        let mut abstraction = CausalAbstraction::new(low_model, high_model);

        // Map high-level vars to groups of low-level vars
        for i in 0..high_size {
            let low_vars: Vec<VariableId> = (0..5)
                .map(|j| VariableId(i * 5 + j))
                .collect();
            abstraction.add_mapping(VariableId(i), low_vars);
        }

        let intervention = Intervention::new(VariableId(0), Value::Continuous(1.0));

        group.throughput(Throughput::Elements(low_size as u64));

        group.bench_with_input(
            BenchmarkId::new("verify_single", low_size),
            &(&abstraction, &intervention),
            |b, (abstraction, intervention)| {
                b.iter(|| black_box(abstraction.verify_consistency(black_box(intervention))))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("compute_error", low_size),
            &abstraction,
            |b, abstraction| {
                b.iter(|| black_box(abstraction.compute_abstraction_error(10)))
            },
        );
    }

    group.finish();
}

fn bench_ate(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/ate");
    group.sample_size(100);

    for &size in &[10, 50, 100] {
        let model = create_dense_model(size, 0.15, 42);
        let treatment = VariableId(0);
        let outcome = VariableId(size - 1);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("dense", size),
            &(&model, treatment, outcome),
            |b, (model, treatment, outcome)| {
                b.iter(|| {
                    black_box(compute_ate(
                        black_box(model),
                        *treatment,
                        *outcome,
                        (0.0, 1.0),
                    ))
                })
            },
        );
    }

    group.finish();
}

fn bench_topological_sort(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/topological_sort");
    group.sample_size(100);

    for &size in &[50, 100, 200, 500] {
        let model = create_dense_model(size, 0.1, 42);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("dense", size),
            &model,
            |b, model| {
                b.iter(|| black_box(model.topological_order()))
            },
        );
    }

    group.finish();
}

fn bench_forward_propagation(c: &mut Criterion) {
    let mut group = c.benchmark_group("causal/forward");
    group.sample_size(50);

    for &size in &[50, 100, 200] {
        let model = create_dense_model(size, 0.1, 42);

        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::new("dense", size),
            &model,
            |b, model| {
                b.iter(|| black_box(model.forward()))
            },
        );
    }

    for &(layers, width) in &[(3, 10), (5, 10), (5, 20)] {
        let model = create_diamond_model(layers, width);
        let total_vars = 2 + (layers - 2) * width;

        group.throughput(Throughput::Elements(total_vars as u64));

        group.bench_with_input(
            BenchmarkId::new(format!("diamond_{}x{}", layers, width), total_vars),
            &model,
            |b, model| {
                b.iter(|| black_box(model.forward()))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_intervention,
    bench_multi_intervention,
    bench_counterfactual,
    bench_abstraction_verification,
    bench_ate,
    bench_topological_sort,
    bench_forward_propagation,
);
criterion_main!(benches);
