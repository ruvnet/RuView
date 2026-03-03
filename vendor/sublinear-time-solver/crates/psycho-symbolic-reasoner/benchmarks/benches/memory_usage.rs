use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use memory_stats::memory_stats;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use sysinfo::{System, SystemExt, ProcessExt, Pid};

#[derive(Debug, Clone)]
struct MemorySnapshot {
    timestamp: Instant,
    physical_memory: usize,
    virtual_memory: usize,
    process_memory: usize,
}

impl MemorySnapshot {
    fn take() -> Option<Self> {
        let timestamp = Instant::now();

        // Get process memory using memory_stats
        let process_memory = memory_stats()?.physical_mem;

        // Get system memory information
        let mut system = System::new_all();
        system.refresh_all();

        let current_pid = std::process::id();
        let physical_memory = system.used_memory() as usize;
        let virtual_memory = system.total_memory() as usize;

        Some(MemorySnapshot {
            timestamp,
            physical_memory,
            virtual_memory,
            process_memory,
        })
    }

    fn memory_diff(&self, other: &MemorySnapshot) -> i64 {
        self.process_memory as i64 - other.process_memory as i64
    }
}

struct MemoryProfiler {
    baseline: Option<MemorySnapshot>,
    samples: Vec<MemorySnapshot>,
}

impl MemoryProfiler {
    fn new() -> Self {
        Self {
            baseline: MemorySnapshot::take(),
            samples: Vec::new(),
        }
    }

    fn sample(&mut self) {
        if let Some(snapshot) = MemorySnapshot::take() {
            self.samples.push(snapshot);
        }
    }

    fn peak_memory_usage(&self) -> Option<usize> {
        self.samples.iter()
            .map(|s| s.process_memory)
            .max()
    }

    fn average_memory_usage(&self) -> Option<usize> {
        if self.samples.is_empty() {
            return None;
        }

        let sum: usize = self.samples.iter()
            .map(|s| s.process_memory)
            .sum();
        Some(sum / self.samples.len())
    }

    fn memory_growth(&self) -> Option<i64> {
        if let (Some(first), Some(last)) = (self.samples.first(), self.samples.last()) {
            Some(last.memory_diff(first))
        } else {
            None
        }
    }
}

fn bench_graph_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_memory_usage");

    for &size in [1000, 10000, 50000, 100000].iter() {
        group.bench_with_input(
            BenchmarkId::new("graph_growth", size),
            &size,
            |b, &size| {
                b.iter_custom(|iters| {
                    let mut total_duration = Duration::new(0, 0);
                    let mut profiler = MemoryProfiler::new();

                    for _ in 0..iters {
                        profiler.sample();
                        let start = Instant::now();

                        let mut graph = graph_reasoner::KnowledgeGraph::new();

                        // Add facts incrementally to monitor memory growth
                        for i in 0..size {
                            let fact = graph_reasoner::Fact::new(
                                &format!("entity_{}", i),
                                "relates_to",
                                &format!("entity_{}", (i + 1) % size)
                            );

                            if let Err(e) = graph.add_fact(fact) {
                                eprintln!("Error adding fact: {}", e);
                            }

                            // Sample memory every 1000 facts
                            if i % 1000 == 0 {
                                profiler.sample();
                            }
                        }

                        profiler.sample();
                        total_duration += start.elapsed();

                        // Force memory usage calculation
                        let stats = graph.get_statistics();
                        black_box((graph, stats));
                    }

                    // Report memory statistics
                    if let Some(peak) = profiler.peak_memory_usage() {
                        eprintln!("Peak memory usage for size {}: {} bytes", size, peak);
                    }
                    if let Some(growth) = profiler.memory_growth() {
                        eprintln!("Memory growth for size {}: {} bytes", size, growth);
                    }

                    total_duration
                });
            }
        );
    }

    group.finish();
}

