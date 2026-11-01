//! The RF-aware Gaussian primitive (ADR-275 §2).

use serde::{Deserialize, Serialize};

use crate::{Result, UnifiedError};

/// Frequency bands with distinct material interaction (index into the
/// reflectivity table).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Band {
    /// 2.4 GHz (WiFi b/g/n, BLE).
    Ghz2_4,
    /// 5–6 GHz (WiFi a/ac/ax, NR n77/n78-ish).
    Ghz5,
    /// 6–8 GHz (WiFi 6E/7, UWB).
    Ghz6,
    /// 60 GHz (mmWave radar / 802.11ad).
    Ghz60,
}

impl Band {
    /// Number of modeled bands.
    pub const COUNT: usize = 4;

    /// Table index.
    #[must_use]
    pub fn index(self) -> usize {
        match self {
            Self::Ghz2_4 => 0,
            Self::Ghz5 => 1,
            Self::Ghz6 => 2,
            Self::Ghz60 => 3,
        }
    }

    /// Band for a carrier frequency in Hz (nearest-band bucketing).
    #[must_use]
    pub fn from_freq_hz(f: f64) -> Self {
        if f < 4.0e9 {
            Self::Ghz2_4
        } else if f < 6.0e9 {
            Self::Ghz5
        } else if f < 2.0e10 {
            Self::Ghz6
        } else {
            Self::Ghz60
        }
    }
}

/// Number of incident-angle bins in the reflectivity table (0°–90° in 4
/// bins of 22.5°).
pub const ANGLE_BINS: usize = 4;

/// Semantic embedding width carried by each Gaussian.
pub const SEMANTIC_DIM: usize = 16;

/// Coarse motion state of the matter a Gaussian represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotionState {
    /// Structural / furniture: no observed Doppler.
    Static,
    /// Slow biological motion (breathing, posture shifts).
    Slow,
    /// Walking-speed or faster motion.
    Fast,
}

/// Where a Gaussian's evidence came from (ADR-273 acceptance item 8: every
/// output carries provenance and model version).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    /// Device that observed the evidence.
    pub device_id: String,
    /// Version of the model that produced the update.
    pub model_version: u32,
    /// Whether the evidence is synthetic (ADR-276 honest labeling).
    pub synthetic: bool,
}

/// What kind of entity a Gaussian links to in the scene graph / RuVector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityLink {
    /// Kind of the linked entity.
    pub kind: super::graph::EntityKind,
    /// Stable identifier of the entity.
    pub id: String,
}

/// One anisotropic RF-aware Gaussian (ADR-275 §2.1) — the six attribute
/// groups from the ADR: geometry, semantics, RF response, motion,
/// trust/lifecycle, and entity links.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RfGaussian {
    /// Centre, metres, room frame.
    pub position: [f64; 3],
    /// Standard deviations along the principal axes, metres (> 0).
    pub scale: [f64; 3],
    /// Orientation quaternion `[w, x, y, z]` (normalized on construction).
    pub orientation: [f64; 4],
    /// Peak extinction coefficient (nepers/metre at the centre) — the
    /// Beer–Lambert density used by the gain model. `>= 0`.
    pub occupancy: f64,
    /// Visual/semantic embedding.
    pub semantic: [f32; SEMANTIC_DIM],
    /// RF reflectivity by `[band][incident-angle bin]`, `0..=1` amplitude.
    pub reflectivity: [[f64; ANGLE_BINS]; Band::COUNT],
    /// Radial velocity estimate, m/s (signed).
    pub doppler_mps: f64,
    /// Doppler estimate variance, (m/s)² (ADR-281 lifecycle fields).
    pub doppler_variance: f64,
    /// Coarse motion class.
    pub motion: MotionState,
    /// Confidence in `[0, 1]`.
    pub confidence: f64,
    /// First-observed timestamp, ns since epoch (static structure is
    /// distinguishable from transients by lifetime, not just decay τ).
    pub first_seen_ns: u64,
    /// Last-updated timestamp, ns since epoch.
    pub timestamp_ns: u64,
    /// Time up to which `confidence` has already been decayed, ns since
    /// epoch. [`super::map::GaussianMap::decay`] only applies the interval
    /// after `max(decayed_to_ns, timestamp_ns)`, so repeated calls compose
    /// instead of compounding. Defaults to 0 for older serialized maps.
    #[serde(default)]
    pub decayed_to_ns: u64,
    /// Confidence e-folding time in seconds (decay clock).
    pub decay_tau_s: f64,
    /// Evidence provenance.
    pub provenance: Provenance,
    /// Receipt ids of the source frames that shaped this Gaussian
    /// (measurement→inference lineage; capped on fusion).
    pub source_receipts: Vec<u128>,
    /// Links into the scene graph / RuVector entities.
    pub links: Vec<EntityLink>,
}

