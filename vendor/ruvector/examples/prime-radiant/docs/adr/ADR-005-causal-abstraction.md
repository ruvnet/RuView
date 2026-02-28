# ADR-005: Causal Abstraction for Mechanistic Interpretability

**Status**: Accepted
**Date**: 2024-12-15
**Authors**: RuVector Team
**Supersedes**: None

---

## Context

Understanding *why* neural networks produce their outputs requires more than correlation analysis. We need:

1. **Causal mechanisms**: Which components actually cause specific behaviors
2. **Interventional reasoning**: What happens when we modify internal states
3. **Abstraction levels**: How low-level computations relate to high-level concepts
4. **Alignment verification**: Whether learned mechanisms match intended behavior

Traditional interpretability approaches provide:
- Attention visualization (correlational, not causal)
- Gradient-based attribution (local approximations)
- Probing classifiers (detect presence, not causation)

These fail to distinguish "correlates with output" from "causes output."

### Why Causal Abstraction?

Causal abstraction theory (Geiger et al., 2021) provides a rigorous framework for:

1. **Defining interpretations**: Mapping neural computations to high-level concepts
2. **Testing interpretations**: Using interventions to verify causal structure
3. **Measuring alignment**: Quantifying how well neural mechanisms match intended algorithms
4. **Localizing circuits**: Finding minimal subnetworks that implement behaviors

---

## Decision

We implement **causal abstraction** as the foundation for mechanistic interpretability in Prime-Radiant.

### Core Concepts

#### 1. Causal Models

```rust
/// A causal model with variables and structural equations
pub struct CausalModel {
    /// Variable nodes
    variables: HashMap<VariableId, Variable>,
    /// Directed edges (cause -> effect)
    edges: HashSet<(VariableId, VariableId)>,
    /// Structural equations: V = f(Pa(V), noise)
    equations: HashMap<VariableId, StructuralEquation>,
    /// Exogenous noise distributions
    noise: HashMap<VariableId, NoiseDistribution>,
}

/// A variable in the causal model
pub struct Variable {
    pub id: VariableId,
    pub name: String,
    pub domain: VariableDomain,
    pub level: AbstractionLevel,
}

/// Structural equation defining variable's value
pub enum StructuralEquation {
    /// f(inputs) -> output
    Function(Box<dyn Fn(&[Value]) -> Value>),
    /// Neural network component
    Neural(NeuralComponent),
    /// Identity (exogenous variable)
    Exogenous,
}
```

#### 2. Interventions

```rust
/// An intervention on a causal model
pub enum Intervention {
    /// Set variable to constant value: do(X = x)
    Hard(VariableId, Value),
    /// Modify value by function: do(X = f(X))
    Soft(VariableId, Box<dyn Fn(Value) -> Value>),
    /// Interchange values between runs
    Interchange(VariableId, SourceId),
    /// Activation patching
    Patch(VariableId, Vec<f32>),
}

impl CausalModel {
    /// Apply intervention and compute effects
    pub fn intervene(&self, intervention: &Intervention) -> CausalModel {
        let mut modified = self.clone();
        match intervention {
            Intervention::Hard(var, value) => {
                // Remove all incoming edges
                modified.edges.retain(|(_, target)| target != var);
                // Set constant equation
                modified.equations.insert(*var, StructuralEquation::constant(*value));
            }
            Intervention::Soft(var, f) => {
                // Compose with existing equation
                let old_eq = modified.equations.get(var).unwrap();
                modified.equations.insert(*var, old_eq.compose(f));
            }
            // ...
        }
        modified
    }
}
```

#### 3. Causal Abstraction

```rust
/// A causal abstraction between two models
pub struct CausalAbstraction {
    /// Low-level (concrete) model
    low: CausalModel,
    /// High-level (abstract) model
    high: CausalModel,
    /// Variable mapping: low -> high
    tau: HashMap<VariableId, VariableId>,
    /// Intervention mapping
    intervention_map: Box<dyn Fn(&Intervention) -> Intervention>,
}

impl CausalAbstraction {
    /// Check if abstraction is valid (interventional consistency)
    pub fn is_valid(&self, test_interventions: &[Intervention]) -> bool {
        for intervention in test_interventions {
            // Map intervention to high level
            let high_intervention = (self.intervention_map)(intervention);

            // Intervene on both models
            let low_result = self.low.intervene(intervention);
            let high_result = self.high.intervene(&high_intervention);

            // Check outputs match (up to tau)
            let low_output = low_result.output();
            let high_output = high_result.output();

            if !self.outputs_match(&low_output, &high_output) {
                return false;
            }
        }
        true
    }

    /// Compute interchange intervention accuracy
    pub fn iia(&self,
               base_inputs: &[Input],
               source_inputs: &[Input],
               target_var: VariableId) -> f64 {
        let mut correct = 0;
        let total = base_inputs.len() * source_inputs.len();

        for base in base_inputs {
            for source in source_inputs {
                // Run high-level model with intervention
                let high_base = self.high.run(base);
                let high_source = self.high.run(source);
                let high_interchanged = self.high.intervene(
                    &Intervention::Interchange(target_var, high_source.id)
                ).run(base);

                // Run low-level model with corresponding intervention
                let low_base = self.low.run(base);
                let low_source = self.low.run(source);
                let low_intervention = (self.intervention_map)(
                    &Intervention::Interchange(self.tau[&target_var], low_source.id)
                );
                let low_interchanged = self.low.intervene(&low_intervention).run(base);

                // Check if behaviors match
                if self.outputs_match(&low_interchanged, &high_interchanged) {
                    correct += 1;
                }
            }
        }

        correct as f64 / total as f64
    }
}
```

