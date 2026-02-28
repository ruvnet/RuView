# RuvLLM: TDD and Iterative Refinement

## SPARC Phase 4: Refinement

---

## 1. Core Philosophy: Three-Layer Self-Learning

### 1.1 The Mental Model

> **"The intelligence is not in one model anymore. It is in the loop."**

RuvLLM treats:
- **LFM2 weights** as a **stable cortex** (fixed core reasoning engine)
- **Ruvector** as the **living synaptic mesh** (adapts continuously)
- **FastGRNN** as the **control circuit** (learns when to use what)

This creates a system that genuinely learns from experience without requiring constant model retraining.

### 1.2 Three Adaptation Timescales

| Timescale | Mechanism | What Changes | Frequency |
|-----------|-----------|--------------|-----------|
| **Short-term** | Memory + Routing | Graph structure, attention patterns, routing decisions | Every request |
| **Medium-term** | Compression | Concept nodes, graph hierarchy, router weights | Hourly/Daily |
| **Long-term** | Weight tuning | LFM2 fine-tuned variants | Weekly/Monthly |

---

## 2. Self-Learning Loop Architecture

### 2.1 Loop A: Memory Growth and Refinement

**What happens on every request:**

```
Request → Response → Outcome
     ↓
┌────────────────────────────────────────────────────────────────┐
│                    Memory Growth Loop                          │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. WRITE to ruvector:                                          │
│     ┌─────────────────────────────────────────────────────────┐│
│     │  - Question (query embedding + text)                     ││
│     │  - Answer (response embedding + text)                    ││
│     │  - Retrieved documents (context used)                    ││
│     │  - Final outcome (quality score, task success)           ││
│     │  - User feedback if any (explicit signals)               ││
│     └─────────────────────────────────────────────────────────┘│
│                                                                  │
│  2. GRAPH RULES:                                                │
│     ┌─────────────────────────────────────────────────────────┐│
│     │  ✓ Strengthen edges between nodes that co-appear        ││
│     │    in good answers                                       ││
│     │  ✓ Weaken/prune edges rarely used or correlating        ││
│     │    with bad answers                                      ││
│     │  ✓ Update attention weights based on success patterns   ││
│     └─────────────────────────────────────────────────────────┘│
│                                                                  │
│  3. RESULT:                                                     │
│     Same LFM2 checkpoint → Different answers over time         │
│     because the graph, weights, and attention improve          │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

**TDD Tests for Loop A:**

```rust
#[cfg(test)]
mod memory_growth_tests {
    use super::*;

    #[test]
    fn test_successful_interaction_strengthens_edges() {
        // Given: A memory with two related nodes
        let mut memory = RuvectorMemory::new_test();
        let node_a = memory.insert_node("Machine learning is a subset of AI");
        let node_b = memory.insert_node("Neural networks are ML models");
        memory.insert_edge(node_a, node_b, EdgeType::SameTopic, 0.5);

        // When: A successful query uses both nodes
        let outcome = InteractionOutcome {
            quality_score: 0.9,
            used_nodes: vec![node_a.clone(), node_b.clone()],
            task_success: true,
        };
        memory.apply_outcome(&outcome);

        // Then: Edge weight should increase
        let edge = memory.get_edge(&node_a, &node_b).unwrap();
        assert!(edge.weight > 0.5);
    }

    #[test]
    fn test_failed_interaction_weakens_edges() {
        // Given: A memory with edge
        let mut memory = RuvectorMemory::new_test();
        let node_a = memory.insert_node("Topic A");
        let node_b = memory.insert_node("Unrelated B");
        memory.insert_edge(node_a, node_b, EdgeType::SameTopic, 0.5);

        // When: Query uses these but fails
        let outcome = InteractionOutcome {
            quality_score: 0.3,
            used_nodes: vec![node_a.clone(), node_b.clone()],
            task_success: false,
        };
        memory.apply_outcome(&outcome);

        // Then: Edge weight should decrease
        let edge = memory.get_edge(&node_a, &node_b).unwrap();
        assert!(edge.weight < 0.5);
    }

    #[test]
    fn test_unused_edges_decay_over_time() {
        // Given: An edge that hasn't been used
        let mut memory = RuvectorMemory::new_test();
        let edge = memory.create_edge_with_last_used(
            "node_a", "node_b",
            0.5,
            Instant::now() - Duration::from_days(30)
        );

        // When: Periodic cleanup runs
        memory.apply_decay(DECAY_RATE, MIN_INTERACTIONS_BEFORE_PRUNE);

        // Then: Edge weight should have decayed
        let updated = memory.get_edge(&edge.src, &edge.dst).unwrap();
        assert!(updated.weight < 0.5);
    }

