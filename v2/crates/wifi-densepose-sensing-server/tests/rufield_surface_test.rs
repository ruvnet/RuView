//! ADR-262 **P3** acceptance gate — the live RuField surface.
//!
//! In-process integration test (mirrors the `/ws/sensing` / #1050 oneshot
//! style with `tower::ServiceExt::oneshot`): drives synthetic sensing cycles
//! through the real `FieldSurface` + the real `/api/field` router, and asserts:
//!
//! 1. an injected `Anonymous` (occupancy) cycle surfaces a **well-formed signed
//!    `FieldEvent`** — `Modality::WifiCsi`, privacy class consistent with the
//!    trust (P2, never P1), `is_fusable` (ed25519 receipt verifies), real
//!    timestamp;
//! 2. an empty / no-presence cycle produces **no phantom event** (explicit
//!    empty payload);
//! 3. the **privacy-safety pin** — an injected `Derived` (identity) trust state
//!    never surfaces as a low-privacy event on `/api/field` (held edge-local).
//!
//! These gates are plumbing + privacy-safety, NOT accuracy (ADR-262 §0 / §6).

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tokio::sync::RwLock;
use tower::ServiceExt; // `oneshot`

use wifi_densepose_rufield::{is_fusable, verify_event, FieldEvent, Modality, PrivacyClass};
use wifi_densepose_sensing_server::rufield_surface::{
    self, FieldState, FieldSurface, RuViewPrivacyClass, SensingClass, SensingFeatures, SignalField,
};

/// A fixed dev seed for deterministic, signed events under test.
const TEST_SEED: &[u8; 32] = b"adr262-p3-integration-test-seed!";

fn features() -> SensingFeatures {
    SensingFeatures {
        mean_rssi: -55.0,
        variance: 0.4,
        motion_band_power: 2.0,
        breathing_band_power: 0.3,
        dominant_freq_hz: 0.25,
        change_points: 1,
        spectral_power: 3.0,
    }
}

fn class(presence: bool) -> SensingClass {
    SensingClass {
        motion_level: if presence { "low".into() } else { "none".into() },
        presence,
        confidence: if presence { 0.82 } else { 0.05 },
    }
}

/// A small 2×1×2 signal field with a clear peak, so the bridge derives a real
/// (non-fabricated) position from the strongest cell.
fn signal_field() -> SignalField {
    SignalField {
        grid_size: [2, 1, 2],
        values: vec![0.1, 0.2, 0.9, 0.3], // peak at index 2
    }
}

/// Build a `FieldState` + the real `/api/field` + `/ws/field` router over it.
fn surface_router() -> (FieldState, axum::Router) {
    let state: FieldState = Arc::new(RwLock::new(FieldSurface::from_seed(TEST_SEED, true)));
    let app = rufield_surface::router(state.clone());
    (state, app)
}

/// Drive one cycle into the surface (the in-process equivalent of the live
/// sensing loop calling `emit()` per cycle).
async fn inject(state: &FieldState, trust: RuViewPrivacyClass, presence: bool, identity_bound: bool) {
    let snap = rufield_surface::build_snapshot(
        1_791_986_400_000_000_000,
        "esp32_node_7".into(),
        features(),
        class(presence),
        Some(signal_field()),
        trust,
        false, // demoted
        identity_bound,
        false, // synthetic
    );
    state.write().await.emit(&snap);
}

/// `GET /api/field` and parse the `events` array.
async fn get_field_events(app: &axum::Router) -> Vec<FieldEvent> {
    let resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/field")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK, "/api/field must return 200");
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["spec"], "rufield");
    serde_json::from_value(v["events"].clone()).expect("events array deserializes to FieldEvents")
}

#[tokio::test]
async fn gate_anonymous_cycle_surfaces_wellformed_signed_event() {
    let (state, app) = surface_router();
    inject(&state, RuViewPrivacyClass::Anonymous, true, false).await;

    let events = get_field_events(&app).await;
    assert_eq!(events.len(), 1, "one occupancy cycle ⇒ exactly one surfaced event");
    let ev = &events[0];

    // Well-formed: WiFi-CSI modality, real timestamp.
    assert_eq!(ev.tensor.modality, Modality::WifiCsi);
    assert_eq!(ev.timestamp_ns, 1_791_986_400_000_000_000);
    assert!(ev.timestamp_ns > 0, "real (non-zero) timestamp");

    // Privacy consistent with the injected trust: Anonymous → P2, NEVER P1.
    assert_eq!(ev.observation.privacy_class, PrivacyClass::P2);
    assert_ne!(ev.observation.privacy_class, PrivacyClass::P1);

    // Signed + fusable: the ed25519 receipt verifies (real, non-synthetic).
    assert!(!ev.provenance.synthetic, "live event is non-synthetic");
    assert!(verify_event(ev).is_ok(), "ed25519 signature must verify");
    assert!(is_fusable(ev), "verified receipt ⇒ fusable");

    // Real position derived from the signal-field peak (not fabricated).
    assert!(ev.observation.range_m.is_some(), "field peak ⇒ a real range readout");
}

#[tokio::test]
async fn gate_empty_cycle_produces_no_phantom_event() {
    let (state, app) = surface_router();
    // A no-presence cycle: nothing to describe.
    inject(&state, RuViewPrivacyClass::Anonymous, false, false).await;

    let events = get_field_events(&app).await;
    assert!(
        events.is_empty(),
        "no-presence cycle must surface no phantom event (explicit empty payload)"
    );
}

#[tokio::test]
async fn gate_derived_trust_never_surfaces_low_privacy() {
    // The privacy-safety pin (ADR-262 §3.3 / §6): a Derived (identity) trust
    // state maps to P4/P5 and is held edge-local — it must NEVER appear on the
    // network surface, and certainly never as a low-privacy (P1/P2) event.
    for identity_bound in [false, true] {
        let (state, app) = surface_router();
        inject(&state, RuViewPrivacyClass::Derived, true, identity_bound).await;

        let events = get_field_events(&app).await;
        assert!(
            events.is_empty(),
            "Derived cycle (identity_bound={identity_bound}) must not surface on /api/field"
        );
    }
}

#[tokio::test]
async fn gate_mixed_stream_surfaces_only_egress_safe_events() {
    // Determinism / privacy-safety over a stream: Anonymous cycles surface,
    // interleaved Derived cycles are dropped — the surface only ever carries
    // egress-safe (P1/P2) events.
    let (state, app) = surface_router();
    inject(&state, RuViewPrivacyClass::Anonymous, true, false).await; // P2 → surfaced
    inject(&state, RuViewPrivacyClass::Derived, true, false).await; // P4 → dropped
    inject(&state, RuViewPrivacyClass::Anonymous, true, false).await; // P2 → surfaced
    inject(&state, RuViewPrivacyClass::Derived, true, true).await; // P5 → dropped

    let events = get_field_events(&app).await;
    assert_eq!(events.len(), 2, "only the two Anonymous cycles surface");
    for ev in &events {
        assert_eq!(ev.observation.privacy_class, PrivacyClass::P2);
        assert!(is_fusable(ev));
    }
}
