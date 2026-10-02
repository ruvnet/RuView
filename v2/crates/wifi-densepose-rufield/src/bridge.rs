//! The conversion: `SensingSnapshot` → signed `FieldEvent` (ADR-262 P1).
//!
//! This is the in-process `SensingServerAdapter` core (ADR-262 §4 P1 / §5.1):
//! it consumes a `(SensingUpdate, TrustedOutput)` join — modelled here as a
//! [`SensingSnapshot`] of owned primitives — and emits one signed
//! [`FieldEvent`] (`Modality::WifiCsi`, axis `[Frequency]`) per cycle.

use crate::privacy::egress_class;
use crate::snapshot::{SensingSnapshot, SignalField};
use rufield_core::{
    FieldAxis, FieldEvent, FieldTensor, Modality, Observation, PrivacyClass, ProvenanceRef,
    SensorDescriptor,
};
use rufield_provenance::{sha256_hex, Signer};
use std::collections::BTreeMap;

/// Model id stamped on emitted events (ADR-262 — derived features come from
/// RuView's `/ws/sensing` pipeline, not a trained encoder).
const MODEL_ID: &str = "ruview_sensing_server_v1";

/// Firmware hash placeholder until the real ESP32 firmware image hash is wired
/// through (ADR-262 §8 open question 3 — the BLAKE3 engine witness slot). A
/// stable `sha256:` over the model id keeps it a real digest, not a fake.
fn firmware_hash() -> String {
    sha256_hex(MODEL_ID.as_bytes())
}

/// Squash a non-negative power-like scalar into `[0, 1]` deterministically.
/// `x / (x + 1)` — monotone, no panics, no calibration claim.
fn squash(x: f64) -> f32 {
    if !x.is_finite() || x <= 0.0 {
        return 0.0;
    }
    (x / (x + 1.0)) as f32
}

/// Build the `Observation.features` map the RuField fusion engine reads
/// (`rufield-fusion/engine.rs:217-228`: `motion_energy`, `breathing_band`,
/// `transient`, `presence`, `range_m`, plus `posture_height`).
fn build_features(snap: &SensingSnapshot, range_m: Option<f32>) -> BTreeMap<String, f32> {
    let f = &snap.features;
    let mut m = BTreeMap::new();
    m.insert("motion_energy".to_string(), squash(f.motion_band_power));
    m.insert("breathing_band".to_string(), squash(f.breathing_band_power));
    m.insert("transient".to_string(), squash(f.change_points as f64));
    m.insert(
        "presence".to_string(),
        if snap.classification.presence { 1.0 } else { 0.0 },
    );
    if let Some(r) = range_m {
        m.insert("range_m".to_string(), r);
    }
    m
}

/// Derive a real range (metres) and motion vector from the strongest signal
/// field peak, if a field is present. Returns `(range_m, motion_vector,
/// space_cell)` — all `None` when there is no field (we do NOT fabricate
/// coordinates, per ADR-262 §4 P1).
fn derive_position(
    field: Option<&SignalField>,
) -> (Option<f32>, Option<[f32; 3]>, Option<[i32; 3]>) {
    let Some(field) = field else {
        return (None, None, None);
    };
    let Some(cell) = field.peak_cell() else {
        return (None, None, None);
    };
    // Range from origin in grid-cell units (real readout, not calibrated
    // metres — the honesty caveat from `field_localize.rs:16-27`).
    let [x, y, z] = cell;
    let range = ((x * x + y * y + z * z) as f32).sqrt();
    let mag = if range > 0.0 { range } else { 1.0 };
    let motion_vector = [x as f32 / mag, y as f32 / mag, z as f32 / mag];
    (Some(range), Some(motion_vector), Some(cell))
}

/// Stable, deterministic event id from `(node_id, timestamp_ns)`. No RNG, so
/// the same snapshot always yields the same id (required for the determinism
/// gate).
fn event_id(snap: &SensingSnapshot) -> String {
    format!("ruview-{}-{}", snap.node_id, snap.timestamp_ns)
}