    #[test]
    fn test_attention_weights_update_from_success_patterns() {
        // Given: Graph attention engine with initial weights
        let mut attention = GraphAttentionEngine::new_test();
        let initial_weights = attention.get_edge_bias_weights();

        // When: Train on successful interaction patterns
        let patterns = vec![
            AttentionPattern {
                edges_used: vec![EdgeType::Cites],
                outcome_quality: 0.95,
            },
            AttentionPattern {
                edges_used: vec![EdgeType::Cites],
                outcome_quality: 0.90,
            },
        ];
        attention.train_on_patterns(&patterns);

        // Then: Edge type "Cites" should have higher attention bias
        let updated_weights = attention.get_edge_bias_weights();
        assert!(updated_weights[EdgeType::Cites] > initial_weights[EdgeType::Cites]);
    }
}
```

### 2.2 Loop B: Router Learning

**What the router learns:**

```
┌────────────────────────────────────────────────────────────────┐
│                    Router Learning Loop                         │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  For each query, LOG:                                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Router features (128-dim input vector)                │   │
│  │  - Chosen route (model, context, temp, top_p)            │   │
│  │  - Actual latency and cost                               │   │
│  │  - Quality score (judge model or task outcome)           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  Periodically RETRAIN FastGRNN:                                │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Objective: Prefer cheaper routes when quality holds     │   │
│  │            Escalate only when necessary                  │   │
│  │                                                           │   │
│  │  Loss = -Quality + λ·Cost + μ·LatencyPenalty             │   │
│  │                                                           │   │
│  │  Constraints:                                             │   │
│  │    - Quality must exceed threshold θ_min                  │   │
│  │    - Latency must meet SLA                                │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  RESULT: Router becomes self-learning policy over your stack   │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

**TDD Tests for Loop B:**

```rust
#[cfg(test)]
mod router_learning_tests {
    use super::*;

    #[test]
    fn test_router_prefers_smaller_model_when_quality_sufficient() {
        // Given: Training data showing 700M achieves same quality as 1.2B
        let training_data = vec![
            RouterSample {
                features: simple_query_features(),
                model_used: ModelSize::M700,
                quality: 0.92,
                latency_ms: 150.0,
                cost: 0.001,
            },
            RouterSample {
                features: simple_query_features(),
                model_used: ModelSize::B1_2,
                quality: 0.93,  // Only marginally better
                latency_ms: 300.0,
                cost: 0.003,
            },
        ];

        // When: Router is trained
        let mut router = FastGRNNRouter::new_test();
        router.train(&training_data, QUALITY_THRESHOLD);

        // Then: Router should prefer 700M for similar queries
        let decision = router.forward(&simple_query_features(), &initial_hidden());
        assert_eq!(decision.model, ModelSize::M700);
    }

    #[test]
    fn test_router_escalates_for_complex_queries() {
        // Given: Training data showing complex queries need larger models
        let training_data = vec![
            RouterSample {
                features: complex_query_features(),
                model_used: ModelSize::M700,
                quality: 0.45,  // Poor quality
                latency_ms: 150.0,
                cost: 0.001,
            },
            RouterSample {
                features: complex_query_features(),
                model_used: ModelSize::B2_6,
                quality: 0.91,  // Good quality
                latency_ms: 500.0,
                cost: 0.010,
            },
        ];

        // When: Router is trained
        let mut router = FastGRNNRouter::new_test();
        router.train(&training_data, QUALITY_THRESHOLD);

        // Then: Router should choose 2.6B for complex queries
        let decision = router.forward(&complex_query_features(), &initial_hidden());
        assert_eq!(decision.model, ModelSize::B2_6);
    }

    #[test]
    fn test_router_confidence_correlates_with_seen_patterns() {
        // Given: Router trained on specific feature patterns
        let mut router = FastGRNNRouter::new_test();
        let seen_features = vec![training_features_a(), training_features_b()];
        router.train(&samples_from_features(&seen_features), QUALITY_THRESHOLD);

        // When: Querying with seen vs unseen patterns
        let seen_decision = router.forward(&training_features_a(), &initial_hidden());
        let unseen_decision = router.forward(&novel_features(), &initial_hidden());

        // Then: Confidence should be higher for seen patterns
        assert!(seen_decision.confidence > unseen_decision.confidence);
    }

    #[test]
    fn test_router_ewc_prevents_forgetting() {
        // Given: Router trained on task A
        let mut router = FastGRNNRouter::new_test();
        let mut ewc = ElasticWeightConsolidation::new(0.4);
        router.train(&task_a_samples(), QUALITY_THRESHOLD);
        let task_a_accuracy_before = router.evaluate(&task_a_samples());

        // Compute Fisher and store optimal weights
        ewc.compute_fisher(&router, &task_a_samples());

        // When: Train on task B with EWC
        router.train_with_ewc(&task_b_samples(), &ewc, QUALITY_THRESHOLD);

        // Then: Task A accuracy should not significantly degrade
        let task_a_accuracy_after = router.evaluate(&task_a_samples());
        assert!(task_a_accuracy_after > task_a_accuracy_before - 0.05);
    }
}
```

