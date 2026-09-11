//! Wires the ADR-302 OOD gate (`ruview-ood`) into the live sensing loop.
//!
//! Before this module, RuView's entire ADR-300 honesty substrate — the
//! witness chain, authenticated sensor identity, capability certificates, and
//! this OOD gate — compiled and passed its own unit tests but was never
//! reachable from anything that actually runs: `wifi-densepose-sensing-server`
//! depended on none of it. A live classification could report `person_present`
//! with zero domain-generalization check, no matter how far the room had
//! drifted from whatever it was tuned on. This module is the first strand
//! connected: an **opt-in** (`--calibration-certificate <path>`) gate that
//! attaches a KNOWN/DEGRADED/UNKNOWN verdict to each cycle's classification.
//!
//! ## What is NOT solved here
//!
//! - **Minting a real certificate still requires the offline
//!   `wifi-densepose-cli calibrate`/`enroll`/`train-room` pipeline** (ADR-151).
//!   This module only *consumes* a certificate; it does not calibrate a room.
//! - **The live fingerprint fed to the gate is a documented approximation**
//!   ([`live_fingerprint_from_stats`]), built from the scalar features the
//!   live loop already tracks (`mean_rssi`, `variance`, `motion_band_power`).
//!   It is NOT the same rigor as [`RoomFingerprint::from_bank`] (an enrolled
//!   `SpecialistBank`'s presence-gate statistics) — the gate still functions
//!   correctly (drift is still monotonic and comparable across cycles), but a
//!   consumer should not read `distance` here as calibration-grade.
//!   **Empirically confirmed, not just theoretical**: live-verifying this
//!   module against a hand-minted certificate whose fingerprint was chosen to
//!   closely match the live simulated feed's own observed scalars still
//!   produced `distance.total` around 0.9 (squashed distance is `[0,1)`; the
//!   default envelope's outer threshold is 0.15) — nowhere near KNOWN. The
//!   live approximation's raw dBm/variance units evidently don't sit on the
//!   scale `RoomFingerprint::distance`'s constants (`MEAN_SCALE`/`VAR_SCALE`)
//!   were tuned for, which were fit against real enrolled-bank statistics.
//!   Practical effect: expect this gate to report `DEGRADED`/`UNKNOWN` far
//!   more often than a real per-room `SpecialistBank`-derived fingerprint
//!   would, even for a genuinely fine room — a real deployment needs either a
//!   rescaled live fingerprint or (better) the enrolled-bank fingerprint kept
//!   live-updated, neither of which this module does yet.
//! - **The fourth ADR-302 input, model uncertainty, is not available.** The
//!   live classifier is heuristic, not a model with a calibrated predictive
//!   uncertainty head — there is nothing honest to report there yet, so it is
//!   fixed at `0.0` (never escalates the gate) and the response says so
//!   explicitly (`"uncertainty_available": false`) rather than pretending a
//!   score exists.

use std::sync::Arc;

use ruview_ood::{
    assess_certificate, gate::Inference, CompatibilityEnvelope, ExpectedIdentity, InferenceGate,
    RoomFingerprint, SignalQuality,
};
use wifi_densepose_calibration::certificate::{CalibrationCertificate, KeyedHashSigner};

/// Current wall-clock time as Unix seconds, for callers that need to pass
/// `now_unix_s` into [`evaluate`] but have no injected clock of their own
/// (the live sensing loop, unlike this module's own pure core, is not
/// required to be deterministic). Saturates to `0` rather than panicking on
/// a pre-1970 clock.
#[must_use]
pub fn now_unix_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Env var carrying the calibration certificate's verification key id.
/// Required when `--calibration-certificate` is set.
pub const CALIBRATION_KEY_ID_ENV: &str = "WDP_CALIBRATION_KEY_ID";
/// Env var carrying the calibration certificate's verification secret.
/// Required when `--calibration-certificate` is set. Never logged.
pub const CALIBRATION_KEY_SECRET_ENV: &str = "WDP_CALIBRATION_KEY_SECRET";

/// Loaded, verified OOD-gate state: the certificate, its verifier, the
/// expected space/device identity, and the gate itself. Held once at startup
/// (loading is fallible and deliberately fails closed — see [`load`]).
#[derive(Debug)]
pub struct OodGateState {
    certificate: CalibrationCertificate,
    verifier: KeyedHashSigner,
    space_id: String,
    device_id: String,
    gate: InferenceGate,
}

