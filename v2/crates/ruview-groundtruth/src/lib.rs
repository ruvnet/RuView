//! # `ruview-groundtruth` — reference sensors as a formal validation plane (ADR-303)
//!
//! This crate generalizes the ADR-293 vitals ground-truth rig from a single
//! measurand to **any** phenomenon RuView senses (presence, count, range,
//! posture, activity, heart rate, breathing rate) and **any** reference
//! modality (camera, mmWave, pressure mat, wearable, pulse oximeter,
//! microphone, manual label). Its defining design decision (ADR-303) is that
//! reference sensors are a **validation plane, never inference inputs**: this
//! crate compares RF estimates against independent observation and never hands
//! a reference reading back to an estimator.
//!
//! ## Pipeline
//!
//! ```text
//! ReferenceObservation… ─► ReferenceSeries ─┐
//!                                            ├─► estimate_alignment (constant
//! EstimateSeries (RF, real|synthetic) ───────┘        offset, bounded xcorr on
//!                                                      a common grid) ─► Alignment
//!        │
//!        └─► AgreementReport::build(scope, cfg, tolerance, policy)
//!              ├─ n pairs, coverage, MAE/RMSE/bias | label-agreement
//!              ├─ mandatory SessionScope (subjects, motion, LOS, distance)
//!              ├─ EvidenceGrade (Measured|Claimed|Synthetic)
//!              └─ to_evidence_record → ruview_evidence ledger
//! ```
//!
//! Radar reference logs (one JSON summary per window) enter through
//! [`RadarLabels`], which turns each window into presence, count and range
//! labels with an uncertainty and a reason when a window cannot be labelled.
//! [`auto_label`] pairs those labels with a RuView recording to make
//! occupancy training windows; [`session_disjoint_split`] and
//! [`majority_baseline`] give the split and the baseline any accuracy built
//! on them must be reported against (see `examples/radar_autolabel.rs`).
//!
//! ## Honesty and determinism
//!
//! - **Canonical vocabulary (ADR-300 rule 3):** the report speaks the
//!   [`ruview_ontology`] evidence ladder ([`EvidenceLevel`]) and writes an
//!   [`ruview_evidence`] record — no per-crate reinvention of evidence shapes.
//! - **UNKNOWN is first-class (ADR-300 rule 1):** insufficient overlap yields a
//!   report with zero pairs and a non-MEASURED grade, and an uncomputable
//!   alignment score is `None` — never an error, never a fabricated number.
//! - **Deterministic:** no wall clock and no randomness. All timestamps are
//!   injected; the alignment search and metrics are pure functions of the
//!   inputs.
//! - **Bounded & validated:** every reference/estimate is validated at the
//!   boundary (monotonic timestamps, finite scalars, matching reading family)
//!   and sample/grid/lag counts are capped so malformed input cannot exhaust
//!   memory.
//! - **Grade in types (ADR-293/301):** `Measured` requires an independent
//!   reference, coverage, paired samples, and a reproducer; synthetic input is
//!   `Synthetic`/L0 by construction and cannot be raised.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod agreement;
mod align;
mod autolabel;
mod baseline;
mod error;
mod model;
mod radar;
mod scope;
mod series;
mod source;

pub use agreement::{AgreementMetrics, AgreementReport, EvidenceGrade, GradingPolicy};
pub use align::{
    estimate_alignment, Alignment, AlignmentConfig, MAX_GRID_POINTS, MAX_LAG_STEPS,
};
pub use autolabel::{
    auto_label, parse_recording_jsonl, AutoLabelConfig, AutoLabelOutput, CsiFrame,
    CsiWindowSummary, LabelledWindow, Occupancy, SkipReason, MAX_RECORDING_LINE_BYTES,
};
pub use baseline::{majority_baseline, session_disjoint_split, BaselineReport, SessionSplit};
pub use error::{GroundTruthError, MAX_STR_LEN};
pub use model::{DataProvenance, Measurand, Reading, ReadingKind};
pub use radar::{
    label_window, parse_radar_jsonl, LabelStatus, LabelUncertainty, PresenceBasis,
    RadarAdapterConfig, RadarLabel, RadarLabels, RadarTarget, RadarWindow, RadarWindowSource,
    UnknownReason, LABEL_ABSENT, LABEL_PRESENT, MAX_RADAR_LINE_BYTES, MAX_RADAR_TARGETS,
};
pub use scope::{DistanceBand, LineOfSight, MotionState, SessionScope, MAX_SUBJECTS};
pub use series::{EstimateSeries, ReferenceObservation, ReferenceSeries, MAX_SAMPLES};
pub use source::{ReferenceModality, ReferenceSource};

