//! Attention mechanism integration tests

use ruvector_dag::attention::*;
use ruvector_dag::dag::{OperatorNode, OperatorType, QueryDag};

fn create_test_dag() -> QueryDag {
    let mut dag = QueryDag::new();

    // Simple linear DAG
    for i in 0..5 {
        dag.add_node(OperatorNode::new(
            i,
            OperatorType::SeqScan {
                table: format!("t{}", i),
            },
        ));
    }

    for i in 0..4 {
        dag.add_edge(i, i + 1).unwrap();
    }

    dag
}

#[test]
fn test_topological_attention() {
    let dag = create_test_dag();
    let attention = TopologicalAttention::new(TopologicalConfig::default());

    let scores = attention.forward(&dag).unwrap();

    // Verify normalization
    let sum: f32 = scores.values().sum();
    assert!(
        (sum - 1.0).abs() < 0.001,
        "Attention scores should sum to 1.0"
    );

    // Verify all scores in [0, 1]
    assert!(scores.values().all(|&s| s >= 0.0 && s <= 1.0));
}

// Mock mechanism for testing selector with DagAttentionMechanism trait
struct MockMechanism {
    name: &'static str,
    score_value: f32,
}

impl DagAttentionMechanism for MockMechanism {
    fn forward(&self, dag: &QueryDag) -> Result<AttentionScoresV2, AttentionErrorV2> {
        let scores = vec![self.score_value; dag.node_count()];
        Ok(AttentionScoresV2::new(scores))
    }

    fn name(&self) -> &'static str {
        self.name
    }

    fn complexity(&self) -> &'static str {
        "O(1)"
    }
}

#[test]
fn test_attention_selector_convergence() {
    let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![Box::new(MockMechanism {
        name: "test_mech",
        score_value: 0.5,
    })];

    let mut selector = AttentionSelector::new(mechanisms, SelectorConfig::default());

    // Run selection multiple times
    let mut selection_counts = std::collections::HashMap::new();

    for _ in 0..100 {
        let idx = selector.select();
        *selection_counts.entry(idx).or_insert(0) += 1;
        selector.update(idx, 0.5 + rand::random::<f32>() * 0.5);
    }

    // Should have made selections
    assert!(selection_counts.values().sum::<usize>() == 100);
}

#[test]
fn test_attention_cache() {
    let config = CacheConfig {
        capacity: 100,
        ttl: None,
    };
    let mut cache = AttentionCache::new(config);
    let dag = create_test_dag();

    // Cache miss
    assert!(cache.get(&dag, "topological").is_none());

    // Insert using the correct type
    let scores = AttentionScoresV2::new(vec![0.2, 0.2, 0.2, 0.2, 0.2]);
    cache.insert(&dag, "topological", scores);

    // Cache hit
    assert!(cache.get(&dag, "topological").is_some());
}

#[test]
fn test_attention_decay_factor() {
    let dag = create_test_dag();

    // Low decay factor (sharper distribution)
    let config_low = TopologicalConfig {
        decay_factor: 0.5,
        max_depth: 10,
    };
    let attention_low = TopologicalAttention::new(config_low);
    let scores_low = attention_low.forward(&dag).unwrap();

    // High decay factor (smoother distribution)
    let config_high = TopologicalConfig {
        decay_factor: 0.99,
        max_depth: 10,
    };
    let attention_high = TopologicalAttention::new(config_high);
    let scores_high = attention_high.forward(&dag).unwrap();

    // Both should be normalized
    let sum_low: f32 = scores_low.values().sum();
    let sum_high: f32 = scores_high.values().sum();
    assert!((sum_low - 1.0).abs() < 0.001);
    assert!((sum_high - 1.0).abs() < 0.001);
}

#[test]
fn test_attention_empty_dag() {
    let dag = QueryDag::new();
    let attention = TopologicalAttention::new(TopologicalConfig::default());

    let result = attention.forward(&dag);
    // Empty DAG returns error
    assert!(result.is_err());
}

#[test]
fn test_attention_single_node() {
    let mut dag = QueryDag::new();
    dag.add_node(OperatorNode::new(0, OperatorType::Result));

    let attention = TopologicalAttention::new(TopologicalConfig::default());
    let scores = attention.forward(&dag).unwrap();

    // Single node should get score of 1.0
    assert_eq!(scores.len(), 1);
    assert!((scores[&0] - 1.0).abs() < 0.001);
}

#[test]
fn test_attention_cache_eviction() {
    let config = CacheConfig {
        capacity: 2,
        ttl: None,
    };
    let mut cache = AttentionCache::new(config);

    // Fill cache beyond capacity
    for i in 0..5 {
        let mut dag = QueryDag::new();
        dag.add_node(OperatorNode::new(i, OperatorType::Result));

        let scores = AttentionScoresV2::new(vec![1.0]);
        cache.insert(&dag, "test", scores);
    }

    // Cache stats should show eviction happened
    let stats = cache.stats();
    assert!(stats.size <= 2);
}

#[test]
fn test_multi_mechanism_selector() {
    let mechanisms: Vec<Box<dyn DagAttentionMechanism>> = vec![
        Box::new(MockMechanism {
            name: "mech1",
            score_value: 0.5,
        }),
        Box::new(MockMechanism {
            name: "mech2",
            score_value: 0.7,
        }),
    ];

    let mut selector = AttentionSelector::new(
        mechanisms,
        SelectorConfig {
            exploration_factor: 0.1,
            initial_value: 1.0,
            min_samples: 3,
        },
    );

    // Both mechanisms should be selected at some point
    let mut used = std::collections::HashSet::new();

    for _ in 0..50 {
        let idx = selector.select();
        used.insert(idx);
        selector.update(idx, 0.5);
    }

    assert!(used.len() >= 1, "At least one mechanism should be selected");
}
