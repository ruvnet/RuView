//! SYNTHETIC metrics and denominator oracles; no sensing accuracy claim.
use super::*;
use crate::room_learner::training::{learning_tests::dataset, FitConfig};
use std::sync::OnceLock;
fn model() -> RoomModel {
    static MODEL: OnceLock<RoomModel> = OnceLock::new();
    MODEL
        .get_or_init(|| {
            RoomModel::fit(
                &dataset(8),
                FitConfig {
                    epochs: 2,
                    ..FitConfig::default()
                },
            )
            .unwrap()
            .0
        })
        .clone()
}
#[test]
fn actual_evaluation_retains_legacy_fields_and_has_all_declared_conditions() {
    let ds = dataset(8);
    let report = model().evaluate(ds.held_out()).unwrap();
    assert_eq!(report.samples, 4);
    assert_eq!(report.declared_trial_count, 4);
    assert_eq!(report.per_condition.len(), 4);
    assert_eq!(report.scored + report.abstained, report.samples);
    assert_eq!(report.binary_cross_entropy_samples, 4);
    assert!(report.binary_cross_entropy.unwrap().is_finite());
    assert!(!report.independence_verified);
    report.validate().unwrap();
    let json = serde_json::to_value(&report).unwrap();
    for key in ["samples", "correct", "abstained", "binary_cross_entropy"] {
        assert!(json.get(key).is_some());
    }
    for condition in report.per_condition {
        assert_eq!(condition.samples, 1);
        assert_eq!(condition.declared_trial_count, 1);
    }
}
#[test]
fn windows_are_not_distinct_declared_trials_and_missing_positives_reduce_unconditional_recall() {
    let ds = dataset(8);
    let mut windows = vec![ds.held_out()[2].clone(); 4];
    for (i, row) in windows.iter_mut().enumerate() {
        row.id = format!("window-{i}");
    }
    let report = model().evaluate(&windows).unwrap();
    assert_eq!(report.samples, 4);
    assert_eq!(report.declared_trial_count, 1);
    assert_eq!(report.declared_session_count, 1);
    let mut missing = ds.held_out()[2].clone();
    missing.views.clear();
    let report = model().evaluate(&[missing]).unwrap();
    assert_eq!(
        (
            report.samples,
            report.correct,
            report.abstained,
            report.missing
        ),
        (1, 0, 1, 1)
    );
    assert_eq!(report.rates.evaluated_cohort_coverage, Some(0.0));
    assert_eq!(report.rates.unconditional_sensitivity, Some(0.0));
    assert_eq!(report.rates.scored_sensitivity, None);
    assert_eq!(report.binary_cross_entropy, None);
    assert_eq!(report.binary_cross_entropy_samples, 0);
    assert_eq!(report.rates.unconditional_specificity, None);
}
#[test]
fn abstentions_are_in_unconditional_sensitivity_and_absent_conditions_are_null() {
    let ds = dataset(8);
    let row = &ds.held_out()[2];
    let mut acc = Accumulator::default();
    acc.observe(
        row,
        &RoomPrediction {
            decision: PresenceDecision::Occupied,
            occupied_probability: Some(0.9),
            reason: None,
            physically_validated: false,
        },
    );
    acc.observe(
        row,
        &RoomPrediction {
            decision: PresenceDecision::Abstain,
            occupied_probability: Some(0.5),
            reason: None,
            physically_validated: false,
        },
    );
    acc.observe(
        row,
        &RoomPrediction {
            decision: PresenceDecision::Abstain,
            occupied_probability: None,
            reason: None,
            physically_validated: false,
        },
    );
    let summary = acc.condition(RoomLabel::OccupiedStatic);
    assert_eq!(summary.rates.unconditional_sensitivity, Some(1.0 / 3.0));
    assert_eq!(summary.rates.scored_sensitivity, Some(1.0));
    assert_eq!(summary.rates.evaluated_cohort_coverage, Some(1.0 / 3.0));
    assert_eq!(summary.binary_cross_entropy_samples, 2);
    assert_eq!(summary.abstained, 2);
    assert_eq!(summary.missing, 1);
    let empty = Accumulator::default().condition(RoomLabel::Empty);
    assert_eq!(empty.rates.evaluated_cohort_coverage, None);
    assert_eq!(empty.rates.unconditional_accuracy, None);
    assert_eq!(empty.binary_cross_entropy, None);
}
#[test]
fn unknown_train_inference_mixed_roles_wrong_context_and_duplicate_windows_are_rejected() {
    let ds = dataset(8);
    let model = model();
    assert!(model.evaluate(ds.train()).is_err());
    let mut row = ds.held_out()[0].clone();
    row.label = RoomLabel::Unknown;
    assert!(model.evaluate(&[row.clone()]).is_err());
    row.role = DataRole::Inference;
    assert!(model.evaluate(&[row]).is_err());
    assert!(model
        .evaluate(&[ds.validation()[0].clone(), ds.held_out()[0].clone()])
        .is_err());
    assert!(model
        .evaluate(&[ds.held_out()[0].clone(), ds.held_out()[0].clone()])
        .is_err());
    let mut wrong = ds.held_out()[0].clone();
    wrong.context_digest = "another-room".into();
    assert!(model.evaluate(&[wrong]).is_err());
}
#[test]
fn tampered_metrics_nonfinite_losses_and_inflated_claims_fail_validation() {
    let report = model().evaluate(dataset(8).held_out()).unwrap();
    let mut bad = report.clone();
    bad.correct = bad.samples + 1;
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.independence_verified = true;
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.binary_cross_entropy = Some(f32::NAN);
    assert!(bad.validate().is_err());
    let mut bad = report.clone();
    bad.per_condition[0].rates.evaluated_cohort_coverage = Some(1.5);
    assert!(bad.validate().is_err());
    let mut bad = report;
    bad.confusion.true_empty = usize::MAX;
    assert!(bad.validate().is_err());
}

