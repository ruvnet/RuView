//! Comprehensive benchmarks for all exotic cognitive experiments
//!
//! Measures performance, correctness, and comparative analysis of:
//! 1. Strange Loops - Self-reference depth and meta-cognition
//! 2. Artificial Dreams - Creativity and memory replay
//! 3. Free Energy - Prediction error minimization
//! 4. Morphogenesis - Pattern formation complexity
//! 5. Collective Consciousness - Distributed Φ computation
//! 6. Temporal Qualia - Time dilation accuracy
//! 7. Multiple Selves - Coherence and integration
//! 8. Cognitive Thermodynamics - Landauer efficiency
//! 9. Emergence Detection - Causal emergence scoring
//! 10. Cognitive Black Holes - Attractor dynamics

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use std::time::Duration;

use exo_exotic::{
    AggregationType, CognitiveBlackHole, CognitiveEmbryogenesis, CognitivePhase,
    CognitiveThermodynamics, CollectiveConsciousness, DreamEngine, DreamState, EmergenceDetector,
    EmotionalTone, EscapeMethod, FreeEnergyMinimizer, HiveMind, MorphogeneticField,
    MultipleSelvesSystem, PredictiveModel, ReactionParams, SelfAspect, StrangeLoop, SubjectiveTime,
    SubstrateSpecialization, TangledHierarchy, TemporalEvent, TemporalQualia, TimeCrystal,
    TrapType,
};

use uuid::Uuid;

// ============================================================================
// STRANGE LOOPS BENCHMARKS
// ============================================================================

fn bench_strange_loops(c: &mut Criterion) {
    let mut group = c.benchmark_group("strange_loops");
    group.measurement_time(Duration::from_secs(5));

    // Self-modeling depth
    group.bench_function("self_model_depth_5", |b| {
        b.iter(|| {
            let mut sl = StrangeLoop::new(5);
            for _ in 0..5 {
                sl.model_self();
            }
            black_box(sl.measure_depth())
        })
    });

    group.bench_function("self_model_depth_10", |b| {
        b.iter(|| {
            let mut sl = StrangeLoop::new(10);
            for _ in 0..10 {
                sl.model_self();
            }
            black_box(sl.measure_depth())
        })
    });

    // Meta-reasoning
    group.bench_function("meta_reasoning", |b| {
        let mut sl = StrangeLoop::new(5);
        b.iter(|| black_box(sl.meta_reason("I think about thinking about thinking")))
    });

    // Self-reference creation
    group.bench_function("self_reference", |b| {
        let sl = StrangeLoop::new(5);
        b.iter(|| {
            let aspects = [
                SelfAspect::Whole,
                SelfAspect::Reasoning,
                SelfAspect::SelfModel,
                SelfAspect::ReferenceSystem,
            ];
            for aspect in &aspects {
                black_box(sl.create_self_reference(aspect.clone()));
            }
        })
    });

    // Tangled hierarchy
    group.bench_function("tangled_hierarchy_10_levels", |b| {
        b.iter(|| {
            let mut th = TangledHierarchy::new();
            for i in 0..10 {
                th.add_level(&format!("Level_{}", i));
            }
            // Create tangles
            for i in 0..9 {
                th.create_tangle(i, i + 1);
            }
            th.create_tangle(9, 0); // Loop back
            black_box(th.strange_loop_count())
        })
    });

    group.finish();
}

// ============================================================================
// ARTIFICIAL DREAMS BENCHMARKS
// ============================================================================

