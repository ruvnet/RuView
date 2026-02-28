use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ruvector_domain_expansion::{
    AccelerationScoreboard, ArmId, ContextBucket, ConvergenceThresholds, CostCurve, CostCurvePoint,
    CuriosityBonus, DecayingBeta, DomainExpansionEngine, DomainId, MetaLearningEngine,
    MetaThompsonEngine, ParetoFront, ParetoPoint, PlateauDetector, PolicyKnobs, PopulationSearch,
    RegretTracker, Solution, TransferPrior,
};

fn bench_task_generation(c: &mut Criterion) {
    let engine = DomainExpansionEngine::new();
    let domains = engine.domain_ids();

    let mut group = c.benchmark_group("task_generation");

    for domain_id in &domains {
        group.bench_function(format!("{}", domain_id), |b| {
            b.iter(|| engine.generate_tasks(black_box(domain_id), black_box(10), black_box(0.5)))
        });
    }
    group.finish();
}

fn bench_evaluation(c: &mut Criterion) {
    let engine = DomainExpansionEngine::new();
    let rust_id = DomainId("rust_synthesis".into());
    let tasks = engine.generate_tasks(&rust_id, 10, 0.5);

    let solution = Solution {
        task_id: tasks[0].id.clone(),
        content:
            "fn sum_positives(values: &[i64]) -> i64 { values.iter().filter(|&&x| x > 0).sum() }"
                .into(),
        data: serde_json::Value::Null,
    };

    c.bench_function("evaluate_rust_solution", |b| {
        b.iter(|| {
            let mut eng = DomainExpansionEngine::new();
            eng.evaluate_and_record(
                black_box(&rust_id),
                black_box(&tasks[0]),
                black_box(&solution),
                ContextBucket {
                    difficulty_tier: "medium".into(),
                    category: "transform".into(),
                },
                ArmId("greedy".into()),
            )
        })
    });
}

fn bench_embedding(c: &mut Criterion) {
    let engine = DomainExpansionEngine::new();
    let rust_id = DomainId("rust_synthesis".into());

    let solution = Solution {
        task_id: "bench".into(),
        content: "fn foo() { for i in 0..10 { if i > 5 { let x = i.max(3); } } }".into(),
        data: serde_json::Value::Null,
    };

    c.bench_function("embed_solution", |b| {
        b.iter(|| engine.embed(black_box(&rust_id), black_box(&solution)))
    });
}

fn bench_thompson_sampling(c: &mut Criterion) {
    let mut engine = MetaThompsonEngine::new(vec![
        "greedy".into(),
        "exploratory".into(),
        "conservative".into(),
        "speculative".into(),
    ]);

    let domain = DomainId("bench".into());
    engine.init_domain_uniform(domain.clone());

    let bucket = ContextBucket {
        difficulty_tier: "medium".into(),
        category: "algorithm".into(),
    };

    // Pre-populate with data
    for i in 0..100 {
        let arm = ArmId(format!(
            "{}",
            ["greedy", "exploratory", "conservative", "speculative"][i % 4]
        ));
        let reward = if i % 4 == 0 { 0.9 } else { 0.4 };
        engine.record_outcome(&domain, bucket.clone(), arm, reward, 1.0);
    }

    c.bench_function("thompson_select_arm", |b| {
        b.iter(|| {
            let mut rng = rand::thread_rng();
            engine.select_arm(black_box(&domain), black_box(&bucket), &mut rng)
        })
    });
}

fn bench_population_evolve(c: &mut Criterion) {
    let mut search = PopulationSearch::new(16);

    // Pre-populate fitness
    for i in 0..16 {
        if let Some(kernel) = search.kernel_mut(i) {
            kernel.record_score(DomainId("bench".into()), i as f32 / 16.0, 1.0);
        }
    }

    c.bench_function("population_evolve_16", |b| {
        b.iter(|| {
            let mut s = search.clone();
            s.evolve();
        })
    });
}