### 2.3 Loop C: Compression and Abstraction

**How the system avoids bloat:**

```
┌────────────────────────────────────────────────────────────────┐
│               Compression and Abstraction Loop                  │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  PERIODICALLY (hourly/daily):                                   │
│                                                                  │
│  1. CLUSTER DETECTION                                           │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  Identify clusters of similar nodes in graph:            │   │
│  │  - Dense neighborhoods with similar embeddings           │   │
│  │  - Frequently co-retrieved node sets                     │   │
│  │  - High edge connectivity within cluster                 │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  2. LFM2 SUMMARIZATION                                         │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  For each cluster:                                        │   │
│  │  - Feed cluster nodes to LFM2                            │   │
│  │  - Generate summary "concept" node                        │   │
│  │  - Create embedding for concept                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  3. HIERARCHICAL ATTACHMENT                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Concept node becomes parent of cluster members        │   │
│  │  - Add "contains" edges from concept to members          │   │
│  │  - Future queries see concept first in attention         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  4. ARCHIVAL                                                    │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  - Old, rarely-used fine-grained nodes → cold storage    │   │
│  │  - Concept summaries stay in hot tier                    │   │
│  │  - Preserve graph structure for rehydration              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
│  RESULT: Hierarchy of concepts, not ever-growing bag of chunks │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

**TDD Tests for Loop C:**

```rust
#[cfg(test)]
mod compression_tests {
    use super::*;

    #[test]
    fn test_cluster_detection_finds_dense_neighborhoods() {
        // Given: Graph with clear clusters
        let mut memory = RuvectorMemory::new_test();

        // Cluster 1: ML topics (densely connected)
        let ml_nodes = vec![
            memory.insert_node("Neural networks learn patterns"),
            memory.insert_node("Deep learning uses multiple layers"),
            memory.insert_node("Backpropagation trains neural nets"),
        ];
        for i in 0..ml_nodes.len() {
            for j in i+1..ml_nodes.len() {
                memory.insert_edge(&ml_nodes[i], &ml_nodes[j], EdgeType::SameTopic, 0.9);
            }
        }

        // Cluster 2: Cooking topics (densely connected)
        let cooking_nodes = vec![
            memory.insert_node("Sourdough needs starter"),
            memory.insert_node("Bread baking requires patience"),
        ];
        memory.insert_edge(&cooking_nodes[0], &cooking_nodes[1], EdgeType::SameTopic, 0.85);

        // When: Run cluster detection
        let clusters = memory.detect_clusters(MIN_CLUSTER_SIZE, MIN_EDGE_DENSITY);

        // Then: Should find two distinct clusters
        assert_eq!(clusters.len(), 2);
        assert!(clusters.iter().any(|c| c.nodes.len() == 3)); // ML cluster
        assert!(clusters.iter().any(|c| c.nodes.len() == 2)); // Cooking cluster
    }

    #[test]
    fn test_summarization_creates_concept_node() {
        // Given: A cluster of related nodes
        let cluster = Cluster {
            nodes: vec![
                Node::new("Rust is memory safe"),
                Node::new("Rust has zero-cost abstractions"),
                Node::new("Rust prevents data races"),
            ],
            centroid: compute_centroid(&cluster.nodes),
        };

        // When: Generate summary
        let summarizer = ClusterSummarizer::new(lfm2_model());
        let concept = summarizer.summarize(&cluster);

        // Then: Concept should capture key themes
        assert!(concept.text.to_lowercase().contains("rust"));
        assert!(concept.node_type == NodeType::Concept);
        assert!(concept.metadata.contains_key("source_cluster_size"));
    }

