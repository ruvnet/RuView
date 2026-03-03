//! Comprehensive benchmarks for Lean Agentic Learning System
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstream::{
    LeanAgenticSystem, LeanAgenticConfig, AgentContext,
    FormalReasoner, AgenticLoop, KnowledgeGraph, StreamLearner,
    Action, Entity, EntityType,
    TemporalComparator, Sequence, ComparisonAlgorithm,
    RealtimeScheduler, SchedulingPolicy, Priority,
    AttractorAnalyzer, BehaviorAttractorAnalyzer, Trajectory,
    TemporalNeuralSolver, TemporalFormula, TemporalTrace, TemporalState,
    MetaLearner, MetaLevel,
};
use std::collections::HashMap;
use tokio::runtime::Runtime;

fn benchmark_formal_reasoning(c: &mut Criterion) {
    let mut group = c.benchmark_group("formal_reasoning");

    let rt = Runtime::new().unwrap();

    // Benchmark action verification
    group.bench_function("verify_action", |b| {
        b.iter(|| {
            rt.block_on(async {
                let reasoner = FormalReasoner::new();
                let action = Action {
                    action_type: "test".to_string(),
                    description: "Test action".to_string(),
                    parameters: HashMap::new(),
                    tool_calls: vec![],
                    expected_outcome: Some("success".to_string()),
                    expected_reward: 0.8,
                };
                let context = AgentContext::new("test_session".to_string());

                black_box(reasoner.verify_action(&action, &context).await.unwrap())
            })
        });
    });

    // Benchmark theorem proving
    group.bench_function("prove_theorem", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut reasoner = FormalReasoner::new();
                black_box(
                    reasoner.prove_theorem(
                        "Q".to_string(),
                        vec!["P".to_string(), "P -> Q".to_string()],
                    ).await
                )
            })
        });
    });

    group.finish();
}

fn benchmark_agentic_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("agentic_loop");

    let rt = Runtime::new().unwrap();
    let config = LeanAgenticConfig::default();

    // Benchmark planning phase
    group.bench_function("plan", |b| {
        b.iter(|| {
            rt.block_on(async {
                let agent = AgenticLoop::new(config.clone());
                let context = AgentContext::new("test_session".to_string());

                black_box(agent.plan(&context, "What is the weather?").await.unwrap())
            })
        });
    });

    // Benchmark action selection
    group.bench_function("select_and_execute", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut agent = AgenticLoop::new(config.clone());
                let context = AgentContext::new("test_session".to_string());
                let plan = agent.plan(&context, "test input").await.unwrap();
                let action = agent.select_action(&plan).await.unwrap();

                black_box(agent.execute(&action).await.unwrap())
            })
        });
    });

    // Benchmark learning update
    group.bench_function("learn", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut agent = AgenticLoop::new(config.clone());
                let context = AgentContext::new("test_session".to_string());
                let plan = agent.plan(&context, "test").await.unwrap();
                let action = agent.select_action(&plan).await.unwrap();
                let observation = agent.execute(&action).await.unwrap();
                let reward = agent.compute_reward(&observation).await.unwrap();

                let signal = midstream::LearningSignal {
                    action,
                    observation,
                    reward,
                };

                black_box(agent.learn(signal).await.unwrap())
            })
        });
    });

    group.finish();
}

