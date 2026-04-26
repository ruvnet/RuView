//! Deterministic proof bundle — Pass 6 of the implementation plan.
//!
//! Mirrors the `archive/v1/data/proof/verify.py` pattern: feed a known
//! reference scene through the full pipeline, hash the output, and compare
//! against a published witness. If the hash matches, the simulator's
//! physics constants and code paths are byte-identical to the published
//! reference. If it doesn't, *something* drifted — and the test surfaces
//! it loudly.
//!
//! # The reference scenario
//!
//! [`Proof::REFERENCE_SCENE_JSON`] is a small ferrous-anomaly scene that
//! exercises every primitive type ([`crate::scene::DipoleSource`],
//! [`crate::scene::CurrentLoop`], [`crate::scene::FerrousObject`]) plus a
//! single sensor at the origin and a non-zero ambient field. The
//! [`PipelineConfig::default`] applies COTS-grade physics and seed `42`
//! drives the shot-noise stream.
//!
//! # The witness
//!
//! [`Proof::EXPECTED_WITNESS`] is the SHA-256 over the concatenated
//! [`crate::MagFrame`] bytes of running the reference scene for
//! [`Proof::N_SAMPLES`] samples. Stored as a hex constant in this module
//! so the test suite can re-derive and assert it.
//!
//! # What the proof guards against
//!
//! - **Silent constant drift** — anyone changing `D_GS`, `GAMMA_E`, `MU_0`,
//!   contrast, or T₂* defaults shifts the witness; the test fails.
//! - **PRNG regressions** — same seed → same byte stream is the
//!   deterministic-witness contract. If `rand_chacha` ever changes its
//!   stream layout, the witness changes and CI catches it.
//! - **Frame-format drift** — any change to [`crate::MagFrame`]'s
//!   serialisation (field reordering, magic bump, layout shift) shifts
//!   the witness.
//! - **Pipeline-stage drift** — adding a stage, reordering, or changing
//!   the LSQ inversion constant shifts the witness.

use crate::pipeline::{Pipeline, PipelineConfig};
use crate::scene::Scene;
use crate::NvsimError;

/// Deterministic-proof harness for nvsim.
pub struct Proof;

impl Proof {
    /// Number of samples in the reference run. Picked small enough that
    /// the test runs in milliseconds; large enough that any drift in the
    /// pipeline's per-sample arithmetic produces a different hash.
    pub const N_SAMPLES: usize = 256;

    /// Deterministic seed for the shot-noise PRNG.
    pub const SEED: u64 = 42;

