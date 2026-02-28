//! Integration tests for MinCut optimization

use ruvector_dag::*;

#[test]
fn test_mincut_engine_basic() {
    let mut dag = QueryDag::new();

    // Create a simple query plan: SeqScan -> Filter -> Sort
    let scan = dag.add_node(OperatorNode::seq_scan(0, "users").with_estimates(1000.0, 100.0));
    let filter = dag.add_node(OperatorNode::filter(0, "age > 18").with_estimates(500.0, 50.0));
    let sort =
        dag.add_node(OperatorNode::sort(0, vec!["name".to_string()]).with_estimates(500.0, 150.0));

    dag.add_edge(scan, filter).unwrap();
    dag.add_edge(filter, sort).unwrap();

    // Build min-cut engine
    let mut engine = DagMinCutEngine::new(MinCutConfig::default());
    engine.build_from_dag(&dag);

    assert_eq!(dag.node_count(), 3);
}

#[test]
fn test_bottleneck_analysis() {
    let mut dag = QueryDag::new();

    // Create a query plan with a potential bottleneck
    let scan = dag.add_node(
        OperatorNode::seq_scan(0, "users").with_estimates(10000.0, 1000.0), // High cost
    );
    let filter = dag.add_node(
        OperatorNode::filter(0, "active = true").with_estimates(5000.0, 10.0), // Low cost
    );

    dag.add_edge(scan, filter).unwrap();

    // Compute criticality
    let mut engine = DagMinCutEngine::new(MinCutConfig::default());
    engine.build_from_dag(&dag);
    let criticality = engine.compute_criticality(&dag);

    // Analyze bottlenecks
    let analysis = BottleneckAnalysis::analyze(&dag, &criticality);

    assert!(analysis.total_cost > 0.0);
    assert!(analysis.critical_path_cost > 0.0);
}

#[test]
fn test_redundancy_suggestions() {
    let mut dag = QueryDag::new();

    let scan = dag
        .add_node(OperatorNode::hnsw_scan(0, "embeddings_idx", 100).with_estimates(1000.0, 200.0));

    // Create a high-criticality bottleneck
    let bottleneck = Bottleneck {
        node_id: scan,
        score: 0.8,
        impact_estimate: 160.0,
        suggested_action: "Test".to_string(),
    };

    let suggestions = RedundancySuggestion::generate(&dag, &[bottleneck]);

    assert_eq!(suggestions.len(), 1);
    assert!(matches!(
        suggestions[0].strategy,
        RedundancyStrategy::Prefetch
    ));
}

#[test]
fn test_local_kcut_computation() {
    let mut dag = QueryDag::new();

    // Create a simple chain
    let n0 = dag.add_node(OperatorNode::seq_scan(0, "t0").with_estimates(100.0, 10.0));
    let n1 = dag.add_node(OperatorNode::filter(0, "f1").with_estimates(50.0, 20.0));
    let n2 = dag.add_node(OperatorNode::sort(0, vec!["c1".to_string()]).with_estimates(50.0, 30.0));

    dag.add_edge(n0, n1).unwrap();
    dag.add_edge(n1, n2).unwrap();

    let mut engine = DagMinCutEngine::new(MinCutConfig {
        epsilon: 0.1,
        local_search_depth: 5,
        cache_cuts: true,
    });

    engine.build_from_dag(&dag);
    let result = engine.compute_mincut(n0, n2);

    // Should find some cut
    assert!(result.cut_value >= 0.0);
}

#[test]
fn test_dynamic_edge_update() {
    let mut dag = QueryDag::new();

    let n0 = dag.add_node(OperatorNode::seq_scan(0, "t0").with_estimates(100.0, 10.0));
    let n1 = dag.add_node(OperatorNode::filter(0, "f1").with_estimates(50.0, 20.0));
    let n2 = dag.add_node(OperatorNode::sort(0, vec!["c1".to_string()]).with_estimates(50.0, 30.0));

    dag.add_edge(n0, n1).unwrap();
    dag.add_edge(n1, n2).unwrap();

    let mut engine = DagMinCutEngine::new(MinCutConfig::default());
    engine.build_from_dag(&dag);

    // Test dynamic update - O(n^0.12) amortized
    engine.update_edge(n0, n1, 15.0);

    // Cache should be invalidated
    let result = engine.compute_mincut(n0, n2);
    assert!(result.cut_value >= 0.0);
}

#[test]
fn test_mincut_config() {
    let config = MinCutConfig {
        epsilon: 0.05,
        local_search_depth: 10,
        cache_cuts: false,
    };

    assert_eq!(config.epsilon, 0.05);
    assert_eq!(config.local_search_depth, 10);
    assert!(!config.cache_cuts);
}
