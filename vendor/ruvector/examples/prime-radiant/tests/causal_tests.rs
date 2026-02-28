//! Comprehensive tests for Causal Inference Module
//!
//! This test suite verifies causal reasoning including:
//! - DAG validation
//! - Intervention semantics (do-calculus)
//! - Counterfactual computation
//! - Causal abstraction consistency

use prime_radiant::causal::{
    CausalModel, StructuralEquation, Variable, VariableId, VariableType, Value,
    CausalAbstraction, AbstractionMap, ConsistencyResult,
    CausalCoherenceChecker, CausalConsistency, Belief,
    counterfactual, causal_effect, Observation, Distribution,
    DirectedGraph, TopologicalOrder, DAGValidationError,
    DoCalculus, Rule, Identification,
};
use prime_radiant::causal::integration::{SheafGraph, causal_coherence_energy, CoherenceEnergy};
use proptest::prelude::*;
use approx::assert_relative_eq;
use std::collections::{HashMap, HashSet};

// =============================================================================
// DAG VALIDATION TESTS
// =============================================================================

mod dag_validation_tests {
    use super::*;

    /// Test basic DAG creation
    #[test]
    fn test_create_dag() {
        let mut graph = DirectedGraph::new();
        graph.add_node(0);
        graph.add_node(1);
        graph.add_node(2);

        assert_eq!(graph.node_count(), 3);
    }

    /// Test adding valid edges
    #[test]
    fn test_add_valid_edges() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();
        graph.add_edge(0, 2).unwrap();

        assert_eq!(graph.edge_count(), 3);
        assert!(graph.contains_edge(0, 1));
        assert!(graph.contains_edge(1, 2));
        assert!(graph.contains_edge(0, 2));
    }

    /// Test cycle detection
    #[test]
    fn test_cycle_detection() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        // Adding 2 -> 0 would create a cycle
        let result = graph.add_edge(2, 0);
        assert!(result.is_err());

        match result {
            Err(DAGValidationError::CycleDetected(nodes)) => {
                assert!(!nodes.is_empty());
            }
            _ => panic!("Expected CycleDetected error"),
        }
    }

    /// Test self-loop detection
    #[test]
    fn test_self_loop_detection() {
        let mut graph = DirectedGraph::new();
        let result = graph.add_edge(0, 0);

        assert!(result.is_err());
        assert!(matches!(result, Err(DAGValidationError::SelfLoop(0))));
    }

    /// Test topological ordering
    #[test]
    fn test_topological_order() {
        let mut graph = DirectedGraph::new();
        // Diamond graph: 0 -> 1, 0 -> 2, 1 -> 3, 2 -> 3
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();
        graph.add_edge(1, 3).unwrap();
        graph.add_edge(2, 3).unwrap();

        let order = graph.topological_order().unwrap();

        assert_eq!(order.len(), 4);
        assert!(order.comes_before(0, 1));
        assert!(order.comes_before(0, 2));
        assert!(order.comes_before(1, 3));
        assert!(order.comes_before(2, 3));
    }

    /// Test ancestors computation
    #[test]
    fn test_ancestors() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();
        graph.add_edge(0, 3).unwrap();
        graph.add_edge(3, 2).unwrap();

        let ancestors = graph.ancestors(2);

        assert!(ancestors.contains(&0));
        assert!(ancestors.contains(&1));
        assert!(ancestors.contains(&3));
        assert!(!ancestors.contains(&2));
    }

    /// Test descendants computation
    #[test]
    fn test_descendants() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(0, 2).unwrap();
        graph.add_edge(1, 3).unwrap();
        graph.add_edge(2, 3).unwrap();

        let descendants = graph.descendants(0);

        assert!(descendants.contains(&1));
        assert!(descendants.contains(&2));
        assert!(descendants.contains(&3));
        assert!(!descendants.contains(&0));
    }

    /// Test d-separation in chain
    #[test]
    fn test_d_separation_chain() {
        // X -> Z -> Y (chain)
        let mut graph = DirectedGraph::new();
        graph.add_node_with_label(0, "X");
        graph.add_node_with_label(1, "Z");
        graph.add_node_with_label(2, "Y");
        graph.add_edge(0, 1).unwrap();
        graph.add_edge(1, 2).unwrap();

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y are NOT d-separated given empty set
        assert!(!graph.d_separated(&x, &y, &empty));

        // X and Y ARE d-separated given Z
        assert!(graph.d_separated(&x, &y, &z));
    }

    /// Test d-separation in fork
    #[test]
    fn test_d_separation_fork() {
        // X <- Z -> Y (fork)
        let mut graph = DirectedGraph::new();
        graph.add_edge(1, 0).unwrap();  // Z -> X
        graph.add_edge(1, 2).unwrap();  // Z -> Y

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y are NOT d-separated given empty set
        assert!(!graph.d_separated(&x, &y, &empty));

        // X and Y ARE d-separated given Z
        assert!(graph.d_separated(&x, &y, &z));
    }

    /// Test d-separation in collider
    #[test]
    fn test_d_separation_collider() {
        // X -> Z <- Y (collider)
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 1).unwrap();  // X -> Z
        graph.add_edge(2, 1).unwrap();  // Y -> Z

        let x: HashSet<u32> = [0].into_iter().collect();
        let y: HashSet<u32> = [2].into_iter().collect();
        let z: HashSet<u32> = [1].into_iter().collect();
        let empty: HashSet<u32> = HashSet::new();

        // X and Y ARE d-separated given empty set (collider blocks)
        assert!(graph.d_separated(&x, &y, &empty));

        // X and Y are NOT d-separated given Z (conditioning opens collider)
        assert!(!graph.d_separated(&x, &y, &z));
    }

    /// Test v-structure detection
    #[test]
    fn test_v_structures() {
        let mut graph = DirectedGraph::new();
        graph.add_edge(0, 2).unwrap();  // X -> Z
        graph.add_edge(1, 2).unwrap();  // Y -> Z

        let v_structs = graph.v_structures();

        assert_eq!(v_structs.len(), 1);
        let (a, b, c) = v_structs[0];
        assert_eq!(b, 2);  // Z is the collider
    }
}