fn benchmark_knowledge_graph(c: &mut Criterion) {
    let mut group = c.benchmark_group("knowledge_graph");

    let rt = Runtime::new().unwrap();

    // Benchmark entity extraction
    group.bench_function("extract_entities", |b| {
        b.iter(|| {
            rt.block_on(async {
                let kg = KnowledgeGraph::new();
                let text = "Alice works at Google in California with Bob and Charlie";

                black_box(kg.extract_entities(text).await.unwrap())
            })
        });
    });

    // Benchmark graph updates
    group.bench_function("update_graph", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut kg = KnowledgeGraph::new();
                let entities = vec![
                    Entity {
                        id: "e1".to_string(),
                        name: "Alice".to_string(),
                        entity_type: EntityType::Person,
                        attributes: HashMap::new(),
                        confidence: 0.9,
                    },
                    Entity {
                        id: "e2".to_string(),
                        name: "Google".to_string(),
                        entity_type: EntityType::Organization,
                        attributes: HashMap::new(),
                        confidence: 0.95,
                    },
                ];

                black_box(kg.update(entities).await.unwrap())
            })
        });
    });

    // Benchmark relation finding
    group.bench_function("find_related", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut kg = KnowledgeGraph::new();

                // Setup
                kg.add_relation(midstream::Relation {
                    id: "r1".to_string(),
                    subject: "alice".to_string(),
                    predicate: "works_at".to_string(),
                    object: "google".to_string(),
                    confidence: 0.9,
                    source: "text".to_string(),
                });

                black_box(kg.find_related("alice", 2))
            })
        });
    });

    group.finish();
}

fn benchmark_stream_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("stream_learning");

    let rt = Runtime::new().unwrap();

    // Benchmark online learning update
    group.bench_function("online_update", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut learner = StreamLearner::new(0.01);
                let action = Action {
                    action_type: "test".to_string(),
                    description: "Test action".to_string(),
                    parameters: HashMap::new(),
                    tool_calls: vec![],
                    expected_outcome: None,
                    expected_reward: 0.5,
                };

                black_box(learner.update(&action, 1.0, "test context").await.unwrap())
            })
        });
    });

    // Benchmark reward prediction
    group.bench_function("predict_reward", |b| {
        b.iter(|| {
            rt.block_on(async {
                let learner = StreamLearner::new(0.01);
                let action = Action {
                    action_type: "test".to_string(),
                    description: "Test action".to_string(),
                    parameters: HashMap::new(),
                    tool_calls: vec![],
                    expected_outcome: None,
                    expected_reward: 0.5,
                };

                black_box(learner.predict_reward(&action, "test context").await)
            })
        });
    });

    group.finish();
}

fn benchmark_end_to_end(c: &mut Criterion) {
    let mut group = c.benchmark_group("end_to_end");
    group.sample_size(50); // Reduce sample size for slower benchmarks

    let rt = Runtime::new().unwrap();

    // Benchmark full processing pipeline
    for size in [10, 50, 100, 500].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                rt.block_on(async {
                    let config = LeanAgenticConfig::default();
                    let system = LeanAgenticSystem::new(config);
                    let mut context = AgentContext::new("bench_session".to_string());

                    for i in 0..size {
                        let chunk = format!("Message {} with some content", i);
                        black_box(
                            system.process_stream_chunk(&chunk, context.clone()).await.unwrap()
                        );
                        context.add_message(chunk);
                    }
                })
            });
        });
    }

    group.finish();
}

fn benchmark_concurrent_sessions(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_sessions");
    group.sample_size(20);

    let rt = Runtime::new().unwrap();

    // Benchmark multiple concurrent sessions
    for num_sessions in [1, 10, 50, 100].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(num_sessions),
            num_sessions,
            |b, &num_sessions| {
                b.iter(|| {
                    rt.block_on(async {
                        let config = LeanAgenticConfig::default();
                        let system = LeanAgenticSystem::new(config);

                        let mut handles = vec![];

                        for i in 0..num_sessions {
                            let sys = &system;
                            let handle = tokio::spawn(async move {
                                let context = AgentContext::new(format!("session_{}", i));
                                sys.process_stream_chunk("test message", context)
                                    .await
                                    .unwrap()
                            });
                            handles.push(handle);
                        }

                        for handle in handles {
                            black_box(handle.await.unwrap());
                        }
                    })
                });
            },
        );
    }

    group.finish();
}

