use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde_json::json;

// Mock implementations of baseline AI reasoning systems for comparison
mod baseline_systems {
    use super::*;

    pub struct SimpleRuleEngine {
        rules: Vec<String>,
    }

    impl SimpleRuleEngine {
        pub fn new() -> Self {
            Self { rules: Vec::new() }
        }

        pub fn add_rule(&mut self, rule: &str) {
            self.rules.push(rule.to_string());
        }

        pub fn infer(&self, facts: &[String]) -> Vec<String> {
            // Simple pattern matching inference
            let mut inferences = Vec::new();

            for rule in &self.rules {
                if rule.contains("if") && rule.contains("then") {
                    // Very basic rule processing simulation
                    for fact in facts {
                        if rule.contains(&fact[..std::cmp::min(fact.len(), 10)]) {
                            inferences.push(format!("inferred_from_{}", fact));
                        }
                    }
                }
            }

            inferences
        }
    }

    pub struct BasicTextAnalyzer {
        positive_words: Vec<String>,
        negative_words: Vec<String>,
    }

    impl BasicTextAnalyzer {
        pub fn new() -> Self {
            Self {
                positive_words: vec![
                    "love".to_string(), "great".to_string(), "excellent".to_string(),
                    "amazing".to_string(), "wonderful".to_string(), "happy".to_string(),
                ],
                negative_words: vec![
                    "hate".to_string(), "terrible".to_string(), "awful".to_string(),
                    "bad".to_string(), "horrible".to_string(), "sad".to_string(),
                ],
            }
        }

        pub fn analyze_sentiment(&self, text: &str) -> f64 {
            let words: Vec<&str> = text.to_lowercase().split_whitespace().collect();
            let positive_count = words.iter()
                .filter(|word| self.positive_words.contains(&word.to_string()))
                .count();
            let negative_count = words.iter()
                .filter(|word| self.negative_words.contains(&word.to_string()))
                .count();

            let total_words = words.len() as f64;
            if total_words == 0.0 {
                return 0.0;
            }

            (positive_count as f64 - negative_count as f64) / total_words
        }

        pub fn extract_preferences(&self, text: &str) -> Vec<String> {
            let mut preferences = Vec::new();
            let text_lower = text.to_lowercase();

            // Simple keyword-based preference extraction
            if text_lower.contains("prefer") || text_lower.contains("like") {
                let words: Vec<&str> = text.split_whitespace().collect();
                for (i, word) in words.iter().enumerate() {
                    if word.to_lowercase() == "prefer" || word.to_lowercase() == "like" {
                        if i + 1 < words.len() {
                            preferences.push(words[i + 1].to_string());
                        }
                    }
                }
            }

            preferences
        }
    }

    pub struct NaivePlanner {
        actions: Vec<String>,
    }

    impl NaivePlanner {
        pub fn new() -> Self {
            Self { actions: Vec::new() }
        }

        pub fn add_action(&mut self, action: &str) {
            self.actions.push(action.to_string());
        }

        pub fn plan(&self, _initial_state: &str, goal: &str) -> Vec<String> {
            // Naive planning: just return all actions
            let mut plan = self.actions.clone();
            plan.push(format!("achieve_{}", goal));
            plan
        }
    }
}

