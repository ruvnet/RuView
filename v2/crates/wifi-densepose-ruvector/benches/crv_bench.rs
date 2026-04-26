//! Benchmarks for CRV (Coordinate Remote Viewing) integration.
//!
//! Measures throughput of gestalt classification, sensory encoding,
//! full session pipelines, cross-session convergence, and embedding
//! dimension scaling using the `ruvector-crv` crate directly.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ruvector_crv::{
    CrvConfig, CrvSessionManager, GestaltType, SensoryModality, StageIData, StageIIData,
    StageIIIData, StageIVData,
};
use ruvector_crv::types::{
    GeometricKind, SketchElement, SpatialRelationType, SpatialRelationship,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a synthetic CSI-like ideogram stroke with `n` subcarrier points.
fn make_stroke(n: usize) -> Vec<(f32, f32)> {
    (0..n)
        .map(|i| {
            let t = i as f32 / n as f32;
            (t, (t * std::f32::consts::TAU).sin() * 0.5 + 0.5)
        })
        .collect()
}

/// Build a Stage I data frame representing a single CSI gestalt sample.
fn make_stage_i(gestalt: GestaltType) -> StageIData {
    StageIData {
        stroke: make_stroke(64),
        spontaneous_descriptor: "angular rising".to_string(),
        classification: gestalt,
        confidence: 0.85,
    }
}

/// Build a Stage II sensory data frame.
fn make_stage_ii() -> StageIIData {
    StageIIData {
        impressions: vec![
            (SensoryModality::Texture, "rough metallic".to_string()),
            (SensoryModality::Temperature, "warm".to_string()),
            (SensoryModality::Color, "silver-gray".to_string()),
            (SensoryModality::Luminosity, "reflective".to_string()),
            (SensoryModality::Sound, "low hum".to_string()),
        ],
        feature_vector: None,
    }
}

/// Build a Stage III spatial sketch.
fn make_stage_iii() -> StageIIIData {
    StageIIIData {
        sketch_elements: vec![
            SketchElement {
                label: "tower".to_string(),
                kind: GeometricKind::Rectangle,
                position: (0.5, 0.8),
                scale: Some(3.0),
            },
            SketchElement {
                label: "base".to_string(),
                kind: GeometricKind::Rectangle,
                position: (0.5, 0.2),
                scale: Some(5.0),
            },
            SketchElement {
                label: "antenna".to_string(),
                kind: GeometricKind::Line,
                position: (0.5, 0.95),
                scale: Some(1.0),
            },
        ],
        relationships: vec![
            SpatialRelationship {
                from: "tower".to_string(),
                to: "base".to_string(),
                relation: SpatialRelationType::Above,
                strength: 0.9,
            },
            SpatialRelationship {
                from: "antenna".to_string(),
                to: "tower".to_string(),
                relation: SpatialRelationType::Above,
                strength: 0.85,
            },
        ],
    }
}

/// Build a Stage IV emotional / AOL data frame.
fn make_stage_iv() -> StageIVData {
    StageIVData {
        emotional_impact: vec![
            ("awe".to_string(), 0.7),
            ("curiosity".to_string(), 0.6),
            ("unease".to_string(), 0.3),
        ],
        tangibles: vec!["metal structure".to_string(), "concrete".to_string()],
        intangibles: vec!["transmission".to_string(), "power".to_string()],
        aol_detections: vec![],
    }
}

/// Create a manager with one session pre-loaded with 4 stages of data.
fn populated_manager(dims: usize) -> (CrvSessionManager, String) {
    let config = CrvConfig {
        dimensions: dims,
        ..CrvConfig::default()
    };
    let mut mgr = CrvSessionManager::new(config);
    let sid = "bench-sess".to_string();
    mgr.create_session(sid.clone(), "coord-001".to_string())
        .unwrap();
    mgr.add_stage_i(&sid, &make_stage_i(GestaltType::Manmade))
        .unwrap();
    mgr.add_stage_ii(&sid, &make_stage_ii()).unwrap();
    mgr.add_stage_iii(&sid, &make_stage_iii()).unwrap();
    mgr.add_stage_iv(&sid, &make_stage_iv()).unwrap();
    (mgr, sid)
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

/// Benchmark: classify a single CSI frame through Stage I (64 subcarriers).
fn gestalt_classify_single(c: &mut Criterion) {
    let config = CrvConfig {
        dimensions: 64,
        ..CrvConfig::default()
    };
    let mut manager = CrvSessionManager::new(config);
    manager
        .create_session("gc-single".to_string(), "coord-gc".to_string())
        .unwrap();

    let data = make_stage_i(GestaltType::Manmade);

    c.bench_function("gestalt_classify_single", |b| {
        b.iter(|| {
            manager
                .add_stage_i("gc-single", black_box(&data))
                .unwrap();
        })
    });
}

/// Benchmark: classify a batch of 100 CSI frames through Stage I.
fn gestalt_classify_batch(c: &mut Criterion) {
    let config = CrvConfig {
        dimensions: 64,
        ..CrvConfig::default()
    };

    let gestalts = GestaltType::all();
    let frames: Vec<StageIData> = (0..100)
        .map(|i| make_stage_i(gestalts[i % gestalts.len()]))
        .collect();

    c.bench_function("gestalt_classify_batch_100", |b| {
        b.iter(|| {
            let mut manager = CrvSessionManager::new(CrvConfig {
                dimensions: 64,
                ..CrvConfig::default()
            });
            manager
                .create_session("gc-batch".to_string(), "coord-gcb".to_string())
                .unwrap();

            for frame in black_box(&frames) {
                manager.add_stage_i("gc-batch", frame).unwrap();
            }
        })
    });
}

/// Benchmark: extract sensory features from a single CSI frame (Stage II).
fn sensory_encode_single(c: &mut Criterion) {
    let config = CrvConfig {
        dimensions: 64,
        ..CrvConfig::default()
    };
    let mut manager = CrvSessionManager::new(config);
    manager
        .create_session("se-single".to_string(), "coord-se".to_string())
        .unwrap();

    let data = make_stage_ii();

    c.bench_function("sensory_encode_single", |b| {
        b.iter(|| {
            manager
                .add_stage_ii("se-single", black_box(&data))
                .unwrap();
        })
    });
}

/// Benchmark: full session pipeline -- create session, add 10 mixed-stage
/// frames, run Stage V interrogation, and run Stage VI partitioning.
fn pipeline_full_session(c: &mut Criterion) {
    let stage_i_data = make_stage_i(GestaltType::Manmade);
    let stage_ii_data = make_stage_ii();
    let stage_iii_data = make_stage_iii();
    let stage_iv_data = make_stage_iv();

    c.bench_function("pipeline_full_session", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let config = CrvConfig {
                dimensions: 64,
                ..CrvConfig::default()
            };
            let mut manager = CrvSessionManager::new(config);
            let sid = format!("pfs-{}", counter);
            manager
                .create_session(sid.clone(), "coord-pfs".to_string())
                .unwrap();

            // 10 frames across stages I-IV
            for _ in 0..3 {
                manager
                    .add_stage_i(&sid, black_box(&stage_i_data))
                    .unwrap();
            }
            for _ in 0..3 {
                manager
                    .add_stage_ii(&sid, black_box(&stage_ii_data))
                    .unwrap();
            }
            for _ in 0..2 {
                manager
                    .add_stage_iii(&sid, black_box(&stage_iii_data))
                    .unwrap();
            }
            for _ in 0..2 {
                manager
                    .add_stage_iv(&sid, black_box(&stage_iv_data))
                    .unwrap();
            }

            // Stage V: interrogate with a probe embedding
            let probe_emb = vec![0.1f32; 64];
            let probes: Vec<(&str, u8, Vec<f32>)> = vec![
                ("structure query", 1, probe_emb.clone()),
                ("texture query", 2, probe_emb.clone()),
            ];
            let _ = manager.run_stage_v(&sid, &probes, 3);

            // Stage VI: partition
            let _ = manager.run_stage_vi(&sid);
        })
    });
}

