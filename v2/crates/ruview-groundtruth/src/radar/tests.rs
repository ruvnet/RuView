//! Unit tests for the radar window adapter.

use super::*;
use crate::model::Reading;

fn win(t: i64) -> RadarWindow {
    RadarWindow {
        timestamp_ms: t + 1000,
        window_start_ms: Some(t),
        health: Some("ok".into()),
        quality: Some(1.0),
        source: Some(RadarWindowSource {
            verified: true,
            simulated: false,
        }),
        presence: None,
        target_count: None,
        max_target_count: None,
        distance_cm: None,
        targets: Vec::new(),
    }
}

fn cfg() -> RadarAdapterConfig {
    RadarAdapterConfig::default()
}

#[test]
fn parses_tracker_and_vitals_lines_and_ignores_unknown_fields() {
    let text = r#"
{"timestamp_ms":2000,"window_start_ms":1000,"health":"ok","quality":1.0,"source":{"kind":"uart","verified":true,"simulated":false},"target_count":2,"max_target_count":3,"targets":[{"slot":1,"x_m":0.3,"y_m":1.2,"speed_mps":0.1},{"slot":2,"x_m":-1.0,"y_m":2.0}],"frames":10}

{"timestamp_ms":3000,"presence":true,"distance_cm":150.0,"heart_rate_bpm":61.0}
"#;
    let w = parse_radar_jsonl(text).unwrap();
    assert_eq!(w.len(), 2);
    assert_eq!(w[0].targets.len(), 2);
    assert_eq!(w[0].max_target_count, Some(3));
    assert_eq!(w[1].presence, Some(true));
    assert_eq!(w[1].window_start_ms, None);
}

#[test]
fn malformed_line_is_a_row_numbered_error() {
    let err = parse_radar_jsonl("{\"timestamp_ms\":1}\n\nnot json\n").unwrap_err();
    assert!(matches!(err, GroundTruthError::InvalidRow { row: 3, .. }));
    let err = parse_radar_jsonl("{\"presence\":true}").unwrap_err();
    assert!(matches!(err, GroundTruthError::InvalidRow { row: 1, .. }));
}

#[test]
fn over_long_line_is_rejected() {
    let line = format!(
        "{{\"timestamp_ms\":1,\"health\":\"{}\"}}",
        "x".repeat(MAX_RADAR_LINE_BYTES)
    );
    let err = parse_radar_jsonl(&line).unwrap_err();
    assert!(matches!(err, GroundTruthError::InvalidRow { row: 1, .. }));
}

#[test]
fn tracker_window_gives_count_interval_and_nearest_range() {
    let mut w = win(0);
    w.target_count = Some(2);
    w.max_target_count = Some(3);
    w.targets = vec![
        RadarTarget { x_m: 0.6, y_m: 0.8 },
        RadarTarget { x_m: 0.0, y_m: 2.5 },
    ];
    let l = label_window(&w, 0, &cfg());
    assert_eq!(l.status, LabelStatus::Labelled);
    assert_eq!(l.presence, Some(true));
    assert_eq!(l.presence_basis, Some(PresenceBasis::TargetCount));
    assert_eq!(l.person_count, Some(2));
    assert_eq!(
        (l.uncertainty.count_low, l.uncertainty.count_high),
        (Some(2), Some(3))
    );
    assert!((l.nearest_range_m.unwrap() - 1.0).abs() < 1e-12);
    assert_eq!(l.uncertainty.range_sigma_m, Some(0.15));
    assert_eq!(l.at_unix_ms, 500);
}

#[test]
fn empty_tracker_window_is_absent_with_zero_count() {
    let mut w = win(0);
    w.target_count = Some(0);
    let l = label_window(&w, 0, &cfg());
    assert_eq!(l.presence, Some(false));
    assert_eq!(l.person_count, Some(0));
    assert_eq!(l.nearest_range_m, None);
    assert_eq!(l.uncertainty.range_sigma_m, None);
}

