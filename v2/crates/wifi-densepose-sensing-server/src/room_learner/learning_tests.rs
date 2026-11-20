//! SYNTHETIC leakage-free RF fixtures: learning mechanics, not room accuracy.
use crate::room_learner::model::*;
use crate::room_learner::observation::*;
use crate::room_learner::training::*;
use ruview_unified::RfModality;
use std::sync::atomic::AtomicBool;

fn context(width: usize) -> CaptureContext {
    CaptureContext {
        owner_scope: "synthetic-owner".into(),
        installation_id: "synthetic-install".into(),
        room_digest: "synthetic-room".into(),
        revision: 1,
        sources: (1..=2)
            .map(|id| SourceSpec {
                source_id: id,
                device_id: format!("synthetic-device-{id}"),
                grid_id: format!("phy-{width}-bins"),
                feature_count: width,
                link_index: 0,
                center_frequency_hz: 2_400_000_000,
                bandwidth_hz: 20_000_000,
                signal_kind: RfModality::Synthetic,
                link_id: format!("link-{id}"),
                geometry: None,
            })
            .collect(),
    }
}
fn cohort(c: &CaptureContext, id: u64, label: RoomLabel, role: DataRole) -> CaptureCohort {
    let views = c
        .sources
        .iter()
        .map(|source| {
            let shift = match label {
                RoomLabel::Empty | RoomLabel::Unknown => 0.0,
                RoomLabel::AdjacentOnly => {
                    if source.source_id == 1 {
                        1.7
                    } else {
                        -1.7
                    }
                }
                RoomLabel::OccupiedStatic | RoomLabel::OccupiedMoving => 1.7,
            };
            let variation = (id % 5) as f32 * 0.015;
            let samples = (0..4)
                .map(|t| {
                    (0..source.feature_count)
                        .map(|b| {
                            let motion = if label == RoomLabel::OccupiedMoving {
                                (t as f32 * 1.3).sin() * 0.12
                            } else {
                                0.0
                            };
                            10.0 + b as f32 * 0.003
                                + (t as f32 - 1.5) * 0.03
                                + shift
                                + variation
                                + motion
                        })
                        .collect()
                })
                .collect();
            SourceView {
                source_id: source.source_id,
                device_id: source.device_id.clone(),
                grid_id: source.grid_id.clone(),
                signal_kind: source.signal_kind,
                link_index: 0,
                center_frequency_hz: 2_400_000_000,
                bandwidth_hz: 20_000_000,
                link_id: source.link_id.clone(),
                geometry: None,
                samples,
                frame_ids: (0..4).map(|t| u128::from(id) * 10 + t + 1).collect(),
                sequence_ids: vec![1, 2, 3, 4],
                first_time_ns: id * 1000 + 1,
                last_time_ns: id * 1000 + 101,
                clock: ClockInfo {
                    kind: ClockKind::Unknown,
                    uncertainty_ns: u64::MAX,
                },
                sample_age_ns: 0,
                phase: PhaseAvailability::MagnitudeOnly,
                provenance: CaptureProvenance::Synthetic,
            }
        })
        .collect();
    CaptureCohort {
        id: format!("synthetic-{id}"),
        context_digest: c.digest(),
        trial_id: format!("trial-{id}"),
        session_id: format!("session-{id}"),
        role,
        label,
        views,
        teacher: None,
        provenance: CaptureProvenance::Synthetic,
    }
}
pub(crate) fn dataset(width: usize) -> ValidatedDataset {
    let c = context(width);
    let labels = [
        RoomLabel::Empty,
        RoomLabel::AdjacentOnly,
        RoomLabel::OccupiedStatic,
        RoomLabel::OccupiedMoving,
    ];
    let mut rows = Vec::new();
    for (i, label) in labels.iter().cycle().take(8).enumerate() {
        rows.push(cohort(&c, i as u64 + 1, *label, DataRole::Train));
    }
    for (i, label) in labels.iter().enumerate() {
        rows.push(cohort(&c, 100 + i as u64, *label, DataRole::Validation));
        rows.push(cohort(&c, 200 + i as u64, *label, DataRole::HeldOut));
    }
    ValidatedDataset::new(c, rows).unwrap()
}
fn config() -> FitConfig {
    FitConfig {
        epochs: 32,
        ..FitConfig::default()
    }
}