/// Benchmark: cross-session convergence analysis with 2 independent
/// sessions of 10 frames each, targeting the same coordinate.
fn convergence_two_sessions(c: &mut Criterion) {
    let gestalts = [GestaltType::Manmade, GestaltType::Natural, GestaltType::Energy];
    let stage_ii_data = make_stage_ii();

    c.bench_function("convergence_two_sessions", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            let config = CrvConfig {
                dimensions: 64,
                convergence_threshold: 0.5,
                ..CrvConfig::default()
            };
            let mut manager = CrvSessionManager::new(config);
            let coord = format!("conv-coord-{}", counter);

            // Session A: 10 frames
            let sid_a = format!("viewer-a-{}", counter);
            manager
                .create_session(sid_a.clone(), coord.clone())
                .unwrap();
            for i in 0..5 {
                let data = make_stage_i(gestalts[i % gestalts.len()]);
                manager.add_stage_i(&sid_a, black_box(&data)).unwrap();
            }
            for _ in 0..5 {
                manager
                    .add_stage_ii(&sid_a, black_box(&stage_ii_data))
                    .unwrap();
            }

            // Session B: 10 frames (similar but not identical)
            let sid_b = format!("viewer-b-{}", counter);
            manager
                .create_session(sid_b.clone(), coord.clone())
                .unwrap();
            for i in 0..5 {
                let data = make_stage_i(gestalts[(i + 1) % gestalts.len()]);
                manager.add_stage_i(&sid_b, black_box(&data)).unwrap();
            }
            for _ in 0..5 {
                manager
                    .add_stage_ii(&sid_b, black_box(&stage_ii_data))
                    .unwrap();
            }

            // Convergence analysis
            let _ = manager.find_convergence(&coord, black_box(0.5));
        })
    });
}

