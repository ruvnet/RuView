//! End to end on SYNTHETIC features: parity with the incumbent, fit, export,
//! load, shadow scoring and replay. No number here is a sensing result.

use ruview_fann_head::replay::{evaluate, parse_rows, Condition, ReplayRow, Split};
use ruview_fann_head::shadow::{CandidateOutcome, ShadowScorer};
use ruview_fann_head::train::{fit, FitConfig};
use ruview_fann_head::{decide, FannHead, HeadContext, HeadError, LinearHead, PresenceDecision};

fn context() -> HeadContext {
    HeadContext {
        feature_schema: "aether-skip/1".into(),
        sources: vec!["node-a".into(), "node-b".into()],
        room_revision: "office-r1".into(),
    }
}

/// Deterministic pseudo-random value in [-1, 1).
fn unit(seed: &mut u64) -> f32 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    ((*seed >> 40) as f32 / (1u64 << 23) as f32) - 1.0
}

fn incumbent() -> LinearHead {
    LinearHead::new(vec![
        0.9, -0.4, 0.3, 1.2, -0.7, 0.5, 0.2, -1.1, 0.8, 0.6, -0.15,
    ])
    .unwrap()
}

#[test]
fn parity_head_matches_the_incumbent_bit_for_bit() {
    let incumbent = incumbent();
    let parity = FannHead::from_linear(&incumbent, context()).unwrap();
    let mut scorer = parity.scorer().unwrap();
    let mut seed = 7u64;
    let mut decisions = [0usize; 3];
    for _ in 0..20_000 {
        let x: Vec<f32> = (0..10).map(|_| unit(&mut seed) * 3.0).collect();
        let a = incumbent.probability(&x).unwrap();
        let b = parity.score(&mut scorer, &x).unwrap();
        assert_eq!(a.to_bits(), b.to_bits(), "probability differs for {x:?}");
        assert_eq!(decide(a), decide(b));
        decisions[decide(a) as usize] += 1;
    }
    assert!(
        decisions.iter().all(|&n| n > 0),
        "all three decisions exercised: {decisions:?}"
    );
}

#[test]
fn parity_holds_on_the_abstention_boundaries() {
    // One feature, weight 1, bias 0: the logit is the input.
    let incumbent = LinearHead::new(vec![1.0, 0.0]).unwrap();
    let parity = FannHead::from_linear(&incumbent, context()).unwrap();
    let mut scorer = parity.scorer().unwrap();
    for target in [0.45f32, 0.55, 0.5] {
        let logit = (target / (1.0 - target)).ln();
        for step in -3i32..=3 {
            let x = [f32::from_bits((logit.to_bits() as i32 + step) as u32)];
            if !x[0].is_finite() {
                // Stepping below +0.0 in bit space lands on NaN; both reject it.
                assert!(incumbent.probability(&x).is_err());
                assert!(parity.score(&mut scorer, &x).is_err());
                continue;
            }
            let a = incumbent.probability(&x).unwrap();
            let b = parity.score(&mut scorer, &x).unwrap();
            assert_eq!(a.to_bits(), b.to_bits());
            assert_eq!(decide(a), decide(b));
        }
    }
    assert_eq!(decide(0.45), PresenceDecision::Abstain);
    assert_eq!(decide(0.55), PresenceDecision::Abstain);
    assert_eq!(decide(0.5500001), PresenceDecision::Occupied);
    assert_eq!(decide(0.4499999), PresenceDecision::Empty);
}

#[test]
fn candidate_is_49_parameters_and_fits_a_non_linear_rule() {
    // XOR-like rule a linear head cannot represent.
    let mut seed = 11u64;
    let rows = 256;
    let mut features = Vec::with_capacity(rows * 10);
    let mut labels = Vec::with_capacity(rows);
    for _ in 0..rows {
        let x: Vec<f32> = (0..10).map(|_| unit(&mut seed)).collect();
        labels.push((x[0] > 0.0) != (x[1] > 0.0));
        features.extend(x);
    }
    let candidate = fit(
        &features,
        &labels,
        context(),
        FitConfig {
            epochs: 1500,
            ..FitConfig::candidate()
        },
    )
    .unwrap();
    let control = fit(
        &features,
        &labels,
        context(),
        FitConfig {
            epochs: 1500,
            ..FitConfig::linear_control()
        },
    )
    .unwrap();
    assert_eq!(candidate.head.parameter_count(), 49);
    assert_eq!(control.head.parameter_count(), 11);
    assert!(
        candidate.final_train_mse < control.final_train_mse * 0.8,
        "candidate {} vs linear control {}",
        candidate.final_train_mse,
        control.final_train_mse
    );
    // Deterministic: the same recipe yields the same model.
    let again = fit(
        &features,
        &labels,
        context(),
        FitConfig {
            epochs: 1500,
            ..FitConfig::candidate()
        },
    )
    .unwrap();
    assert_eq!(candidate.head.digest(), again.head.digest());
}

