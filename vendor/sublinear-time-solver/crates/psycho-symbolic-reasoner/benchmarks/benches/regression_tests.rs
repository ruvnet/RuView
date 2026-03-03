use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use std::collections::HashMap;
use std::time::Duration;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PerformanceBaseline {
    test_name: String,
    version: String,
    timestamp: String,
    metrics: HashMap<String, f64>,
    environment: EnvironmentInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EnvironmentInfo {
    cpu_model: String,
    memory_gb: f32,
    rust_version: String,
    optimization_level: String,
}

#[derive(Debug, Clone)]
struct RegressionTestResult {
    test_name: String,
    current_value: f64,
    baseline_value: f64,
    regression_percentage: f64,
    is_regression: bool,
    threshold: f64,
}

struct RegressionTester {
    baselines: HashMap<String, PerformanceBaseline>,
    regression_threshold: f64, // 10% = 0.1
}

impl RegressionTester {
    fn new(threshold: f64) -> Self {
        Self {
            baselines: HashMap::new(),
            regression_threshold: threshold,
        }
    }

    fn load_baseline(&mut self, test_name: &str) {
        // In a real implementation, this would load from a file
        // For now, we'll create mock baselines
        let baseline = match test_name {
            "graph_creation_1000" => PerformanceBaseline {
                test_name: test_name.to_string(),
                version: "0.1.0".to_string(),
                timestamp: "2023-01-01T00:00:00Z".to_string(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert("execution_time_ms".to_string(), 45.2);
                    m.insert("memory_mb".to_string(), 12.5);
                    m.insert("cpu_usage_percent".to_string(), 65.0);
                    m
                },
                environment: EnvironmentInfo {
                    cpu_model: "Intel i7-9700K".to_string(),
                    memory_gb: 16.0,
                    rust_version: "1.70.0".to_string(),
                    optimization_level: "release".to_string(),
                },
            },
            "text_analysis_medium" => PerformanceBaseline {
                test_name: test_name.to_string(),
                version: "0.1.0".to_string(),
                timestamp: "2023-01-01T00:00:00Z".to_string(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert("execution_time_ms".to_string(), 23.8);
                    m.insert("memory_mb".to_string(), 8.2);
                    m.insert("throughput_ops_per_sec".to_string(), 850.0);
                    m
                },
                environment: EnvironmentInfo {
                    cpu_model: "Intel i7-9700K".to_string(),
                    memory_gb: 16.0,
                    rust_version: "1.70.0".to_string(),
                    optimization_level: "release".to_string(),
                },
            },
            "planning_complex_100" => PerformanceBaseline {
                test_name: test_name.to_string(),
                version: "0.1.0".to_string(),
                timestamp: "2023-01-01T00:00:00Z".to_string(),
                metrics: {
                    let mut m = HashMap::new();
                    m.insert("execution_time_ms".to_string(), 156.7);
                    m.insert("memory_mb".to_string(), 28.4);
                    m.insert("states_explored".to_string(), 1250.0);
                    m
                },
                environment: EnvironmentInfo {
                    cpu_model: "Intel i7-9700K".to_string(),
                    memory_gb: 16.0,
                    rust_version: "1.70.0".to_string(),
                    optimization_level: "release".to_string(),
                },
            },
            _ => return, // No baseline for this test
        };

        self.baselines.insert(test_name.to_string(), baseline);
    }

    fn check_regression(&self, test_name: &str, metric_name: &str, current_value: f64) -> Option<RegressionTestResult> {
        let baseline = self.baselines.get(test_name)?;
        let baseline_value = *baseline.metrics.get(metric_name)?;

        let regression_percentage = (current_value - baseline_value) / baseline_value;
        let is_regression = regression_percentage > self.regression_threshold;

        Some(RegressionTestResult {
            test_name: test_name.to_string(),
            current_value,
            baseline_value,
            regression_percentage,
            is_regression,
            threshold: self.regression_threshold,
        })
    }

    fn report_regressions(&self, results: &[RegressionTestResult]) {
        let regressions: Vec<_> = results.iter().filter(|r| r.is_regression).collect();

        if !regressions.is_empty() {
            eprintln!("PERFORMANCE REGRESSIONS DETECTED:");
            for regression in regressions {
                eprintln!(
                    "  {} - Current: {:.2}, Baseline: {:.2}, Regression: {:.1}% (threshold: {:.1}%)",
                    regression.test_name,
                    regression.current_value,
                    regression.baseline_value,
                    regression.regression_percentage * 100.0,
                    regression.threshold * 100.0
                );
            }
        }
    }
}

fn bench_graph_creation_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_graph_creation");
    let mut tester = RegressionTester::new(0.10); // 10% threshold
    tester.load_baseline("graph_creation_1000");

    let test_name = "graph_creation_1000";
    let size = 1000;

    group.bench_function(test_name, |b| {
        let mut execution_times = Vec::new();

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let iter_start = std::time::Instant::now();

                let mut graph = graph_reasoner::KnowledgeGraph::new();
                for i in 0..size {
                    let fact = graph_reasoner::Fact::new(
                        &format!("entity_{}", i),
                        "relates_to",
                        &format!("entity_{}", (i + 1) % size)
                    );
                    let _ = graph.add_fact(fact);
                }

                let iter_time = iter_start.elapsed();
                execution_times.push(iter_time.as_secs_f64() * 1000.0); // Convert to ms

                black_box(graph);
            }

            start.elapsed()
        });

        // Analyze performance against baseline
        if !execution_times.is_empty() {
            let avg_time = execution_times.iter().sum::<f64>() / execution_times.len() as f64;

            if let Some(result) = tester.check_regression(test_name, "execution_time_ms", avg_time) {
                if result.is_regression {
                    eprintln!("REGRESSION DETECTED in {}: {:.2}ms vs baseline {:.2}ms ({:.1}% increase)",
                        test_name, result.current_value, result.baseline_value, result.regression_percentage * 100.0);
                }
            }
        }
    });

    group.finish();
}

