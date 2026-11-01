//! SYNTHETIC exact sign-inversion oracle: pooled views cannot identify the room.
use crate::room_learner::{
    model::*,
    observation::*,
    training::{learning_tests::dataset, FitConfig},
};

fn inverted_dataset(reverse: bool) -> ValidatedDataset {
    let base = dataset(8);
    let mut rows = base
        .train()
        .iter()
        .chain(base.validation())
        .chain(base.held_out())
        .cloned()
        .collect::<Vec<_>>();
    for c in &mut rows {
        for v in &mut c.views {
            for (t, frame) in v.samples.iter_mut().enumerate() {
                for (b, x) in frame.iter_mut().enumerate() {
                    let empty = 10.0 + t as f32 * 0.02 + b as f32 * 0.001;
                    let state = match c.label {
                        RoomLabel::Empty => 0.0,
                        RoomLabel::AdjacentOnly => -1.0,
                        _ => 1.0,
                    };
                    let source = if (v.source_id == 1) != reverse {
                        1.0
                    } else {
                        -1.0
                    };
                    *x = empty + source * state * 0.25;
                }
            }
        }
    }
    ValidatedDataset::new(base.context().clone(), rows).unwrap()
}
fn cosine(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum::<f32>()
        / (a.iter().map(|x| x * x).sum::<f32>().sqrt()
            * b.iter().map(|x| x * x).sum::<f32>().sqrt())
        .max(1e-10)
}
fn margin(
    model: &RoomModel,
    occupied: &CaptureCohort,
    adjacent: &CaptureCohort,
    condition: bool,
) -> f32 {
    let mut stripped = model.clone();
    if !condition {
        for i in 1..stripped.source_projection_weights.len() {
            stripped.source_projection_weights[i] = stripped.source_projection_weights[0].clone();
        }
    }
    let model = if condition { model } else { &stripped };
    let mut a = model.tokens(occupied, false).unwrap();
    let mut b = model.tokens(adjacent, false).unwrap();
    if !condition {
        for tokens in a.iter_mut().chain(b.iter_mut()) {
            for row in tokens {
                row[model.encoder_config().bins..].fill(0.0);
            }
        }
    }
    let (_, a) = model.represent(&a, &model.parameters()).unwrap();
    let (_, b) = model.represent(&b, &model.parameters()).unwrap();
    (cosine(&a[0], &a[1]) + cosine(&b[0], &b[1]) - cosine(&a[0], &b[1]) - cosine(&b[0], &a[1]))
        * 0.5
}
fn check_inverted_case(reverse: bool) {
    let ds = inverted_dataset(reverse);
    let occupied = &ds.held_out()[2];
    let adjacent = &ds.held_out()[1];
    let pooled = |c: &CaptureCohort| {
        c.views
            .iter()
            .flat_map(|v| v.samples.iter().flatten())
            .sum::<f32>()
    };
    assert!((pooled(occupied) - pooled(adjacent)).abs() < 0.001);
    assert_eq!(occupied.views[0].samples, adjacent.views[1].samples);
    assert_eq!(occupied.views[1].samples, adjacent.views[0].samples);
    let (model, report) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 48,
            max_duration_ms: 120_000,
            learning_rate: 0.5,
            hidden_dim: 4,
            embedding_dim: 4,
            contrastive_weight: 5.0,
            supervised_weight: 0.5,
            ..FitConfig::default()
        },
    )
    .unwrap();
    let conditional = margin(&model, occupied, adjacent, true);
    let unconditioned = margin(&model, occupied, adjacent, false);
    eprintln!("SYNTHETIC inverted RF InfoNCE {}->{}; positive-vs-opposite margin conditioned={conditional}, removed-tags={unconditioned}; report={report:?}",report.initial.contrastive,report.final_objective.contrastive);
    assert!(report.backbone_delta_l2 > 1e-4);
    assert!(report.projection_delta_l2 > 1e-4);
    assert!(report.final_objective.contrastive < report.initial.contrastive - 0.02);
    assert!(
        conditional > 0.1,
        "shared encoder must align same-event opposite-sign views"
    );
    assert!(
        unconditioned <= 1e-5,
        "removed conditioning must recover the impossible symmetric-view case"
    );
    let evaluation = model.evaluate(ds.held_out()).unwrap();
    assert_eq!(evaluation.correct, 4, "{evaluation:?}");
    assert_eq!(evaluation.abstained, 0);
    assert!(!model.infer(occupied).physically_validated);
}
#[test]
fn opposite_source_responses_learn_real_alignment_and_disjoint_room_controls() {
    check_inverted_case(false);
}
#[test]
fn reversed_response_polarities_learn_with_the_same_generic_initialization() {
    check_inverted_case(true);
}
#[test]
fn source_count_and_metadata_cannot_alias_canonical_rf_or_evade_import_bounds() {
    let ds = dataset(56);
    let (model, report) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 1,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert_eq!(model.encoder_config().bins, 56);
    assert_eq!(model.encoder_config().source_count, 2);
    assert!(report.parameter_count <= 512);
    let original: serde_json::Value = serde_json::from_slice(&model.export().unwrap()).unwrap();
    for count in [0, 1, 3, 5000] {
        let mut bad = original.clone();
        bad["config"]["source_count"] = serde_json::json!(count);
        assert!(RoomModel::import(&serde_json::to_vec(&bad).unwrap()).is_err());
    }
    let mut missing = original;
    missing["config"]
        .as_object_mut()
        .unwrap()
        .remove("source_count");
    assert!(RoomModel::import(&serde_json::to_vec(&missing).unwrap()).is_err());
    for malformed in [
        serde_json::Value::Null,
        serde_json::json!([]),
        serde_json::json!([[], []]),
    ] {
        let mut invalid: serde_json::Value =
            serde_json::from_slice(&model.export().unwrap()).unwrap();
        invalid["source_projection_weights"] = malformed;
        assert!(RoomModel::import(&serde_json::to_vec(&invalid).unwrap()).is_err());
    }
    let tokens = model.tokens(&ds.held_out()[0], false).unwrap();
    let (features, _) = model.represent(&tokens, &model.parameters()).unwrap();
    let mut zeroed = tokens;
    for source in &mut zeroed {
        for row in source {
            row[56..].fill(0.0);
        }
    }
    let (without_tags, _) = model.represent(&zeroed, &model.parameters()).unwrap();
    let width = model.encoder_config().embedding_dim + 2;
    for source in 0..2 {
        assert_eq!(
            &features[source * width + width - 2..source * width + width],
            &without_tags[source * width + width - 2..source * width + width]
        );
    }
}