// The canonical evidence ladder is the ontology's, re-exported so downstream
// crates use one vocabulary (ADR-300 rule 3).
pub use ruview_ontology::EvidenceLevel;

#[cfg(test)]
mod tests {
    use super::*;
    use ruview_evidence::{
        AccuracyMetrics, EvidenceContext, EvidenceLedger, ProvenanceClass,
        EvidenceLevel as LedgerLevel,
    };

    fn src() -> ReferenceSource {
        ReferenceSource::new(
            ReferenceModality::Wearable,
            "chest-strap-A",
            "Polar H10",
            "ecg",
        )
        .unwrap()
    }

    fn scope() -> SessionScope {
        SessionScope::new(
            1,
            MotionState::Static,
            LineOfSight::Los,
            DistanceBand::Near,
        )
        .unwrap()
    }

    fn measured_policy() -> GradingPolicy {
        GradingPolicy::new(0.5, EvidenceLevel::L3, Some("cargo test -p ruview-groundtruth".into()))
            .unwrap()
    }

    fn scalar_series_est(measurand: Measurand, prov: DataProvenance, vals: &[(i64, f64)]) -> EstimateSeries {
        let samples = vals
            .iter()
            .map(|&(t, v)| ReferenceObservation::scalar(t, v))
            .collect();
        EstimateSeries::new(measurand, "rf-model-v1", prov, samples).unwrap()
    }

    fn scalar_series_ref(measurand: Measurand, vals: &[(i64, f64)]) -> ReferenceSeries {
        let samples = vals
            .iter()
            .map(|&(t, v)| ReferenceObservation::scalar(t, v))
            .collect();
        ReferenceSeries::new(src(), measurand, samples).unwrap()
    }

    // A distinctive, non-periodic pattern so the cross-correlation peaks
    // uniquely at the true lag (digits of pi).
    const PATTERN: [f64; 11] = [3., 1., 4., 1., 5., 9., 2., 6., 5., 3., 5.];