fn bench_text_extractor_memory_leak(c: &mut Criterion) {
    let mut group = c.benchmark_group("text_extractor_memory_leak");

    let test_texts = vec![
        "Short text".to_string(),
        "Medium length text with emotional content and preferences. I love this and hate that.".to_string(),
        "Long text that repeats many times to test memory allocation patterns. ".repeat(100),
    ];

    for (i, text) in test_texts.iter().enumerate() {
        group.bench_with_input(
            BenchmarkId::new("repeated_analysis", i),
            text,
            |b, text| {
                b.iter_custom(|iters| {
                    let mut profiler = MemoryProfiler::new();
                    let start = Instant::now();

                    for iteration in 0..iters {
                        profiler.sample();

                        let extractor = extractors::TextExtractor::new();

                        // Perform multiple analyses to stress memory
                        for _ in 0..10 {
                            let sentiment = extractor.analyze_sentiment(text);
                            let preferences = extractor.extract_preferences(text);
                            let emotions = extractor.detect_emotions(text);
                            let combined = extractor.analyze_all(text);

                            black_box((sentiment, preferences, emotions, combined));
                        }

                        // Sample memory every 100 iterations
                        if iteration % 100 == 0 {
                            profiler.sample();
                        }
                    }

                    profiler.sample();

                    // Report potential memory leaks
                    if let Some(growth) = profiler.memory_growth() {
                        if growth > 1024 * 1024 { // More than 1MB growth
                            eprintln!("Potential memory leak detected: {} bytes growth", growth);
                        }
                    }

                    start.elapsed()
                });
            }
        );
    }

    group.finish();
}

fn bench_planning_memory_complexity(c: &mut Criterion) {
    let mut group = c.benchmark_group("planning_memory_complexity");

    for &complexity in [50, 100, 200, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("planning_memory", complexity),
            &complexity,
            |b, &complexity| {
                b.iter_custom(|iters| {
                    let mut profiler = MemoryProfiler::new();
                    let start = Instant::now();

                    for _ in 0..iters {
                        profiler.sample();

                        // Generate complex state
                        let mut properties = HashMap::new();
                        for i in 0..complexity {
                            properties.insert(
                                format!("property_{}", i),
                                serde_json::Value::Array(vec![
                                    serde_json::Value::Number(serde_json::Number::from(i)),
                                    serde_json::Value::String(format!("value_{}", i)),
                                ])
                            );
                        }
                        let initial_state = planner::State::new(properties);

                        // Generate goal
                        let mut goal_conditions = HashMap::new();
                        goal_conditions.insert("property_0".to_string(), serde_json::Value::Bool(true));
                        let goal = planner::Goal::new("complex_goal", goal_conditions);

                        // Generate many actions
                        let mut actions = Vec::new();
                        for i in 0..complexity * 2 {
                            let mut preconditions = HashMap::new();
                            let mut effects = HashMap::new();

                            effects.insert(
                                format!("property_{}", i % complexity),
                                serde_json::Value::Bool(true)
                            );

                            let action = planner::Action::new(
                                &format!("action_{}", i),
                                preconditions,
                                effects,
                                1.0,
                            );
                            actions.push(action);
                        }

                        profiler.sample();

                        // Run planning algorithm
                        let planner = planner::AStarPlanner::new();
                        let plan = planner.plan(
                            &initial_state,
                            &goal,
                            &actions,
                            Some(1000)
                        );

                        profiler.sample();

                        black_box((initial_state, goal, actions, plan));
                    }

                    // Check for excessive memory usage during planning
                    if let Some(peak) = profiler.peak_memory_usage() {
                        if peak > 500 * 1024 * 1024 { // More than 500MB
                            eprintln!("High memory usage detected: {} bytes", peak);
                        }
                    }

                    start.elapsed()
                });
            }
        );
    }

    group.finish();
}