fn bench_graph_reasoning_vs_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_reasoning_comparison");

    let fact_counts = [100, 500, 1000, 2000];

    for &fact_count in fact_counts.iter() {
        // Our psycho-symbolic reasoner
        group.bench_with_input(
            BenchmarkId::new("psycho_symbolic_reasoner", fact_count),
            &fact_count,
            |b, &fact_count| {
                b.iter(|| {
                    let mut graph = graph_reasoner::KnowledgeGraph::new();
                    let mut inference_engine = graph_reasoner::InferenceEngine::new();
                    let mut rule_engine = graph_reasoner::RuleEngine::new();

                    // Add facts
                    for i in 0..fact_count {
                        let fact = graph_reasoner::Fact::new(
                            &format!("entity_{}", i),
                            "relates_to",
                            &format!("entity_{}", (i + 1) % fact_count)
                        );
                        let _ = graph.add_fact(fact);
                    }

                    // Add some rules
                    let rule = graph_reasoner::Rule::new(
                        "transitivity",
                        r#"{"if": [{"subject": "?x", "predicate": "relates_to", "object": "?y"}, {"subject": "?y", "predicate": "relates_to", "object": "?z"}], "then": {"subject": "?x", "predicate": "indirectly_relates_to", "object": "?z"}}"#.to_string()
                    ).unwrap();
                    rule_engine.add_rule(rule);

                    // Perform inference
                    let results = inference_engine.infer(&mut graph, &rule_engine, 3);

                    black_box((graph, results));
                });
            }
        );

        // Baseline simple rule engine
        group.bench_with_input(
            BenchmarkId::new("simple_rule_engine", fact_count),
            &fact_count,
            |b, &fact_count| {
                b.iter(|| {
                    let mut rule_engine = baseline_systems::SimpleRuleEngine::new();

                    // Add rules
                    rule_engine.add_rule("if entity_X relates_to entity_Y then entity_X indirectly_relates_to entity_Y");

                    // Create facts
                    let mut facts = Vec::new();
                    for i in 0..fact_count {
                        facts.push(format!("entity_{} relates_to entity_{}", i, (i + 1) % fact_count));
                    }

                    // Perform inference
                    let results = rule_engine.infer(&facts);

                    black_box((rule_engine, results));
                });
            }
        );
    }

    group.finish();
}

fn bench_text_analysis_vs_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_analysis_comparison");

    let test_texts = vec![
        "I love this product! It's amazing and wonderful. I'm so happy with it.",
        "This is terrible. I hate everything about it. It's awful and disappointing.",
        "The weather is nice today. I prefer sunny days over rainy ones. I like to walk in the park.",
        "I'm feeling excited about the new project, but also worried about the deadlines. I prefer working in teams rather than alone.",
    ];

    for (i, text) in test_texts.iter().enumerate() {
        // Our text extractor
        group.bench_with_input(
            BenchmarkId::new("psycho_symbolic_extractor", i),
            text,
            |b, text| {
                b.iter(|| {
                    let extractor = extractors::TextExtractor::new();
                    let sentiment = extractor.analyze_sentiment(black_box(text));
                    let preferences = extractor.extract_preferences(black_box(text));
                    let emotions = extractor.detect_emotions(black_box(text));

                    black_box((sentiment, preferences, emotions));
                });
            }
        );

        // Baseline text analyzer
        group.bench_with_input(
            BenchmarkId::new("basic_text_analyzer", i),
            text,
            |b, text| {
                b.iter(|| {
                    let analyzer = baseline_systems::BasicTextAnalyzer::new();
                    let sentiment = analyzer.analyze_sentiment(black_box(text));
                    let preferences = analyzer.extract_preferences(black_box(text));

                    black_box((sentiment, preferences));
                });
            }
        );
    }

    group.finish();
}