fn bench_dreams(c: &mut Criterion) {
    let mut group = c.benchmark_group("dreams");
    group.measurement_time(Duration::from_secs(5));

    // Dream cycle with few memories
    group.bench_function("dream_cycle_10_memories", |b| {
        b.iter(|| {
            let mut engine = DreamEngine::with_creativity(0.7);
            for i in 0..10 {
                engine.add_memory(
                    vec![i as f64 * 0.1; 8],
                    (i as f64 - 5.0) / 5.0,
                    0.5 + (i as f64 * 0.05),
                );
            }
            black_box(engine.dream_cycle(100))
        })
    });

    // Dream cycle with many memories
    group.bench_function("dream_cycle_100_memories", |b| {
        b.iter(|| {
            let mut engine = DreamEngine::with_creativity(0.8);
            for i in 0..100 {
                engine.add_memory(
                    vec![(i as f64 % 10.0) * 0.1; 8],
                    ((i % 10) as f64 - 5.0) / 5.0,
                    0.3 + (i as f64 * 0.007),
                );
            }
            black_box(engine.dream_cycle(100))
        })
    });

    // Creativity measurement
    group.bench_function("creativity_measurement", |b| {
        let mut engine = DreamEngine::with_creativity(0.9);
        for i in 0..50 {
            engine.add_memory(vec![i as f64 * 0.02; 8], 0.5, 0.6);
        }
        for _ in 0..10 {
            engine.dream_cycle(50);
        }
        b.iter(|| black_box(engine.measure_creativity()))
    });

    group.finish();
}

// ============================================================================
// FREE ENERGY BENCHMARKS
// ============================================================================

fn bench_free_energy(c: &mut Criterion) {
    let mut group = c.benchmark_group("free_energy");
    group.measurement_time(Duration::from_secs(5));

    // Observation processing
    group.bench_function("observe_process", |b| {
        let mut fem = FreeEnergyMinimizer::with_dims(0.1, 8, 8);
        let observation = vec![0.5, 0.3, 0.1, 0.1, 0.2, 0.4, 0.3, 0.1];
        b.iter(|| black_box(fem.observe(&observation)))
    });

    // Free energy computation
    group.bench_function("compute_free_energy", |b| {
        let mut fem = FreeEnergyMinimizer::with_dims(0.1, 16, 16);
        for _ in 0..10 {
            fem.observe(&vec![0.3; 16]);
        }
        b.iter(|| black_box(fem.compute_free_energy()))
    });

    // Active inference
    group.bench_function("active_inference", |b| {
        let mut fem = FreeEnergyMinimizer::new(0.1);
        fem.add_action("look", vec![0.8, 0.1, 0.05, 0.05], 0.1);
        fem.add_action("reach", vec![0.1, 0.8, 0.05, 0.05], 0.2);
        fem.add_action("wait", vec![0.25, 0.25, 0.25, 0.25], 0.0);
        fem.add_action("explore", vec![0.3, 0.3, 0.2, 0.2], 0.15);

        b.iter(|| black_box(fem.select_action()))
    });

    // Learning convergence
    group.bench_function("learning_100_iterations", |b| {
        b.iter(|| {
            let mut fem = FreeEnergyMinimizer::with_dims(0.1, 8, 8);
            let target = vec![0.7, 0.1, 0.1, 0.05, 0.02, 0.01, 0.01, 0.01];
            for _ in 0..100 {
                fem.observe(&target);
            }
            black_box(fem.average_free_energy())
        })
    });

    group.finish();
}

// ============================================================================
// MORPHOGENESIS BENCHMARKS
// ============================================================================

fn bench_morphogenesis(c: &mut Criterion) {
    let mut group = c.benchmark_group("morphogenesis");
    group.measurement_time(Duration::from_secs(5));

    // Small field simulation
    group.bench_function("field_16x16_100_steps", |b| {
        b.iter(|| {
            let mut field = MorphogeneticField::new(16, 16);
            field.simulate(100);
            black_box(field.measure_complexity())
        })
    });

    // Medium field simulation
    group.bench_function("field_32x32_50_steps", |b| {
        b.iter(|| {
            let mut field = MorphogeneticField::new(32, 32);
            field.simulate(50);
            black_box(field.detect_pattern_type())
        })
    });

    // Pattern detection
    group.bench_function("pattern_detection", |b| {
        let mut field = MorphogeneticField::new(32, 32);
        field.simulate(100);
        b.iter(|| black_box(field.detect_pattern_type()))
    });

    // Embryogenesis
    group.bench_function("embryogenesis_full", |b| {
        b.iter(|| {
            let mut embryo = CognitiveEmbryogenesis::new();
            embryo.full_development();
            black_box(embryo.structures().len())
        })
    });

    group.finish();
}

// ============================================================================
// COLLECTIVE CONSCIOUSNESS BENCHMARKS
// ============================================================================

