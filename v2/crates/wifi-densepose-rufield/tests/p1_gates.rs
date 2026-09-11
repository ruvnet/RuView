//! ADR-262 P1 acceptance gates. Each test below IS an acceptance criterion.
//!
//! - round-trip: snapshot → FieldEvent → serde → equal
//! - is_fusable: emitted event passes the §11 fusability invariant
//! - fusion ingest accept: `RuFieldFusion::ingest` accepts it + `infer` runs
//! - privacy safety: `Derived` never maps to a low-privacy class (the §3.3 trap)
//! - determinism: same snapshot + same signer seed → identical event

use rufield_core::{FusionEngine, InferenceQuery, PrivacyClass};
use rufield_fusion::RuFieldFusion;
use rufield_provenance::{is_fusable, verify_event, Signer};
use wifi_densepose_rufield::{
    map_privacy, snapshot_to_field_event, RuViewPrivacyClass, SensingClass, SensingFeatures,
    SensingSnapshot, SignalField,
};

const SEED: &[u8; 32] = b"adr-262-bridge-seed-32-bytes-ok!";

fn signer() -> Signer {
    Signer::from_seed(SEED)
}

/// A representative snapshot with a real signal field (so a position is derived).
fn sample_snapshot() -> SensingSnapshot {
    SensingSnapshot {
        timestamp_ns: 1_791_986_400_123_456_789,
        features: SensingFeatures {
            mean_rssi: -52.5,
            variance: 0.73,
            motion_band_power: 2.4,
            breathing_band_power: 0.6,
            dominant_freq_hz: 0.27,
            change_points: 2,
            spectral_power: 4.1,
        },
        classification: SensingClass {
            motion_level: "high".into(),
            presence: true,
            confidence: 0.88,
        },
        signal_field: Some(SignalField {
            grid_size: [2, 1, 2],
            // peak at flat index 2 → cell [1,0,0]
            values: vec![0.1, 0.2, 0.9, 0.3],
        }),
        trust_class: RuViewPrivacyClass::Anonymous,
        demoted: false,
        identity_bound: false,
        node_id: "esp32_room_01".into(),
        synthetic: false,
    }
}

#[test]
fn gate_round_trip_serde_equal() {
    let ev = snapshot_to_field_event(&sample_snapshot(), &signer());
    let json = serde_json::to_string(&ev).expect("serialize");
    let back: rufield_core::FieldEvent = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(ev, back, "FieldEvent must round-trip through serde unchanged");
}

#[test]
fn gate_is_fusable_verified_receipt() {
    let ev = snapshot_to_field_event(&sample_snapshot(), &signer());
    // Real (non-synthetic) event must carry a verifying ed25519 signature.
    assert!(!ev.provenance.synthetic, "live event must NOT be marked synthetic");
    assert!(ev.provenance.signature_hex.is_some(), "must be signed");
    assert!(verify_event(&ev).is_ok(), "signature must verify");
    assert!(is_fusable(&ev), "verified receipt ⇒ fusable (§11 invariant)");
}

/// Regression guard: `synthetic` must be a straight pass-through of the
/// caller's `SensingSnapshot.synthetic`, never hardcoded. Before this fix the
/// bridge always stamped `synthetic: false`, so a simulated/demo-source
/// snapshot produced an event indistinguishable from real hardware — a
/// dishonest claim this crate's own docs (§0 / §6) exist to prevent.
#[test]
fn gate_synthetic_flag_passes_through_honestly() {
    let mut snap = sample_snapshot();

    snap.synthetic = true;
    let ev = snapshot_to_field_event(&snap, &signer());
    assert!(
        ev.provenance.synthetic,
        "a snapshot marked synthetic must produce a synthetic-marked event"
    );

    snap.synthetic = false;
    let ev = snapshot_to_field_event(&snap, &signer());
    assert!(
        !ev.provenance.synthetic,
        "a snapshot marked non-synthetic must produce a non-synthetic-marked event"
    );
}