#[test]
fn three_native_56_bin_sources_train_every_projection_within_actual_budget() {
    let base = dataset(56);
    let mut context = base.context().clone();
    let mut source = context.sources[0].clone();
    source.source_id = 3;
    source.device_id = "synthetic-device-3".into();
    source.link_id = "link-3".into();
    context.sources.push(source.clone());
    let mut rows = base
        .train()
        .iter()
        .chain(base.validation())
        .chain(base.held_out())
        .cloned()
        .collect::<Vec<_>>();
    for row in &mut rows {
        let mut view = row.views[0].clone();
        view.source_id = 3;
        view.device_id = source.device_id.clone();
        view.link_id = source.link_id.clone();
        row.views.push(view);
        row.context_digest = context.digest();
    }
    let ds = ValidatedDataset::new(context, rows).unwrap();
    let (model, report) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 1,
            ..FitConfig::default()
        },
    )
    .unwrap();
    assert_eq!(model.encoder_config().source_count, 3);
    assert_eq!(report.parameter_count, 395);
    assert!(report.backbone_delta_l2 > 0.0);
    let mut initial_model = model.clone();
    initial_model.encoder_weights = initial_encoder(model.encoder_config())
        .unwrap()
        .transformer
        .flatten_weights();
    initial_model.source_projection_weights = initial_projection_weights(model.encoder_config());
    center_empty_heads(&mut initial_model, ds.train()).unwrap();
    let initial = initial_model.source_projection_weights;
    for (before, after) in initial.iter().zip(&model.source_projection_weights) {
        assert!(before.iter().zip(after).any(|(a, b)| (a - b).abs() > 1e-6));
    }
    let tokens = model.tokens(&ds.held_out()[0], false).unwrap();
    assert_eq!(tokens[2][0].len(), 59);
    assert_eq!(&tokens[2][0][56..], &[0.0, 0.0, 1.0]);
}