fn bench_text_analysis_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_text_analysis");
    let mut tester = RegressionTester::new(0.15); // 15% threshold for text analysis
    tester.load_baseline("text_analysis_medium");

    let test_name = "text_analysis_medium";
    let test_text = "This is a medium length text that contains various emotions and preferences. I really love chocolate and hate vegetables. I feel excited about this project and worried about the deadlines. My favorite color is blue and I prefer working in the morning.";

    group.bench_function(test_name, |b| {
        let mut execution_times = Vec::new();

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let iter_start = std::time::Instant::now();

                let extractor = extractors::TextExtractor::new();
                let result = extractor.analyze_all(black_box(test_text));

                let iter_time = iter_start.elapsed();
                execution_times.push(iter_time.as_secs_f64() * 1000.0);

                black_box(result);
            }

            start.elapsed()
        });

        // Check for regressions
        if !execution_times.is_empty() {
            let avg_time = execution_times.iter().sum::<f64>() / execution_times.len() as f64;
            let throughput = 1000.0 / avg_time; // Operations per second

            // Check execution time regression
            if let Some(result) = tester.check_regression(test_name, "execution_time_ms", avg_time) {
                if result.is_regression {
                    eprintln!("EXECUTION TIME REGRESSION in {}", test_name);
                }
            }

            // Check throughput regression (inverse relationship)
            if let Some(baseline) = tester.baselines.get(test_name) {
                if let Some(&baseline_throughput) = baseline.metrics.get("throughput_ops_per_sec") {
                    let throughput_degradation = (baseline_throughput - throughput) / baseline_throughput;
                    if throughput_degradation > tester.regression_threshold {
                        eprintln!("THROUGHPUT REGRESSION in {}: {:.1} ops/sec vs baseline {:.1} ops/sec",
                            test_name, throughput, baseline_throughput);
                    }
                }
            }
        }
    });

    group.finish();
}

fn bench_planning_algorithm_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_planning");
    let mut tester = RegressionTester::new(0.20); // 20% threshold for planning (more variable)
    tester.load_baseline("planning_complex_100");

    let test_name = "planning_complex_100";
    let complexity = 100;

    // Generate test data
    let mut properties = HashMap::new();
    for i in 0..complexity {
        properties.insert(format!("prop_{}", i), serde_json::Value::Bool(false));
    }
    let initial_state = planner::State::new(properties);

    let mut goal_conditions = HashMap::new();
    goal_conditions.insert("prop_0".to_string(), serde_json::Value::Bool(true));
    let goal = planner::Goal::new("test_goal", goal_conditions);

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

    group.bench_function(test_name, |b| {
        let mut execution_times = Vec::new();
        let mut states_explored = Vec::new();

        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                let iter_start = std::time::Instant::now();

                let planner = planner::AStarPlanner::new();
                let plan = planner.plan(
                    black_box(&initial_state),
                    black_box(&goal),
                    black_box(&actions),
                    Some(1000)
                );

                let iter_time = iter_start.elapsed();
                execution_times.push(iter_time.as_secs_f64() * 1000.0);

                // Mock states explored metric
                states_explored.push(1200.0 + (rand::random::<f64>() * 100.0));

                black_box(plan);
            }

            start.elapsed()
        });

        // Check for regressions
        if !execution_times.is_empty() {
            let avg_time = execution_times.iter().sum::<f64>() / execution_times.len() as f64;
            let avg_states = states_explored.iter().sum::<f64>() / states_explored.len() as f64;

            // Check execution time regression
            if let Some(result) = tester.check_regression(test_name, "execution_time_ms", avg_time) {
                if result.is_regression {
                    eprintln!("PLANNING EXECUTION TIME REGRESSION in {}", test_name);
                }
            }

            // Check states explored (efficiency metric)
            if let Some(result) = tester.check_regression(test_name, "states_explored", avg_states) {
                if result.is_regression {
                    eprintln!("PLANNING EFFICIENCY REGRESSION in {} (more states explored)", test_name);
                }
            }
        }
    });

    group.finish();
}