#[test]
fn export_load_round_trip_scores_identically_and_is_bound_to_its_context() {
    let features: Vec<f32> = (0..40)
        .map(|i| ((i * 37) % 19) as f32 / 9.0 - 1.0)
        .collect();
    let labels = [true, false, true, false];
    let head = fit(&features, &labels, context(), FitConfig::candidate())
        .unwrap()
        .head;
    let json = head.to_json();
    let loaded = FannHead::from_json(&json).unwrap();
    assert_eq!(loaded.digest(), head.digest());
    let (mut a, mut b) = (head.scorer().unwrap(), loaded.scorer().unwrap());
    for row in features.chunks(10) {
        assert_eq!(
            head.score(&mut a, row).unwrap().to_bits(),
            loaded.score(&mut b, row).unwrap().to_bits()
        );
    }

    // Tampered weights, a foreign engine revision and unknown fields are refused.
    let tampered = json.replacen("\"weights\":[", "\"weights\":[0.123,", 1);
    assert!(FannHead::from_json(&tampered).is_err());
    let foreign = json.replace(
        ruview_fann_head::RUV_FANN_REV,
        "0000000000000000000000000000000000000000",
    );
    assert!(matches!(
        FannHead::from_json(&foreign),
        Err(HeadError::Invalid(_))
    ));
    assert!(FannHead::from_json(&json.replacen('{', "{\"extra\":1,", 1)).is_err());

    // A head fitted for one room revision cannot shadow another.
    let other = HeadContext {
        room_revision: "office-r2".into(),
        ..context()
    };
    assert!(matches!(
        loaded.require_context(&other),
        Err(HeadError::Context(_))
    ));
    assert!(ShadowScorer::new(&incumbent(), &loaded, &other).is_err());
}

#[test]
fn shadow_scores_beside_the_incumbent_and_a_bad_input_is_unavailable() {
    let incumbent = incumbent();
    let candidate = FannHead::from_linear(&incumbent, context()).unwrap();
    let mut shadow = ShadowScorer::new(&incumbent, &candidate, &context()).unwrap();
    let x: Vec<f32> = (0..10).map(|i| i as f32 * 0.1).collect();
    let score = shadow.score("cohort-1", &x).unwrap();
    assert_eq!(score.decisions_agree, Some(true));
    assert_eq!(score.candidate_digest, candidate.digest());
    assert!(matches!(score.candidate, CandidateOutcome::Scored { .. }));
    // The incumbent rejecting an input is the caller's Unavailable path.
    assert!(shadow.score("cohort-2", &[f32::NAN; 10]).is_err());
    assert!(shadow.score("cohort-3", &[0.0; 3]).is_err());
    let json = serde_json::to_string(&shadow.score("cohort-4", &x).unwrap()).unwrap();
    assert!(
        json.contains("\"observation_id\":\"cohort-4\"") && json.contains("\"status\":\"scored\"")
    );
    assert_eq!(shadow.counters().scored, 2);
    assert_eq!(shadow.counters().agree, 2);
}

#[test]
fn invalid_heads_are_refused() {
    assert!(LinearHead::new(vec![1.0]).is_err());
    assert!(LinearHead::new(vec![f32::INFINITY, 0.0]).is_err());
    assert!(LinearHead::new(vec![0.0; 66]).is_err());
    let empty_sources = HeadContext {
        sources: vec![],
        ..context()
    };
    assert!(FannHead::from_linear(&incumbent(), empty_sources).is_err());
    let one_class = fit(&[0.0; 20], &[true, true], context(), FitConfig::candidate());
    assert!(matches!(one_class, Err(HeadError::Invalid(_))));
}

/// Synthetic room: presence raises feature 0; adjacent-room activity raises
/// feature 1 and, weakly, feature 0. The "incumbent" keys on their sum, so it
/// raises false occupied readings on adjacent-only rows.
fn synthetic_rows(room: &str, seed: u64) -> Vec<ReplayRow> {
    let mut seed = seed;
    let mut rows = Vec::new();
    let sessions = [
        ("s1", Split::Train),
        ("s2", Split::Train),
        ("s3", Split::Heldout),
        ("s4", Split::Heldout),
    ];
    for (session, split) in sessions {
        for condition in [
            Condition::QuietEmpty,
            Condition::AdjacentOnly,
            Condition::StillOccupied,
            Condition::MovingOccupied,
        ] {
            for _ in 0..12 {
                let mut x: Vec<f32> = (0..10).map(|_| unit(&mut seed) * 0.15).collect();
                match condition {
                    Condition::QuietEmpty => {}
                    Condition::AdjacentOnly => {
                        x[1] += 1.5;
                        x[0] += 0.4;
                    }
                    Condition::StillOccupied => x[0] += 1.2,
                    Condition::MovingOccupied => {
                        x[0] += 1.4;
                        x[2] += 0.8;
                    }
                }
                let incumbent_logit = 3.0 * (x[0] + x[1]) - 2.0;
                rows.push(ReplayRow {
                    room: room.into(),
                    session: format!("{room}-{session}"),
                    split,
                    condition,
                    features: x,
                    incumbent_probability: Some(ruview_fann_head::sigmoid(incumbent_logit)),
                    synthetic: true,
                    timestamp_ms: None,
                    label_source: Some("reference".into()),
                });
            }
        }
    }
    rows
}