#[test]
fn vitals_window_uses_presence_flag_and_distance() {
    let mut w = win(0);
    w.presence = Some(true);
    w.distance_cm = Some(180.0);
    let l = label_window(&w, 0, &cfg());
    assert_eq!(l.presence_basis, Some(PresenceBasis::PresenceFlag));
    assert_eq!(l.person_count, None);
    assert!((l.nearest_range_m.unwrap() - 1.8).abs() < 1e-12);
}

#[test]
fn presence_count_conflict_is_unknown() {
    let mut w = win(0);
    w.presence = Some(false);
    w.target_count = Some(1);
    let l = label_window(&w, 0, &cfg());
    assert_eq!(
        l.status,
        LabelStatus::Unknown(UnknownReason::PresenceCountConflict)
    );
    assert_eq!(l.presence, None);
}

#[test]
fn untrusted_sources_are_unknown() {
    let mut w = win(0);
    w.target_count = Some(1);

    let mut sim = w.clone();
    sim.source = Some(RadarWindowSource {
        verified: false,
        simulated: true,
    });
    assert_eq!(
        label_window(&sim, 0, &cfg()).status,
        LabelStatus::Unknown(UnknownReason::Simulated)
    );
    let allow = RadarAdapterConfig {
        allow_simulated: true,
        ..cfg()
    };
    assert_eq!(label_window(&sim, 0, &allow).status, LabelStatus::Labelled);

    let mut unv = w.clone();
    unv.source = None;
    assert_eq!(
        label_window(&unv, 0, &cfg()).status,
        LabelStatus::Unknown(UnknownReason::Unverified)
    );
    let lax = RadarAdapterConfig {
        require_verified: false,
        ..cfg()
    };
    assert_eq!(label_window(&unv, 0, &lax).status, LabelStatus::Labelled);

    let mut dead = w.clone();
    dead.health = Some("no_source".into());
    assert_eq!(
        label_window(&dead, 0, &cfg()).status,
        LabelStatus::Unknown(UnknownReason::NoSource)
    );

    let mut low = w.clone();
    low.quality = Some(0.2);
    let strict = RadarAdapterConfig {
        min_quality: 0.5,
        ..cfg()
    };
    assert_eq!(
        label_window(&low, 0, &strict).status,
        LabelStatus::Unknown(UnknownReason::LowQuality)
    );
    low.quality = None;
    assert_eq!(
        label_window(&low, 0, &strict).status,
        LabelStatus::Unknown(UnknownReason::LowQuality)
    );

    let mut nothing = win(0);
    nothing.targets.clear();
    assert_eq!(
        label_window(&nothing, 0, &cfg()).status,
        LabelStatus::Unknown(UnknownReason::NoObservation)
    );

    let mut bad = w;
    bad.window_start_ms = Some(bad.timestamp_ms + 1);
    assert_eq!(
        label_window(&bad, 0, &cfg()).status,
        LabelStatus::Unknown(UnknownReason::BadTimestamps)
    );
}

#[test]
fn out_of_range_and_non_finite_targets_are_ignored() {
    let mut w = win(0);
    w.targets = vec![
        RadarTarget {
            x_m: 0.0,
            y_m: 40.0,
        },
        RadarTarget {
            x_m: f64::NAN,
            y_m: 1.0,
        },
    ];
    let l = label_window(&w, 0, &cfg());
    assert_eq!(l.person_count, Some(0));
    assert_eq!(l.presence, Some(false));
}