    /// Reference scene — JSON form, parsed at runtime so the test
    /// suite can serialise it back out for sanity-checking. Exercises
    /// every primitive type the simulator supports.
    pub const REFERENCE_SCENE_JSON: &'static str = r#"{
        "dipoles": [
            {"position": [0.0, 0.0, 0.5], "moment": [0.0, 0.0, 1.0e-3]},
            {"position": [0.3, 0.0, 0.4], "moment": [1.0e-4, 5.0e-5, 0.0]}
        ],
        "loops": [
            {"centre": [0.0, 0.2, 0.6], "normal": [0.0, 1.0, 0.0], "radius": 0.05, "current": 0.5, "n_segments": 64}
        ],
        "ferrous": [
            {"position": [0.5, 0.0, 0.0], "volume": 1.0e-4, "susceptibility": 5000.0}
        ],
        "eddy": [],
        "sensors": [[0.0, 0.0, 0.0]],
        "ambient_field": [1.0e-6, 0.0, 0.0]
    }"#;

    /// Build the reference scene by parsing [`REFERENCE_SCENE_JSON`].
    pub fn reference_scene() -> Result<Scene, NvsimError> {
        Ok(serde_json::from_str(Self::REFERENCE_SCENE_JSON)?)
    }

    /// Run the reference pipeline and return its SHA-256 witness.
    ///
    /// Same `(scene, config, seed)` produces byte-identical witnesses
    /// across runs and machines — that's the determinism contract this
    /// proof guards.
    pub fn generate() -> Result<[u8; 32], NvsimError> {
        let scene = Self::reference_scene()?;
        let cfg = PipelineConfig::default();
        let pipeline = Pipeline::new(scene, cfg, Self::SEED);
        let (_, witness) = pipeline.run_with_witness(Self::N_SAMPLES);
        Ok(witness)
    }

    /// Verify the reference pipeline against the supplied expected hash.
    /// Returns `Ok(())` iff the regenerated witness matches; otherwise
    /// returns the actual hash so the caller can update the published
    /// constant after auditing the drift.
    pub fn verify(expected: &[u8; 32]) -> Result<(), [u8; 32]> {
        let actual = Self::generate().map_err(|_| [0u8; 32])?;
        if &actual == expected {
            Ok(())
        } else {
            Err(actual)
        }
    }

    /// Render a 32-byte hash as 64 hex characters. Used by the test suite
    /// to format failure messages so the developer can update the published
    /// constant without re-running `xxd`.
    pub fn hex(witness: &[u8; 32]) -> String {
        let mut s = String::with_capacity(64);
        for b in witness {
            s.push_str(&format!("{b:02x}"));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_scene_parses() {
        let scene = Proof::reference_scene().expect("reference scene must parse");
        assert_eq!(scene.dipoles.len(), 2);
        assert_eq!(scene.loops.len(), 1);
        assert_eq!(scene.ferrous.len(), 1);
        assert_eq!(scene.sensors.len(), 1);
        assert_eq!(scene.ambient_field, [1.0e-6, 0.0, 0.0]);
    }

    #[test]
    fn proof_generate_is_deterministic_across_runs() {
        // Same Proof::generate() must produce byte-identical witnesses
        // across repeated calls — the determinism contract the proof
        // bundle exists to guard.
        let w1 = Proof::generate().unwrap();
        let w2 = Proof::generate().unwrap();
        assert_eq!(w1, w2);
    }

    #[test]
    fn proof_witness_changes_when_seed_changes() {
        // Sanity: a different seed must produce a different witness, or
        // the seed isn't actually being used.
        let w1 = Proof::generate().unwrap();
        let scene = Proof::reference_scene().unwrap();
        let cfg = PipelineConfig::default();
        let p = Pipeline::new(scene, cfg, Proof::SEED + 1);
        let (_, w2) = p.run_with_witness(Proof::N_SAMPLES);
        assert_ne!(w1, w2);
    }

    #[test]
    fn proof_hex_formats_64_chars() {
        let bytes = [0xAB_u8; 32];
        let hex = Proof::hex(&bytes);
        assert_eq!(hex.len(), 64);
        assert_eq!(hex, "ab".repeat(32));
    }

    #[test]
    fn proof_witness_publishes_a_known_value() {
        // Pin the published witness so any future drift in the simulator's
        // physics, PRNG, frame format, or pipeline ordering surfaces here.
        // If this test fails, audit the change. If the change is intentional,
        // re-derive the new witness with `Proof::hex(&Proof::generate()?)`
        // and update the constant below.
        let actual = Proof::generate().unwrap();
        let actual_hex = Proof::hex(&actual);
        let published_hex = include_published_witness();
        assert_eq!(
            actual_hex, published_hex,
            "Proof witness drifted. Audit the change, then update PUBLISHED_WITNESS_HEX."
        );
    }

    /// Published witness for the reference scene at SEED = 42, N_SAMPLES = 256.
    /// Computed from this test suite on first build; subsequent runs assert
    /// byte-equivalence.
    fn include_published_witness() -> &'static str {
        // The very first run computes this; we pin it from `Proof::generate`
        // executed in this test on first invocation. Hard-coded after capture.
        PUBLISHED_WITNESS_HEX
    }

    /// Captured first-run-on-x86_64-Windows. Same `(scene, seed=42,
    /// n_samples=256, PipelineConfig::default())` must reproduce on every
    /// machine, every run. Drift = audit + update.
    const PUBLISHED_WITNESS_HEX: &str =
        "cc8de9b01b0ff5bd97a6c17848a3f156c174ea7589d0888164a441584ec593b4";
}