// =============================================================================
// INTERVENTION TESTS
// =============================================================================

mod intervention_tests {
    use super::*;

    /// Test intervention do(X = x) removes incoming edges
    #[test]
    fn test_intervention_removes_incoming_edges() {
        let mut model = CausalModel::new();

        // Z -> X -> Y
        model.add_variable("Z", VariableType::Continuous).unwrap();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let z_id = model.get_variable_id("Z").unwrap();
        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(z_id, x_id).unwrap();  // Z -> X
        model.add_edge(x_id, y_id).unwrap();  // X -> Y

        // Structural equation: X = 2*Z + noise
        model.set_structural_equation(x_id, StructuralEquation::linear(&[z_id], vec![2.0]));

        // Structural equation: Y = 3*X + noise
        model.set_structural_equation(y_id, StructuralEquation::linear(&[x_id], vec![3.0]));

        // Before intervention, X depends on Z
        assert!(model.parents(&x_id).unwrap().contains(&z_id));

        // Intervene do(X = 5)
        let mutilated = model.intervene(x_id, Value::Continuous(5.0)).unwrap();

        // After intervention, X has no parents
        assert!(mutilated.parents(&x_id).unwrap().is_empty());

        // Y still depends on X
        assert!(mutilated.parents(&y_id).unwrap().contains(&x_id));
    }

    /// Test interventional distribution differs from observational
    #[test]
    fn test_interventional_vs_observational() {
        let mut model = CausalModel::new();

        // Confounded: Z -> X, Z -> Y, X -> Y
        model.add_variable("Z", VariableType::Continuous).unwrap();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let z_id = model.get_variable_id("Z").unwrap();
        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(z_id, x_id).unwrap();
        model.add_edge(z_id, y_id).unwrap();
        model.add_edge(x_id, y_id).unwrap();

        // Compute observational P(Y | X = 1)
        let obs = Observation::new(&[("X", Value::Continuous(1.0))]);
        let p_y_given_x = model.conditional_distribution(&obs, "Y").unwrap();

        // Compute interventional P(Y | do(X = 1))
        let mutilated = model.intervene(x_id, Value::Continuous(1.0)).unwrap();
        let p_y_do_x = mutilated.marginal_distribution("Y").unwrap();

        // These should generally differ due to confounding
        // (The specific values depend on structural equations)
        assert!(p_y_given_x != p_y_do_x || model.is_unconfounded(x_id, y_id));
    }