#[test]
fn series_feed_the_agreement_machinery() {
    let mut windows = Vec::new();
    for (i, n) in [0u32, 1, 2, 1].iter().enumerate() {
        let mut w = win(i as i64 * 1000);
        w.target_count = Some(*n);
        w.targets = (0..*n)
            .map(|k| RadarTarget {
                x_m: 0.0,
                y_m: 1.0 + f64::from(k),
            })
            .collect();
        windows.push(w);
    }
    let mut unknown = win(10_000);
    unknown.source = None;
    windows.push(unknown);

    let labels = RadarLabels::from_windows("radar-1", "ld2450", &windows, cfg()).unwrap();
    assert_eq!(labels.labels.len(), 5);
    assert_eq!(labels.labelled().count(), 4);
    assert_eq!(labels.source.modality, ReferenceModality::MmWave);

    let p = labels.presence_series().unwrap();
    let got: Vec<&str> = p
        .samples()
        .iter()
        .filter_map(|s| s.reading.as_label())
        .collect();
    assert_eq!(got, ["absent", "present", "present", "present"]);

    let c = labels.person_count_series().unwrap();
    assert_eq!(c.samples()[2].reading, Reading::Scalar(2.0));

    let r = labels.range_series().unwrap();
    assert_eq!(r.len(), 3);
    assert_eq!(r.samples()[0].reading, Reading::Scalar(1.0));
}

#[test]
fn bad_config_is_rejected() {
    let bad = RadarAdapterConfig {
        range_sigma_m: -1.0,
        ..cfg()
    };
    assert!(RadarLabels::from_windows("r", "d", &[], bad).is_err());
    let bad = RadarAdapterConfig {
        min_quality: 2.0,
        ..cfg()
    };
    assert!(bad.validate().is_err());
}

#[test]
fn radar_presence_grades_an_rf_estimate() {
    use crate::{
        AgreementMetrics, AgreementReport, AlignmentConfig, DataProvenance, DistanceBand,
        EstimateSeries, GradingPolicy, LineOfSight, MotionState, SessionScope,
    };
    use ruview_ontology::EvidenceLevel;

    let truth = [0u32, 0, 1, 1, 1, 0];
    let windows: Vec<RadarWindow> = truth
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut w = win(i as i64 * 1000);
            w.target_count = Some(*n);
            w
        })
        .collect();
    let labels = RadarLabels::from_windows("radar-1", "ld2450", &windows, cfg()).unwrap();
    let reference = labels.presence_series().unwrap();

    // RF estimate stamped at the same midpoints, wrong on one window.
    let est_labels = [
        "absent", "present", "present", "present", "present", "absent",
    ];
    let est = EstimateSeries::new(
        Measurand::Presence,
        "rf-presence-v1",
        DataProvenance::Real,
        est_labels
            .iter()
            .enumerate()
            .map(|(i, l)| ReferenceObservation::label(i as i64 * 1000 + 500, *l))
            .collect(),
    )
    .unwrap();
    let scope =
        SessionScope::new(1, MotionState::Mixed, LineOfSight::Los, DistanceBand::Near).unwrap();
    let align = AlignmentConfig {
        grid_ms: 1000,
        max_lag_ms: 0,
        max_gap_ms: 400,
    };
    let policy = GradingPolicy::new(0.5, EvidenceLevel::L2, None).unwrap();
    let report = AgreementReport::build(&est, &reference, scope, &align, 0.0, &policy).unwrap();
    match report.metrics {
        AgreementMetrics::Categorical { n_agree, .. } => assert_eq!(n_agree, 5),
        other => panic!("expected categorical metrics, got {other:?}"),
    }
}

#[test]
fn labels_are_deterministic() {
    let mut w = win(0);
    w.target_count = Some(1);
    w.targets = vec![RadarTarget { x_m: 0.3, y_m: 0.4 }];
    assert_eq!(label_window(&w, 0, &cfg()), label_window(&w, 0, &cfg()));
}

#[test]
fn radar_labels_are_claimed_never_measured() {
    use crate::agreement::EvidenceGrade;
    let mut present = win(0);
    present.target_count = Some(1);
    let unknown = win(1000); // no observation -> Unknown
    for w in [present, unknown] {
        assert_eq!(label_window(&w, 0, &cfg()).evidence, EvidenceGrade::Claimed);
    }
    // A label deserialised from older JSON without the field is also Claimed.
    let mut v = serde_json::to_value(label_window(&win(0), 0, &cfg())).unwrap();
    v.as_object_mut().unwrap().remove("evidence");
    let back: RadarLabel = serde_json::from_value(v).unwrap();
    assert_eq!(back.evidence, EvidenceGrade::Claimed);
}