fn benchmark_temporal_comparison(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_comparison");

    // Benchmark DTW with different sequence sizes
    for size in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("dtw", size),
            size,
            |b, &size| {
                let mut comparator = TemporalComparator::<i32>::new();
                let seq1: Vec<i32> = (0..size).collect();
                let seq2: Vec<i32> = (0..size).map(|x| x + (x % 3)).collect();

                b.iter(|| {
                    black_box(comparator.compare(&seq1, &seq2, ComparisonAlgorithm::DTW))
                });
            },
        );
    }

    // Benchmark LCS
    for size in [10, 50, 100, 200].iter() {
        group.bench_with_input(
            BenchmarkId::new("lcs", size),
            size,
            |b, &size| {
                let mut comparator = TemporalComparator::<i32>::new();
                let seq1: Vec<i32> = (0..size).collect();
                let seq2: Vec<i32> = (0..size).map(|x| x + (x % 2)).collect();

                b.iter(|| {
                    black_box(comparator.compare(&seq1, &seq2, ComparisonAlgorithm::LCS))
                });
            },
        );
    }

    // Benchmark edit distance
    group.bench_function("edit_distance", |b| {
        let mut comparator = TemporalComparator::<char>::new();
        let seq1: Vec<char> = "kitten".chars().collect();
        let seq2: Vec<char> = "sitting".chars().collect();

        b.iter(|| {
            black_box(comparator.compare(&seq1, &seq2, ComparisonAlgorithm::EditDistance))
        });
    });

    // Benchmark pattern detection
    group.bench_function("pattern_detection", |b| {
        let comparator = TemporalComparator::<i32>::new();
        let sequence: Vec<i32> = (0..1000).map(|x| x % 10).collect();
        let pattern = vec![1, 2, 3];

        b.iter(|| {
            black_box(comparator.detect_pattern(&sequence, &pattern))
        });
    });

    // Benchmark find similar with cache
    group.bench_function("find_similar", |b| {
        let mut comparator = TemporalComparator::<i32>::new();

        // Add many sequences
        for i in 0..100 {
            comparator.add_sequence(Sequence {
                data: (0..50).map(|x| x + i).collect(),
                timestamp: i as i64,
                id: format!("seq_{}", i),
            });
        }

        let query: Vec<i32> = (0..50).collect();

        b.iter(|| {
            black_box(comparator.find_similar(&query, 0.7, ComparisonAlgorithm::LCS))
        });
    });

    group.finish();
}

fn benchmark_scheduler(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler");

    let rt = Runtime::new().unwrap();

    // Benchmark task scheduling
    group.bench_function("schedule_task", |b| {
        b.iter(|| {
            rt.block_on(async {
                let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);
                let action = Action {
                    action_type: "test".to_string(),
                    description: "Test task".to_string(),
                    parameters: HashMap::new(),
                    tool_calls: vec![],
                    expected_outcome: None,
                    expected_reward: 0.8,
                };

                black_box(
                    scheduler.schedule(
                        action,
                        Priority::Medium,
                        std::time::Duration::from_secs(1),
                        std::time::Duration::from_millis(10),
                    ).await
                )
            })
        });
    });

    // Benchmark task retrieval with EDF
    group.bench_function("next_task_edf", |b| {
        b.iter(|| {
            rt.block_on(async {
                let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

                // Schedule multiple tasks
                for i in 0..10 {
                    let action = Action {
                        action_type: format!("task_{}", i),
                        description: format!("Task {}", i),
                        parameters: HashMap::new(),
                        tool_calls: vec![],
                        expected_outcome: None,
                        expected_reward: 0.8,
                    };

                    scheduler.schedule(
                        action,
                        Priority::Medium,
                        std::time::Duration::from_millis(100 + i * 10),
                        std::time::Duration::from_millis(10),
                    ).await;
                }

                black_box(scheduler.next_task().await)
            })
        });
    });

    // Benchmark priority scheduling
    group.bench_function("next_task_priority", |b| {
        b.iter(|| {
            rt.block_on(async {
                let scheduler = RealtimeScheduler::new(SchedulingPolicy::FixedPriority);

                // Schedule tasks with different priorities
                for priority in &[Priority::Low, Priority::Medium, Priority::High, Priority::Critical] {
                    let action = Action {
                        action_type: format!("task_{:?}", priority),
                        description: format!("Task {:?}", priority),
                        parameters: HashMap::new(),
                        tool_calls: vec![],
                        expected_outcome: None,
                        expected_reward: 0.8,
                    };

                    scheduler.schedule(
                        action,
                        *priority,
                        std::time::Duration::from_secs(1),
                        std::time::Duration::from_millis(10),
                    ).await;
                }

                black_box(scheduler.next_task().await)
            })
        });
    });

    // Benchmark scheduling with high load
    for num_tasks in [10, 50, 100, 500].iter() {
        group.bench_with_input(
            BenchmarkId::new("high_load", num_tasks),
            num_tasks,
            |b, &num_tasks| {
                b.iter(|| {
                    rt.block_on(async {
                        let scheduler = RealtimeScheduler::new(SchedulingPolicy::EarliestDeadlineFirst);

                        for i in 0..num_tasks {
                            let action = Action {
                                action_type: format!("task_{}", i),
                                description: format!("Task {}", i),
                                parameters: HashMap::new(),
                                tool_calls: vec![],
                                expected_outcome: None,
                                expected_reward: 0.8,
                            };

                            scheduler.schedule(
                                action,
                                Priority::Medium,
                                std::time::Duration::from_millis(100),
                                std::time::Duration::from_millis(10),
                            ).await;
                        }

                        // Retrieve all tasks
                        let mut count = 0;
                        while scheduler.next_task().await.is_some() {
                            count += 1;
                        }
                        black_box(count)
                    })
                });
            },
        );
    }

    group.finish();
}