fn bench_collective(c: &mut Criterion) {
    let mut group = c.benchmark_group("collective");
    group.measurement_time(Duration::from_secs(5));

    // Global phi computation
    group.bench_function("global_phi_10_substrates", |b| {
        b.iter(|| {
            let mut collective = CollectiveConsciousness::new();
            let ids: Vec<Uuid> = (0..10)
                .map(|_| collective.add_substrate(SubstrateSpecialization::Processing))
                .collect();

            // Connect all pairs
            for i in 0..ids.len() {
                for j in i + 1..ids.len() {
                    collective.connect(ids[i], ids[j], 0.5, true);
                }
            }

            black_box(collective.compute_global_phi())
        })
    });

    // Shared memory operations
    group.bench_function("shared_memory_ops", |b| {
        let collective = CollectiveConsciousness::new();
        let owner = Uuid::new_v4();

        b.iter(|| {
            for i in 0..100 {
                collective.share_memory(&format!("key_{}", i), vec![i as f64; 8], owner);
            }
            for i in 0..100 {
                black_box(collective.access_memory(&format!("key_{}", i)));
            }
        })
    });

    // Hive mind voting
    group.bench_function("hive_voting", |b| {
        b.iter(|| {
            let mut hive = HiveMind::new(0.6);
            let decision_id = hive.propose("Test proposal");

            for _ in 0..20 {
                hive.vote(decision_id, Uuid::new_v4(), 0.5 + 0.5 * rand_f64());
            }

            black_box(hive.resolve(decision_id))
        })
    });

    group.finish();
}

// ============================================================================
// TEMPORAL QUALIA BENCHMARKS
// ============================================================================

fn bench_temporal(c: &mut Criterion) {
    let mut group = c.benchmark_group("temporal");
    group.measurement_time(Duration::from_secs(5));

    // Experience processing
    group.bench_function("experience_100_events", |b| {
        b.iter(|| {
            let mut tq = TemporalQualia::new();
            for i in 0..100 {
                tq.experience(TemporalEvent {
                    id: Uuid::new_v4(),
                    objective_time: i as f64,
                    subjective_time: 0.0,
                    information: 0.5,
                    arousal: 0.3 + 0.4 * (i as f64 / 100.0),
                    novelty: 0.8 - 0.6 * (i as f64 / 100.0),
                });
            }
            black_box(tq.measure_dilation())
        })
    });

    // Time crystal contribution
    group.bench_function("time_crystals", |b| {
        let mut tq = TemporalQualia::new();
        for i in 0..5 {
            tq.add_time_crystal((i + 1) as f64 * 10.0, 1.0 / (i + 1) as f64, vec![0.1; 4]);
        }

        b.iter(|| {
            let mut total = 0.0;
            for t in 0..100 {
                total += tq.crystal_contribution(t as f64);
            }
            black_box(total)
        })
    });

    // Subjective time
    group.bench_function("subjective_time_ticks", |b| {
        let mut st = SubjectiveTime::new();
        b.iter(|| {
            for _ in 0..1000 {
                st.tick(0.1);
            }
            black_box(st.now())
        })
    });

    group.finish();
}

// ============================================================================
// MULTIPLE SELVES BENCHMARKS
// ============================================================================

fn bench_multiple_selves(c: &mut Criterion) {
    let mut group = c.benchmark_group("multiple_selves");
    group.measurement_time(Duration::from_secs(5));

    // Coherence measurement
    group.bench_function("coherence_5_selves", |b| {
        b.iter(|| {
            let mut system = MultipleSelvesSystem::new();
            for i in 0..5 {
                system.add_self(
                    &format!("Self_{}", i),
                    EmotionalTone {
                        valence: (i as f64 - 2.0) / 2.0,
                        arousal: 0.5,
                        dominance: 0.3 + i as f64 * 0.1,
                    },
                );
            }
            black_box(system.measure_coherence())
        })
    });

    // Conflict resolution
    group.bench_function("conflict_resolution", |b| {
        b.iter(|| {
            let mut system = MultipleSelvesSystem::new();
            let id1 = system.add_self(
                "Self1",
                EmotionalTone {
                    valence: 0.8,
                    arousal: 0.6,
                    dominance: 0.7,
                },
            );
            let id2 = system.add_self(
                "Self2",
                EmotionalTone {
                    valence: -0.3,
                    arousal: 0.4,
                    dominance: 0.5,
                },
            );

            system.create_conflict(id1, id2);
            black_box(system.resolve_conflict(id1, id2))
        })
    });

    // Merge operation
    group.bench_function("merge_selves", |b| {
        b.iter(|| {
            let mut system = MultipleSelvesSystem::new();
            let id1 = system.add_self(
                "Part1",
                EmotionalTone {
                    valence: 0.5,
                    arousal: 0.5,
                    dominance: 0.5,
                },
            );
            let id2 = system.add_self(
                "Part2",
                EmotionalTone {
                    valence: 0.5,
                    arousal: 0.5,
                    dominance: 0.5,
                },
            );
            black_box(system.merge(id1, id2))
        })
    });

    group.finish();
}

