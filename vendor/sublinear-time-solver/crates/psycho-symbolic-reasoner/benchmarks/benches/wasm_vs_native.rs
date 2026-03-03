use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::process::Command;
use std::time::{Duration, Instant};
use std::collections::HashMap;

// Performance comparison framework for WASM vs Native
struct PerformanceComparison {
    wasm_results: HashMap<String, Duration>,
    native_results: HashMap<String, Duration>,
}

impl PerformanceComparison {
    fn new() -> Self {
        Self {
            wasm_results: HashMap::new(),
            native_results: HashMap::new(),
        }
    }

    fn add_result(&mut self, test_name: &str, wasm_time: Duration, native_time: Duration) {
        self.wasm_results.insert(test_name.to_string(), wasm_time);
        self.native_results.insert(test_name.to_string(), native_time);
    }

    fn get_speedup_ratio(&self, test_name: &str) -> Option<f64> {
        let wasm_time = self.wasm_results.get(test_name)?;
        let native_time = self.native_results.get(test_name)?;

        if native_time.as_nanos() > 0 {
            Some(wasm_time.as_nanos() as f64 / native_time.as_nanos() as f64)
        } else {
            None
        }
    }
}

// Mock WASM runtime simulation
fn simulate_wasm_overhead(native_duration: Duration, overhead_factor: f64) -> Duration {
    let additional_time = native_duration.as_nanos() as f64 * overhead_factor;
    Duration::from_nanos((native_duration.as_nanos() as f64 + additional_time) as u64)
}

fn bench_graph_reasoning_wasm_vs_native(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_vs_native_graph");

    // Test data sizes
    let test_sizes = [100, 1000, 5000];

    for &size in test_sizes.iter() {
        // Native benchmark
        group.bench_with_input(
            BenchmarkId::new("native_graph_creation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let start = Instant::now();

                    // Simulate native graph operations
                    let mut graph = graph_reasoner::KnowledgeGraph::new();
                    for i in 0..size {
                        let fact = graph_reasoner::Fact::new(
                            &format!("entity_{}", i),
                            "relates_to",
                            &format!("entity_{}", (i + 1) % size)
                        );
                        let _ = graph.add_fact(fact);
                    }

                    let duration = start.elapsed();
                    black_box((graph, duration));
                });
            }
        );

        // WASM simulation benchmark
        group.bench_with_input(
            BenchmarkId::new("wasm_graph_creation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let start = Instant::now();

                    // Simulate WASM overhead (typically 1.5-3x slower)
                    let native_start = Instant::now();
                    let mut graph = graph_reasoner::KnowledgeGraph::new();
                    for i in 0..size {
                        let fact = graph_reasoner::Fact::new(
                            &format!("entity_{}", i),
                            "relates_to",
                            &format!("entity_{}", (i + 1) % size)
                        );
                        let _ = graph.add_fact(fact);
                    }
                    let native_duration = native_start.elapsed();

                    // Simulate WASM overhead
                    let wasm_duration = simulate_wasm_overhead(native_duration, 1.8);
                    std::thread::sleep(wasm_duration - native_duration);

                    let total_duration = start.elapsed();
                    black_box((graph, total_duration));
                });
            }
        );
    }

    group.finish();
}