fn bench_long_running_process_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("long_running_memory");

    group.bench_function("sustained_operations", |b| {
        b.iter_custom(|iters| {
            let mut profiler = MemoryProfiler::new();
            let start = Instant::now();

            // Simulate long-running process
            for iteration in 0..iters {
                profiler.sample();

                // Create and use all components
                let mut graph = graph_reasoner::KnowledgeGraph::new();
                let extractor = extractors::TextExtractor::new();
                let planner = planner::AStarPlanner::new();

                // Perform sustained operations
                for i in 0..100 {
                    // Graph operations
                    let fact = graph_reasoner::Fact::new(
                        &format!("entity_{}_{}", iteration, i),
                        "relates_to",
                        &format!("entity_{}_{}", iteration, (i + 1) % 100)
                    );
                    let _ = graph.add_fact(fact);

                    // Text processing
                    let text = format!("Processing iteration {} item {}", iteration, i);
                    let _ = extractor.analyze_all(&text);

                    // Planning
                    if i % 10 == 0 {
                        let mut properties = HashMap::new();
                        properties.insert("step".to_string(), serde_json::Value::Number(serde_json::Number::from(i)));
                        let state = planner::State::new(properties);

                        let mut goal_conditions = HashMap::new();
                        goal_conditions.insert("step".to_string(), serde_json::Value::Number(serde_json::Number::from(100)));
                        let goal = planner::Goal::new("reach_step", goal_conditions);

                        let actions = vec![
                            planner::Action::new(
                                "increment",
                                HashMap::new(),
                                {
                                    let mut effects = HashMap::new();
                                    effects.insert("step".to_string(), serde_json::Value::Number(serde_json::Number::from(i + 1)));
                                    effects
                                },
                                1.0,
                            )
                        ];

                        let _ = planner.plan(&state, &goal, &actions, Some(10));
                    }
                }

                // Sample memory periodically
                if iteration % 10 == 0 {
                    profiler.sample();
                }

                black_box((graph, extractor, planner));
            }

            profiler.sample();

            // Detect memory leaks in long-running processes
            if let Some(growth) = profiler.memory_growth() {
                let growth_per_iteration = growth as f64 / iters as f64;
                if growth_per_iteration > 1024.0 { // More than 1KB per iteration
                    eprintln!("Memory leak detected: {:.2} bytes per iteration", growth_per_iteration);
                }
            }

            start.elapsed()
        });
    });

    group.finish();
}

fn bench_concurrent_memory_access(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_memory_access");

    group.bench_function("multi_thread_memory", |b| {
        b.iter_custom(|iters| {
            let mut profiler = MemoryProfiler::new();
            let start = Instant::now();

            for _ in 0..iters {
                profiler.sample();

                let handles: Vec<_> = (0..4).map(|thread_id| {
                    std::thread::spawn(move || {
                        let mut thread_memory = Vec::new();

                        // Each thread performs memory-intensive operations
                        for i in 0..1000 {
                            let mut graph = graph_reasoner::KnowledgeGraph::new();

                            for j in 0..100 {
                                let fact = graph_reasoner::Fact::new(
                                    &format!("thread_{}_{}", thread_id, j),
                                    "processes",
                                    &format!("item_{}", i * 100 + j)
                                );
                                let _ = graph.add_fact(fact);
                            }

                            thread_memory.push(graph);
                        }

                        thread_memory
                    })
                }).collect();

                let results: Vec<_> = handles.into_iter()
                    .map(|h| h.join().unwrap())
                    .collect();

                profiler.sample();
                black_box(results);
            }

            profiler.sample();

            // Check for memory issues in concurrent access
            if let Some(peak) = profiler.peak_memory_usage() {
                eprintln!("Peak concurrent memory usage: {} bytes", peak);
            }

            start.elapsed()
        });
    });

    group.finish();
}

fn bench_garbage_collection_simulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("gc_simulation");

    group.bench_function("allocation_deallocation_cycles", |b| {
        b.iter_custom(|iters| {
            let mut profiler = MemoryProfiler::new();
            let start = Instant::now();

            for cycle in 0..iters {
                profiler.sample();

                // Allocation phase
                let mut allocated_objects = Vec::new();

                for i in 0..1000 {
                    // Allocate various objects
                    let graph = graph_reasoner::KnowledgeGraph::new();
                    let extractor = extractors::TextExtractor::new();

                    // Create some data structures
                    let mut large_map = HashMap::new();
                    for j in 0..100 {
                        large_map.insert(
                            format!("key_{}_{}", cycle, i * 100 + j),
                            vec![j; 100] // Vec with 100 elements
                        );
                    }

                    allocated_objects.push((graph, extractor, large_map));
                }

                profiler.sample();

                // Partial deallocation (simulating GC)
                let keep_count = allocated_objects.len() / 2;
                allocated_objects.truncate(keep_count);

                profiler.sample();

                // Force deallocation
                drop(allocated_objects);

                profiler.sample();
            }

            // Analyze memory patterns
            if let (Some(peak), Some(avg)) = (profiler.peak_memory_usage(), profiler.average_memory_usage()) {
                let ratio = peak as f64 / avg as f64;
                if ratio > 2.0 {
                    eprintln!("High memory fluctuation detected: peak/avg ratio = {:.2}", ratio);
                }
            }

            start.elapsed()
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_graph_memory_usage,
    bench_text_extractor_memory_leak,
    bench_planning_memory_complexity,
    bench_long_running_process_memory,
    bench_concurrent_memory_access,
    bench_garbage_collection_simulation
);

criterion_main!(benches);