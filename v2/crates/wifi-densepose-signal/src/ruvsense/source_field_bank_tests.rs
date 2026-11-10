use super::*;

// SYNTHETIC software fixtures, not measured RF or a physical qualification.
fn config() -> FieldModelConfig {
    FieldModelConfig {
        n_links: 1,
        n_subcarriers: 4,
        n_modes: 3,
        min_calibration_frames: 16,
        min_calibration_duration_s: 0.0,
        baseline_expiry_s: 10.0,
    }
}
fn spec(id: u8) -> SourceFieldSpec {
    SourceFieldSpec {
        source_id: id,
        grid_id: format!("grid-{id}"),
        config: config(),
    }
}
fn bank() -> SourceFieldBank {
    SourceFieldBank::new(vec![spec(15), spec(16)], 1000).unwrap()
}
fn frame(base: f64, i: u64) -> Vec<f64> {
    (0..4)
        .map(|j| base + j as f64 * 0.1 + i as f64 * 0.01)
        .collect()
}
fn train(bank: &mut SourceFieldBank, id: u8, base: f64) {
    for i in 0..16 {
        bank.feed(id, &format!("grid-{id}"), i, i + 1, &frame(base, i))
            .unwrap();
    }
}

#[test]
fn source_scale_does_not_change_the_other_sources_model() {
    let mut bank = bank();
    let mut control = FieldModel::new(config()).unwrap();
    for i in 0..16 {
        let f = frame(10.0, i);
        control.feed_calibration(&[f.clone()]).unwrap();
        bank.feed(15, "grid-15", i, i + 1, &f).unwrap();
        bank.feed(16, "grid-16", i, i + 1, &frame(10000.0, i))
            .unwrap();
    }
    control.finalize_calibration(100, 42).unwrap();
    bank.freeze(100, 42).unwrap();
    let frozen = bank.frozen_models().unwrap();
    assert_eq!(
        frozen.iter().map(|s| s.source_id).collect::<Vec<_>>(),
        vec![15, 16]
    );
    assert_eq!(
        frozen[0].snapshot.modes,
        control.export_snapshot().unwrap().modes
    );
    assert!(frozen[1].snapshot.modes.baseline[0][0] > 9000.0);
}

#[test]
fn observed_or_missing_members_cannot_borrow_another_members_frames() {
    let mut b = bank();
    train(&mut b, 15, 10.0);
    b.feed(16, "grid-16", 0, 1, &frame(50.0, 0)).unwrap();
    assert!(matches!(
        b.freeze(100, 42),
        Err(SourceBankError::IncompleteSource(16))
    ));
    assert_eq!(b.phase(), SourceBankPhase::Collecting);
    assert!(b.frozen_models().is_err());
    assert_eq!(b.frame_counts()[&16], 1);
}

#[test]
fn invalid_shapes_and_nonfinite_frames_invalidate_the_generation() {
    for bad in [
        vec![1.0],
        vec![f64::NAN; 4],
        vec![f64::INFINITY; 4],
        vec![f64::MAX; 4],
    ] {
        let mut b = bank();
        assert!(matches!(
            b.feed(15, "grid-15", 0, 1, &bad),
            Err(SourceBankError::SourceChanged(15))
        ));
        assert_eq!(b.phase(), SourceBankPhase::Invalid);
        assert!(b.freeze(2, 42).is_err());
    }
}

#[test]
fn grid_sequence_clock_and_continuity_changes_are_not_partial_success() {
    for (grid, ordinal, time) in [
        ("other", 1, 2),
        ("grid-15", 0, 2),
        ("grid-15", 1, 1),
        ("grid-15", 1, 1002),
    ] {
        let mut b = bank();
        b.feed(15, "grid-15", 0, 1, &frame(10.0, 0)).unwrap();
        assert!(b.feed(15, grid, ordinal, time, &frame(10.0, 1)).is_err());
        assert_eq!(b.phase(), SourceBankPhase::Invalid);
        assert!(b.frozen_models().is_err());
    }
}

