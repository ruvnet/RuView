//! Scene types — ground-truth magnetic sources and ferrous-object distortion.
//!
//! Per `docs/research/quantum-sensing/15-nvsim-implementation-plan.md` §1.3
//! and §2.1. All coordinates SI (metres, A·m², A); all moments are 3-vectors
//! in the simulator's global frame. Sign convention: right-hand rule.

use serde::{Deserialize, Serialize};

/// 3-vector position / moment / direction. SI units.
pub type Vec3 = [f64; 3];

/// A point magnetic dipole in SI units. The dominant primitive — used for
/// far-field approximations of permanent magnets, current loops at distance,
/// and the linearised induced moment of ferrous objects.
///
/// Field at `r` (relative to dipole):
/// `B = (μ₀ / 4π r³) · [3(m·r̂)r̂ − m]`  (Jackson 3e §5.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DipoleSource {
    /// Position in metres.
    pub position: Vec3,
    /// Magnetic moment in A·m².
    pub moment: Vec3,
}

impl DipoleSource {
    /// Construct a dipole source.
    pub const fn new(position: Vec3, moment: Vec3) -> Self {
        Self { position, moment }
    }
}

/// A planar circular current loop, discretised at sample time into `n_segments`
/// straight segments for numerical Biot–Savart integration. The loop's normal
/// vector follows the right-hand rule on `current` (positive current produces
/// a moment along `+normal`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CurrentLoop {
    /// Centre of the loop (m).
    pub centre: Vec3,
    /// Unit normal vector (right-hand rule on current).
    pub normal: Vec3,
    /// Loop radius (m).
    pub radius: f64,
    /// Steady-state current (A).
    pub current: f64,
    /// Number of straight-segment chords for Biot–Savart integration. Default 64.
    #[serde(default = "default_segments")]
    pub n_segments: u32,
}

const fn default_segments() -> u32 {
    64
}

impl CurrentLoop {
    /// Construct a loop with the default 64-segment discretisation.
    pub fn new(centre: Vec3, normal: Vec3, radius: f64, current: f64) -> Self {
        Self {
            centre,
            normal,
            radius,
            current,
            n_segments: default_segments(),
        }
    }
}

/// A ferrous (high-χ) object that picks up a linearly-induced moment from the
/// ambient field and re-radiates as a dipole. Linear approximation —
/// `m_induced = χ · V · H_ambient` — valid in low-field, unsaturated regime
/// (Cullity & Graham 2e §2). For RuView geometry this is the dominant
/// "metallic-object detection" signal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FerrousObject {
    /// Centre of mass / centroid (m).
    pub position: Vec3,
    /// Volume (m³).
    pub volume: f64,
    /// Magnetic susceptibility (dimensionless). 5000 ≈ low-carbon steel.
    pub susceptibility: f64,
}

impl FerrousObject {
    /// Construct a steel-default ferrous object (χ ≈ 5000).
    pub fn steel(position: Vec3, volume: f64) -> Self {
        Self {
            position,
            volume,
            susceptibility: 5000.0,
        }
    }
}

/// A simple eddy-current loop — a planar conductor that generates an opposing
/// dipole moment per Faraday's law when the ambient flux changes. Faraday +
/// Ohm: `I(t) = -(σ A / L) · dΦ/dt`. Geometry simplified to "thin disc with
/// scalar inductance" — see plan §2.1: no primary source for arbitrary
/// geometry, so this primitive is intentionally approximate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EddyCurrent {
    /// Centre of the disc (m).
    pub position: Vec3,
    /// Disc area (m²).
    pub area: f64,
    /// Conductivity (S/m). Copper ≈ 5.96e7.
    pub conductivity: f64,
    /// Disc inductance (H). Caller-supplied scalar.
    pub inductance: f64,
    /// Disc-normal unit vector.
    pub normal: Vec3,
}

/// Aggregate ground-truth scene — a list of every magnetic primitive plus a
/// list of sensor positions where the simulator should sample the field.
///
/// `Scene` is the canonical input to [`crate::Pipeline`]. Two scenes that
/// serialise to the same JSON produce the same `(simulator, seed)` proof
/// bundle.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    /// Dipole sources (point moments).
    pub dipoles: Vec<DipoleSource>,
    /// Current-carrying loops.
    pub loops: Vec<CurrentLoop>,
    /// Ferrous objects (linearly-induced dipoles).
    pub ferrous: Vec<FerrousObject>,
    /// Eddy-current discs (Faraday + Ohm).
    pub eddy: Vec<EddyCurrent>,
    /// Sensor positions (one MagFrame per sensor per timestep).
    pub sensors: Vec<Vec3>,
    /// Ambient field at infinity (T) — drives ferrous induced-moment
    /// computation. Zero by default.
    #[serde(default)]
    pub ambient_field: Vec3,
}

impl Scene {
    /// Construct an empty scene with no sources and no sensors.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a dipole source.
    pub fn add_dipole(&mut self, dipole: DipoleSource) -> &mut Self {
        self.dipoles.push(dipole);
        self
    }

    /// Append a current loop.
    pub fn add_loop(&mut self, l: CurrentLoop) -> &mut Self {
        self.loops.push(l);
        self
    }

    /// Append a ferrous object.
    pub fn add_ferrous(&mut self, ferrous: FerrousObject) -> &mut Self {
        self.ferrous.push(ferrous);
        self
    }

    /// Append a sensor location.
    pub fn add_sensor(&mut self, position: Vec3) -> &mut Self {
        self.sensors.push(position);
        self
    }

    /// Total source count across all primitives.
    pub fn n_sources(&self) -> usize {
        self.dipoles.len() + self.loops.len() + self.ferrous.len() + self.eddy.len()
    }

    /// Canonical JSON representation. Used by the proof bundle for content
    /// addressing — two scenes with the same JSON produce the same witness.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        // serde_json::to_string is deterministic for serde-derived types when
        // the underlying field order is stable, which it is here.
        serde_json::to_string(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dipole_construction_round_trip_via_json() {
        let d = DipoleSource::new([1.0, 2.0, 3.0], [0.1, 0.2, 0.3]);
        let s = serde_json::to_string(&d).unwrap();
        let d2: DipoleSource = serde_json::from_str(&s).unwrap();
        assert_eq!(d, d2);
    }

    #[test]
    fn current_loop_default_n_segments_is_64() {
        let l = CurrentLoop::new([0.0; 3], [0.0, 0.0, 1.0], 0.05, 1.5);
        assert_eq!(l.n_segments, 64);
    }

    #[test]
    fn empty_scene_is_default_and_serialises() {
        let s = Scene::new();
        assert_eq!(s.n_sources(), 0);
        assert_eq!(s.sensors.len(), 0);
        let _ = s.to_canonical_json().unwrap();
    }

    #[test]
    fn scene_round_trip_via_json_preserves_all_primitives() {
        let mut s = Scene::new();
        s.add_dipole(DipoleSource::new([0.0; 3], [1e-6, 0.0, 0.0]));
        s.add_loop(CurrentLoop::new([0.0; 3], [0.0, 0.0, 1.0], 0.1, 0.5));
        s.add_ferrous(FerrousObject::steel([0.5; 3], 1e-3));
        s.add_sensor([1.0, 0.0, 0.0]);
        let json = s.to_canonical_json().unwrap();
        let s2: Scene = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }
}
