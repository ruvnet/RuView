//! Causal Abstraction Networks for Prime-Radiant
//!
//! This module implements causal reasoning primitives based on structural causal models
//! (SCMs), causal abstraction theory, and do-calculus. Key capabilities:
//!
//! - **CausalModel**: Directed acyclic graph (DAG) of causal relationships with
//!   structural equations defining each variable as a function of its parents
//! - **CausalAbstraction**: Maps between low-level and high-level causal models,
//!   preserving interventional semantics
//! - **CausalCoherenceChecker**: Validates causal consistency of beliefs and detects
//!   spurious correlations
//! - **Counterfactual Reasoning**: Computes counterfactual queries and causal effects
//!
//! ## Architecture
//!
//! The causal module integrates with Prime-Radiant's sheaf-theoretic framework:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                    Prime-Radiant Core                           │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  SheafGraph ◄──── causal_coherence_energy ────► CausalModel    │
//! │      │                                               │          │
//! │      ▼                                               ▼          │
//! │  CoherenceEnergy                            CausalAbstraction   │
//! │      │                                               │          │
//! │      └───────────► Combined Coherence ◄──────────────┘          │
//! └─────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use prime_radiant::causal::{CausalModel, Intervention, counterfactual};
//!
//! // Build a causal model
//! let mut model = CausalModel::new();
//! model.add_variable("X", VariableType::Continuous);
//! model.add_variable("Y", VariableType::Continuous);
//! model.add_structural_equation("Y", &["X"], |parents| {
//!     Value::Continuous(2.0 * parents[0].as_f64() + 0.5)
//! });
//!
//! // Perform intervention do(X = 1.0)
//! let intervention = Intervention::new("X", Value::Continuous(1.0));
//! let result = model.intervene(&intervention);
//!
//! // Compute counterfactual
//! let observation = Observation::new(&[("Y", Value::Continuous(3.0))]);
//! let cf = counterfactual(&model, &observation, &intervention);
//! ```

pub mod model;
pub mod abstraction;
pub mod coherence;
pub mod counterfactual;
pub mod graph;
pub mod do_calculus;

// Re-exports
pub use model::{
    CausalModel, StructuralEquation, Variable, VariableId, VariableType, Value,
    Mechanism, CausalModelError, MutilatedModel, Distribution, Observation,
    IntervenedModel, CausalModelBuilder, Intervention,
};
pub use abstraction::{
    CausalAbstraction, AbstractionMap, AbstractionError, ConsistencyResult,
};
pub use coherence::{
    CausalCoherenceChecker, CausalConsistency, SpuriousCorrelation, Belief,
    CausalQuery, CausalAnswer, CoherenceEnergy,
};
pub use counterfactual::{
    counterfactual, causal_effect,
    CounterfactualQuery, AverageTreatmentEffect,
};
pub use graph::{DirectedGraph, TopologicalOrder, DAGValidationError};
pub use do_calculus::{DoCalculus, Rule, Identification, IdentificationError};

/// Integration with Prime-Radiant's sheaf-theoretic framework
pub mod integration {
    use super::*;

    /// Placeholder for SheafGraph from the main Prime-Radiant module
    pub struct SheafGraph {
        pub nodes: Vec<String>,
        pub edges: Vec<(usize, usize)>,
        pub sections: Vec<Vec<f64>>,
    }

    /// Compute combined coherence energy from structural and causal consistency
    ///
    /// This function bridges Prime-Radiant's sheaf cohomology with causal structure:
    /// - Sheaf consistency measures local-to-global coherence of beliefs
    /// - Causal consistency measures alignment with causal structure
    ///
    /// The combined energy is minimized when both constraints are satisfied.
    pub fn causal_coherence_energy(
        sheaf_graph: &SheafGraph,
        causal_model: &CausalModel,
    ) -> CoherenceEnergy {
        // Compute structural coherence from sheaf
        let structural_energy = compute_structural_energy(sheaf_graph);

        // Compute causal coherence
        let causal_energy = compute_causal_energy(sheaf_graph, causal_model);

        // Compute intervention consistency
        let intervention_energy = compute_intervention_energy(sheaf_graph, causal_model);

        CoherenceEnergy {
            total: structural_energy + causal_energy + intervention_energy,
            structural_component: structural_energy,
            causal_component: causal_energy,
            intervention_component: intervention_energy,
            is_coherent: (structural_energy + causal_energy + intervention_energy) < 1e-6,
        }
    }