/// Load a calibration certificate from `path`, verify its signature
/// immediately using [`CALIBRATION_KEY_ID_ENV`]/[`CALIBRATION_KEY_SECRET_ENV`],
/// and bind it to `(space_id, device_id)`.
///
/// Fails closed: any error here (missing file, malformed JSON, missing env
/// vars, bad signature) is returned to the caller, which is expected to abort
/// startup rather than run with a gate that silently never gates anything.
/// A certificate that merely doesn't verify is a startup error, not a
/// runtime `UNKNOWN` — an operator who passed `--calibration-certificate`
/// asked for gating and deserves to know immediately if the file they gave
/// is broken, not have it silently do nothing.
pub fn load(
    path: &str,
    space_id: impl Into<String>,
    device_id: impl Into<String>,
) -> Result<OodGateState, String> {
    let json = std::fs::read_to_string(path)
        .map_err(|e| format!("read calibration certificate {path}: {e}"))?;
    let certificate = CalibrationCertificate::from_json(&json)
        .map_err(|e| format!("parse calibration certificate {path}: {e}"))?;

    let key_id = std::env::var(CALIBRATION_KEY_ID_ENV)
        .map_err(|_| format!("{CALIBRATION_KEY_ID_ENV} is not set (required with --calibration-certificate)"))?;
    let key_secret = std::env::var(CALIBRATION_KEY_SECRET_ENV).map_err(|_| {
        format!("{CALIBRATION_KEY_SECRET_ENV} is not set (required with --calibration-certificate)")
    })?;
    let verifier = KeyedHashSigner::new(key_id, key_secret.into_bytes());

    if !certificate.verify_signature(&verifier) {
        return Err(format!(
            "calibration certificate {path} failed signature verification — refusing to start gated \
             (check {CALIBRATION_KEY_ID_ENV}/{CALIBRATION_KEY_SECRET_ENV} match the minting key)"
        ));
    }

    Ok(OodGateState {
        certificate,
        verifier,
        space_id: space_id.into(),
        device_id: device_id.into(),
        gate: InferenceGate::default(),
    })
}

/// Build a live [`RoomFingerprint`] approximation from the scalar features the
/// live sensing loop already computes every cycle. See the module docs'
/// honesty caveat — this is NOT the enrolled-bank rigor of
/// [`RoomFingerprint::from_bank`], just an honestly-labelled best-effort
/// proxy built from the same scalars every consumer of `FeatureInfo` already
/// sees, so the OOD gate has *something* comparable to drift-check against
/// without requiring a second live-calibration subsystem.
#[must_use]
pub fn live_fingerprint_from_stats(mean_rssi: f64, variance: f64, motion_band_power: f64) -> RoomFingerprint {
    RoomFingerprint {
        schema_version: 1,
        empty_mean: mean_rssi as f32,
        empty_variance: variance.max(0.0) as f32,
        occupied_variance: (variance + motion_band_power).max(0.0) as f32,
        // Not derivable from live scalars alone; zeroed rather than guessed
        // (RoomFingerprint::from_bank does the same when a bank has no
        // presence gate — this mirrors that documented "unavailable" convention).
        presence_threshold: 0.0,
        occupancy_mean_shift: motion_band_power as f32,
        geometry: Default::default(),
    }
}

