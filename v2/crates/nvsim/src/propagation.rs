//! Per-material magnetic-field attenuation along sensor–source line-of-sight
//! segments — Pass 3 of the implementation plan.
//!
//! Free-space `1/r³` falloff lives in [`crate::source`] (it's part of the
//! dipole formula). This layer applies *additional* attenuation when the LoS
//! crosses material slabs of known thickness. Default — for air / vacuum —
//! is the identity transform.
//!
//! # Primary sources
//!
//! - Jackson, *Classical Electrodynamics* 3e (1999) §5.8, §8.1 — skin depth.
//! - Cullity & Graham, *Introduction to Magnetic Materials* 2e (2009) Ch. 2.
//! - Ulrich, *NDT&E Int.* 35 (2002) — concrete-attenuation proxy (cited as
//!   *proxy*; the real research gap is plan §6.3).
//!
//! # Honest scope
//!
//! Plan §2.2 explicitly marks drywall / brick / dry-concrete loss values as
//! **conjectural** with defensible defaults. We re-state that here in code:
//! the table is the best public-domain estimate at DC–10 kHz, but no
//! systematic measurement of residential-wall magnetic-field penetration
//! loss at RuView geometry has been published. Reinforced concrete carries
//! a warning flag so consumers know to escalate.

use crate::scene::Vec3;

/// Material categories the simulator knows about. Extend by adding to this
/// enum + the per-material entry in [`material_loss_db_per_m`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Material {
    /// Vacuum / air. Identity attenuation.
    Air,
    /// Gypsum drywall, dry. Conjectural 0 dB/m.
    Drywall,
    /// Dry brick. Conjectural 0 dB/m.
    Brick,
    /// Dry concrete, no rebar. Conjectural 0.5 dB/m (Ulrich 2002 proxy).
    ConcreteDry,
    /// Reinforced concrete. 20 dB/m + raises the heavy-attenuation flag.
    ReinforcedConcrete,
    /// Sheet steel (low-carbon). Frequency-dependent skin-depth attenuation
    /// per Jackson §8.1; the simulator passes a representative DC value.
    SheetSteel,
}

/// One slab of material along a line-of-sight segment.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LosSegment {
    /// Material in this slab.
    pub material: Material,
    /// Path length through the slab (m). Must be `>= 0` and finite; `0`
    /// is the documented no-op input.
    pub path_m: f64,
}

/// Per-meter loss in decibels at DC–10 kHz. See plan §2.2 for primary
/// sources and conjecture markers.
pub fn material_loss_db_per_m(m: Material) -> f64 {
    match m {
        Material::Air => 0.0,
        Material::Drywall => 0.0,           // conjecture: gypsum non-ferromagnetic
        Material::Brick => 0.0,             // conjecture: same logic as drywall
        Material::ConcreteDry => 0.5,       // conjecture: Ulrich 2002 proxy
        Material::ReinforcedConcrete => 20.0, // proxy + warning flag (plan §2.2)
        Material::SheetSteel => 100.0,      // frequency-dependent in reality;
                                            // representative DC bulk loss
    }
}

/// True iff this material warrants the `HEAVY_ATTENUATION` frame flag
/// (i.e. the simulator's confidence in the per-meter loss is poor and the
/// downstream consumer should know to interpret the reading with caution).
pub fn material_is_heavy(m: Material) -> bool {
    matches!(m, Material::ReinforcedConcrete | Material::SheetSteel)
}

/// Apply per-segment attenuation to an incoming 3-vector field. Returns
/// `(B_out, heavy_flag)` where `heavy_flag` is `true` if any segment was
/// flagged as heavy / low-confidence.
///
/// Total loss is the sum of `path_m × loss_db_per_m` across segments,
/// converted to a linear scale factor. NaN-safe — segments with non-finite
/// `path_m` are skipped (no contribution, no panic).
pub fn attenuate(b_in: Vec3, segments: &[LosSegment]) -> (Vec3, bool) {
    let mut total_db = 0.0_f64;
    let mut heavy = false;
    for seg in segments {
        if !seg.path_m.is_finite() || seg.path_m <= 0.0 {
            continue;
        }
        total_db += seg.path_m * material_loss_db_per_m(seg.material);
        heavy |= material_is_heavy(seg.material);
    }
    let scale = 10.0_f64.powf(-total_db / 20.0);
    (
        [b_in[0] * scale, b_in[1] * scale, b_in[2] * scale],
        heavy,
    )
}

/// Aggregate "propagator" type — currently a stateless wrapper over
/// [`attenuate`] but a struct to keep room for future per-frequency or
/// per-thickness parameters without breaking the call-site shape.
#[derive(Debug, Clone, Copy, Default)]
pub struct Propagator;