/// Benchmark: session creation overhead alone.
fn crv_session_create(c: &mut Criterion) {
    c.bench_function("crv_session_create", |b| {
        b.iter(|| {
            let config = CrvConfig {
                dimensions: 32,
                ..CrvConfig::default()
            };
            let mut manager = CrvSessionManager::new(black_box(config));
            manager
                .create_session(
                    black_box("sess-1".to_string()),
                    black_box("coord-1".to_string()),
                )
                .unwrap();
        })
    });
}

/// Benchmark: embedding dimension scaling (32, 128, 384).
///
/// Measures Stage I + Stage II encode time across different embedding
/// dimensions to characterize how cost grows with dimensionality.
fn crv_embedding_dimension_scaling(c: &mut Criterion) {
    let stage_i_data = make_stage_i(GestaltType::Manmade);
    let stage_ii_data = make_stage_ii();

    let mut group = c.benchmark_group("crv_embedding_dimension_scaling");
    for dims in [32, 128, 384] {
        group.bench_with_input(BenchmarkId::from_parameter(dims), &dims, |b, &dims| {
            let mut counter = 0u64;
            b.iter(|| {
                counter += 1;
                let config = CrvConfig {
                    dimensions: dims,
                    ..CrvConfig::default()
                };
                let mut manager = CrvSessionManager::new(config);
                let sid = format!("dim-{}-{}", dims, counter);
                manager
                    .create_session(sid.clone(), "coord-dim".to_string())
                    .unwrap();

                // Encode one Stage I + one Stage II at this dimensionality
                let emb_i = manager
                    .add_stage_i(&sid, black_box(&stage_i_data))
                    .unwrap();
                let emb_ii = manager
                    .add_stage_ii(&sid, black_box(&stage_ii_data))
                    .unwrap();

                assert_eq!(emb_i.len(), dims);
                assert_eq!(emb_ii.len(), dims);
            })
        });
    }
    group.finish();
}

/// Benchmark: Stage VI partitioning on a pre-populated session
/// (4 stages of accumulated data).
fn crv_stage_vi_partition(c: &mut Criterion) {
    c.bench_function("crv_stage_vi_partition", |b| {
        let mut counter = 0u64;
        b.iter(|| {
            counter += 1;
            // Re-create the populated manager each iteration because
            // run_stage_vi mutates the session (appends an entry).
            let (mut mgr, sid) = populated_manager(64);
            let _ = mgr.run_stage_vi(black_box(&sid));
        })
    });
}

// ---------------------------------------------------------------------------
// Criterion groups
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    gestalt_classify_single,
    gestalt_classify_batch,
    sensory_encode_single,
    pipeline_full_session,
    convergence_two_sessions,
    crv_session_create,
    crv_embedding_dimension_scaling,
    crv_stage_vi_partition,
);

criterion_main!(benches);