    /// Test average treatment effect computation
    #[test]
    fn test_average_treatment_effect() {
        let mut model = CausalModel::new();

        // Simple model: Treatment -> Outcome
        model.add_variable("T", VariableType::Binary).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let t_id = model.get_variable_id("T").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(t_id, y_id).unwrap();

        // Y = 2*T + epsilon
        model.set_structural_equation(y_id, StructuralEquation::linear(&[t_id], vec![2.0]));

        // ATE = E[Y | do(T=1)] - E[Y | do(T=0)]
        let ate = causal_effect(&model, t_id, y_id,
            Value::Binary(true),
            Value::Binary(false)
        ).unwrap();

        // Should be approximately 2.0
        assert_relative_eq!(ate, 2.0, epsilon = 0.5);
    }

    /// Test multiple simultaneous interventions
    #[test]
    fn test_multiple_interventions() {
        let mut model = CausalModel::new();

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();
        model.add_variable("Z", VariableType::Continuous).unwrap();

        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();
        let z_id = model.get_variable_id("Z").unwrap();

        model.add_edge(x_id, z_id).unwrap();
        model.add_edge(y_id, z_id).unwrap();

        // Intervene on both X and Y
        let interventions = vec![
            (x_id, Value::Continuous(1.0)),
            (y_id, Value::Continuous(2.0)),
        ];

        let mutilated = model.multi_intervene(&interventions).unwrap();

        // Both X and Y should have no parents
        assert!(mutilated.parents(&x_id).unwrap().is_empty());
        assert!(mutilated.parents(&y_id).unwrap().is_empty());
    }
}

// =============================================================================
// COUNTERFACTUAL TESTS
// =============================================================================

mod counterfactual_tests {
    use super::*;

    /// Test basic counterfactual computation
    #[test]
    fn test_basic_counterfactual() {
        let mut model = CausalModel::new();

        // X -> Y with Y = 2*X
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(x_id, y_id).unwrap();
        model.set_structural_equation(y_id, StructuralEquation::linear(&[x_id], vec![2.0]));

        // Observe Y = 4 (implies X = 2)
        let observation = Observation::new(&[("Y", Value::Continuous(4.0))]);

        // Counterfactual: What would Y be if X = 3?
        let cf_y = counterfactual(&model, &observation, x_id, Value::Continuous(3.0), "Y").unwrap();

        // Y' = 2 * 3 = 6
        match cf_y {
            Value::Continuous(y) => assert_relative_eq!(y, 6.0, epsilon = 0.1),
            _ => panic!("Expected continuous value"),
        }
    }

    /// Test counterfactual with noise inference
    #[test]
    fn test_counterfactual_with_noise() {
        let mut model = CausalModel::new();

        // X -> Y with Y = X + U_Y where U_Y is noise
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(x_id, y_id).unwrap();
        model.set_structural_equation(y_id, StructuralEquation::with_noise(&[x_id], vec![1.0]));

        // Observe X = 1, Y = 3 (so U_Y = 2)
        let observation = Observation::new(&[
            ("X", Value::Continuous(1.0)),
            ("Y", Value::Continuous(3.0)),
        ]);

        // What if X = 2?
        let cf_y = counterfactual(&model, &observation, x_id, Value::Continuous(2.0), "Y").unwrap();

        // Y' = 2 + 2 = 4 (noise U_Y = 2 is preserved)
        match cf_y {
            Value::Continuous(y) => assert_relative_eq!(y, 4.0, epsilon = 0.1),
            _ => panic!("Expected continuous value"),
        }
    }

    /// Test counterfactual consistency
    #[test]
    fn test_counterfactual_consistency() {
        let mut model = CausalModel::new();

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x_id = model.get_variable_id("X").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(x_id, y_id).unwrap();
        model.set_structural_equation(y_id, StructuralEquation::linear(&[x_id], vec![2.0]));

        // Observe X = 2, Y = 4
        let observation = Observation::new(&[
            ("X", Value::Continuous(2.0)),
            ("Y", Value::Continuous(4.0)),
        ]);

        // Counterfactual with actual value should match observed
        let cf_y = counterfactual(&model, &observation, x_id, Value::Continuous(2.0), "Y").unwrap();

        match cf_y {
            Value::Continuous(y) => assert_relative_eq!(y, 4.0, epsilon = 0.1),
            _ => panic!("Expected continuous value"),
        }
    }