fn bench_text_processing_wasm_vs_native(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_vs_native_text");

    let test_texts = vec![
        "Short text for testing".to_string(),
        "This is a medium length text that contains various emotions and preferences. I really love chocolate and hate vegetables. I feel excited about this project!".to_string(),
        "Very long text with multiple sentences and complex emotional content. ".repeat(50),
    ];

    for (i, text) in test_texts.iter().enumerate() {
        // Native benchmark
        group.bench_with_input(
            BenchmarkId::new("native_text_analysis", i),
            text,
            |b, text| {
                b.iter(|| {
                    let start = Instant::now();
                    let extractor = extractors::TextExtractor::new();
                    let result = extractor.analyze_all(black_box(text));
                    let duration = start.elapsed();
                    black_box((result, duration));
                });
            }
        );

        // WASM simulation benchmark
        group.bench_with_input(
            BenchmarkId::new("wasm_text_analysis", i),
            text,
            |b, text| {
                b.iter(|| {
                    let start = Instant::now();

                    let native_start = Instant::now();
                    let extractor = extractors::TextExtractor::new();
                    let result = extractor.analyze_all(text);
                    let native_duration = native_start.elapsed();

                    // Simulate WASM overhead (text processing typically has less overhead)
                    let wasm_duration = simulate_wasm_overhead(native_duration, 1.3);
                    if wasm_duration > native_duration {
                        std::thread::sleep(wasm_duration - native_duration);
                    }

                    let total_duration = start.elapsed();
                    black_box((result, total_duration));
                });
            }
        );
    }

    group.finish();
}

fn bench_planning_wasm_vs_native(c: &mut Criterion) {
    let mut group = c.benchmark_group("wasm_vs_native_planning");

    let complexities = [20, 50, 100];

    for &complexity in complexities.iter() {
        // Generate test data
        let mut properties = HashMap::new();
        for i in 0..complexity {
            properties.insert(format!("prop_{}", i), serde_json::Value::Bool(false));
        }
        let initial_state = planner::State::new(properties.clone());

        let mut goal_conditions = HashMap::new();
        goal_conditions.insert("prop_0".to_string(), serde_json::Value::Bool(true));
        let goal = planner::Goal::new("test_goal", goal_conditions);

        let mut actions = Vec::new();
        for i in 0..complexity {
            let mut preconditions = HashMap::new();
            let mut effects = HashMap::new();
            effects.insert(format!("prop_{}", i), serde_json::Value::Bool(true));

            let action = planner::Action::new(
                &format!("action_{}", i),
                preconditions,
                effects,
                1.0,
            );
            actions.push(action);
        }

        // Native benchmark
        group.bench_with_input(
            BenchmarkId::new("native_planning", complexity),
            &(initial_state.clone(), goal.clone(), actions.clone()),
            |b, (initial_state, goal, actions)| {
                b.iter(|| {
                    let start = Instant::now();
                    let planner = planner::AStarPlanner::new();
                    let plan = planner.plan(
                        black_box(initial_state),
                        black_box(goal),
                        black_box(actions),
                        Some(1000)
                    );
                    let duration = start.elapsed();
                    black_box((plan, duration));
                });
            }
        );

        // WASM simulation benchmark
        group.bench_with_input(
            BenchmarkId::new("wasm_planning", complexity),
            &(initial_state, goal, actions),
            |b, (initial_state, goal, actions)| {
                b.iter(|| {
                    let start = Instant::now();

                    let native_start = Instant::now();
                    let planner = planner::AStarPlanner::new();
                    let plan = planner.plan(
                        initial_state,
                        goal,
                        actions,
                        Some(1000)
                    );
                    let native_duration = native_start.elapsed();

                    // Planning algorithms typically have higher WASM overhead due to memory allocation
                    let wasm_duration = simulate_wasm_overhead(native_duration, 2.2);
                    if wasm_duration > native_duration {
                        std::thread::sleep(wasm_duration - native_duration);
                    }

                    let total_duration = start.elapsed();
                    black_box((plan, total_duration));
                });
            }
        );
    }

    group.finish();
}