#[test]
fn gate_fusion_ingest_accepts_and_infers() {
    let ev = snapshot_to_field_event(&sample_snapshot(), &signer());
    let mut engine = RuFieldFusion::new();
    engine.ingest(ev).expect("fusion engine must accept the signed event");
    // infer() must run without error (may or may not produce inferences).
    let inferences = engine
        .infer(&InferenceQuery::all())
        .expect("infer() must run");
    // The graph recorded the event/sensor provenance nodes.
    assert!(
        engine.graph().node_count() >= 2,
        "ingest should record sensor + event nodes"
    );
    let _ = inferences; // count is not an accuracy claim
}

#[test]
fn gate_privacy_safety_derived_never_maps_to_low_privacy() {
    // THE critical §3.3 gate. Derived carries identity ⇒ P4/P5, NEVER P1.
    let p4 = map_privacy(RuViewPrivacyClass::Derived, false);
    let p5 = map_privacy(RuViewPrivacyClass::Derived, true);
    assert_eq!(p4, PrivacyClass::P4);
    assert_eq!(p5, PrivacyClass::P5);
    assert!(p4 >= PrivacyClass::P4, "Derived must be in the identity tier");
    assert_ne!(p4, PrivacyClass::P1, "Derived must NEVER be P1");

    // And end-to-end: an emitted event from a Derived snapshot must be P4/P5.
    let mut snap = sample_snapshot();
    snap.trust_class = RuViewPrivacyClass::Derived;
    let ev = snapshot_to_field_event(&snap, &signer());
    assert!(
        ev.observation.privacy_class >= PrivacyClass::P4,
        "emitted Derived event must be P4 or P5, got {:?}",
        ev.observation.privacy_class
    );
    assert_eq!(ev.observation.privacy_class, ev.tensor.privacy_class);
}

/// Full §3.3 table over every RuView class → expected RuField class.
#[test]
fn gate_privacy_table_over_every_ruview_class() {
    let cases = [
        (RuViewPrivacyClass::Raw, false, PrivacyClass::P0),
        (RuViewPrivacyClass::Derived, false, PrivacyClass::P4),
        (RuViewPrivacyClass::Derived, true, PrivacyClass::P5),
        (RuViewPrivacyClass::Anonymous, false, PrivacyClass::P2),
        (RuViewPrivacyClass::Restricted, false, PrivacyClass::P2),
    ];
    for (ruview, id_bound, expected) in cases {
        assert_eq!(
            map_privacy(ruview, id_bound),
            expected,
            "{ruview:?} (identity_bound={id_bound}) must map to {expected:?}"
        );
    }
}

/// Fail-closed: a demoted Raw snapshot must NOT emit P0 (raw) — it floors to P2.
#[test]
fn gate_demotion_is_fail_closed() {
    let mut snap = sample_snapshot();
    snap.trust_class = RuViewPrivacyClass::Raw; // would be P0
    snap.demoted = true; // governed engine demotion
    let ev = snapshot_to_field_event(&snap, &signer());
    assert!(
        ev.observation.privacy_class >= PrivacyClass::P2,
        "demoted cycle must floor to >= P2, got {:?}",
        ev.observation.privacy_class
    );
}

#[test]
fn gate_determinism_same_seed_identical_event() {
    let snap = sample_snapshot();
    let a = snapshot_to_field_event(&snap, &Signer::from_seed(SEED));
    let b = snapshot_to_field_event(&snap, &Signer::from_seed(SEED));
    assert_eq!(a, b, "same snapshot + same signer seed ⇒ identical event");
    // Including the signature (ed25519 is deterministic).
    assert_eq!(a.provenance.signature_hex, b.provenance.signature_hex);
}

#[test]
fn no_fabricated_position_when_field_absent() {
    let mut snap = sample_snapshot();
    snap.signal_field = None;
    let ev = snapshot_to_field_event(&snap, &signer());
    assert!(ev.observation.range_m.is_none(), "no field ⇒ no fabricated range");
    assert!(ev.observation.space_cell.is_none(), "no field ⇒ no fabricated cell");
    assert!(
        ev.observation.motion_vector.is_none(),
        "no field ⇒ no fabricated motion vector"
    );
}

#[test]
fn derives_real_position_from_field_peak() {
    let ev = snapshot_to_field_event(&sample_snapshot(), &signer());
    // peak at flat index 2, grid [2,1,2] (row-major) → cell [1,0,0]
    assert_eq!(ev.observation.space_cell, Some([1, 0, 0]));
    assert_eq!(ev.observation.range_m, Some(1.0));
}