fn bench_planning_vs_baseline(c: &mut Criterion) {
    let mut group = c.benchmark_group("planning_comparison");

    let complexities = [20, 50, 100];

    for &complexity in complexities.iter() {
        // Our A* planner
        group.bench_with_input(
            BenchmarkId::new("astar_planner", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    // Generate state
                    let mut properties = HashMap::new();
                    for i in 0..complexity {
                        properties.insert(format!("prop_{}", i), serde_json::Value::Bool(false));
                    }
                    let initial_state = planner::State::new(properties);

                    // Generate goal
                    let mut goal_conditions = HashMap::new();
                    goal_conditions.insert("prop_0".to_string(), serde_json::Value::Bool(true));
                    let goal = planner::Goal::new("achieve_goal", goal_conditions);

                    // Generate actions
                    let mut actions = Vec::new();
                    for i in 0..complexity {
                        let mut effects = HashMap::new();
                        effects.insert(format!("prop_{}", i), serde_json::Value::Bool(true));

                        let action = planner::Action::new(
                            &format!("action_{}", i),
                            HashMap::new(),
                            effects,
                            1.0,
                        );
                        actions.push(action);
                    }

                    let planner = planner::AStarPlanner::new();
                    let plan = planner.plan(
                        black_box(&initial_state),
                        black_box(&goal),
                        black_box(&actions),
                        Some(1000)
                    );

                    black_box(plan);
                });
            }
        );

        // Baseline naive planner
        group.bench_with_input(
            BenchmarkId::new("naive_planner", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    let mut planner = baseline_systems::NaivePlanner::new();

                    // Add actions
                    for i in 0..complexity {
                        planner.add_action(&format!("action_{}", i));
                    }

                    let initial_state = "initial";
                    let goal = "goal";

                    let plan = planner.plan(black_box(initial_state), black_box(goal));

                    black_box(plan);
                });
            }
        );
    }

    group.finish();
}

fn bench_memory_efficiency_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_efficiency_comparison");

    let data_sizes = [1000, 5000, 10000];

    for &size in data_sizes.iter() {
        // Our system (optimized data structures)
        group.bench_with_input(
            BenchmarkId::new("optimized_structures", size),
            &size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let start = Instant::now();

                    for _ in 0..iters {
                        let mut graph = graph_reasoner::KnowledgeGraph::new();

                        for i in 0..size {
                            let fact = graph_reasoner::Fact::new(
                                &format!("e_{}", i),
                                "r",
                                &format!("e_{}", (i + 1) % size)
                            );
                            let _ = graph.add_fact(fact);
                        }

                        // Perform some operations
                        let stats = graph.get_statistics();

                        black_box((graph, stats));
                    }

                    start.elapsed()
                });
            }
        );

        // Baseline system (naive data structures)
        group.bench_with_input(
            BenchmarkId::new("naive_structures", size),
            &size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let start = Instant::now();

                    for _ in 0..iters {
                        // Simulate naive storage using basic Vec and HashMap
                        let mut facts: Vec<String> = Vec::new();
                        let mut index: HashMap<String, Vec<usize>> = HashMap::new();

                        for i in 0..size {
                            let fact = format!("e_{} r e_{}", i, (i + 1) % size);
                            let fact_id = facts.len();
                            facts.push(fact.clone());

                            // Naive indexing
                            let subject = format!("e_{}", i);
                            index.entry(subject).or_insert_with(Vec::new).push(fact_id);
                        }

                        // Simulate query operations
                        let mut query_results = Vec::new();
                        for i in 0..std::cmp::min(10, size) {
                            let subject = format!("e_{}", i);
                            if let Some(fact_ids) = index.get(&subject) {
                                for &fact_id in fact_ids {
                                    query_results.push(facts[fact_id].clone());
                                }
                            }
                        }

                        black_box((facts, index, query_results));
                    }

                    start.elapsed()
                });
            }
        );
    }

    group.finish();
}