#[test]
fn prepared_training_rows_match_uncached_representation_for_parameter_probes() {
    use super::{represent_row, Row};
    for width in [8, 56, 128] {
        let ds = dataset(width);
        let (model, _) = RoomModel::fit(
            &ds,
            FitConfig {
                epochs: 1,
                hidden_dim: if width == 128 { 2 } else { 3 },
                embedding_dim: if width == 128 { 2 } else { 3 },
                ..config()
            },
        )
        .unwrap();
        let parameters = model.parameters();
        for cohort in ds.train() {
            let tokens = model.tokens(cohort, false).unwrap();
            let prepared = Row::prepare(&model, tokens.clone(), 0.0, true).unwrap();
            // Include the unchanged parameters and every positive/negative
            // finite-difference probe, across backbone, projectors and head.
            for probe in 0..=parameters.len() * 2 {
                let mut params = parameters.clone();
                if probe > 0 {
                    let index = (probe - 1) / 2;
                    params[index] += if probe % 2 == 1 { 0.002 } else { -0.002 };
                }
                let expected = model.represent(&tokens, &params).unwrap();
                let mut encoder = extractor(&model.config);
                encoder
                    .transformer
                    .unflatten_weights(&params[..model.encoder_weights.len()])
                    .unwrap();
                let projectors = model.source_projectors(&params).unwrap();
                let actual = represent_row(&model, &prepared, &mut encoder, &projectors).unwrap();
                let bits = |values: &[f32]| values.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
                assert_eq!(
                    bits(&actual.0),
                    bits(&expected.0),
                    "width={width} probe={probe}"
                );
                assert_eq!(
                    actual.1.iter().map(|x| bits(x)).collect::<Vec<_>>(),
                    expected.1.iter().map(|x| bits(x)).collect::<Vec<_>>(),
                    "width={width} probe={probe}"
                );
            }
        }
    }
}

#[test]
fn prepared_training_rows_reject_wrong_source_time_width_and_nonfinite_values() {
    use super::Row;
    let ds = dataset(8);
    let (model, _) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 1,
            ..config()
        },
    )
    .unwrap();
    let valid = model.tokens(&ds.train()[0], false).unwrap();
    let mut wrong_source = valid.clone();
    wrong_source.pop();
    let mut wrong_time = valid.clone();
    wrong_time[0].pop();
    let mut wrong_width = valid.clone();
    wrong_width[0][0].pop();
    let mut nonfinite = valid.clone();
    nonfinite[0][0][0] = f32::NAN;
    for invalid in [wrong_source, wrong_time, wrong_width, nonfinite] {
        assert!(Row::prepare(&model, invalid, 0.0, true).is_err());
    }
}