impl Propagator {
    /// Identity-attenuation propagator (air/free-space).
    pub fn new() -> Self {
        Self
    }

    /// Run [`attenuate`] across a slice of LoS segments.
    pub fn attenuate(self, b_in: Vec3, segments: &[LosSegment]) -> (Vec3, bool) {
        attenuate(b_in, segments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn free_space_is_identity_transform() {
        // Air with any path length: B_out == B_in, no heavy flag.
        let b_in = [1.0e-9, 2.0e-9, 3.0e-9];
        let segs = [LosSegment {
            material: Material::Air,
            path_m: 5.0,
        }];
        let (b_out, heavy) = attenuate(b_in, &segs);
        assert_relative_eq!(b_out[0], b_in[0], max_relative = 1e-12);
        assert_relative_eq!(b_out[1], b_in[1], max_relative = 1e-12);
        assert_relative_eq!(b_out[2], b_in[2], max_relative = 1e-12);
        assert!(!heavy);
    }

    #[test]
    fn drywall_is_approximately_zero_db() {
        // Plan §2.2 marks drywall as conjectural 0 dB/m. The simulator
        // commits to identity for now; if a primary source is ever cited
        // this test is the regression boundary.
        let b_in = [1.0e-9, 0.0, 0.0];
        let segs = [LosSegment {
            material: Material::Drywall,
            path_m: 0.1,
        }];
        let (b_out, heavy) = attenuate(b_in, &segs);
        assert_relative_eq!(b_out[0], b_in[0], max_relative = 1e-12);
        assert!(!heavy, "drywall is not flagged as heavy");
    }

    #[test]
    fn dry_concrete_attenuates_at_half_db_per_meter() {
        // 0.5 dB/m × 2 m = 1 dB total. Linear scale = 10^(-1/20) ≈ 0.8913.
        let b_in = [1.0_f64, 0.0, 0.0];
        let segs = [LosSegment {
            material: Material::ConcreteDry,
            path_m: 2.0,
        }];
        let (b_out, heavy) = attenuate(b_in, &segs);
        let expected = 10.0_f64.powf(-1.0 / 20.0);
        assert_relative_eq!(b_out[0], expected, max_relative = 1e-12);
        assert!(!heavy, "dry concrete is not flagged heavy");
    }

    #[test]
    fn reinforced_concrete_attenuates_and_raises_heavy_flag() {
        // 20 dB/m × 0.2 m = 4 dB. Linear scale = 10^(-0.2) ≈ 0.6310.
        let b_in = [1.0_f64; 3];
        let segs = [LosSegment {
            material: Material::ReinforcedConcrete,
            path_m: 0.2,
        }];
        let (b_out, heavy) = attenuate(b_in, &segs);
        let expected = 10.0_f64.powf(-4.0 / 20.0);
        for k in 0..3 {
            assert_relative_eq!(b_out[k], expected, max_relative = 1e-12);
        }
        assert!(heavy, "reinforced concrete must raise heavy_flag");
    }

    #[test]
    fn nan_or_negative_path_is_skipped_without_nan_in_output() {
        // A degenerate or hostile input must not propagate NaN/Inf to the
        // pipeline (the digitiser would otherwise produce a poisoned frame).
        let b_in = [1.0_f64, 2.0, 3.0];
        let segs = [
            LosSegment {
                material: Material::ConcreteDry,
                path_m: f64::NAN,
            },
            LosSegment {
                material: Material::Drywall,
                path_m: -1.0, // negative paths are skipped, not negated
            },
            LosSegment {
                material: Material::Air,
                path_m: 5.0,
            },
        ];
        let (b_out, heavy) = attenuate(b_in, &segs);
        for k in 0..3 {
            assert!(
                b_out[k].is_finite(),
                "B[{k}] = {} is non-finite — pass-3 NaN guard failed",
                b_out[k]
            );
            // Air alone -> identity; the malformed segments contributed nothing.
            assert_relative_eq!(b_out[k], b_in[k], max_relative = 1e-12);
        }
        assert!(!heavy);
    }

    #[test]
    fn empty_los_returns_input_unchanged() {
        let b_in = [1.0_f64, 2.0, 3.0];
        let (b_out, heavy) = attenuate(b_in, &[]);
        assert_eq!(b_out, b_in);
        assert!(!heavy);
    }

    #[test]
    fn propagator_struct_dispatches_to_free_function() {
        let b_in = [1.0_f64, 2.0, 3.0];
        let segs = [LosSegment {
            material: Material::Air,
            path_m: 1.0,
        }];
        let p = Propagator::new();
        let (b_out, _) = p.attenuate(b_in, &segs);
        assert_eq!(b_out, b_in);
    }
}