/// Maximum receipts retained per Gaussian after fusion (bounded lineage).
pub const MAX_SOURCE_RECEIPTS: usize = 16;

impl RfGaussian {
    /// Validated constructor: normalizes the quaternion and range-checks
    /// every numeric field (boundary rule — the map assumes validity).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        position: [f64; 3],
        scale: [f64; 3],
        orientation: [f64; 4],
        occupancy: f64,
        confidence: f64,
        timestamp_ns: u64,
        decay_tau_s: f64,
        provenance: Provenance,
    ) -> Result<Self> {
        for v in position.iter().chain(scale.iter()).chain(orientation.iter()) {
            if !v.is_finite() {
                return Err(UnifiedError::InvalidInput("non-finite Gaussian field".into()));
            }
        }
        // Physical plausibility bounds, not just positivity: a subnormal
        // scale (e.g. 5e-324) passes `> 0` but overflows `1/σ²` to ∞ and
        // turns the density at the Gaussian's own centre into NaN (found
        // by `tests/security_boundaries.rs` property testing); a
        // kilometre-scale σ is equally meaningless indoors.
        const MIN_SCALE_M: f64 = 1e-6;
        const MAX_SCALE_M: f64 = 1e4;
        if scale.iter().any(|s| !(*s >= MIN_SCALE_M && *s <= MAX_SCALE_M)) {
            return Err(UnifiedError::InvalidInput(format!(
                "scale must be within [{MIN_SCALE_M}, {MAX_SCALE_M}] m, got {scale:?}"
            )));
        }
        let norm =
            orientation.iter().map(|q| q * q).sum::<f64>().sqrt();
        if norm < 1e-6 {
            return Err(UnifiedError::InvalidInput("degenerate quaternion".into()));
        }
        let orientation = [
            orientation[0] / norm,
            orientation[1] / norm,
            orientation[2] / norm,
            orientation[3] / norm,
        ];
        // Extinction beyond 1e6 nepers/m is physically meaningless and
        // only serves to smuggle ∞ into downstream gain products.
        if !(occupancy.is_finite() && (0.0..=1e6).contains(&occupancy)) {
            return Err(UnifiedError::InvalidInput(format!(
                "occupancy must be in [0, 1e6] nepers/m, got {occupancy}"
            )));
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Err(UnifiedError::InvalidInput(format!(
                "confidence must be in [0,1], got {confidence}"
            )));
        }
        if !(decay_tau_s.is_finite() && decay_tau_s > 0.0) {
            return Err(UnifiedError::InvalidInput(format!(
                "decay_tau_s must be > 0, got {decay_tau_s}"
            )));
        }
        Ok(Self {
            position,
            scale,
            orientation,
            occupancy,
            semantic: [0.0; SEMANTIC_DIM],
            reflectivity: [[0.0; ANGLE_BINS]; Band::COUNT],
            doppler_mps: 0.0,
            doppler_variance: 0.0,
            motion: MotionState::Static,
            confidence,
            first_seen_ns: timestamp_ns,
            timestamp_ns,
            decayed_to_ns: timestamp_ns,
            decay_tau_s,
            provenance,
            source_receipts: Vec::new(),
            links: Vec::new(),
        })
    }

    /// Rotation matrix (row-major 3×3) from the unit quaternion.
    #[must_use]
    pub fn rotation(&self) -> [[f64; 3]; 3] {
        let [w, x, y, z] = self.orientation;
        [
            [1.0 - 2.0 * (y * y + z * z), 2.0 * (x * y - w * z), 2.0 * (x * z + w * y)],
            [2.0 * (x * y + w * z), 1.0 - 2.0 * (x * x + z * z), 2.0 * (y * z - w * x)],
            [2.0 * (x * z - w * y), 2.0 * (y * z + w * x), 1.0 - 2.0 * (x * x + y * y)],
        ]
    }

    /// Inverse covariance `Σ⁻¹ = R·diag(1/σ²)·Rᵀ` (row-major 3×3).
    #[must_use]
    pub fn inv_covariance(&self) -> [[f64; 3]; 3] {
        let r = self.rotation();
        let inv_s2 = [
            1.0 / (self.scale[0] * self.scale[0]),
            1.0 / (self.scale[1] * self.scale[1]),
            1.0 / (self.scale[2] * self.scale[2]),
        ];
        let mut out = [[0.0; 3]; 3];
        for i in 0..3 {
            for j in 0..3 {
                let mut acc = 0.0;
                for (k, is2) in inv_s2.iter().enumerate() {
                    acc += r[i][k] * is2 * r[j][k];
                }
                out[i][j] = acc;
            }
        }
        out
    }

    /// Unnormalized density `exp(-½·dᵀΣ⁻¹d)` at a point (1 at the centre).
    #[must_use]
    pub fn density_at(&self, p: [f64; 3]) -> f64 {
        let d = [p[0] - self.position[0], p[1] - self.position[1], p[2] - self.position[2]];
        let si = self.inv_covariance();
        let mut q = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                q += d[i] * si[i][j] * d[j];
            }
        }
        (-0.5 * q).exp()
    }

    /// Squared Mahalanobis distance of a point from this Gaussian.
    #[must_use]
    pub fn mahalanobis_sq(&self, p: [f64; 3]) -> f64 {
        let d = [p[0] - self.position[0], p[1] - self.position[1], p[2] - self.position[2]];
        let si = self.inv_covariance();
        let mut q = 0.0;
        for i in 0..3 {
            for j in 0..3 {
                q += d[i] * si[i][j] * d[j];
            }
        }
        q
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prov() -> Provenance {
        Provenance { device_id: "test".into(), model_version: 1, synthetic: true }
    }

    #[test]
    fn constructor_validates_and_normalizes() {
        let g = RfGaussian::new(
            [1.0, 2.0, 1.0],
            [0.5, 0.5, 1.0],
            [2.0, 0.0, 0.0, 0.0], // non-unit quaternion → normalized
            0.5,
            0.9,
            0,
            300.0,
            prov(),
        )
        .expect("valid");
        assert!((g.orientation[0] - 1.0).abs() < 1e-12);

        assert!(RfGaussian::new([0.0; 3], [0.0, 1.0, 1.0], [1.0, 0.0, 0.0, 0.0], 0.1, 0.5, 0, 60.0, prov())
            .is_err());
        assert!(RfGaussian::new([0.0; 3], [1.0; 3], [1.0, 0.0, 0.0, 0.0], -0.1, 0.5, 0, 60.0, prov())
            .is_err());
        assert!(RfGaussian::new([0.0; 3], [1.0; 3], [1.0, 0.0, 0.0, 0.0], 0.1, 1.5, 0, 60.0, prov())
            .is_err());
    }

    #[test]
    fn density_peaks_at_centre_and_respects_anisotropy() {
        let g = RfGaussian::new(
            [0.0; 3],
            [1.0, 0.1, 1.0], // thin along y
            [1.0, 0.0, 0.0, 0.0],
            0.5,
            0.9,
            0,
            300.0,
            prov(),
        )
        .expect("valid");
        assert!((g.density_at([0.0; 3]) - 1.0).abs() < 1e-12);
        // Same offset along x (wide) vs y (thin): y decays far faster.
        // Analytic ratio at 0.3 m: exp(-0.045)/exp(-4.5) ≈ 86.
        assert!(g.density_at([0.3, 0.0, 0.0]) > g.density_at([0.0, 0.3, 0.0]) * 80.0);
    }

    #[test]
    fn rotated_gaussian_rotates_its_metric() {
        // 90° rotation about z maps the thin y-axis onto x.
        let s = std::f64::consts::FRAC_1_SQRT_2;
        let g = RfGaussian::new(
            [0.0; 3],
            [1.0, 0.1, 1.0],
            [s, 0.0, 0.0, s], // quaternion for Rz(90°)
            0.5,
            0.9,
            0,
            300.0,
            prov(),
        )
        .expect("valid");
        assert!(g.density_at([0.0, 0.3, 0.0]) > g.density_at([0.3, 0.0, 0.0]) * 80.0);
    }

    #[test]
    fn band_bucketing() {
        assert_eq!(Band::from_freq_hz(2.437e9), Band::Ghz2_4);
        assert_eq!(Band::from_freq_hz(5.18e9), Band::Ghz5);
        assert_eq!(Band::from_freq_hz(6.5e9), Band::Ghz6);
        assert_eq!(Band::from_freq_hz(60e9), Band::Ghz60);
    }
}