fn bench_memory_allocation_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_allocation_overhead");

    let allocation_sizes = [1000, 10000, 100000];

    for &size in allocation_sizes.iter() {
        // Native memory allocation
        group.bench_with_input(
            BenchmarkId::new("native_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let start = Instant::now();
                    let mut vectors: Vec<Vec<u8>> = Vec::new();

                    for i in 0..100 {
                        let mut vec = Vec::with_capacity(size);
                        for j in 0..size {
                            vec.push((i + j) as u8);
                        }
                        vectors.push(vec);
                    }

                    let duration = start.elapsed();
                    black_box((vectors, duration));
                });
            }
        );

        // WASM memory allocation simulation
        group.bench_with_input(
            BenchmarkId::new("wasm_allocation", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let start = Instant::now();

                    let native_start = Instant::now();
                    let mut vectors: Vec<Vec<u8>> = Vec::new();

                    for i in 0..100 {
                        let mut vec = Vec::with_capacity(size);
                        for j in 0..size {
                            vec.push((i + j) as u8);
                        }
                        vectors.push(vec);
                    }
                    let native_duration = native_start.elapsed();

                    // Memory allocation in WASM has significant overhead
                    let wasm_duration = simulate_wasm_overhead(native_duration, 3.0);
                    if wasm_duration > native_duration {
                        std::thread::sleep(wasm_duration - native_duration);
                    }

                    let total_duration = start.elapsed();
                    black_box((vectors, total_duration));
                });
            }
        );
    }

    group.finish();
}

fn bench_serialization_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_overhead");

    // Create test data
    let mut large_data = HashMap::new();
    for i in 0..1000 {
        large_data.insert(
            format!("key_{}", i),
            serde_json::json!({
                "id": i,
                "name": format!("item_{}", i),
                "values": vec![i, i * 2, i * 3],
                "metadata": {
                    "created": "2023-01-01",
                    "active": i % 2 == 0
                }
            })
        );
    }

    // Native serialization
    group.bench_function("native_serialization", |b| {
        b.iter(|| {
            let start = Instant::now();
            let serialized = serde_json::to_string(&large_data);
            let duration = start.elapsed();
            black_box((serialized, duration));
        });
    });

    // WASM serialization simulation
    group.bench_function("wasm_serialization", |b| {
        b.iter(|| {
            let start = Instant::now();

            let native_start = Instant::now();
            let serialized = serde_json::to_string(&large_data);
            let native_duration = native_start.elapsed();

            // Serialization overhead in WASM
            let wasm_duration = simulate_wasm_overhead(native_duration, 1.6);
            if wasm_duration > native_duration {
                std::thread::sleep(wasm_duration - native_duration);
            }

            let total_duration = start.elapsed();
            black_box((serialized, total_duration));
        });
    });

    group.finish();
}

fn bench_startup_time_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup_time");

    // Native startup simulation
    group.bench_function("native_startup", |b| {
        b.iter(|| {
            let start = Instant::now();

            // Simulate native component initialization
            let graph_reasoner = graph_reasoner::GraphReasoner::new();
            let text_extractor = extractors::TextExtractor::new();
            let planner = planner::AStarPlanner::new();

            let duration = start.elapsed();
            black_box((graph_reasoner, text_extractor, planner, duration));
        });
    });

    // WASM startup simulation (typically much slower)
    group.bench_function("wasm_startup", |b| {
        b.iter(|| {
            let start = Instant::now();

            // Simulate WASM module loading and initialization overhead
            std::thread::sleep(Duration::from_millis(50)); // WASM module loading

            let native_start = Instant::now();
            let graph_reasoner = graph_reasoner::GraphReasoner::new();
            let text_extractor = extractors::TextExtractor::new();
            let planner = planner::AStarPlanner::new();
            let native_duration = native_start.elapsed();

            // Additional WASM initialization overhead
            let wasm_duration = simulate_wasm_overhead(native_duration, 2.5);
            if wasm_duration > native_duration {
                std::thread::sleep(wasm_duration - native_duration);
            }

            let total_duration = start.elapsed();
            black_box((graph_reasoner, text_extractor, planner, total_duration));
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_reasoning_wasm_vs_native,
    bench_text_processing_wasm_vs_native,
    bench_planning_wasm_vs_native,
    bench_memory_allocation_overhead,
    bench_serialization_overhead,
    bench_startup_time_comparison
);

criterion_main!(benches);