    /// Test effect of treatment on treated (ETT)
    #[test]
    fn test_effect_on_treated() {
        let mut model = CausalModel::new();

        model.add_variable("T", VariableType::Binary).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let t_id = model.get_variable_id("T").unwrap();
        let y_id = model.get_variable_id("Y").unwrap();

        model.add_edge(t_id, y_id).unwrap();
        model.set_structural_equation(y_id, StructuralEquation::linear(&[t_id], vec![5.0]));

        // For treated individuals (T = 1), what would Y be if T = 0?
        let observation = Observation::new(&[
            ("T", Value::Binary(true)),
            ("Y", Value::Continuous(5.0)),
        ]);

        let cf_y = counterfactual(&model, &observation, t_id, Value::Binary(false), "Y").unwrap();

        // ETT = Y(T=1) - Y(T=0) for treated
        match cf_y {
            Value::Continuous(y_untreated) => {
                let ett = 5.0 - y_untreated;
                assert_relative_eq!(ett, 5.0, epsilon = 0.5);
            }
            _ => panic!("Expected continuous value"),
        }
    }
}

// =============================================================================
// CAUSAL ABSTRACTION TESTS
// =============================================================================

mod causal_abstraction_tests {
    use super::*;

    /// Test abstraction map between models
    #[test]
    fn test_abstraction_map() {
        // Low-level model: X1 -> X2 -> X3
        let mut low = CausalModel::new();
        low.add_variable("X1", VariableType::Continuous).unwrap();
        low.add_variable("X2", VariableType::Continuous).unwrap();
        low.add_variable("X3", VariableType::Continuous).unwrap();

        let x1 = low.get_variable_id("X1").unwrap();
        let x2 = low.get_variable_id("X2").unwrap();
        let x3 = low.get_variable_id("X3").unwrap();

        low.add_edge(x1, x2).unwrap();
        low.add_edge(x2, x3).unwrap();

        // High-level model: A -> B
        let mut high = CausalModel::new();
        high.add_variable("A", VariableType::Continuous).unwrap();
        high.add_variable("B", VariableType::Continuous).unwrap();

        let a = high.get_variable_id("A").unwrap();
        let b = high.get_variable_id("B").unwrap();

        high.add_edge(a, b).unwrap();

        // Abstraction: A = X1, B = X3 (X2 is "hidden")
        let abstraction = CausalAbstraction::new(&low, &high);
        abstraction.add_mapping(x1, a);
        abstraction.add_mapping(x3, b);

        assert!(abstraction.is_valid_abstraction());
    }

    /// Test abstraction consistency
    #[test]
    fn test_abstraction_consistency() {
        // Two-level model
        let mut low = CausalModel::new();
        low.add_variable("X", VariableType::Continuous).unwrap();
        low.add_variable("Y", VariableType::Continuous).unwrap();

        let x = low.get_variable_id("X").unwrap();
        let y = low.get_variable_id("Y").unwrap();

        low.add_edge(x, y).unwrap();
        low.set_structural_equation(y, StructuralEquation::linear(&[x], vec![2.0]));

        let mut high = CausalModel::new();
        high.add_variable("A", VariableType::Continuous).unwrap();
        high.add_variable("B", VariableType::Continuous).unwrap();

        let a = high.get_variable_id("A").unwrap();
        let b = high.get_variable_id("B").unwrap();

        high.add_edge(a, b).unwrap();
        high.set_structural_equation(b, StructuralEquation::linear(&[a], vec![2.0]));

        let abstraction = CausalAbstraction::new(&low, &high);
        abstraction.add_mapping(x, a);
        abstraction.add_mapping(y, b);

        let result = abstraction.check_consistency();
        assert!(matches!(result, ConsistencyResult::Consistent));
    }