    #[test]
    fn test_concept_nodes_are_prioritized_in_retrieval() {
        // Given: Memory with concept and detail nodes
        let mut memory = RuvectorMemory::new_test();
        let concept = memory.insert_node_typed(
            "Rust programming overview",
            NodeType::Concept
        );
        let detail = memory.insert_node_typed(
            "Rust's borrow checker enforces ownership",
            NodeType::Document
        );
        memory.insert_edge(&concept, &detail, EdgeType::Contains, 1.0);

        // When: Query about Rust
        let query_embedding = embed("Tell me about Rust");
        let results = memory.search_with_concept_boost(&query_embedding, 10);

        // Then: Concept should appear before (or with higher weight than) details
        let concept_idx = results.iter().position(|r| r.id == concept.id).unwrap();
        let detail_idx = results.iter().position(|r| r.id == detail.id).unwrap();
        assert!(concept_idx < detail_idx);
    }

    #[test]
    fn test_archival_moves_old_nodes_to_cold_storage() {
        // Given: Nodes with different access patterns
        let mut memory = RuvectorMemory::new_test();
        let hot_node = memory.insert_node_with_access(
            "Recently used content",
            AccessStats { last_used: now(), use_count: 50 }
        );
        let cold_node = memory.insert_node_with_access(
            "Old unused content",
            AccessStats { last_used: now() - Duration::from_days(90), use_count: 1 }
        );

        // When: Run archival
        memory.run_archival(
            MAX_AGE_DAYS,
            MIN_USE_COUNT,
            COLD_STORAGE_PATH
        );

        // Then: Hot node stays, cold node archived
        assert!(memory.contains(&hot_node.id));
        assert!(!memory.contains(&cold_node.id));
        assert!(cold_storage_contains(&cold_node.id));
    }
}
```

---

## 3. Weight-Level Self-Learning (Controlled)

### 3.1 The Safe Outer Loop

**Weight updates happen outside production, in a controlled pipeline:**

```
┌────────────────────────────────────────────────────────────────┐
│           Weight-Level Self-Learning Pipeline                   │
├────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  STEP 1: COLLECT TRAINING TRACES (continuous)           │   │
│  │                                                           │   │
│  │  From live system, store:                                 │   │
│  │  - (prompt, retrieved_context, final_answer, outcome)    │   │
│  │  - Judge scores or human ratings                         │   │
│  │  - Explicit error cases                                   │   │
│  │                                                           │   │
│  │  Tag by: domain, difficulty, risk_level                  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                           ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  STEP 2: BUILD ROLLING CURRICULUM (nightly/weekly)      │   │
│  │                                                           │   │
│  │  Sample recent traces:                                    │   │
│  │  - Up-weight hard or high-value tasks                    │   │
│  │  - Filter out cases where context was wrong              │   │
│  │                                                           │   │
│  │  Create three sets:                                       │   │
│  │  ┌───────────────┐ ┌───────────────┐ ┌───────────────┐  │   │
│  │  │      SFT      │ │  Preference   │ │   Retrieval   │  │   │
│  │  │    (good      │ │    Pairs      │ │  Correction   │  │   │
│  │  │   answers)    │ │ (good vs bad) │ │    (context   │  │   │
│  │  │               │ │               │ │   selection)  │  │   │
│  │  └───────────────┘ └───────────────┘ └───────────────┘  │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                           ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  STEP 3: TRAIN STUDENT VARIANTS (offline)               │   │
│  │                                                           │   │
│  │  Take current best LFM2 checkpoint:                      │   │
│  │  1. Run supervised fine-tuning on new traces             │   │
│  │  2. Optionally run preference objective on pairs         │   │
│  │  3. Validate on fixed holdout + public benchmarks        │   │
│  │                                                           │   │
│  │  Output: "LFM2-ruv-edition-vN"                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                     │
│                           ▼                                     │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │  STEP 4: GATED DEPLOYMENT (A/B testing)                 │   │
│  │                                                           │   │
│  │  ┌─────────────────────────────────────────────────────┐│   │
│  │  │  Production Traffic                                  ││   │
│  │  │  ┌────────────────┐  ┌────────────────┐            ││   │
│  │  │  │  90% → Current │  │  10% → Student │            ││   │
│  │  │  │     Model      │  │     vN         │            ││   │
│  │  │  └────────────────┘  └────────────────┘            ││   │
│  │  └─────────────────────────────────────────────────────┘│   │
│  │                                                           │   │
│  │  Compare: quality, latency, failure_rate                 │   │
│  │  Promote IFF: student dominates OR ties on key metrics   │   │
│  │                                                           │   │
│  │  ⚠️  Never free-write weights in-place                  │   │
│  │  ⚠️  Always retrain in controlled loop                   │   │
│  │  ⚠️  Promote only when safe                              │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└────────────────────────────────────────────────────────────────┘
```

**TDD Tests for Weight-Level Learning:**

```rust
#[cfg(test)]
mod weight_learning_tests {
    use super::*;