#[test]
fn replay_compares_incumbent_control_and_candidate_on_held_out_sessions() {
    let mut rows = synthetic_rows("office", 3);
    rows.extend(synthetic_rows("den", 5));
    // Through the JSONL boundary, as the CLI reads it.
    let text: String = rows
        .iter()
        .map(|r| serde_json::to_string(r).unwrap() + "\n")
        .collect();
    let report = evaluate(
        &parse_rows(&text).unwrap(),
        &context(),
        FitConfig::candidate(),
    )
    .unwrap();

    assert_eq!(report.evidence, "SYNTHETIC");
    assert_eq!(report.rooms.len(), 2);
    for room in &report.rooms {
        assert_eq!(
            (room.train_rows, room.heldout_rows, room.heldout_sessions),
            (96, 96, 2)
        );
        assert_eq!(room.candidate_parameters, 49);
        let incumbent = room.incumbent.as_ref().unwrap();
        // The synthetic incumbent fails on adjacent-only rows; the refit heads do not.
        assert_eq!(incumbent.passes_local_gates, Some(false));
        assert_eq!(
            room.candidate.passes_local_gates,
            Some(true),
            "{:?}",
            room.candidate.gate_failures
        );
        assert_eq!(room.linear_control.passes_local_gates, Some(true));
        let adjacent = room.adjacent.as_ref().unwrap();
        assert_eq!(adjacent.rows, 24);
        assert!(adjacent.incumbent_false_occupied > adjacent.candidate_false_occupied);
        assert!(adjacent.meets_pilot_target, "{adjacent:?}");
    }
}

#[test]
fn replay_refuses_leaky_or_incomplete_input_and_keeps_abstentions_in_denominators() {
    let mut rows = synthetic_rows("office", 3);
    // A session in two splits would leak HeldOut into Train.
    let mut leaky = rows.clone();
    leaky[0].split = Split::Heldout;
    assert!(evaluate(&leaky, &context(), FitConfig::candidate()).is_err());
    let only_train: Vec<ReplayRow> = rows
        .iter()
        .filter(|r| r.split == Split::Train)
        .cloned()
        .collect();
    assert!(evaluate(&only_train, &context(), FitConfig::candidate()).is_err());

    // An incumbent that abstains on every still-occupied row has zero sensitivity there.
    for r in rows
        .iter_mut()
        .filter(|r| r.condition == Condition::StillOccupied)
    {
        r.incumbent_probability = Some(0.5);
    }
    let report = evaluate(&rows, &context(), FitConfig::candidate()).unwrap();
    let still = report.rooms[0].incumbent.as_ref().unwrap().conditions[&Condition::StillOccupied];
    assert_eq!((still.sensitivity, still.coverage), (Some(0.0), 0.0));

    // Without a recorded incumbent there is no incumbent score and no comparison.
    for r in &mut rows {
        r.incumbent_probability = None;
    }
    let report = evaluate(&rows, &context(), FitConfig::candidate()).unwrap();
    assert!(report.rooms[0].incumbent.is_none() && report.rooms[0].adjacent.is_none());
}

#[test]
fn learning_curve_reports_unfittable_prefixes_and_where_the_gates_settle() {
    use ruview_fann_head::curve::learning_curve;
    let mut rows = synthetic_rows("office", 3);
    for (i, r) in rows.iter_mut().enumerate() {
        r.timestamp_ms = Some(1_000 * i as i64);
    }
    let curve = learning_curve(&rows, &context(), FitConfig::candidate()).unwrap();
    assert!(curve.labels_are_reference);
    let room = &curve.rooms[0];
    assert_eq!((room.prefix_unit, room.heldout_rows), ("row_fractions", 96));
    let sizes: Vec<usize> = room.points.iter().map(|p| p.train_rows).collect();
    assert_eq!(sizes, [6, 12, 24, 48, 72, 96]);
    // The first 24 rows are empty and adjacent-only: one class, nothing to fit.
    for p in &room.points[..3] {
        assert!(p.candidate.is_none() && p.linear_control.is_none());
    }
    let last = room.points.last().unwrap();
    assert_eq!(last.train_sessions, 2);
    assert_eq!(
        last.candidate.as_ref().unwrap().passes_local_gates,
        Some(true)
    );
    // Two sessions of 48 rows at 1 s spacing: 47 s each.
    assert_eq!(last.train_minutes, Some(94.0 / 60.0));
    let from = room.candidate_passes_from_rows.unwrap();
    assert!((48..=96).contains(&from), "{from}");

    // Three Train sessions switch the prefixes to session boundaries.
    for r in rows.iter_mut().filter(|r| r.session == "office-s3") {
        r.split = Split::Train;
    }
    let curve = learning_curve(&rows, &context(), FitConfig::candidate()).unwrap();
    let room = &curve.rooms[0];
    assert_eq!(room.prefix_unit, "sessions");
    let sizes: Vec<usize> = room.points.iter().map(|p| p.train_rows).collect();
    assert_eq!(sizes, [48, 96, 144]);
}