fn bench_knobs_mutate(c: &mut Criterion) {
    let knobs = PolicyKnobs::default_knobs();
    c.bench_function("knobs_mutate", |b| {
        b.iter(|| {
            let mut rng = rand::thread_rng();
            black_box(knobs.mutate(&mut rng, 0.3))
        })
    });
}

fn bench_cost_curve_auc(c: &mut Criterion) {
    let mut curve = CostCurve::new(DomainId("bench".into()), ConvergenceThresholds::default());
    for i in 0..1000 {
        curve.record(CostCurvePoint {
            cycle: i,
            accuracy: (i as f32 / 1000.0).min(1.0),
            cost_per_solve: 1.0 / (i as f32 + 1.0),
            robustness: (i as f32 / 1000.0).min(1.0),
            policy_violations: 0,
            timestamp: i as f64,
        });
    }

    c.bench_function("cost_curve_auc_1000pts", |b| {
        b.iter(|| black_box(curve.auc_accuracy()))
    });
}

fn bench_transfer_prior_extract(c: &mut Criterion) {
    let domain = DomainId("bench".into());
    let mut prior = TransferPrior::uniform(domain);

    // Populate with 100 buckets x 4 arms
    for b in 0..100 {
        for a in 0..4 {
            let bucket = ContextBucket {
                difficulty_tier: format!("tier_{}", b % 3),
                category: format!("cat_{}", b),
            };
            let arm = ArmId(format!("arm_{}", a));
            for _ in 0..20 {
                prior.update_posterior(bucket.clone(), arm.clone(), 0.7);
            }
        }
    }

    c.bench_function("transfer_prior_extract_100buckets", |b| {
        b.iter(|| black_box(prior.extract_summary()))
    });
}

// ═══════════════════════════════════════════════════════════════════
// Meta-Learning Benchmarks
// ═══════════════════════════════════════════════════════════════════

fn bench_regret_tracker(c: &mut Criterion) {
    let bucket = ContextBucket {
        difficulty_tier: "medium".into(),
        category: "algo".into(),
    };
    let arms: Vec<ArmId> = (0..4).map(|i| ArmId(format!("arm_{}", i))).collect();

    let mut group = c.benchmark_group("meta_learning");

    group.bench_function("regret_record_1k", |b| {
        b.iter(|| {
            let mut tracker = RegretTracker::new(50);
            for i in 0..1000 {
                let arm = &arms[i % 4];
                let reward = if i % 4 == 0 { 0.9 } else { 0.4 };
                tracker.record(black_box(&bucket), black_box(arm), black_box(reward));
            }
            black_box(tracker.average_regret())
        })
    });

    group.bench_function("regret_summary", |b| {
        let mut tracker = RegretTracker::new(50);
        for i in 0..1000 {
            let arm = &arms[i % 4];
            tracker.record(&bucket, arm, if i % 4 == 0 { 0.9 } else { 0.4 });
        }
        b.iter(|| black_box(tracker.summary()))
    });

    group.finish();
}

fn bench_decaying_beta(c: &mut Criterion) {
    let mut group = c.benchmark_group("decaying_beta");

    group.bench_function("update_1k", |b| {
        b.iter(|| {
            let mut db = DecayingBeta::new(0.995);
            for i in 0..1000 {
                let reward = if i % 3 == 0 { 0.9 } else { 0.4 };
                db.update(black_box(reward));
            }
            black_box(db.mean())
        })
    });

    group.bench_function("update_vs_standard", |b| {
        b.iter(|| {
            // Compare DecayingBeta vs standard BetaParams
            let mut db = DecayingBeta::new(0.995);
            let mut std_beta = ruvector_domain_expansion::BetaParams::uniform();
            for i in 0..500 {
                let reward = if i % 3 == 0 { 0.9 } else { 0.4 };
                db.update(reward);
                std_beta.update(reward);
            }
            black_box((db.mean(), std_beta.mean()))
        })
    });

    group.finish();
}