#[test]
fn stale_and_future_source_windows_cannot_freeze() {
    let mut b = bank();
    train(&mut b, 15, 10.0);
    train(&mut b, 16, 20.0);
    assert!(matches!(
        b.freeze(1017, 42),
        Err(SourceBankError::StaleSource(15))
    ));
    assert!(matches!(
        b.freeze(15, 42),
        Err(SourceBankError::StaleSource(15))
    ));
    assert_eq!(b.phase(), SourceBankPhase::Collecting);
    assert!(b.frozen_models().is_err());
}

#[test]
fn freeze_is_immutable_and_does_not_renew_model_age() {
    let mut b = bank();
    train(&mut b, 15, 10.0);
    train(&mut b, 16, 20.0);
    b.freeze(100, 42).unwrap();
    assert!(b.feed(15, "grid-15", 16, 101, &frame(10.0, 16)).is_err());
    assert!(b.freeze(200, 42).is_err());
    assert!(b.freshness(99).is_err());
    assert!(b
        .freshness(6_000_100)
        .unwrap()
        .values()
        .all(|x| *x == CalibrationStatus::Stale));
    assert!(b
        .freshness(11_000_100)
        .unwrap()
        .values()
        .all(|x| *x == CalibrationStatus::Expired));
    assert!(b
        .frozen_models()
        .unwrap()
        .iter()
        .all(|s| s.snapshot.modes.calibrated_at_us == 100));
}

#[test]
fn configuration_is_bounded_and_unknown_nodes_do_not_enter_training() {
    assert!(SourceFieldBank::new(vec![spec(15)], 1000).is_err());
    assert!(SourceFieldBank::new(vec![spec(15), spec(15)], 1000).is_err());
    assert!(SourceFieldBank::new(vec![spec(15), spec(16)], 0).is_err());
    let mut wide = spec(16);
    wide.config.n_subcarriers = 10000;
    assert!(SourceFieldBank::new(vec![spec(15), wide], 1000).is_err());
    let mut b = bank();
    assert!(matches!(
        b.feed(99, "grid-99", 0, 1, &frame(1.0, 0)),
        Err(SourceBankError::UnknownSource(99))
    ));
    assert_eq!(b.phase(), SourceBankPhase::Collecting);
    assert!(b.frame_counts().values().all(|n| *n == 0));
}

#[test]
fn shadow_windows_compare_individually_and_abstain_on_incomplete_or_stale_sets() {
    let mut b = bank();
    for i in 0..1000 {
        b.feed(15, "grid-15", i, i + 1, &frame(10.0, i)).unwrap();
        b.feed(16, "grid-16", i, i + 1, &frame(10000.0, i)).unwrap();
    }
    b.freeze(1500, 42).unwrap();
    let quiet: Vec<Vec<f64>> = (450..500).map(|i| frame(10.0, i)).collect();
    let changed: Vec<Vec<f64>> = (450..500).map(|i| frame(11000.0, i)).collect();
    let windows = [
        SourceFieldWindow {
            source_id: 15,
            grid_id: "grid-15",
            observed_at_us: 1990,
            frames: &quiet,
        },
        SourceFieldWindow {
            source_id: 16,
            grid_id: "grid-16",
            observed_at_us: 1990,
            frames: &changed,
        },
    ];
    let result = b.compare_empty_windows(2000, &windows).unwrap();
    assert!(result[&15].matches_empty);
    assert!(!result[&16].matches_empty);
    assert!(b.compare_empty_windows(2000, &windows[..1]).is_err());
    assert!(b.compare_empty_windows(4000, &windows).is_err());
    assert!(b.compare_empty_windows(1980, &windows).is_err());
    assert!(b.compare_empty_windows(6_001_500, &windows).is_err());
    assert_eq!(b.phase(), SourceBankPhase::Frozen);
}