    #[test]
    fn test_trace_collection_captures_all_components() {
        // Given: A completed interaction
        let trace_collector = TraceCollector::new_test();
        let interaction = Interaction {
            prompt: "What is Rust?",
            context: vec!["Rust is a systems language"],
            response: "Rust is a memory-safe systems programming language",
            quality_score: 0.92,
            task_outcome: TaskOutcome::Success,
        };

        // When: Trace is collected
        let trace = trace_collector.collect(&interaction);

        // Then: All components should be present
        assert!(trace.prompt.is_some());
        assert!(trace.context.len() > 0);
        assert!(trace.response.is_some());
        assert!(trace.quality_score.is_some());
        assert!(trace.domain_tags.len() > 0);
    }

    #[test]
    fn test_curriculum_upweights_hard_tasks() {
        // Given: Mix of easy and hard traces
        let traces = vec![
            Trace { difficulty: 0.2, quality: 0.95, ..default() },  // Easy, good
            Trace { difficulty: 0.9, quality: 0.85, ..default() },  // Hard, good
            Trace { difficulty: 0.3, quality: 0.60, ..default() },  // Easy, bad
        ];

        // When: Build curriculum
        let curriculum = CurriculumBuilder::new()
            .upweight_hard_tasks(true)
            .filter_bad_quality(0.7)
            .build(&traces);

        // Then: Hard successful trace should have higher weight
        let hard_weight = curriculum.weight_for(&traces[1]);
        let easy_weight = curriculum.weight_for(&traces[0]);
        assert!(hard_weight > easy_weight);

        // And: Bad quality trace should be filtered
        assert!(!curriculum.contains(&traces[2]));
    }

    #[test]
    fn test_preference_pairs_correctly_ordered() {
        // Given: Same query with different quality responses
        let good_response = Response { text: "Detailed answer...", quality: 0.9 };
        let bad_response = Response { text: "I don't know", quality: 0.3 };
        let query = "Explain backpropagation";

        // When: Create preference pair
        let pair = PreferencePair::from_responses(query, &good_response, &bad_response);

        // Then: Good should be preferred
        assert_eq!(pair.chosen, good_response.text);
        assert_eq!(pair.rejected, bad_response.text);
    }

    #[test]
    fn test_student_validation_gates_deployment() {
        // Given: Student model that underperforms on holdout
        let student = StudentModel::new_test();
        let holdout = HoldoutDataset::load_test();
        let baseline_accuracy = 0.85;
        let student_accuracy = 0.78;  // Below baseline

        // When: Validate for deployment
        let validation = ValidationResult::new(student_accuracy, baseline_accuracy);

        // Then: Should NOT be approved for deployment
        assert!(!validation.approved_for_deployment());
        assert!(validation.rejection_reason().contains("accuracy"));
    }

    #[test]
    fn test_ab_test_detects_regression() {
        // Given: A/B test results
        let ab_results = ABTestResults {
            control: ABMetrics { quality: 0.90, latency_p50: 200.0, failure_rate: 0.02 },
            treatment: ABMetrics { quality: 0.88, latency_p50: 180.0, failure_rate: 0.05 },
        };

        // When: Evaluate for promotion
        let decision = ABDecision::evaluate(&ab_results, SIGNIFICANCE_THRESHOLD);

        // Then: Should NOT promote due to quality regression + higher failure rate
        assert_eq!(decision, ABDecision::KeepControl);
        assert!(decision.reasons().contains("quality_regression"));
    }
}
```

---

## 4. Test-Driven Development Plan

### 4.1 Testing Pyramid

```
                    ┌─────────────────┐
                    │    E2E Tests    │  (5%)
                    │  Full pipeline  │
                    └────────┬────────┘
                             │
               ┌─────────────┴─────────────┐
               │    Integration Tests      │  (20%)
               │  Cross-component flows    │
               └─────────────┬─────────────┘
                             │
        ┌────────────────────┴────────────────────┐
        │           Unit Tests                    │  (75%)
        │  Individual functions & modules          │
        └─────────────────────────────────────────┘