fn benchmark_attractor_analysis(c: &mut Criterion) {
    let mut group = c.benchmark_group("attractor_analysis");

    // Benchmark attractor detection with different data sizes
    for size in [100, 500, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("analyze", size),
            size,
            |b, &size| {
                let analyzer = AttractorAnalyzer::new(3, 1);

                // Generate test data (logistic map)
                let mut data = Vec::with_capacity(size);
                let mut x = 0.1;
                for _ in 0..size {
                    x = 3.7 * x * (1.0 - x);
                    data.push(x);
                }

                b.iter(|| {
                    black_box(analyzer.analyze(&data).unwrap())
                });
            },
        );
    }

    // Benchmark behavior analysis
    group.bench_function("behavior_analysis", |b| {
        let mut analyzer = BehaviorAttractorAnalyzer::new(2, 200);

        // Populate with history
        for i in 0..200 {
            analyzer.observe(
                0.5 + 0.3 * (i as f64 / 50.0).sin(),
                0.7 + 0.2 * (i as f64 / 30.0).cos(),
            );
        }

        b.iter(|| {
            black_box(analyzer.get_behavior_summary())
        });
    });

    // Benchmark trajectory prediction
    group.bench_function("predict_next", |b| {
        let analyzer = AttractorAnalyzer::new(3, 1);
        let data: Vec<f64> = (0..100).map(|i| (i as f64 * 0.1).sin()).collect();
        let trajectory = Trajectory::from_timeseries(&data, 3, 1);

        b.iter(|| {
            black_box(analyzer.predict_next(&trajectory))
        });
    });

    group.finish();
}