#[test]
fn actual_encoder_and_head_weights_reduce_declared_objective_and_learn_controls() {
    let ds = dataset(8);
    let (model, report) = RoomModel::fit(&ds, config()).unwrap();
    eprintln!("SYNTHETIC report: {report:?}");
    assert!(report.completed_epochs > 0);
    assert!(
        report.encoder_delta_l2 > 1.0e-5,
        "zero-gradient/skipped-encoder mutant survived"
    );
    assert!(
        report.head_delta_l2 > 1.0e-3,
        "skipped task update mutant survived"
    );
    assert!(report.final_objective.total < report.initial.total * 0.8);
    assert!(report.final_objective.supervised < report.initial.supervised * 0.5);
    let held = model.evaluate(ds.held_out()).unwrap();
    assert_eq!(
        held.correct, 4,
        "SYNTHETIC disjoint occupancy/adjacent controls: {held:?}"
    );
    assert_eq!(held.abstained, 0);
    assert!(!model.infer(&ds.held_out()[0]).physically_validated);
}
#[test]
fn held_out_and_validation_changes_cannot_update_weights_or_training_profiles() {
    let ds = dataset(8);
    let mut rows = ds.train().to_vec();
    for c in ds.validation().iter().chain(ds.held_out()) {
        let mut other = c.clone();
        other.label = RoomLabel::OccupiedMoving;
        for v in &mut other.views {
            for frame in &mut v.samples {
                for value in frame {
                    *value += 0.05;
                }
            }
        }
        rows.push(other);
    }
    let changed = ValidatedDataset::new(ds.context().clone(), rows).unwrap();
    let cfg = FitConfig {
        epochs: 8,
        ..config()
    };
    let (a, ar) = RoomModel::fit(&ds, cfg.clone()).unwrap();
    let (b, br) = RoomModel::fit(&changed, cfg).unwrap();
    assert_eq!(a.parameters(), b.parameters());
    assert_eq!(a.profiles(), b.profiles());
    assert_eq!(a.training_manifest(), b.training_manifest());
    assert_eq!(a.export().unwrap(), b.export().unwrap());
    assert_eq!(ar.final_objective.total, br.final_objective.total);
}
#[test]
fn source_order_dc_and_full_56_bin_grid_reach_real_aether_encoder() {
    let ds = dataset(56);
    let (model, report) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 2,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(model.encoder_config().bins, 56);
    assert!(report.parameter_count <= 512);
    assert!(report.encoder_delta_l2 > 0.0);
    let row = &ds.held_out()[1];
    let tokens = model.tokens(row, false).unwrap();
    assert_eq!(tokens[0][0].len(), 58);
    assert_eq!(&tokens[0][0][56..], &[1.0, 0.0]);
    assert_eq!(&tokens[1][0][56..], &[0.0, 1.0]);
    // Individual native frequency bins survive; no fixed8-bin summary is used.
    let mut changed = row.clone();
    changed.views[0].samples[0][55] += 0.05;
    let changed_tokens = model.tokens(&changed, false).unwrap();
    assert_eq!(tokens[0][0][54], changed_tokens[0][0][54]);
    assert_ne!(tokens[0][0][55], changed_tokens[0][0][55]);
    let (_, a) = model.represent(&tokens, &model.parameters()).unwrap();
    let (_, b) = model
        .represent(&changed_tokens, &model.parameters())
        .unwrap();
    let backbone = |input: &[Vec<f32>]| {
        let mut encoder = extractor(model.encoder_config());
        encoder
            .transformer
            .unflatten_weights(&model.encoder_weights)
            .unwrap();
        let mut input = input.to_vec();
        for row in &mut input {
            for x in &mut row[..56] {
                *x /= model.profiles()[0].max_normalized;
            }
        }
        encoder.transformer.embed(&input)
    };
    assert_ne!(
        backbone(&tokens[0]),
        backbone(&changed_tokens[0]),
        "canonical bin must reach shared RF backbone"
    );
    assert_eq!(a[1], b[1], "source0 cannot erase distinct source1 encoding");
    let empty = model.tokens(&ds.held_out()[0], false).unwrap();
    let occupied = model.tokens(&ds.held_out()[2], false).unwrap();
    assert!(
        occupied[0][0][0] - empty[0][0][0] > 1.0,
        "per-window normalization erased DC"
    );
}
#[test]
fn contrastive_objective_updates_encoder_and_is_not_a_logged_task_loss() {
    let ds = dataset(8);
    let (contrast, cr) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 16,
            contrastive_weight: 5.0,
            supervised_weight: 0.001,
            ..config()
        },
    )
    .unwrap();
    let (task, _) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 16,
            contrastive_weight: 0.001,
            ..config()
        },
    )
    .unwrap();
    eprintln!(
        "SYNTHETIC contrastive objective {} -> {}",
        cr.initial.contrastive, cr.final_objective.contrastive
    );
    assert!(cr.final_objective.contrastive < cr.initial.contrastive - 0.001);
    assert!(cr.encoder_delta_l2 > 1.0e-4);
    assert_ne!(contrast.encoder_weights, task.encoder_weights);
}
#[test]
fn model_round_trip_has_identical_predictions_and_no_observation_payload() {
    let ds = dataset(8);
    let (model, _) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 3,
            ..config()
        },
    )
    .unwrap();
    let bytes = model.export().unwrap();
    let json = String::from_utf8(bytes.clone()).unwrap();
    for raw in [
        "samples",
        "frame_ids",
        "sequence_ids",
        "trial_id",
        "synthetic-200",
    ] {
        assert!(!json.contains(raw));
    }
    let imported = RoomModel::import(&bytes).unwrap();
    assert_eq!(model.parameters(), imported.parameters());
    assert_eq!(
        model.infer(&ds.held_out()[2]).occupied_probability,
        imported.infer(&ds.held_out()[2]).occupied_probability
    );
    let original: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for mutant in [
        "version",
        "encoder_weights",
        "head_weights",
        "profiles",
        "config",
        "algorithm",
    ] {
        let mut value = original.clone();
        value[mutant] = serde_json::json!(null);
        assert!(
            RoomModel::import(&serde_json::to_vec(&value).unwrap()).is_err(),
            "accepted malformed {mutant}"
        );
    }
    let mut invalid = model.clone();
    invalid.encoder_weights[0] = f32::NAN;
    assert!(invalid.export().is_err());
    assert!(RoomModel::import(&vec![b' '; 1_048_577]).is_err());
}
#[test]
fn missing_changed_stale_nonfinite_and_out_of_support_sources_abstain() {
    let ds = dataset(8);
    let (model, _) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 2,
            ..config()
        },
    )
    .unwrap();
    type Mutant = fn(&mut CaptureCohort);
    let mutants: &[Mutant] = &[
        |c| {
            c.views.pop();
        },
        |c| c.context_digest = "different-installation".into(),
        |c| c.views[0].device_id = "different-device".into(),
        |c| c.views[0].grid_id = "other-phy".into(),
        |c| c.views[0].samples[0][0] = f32::NAN,
        |c| c.views[0].samples[0][0] = 1.0e8,
        |c| c.views[0].sample_age_ns = MAX_AGE_NS + 1,
    ];
    for mutate in mutants {
        let mut row = ds.held_out()[0].clone();
        mutate(&mut row);
        let prediction = model.infer(&row);
        assert_eq!(prediction.decision, PresenceDecision::Abstain);
        assert!(prediction.occupied_probability.is_none());
    }
}
#[test]
fn parameter_time_evaluation_budgets_and_cancellation_fail_closed() {
    let ds = dataset(8);
    assert!(RoomModel::fit(
        &ds,
        FitConfig {
            max_parameters: 1,
            ..config()
        }
    )
    .unwrap_err()
    .contains("parameter budget"));
    assert!(RoomModel::fit(
        &ds,
        FitConfig {
            max_objective_evaluations: 2,
            ..config()
        }
    )
    .unwrap_err()
    .contains("evaluation budget"));
    assert!(RoomModel::fit(
        &ds,
        FitConfig {
            max_duration_ms: 1,
            ..config()
        }
    )
    .unwrap_err()
    .contains("time budget"));
    assert!(
        RoomModel::fit_with_cancel(&ds, config(), &AtomicBool::new(true))
            .unwrap_err()
            .contains("cancelled")
    );
    for cfg in [
        FitConfig {
            learning_rate: f32::NAN,
            ..config()
        },
        FitConfig {
            epochs: 129,
            ..config()
        },
        FitConfig {
            hidden_dim: 5000,
            ..config()
        },
    ] {
        assert!(RoomModel::fit(&ds, cfg).is_err());
    }
}