/// Gate one cycle's classification. Pure given its inputs (no clock read —
/// `now_unix_s` is the caller's, `Arc` avoids cloning the loaded certificate
/// per cycle). Returns a JSON view combining the gate's full decision
/// ([`ruview_ood::GatedInference`], which already carries state, cause,
/// distance, signal quality, calibration compatibility, and any
/// recalibration signal) with the honesty caveat about the missing fourth
/// input.
#[must_use]
pub fn evaluate(
    state: &Arc<OodGateState>,
    live_fingerprint: &RoomFingerprint,
    signal_quality_score: f32,
    contradiction: bool,
    frame_valid: bool,
    confidence: f32,
    now_unix_s: i64,
) -> serde_json::Value {
    let expected = ExpectedIdentity {
        space_id: &state.space_id,
        device_id: &state.device_id,
    };
    let (distance, compat) = assess_certificate(
        &state.certificate,
        live_fingerprint,
        expected,
        now_unix_s,
        &state.verifier,
    );
    let quality = SignalQuality::new(signal_quality_score.clamp(0.0, 1.0), contradiction, frame_valid)
        .unwrap_or(SignalQuality {
            score: 0.0,
            contradiction: true,
            valid: false,
        });
    // Fourth ADR-302 input (uncertainty) is unavailable from the heuristic
    // classifier — fixed at 0.0, which never escalates the gate on its own
    // (see InferenceGate::evaluate: only uncertainty > max_uncertainty_known
    // escalates, and 0.0 never exceeds a positive threshold).
    let inference = Inference::new((), confidence, 0.0);
    let envelope: CompatibilityEnvelope = state.certificate.envelope;
    let gated = state.gate.evaluate(inference, distance, envelope, quality, compat);

    let mut view = serde_json::to_value(&gated).unwrap_or(serde_json::Value::Null);
    if let Some(obj) = view.as_object_mut() {
        obj.insert("uncertainty_available".to_string(), serde_json::Value::Bool(false));
        obj.insert(
            "fingerprint_source".to_string(),
            serde_json::Value::String("live_approximation".to_string()),
        );
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_calibration::bank::SpecialistBank;
    use wifi_densepose_calibration::certificate::{
        CalibrationTier, CharacterizationSource, EvidenceLevel, MintParams,
    };

    /// `key_id` must equal the certificate's `sensor_id` (`mint` enforces
    /// this) — always build the signer from the same device id the
    /// certificate is minted for, rather than a fixed literal, so a test
    /// cannot accidentally mismatch the two.
    fn signer_for(key_id: &str) -> KeyedHashSigner {
        KeyedHashSigner::new(key_id, b"test-secret-do-not-use-in-prod".to_vec())
    }

    /// A minimal, empty specialist bank — no anchors trained, just enough
    /// structure to derive a (zeroed) `RoomFingerprint` and mint a synthetic
    /// test certificate. `SpecialistBank` has no `Default`/no-anchor
    /// constructor of its own (`train` requires ≥1 anchor), so this builds
    /// the literal directly — every field is `pub`, this is not reaching
    /// around an invariant, just skipping enrollment for a unit test.
    fn empty_bank() -> SpecialistBank {
        SpecialistBank {
            room_id: "room/test".to_string(),
            baseline_id: "baseline/test".to_string(),
            trained_at_unix_s: 0,
            anchor_count: 0,
            geometry: Vec::new(),
            presence: None,
            posture: None,
            breathing: Default::default(),
            heartbeat: Default::default(),
            restlessness: None,
            anomaly: None,
        }
    }

    fn minted_state(space_id: &str, device_id: &str, now: i64, validity_secs: i64) -> Arc<OodGateState> {
        let bank = empty_bank();
        let fingerprint = RoomFingerprint::from_bank(&bank);
        let s = signer_for(device_id);
        let params = MintParams {
            space_id: space_id.to_string(),
            sensor_id: device_id.to_string(),
            captured_at_unix_s: now,
            validity_secs,
            version: 1,
            tier: CalibrationTier::Auto,
            evidence: EvidenceLevel::L0Synthetic,
            source: CharacterizationSource::Synthetic,
            envelope: CompatibilityEnvelope::default(),
        };
        let certificate =
            CalibrationCertificate::mint(params, &bank, &s).expect("synthetic certificate mints");
        assert!(certificate.verify_signature(&s), "test fixture must self-verify");
        let _ = fingerprint; // documents the certificate's own fingerprint derivation path
        Arc::new(OodGateState {
            certificate,
            verifier: s,
            space_id: space_id.to_string(),
            device_id: device_id.to_string(),
            gate: InferenceGate::default(),
        })
    }

    #[test]
    fn fresh_certificate_and_matching_live_fingerprint_gate_known() {
        let now = 1_000_000;
        let state = minted_state("space/a", "sensor/a", now, 3600);
        let live = RoomFingerprint::from_bank(&empty_bank());

        let view = evaluate(&state, &live, 0.95, false, true, 0.9, now);
        assert_eq!(view["state"], serde_json::json!("Known"));
        assert_eq!(view["calibration_compat"], serde_json::json!("Valid"));
        assert_eq!(view["uncertainty_available"], serde_json::json!(false));
        assert_eq!(view["fingerprint_source"], serde_json::json!("live_approximation"));
        // KNOWN must still return the class/confidence, not suppress it.
        assert!(view["confidence"].is_number());
    }

    #[test]
    fn expired_certificate_gates_unknown_regardless_of_drift() {
        let now = 1_000_000;
        // validity_secs = 10 → already expired by the time we evaluate far later.
        let state = minted_state("space/a", "sensor/a", now, 10);
        let live = RoomFingerprint::from_bank(&empty_bank());

        let view = evaluate(&state, &live, 0.99, false, true, 0.99, now + 10_000);
        assert_eq!(
            view["state"],
            serde_json::json!({"Unknown": "CertificateExpired"})
        );
        // The confident class must be suppressed — ADR-300 rule 1.
        assert!(view["confidence"].is_null());
        assert!(view["class"].is_null());
    }

    #[test]
    fn device_mismatch_gates_unknown_even_with_a_valid_signature() {
        let now = 1_000_000;
        let state = minted_state("space/a", "sensor/a", now, 3600);
        // Same OodGateState object, but evaluated as if it were a different
        // device's cycle — exercised via a hand-built mismatched state.
        let mismatched = Arc::new(OodGateState {
            certificate: state.certificate.clone(),
            // Verifies with the key that actually signed the certificate
            // ("sensor/a", matching `minted_state` above) — the mismatch
            // under test is the *expected* device_id below, not the signer.
            verifier: signer_for("sensor/a"),
            space_id: "space/a".to_string(),
            device_id: "sensor/DIFFERENT".to_string(),
            gate: InferenceGate::default(),
        });
        let live = RoomFingerprint::from_bank(&empty_bank());

        let view = evaluate(&mismatched, &live, 0.95, false, true, 0.9, now);
        assert_eq!(view["state"], serde_json::json!({"Unknown": "DeviceMismatch"}));
    }

    #[test]
    fn live_fingerprint_from_stats_is_deterministic_and_maps_fields_honestly() {
        let a = live_fingerprint_from_stats(-50.0, 2.0, 1.5);
        let b = live_fingerprint_from_stats(-50.0, 2.0, 1.5);
        assert_eq!(a, b, "same inputs must yield byte-identical fingerprints");
        assert_eq!(a.empty_mean, -50.0);
        assert_eq!(a.empty_variance, 2.0);
        assert_eq!(a.occupied_variance, 3.5);
        assert_eq!(a.occupancy_mean_shift, 1.5);
        assert_eq!(a.presence_threshold, 0.0, "not derivable live — honestly zeroed, not guessed");
    }

    #[test]
    fn load_rejects_a_missing_file() {
        // SAFETY: single-threaded test process; no concurrent env mutation.
        std::env::set_var(CALIBRATION_KEY_ID_ENV, "x");
        std::env::set_var(CALIBRATION_KEY_SECRET_ENV, "y");
        let err = load("/nonexistent/path/does-not-exist.json", "space/a", "sensor/a")
            .expect_err("missing file must fail closed");
        assert!(err.contains("read calibration certificate"), "{err}");
        std::env::remove_var(CALIBRATION_KEY_ID_ENV);
        std::env::remove_var(CALIBRATION_KEY_SECRET_ENV);
    }

    #[test]
    fn load_rejects_when_key_env_vars_are_absent() {
        std::env::remove_var(CALIBRATION_KEY_ID_ENV);
        std::env::remove_var(CALIBRATION_KEY_SECRET_ENV);
        let dir = std::env::temp_dir().join(format!("wdp-ood-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("cert.json");

        let bank = empty_bank();
        let s = signer_for("test-sensor");
        let params = MintParams {
            space_id: "space/a".to_string(),
            sensor_id: "test-sensor".to_string(),
            captured_at_unix_s: 0,
            validity_secs: 3600,
            version: 1,
            tier: CalibrationTier::Auto,
            evidence: EvidenceLevel::L0Synthetic,
            source: CharacterizationSource::Synthetic,
            envelope: CompatibilityEnvelope::default(),
        };
        let cert = CalibrationCertificate::mint(params, &bank, &s).unwrap();
        std::fs::write(&path, cert.to_json().unwrap()).unwrap();

        let err = load(path.to_str().unwrap(), "space/a", "test-sensor")
            .expect_err("missing env vars must fail closed, never silently skip verification");
        assert!(err.contains(CALIBRATION_KEY_ID_ENV), "{err}");

        std::fs::remove_dir_all(&dir).ok();
    }
}