fn benchmark_temporal_neural(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal_neural");

    // Benchmark basic LTL verification
    group.bench_function("verify_atom", |b| {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..50 {
            let mut state = TemporalState::new(std::time::Duration::from_secs(i));
            state.set("safe".to_string(), true);
            trace.add_state(state);
        }

        let formula = TemporalFormula::atom("safe");

        b.iter(|| {
            black_box(solver.verify(&formula, &trace))
        });
    });

    // Benchmark eventually operator
    group.bench_function("verify_eventually", |b| {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..100 {
            let mut state = TemporalState::new(std::time::Duration::from_secs(i));
            state.set("goal".to_string(), i == 75);
            trace.add_state(state);
        }

        let formula = TemporalFormula::eventually(TemporalFormula::atom("goal"));

        b.iter(|| {
            black_box(solver.verify(&formula, &trace))
        });
    });

    // Benchmark globally operator
    group.bench_function("verify_globally", |b| {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..100 {
            let mut state = TemporalState::new(std::time::Duration::from_secs(i));
            state.set("invariant".to_string(), true);
            trace.add_state(state);
        }

        let formula = TemporalFormula::globally(TemporalFormula::atom("invariant"));

        b.iter(|| {
            black_box(solver.verify(&formula, &trace))
        });
    });

    // Benchmark complex formulas
    group.bench_function("verify_complex", |b| {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..50 {
            let mut state = TemporalState::new(std::time::Duration::from_secs(i));
            state.set("request".to_string(), i % 10 == 0);
            state.set("response".to_string(), i % 10 == 5);
            trace.add_state(state);
        }

        // G(request -> F response)
        let formula = TemporalFormula::globally(
            TemporalFormula::implies(
                TemporalFormula::atom("request"),
                TemporalFormula::eventually(TemporalFormula::atom("response"))
            )
        );

        b.iter(|| {
            black_box(solver.verify(&formula, &trace))
        });
    });

    // Benchmark MTL (bounded temporal)
    group.bench_function("verify_bounded", |b| {
        let mut solver = TemporalNeuralSolver::new();
        let mut trace = TemporalTrace::new();

        for i in 0..100 {
            let mut state = TemporalState::new(std::time::Duration::from_millis(i * 10));
            state.set("event".to_string(), i == 50);
            trace.add_state(state);
        }

        let formula = TemporalFormula::eventually_bounded(
            TemporalFormula::atom("event"),
            std::time::Duration::from_millis(400),
            std::time::Duration::from_millis(600),
        );

        b.iter(|| {
            black_box(solver.verify(&formula, &trace))
        });
    });

    group.finish();
}

fn benchmark_meta_learning(c: &mut Criterion) {
    let mut group = c.benchmark_group("meta_learning");

    // Benchmark learning at different meta levels
    for level in [MetaLevel::Object, MetaLevel::Meta1, MetaLevel::Meta2].iter() {
        group.bench_with_input(
            BenchmarkId::new("learn", format!("{:?}", level)),
            level,
            |b, &level| {
                let mut learner = MetaLearner::new(100);

                // Ascend to the target level
                while learner.current_level() != level {
                    let _ = learner.ascend();
                }

                b.iter(|| {
                    learner.learn("Test learning content".to_string(), 0.8);
                });
            },
        );
    }

    // Benchmark pattern detection
    group.bench_function("detect_patterns", |b| {
        b.iter(|| {
            let mut learner = MetaLearner::new(100);

            // Learn many things to trigger pattern detection
            for i in 0..20 {
                learner.learn(format!("Learning {}", i), 0.7 + (i % 3) as f64 * 0.1);
                if i % 3 == 0 {
                    let _ = learner.ascend();
                } else if i % 3 == 2 {
                    let _ = learner.descend();
                }
            }

            black_box(learner.get_summary())
        });
    });

    // Benchmark strange loop detection
    group.bench_function("detect_loops", |b| {
        b.iter(|| {
            let mut learner = MetaLearner::new(100);

            // Create oscillating pattern
            for i in 0..10 {
                learner.learn(format!("Item {}", i), 0.75);
                if i % 2 == 0 {
                    let _ = learner.ascend();
                } else {
                    let _ = learner.descend();
                }
            }

            black_box(learner.get_strange_loops())
        });
    });

    // Benchmark safety checks
    group.bench_function("safety_check", |b| {
        let learner = MetaLearner::new(100);

        b.iter(|| {
            black_box(learner.safety_check())
        });
    });

    // Benchmark meta-level transitions
    group.bench_function("level_transitions", |b| {
        b.iter(|| {
            let mut learner = MetaLearner::new(100);

            // Go up and down the hierarchy
            for _ in 0..5 {
                let _ = learner.ascend();
            }
            for _ in 0..5 {
                let _ = learner.descend();
            }

            black_box(learner.current_level())
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_formal_reasoning,
    benchmark_agentic_loop,
    benchmark_knowledge_graph,
    benchmark_stream_learning,
    benchmark_end_to_end,
    benchmark_concurrent_sessions,
    benchmark_temporal_comparison,
    benchmark_scheduler,
    benchmark_attractor_analysis,
    benchmark_temporal_neural,
    benchmark_meta_learning,
);

criterion_main!(benches);