```

### 4.2 Test Categories by Component

#### 4.2.1 Orchestrator Tests

```rust
#[cfg(test)]
mod orchestrator_tests {
    #[test]
    fn test_request_routing_respects_session() { }

    #[test]
    fn test_rate_limiting_rejects_excess_requests() { }

    #[test]
    fn test_cache_hit_bypasses_processing() { }

    #[test]
    fn test_cache_miss_triggers_full_pipeline() { }

    #[test]
    fn test_error_handling_returns_graceful_response() { }

    #[test]
    fn test_metrics_recorded_for_all_requests() { }
}
```

#### 4.2.2 Embedding Service Tests

```rust
#[cfg(test)]
mod embedding_tests {
    #[test]
    fn test_embedding_dimension_matches_config() { }

    #[test]
    fn test_similar_texts_have_similar_embeddings() { }

    #[test]
    fn test_different_texts_have_different_embeddings() { }

    #[test]
    fn test_long_text_truncation() { }

    #[test]
    fn test_batch_embedding_matches_individual() { }

    #[test]
    fn test_empty_string_handling() { }
}
```

#### 4.2.3 Router Tests

```rust
#[cfg(test)]
mod router_tests {
    #[test]
    fn test_forward_produces_valid_probabilities() { }

    #[test]
    fn test_hidden_state_updates_across_calls() { }

    #[test]
    fn test_confidence_threshold_triggers_fallback() { }

    #[test]
    fn test_gradient_computation() { }

    #[test]
    fn test_sparse_matrix_operations() { }

    #[test]
    fn test_low_rank_matrix_approximation() { }
}
```

#### 4.2.4 Memory Tests

```rust
#[cfg(test)]
mod memory_tests {
    #[test]
    fn test_hnsw_search_returns_k_neighbors() { }

    #[test]
    fn test_graph_expansion_respects_hop_limit() { }

    #[test]
    fn test_writeback_queue_batches_correctly() { }

    #[test]
    fn test_deduplication_prevents_near_duplicates() { }

    #[test]
    fn test_metadata_filtering() { }

    #[test]
    fn test_edge_weight_update() { }
}
```

#### 4.2.5 Attention Tests

```rust
#[cfg(test)]
mod attention_tests {
    #[test]
    fn test_attention_weights_sum_to_one() { }

    #[test]
    fn test_edge_features_influence_attention() { }

    #[test]
    fn test_multi_head_concatenation() { }

    #[test]
    fn test_residual_connection_preserved() { }

    #[test]
    fn test_layer_norm_normalization() { }

    #[test]
    fn test_attention_ranking_matches_weights() { }
}
```

#### 4.2.6 Inference Tests

```rust
#[cfg(test)]
mod inference_tests {
    #[test]
    fn test_model_loading_correct_size() { }

    #[test]
    fn test_kv_cache_reuse() { }

    #[test]
    fn test_generation_respects_max_tokens() { }

    #[test]
    fn test_temperature_affects_randomness() { }

    #[test]
    fn test_top_p_filtering() { }

    #[test]
    fn test_model_eviction_under_memory_pressure() { }
}
```

#### 4.2.7 Learning Tests

```rust
#[cfg(test)]
mod learning_tests {
    #[test]
    fn test_replay_buffer_reservoir_sampling() { }

    #[test]
    fn test_ewc_regularization_value() { }

    #[test]
    fn test_fisher_information_computation() { }

    #[test]
    fn test_quality_judge_score_range() { }

    #[test]
    fn test_writeback_threshold_filtering() { }

    #[test]
    fn test_background_training_thread() { }
}
```

### 4.3 Integration Test Scenarios

```rust
#[cfg(test)]
mod integration_tests {
    /// Test full request-response cycle
    #[tokio::test]
    async fn test_end_to_end_query() {
        let system = RuvLLMSystem::new_test().await;

        let response = system.process(Request {
            query: "What is machine learning?",
            session_id: Some("test-session"),
            constraints: Default::default(),
        }).await.unwrap();

        assert!(!response.text.is_empty());
        assert!(response.confidence > 0.0);
        assert!(!response.sources.is_empty());
    }