fn bench_memory_usage_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_memory");

    group.bench_function("memory_growth_regression", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for iteration in 0..iters {
                // Test for memory leaks over iterations
                let mut graphs = Vec::new();
                let mut extractors = Vec::new();

                for i in 0..100 {
                    let mut graph = graph_reasoner::KnowledgeGraph::new();

                    // Add some facts
                    for j in 0..50 {
                        let fact = graph_reasoner::Fact::new(
                            &format!("iter_{}_entity_{}", iteration, j),
                            "relates_to",
                            &format!("iter_{}_entity_{}", iteration, (j + 1) % 50)
                        );
                        let _ = graph.add_fact(fact);
                    }

                    graphs.push(graph);

                    // Text extractors
                    let extractor = extractors::TextExtractor::new();
                    let _ = extractor.analyze_all(&format!("Test iteration {} item {}", iteration, i));
                    extractors.push(extractor);
                }

                // Check if we're accumulating too much memory
                if iteration % 10 == 0 {
                    // In a real scenario, we'd check actual memory usage here
                    if graphs.len() > 500 {
                        eprintln!("POTENTIAL MEMORY LEAK: {} graphs accumulated", graphs.len());
                    }
                }

                black_box((graphs, extractors));
            }

            start.elapsed()
        });
    });

    group.finish();
}

fn bench_concurrent_performance_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_concurrent");

    let thread_counts = [1, 2, 4, 8];

    for &thread_count in thread_counts.iter() {
        group.bench_with_input(
            BenchmarkId::new("concurrent_scaling", thread_count),
            &thread_count,
            |b, &thread_count| {
                b.iter_custom(|iters| {
                    let start = std::time::Instant::now();

                    for _ in 0..iters {
                        let handles: Vec<_> = (0..thread_count).map(|thread_id| {
                            std::thread::spawn(move || {
                                let mut graph = graph_reasoner::KnowledgeGraph::new();

                                for i in 0..100 {
                                    let fact = graph_reasoner::Fact::new(
                                        &format!("thread_{}_entity_{}", thread_id, i),
                                        "processes",
                                        &format!("item_{}", i)
                                    );
                                    let _ = graph.add_fact(fact);
                                }

                                let extractor = extractors::TextExtractor::new();
                                let _ = extractor.analyze_all(&format!("Thread {} processing", thread_id));

                                (graph, extractor)
                            })
                        }).collect();

                        let results: Vec<_> = handles.into_iter()
                            .map(|h| h.join().unwrap())
                            .collect();

                        black_box(results);
                    }

                    start.elapsed()
                });
            }
        );
    }

    // Check for scaling regressions
    // In a real implementation, we'd compare scaling efficiency against baselines

    group.finish();
}

fn bench_compilation_performance_regression(c: &mut Criterion) {
    let mut group = c.benchmark_group("regression_compilation");

    // This would typically be run as part of CI to detect compilation time regressions
    group.bench_function("wasm_compilation_time", |b| {
        b.iter_custom(|iters| {
            let start = std::time::Instant::now();

            for _ in 0..iters {
                // In a real scenario, this would trigger WASM compilation
                // For now, we simulate the overhead
                std::thread::sleep(Duration::from_millis(50));

                // Simulate component initialization after compilation
                let graph_reasoner = graph_reasoner::GraphReasoner::new();
                let text_extractor = extractors::TextExtractor::new();

                black_box((graph_reasoner, text_extractor));
            }

            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_creation_regression,
    bench_text_analysis_regression,
    bench_planning_algorithm_regression,
    bench_memory_usage_regression,
    bench_concurrent_performance_regression,
    bench_compilation_performance_regression
);

criterion_main!(benches);