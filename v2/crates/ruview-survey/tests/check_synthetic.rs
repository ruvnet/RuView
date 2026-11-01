//! SYNTHETIC check-mode scenarios. Ranges are generated from a known layout
//! with FTM-like bias and noise taken from the 2026-10-04 bench envelope
//! (bias −0.5 to +1.9 m per link, spread ±0.3 m). No hardware is involved.

use ruview_survey::*;

/// Asymmetric five-node layout in a 10 × 6 m space, metres.
const TRUTH: [(u8, [f64; 3]); 5] = [
    (1, [0.3, 0.2, 1.0]),
    (2, [9.6, 0.5, 1.2]),
    (3, [8.8, 5.7, 1.0]),
    (4, [1.1, 5.2, 1.4]),
    (5, [4.4, 2.1, 0.8]),
];

fn positions_string(layout: &[(u8, [f64; 3])], scale: f64) -> String {
    layout
        .iter()
        .map(|(id, p)| format!("{id}:{},{},{}", p[0] * scale, p[1] * scale, p[2] * scale))
        .collect::<Vec<_>>()
        .join(";")
}

fn dist(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// Deterministic pseudo-random value in [0, 1) from a seed.
fn unit(seed: u64) -> f64 {
    let x = seed
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((x >> 11) as f64) / ((1u64 << 53) as f64)
}

/// Ranges for every pair of `layout`: per-link bias in [bias_lo, bias_hi]
/// plus ±`noise` m session noise (reported as sigma), `sessions` per link.
fn ranges_with(
    layout: &[(u8, [f64; 3])],
    bias_lo: f64,
    bias_hi: f64,
    noise: f64,
    sessions: u64,
) -> Vec<RangeObservation> {
    let mut out = Vec::new();
    for (i, (a, pa)) in layout.iter().enumerate() {
        for (b, pb) in layout.iter().skip(i + 1) {
            let link_seed = u64::from(*a) * 131 + u64::from(*b);
            let bias = bias_lo + (bias_hi - bias_lo) * unit(link_seed);
            for s in 0..sessions {
                let jitter = noise * (2.0 * unit(link_seed * 977 + s) - 1.0);
                let range = (dist(pa, pb) + bias + jitter).max(0.0);
                out.push(RangeObservation {
                    rssi_dbm: Some(-55),
                    ..RangeObservation::ftm(*a, *b, range, noise)
                });
            }
        }
    }
    out
}

/// FTM-like ranges: ±0.3 m session noise, as on the bench.
fn ranges(
    layout: &[(u8, [f64; 3])],
    bias_lo: f64,
    bias_hi: f64,
    sessions: u64,
) -> Vec<RangeObservation> {
    ranges_with(layout, bias_lo, bias_hi, 0.3, sessions)
}

#[test]
fn correct_config_with_ftm_bias_is_consistent() {
    let positions = parse_node_positions(&positions_string(&TRUTH, 1.0)).unwrap();
    let obs = ranges(&TRUTH, -0.5, 1.9, 5);
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert_eq!(report.verdict, Verdict::Consistent, "{report:#?}");
    assert_eq!(report.links.len(), 10);
    assert!(report.links.iter().all(|l| l.status == LinkStatus::Ok));
}

#[test]
fn positions_entered_in_feet_are_flagged_as_unit_mismatch() {
    // Operator measured in feet and typed the numbers as if they were metres.
    let feet = 1.0 / 0.3048;
    let positions = parse_node_positions(&positions_string(&TRUTH, feet)).unwrap();
    let obs = ranges(&TRUTH, -0.5, 1.9, 5);
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert_eq!(report.verdict, Verdict::Warnings);
    let unit = report
        .findings
        .iter()
        .find_map(|f| match f {
            Finding::UnitMismatch {
                unit, fitted_scale, ..
            } => Some((unit.clone(), *fitted_scale)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no unit finding: {report:#?}"));
    assert_eq!(unit.0, "feet");
    assert!((0.25..0.37).contains(&unit.1), "scale {}", unit.1);
    // The unit finding explains the residuals; no swap or per-link noise.
    assert!(!report.findings.iter().any(|f| matches!(
        f,
        Finding::IdSwapSuggested { .. } | Finding::LinkOutlier { .. } | Finding::NodeMoved { .. }
    )));
}

#[test]
fn positions_in_centimetres_are_flagged() {
    let positions = parse_node_positions(&positions_string(&TRUTH, 100.0)).unwrap();
    let obs = ranges(&TRUTH, -0.5, 1.9, 3);
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, Finding::UnitMismatch { unit, .. } if unit == "centimetres")));
}

#[test]
fn swapped_ids_are_suggested_back() {
    // The operator typed node 2's position under id 4 and vice versa.
    let mut configured = TRUTH;
    configured[1].1 = TRUTH[3].1;
    configured[3].1 = TRUTH[1].1;
    let positions = parse_node_positions(&positions_string(&configured, 1.0)).unwrap();
    let obs = ranges(&TRUTH, -0.5, 1.9, 5);
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert_eq!(report.verdict, Verdict::Warnings);
    let (assignment, equivalent) = report
        .findings
        .iter()
        .find_map(|f| match f {
            Finding::IdSwapSuggested {
                assignment,
                equivalent_assignments,
                ..
            } => Some((assignment.clone(), *equivalent_assignments)),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no swap finding: {report:#?}"));
    assert_eq!(assignment, vec![(2, 4), (4, 2)]);
    // Nodes 1-4 sit near the corners of a rectangle, so at FTM tolerances a
    // four-id rotation fits about as well. The fewest-changes rule picks
    // the two-id swap and the tie is reported.
    assert!(equivalent >= 2, "{report:#?}");
}

#[test]
fn three_way_rotation_is_suggested_back() {
    // Ids 1, 3, 5 rotated: 1 got 3's position, 3 got 5's, 5 got 1's.
    let mut configured = TRUTH;
    configured[0].1 = TRUTH[2].1;
    configured[2].1 = TRUTH[4].1;
    configured[4].1 = TRUTH[0].1;
    let positions = parse_node_positions(&positions_string(&configured, 1.0)).unwrap();
    let obs = ranges(&TRUTH, -0.5, 1.9, 5);
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    let assignment = report
        .findings
        .iter()
        .find_map(|f| match f {
            Finding::IdSwapSuggested { assignment, .. } => Some(assignment.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no swap finding: {report:#?}"));
    // Node 1's ranges match the position configured for node 5, and so on.
    assert_eq!(assignment, vec![(1, 5), (3, 1), (5, 3)]);
}

/// A 6 × 4 m rectangle with a node in each corner.
const RECT: [(u8, [f64; 3]); 4] = [
    (1, [0.0, 0.0, 1.0]),
    (2, [6.0, 0.0, 1.0]),
    (3, [6.0, 4.0, 1.0]),
    (4, [0.0, 4.0, 1.0]),
];

#[test]
fn relabelling_that_is_a_symmetry_is_invisible() {
    // Ids rotated 180 degrees (1<->3, 2<->4). Every pairwise distance is
    // unchanged, so ranges cannot see it and the check must not invent a
    // swap. Check mode cannot catch this class of mistake.
    let mut configured = RECT;
    configured[0].1 = RECT[2].1;
    configured[2].1 = RECT[0].1;
    configured[1].1 = RECT[3].1;
    configured[3].1 = RECT[1].1;
    let positions = parse_node_positions(&positions_string(&configured, 1.0)).unwrap();
    let obs = ranges_with(&RECT, 0.0, 0.05, 0.05, 3);
    let report = check(&positions, &obs, &CheckParams::uwb()).unwrap();
    assert_eq!(report.verdict, Verdict::Consistent, "{report:#?}");
}

#[test]
fn swap_along_a_wall_of_a_rectangle_is_ambiguous() {
    // Ids 1 and 2 swapped along the long wall. Mirroring the rectangle
    // turns that into a 3<->4 swap with identical distances, so both
    // two-id fixes fit equally and the check must say it cannot choose.
    let mut configured = RECT;
    configured[0].1 = RECT[1].1;
    configured[1].1 = RECT[0].1;
    let positions = parse_node_positions(&positions_string(&configured, 1.0)).unwrap();
    let obs = ranges_with(&RECT, 0.0, 0.05, 0.05, 3);
    let report = check(&positions, &obs, &CheckParams::uwb()).unwrap();
    assert_eq!(report.verdict, Verdict::Warnings);
    assert!(
        report
            .findings
            .iter()
            .any(|f| matches!(f, Finding::IdSwapAmbiguous { equivalent_assignments } if *equivalent_assignments >= 2)),
        "{report:#?}"
    );
}

#[test]
fn moved_node_is_flagged_without_blaming_others() {
    // Node 5 was moved 2 m after setup; its configured position is stale.
    // UWB-grade ranges: uncalibrated FTM tolerances hide moves this size.
    let mut actual = TRUTH;
    actual[4].1 = [2.6, 1.2, 0.8];
    let positions = parse_node_positions(&positions_string(&TRUTH, 1.0)).unwrap();
    let obs = ranges_with(&actual, 0.0, 0.1, 0.05, 5);
    let report = check(&positions, &obs, &CheckParams::uwb()).unwrap();
    assert_eq!(report.verdict, Verdict::Warnings);
    let moved: Vec<u8> = report
        .findings
        .iter()
        .filter_map(|f| match f {
            Finding::NodeMoved { node, .. } => Some(*node),
            _ => None,
        })
        .collect();
    assert_eq!(moved, vec![5], "{report:#?}");
    assert!(!report
        .findings
        .iter()
        .any(|f| matches!(f, Finding::IdSwapSuggested { .. })));
}

#[test]
fn bench_style_single_link_outlier_is_reported_as_link_outlier() {
    // One link over-reads by +4.1 m, as the 3.71 m bench start did.
    let positions = parse_node_positions(&positions_string(&TRUTH, 1.0)).unwrap();
    let mut obs = ranges(&TRUTH, 0.0, 0.5, 3);
    for o in obs.iter_mut().filter(|o| o.pair() == (1, 5)) {
        o.range_m += 4.1;
    }
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert_eq!(report.verdict, Verdict::Warnings);
    assert!(report
        .findings
        .iter()
        .any(|f| matches!(f, Finding::LinkOutlier { a: 1, b: 5, .. })));
    assert!(!report.findings.iter().any(|f| matches!(
        f,
        Finding::IdSwapSuggested { .. } | Finding::NodeMoved { .. } | Finding::UnitMismatch { .. }
    )));
}

#[test]
fn per_boot_calibration_tightens_tolerance() {
    // Same responder boot for every session; one reference link at a taped
    // 3.0 m recovers a +0.7 m bias, so a 1.5 m misplacement that
    // uncalibrated FTM cannot see becomes visible.
    let positions = parse_node_positions("1:0,0,1;2:5,0,1").unwrap();
    let boot = Some(42);
    let reference: Vec<RangeObservation> = (0..5)
        .map(|_| RangeObservation {
            responder_boot: boot,
            ..RangeObservation::ftm(1, 2, 3.0 + 0.7, 0.3)
        })
        .collect();
    let mut table = CalibrationTable::new();
    let offset = table.record_reference(&reference, 3.0).unwrap();
    assert!((offset - 0.7).abs() < 1e-9);

    // Node 2 actually sits at 6.5 m, not the configured 5.0 m.
    let live = RangeObservation {
        responder_boot: boot,
        ..RangeObservation::ftm(1, 2, 6.5 + 0.7, 0.1)
    };
    let raw = check(
        &positions,
        std::slice::from_ref(&live),
        &CheckParams::ftm_uncalibrated(),
    )
    .unwrap();
    assert_eq!(raw.links[0].status, LinkStatus::Ok);

    let calibrated = table.apply(&live);
    let report = check(&positions, &[calibrated], &CheckParams::ftm_uncalibrated()).unwrap();
    assert!(report.links[0].calibrated);
    assert_eq!(report.links[0].status, LinkStatus::Outlier, "{report:#?}");

    // After a responder reboot the offset no longer applies.
    let rebooted = RangeObservation {
        responder_boot: Some(43),
        ..live
    };
    assert!(!table.apply(&rebooted).calibrated);
}

#[test]
fn unknown_and_unranged_nodes_are_reported() {
    let positions = parse_node_positions("1:0,0,1;2:4,0,1;3:0,3,1;9:2,2,1").unwrap();
    let obs = [
        RangeObservation::ftm(1, 2, 4.0, 0.2),
        RangeObservation::ftm(1, 3, 3.0, 0.2),
        RangeObservation::ftm(2, 3, 5.0, 0.2),
        RangeObservation::ftm(1, 7, 2.0, 0.2),
    ];
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert!(report.findings.contains(&Finding::UnknownNode {
        node: 7,
        observations: 1
    }));
    assert!(report.findings.contains(&Finding::UnrangedNode { node: 9 }));
    assert_eq!(report.verdict, Verdict::Warnings);
}

#[test]
fn weak_sessions_are_not_scored() {
    let positions = parse_node_positions("1:0,0,1;2:4,0,1").unwrap();
    let obs = [RangeObservation {
        rssi_dbm: Some(-80),
        ..RangeObservation::ftm(1, 2, 12.0, 0.2)
    }];
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    assert_eq!(report.links[0].status, LinkStatus::Weak);
    assert_eq!(report.verdict, Verdict::Insufficient);
}

#[test]
fn noisy_link_is_not_scored() {
    // Shaped like the bench's 7.7 m run at 1.14 m: −68 dBm passes the RSSI
    // gate, but the session ranges spread 5.25 to 12.3 m.
    let positions = parse_node_positions("1:0,0,1;2:4,0,1;3:0,3,1").unwrap();
    let mut obs = vec![
        RangeObservation::ftm(1, 3, 3.2, 0.3),
        RangeObservation::ftm(2, 3, 5.3, 0.3),
    ];
    for r in [5.25, 6.9, 7.7, 7.75, 9.6, 12.3] {
        obs.push(RangeObservation {
            rssi_dbm: Some(-68),
            ..RangeObservation::ftm(1, 2, r, 0.3)
        });
    }
    let report = check(&positions, &obs, &CheckParams::ftm_uncalibrated()).unwrap();
    let link = report.links.iter().find(|l| (l.a, l.b) == (1, 2)).unwrap();
    assert_eq!(link.status, LinkStatus::Noisy, "{report:#?}");
    assert!(link.sigma_m > 1.0);
    assert_eq!(report.verdict, Verdict::Consistent);
}

#[test]
fn report_serializes_to_json() {
    let positions = parse_node_positions(&positions_string(&TRUTH, 1.0)).unwrap();
    let report = check(
        &positions,
        &ranges(&TRUTH, 0.0, 0.5, 2),
        &CheckParams::ftm_uncalibrated(),
    )
    .unwrap();
    let json = serde_json::to_value(&report).unwrap();
    assert_eq!(json["verdict"], "consistent");
    assert_eq!(json["links"].as_array().unwrap().len(), 10);
}