    /// Test multi-turn conversation with context
    #[tokio::test]
    async fn test_multi_turn_context() {
        let system = RuvLLMSystem::new_test().await;
        let session = "multi-turn-test";

        // Turn 1
        let r1 = system.process(Request {
            query: "What is Rust?",
            session_id: Some(session),
            ..Default::default()
        }).await.unwrap();

        // Turn 2 (should use KV cache)
        let r2 = system.process(Request {
            query: "What are its main features?",
            session_id: Some(session),
            ..Default::default()
        }).await.unwrap();

        // Response should reference Rust from context
        assert!(r2.text.to_lowercase().contains("rust") ||
                r2.text.to_lowercase().contains("memory") ||
                r2.text.to_lowercase().contains("safety"));
    }

    /// Test that learning loop updates memory
    #[tokio::test]
    async fn test_learning_updates_memory() {
        let system = RuvLLMSystem::new_test().await;
        let initial_node_count = system.memory.node_count();

        // Process high-quality interaction
        let response = system.process_with_feedback(
            Request { query: "Novel question...", ..Default::default() },
            Feedback { quality: 0.95, explicit_rating: Some(5) }
        ).await.unwrap();

        // Memory should have grown
        let final_node_count = system.memory.node_count();
        assert!(final_node_count > initial_node_count);
    }

    /// Test router learns from experience
    #[tokio::test]
    async fn test_router_adaptation() {
        let mut system = RuvLLMSystem::new_test().await;

        // Process many simple queries
        for _ in 0..100 {
            system.process(Request {
                query: "Simple factual question",
                ..Default::default()
            }).await.unwrap();
        }

        // Trigger training
        system.learning_service.train_router().await;

        // Router should now prefer smaller models for similar queries
        let decision = system.router.forward(
            &simple_query_features(),
            &initial_hidden()
        );
        assert!(decision.model == ModelSize::M350 || decision.model == ModelSize::M700);
    }
}
```

---

## 5. Benchmarking Suite

### 5.1 Performance Benchmarks

```rust
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn embedding_benchmark(c: &mut Criterion) {
    let embedder = EmbeddingService::new_test();

    let mut group = c.benchmark_group("embedding");

    for size in [32, 128, 512, 2048].iter() {
        let text = "a".repeat(*size);
        group.bench_with_input(
            BenchmarkId::new("embed", size),
            &text,
            |b, t| b.iter(|| embedder.embed(t))
        );
    }

    group.finish();
}

fn hnsw_search_benchmark(c: &mut Criterion) {
    let memory = RuvectorMemory::new_with_data(100_000);  // 100K vectors
    let query = random_vector(384);

    let mut group = c.benchmark_group("hnsw_search");

    for k in [10, 32, 64].iter() {
        for ef in [32, 64, 128].iter() {
            group.bench_with_input(
                BenchmarkId::new(format!("k={},ef={}", k, ef), ""),
                &(k, ef),
                |b, (k, ef)| b.iter(|| memory.search(&query, **k, **ef))
            );
        }
    }

    group.finish();
}

fn router_forward_benchmark(c: &mut Criterion) {
    let router = FastGRNNRouter::new_test();
    let features = random_vector(128);
    let hidden = random_vector(64);

    c.bench_function("router_forward", |b| {
        b.iter(|| router.forward(&features, &hidden))
    });
}

fn graph_attention_benchmark(c: &mut Criterion) {
    let attention = GraphAttentionEngine::new_test();
    let query = random_vector(384);
    let subgraph = generate_subgraph(50, 100);  // 50 nodes, 100 edges

    c.bench_function("graph_attention", |b| {
        b.iter(|| attention.attend(&query, &subgraph))
    });
}

criterion_group!(
    benches,
    embedding_benchmark,
    hnsw_search_benchmark,
    router_forward_benchmark,
    graph_attention_benchmark
);
criterion_main!(benches);
```

### 5.2 Quality Benchmarks

```rust
/// Benchmark suite for quality metrics
pub struct QualityBenchmark {
    dataset: BenchmarkDataset,
    judge: QualityJudge,
}