### Activation Patching

```rust
/// Activation patching for neural network interpretability
pub struct ActivationPatcher {
    /// Target layer/component
    target: NeuralComponent,
    /// Patch source
    source: PatchSource,
}

pub enum PatchSource {
    /// From another input's activation
    OtherInput(InputId),
    /// Fixed vector
    Fixed(Vec<f32>),
    /// Noise ablation
    Noise(NoiseDistribution),
    /// Mean ablation
    Mean,
    /// Zero ablation
    Zero,
}

impl ActivationPatcher {
    /// Measure causal effect of patching
    pub fn causal_effect(
        &self,
        model: &NeuralNetwork,
        base_input: &Input,
        metric: &Metric,
    ) -> f64 {
        // Run without patching
        let base_output = model.forward(base_input);
        let base_metric = metric.compute(&base_output);

        // Run with patching
        let patched_output = model.forward_with_patch(base_input, self);
        let patched_metric = metric.compute(&patched_output);

        // Causal effect is the difference
        patched_metric - base_metric
    }
}
```

### Circuit Discovery

```rust
/// Discover minimal circuits implementing a behavior
pub struct CircuitDiscovery {
    /// Target behavior to explain
    behavior: Behavior,
    /// Candidate components
    components: Vec<NeuralComponent>,
    /// Discovered circuits
    circuits: Vec<Circuit>,
}

pub struct Circuit {
    /// Components in the circuit
    components: Vec<NeuralComponent>,
    /// Edges (data flow)
    edges: Vec<(NeuralComponent, NeuralComponent)>,
    /// Faithfulness score (how well circuit explains behavior)
    faithfulness: f64,
    /// Completeness score (how much of behavior is captured)
    completeness: f64,
}

impl CircuitDiscovery {
    /// Use activation patching to find important components
    pub fn find_circuit(&mut self, model: &NeuralNetwork, inputs: &[Input]) -> Circuit {
        let mut important = Vec::new();

        // Test each component
        for component in &self.components {
            let patcher = ActivationPatcher {
                target: component.clone(),
                source: PatchSource::Zero,
            };

            let avg_effect: f64 = inputs.iter()
                .map(|input| patcher.causal_effect(model, input, &self.behavior.metric))
                .sum::<f64>() / inputs.len() as f64;

            if avg_effect.abs() > IMPORTANCE_THRESHOLD {
                important.push((component.clone(), avg_effect));
            }
        }

        // Build circuit from important components
        self.build_circuit(important)
    }
}
```

---

## Consequences

### Positive

1. **Rigorous causality**: Distinguishes correlation from causation
2. **Multi-level analysis**: Connects low-level activations to high-level concepts
3. **Testable interpretations**: Interventions provide empirical verification
4. **Circuit localization**: Identifies minimal subnetworks for behaviors
5. **Alignment checking**: Verifies mechanisms match specifications

### Negative

1. **Combinatorial explosion**: Testing all interventions is exponential
2. **Approximation required**: Full causal analysis is computationally intractable
3. **Abstraction design**: Choosing the right high-level model requires insight
4. **Noise sensitivity**: Small variations can affect intervention outcomes

### Mitigations

1. **Importance sampling**: Focus on high-impact interventions
2. **Hierarchical search**: Use coarse-to-fine circuit discovery
3. **Learned abstractions**: Train models to find good variable mappings
4. **Robust statistics**: Use multiple samples and statistical tests

---

## Integration with Prime-Radiant

### Connection to Sheaf Cohomology

Causal structure forms a sheaf:
- Open sets: Subnetworks
- Sections: Causal mechanisms
- Restriction maps: Marginalization
- Cohomology: Obstruction to global causal explanation

### Connection to Category Theory

Causal abstraction is a functor:
- Objects: Causal models
- Morphisms: Interventional maps
- Composition: Hierarchical abstraction

---

## References

1. Geiger, A., et al. (2021). "Causal Abstractions of Neural Networks." NeurIPS.

2. Pearl, J. (2009). "Causality: Models, Reasoning, and Inference." Cambridge.

3. Conmy, A., et al. (2023). "Towards Automated Circuit Discovery." NeurIPS.

4. Wang, K., et al. (2022). "Interpretability in the Wild." ICLR.

5. Goldowsky-Dill, N., et al. (2023). "Localizing Model Behavior with Path Patching." arXiv.