/// Convert a [`SensingSnapshot`] to a **signed** [`FieldEvent`] (ADR-262 P1).
///
/// 1. Builds a `FieldTensor` (`Modality::WifiCsi`, axis `[Frequency]`) whose
///    values are the RuView feature scalars, with the real `timestamp_ns`.
/// 2. Builds an `Observation` — `motion_vector`/`range_m`/`space_cell` derived
///    from the signal-field peak when present (else `None`; coordinates are
///    never fabricated), `confidence` from the classification, labels from
///    motion-level/presence.
/// 3. Stamps the §3.3 egress privacy class (information-content mapping with
///    the demotion floor) on both tensor and observation.
/// 4. Builds a real `ProvenanceRef` (sha256 raw hash over the tensor/feature
///    bytes, `synthetic` stamped from `snap.synthetic` — the caller's, not
///    this bridge's, judgment of where the cycle came from) and **signs** it
///    with the supplied ed25519 [`Signer`] so `rufield_provenance::is_fusable`
///    passes regardless of which way `synthetic` reads.
///
/// Determinism: with no RNG anywhere and a deterministic ed25519 signer, the
/// same `snap` + same signer seed yields a byte-identical event.
#[must_use]
pub fn snapshot_to_field_event(snap: &SensingSnapshot, signer: &Signer) -> FieldEvent {
    let class = egress_class(snap.trust_class, snap.identity_bound, snap.demoted);

    let (range_m, motion_vector, space_cell) = derive_position(snap.signal_field.as_ref());

    // ── 1. Tensor ──────────────────────────────────────────────────────────
    // The frequency-domain feature scalars, in a stable order.
    let f = &snap.features;
    let values: Vec<f32> = vec![
        f.mean_rssi as f32,
        f.variance as f32,
        f.motion_band_power as f32,
        f.breathing_band_power as f32,
        f.dominant_freq_hz as f32,
        f.spectral_power as f32,
    ];
    let confidence = (snap.classification.confidence as f32).clamp(0.0, 1.0);
    let noise_floor = f.variance.max(0.0) as f32;
    let calibration_id = format!("ruview_node_{}", snap.node_id);

    // `FieldTensor::new` only errors on a shape/axis mismatch; our shape
    // exactly matches `values.len()` and one axis, so this is infallible here.
    let tensor = FieldTensor::new(
        snap.timestamp_ns,
        Modality::WifiCsi,
        vec![FieldAxis::Frequency],
        vec![values.len()],
        values,
        confidence,
        noise_floor,
        Some(calibration_id.clone()),
        class,
    )
    .expect("feature tensor shape is well-formed by construction");

    // ── 2. Observation ─────────────────────────────────────────────────────
    let observation = Observation {
        zone_id: Some(snap.node_id.clone()),
        space_cell,
        range_m,
        velocity_mps: None,
        motion_vector,
        confidence,
        features: build_features(snap, range_m),
        labels: build_labels(snap),
        privacy_class: class,
        // ── fields added upstream since the previous submodule pin ─────────
        //
        // Each empty for its own reason rather than because a default was
        // convenient. `track_id`: this bridge emits per-cycle snapshots, not
        // tracks, and a stable per-person identifier is exactly what §3.3
        // warns about. `attributes`: nothing `features` and `labels` do not
        // already carry, and an empty map is omitted from the wire.
        // `identity_evidence` / `channel_sounding_provenance`: both are
        // structurally BLE-only — `validate_evidence_at` rejects either on any
        // other modality — so for `wifi_csi` they are not unset, they are
        // unrepresentable.
        track_id: None,
        attributes: Default::default(),
        identity_evidence: None,
        channel_sounding_provenance: None,
    };

    // ── 3. Provenance (real sha256 over the tensor bytes) ───────────────────
    let raw_hash = sha256_hex(
        &serde_json::to_vec(&tensor).expect("tensor serializes to JSON for hashing"),
    );
    let provenance = ProvenanceRef {
        raw_hash,
        firmware_hash: firmware_hash(),
        model_id: MODEL_ID.to_string(),
        calibration_id,
        synthetic: snap.synthetic,
        signature_hex: None,
        signer_pubkey_hex: None,
    };

    let sensor = SensorDescriptor {
        modality: "wifi_csi".to_string(),
        vendor: if snap.synthetic { "simulated" } else { "esp32" }.to_string(),
        device_id: snap.node_id.clone(),
        placement: "unknown".to_string(),
        // Optional sensor pose, added upstream. Left unset on purpose: a CSI
        // link has no boresight and no surveyed position here, and §6 is
        // explicit that this surface makes no validated room-coordinate claim
        // (`field_localize`). A placeholder pose would be inventing exactly
        // that claim. The ultrasonic path DOES set these, because a handheld
        // scanner genuinely knows where it was pointing.
        coordinate_frame: None,
        position_m: None,
        orientation_xyzw: None,
        clock_domain: "local".to_string(),
    };

    let mut event = FieldEvent::new(
        event_id(snap),
        snap.timestamp_ns,
        sensor,
        tensor,
        observation,
        provenance,
    );

    // ── 4. Sign (ed25519) so `is_fusable` passes for this real event ────────
    signer
        .sign_event(&mut event)
        .expect("ed25519 signing of a serializable event is infallible");

    event
}

/// Labels from the classification. These are descriptive (`person_present`,
/// `motion_<level>`); the RuField fusion engine never reads labels
/// (`event.rs:45-48`), so this carries no identity.
fn build_labels(snap: &SensingSnapshot) -> Vec<String> {
    let mut labels = Vec::new();
    if snap.classification.presence {
        labels.push("person_present".to_string());
    }
    labels.push(format!("motion_{}", snap.classification.motion_level));
    labels
}

/// Convenience: the privacy class that *would* be stamped for a snapshot,
/// without building the whole event. Useful for egress badges (P3) and tests.
#[must_use]
pub fn snapshot_egress_class(snap: &SensingSnapshot) -> PrivacyClass {
    egress_class(snap.trust_class, snap.identity_bound, snap.demoted)
}
