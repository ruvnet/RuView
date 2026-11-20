//! Exact count boundaries and Rust Value/JSON wire contracts; synthetic diagnostics only.
use super::*;
use crate::room_learner::training::{BinaryConfusion, ConditionEvaluation, EvaluationRates};
fn rates(c: &BinaryConfusion) -> EvaluationRates {
    let scored = c.true_empty + c.false_occupied + c.false_empty + c.true_occupied;
    let n = scored + c.abstained_empty + c.abstained_occupied;
    let ratio = |a: usize, b: usize| {
        if b == 0 {
            None
        } else {
            Some(a as f32 / b as f32)
        }
    };
    EvaluationRates {
        evaluated_cohort_coverage: ratio(scored, n),
        unconditional_accuracy: ratio(c.true_empty + c.true_occupied, n),
        scored_accuracy: ratio(c.true_empty + c.true_occupied, scored),
        unconditional_sensitivity: ratio(
            c.true_occupied,
            c.true_occupied + c.false_empty + c.abstained_occupied,
        ),
        scored_sensitivity: ratio(c.true_occupied, c.true_occupied + c.false_empty),
        unconditional_specificity: ratio(
            c.true_empty,
            c.true_empty + c.false_occupied + c.abstained_empty,
        ),
        scored_specificity: ratio(c.true_empty, c.true_empty + c.false_occupied),
    }
}
fn condition(label: RoomLabel, c: BinaryConfusion) -> ConditionEvaluation {
    let scored = c.true_empty + c.false_occupied + c.false_empty + c.true_occupied;
    let abstained = c.abstained_empty + c.abstained_occupied;
    ConditionEvaluation {
        condition: label,
        samples: scored + abstained,
        correct: c.true_empty + c.true_occupied,
        abstained,
        scored,
        missing: c.missing_empty + c.missing_occupied,
        binary_cross_entropy: Some(0.0),
        binary_cross_entropy_samples: scored + abstained,
        declared_trial_count: 1,
        declared_session_count: 1,
        rates: rates(&c),
        confusion: c,
    }
}
fn report(
    false_positive: usize,
    occupied_correct: usize,
    occupied_abstained: usize,
) -> EvaluationReport {
    let conditions = vec![
        condition(
            RoomLabel::Empty,
            BinaryConfusion {
                true_empty: 40 - false_positive,
                false_occupied: false_positive,
                ..Default::default()
            },
        ),
        condition(
            RoomLabel::OccupiedStatic,
            BinaryConfusion {
                true_occupied: occupied_correct,
                false_empty: 10 - occupied_correct - occupied_abstained,
                abstained_occupied: occupied_abstained,
                ..Default::default()
            },
        ),
        condition(
            RoomLabel::OccupiedMoving,
            BinaryConfusion {
                true_occupied: 4,
                ..Default::default()
            },
        ),
        condition(
            RoomLabel::AdjacentOnly,
            BinaryConfusion {
                true_empty: 4,
                ..Default::default()
            },
        ),
    ];
    let mut sum = BinaryConfusion::default();
    for c in &conditions {
        sum.true_empty += c.confusion.true_empty;
        sum.false_occupied += c.confusion.false_occupied;
        sum.true_occupied += c.confusion.true_occupied;
        sum.false_empty += c.confusion.false_empty;
        sum.abstained_occupied += c.confusion.abstained_occupied;
    }
    let report = EvaluationReport {
        samples: conditions.iter().map(|c| c.samples).sum(),
        correct: conditions.iter().map(|c| c.correct).sum(),
        abstained: conditions.iter().map(|c| c.abstained).sum(),
        scored: conditions.iter().map(|c| c.scored).sum(),
        missing: 0,
        binary_cross_entropy: Some(0.0),
        binary_cross_entropy_samples: conditions
            .iter()
            .map(|c| c.binary_cross_entropy_samples)
            .sum(),
        declared_trial_count: 4,
        declared_session_count: 4,
        rates: rates(&sum),
        confusion: sum,
        per_condition: conditions,
        independence_verified: false,
    };
    report.validate().unwrap();
    report
}
#[test]
fn exact_ninety_percent_coverage_and_sensitivity_and_five_percent_false_positives_pass() {
    let checks = criteria(&report(2, 9, 1), &[4; 4]).unwrap();
    assert_eq!(checks[0].false_positive_rate, Some(0.05));
    assert_eq!(checks[1].evaluated_cohort_coverage, 0.9);
    assert_eq!(checks[1].unconditional_sensitivity, Some(0.9));
    let wire = serde_json::to_value(&checks).unwrap();
    assert_eq!(wire[0]["false_positive_rate"], serde_json::json!(0.05));
    assert_eq!(wire[1]["evaluated_cohort_coverage"], serde_json::json!(0.9));
    assert_eq!(wire[1]["unconditional_sensitivity"], serde_json::json!(0.9));
    let json = serde_json::to_string(&wire).unwrap();
    assert!(!json.contains("0.899999") && !json.contains("0.050000000"));
}
#[test]
fn below_threshold_coverage_sensitivity_and_excess_false_positives_fail_separately() {
    assert!(criteria(&report(2, 8, 2), &[4; 4])
        .unwrap_err()
        .contains("coverage"));
    assert!(criteria(&report(2, 8, 0), &[4; 4])
        .unwrap_err()
        .contains("sensitivity"));
    assert!(criteria(&report(3, 9, 1), &[4; 4])
        .unwrap_err()
        .contains("false presence"));
}
#[test]
fn policy_is_canonical_through_json_value_and_direct_hash_encoding() {
    let p = QualificationPolicy::default();
    let value = serde_json::to_value(&p).unwrap();
    assert_eq!(
        value["minimum_unconditional_sensitivity"],
        serde_json::json!(0.9)
    );
    assert_eq!(
        value["minimum_evaluated_cohort_coverage"],
        serde_json::json!(0.9)
    );
    assert_eq!(
        value["maximum_false_positive_rate"],
        serde_json::json!(0.05)
    );
    let encoded = serde_json::to_string(&p).unwrap();
    let expected = r#"{"id":"ruview.local-operational-policy.v1","minimum_cohorts_per_condition":4,"minimum_unconditional_sensitivity":0.9,"maximum_false_positive_rate":0.05,"minimum_evaluated_cohort_coverage":0.9,"coverage_scope":"representative_cohorts_not_wall_time"}"#;
    assert_eq!(encoded, expected);
    assert_eq!(
        hash(&p).unwrap(),
        format!("{:x}", Sha256::digest(expected.as_bytes()))
    );
    assert_eq!(
        serde_json::from_value::<QualificationPolicy>(value).unwrap(),
        p
    );
}