fn sixteen_declared_windows() -> Vec<CaptureCohort> {
    let base = dataset(8);
    let mut rows = Vec::new();
    for condition in base.held_out() {
        for window in 0..4 {
            let mut row = condition.clone();
            row.id = format!("{}-window-{window}", row.id);
            rows.push(row);
        }
    }
    rows
}
#[test]
fn impossible_declared_union_totals_are_rejected_without_rejecting_shared_trials() {
    let rows = sixteen_declared_windows();
    let report = model().evaluate(&rows).unwrap();
    assert_eq!(report.samples, 16);
    assert_eq!(report.declared_trial_count, 4);
    assert_eq!(
        report
            .per_condition
            .iter()
            .map(|p| p.declared_trial_count)
            .sum::<usize>(),
        4
    );
    let mut impossible = report.clone();
    impossible.declared_trial_count = 16;
    assert!(
        impossible.validate().is_err(),
        "global trials cannot exceed per-condition union bound"
    );
    let mut impossible = report;
    impossible.declared_session_count = 16;
    assert!(
        impossible.validate().is_err(),
        "global sessions cannot exceed per-condition union bound"
    );
    let shared = rows
        .into_iter()
        .map(|mut row| {
            row.trial_id = "shared-cross-condition-trial".into();
            row.session_id = "shared-cross-condition-session".into();
            row
        })
        .collect::<Vec<_>>();
    let shared = model().evaluate(&shared).unwrap();
    assert_eq!(shared.declared_trial_count, 1);
    assert_eq!(shared.declared_session_count, 1);
    shared.validate().unwrap();
}
#[test]
fn nonmissing_abstention_probabilities_must_all_contribute_to_bce() {
    let mut json = serde_json::to_value(model()).unwrap();
    for value in json["head_weights"].as_array_mut().unwrap() {
        *value = serde_json::json!(0.0);
    }
    let uncertain = RoomModel::import(&serde_json::to_vec(&json).unwrap()).unwrap();
    let mut report = uncertain.evaluate(&sixteen_declared_windows()).unwrap();
    assert_eq!(
        (
            report.samples,
            report.scored,
            report.missing,
            report.binary_cross_entropy_samples
        ),
        (16, 0, 0, 16)
    );
    report.binary_cross_entropy = None;
    report.binary_cross_entropy_samples = 0;
    for part in &mut report.per_condition {
        part.binary_cross_entropy = None;
        part.binary_cross_entropy_samples = 0;
    }
    assert!(
        report.validate().is_err(),
        "nonmissing uncertain probabilities cannot disappear from BCE"
    );
}