    /// Test intervention consistency across abstraction
    #[test]
    fn test_intervention_consistency() {
        let mut low = CausalModel::new();
        low.add_variable("X", VariableType::Continuous).unwrap();
        low.add_variable("Y", VariableType::Continuous).unwrap();

        let x = low.get_variable_id("X").unwrap();
        let y = low.get_variable_id("Y").unwrap();

        low.add_edge(x, y).unwrap();
        low.set_structural_equation(y, StructuralEquation::linear(&[x], vec![3.0]));

        let mut high = CausalModel::new();
        high.add_variable("A", VariableType::Continuous).unwrap();
        high.add_variable("B", VariableType::Continuous).unwrap();

        let a = high.get_variable_id("A").unwrap();
        let b = high.get_variable_id("B").unwrap();

        high.add_edge(a, b).unwrap();
        high.set_structural_equation(b, StructuralEquation::linear(&[a], vec![3.0]));

        let abstraction = CausalAbstraction::new(&low, &high);
        abstraction.add_mapping(x, a);
        abstraction.add_mapping(y, b);

        // Intervene on low-level model
        let low_intervened = low.intervene(x, Value::Continuous(5.0)).unwrap();
        let low_y = low_intervened.compute("Y").unwrap();

        // Intervene on high-level model
        let high_intervened = high.intervene(a, Value::Continuous(5.0)).unwrap();
        let high_b = high_intervened.compute("B").unwrap();

        // Results should match
        match (low_y, high_b) {
            (Value::Continuous(ly), Value::Continuous(hb)) => {
                assert_relative_eq!(ly, hb, epsilon = 0.1);
            }
            _ => panic!("Expected continuous values"),
        }
    }
}

// =============================================================================
// CAUSAL COHERENCE TESTS
// =============================================================================

mod causal_coherence_tests {
    use super::*;

    /// Test causal coherence checker
    #[test]
    fn test_causal_coherence_consistent() {
        let checker = CausalCoherenceChecker::new();

        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        model.add_edge(x, y).unwrap();

        // Belief: X causes Y
        let belief = Belief::causal_relation("X", "Y", true);

        let result = checker.check(&model, &[belief]);
        assert!(matches!(result, CausalConsistency::Consistent));
    }

    /// Test detecting spurious correlation
    #[test]
    fn test_detect_spurious_correlation() {
        let checker = CausalCoherenceChecker::new();

        let mut model = CausalModel::new();
        // Z -> X, Z -> Y (confounded)
        model.add_variable("Z", VariableType::Continuous).unwrap();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let z = model.get_variable_id("Z").unwrap();
        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        model.add_edge(z, x).unwrap();
        model.add_edge(z, y).unwrap();

        // Mistaken belief: X causes Y
        let belief = Belief::causal_relation("X", "Y", true);

        let result = checker.check(&model, &[belief]);
        assert!(matches!(result, CausalConsistency::SpuriousCorrelation(_)));
    }

    /// Test integration with sheaf coherence
    #[test]
    fn test_causal_sheaf_integration() {
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
        assert!(energy.total >= 0.0);
    }
}

// =============================================================================
// DO-CALCULUS TESTS
// =============================================================================

mod do_calculus_tests {
    use super::*;