impl QualityBenchmark {
    pub async fn run(&self, system: &RuvLLMSystem) -> QualityResults {
        let mut results = QualityResults::default();

        for sample in &self.dataset.samples {
            let response = system.process(Request {
                query: sample.query.clone(),
                ..Default::default()
            }).await.unwrap();

            // Judge quality
            let quality = self.judge.evaluate(
                &sample.query,
                &response.text,
                &response.sources
            ).await;

            // Check against ground truth if available
            if let Some(expected) = &sample.expected_answer {
                let f1 = compute_f1(&response.text, expected);
                results.f1_scores.push(f1);
            }

            results.quality_scores.push(quality);
            results.latencies.push(response.latency);
        }

        results
    }
}
```

---

## 6. Iteration Milestones

### 6.1 Phase 1: Foundation (Weeks 1-2)

| Milestone | Deliverables | Tests |
|-----------|--------------|-------|
| M1.1 | Embedding service stub | Dimension tests |
| M1.2 | Memory service with HNSW | Search tests |
| M1.3 | Basic orchestrator | Integration smoke tests |
| M1.4 | Mock LFM2 interface | Interface contract tests |

### 6.2 Phase 2: Core Pipeline (Weeks 3-4)

| Milestone | Deliverables | Tests |
|-----------|--------------|-------|
| M2.1 | FastGRNN router | Forward pass tests |
| M2.2 | Graph attention engine | Attention computation tests |
| M2.3 | Context builder | Deduplication, truncation tests |
| M2.4 | End-to-end pipeline | Full flow integration tests |

### 6.3 Phase 3: Learning Loops (Weeks 5-6)

| Milestone | Deliverables | Tests |
|-----------|--------------|-------|
| M3.1 | Quality judge | Evaluation tests |
| M3.2 | Replay buffer | Sampling distribution tests |
| M3.3 | EWC integration | Forgetting prevention tests |
| M3.4 | Memory writeback | Graph update tests |

### 6.4 Phase 4: Optimization (Weeks 7-8)

| Milestone | Deliverables | Tests |
|-----------|--------------|-------|
| M4.1 | Router training loop | Learning convergence tests |
| M4.2 | Compression/abstraction | Cluster detection tests |
| M4.3 | Performance tuning | Benchmark suite |
| M4.4 | Production hardening | Load tests, failure injection |

---

## 7. Refinement Checklist

### 7.1 Per-Component Checklist

```
[ ] Orchestrator
    [ ] Request validation
    [ ] Session management
    [ ] Rate limiting
    [ ] Caching
    [ ] Error handling
    [ ] Metrics export

[ ] Embedding Service
    [ ] LFM2 encoder integration
    [ ] Dimension projection
    [ ] Batch processing
    [ ] Tokenization
    [ ] Truncation handling

[ ] FastGRNN Router
    [ ] Cell implementation
    [ ] Sparse weight matrices
    [ ] Low-rank recurrent matrices
    [ ] Output heads
    [ ] Confidence calibration
    [ ] Training loop

[ ] Memory Service
    [ ] HNSW configuration
    [ ] Graph storage
    [ ] Edge operations
    [ ] Writeback queue
    [ ] Deduplication
    [ ] Archival

[ ] Graph Attention
    [ ] Multi-head attention
    [ ] Edge feature encoding
    [ ] Layer stacking
    [ ] Residual connections
    [ ] Output ranking

[ ] Inference Pool
    [ ] Model loading
    [ ] Lazy initialization
    [ ] KV cache management
    [ ] Quantization selection
    [ ] LRU eviction

[ ] Learning Service
    [ ] Quality evaluation
    [ ] Replay buffer
    [ ] EWC regularization
    [ ] Background training
    [ ] Writeback logic
    [ ] Compression jobs
```

### 7.2 Quality Gates

| Gate | Criteria | Status |
|------|----------|--------|
| Unit test coverage | >80% | ⬜ |
| Integration tests passing | 100% | ⬜ |
| Latency P50 | <500ms | ⬜ |
| Quality score mean | >0.8 | ⬜ |
| Router accuracy | >90% | ⬜ |
| Memory efficiency | <4GB | ⬜ |
| No memory leaks | 24h stress test | ⬜ |
| Forgetting rate | <5%/10K | ⬜ |

---

*Document Version: 1.0*
*Last Updated: 2025-12-02*
*Author: RuvLLM Architecture Team*