fn bench_plateau_detector(c: &mut Criterion) {
    let points: Vec<CostCurvePoint> = (0..100)
        .map(|i| CostCurvePoint {
            cycle: i,
            accuracy: 0.80 + (i as f32 * 0.001),
            cost_per_solve: 0.1 / (i as f32 + 1.0),
            robustness: 0.8,
            policy_violations: 0,
            timestamp: i as f64,
        })
        .collect();

    c.bench_function("plateau_check_100pts", |b| {
        b.iter(|| {
            let mut detector = PlateauDetector::new(10, 0.005);
            black_box(detector.check(black_box(&points)))
        })
    });
}

fn bench_pareto_front(c: &mut Criterion) {
    let mut group = c.benchmark_group("pareto_front");

    group.bench_function("insert_100_points", |b| {
        b.iter(|| {
            let mut front = ParetoFront::new();
            for i in 0..100 {
                let acc = (i as f32) / 100.0;
                let cost = -((100 - i) as f32) / 100.0;
                let rob = ((i * 7 + 13) % 100) as f32 / 100.0;
                front.insert(ParetoPoint {
                    kernel_id: format!("k{}", i),
                    objectives: vec![acc, cost, rob],
                    generation: 0,
                });
            }
            black_box(front.len())
        })
    });

    group.bench_function("hypervolume_2d", |b| {
        let mut front = ParetoFront::new();
        for i in 0..20 {
            let x = (i as f32 + 1.0) / 21.0;
            front.insert(ParetoPoint {
                kernel_id: format!("k{}", i),
                objectives: vec![x, 1.0 - x],
                generation: 0,
            });
        }
        b.iter(|| black_box(front.hypervolume(&[0.0, 0.0])))
    });

    group.finish();
}

fn bench_curiosity_bonus(c: &mut Criterion) {
    let arms: Vec<ArmId> = (0..4).map(|i| ArmId(format!("arm_{}", i))).collect();
    let buckets: Vec<ContextBucket> = (0..18)
        .map(|i| ContextBucket {
            difficulty_tier: ["easy", "medium", "hard"][i / 6].into(),
            category: format!("cat_{}", i % 6),
        })
        .collect();

    c.bench_function("curiosity_bonus_18buckets", |b| {
        let mut curiosity = CuriosityBonus::new(1.41);
        for _ in 0..500 {
            for bucket in &buckets {
                for arm in &arms {
                    curiosity.record_visit(bucket, arm);
                }
            }
        }
        b.iter(|| {
            let mut total = 0.0f32;
            for bucket in &buckets {
                for arm in &arms {
                    total += curiosity.bonus(black_box(bucket), black_box(arm));
                }
            }
            black_box(total)
        })
    });
}

fn bench_meta_engine_full_cycle(c: &mut Criterion) {
    c.bench_function("meta_engine_100_decisions", |b| {
        b.iter(|| {
            let mut engine = MetaLearningEngine::new();
            let bucket = ContextBucket {
                difficulty_tier: "medium".into(),
                category: "algo".into(),
            };
            let arm = ArmId("greedy".into());

            for i in 0..100 {
                let reward = if i % 3 == 0 { 0.9 } else { 0.5 };
                engine.record_decision(&bucket, &arm, reward);
            }

            engine.record_kernel("k1", 0.9, 0.2, 0.8, 1);
            black_box(engine.health_check())
        })
    });
}

criterion_group!(
    benches,
    bench_task_generation,
    bench_evaluation,
    bench_embedding,
    bench_thompson_sampling,
    bench_population_evolve,
    bench_knobs_mutate,
    bench_cost_curve_auc,
    bench_transfer_prior_extract,
    bench_regret_tracker,
    bench_decaying_beta,
    bench_plateau_detector,
    bench_pareto_front,
    bench_curiosity_bonus,
    bench_meta_engine_full_cycle,
);
criterion_main!(benches);