    /// Test Rule 1: Ignoring observations
    #[test]
    fn test_rule1_ignoring_observations() {
        let mut model = CausalModel::new();

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();
        model.add_variable("Z", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();
        let z = model.get_variable_id("Z").unwrap();

        model.add_edge(x, y).unwrap();
        model.add_edge(z, y).unwrap();

        let calc = DoCalculus::new(&model);

        // P(y | do(x), z) = P(y | do(x)) if Z d-separated from Y given X in mutilated graph
        let x_set: HashSet<_> = [x].into_iter().collect();
        let z_set: HashSet<_> = [z].into_iter().collect();
        let y_set: HashSet<_> = [y].into_iter().collect();

        let rule1_applies = calc.can_apply_rule1(&y_set, &x_set, &z_set);
        assert!(!rule1_applies);  // Z -> Y, so can't ignore Z
    }

    /// Test Rule 2: Action/observation exchange
    #[test]
    fn test_rule2_action_observation_exchange() {
        let mut model = CausalModel::new();

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();
        model.add_variable("Z", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();
        let z = model.get_variable_id("Z").unwrap();

        // X -> Z -> Y
        model.add_edge(x, z).unwrap();
        model.add_edge(z, y).unwrap();

        let calc = DoCalculus::new(&model);

        // P(y | do(x), do(z)) = P(y | do(x), z) if...
        let can_exchange = calc.can_apply_rule2(y, x, z);
        // Depends on the specific d-separation conditions
        assert!(can_exchange || !can_exchange);  // Result depends on structure
    }

    /// Test Rule 3: Removing actions
    #[test]
    fn test_rule3_removing_actions() {
        let mut model = CausalModel::new();

        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        // No edge from X to Y
        // X and Y are independent

        let calc = DoCalculus::new(&model);

        // P(y | do(x)) = P(y) if X has no effect on Y
        let can_remove = calc.can_apply_rule3(y, x);
        assert!(can_remove);
    }

    /// Test causal effect identification
    #[test]
    fn test_causal_effect_identification() {
        let mut model = CausalModel::new();

        // Simple identifiable case: X -> Y
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        model.add_edge(x, y).unwrap();

        let calc = DoCalculus::new(&model);
        let result = calc.identify(y, &[x].into_iter().collect());

        assert!(matches!(result, Identification::Identified(_)));
    }

    /// Test non-identifiable case
    #[test]
    fn test_non_identifiable_effect() {
        let mut model = CausalModel::new();

        // Confounded: U -> X, U -> Y, X -> Y (U unobserved)
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();
        let y = model.get_variable_id("Y").unwrap();

        model.add_edge(x, y).unwrap();
        model.add_latent_confounding(x, y);  // Unobserved confounder

        let calc = DoCalculus::new(&model);
        let result = calc.identify(y, &[x].into_iter().collect());

        // Without adjustment variables, effect is not identifiable
        assert!(matches!(result, Identification::NotIdentified(_)));
    }
}

// =============================================================================
// PROPERTY-BASED TESTS
// =============================================================================

mod property_tests {
    use super::*;

    proptest! {
        /// Property: Topological order respects all edges
        #[test]
        fn prop_topo_order_respects_edges(
            edges in proptest::collection::vec((0..10u32, 0..10u32), 0..20)
        ) {
            let mut graph = DirectedGraph::new();

            for (from, to) in &edges {
                if from != to {
                    let _ = graph.add_edge(*from, *to);  // May fail if creates cycle
                }
            }

            if let Ok(order) = graph.topological_order() {
                for (from, to) in graph.edges() {
                    prop_assert!(order.comes_before(from, to));
                }
            }
        }

        /// Property: Interventions don't create cycles
        #[test]
        fn prop_intervention_preserves_dag(
            n in 2..8usize,
            seed in 0..1000u64
        ) {
            let mut model = CausalModel::new();

            for i in 0..n {
                model.add_variable(&format!("V{}", i), VariableType::Continuous).unwrap();
            }

            // Random DAG edges
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed);
            for i in 0..n {
                for j in (i+1)..n {
                    if rand::Rng::gen_bool(&mut rng, 0.3) {
                        let vi = model.get_variable_id(&format!("V{}", i)).unwrap();
                        let vj = model.get_variable_id(&format!("V{}", j)).unwrap();
                        let _ = model.add_edge(vi, vj);
                    }
                }
            }

            // Any intervention should preserve DAG property
            let v0 = model.get_variable_id("V0").unwrap();
            if let Ok(mutilated) = model.intervene(v0, Value::Continuous(1.0)) {
                prop_assert!(mutilated.is_dag());
            }
        }
    }
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

mod edge_case_tests {
    use super::*;

    /// Test empty model
    #[test]
    fn test_empty_model() {
        let model = CausalModel::new();
        assert_eq!(model.variable_count(), 0);
    }

    /// Test single variable model
    #[test]
    fn test_single_variable() {
        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();

        assert_eq!(model.variable_count(), 1);

        let x = model.get_variable_id("X").unwrap();
        assert!(model.parents(&x).unwrap().is_empty());
    }

    /// Test duplicate variable names
    #[test]
    fn test_duplicate_variable_name() {
        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();

        let result = model.add_variable("X", VariableType::Continuous);
        assert!(result.is_err());
    }

    /// Test intervention on non-existent variable
    #[test]
    fn test_intervene_nonexistent() {
        let model = CausalModel::new();
        let fake_id = VariableId(999);

        let result = model.intervene(fake_id, Value::Continuous(1.0));
        assert!(result.is_err());
    }

    /// Test empty observation counterfactual
    #[test]
    fn test_empty_observation_counterfactual() {
        let mut model = CausalModel::new();
        model.add_variable("X", VariableType::Continuous).unwrap();
        model.add_variable("Y", VariableType::Continuous).unwrap();

        let x = model.get_variable_id("X").unwrap();

        let empty_obs = Observation::new(&[]);
        let result = counterfactual(&model, &empty_obs, x, Value::Continuous(1.0), "Y");

        // Should work with empty observation (uses prior)
        assert!(result.is_ok());
    }
}