#[test]
fn labels_that_are_not_a_reference_are_flagged() {
    let mut rows = synthetic_rows("office", 3);
    rows[0].label_source = Some("incumbent".into());
    let report = evaluate(&rows, &context(), FitConfig::candidate()).unwrap();
    assert!(!report.labels_are_reference);
    assert_eq!(report.label_sources, ["incumbent", "reference"]);
    rows[1].label_source = None;
    let report = evaluate(&rows, &context(), FitConfig::candidate()).unwrap();
    assert!(report.label_sources.contains(&"unstated".to_string()));
}

#[test]
fn external_shadow_counts_agreement_abstention_and_failures() {
    use ruview_fann_head::shadow::ExternalShadow;
    // One feature, weight 4: the logit is 4x.
    let head = FannHead::from_linear(&LinearHead::new(vec![4.0, 0.0]).unwrap(), context()).unwrap();
    let mut shadow = ExternalShadow::new(head, &context()).unwrap();
    assert_eq!(shadow.features(), 1);
    shadow.score(&[1.0], true); // occupied, agrees
    shadow.score(&[-1.0], false); // empty, agrees
    shadow.score(&[1.0], false); // occupied against an empty incumbent
    shadow.score(&[0.0], true); // 0.5: abstains
    assert!(matches!(
        shadow.score(&[f32::NAN], true),
        CandidateOutcome::Unavailable { .. }
    ));
    let c = shadow.counters();
    assert_eq!(
        (
            c.scored,
            c.agree,
            c.disagree,
            c.candidate_abstain,
            c.unavailable
        ),
        (4, 2, 1, 1, 1)
    );
    let other = HeadContext {
        room_revision: "den-r1".into(),
        ..context()
    };
    let head = FannHead::from_linear(&LinearHead::new(vec![4.0, 0.0]).unwrap(), context()).unwrap();
    assert!(ExternalShadow::new(head, &other).is_err());
}

#[test]
fn balancing_thins_the_larger_class_evenly_and_keeps_order() {
    use ruview_fann_head::train::balance_classes;
    // 2 occupied rows among 10; one feature holding the row index.
    let labels: Vec<bool> = (0..10).map(|i| i == 3 || i == 7).collect();
    let features: Vec<f32> = (0..10).map(|i| i as f32).collect();
    let (x, y) = balance_classes(&features, &labels, 1);
    assert_eq!(y.iter().filter(|&&l| l).count(), 2);
    assert_eq!(y.iter().filter(|&&l| !l).count(), 2);
    // Both occupied rows kept; the empty rows come from both halves, in order.
    assert!(x.contains(&3.0) && x.contains(&7.0));
    assert!(x.windows(2).all(|w| w[0] < w[1]), "{x:?}");
    let empties: Vec<f32> = x
        .iter()
        .zip(&y)
        .filter(|(_, &l)| !l)
        .map(|(v, _)| *v)
        .collect();
    assert!(empties[0] < 5.0 && empties[1] >= 5.0, "{empties:?}");
    // Already balanced, or one class only: unchanged.
    let even = [true, false];
    assert_eq!(balance_classes(&[1.0, 2.0], &even, 1).1, even);
    assert_eq!(balance_classes(&[1.0, 2.0], &[true, true], 1).1.len(), 2);

    // An unbalanced fit reports the rows it actually used.
    let mut feats = Vec::new();
    let mut labs = Vec::new();
    for i in 0..100 {
        let occupied = i % 10 == 0;
        feats.extend([if occupied { 1.0f32 } else { -1.0 }, 0.0]);
        labs.push(occupied);
    }
    let config = FitConfig {
        balance: true,
        ..FitConfig::candidate()
    };
    let report = fit(&feats, &labs, context2(), config).unwrap();
    assert_eq!(report.rows, 20);
}

fn context2() -> HeadContext {
    HeadContext {
        sources: vec!["node-a".into()],
        ..context()
    }
}