    #[test]
    fn alignment_recovers_known_synthetic_offset() {
        // Estimate on a 1 s grid, reference the same pattern shifted +2000 ms.
        let est_vals: Vec<(i64, f64)> = PATTERN
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 * 1000, v))
            .collect();
        let ref_vals: Vec<(i64, f64)> = PATTERN
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 * 1000 + 2000, v))
            .collect();

        let est = scalar_series_est(Measurand::HeartRateBpm, DataProvenance::Real, &est_vals);
        let refr = scalar_series_ref(Measurand::HeartRateBpm, &ref_vals);
        let cfg = AlignmentConfig {
            grid_ms: 1000,
            max_lag_ms: 5000,
            max_gap_ms: 400,
        };

        let a = estimate_alignment(&est, &refr, &cfg).unwrap();
        assert_eq!(a.offset_ms, 2000);
        // Perfect match at the true lag.
        assert!((a.score.unwrap() - 1.0).abs() < 1e-9);
        assert!(a.paired_points >= 10);
    }

    #[test]
    fn alignment_is_deterministic() {
        let est_vals: Vec<(i64, f64)> = PATTERN
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 * 1000, v))
            .collect();
        let ref_vals: Vec<(i64, f64)> = PATTERN
            .iter()
            .enumerate()
            .map(|(i, &v)| (i as i64 * 1000 + 3000, v))
            .collect();
        let est = scalar_series_est(Measurand::HeartRateBpm, DataProvenance::Real, &est_vals);
        let refr = scalar_series_ref(Measurand::HeartRateBpm, &ref_vals);
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 6000, max_gap_ms: 400 };
        let a1 = estimate_alignment(&est, &refr, &cfg).unwrap();
        let a2 = estimate_alignment(&est, &refr, &cfg).unwrap();
        assert_eq!(a1, a2);
        assert_eq!(a1.offset_ms, 3000);
    }

    #[test]
    fn continuous_agreement_matches_hand_computed_fixture() {
        // Aligned at offset 0; errors (e - r) = [-2, 1, -3].
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 10.0), (1000, 20.0), (2000, 30.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 12.0), (1000, 19.0), (2000, 33.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };

        let report = AgreementReport::build(&est, &refr, scope(), &cfg, 2.5, &measured_policy())
            .unwrap();

        assert_eq!(report.n_pairs, 3);
        assert!((report.coverage - 1.0).abs() < 1e-9);
        match report.metrics {
            AgreementMetrics::Continuous { mae, rmse, bias, within_tolerance } => {
                assert!((mae - 2.0).abs() < 1e-9); // (2+1+3)/3
                assert!((rmse - (14.0f64 / 3.0).sqrt()).abs() < 1e-9); // sqrt((4+1+9)/3)
                assert!((bias - (-4.0 / 3.0)).abs() < 1e-9); // (-2+1-3)/3
                assert!((within_tolerance - 2.0 / 3.0).abs() < 1e-9); // |−2|,|1| in, |−3| out
            }
            other => panic!("expected continuous metrics, got {other:?}"),
        }
        // Real reference + full coverage + reproducer => Measured.
        assert_eq!(report.grade, EvidenceGrade::Measured);
        assert_eq!(report.evidence_level, EvidenceLevel::L3);
    }

    #[test]
    fn categorical_label_agreement_matches_fixture() {
        let est_samples = vec![
            ReferenceObservation::label(0, "present"),
            ReferenceObservation::label(1000, "absent"),
            ReferenceObservation::label(2000, "present"),
            ReferenceObservation::label(3000, "present"),
        ];
        let ref_samples = vec![
            ReferenceObservation::label(0, "present"),
            ReferenceObservation::label(1000, "absent"),
            ReferenceObservation::label(2000, "absent"),
            ReferenceObservation::label(3000, "present"),
        ];
        let est = EstimateSeries::new(
            Measurand::Presence,
            "rf-model-v1",
            DataProvenance::Real,
            est_samples,
        )
        .unwrap();
        let refr = ReferenceSeries::new(
            ReferenceSource::new(ReferenceModality::Camera, "cam-1", "RealSense", "labels").unwrap(),
            Measurand::Presence,
            ref_samples,
        )
        .unwrap();
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };

        let report =
            AgreementReport::build(&est, &refr, scope(), &cfg, 0.0, &measured_policy()).unwrap();
        assert_eq!(report.n_pairs, 4);
        match report.metrics {
            AgreementMetrics::Categorical { agreement, n_agree } => {
                assert_eq!(n_agree, 3);
                assert!((agreement - 0.75).abs() < 1e-9);
            }
            other => panic!("expected categorical metrics, got {other:?}"),
        }
    }

    #[test]
    fn measured_report_emits_measured_evidence_record() {
        let est = scalar_series_est(
            Measurand::BreathingRateBrpm,
            DataProvenance::Real,
            &[(0, 12.0), (1000, 13.0), (2000, 12.5)],
        );
        let refr = scalar_series_ref(
            Measurand::BreathingRateBrpm,
            &[(0, 12.0), (1000, 13.0), (2000, 12.5)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        let report =
            AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &measured_policy()).unwrap();
        assert_eq!(report.grade, EvidenceGrade::Measured);

        let ctx = EvidenceContext::new("space-kitchen", "dev-esp32-A", "adult", "rf-model-v1")
            .unwrap();
        let metrics = AccuracyMetrics {
            moving_recall: 0.9,
            stationary_recall: 0.95,
            false_positive_rate: 0.02,
            drift: 0.05,
            uncertainty: 0.1,
            calibration_age_secs: 600,
            sample_count: report.n_pairs as u64,
        };
        let record = report
            .to_evidence_record(ctx.clone(), metrics, 1_700_000_000_000_000)
            .unwrap();
        assert_eq!(record.class(), ProvenanceClass::Measured);
        assert_eq!(record.level(), LedgerLevel::L3);
        assert!(!record.reproducer().is_empty());

        let mut ledger = EvidenceLedger::new();
        let seq = ledger.append(record).unwrap();
        assert_eq!(seq, 0);
        assert_eq!(ledger.query(&ctx).len(), 1);
    }

    #[test]
    fn synthetic_report_emits_l0_synthetic_record() {
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Synthetic,
            &[(0, 60.0), (1000, 61.0), (2000, 62.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 60.0), (1000, 61.0), (2000, 62.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        let report =
            AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &measured_policy()).unwrap();
        // Synthetic input can never be MEASURED, regardless of coverage.
        assert_eq!(report.grade, EvidenceGrade::Synthetic);
        assert_eq!(report.evidence_level, EvidenceLevel::L0);

        let ctx = EvidenceContext::new("space-lab", "dev-sim", "", "rf-model-v1").unwrap();
        let metrics = AccuracyMetrics {
            moving_recall: 1.0,
            stationary_recall: 1.0,
            false_positive_rate: 0.0,
            drift: 0.0,
            uncertainty: 0.0,
            calibration_age_secs: 0,
            sample_count: 3,
        };
        let record = report
            .to_evidence_record(ctx, metrics, 1_700_000_000_000_000)
            .unwrap();
        assert_eq!(record.class(), ProvenanceClass::Synthetic);
        assert_eq!(record.level(), LedgerLevel::L0);
    }

    #[test]
    fn real_data_without_reproducer_grades_claimed() {
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 70.0), (1000, 71.0), (2000, 72.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 70.0), (1000, 71.0), (2000, 72.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        // No reproducer => cannot be Measured even with a real reference.
        let policy = GradingPolicy::new(0.5, EvidenceLevel::L2, None).unwrap();
        let report = AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &policy).unwrap();
        assert_eq!(report.grade, EvidenceGrade::Claimed);
        assert_eq!(report.evidence_level, EvidenceLevel::L2);
        assert!(report.reproducer.is_none());

        let ctx = EvidenceContext::new("space-kitchen", "dev-esp32-A", "adult", "rf-model-v1")
            .unwrap();
        let metrics = AccuracyMetrics {
            moving_recall: 0.8,
            stationary_recall: 0.9,
            false_positive_rate: 0.05,
            drift: 0.1,
            uncertainty: 0.2,
            calibration_age_secs: 100,
            sample_count: 3,
        };
        let record = report.to_evidence_record(ctx, metrics, 1).unwrap();
        assert_eq!(record.class(), ProvenanceClass::Claimed);
    }

    #[test]
    fn low_coverage_grades_claimed_not_measured() {
        // Reference far from the estimate grid: nearest-sample gap exceeds
        // max_gap for most points, so coverage falls below the threshold.
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 60.0), (1000, 61.0), (2000, 62.0), (3000, 63.0)],
        );
        // Reference has a single usable sample near t=0 and a distant gap.
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 60.0), (9000, 99.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        let policy = GradingPolicy::new(0.9, EvidenceLevel::L3, Some("repro".into())).unwrap();
        let report = AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &policy).unwrap();
        assert!(report.coverage < 0.9);
        assert_eq!(report.grade, EvidenceGrade::Claimed);
    }

    #[test]
    fn no_overlap_is_unknown_not_error() {
        // Estimate and reference ranges do not overlap even after the bounded
        // lag search — a first-class UNKNOWN report, not an error.
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 60.0), (1000, 61.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(1_000_000, 60.0), (1_001_000, 61.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 2000, max_gap_ms: 400 };
        let report =
            AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &measured_policy()).unwrap();
        assert_eq!(report.n_pairs, 0);
        assert_eq!(report.coverage, 0.0);
        assert!(report.alignment.score.is_none());
        assert_eq!(report.grade, EvidenceGrade::Claimed);
    }

    #[test]
    fn scope_is_mandatory_and_bounded() {
        // SessionScope::new rejects an absurd subject count at the boundary.
        let err = SessionScope::new(
            MAX_SUBJECTS + 1,
            MotionState::Moving,
            LineOfSight::Nlos,
            DistanceBand::Far,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            GroundTruthError::SubjectCountTooLarge { .. }
        ));
        // An empty-room session (0 subjects) is valid.
        assert!(SessionScope::new(0, MotionState::Static, LineOfSight::Los, DistanceBand::Near)
            .is_ok());
    }

    #[test]
    fn ingest_rejects_malformed_series() {
        // Non-monotonic timestamps.
        let err = ReferenceSeries::new(
            src(),
            Measurand::HeartRateBpm,
            vec![
                ReferenceObservation::scalar(1000, 60.0),
                ReferenceObservation::scalar(1000, 61.0),
            ],
        )
        .unwrap_err();
        assert!(matches!(err, GroundTruthError::NonMonotonic { index: 1, .. }));

        // Reading family mismatched to the measurand.
        let err = ReferenceSeries::new(
            src(),
            Measurand::HeartRateBpm,
            vec![ReferenceObservation::label(0, "present")],
        )
        .unwrap_err();
        assert!(matches!(err, GroundTruthError::ReadingKindMismatch { index: 0, .. }));

        // Empty series.
        let err = ReferenceSeries::new(src(), Measurand::HeartRateBpm, vec![]).unwrap_err();
        assert!(matches!(err, GroundTruthError::EmptySeries));

        // Non-finite scalar.
        let err = ReferenceSeries::new(
            src(),
            Measurand::HeartRateBpm,
            vec![ReferenceObservation::scalar(0, f64::NAN)],
        )
        .unwrap_err();
        assert!(matches!(err, GroundTruthError::NonFiniteValue { index: 0 }));
    }

    #[test]
    fn measurand_mismatch_is_rejected() {
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 60.0), (1000, 61.0)],
        );
        let refr = scalar_series_ref(
            Measurand::BreathingRateBrpm,
            &[(0, 12.0), (1000, 13.0)],
        );
        let cfg = AlignmentConfig::default();
        let err = AgreementReport::build(&est, &refr, scope(), &cfg, 1.0, &measured_policy())
            .unwrap_err();
        assert!(matches!(err, GroundTruthError::MeasurandMismatch { .. }));
    }

    #[test]
    fn report_build_is_deterministic() {
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 10.0), (1000, 20.0), (2000, 30.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 12.0), (1000, 19.0), (2000, 33.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        let r1 = AgreementReport::build(&est, &refr, scope(), &cfg, 2.5, &measured_policy()).unwrap();
        let r2 = AgreementReport::build(&est, &refr, scope(), &cfg, 2.5, &measured_policy()).unwrap();
        assert_eq!(r1, r2);
    }

    #[test]
    fn report_json_round_trips() {
        let est = scalar_series_est(
            Measurand::HeartRateBpm,
            DataProvenance::Real,
            &[(0, 10.0), (1000, 20.0), (2000, 30.0)],
        );
        let refr = scalar_series_ref(
            Measurand::HeartRateBpm,
            &[(0, 12.0), (1000, 19.0), (2000, 33.0)],
        );
        let cfg = AlignmentConfig { grid_ms: 1000, max_lag_ms: 0, max_gap_ms: 400 };
        let report =
            AgreementReport::build(&est, &refr, scope(), &cfg, 2.5, &measured_policy()).unwrap();
        let json = serde_json::to_string(&report).unwrap();
        let back: AgreementReport = serde_json::from_str(&json).unwrap();
        // Structural equality on everything but the alignment score, which can
        // differ by a ULP through a text round-trip (serde_json float parsing).
        assert_eq!(back.source, report.source);
        assert_eq!(back.measurand, report.measurand);
        assert_eq!(back.scope, report.scope);
        assert_eq!(back.n_pairs, report.n_pairs);
        assert_eq!(back.grade, report.grade);
        assert_eq!(back.evidence_level, report.evidence_level);
        assert_eq!(back.metrics, report.metrics);
        assert_eq!(back.alignment.offset_ms, report.alignment.offset_ms);
        assert!(
            (back.alignment.score.unwrap() - report.alignment.score.unwrap()).abs() < 1e-9
        );
    }

    #[test]
    fn grading_policy_rejects_l0_and_bad_coverage() {
        assert!(matches!(
            GradingPolicy::new(0.5, EvidenceLevel::L0, None).unwrap_err(),
            GroundTruthError::GradeLevelConflict { .. }
        ));
        assert!(matches!(
            GradingPolicy::new(1.5, EvidenceLevel::L2, None).unwrap_err(),
            GroundTruthError::InvalidCoverage { .. }
        ));
    }
}