fn bench_scalability_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("scalability_comparison");

    // Test how each system scales with increasing complexity
    let complexities = [100, 500, 1000, 2000, 5000];

    for &complexity in complexities.iter() {
        // Our system scalability
        group.bench_with_input(
            BenchmarkId::new("psycho_symbolic_scalability", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    // Mixed workload test
                    let mut graph = graph_reasoner::KnowledgeGraph::new();
                    let extractor = extractors::TextExtractor::new();

                    // Graph operations
                    for i in 0..complexity / 10 {
                        let fact = graph_reasoner::Fact::new(
                            &format!("entity_{}", i),
                            "connects_to",
                            &format!("entity_{}", (i + 1) % (complexity / 10))
                        );
                        let _ = graph.add_fact(fact);
                    }

                    // Text processing
                    let test_text = format!("Processing {} items with various preferences and emotions.", complexity);
                    let analysis = extractor.analyze_all(&test_text);

                    // Planning
                    let mut properties = HashMap::new();
                    let prop_count = std::cmp::min(complexity / 50, 100);
                    for i in 0..prop_count {
                        properties.insert(format!("p_{}", i), serde_json::Value::Bool(false));
                    }
                    let state = planner::State::new(properties);

                    let mut goal_conditions = HashMap::new();
                    goal_conditions.insert("p_0".to_string(), serde_json::Value::Bool(true));
                    let goal = planner::Goal::new("goal", goal_conditions);

                    let actions = vec![
                        planner::Action::new(
                            "achieve",
                            HashMap::new(),
                            {
                                let mut effects = HashMap::new();
                                effects.insert("p_0".to_string(), serde_json::Value::Bool(true));
                                effects
                            },
                            1.0,
                        )
                    ];

                    let planner = planner::AStarPlanner::new();
                    let plan = planner.plan(&state, &goal, &actions, Some(100));

                    black_box((graph, analysis, plan));
                });
            }
        );

        // Baseline system scalability
        group.bench_with_input(
            BenchmarkId::new("baseline_scalability", complexity),
            &complexity,
            |b, &complexity| {
                b.iter(|| {
                    // Baseline mixed workload
                    let mut rule_engine = baseline_systems::SimpleRuleEngine::new();
                    let analyzer = baseline_systems::BasicTextAnalyzer::new();
                    let mut planner = baseline_systems::NaivePlanner::new();

                    // Rule engine operations
                    rule_engine.add_rule("if X connects_to Y then X relates_to Y");
                    let mut facts = Vec::new();
                    for i in 0..complexity / 10 {
                        facts.push(format!("entity_{} connects_to entity_{}", i, (i + 1) % (complexity / 10)));
                    }
                    let inferences = rule_engine.infer(&facts);

                    // Text analysis
                    let test_text = format!("Processing {} items with various preferences and emotions.", complexity);
                    let sentiment = analyzer.analyze_sentiment(&test_text);
                    let preferences = analyzer.extract_preferences(&test_text);

                    // Planning
                    for i in 0..std::cmp::min(complexity / 50, 100) {
                        planner.add_action(&format!("action_{}", i));
                    }
                    let plan = planner.plan("initial", "goal");

                    black_box((inferences, sentiment, preferences, plan));
                });
            }
        );
    }

    group.finish();
}

fn bench_accuracy_vs_performance_tradeoff(c: &mut Criterion) {
    let mut group = c.benchmark_group("accuracy_performance_tradeoff");

    // Test different accuracy/performance configurations
    let test_text = "I absolutely love this incredible product! It makes me feel so happy and excited. I prefer this over all other alternatives. However, I'm slightly worried about the price and concerned about long-term durability.";

    // High accuracy mode (our system)
    group.bench_function("high_accuracy_mode", |b| {
        b.iter(|| {
            let extractor = extractors::TextExtractor::new();

            // Full analysis with all features
            let sentiment = extractor.analyze_sentiment(black_box(test_text));
            let preferences = extractor.extract_preferences(black_box(test_text));
            let emotions = extractor.detect_emotions(black_box(test_text));
            let combined = extractor.analyze_all(black_box(test_text));

            black_box((sentiment, preferences, emotions, combined));
        });
    });

    // Fast mode (baseline system)
    group.bench_function("fast_mode", |b| {
        b.iter(|| {
            let analyzer = baseline_systems::BasicTextAnalyzer::new();

            // Basic analysis only
            let sentiment = analyzer.analyze_sentiment(black_box(test_text));
            let preferences = analyzer.extract_preferences(black_box(test_text));

            black_box((sentiment, preferences));
        });
    });

    group.finish();
}