    fn compute_structural_energy(sheaf: &SheafGraph) -> f64 {
        // Measure deviation from local consistency
        let mut energy = 0.0;

        for (i, j) in &sheaf.edges {
            if *i < sheaf.sections.len() && *j < sheaf.sections.len() {
                let section_i = &sheaf.sections[*i];
                let section_j = &sheaf.sections[*j];

                // Compute L2 difference (simplified restriction map)
                let min_len = section_i.len().min(section_j.len());
                for k in 0..min_len {
                    let diff = section_i[k] - section_j[k];
                    energy += diff * diff;
                }
            }
        }

        energy
    }

    fn compute_causal_energy(sheaf: &SheafGraph, model: &CausalModel) -> f64 {
        // Check that sheaf structure respects causal ordering
        let mut energy = 0.0;

        if let Ok(topo_order) = model.topological_order() {
            let order_map: std::collections::HashMap<_, _> = topo_order
                .iter()
                .enumerate()
                .map(|(i, v)| (v.clone(), i))
                .collect();

            // Penalize edges that violate causal ordering
            for (i, j) in &sheaf.edges {
                if *i < sheaf.nodes.len() && *j < sheaf.nodes.len() {
                    let node_i = &sheaf.nodes[*i];
                    let node_j = &sheaf.nodes[*j];

                    if let (Some(&order_i), Some(&order_j)) =
                        (order_map.get(node_i), order_map.get(node_j))
                    {
                        // Edge from j to i should have order_j < order_i
                        if order_j > order_i {
                            energy += 1.0;
                        }
                    }
                }
            }
        }

        energy
    }

    fn compute_intervention_energy(sheaf: &SheafGraph, model: &CausalModel) -> f64 {
        // Verify that interventions propagate correctly through sheaf
        let mut energy = 0.0;

        // For each potential intervention point, check consistency
        for (i, node) in sheaf.nodes.iter().enumerate() {
            if let Some(var_id) = model.get_variable_id(node) {
                if let Some(children) = model.children(&var_id) {
                    for child in children {
                        if let Some(child_name) = model.get_variable_name(&child) {
                            // Find corresponding sheaf node
                            if let Some(j) = sheaf.nodes.iter().position(|n| n == &child_name) {
                                // Check if intervention effect is consistent
                                if i < sheaf.sections.len() && j < sheaf.sections.len() {
                                    let parent_section = &sheaf.sections[i];
                                    let child_section = &sheaf.sections[j];

                                    // Simple check: child should be influenced by parent
                                    if !parent_section.is_empty() && !child_section.is_empty() {
                                        // Correlation check (simplified)
                                        let correlation = compute_correlation(parent_section, child_section);
                                        if correlation.abs() < 0.01 {
                                            energy += 0.1; // Weak causal link penalty
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        energy
    }

    fn compute_correlation(a: &[f64], b: &[f64]) -> f64 {
        let n = a.len().min(b.len());
        if n == 0 {
            return 0.0;
        }

        let mean_a: f64 = a.iter().take(n).sum::<f64>() / n as f64;
        let mean_b: f64 = b.iter().take(n).sum::<f64>() / n as f64;

        let mut cov = 0.0;
        let mut var_a = 0.0;
        let mut var_b = 0.0;

        for i in 0..n {
            let da = a[i] - mean_a;
            let db = b[i] - mean_b;
            cov += da * db;
            var_a += da * da;
            var_b += db * db;
        }

        let denom = (var_a * var_b).sqrt();
        if denom < 1e-10 {
            0.0
        } else {
            cov / denom
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::integration::*;

    #[test]
    fn test_module_exports() {
        // Verify all public types are accessible
        let _var_id: VariableId = VariableId(0);
        let _value = Value::Continuous(1.0);
    }

    #[test]
    fn test_causal_coherence_energy() {
        let sheaf = SheafGraph {
            nodes: vec!["X".to_string(), "Y".to_string()],
            edges: vec![(0, 1)],
            sections: vec![vec![1.0, 2.0], vec![2.0, 4.0]],
        };

        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();
        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();
        model.add_edge(x_id, y_id).unwrap();

        let energy = causal_coherence_energy(&sheaf, &model);

        assert!(energy.structural_component >= 0.0);
        assert!(energy.causal_component >= 0.0);
    }
}