// ============================================================================
// COGNITIVE THERMODYNAMICS BENCHMARKS
// ============================================================================

fn bench_thermodynamics(c: &mut Criterion) {
    let mut group = c.benchmark_group("thermodynamics");
    group.measurement_time(Duration::from_secs(5));

    // Landauer cost calculation
    group.bench_function("landauer_cost", |b| {
        let thermo = CognitiveThermodynamics::new(300.0);
        b.iter(|| {
            for bits in 1..100 {
                black_box(thermo.landauer_cost(bits));
            }
        })
    });

    // Erasure operation
    group.bench_function("erasure_100_bits", |b| {
        b.iter(|| {
            let mut thermo = CognitiveThermodynamics::new(300.0);
            thermo.add_energy(10000.0);
            for _ in 0..10 {
                black_box(thermo.erase(100));
            }
        })
    });

    // Maxwell's demon
    group.bench_function("maxwell_demon", |b| {
        b.iter(|| {
            let mut thermo = CognitiveThermodynamics::new(300.0);
            for _ in 0..50 {
                black_box(thermo.run_demon(10));
            }
        })
    });

    // Phase transitions
    group.bench_function("phase_transitions", |b| {
        b.iter(|| {
            let mut thermo = CognitiveThermodynamics::new(300.0);
            for temp in [50.0, 100.0, 300.0, 500.0, 800.0, 1200.0, 5.0] {
                thermo.set_temperature(temp);
                black_box(thermo.phase().clone());
            }
        })
    });

    group.finish();
}

// ============================================================================
// EMERGENCE DETECTION BENCHMARKS
// ============================================================================

fn bench_emergence(c: &mut Criterion) {
    let mut group = c.benchmark_group("emergence");
    group.measurement_time(Duration::from_secs(5));

    // Emergence detection
    group.bench_function("detect_emergence_64_micro", |b| {
        b.iter(|| {
            let mut detector = EmergenceDetector::new();
            let micro_state: Vec<f64> = (0..64).map(|i| (i as f64 / 64.0).sin()).collect();
            detector.set_micro_state(micro_state);
            black_box(detector.detect_emergence())
        })
    });

    // With custom coarse-graining
    group.bench_function("custom_coarse_graining", |b| {
        b.iter(|| {
            let mut detector = EmergenceDetector::new();
            let micro_state: Vec<f64> = (0..64).map(|i| i as f64 * 0.01).collect();

            let groupings: Vec<Vec<usize>> = (0..16)
                .map(|i| vec![i * 4, i * 4 + 1, i * 4 + 2, i * 4 + 3])
                .collect();
            detector.set_coarse_graining(groupings, AggregationType::Mean);

            detector.set_micro_state(micro_state);
            black_box(detector.detect_emergence())
        })
    });

    // Causal emergence tracking
    group.bench_function("causal_emergence_updates", |b| {
        b.iter(|| {
            let mut detector = EmergenceDetector::new();
            for i in 0..100 {
                let micro_state: Vec<f64> = (0..32).map(|j| ((i + j) as f64 * 0.1).sin()).collect();
                detector.set_micro_state(micro_state);
                detector.detect_emergence();
            }
            black_box(detector.causal_emergence().score())
        })
    });

    group.finish();
}

// ============================================================================
// COGNITIVE BLACK HOLES BENCHMARKS
// ============================================================================