fn bench_real_world_workload_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("real_world_workload");

    // Simulate a realistic AI reasoning workload
    group.bench_function("realistic_workload_psycho_symbolic", |b| {
        b.iter(|| {
            // Initialize components
            let mut graph = graph_reasoner::KnowledgeGraph::new();
            let extractor = extractors::TextExtractor::new();
            let planner = planner::AStarPlanner::new();

            // Knowledge graph construction (20% of workload)
            for i in 0..200 {
                let fact = graph_reasoner::Fact::new(
                    &format!("user_{}", i % 50),
                    if i % 3 == 0 { "likes" } else { "dislikes" },
                    &format!("item_{}", i % 30)
                );
                let _ = graph.add_fact(fact);
            }

            // Text analysis (50% of workload)
            let texts = vec![
                "I love the new features but hate the complexity",
                "The interface is great, I prefer this over the old version",
                "I'm excited about the update but worried about bugs",
                "This is amazing! I feel so happy using it",
                "The performance is terrible, I'm frustrated",
            ];

            let mut all_analyses = Vec::new();
            for text in &texts {
                for _ in 0..10 { // Simulate multiple analyses per text
                    let analysis = extractor.analyze_all(text);
                    all_analyses.push(analysis);
                }
            }

            // Planning tasks (30% of workload)
            for task_id in 0..5 {
                let mut properties = HashMap::new();
                for i in 0..20 {
                    properties.insert(format!("task_{}_prop_{}", task_id, i), serde_json::Value::Bool(i % 2 == 0));
                }
                let state = planner::State::new(properties);

                let mut goal_conditions = HashMap::new();
                goal_conditions.insert(format!("task_{}_prop_0", task_id), serde_json::Value::Bool(true));
                let goal = planner::Goal::new(&format!("goal_{}", task_id), goal_conditions);

                let actions = vec![
                    planner::Action::new(
                        &format!("action_{}", task_id),
                        HashMap::new(),
                        {
                            let mut effects = HashMap::new();
                            effects.insert(format!("task_{}_prop_0", task_id), serde_json::Value::Bool(true));
                            effects
                        },
                        1.0 + (task_id as f64 * 0.5),
                    )
                ];

                let plan = planner.plan(&state, &goal, &actions, Some(100));
                black_box(plan);
            }

            black_box((graph, all_analyses));
        });
    });

    group.bench_function("realistic_workload_baseline", |b| {
        b.iter(|| {
            // Baseline system simulation
            let mut rule_engine = baseline_systems::SimpleRuleEngine::new();
            let analyzer = baseline_systems::BasicTextAnalyzer::new();
            let mut planner = baseline_systems::NaivePlanner::new();

            // Knowledge representation (20% of workload)
            let mut facts = Vec::new();
            for i in 0..200 {
                facts.push(format!("user_{} {} item_{}", i % 50, if i % 3 == 0 { "likes" } else { "dislikes" }, i % 30));
            }
            rule_engine.add_rule("if user_X likes item_Y then user_X prefers item_Y");
            let inferences = rule_engine.infer(&facts);

            // Text analysis (50% of workload)
            let texts = vec![
                "I love the new features but hate the complexity",
                "The interface is great, I prefer this over the old version",
                "I'm excited about the update but worried about bugs",
                "This is amazing! I feel so happy using it",
                "The performance is terrible, I'm frustrated",
            ];

            let mut all_analyses = Vec::new();
            for text in &texts {
                for _ in 0..10 {
                    let sentiment = analyzer.analyze_sentiment(text);
                    let preferences = analyzer.extract_preferences(text);
                    all_analyses.push((sentiment, preferences));
                }
            }

            // Planning tasks (30% of workload)
            for task_id in 0..5 {
                for i in 0..20 {
                    planner.add_action(&format!("task_{}_action_{}", task_id, i));
                }
                let plan = planner.plan("initial", &format!("goal_{}", task_id));
                black_box(plan);
            }

            black_box((inferences, all_analyses));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_reasoning_vs_baseline,
    bench_text_analysis_vs_baseline,
    bench_planning_vs_baseline,
    bench_memory_efficiency_comparison,
    bench_scalability_comparison,
    bench_accuracy_vs_performance_tradeoff,
    bench_real_world_workload_comparison
);

criterion_main!(benches);