#[test]
fn sibling_trials_shared_frames_and_overlapping_windows_never_become_negatives() {
    let ds = dataset(8);
    let mut rows = ds.train().to_vec();
    rows[1].trial_id = rows[0].trial_id.clone();
    for i in 0..2 {
        rows[2].views[i].first_time_ns = rows[0].views[i].first_time_ns;
        rows[2].views[i].last_time_ns = rows[0].views[i].last_time_ns;
        rows[3].views[i].frame_ids[0] = rows[0].views[i].frame_ids[0];
    }
    let ds = ValidatedDataset::new(ds.context().clone(), rows).unwrap();
    let (_, report) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 1,
            ..config()
        },
    )
    .unwrap();
    assert_eq!(report.training_cohorts, 8);
    assert_eq!(report.contrastive_trials, 5);
}
#[test]
fn running_fit_observes_cancellation_without_publishing_partial_model() {
    use std::{
        sync::{atomic::Ordering, Arc},
        thread,
        time::Duration,
    };
    let ds = dataset(8);
    let cancel = Arc::new(AtomicBool::new(false));
    let trigger = cancel.clone();
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(2));
        trigger.store(true, Ordering::SeqCst);
    });
    let result = RoomModel::fit_with_cancel(
        &ds,
        FitConfig {
            epochs: 128,
            ..config()
        },
        &cancel,
    );
    handle.join().unwrap();
    assert!(result.unwrap_err().contains("cancelled"));
}

#[test]
fn inference_does_not_invent_empty_ground_truth_or_enter_evaluation() {
    let ds = dataset(8);
    let (model, _) = RoomModel::fit(
        &ds,
        FitConfig {
            epochs: 2,
            ..config()
        },
    )
    .unwrap();
    let mut live = ds.held_out()[2].clone();
    let expected = model.infer(&live).occupied_probability;
    live.role = DataRole::Inference;
    live.label = RoomLabel::Unknown;
    assert_eq!(model.infer(&live).occupied_probability, expected);
    assert!(model
        .evaluate(&[live.clone()])
        .unwrap_err()
        .contains("labeled"));
    assert!(ValidatedDataset::new(ds.context().clone(), vec![live]).is_err());
}