fn bench_black_holes(c: &mut Criterion) {
    let mut group = c.benchmark_group("black_holes");
    group.measurement_time(Duration::from_secs(5));

    // Thought processing
    group.bench_function("process_100_thoughts", |b| {
        b.iter(|| {
            let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 1.5, TrapType::Rumination);
            for i in 0..100 {
                let thought = vec![i as f64 * 0.01; 8];
                black_box(bh.process_thought(thought));
            }
        })
    });

    // Escape attempts
    group.bench_function("escape_attempts", |b| {
        b.iter(|| {
            let mut bh = CognitiveBlackHole::with_params(vec![0.0; 8], 2.0, TrapType::Anxiety);

            // Capture some thoughts
            for _ in 0..10 {
                bh.process_thought(vec![0.1; 8]);
            }

            // Try various escape methods
            bh.attempt_escape(0.5, EscapeMethod::Gradual);
            bh.attempt_escape(1.0, EscapeMethod::Tunneling);
            bh.attempt_escape(2.0, EscapeMethod::Reframe);
            black_box(bh.attempt_escape(5.0, EscapeMethod::External))
        })
    });

    // Orbital decay
    group.bench_function("orbital_decay_1000_ticks", |b| {
        b.iter(|| {
            let mut bh = CognitiveBlackHole::new();
            for _ in 0..5 {
                bh.process_thought(vec![0.2; 8]);
            }
            for _ in 0..1000 {
                bh.tick();
            }
            black_box(bh.captured_count())
        })
    });

    group.finish();
}

// ============================================================================
// INTEGRATED BENCHMARKS
// ============================================================================

fn bench_integrated(c: &mut Criterion) {
    let mut group = c.benchmark_group("integrated");
    group.measurement_time(Duration::from_secs(10));

    // Full experiment suite
    group.bench_function("full_experiment_suite", |b| {
        b.iter(|| {
            let mut experiments = exo_exotic::ExoticExperiments::new();
            black_box(experiments.run_all())
        })
    });

    group.finish();
}

// ============================================================================
// SCALING BENCHMARKS
// ============================================================================

fn bench_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaling");
    group.measurement_time(Duration::from_secs(5));

    // Strange loop scaling
    for depth in [5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("strange_loop_depth", depth),
            &depth,
            |b, &depth| {
                b.iter(|| {
                    let mut sl = StrangeLoop::new(depth);
                    for _ in 0..depth {
                        sl.model_self();
                    }
                    black_box(sl.measure_depth())
                })
            },
        );
    }

    // Morphogenesis scaling
    for size in [8, 16, 32] {
        group.bench_with_input(
            BenchmarkId::new("morphogenesis_field", size),
            &size,
            |b, &size| {
                b.iter(|| {
                    let mut field = MorphogeneticField::new(size, size);
                    field.simulate(50);
                    black_box(field.measure_complexity())
                })
            },
        );
    }

    // Collective consciousness scaling
    for count in [5, 10, 20] {
        group.bench_with_input(
            BenchmarkId::new("collective_substrates", count),
            &count,
            |b, &count| {
                b.iter(|| {
                    let mut collective = CollectiveConsciousness::new();
                    let ids: Vec<Uuid> = (0..count)
                        .map(|_| collective.add_substrate(SubstrateSpecialization::Processing))
                        .collect();

                    for i in 0..ids.len() {
                        for j in i + 1..ids.len() {
                            collective.connect(ids[i], ids[j], 0.5, true);
                        }
                    }
                    black_box(collective.compute_global_phi())
                })
            },
        );
    }

    group.finish();
}

// Helper function
fn rand_f64() -> f64 {
    use std::time::SystemTime;
    let seed = SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(12345) as u64;
    let result = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (result as f64) / (u64::MAX as f64)
}

criterion_group!(
    benches,
    bench_strange_loops,
    bench_dreams,
    bench_free_energy,
    bench_morphogenesis,
    bench_collective,
    bench_temporal,
    bench_multiple_selves,
    bench_thermodynamics,
    bench_emergence,
    bench_black_holes,
    bench_integrated,
    bench_scaling,
);

criterion_main!(benches);
