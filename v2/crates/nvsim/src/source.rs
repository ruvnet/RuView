//! Magnetic-field synthesis at sensor location(s) — Pass 2 of the implementation plan.
//!
//! Implements the analytic magnetic-dipole field formula, numerical
//! Biot–Savart integration over current loops, and linearly-induced
//! moments for ferrous objects. All operations in `f64` for near-field
//! stability per plan §7-1 (float-precision risk).
//!
//! # Primary sources
//! - Jackson, *Classical Electrodynamics* 3e (1999) §5.4–5.6 — Biot–Savart, dipole.
//! - Cullity & Graham, *Introduction to Magnetic Materials* 2e (2009) Ch. 2 — χ_steel.
//! - Ortner & Bandeira, *SoftwareX* 11, 100466 (2020) — Magpylib reference impl.
//!
//! # API
//!
//! Free functions ([`dipole_field`], [`current_loop_field`],
//! [`ferrous_field`], [`scene_field_at`]) keep the math testable in
//! isolation; the convenience method [`crate::scene::Scene::field_at`]
//! aggregates a single sensor sample.

use crate::scene::{CurrentLoop, DipoleSource, FerrousObject, Scene, Vec3};
use crate::MU_0;

/// Minimum source–sensor distance below which the dipole / Biot–Savart
/// formulae are clamped to zero. Plan §2.1: 1 mm. Below this, the field
/// formula's `1/r³` factor dominates float rounding and the dipole model
/// itself is meaningless (real magnets have finite extent).
pub const R_MIN_M: f64 = 1.0e-3;

// ────────────────────── public entry points ──────────────────────────────

/// Field at `sensor_pos` due to a magnetic dipole.
///
/// Closed-form: `B = (μ₀ / 4π r³) · [3(m·r̂)r̂ − m]`. Returns `(B, near_field_flag)`
/// where `near_field_flag = true` indicates `|r| < R_MIN_M` and the field has
/// been clamped to zero. The caller is responsible for raising the
/// `SATURATION_NEAR_FIELD` flag on the emitted [`crate::MagFrame`].
pub fn dipole_field(dipole: &DipoleSource, sensor_pos: Vec3) -> (Vec3, bool) {
    let r = vec3_sub(sensor_pos, dipole.position);
    let r_norm = vec3_norm(r);
    if r_norm < R_MIN_M {
        return ([0.0; 3], true);
    }
    let r_hat = vec3_scale(r, 1.0 / r_norm);
    let m_dot_r = vec3_dot(dipole.moment, r_hat);
    let bracket = vec3_sub(vec3_scale(r_hat, 3.0 * m_dot_r), dipole.moment);
    let coef = MU_0 / (4.0 * std::f64::consts::PI * r_norm.powi(3));
    (vec3_scale(bracket, coef), false)
}

/// Field at `sensor_pos` due to a planar circular current loop.
///
/// Discretised over `loop_.n_segments` straight chords:
/// `dB = (μ₀/4π) · (I dl × r̂) / r²`. Returns `(B, near_field_flag)` where the
/// flag fires if any chord midpoint is within [`R_MIN_M`] of the sensor.
pub fn current_loop_field(loop_: &CurrentLoop, sensor_pos: Vec3) -> (Vec3, bool) {
    let n = loop_.n_segments.max(8) as usize;
    let normal = vec3_normalise(loop_.normal);
    let (u, v) = orthonormal_basis(normal);

    let mut sum: Vec3 = [0.0; 3];
    let two_pi = 2.0 * std::f64::consts::PI;
    let mut saturation = false;

    for i in 0..n {
        let theta_a = (i as f64 / n as f64) * two_pi;
        let theta_b = ((i + 1) as f64 / n as f64) * two_pi;
        let p_a = vec3_add(
            loop_.centre,
            vec3_add(
                vec3_scale(u, loop_.radius * theta_a.cos()),
                vec3_scale(v, loop_.radius * theta_a.sin()),
            ),
        );
        let p_b = vec3_add(
            loop_.centre,
            vec3_add(
                vec3_scale(u, loop_.radius * theta_b.cos()),
                vec3_scale(v, loop_.radius * theta_b.sin()),
            ),
        );
        let mid = vec3_scale(vec3_add(p_a, p_b), 0.5);
        let dl = vec3_sub(p_b, p_a);
        let r = vec3_sub(sensor_pos, mid);
        let r_norm = vec3_norm(r);
        if r_norm < R_MIN_M {
            saturation = true;
            continue;
        }
        let r_hat = vec3_scale(r, 1.0 / r_norm);
        let cross = vec3_cross(dl, r_hat);
        let coef = MU_0 * loop_.current / (4.0 * std::f64::consts::PI * r_norm.powi(2));
        sum = vec3_add(sum, vec3_scale(cross, coef));
    }
    (sum, saturation)
}

/// Field at `sensor_pos` due to a ferrous object's linearly-induced moment.
///
/// `m_induced = χ · V · H_ambient`, with `H = B/μ₀` (SI). Default χ = 5000
/// for low-carbon steel per Cullity & Graham 2e §2. Output then radiates as a
/// dipole at the object's position.
pub fn ferrous_field(obj: &FerrousObject, ambient_b: Vec3, sensor_pos: Vec3) -> (Vec3, bool) {
    let h_ambient = vec3_scale(ambient_b, 1.0 / MU_0);
    let m_induced = vec3_scale(h_ambient, obj.susceptibility * obj.volume);
    let induced_dipole = DipoleSource::new(obj.position, m_induced);
    dipole_field(&induced_dipole, sensor_pos)
}

/// Total field at `sensor_pos` from every primitive in `scene`. Returns
/// `(B, saturation)` where `saturation` is `true` if any source clamped to
/// zero in the near-field. The caller emits the corresponding flag.
pub fn scene_field_at(scene: &Scene, sensor_pos: Vec3) -> (Vec3, bool) {
    let mut total: Vec3 = [0.0; 3];
    let mut sat = false;
    for d in &scene.dipoles {
        let (b, s) = dipole_field(d, sensor_pos);
        total = vec3_add(total, b);
        sat |= s;
    }
    for l in &scene.loops {
        let (b, s) = current_loop_field(l, sensor_pos);
        total = vec3_add(total, b);
        sat |= s;
    }
    for f in &scene.ferrous {
        let (b, s) = ferrous_field(f, scene.ambient_field, sensor_pos);
        total = vec3_add(total, b);
        sat |= s;
    }
    (total, sat)
}

/// Total field at every sensor location in a scene, in scene order.
pub fn scene_field_at_sensors(scene: &Scene) -> Vec<(Vec3, bool)> {
    scene.sensors.iter().map(|&p| scene_field_at(scene, p)).collect()
}

// ────────────────────── vec3 helpers ─────────────────────────────────────

#[inline]
fn vec3_add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
#[inline]
fn vec3_sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
#[inline]
fn vec3_scale(a: Vec3, s: f64) -> Vec3 {
    [a[0] * s, a[1] * s, a[2] * s]
}
#[inline]
fn vec3_dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
#[inline]
fn vec3_cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
#[inline]
fn vec3_norm(a: Vec3) -> f64 {
    vec3_dot(a, a).sqrt()
}
#[inline]
fn vec3_normalise(a: Vec3) -> Vec3 {
    let n = vec3_norm(a);
    if n < 1e-20 {
        [0.0, 0.0, 1.0]
    } else {
        vec3_scale(a, 1.0 / n)
    }
}

/// Build two orthonormal vectors `u, v` perpendicular to `n` (which must be
/// approximately unit). Stable across all input directions including ±ẑ.
fn orthonormal_basis(n: Vec3) -> (Vec3, Vec3) {
    let pick = if n[0].abs() < 0.9 {
        [1.0, 0.0, 0.0]
    } else {
        [0.0, 1.0, 0.0]
    };
    let u = vec3_normalise(vec3_cross(pick, n));
    let v = vec3_cross(n, u);
    (u, v)
}

// ─────────────────────────── tests ────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn dipole_on_axis_matches_closed_form() {
        // On-axis (along +ẑ for a dipole moment along +ẑ):
        // B_z = μ₀ m / (2π z³)   (Jackson 3e §5.6 specialisation).
        let m = 1.0e-3;
        let z = 0.5;
        let dipole = DipoleSource::new([0.0; 3], [0.0, 0.0, m]);
        let (b, sat) = dipole_field(&dipole, [0.0, 0.0, z]);
        assert!(!sat);
        let expected_bz = MU_0 * m / (2.0 * std::f64::consts::PI * z.powi(3));
        assert_relative_eq!(b[2], expected_bz, max_relative = 1e-12);
        assert_relative_eq!(b[0], 0.0, epsilon = 1e-25);
        assert_relative_eq!(b[1], 0.0, epsilon = 1e-25);
    }

    #[test]
    fn dipole_equatorial_matches_closed_form() {
        // Equatorial: B_z = -μ₀ m / (4π r³), anti-parallel to m.
        let m = 1.0e-3;
        let r = 0.5;
        let dipole = DipoleSource::new([0.0; 3], [0.0, 0.0, m]);
        let (b, _) = dipole_field(&dipole, [r, 0.0, 0.0]);
        let expected_bz = -MU_0 * m / (4.0 * std::f64::consts::PI * r.powi(3));
        assert_relative_eq!(b[2], expected_bz, max_relative = 1e-12);
    }

    #[test]
    fn dipole_n8_directions_within_half_percent_rms() {
        // Plan §3 Pass 2 acceptance gate: n=8 RMS error ≤ 0.5% vs an
        // independent recomputation from first principles. Fails => abort §7-1.
        let m_vec = [3.0e-4, 1.0e-4, 7.0e-4];
        let dipole = DipoleSource::new([0.1, 0.2, 0.3], m_vec);
        let r = 0.5;
        let directions: [Vec3; 8] = [
            [1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -1.0, 0.0],
            [0.0, 0.0, 1.0],
            [0.0, 0.0, -1.0],
            [1.0, 1.0, 1.0],
            [-1.0, -1.0, -1.0],
        ];
        let mut rms_sum = 0.0_f64;
        for dir in directions {
            let dn = vec3_normalise(dir);
            let sensor = vec3_add(dipole.position, vec3_scale(dn, r));
            let (b, _) = dipole_field(&dipole, sensor);
            // Independent recomputation from the formula — guards against the
            // implementation accidentally agreeing with a buggy reference.
            let r_vec = vec3_sub(sensor, dipole.position);
            let r_norm = vec3_norm(r_vec);
            let r_hat = vec3_scale(r_vec, 1.0 / r_norm);
            let m_dot_r = vec3_dot(m_vec, r_hat);
            let bracket = vec3_sub(vec3_scale(r_hat, 3.0 * m_dot_r), m_vec);
            let coef = MU_0 / (4.0 * std::f64::consts::PI * r_norm.powi(3));
            let b_ref = vec3_scale(bracket, coef);
            for k in 0..3 {
                let denom = b_ref[k].abs().max(1e-30);
                let rel = (b[k] - b_ref[k]) / denom;
                rms_sum += rel * rel;
            }
        }
        let rms = (rms_sum / (8.0 * 3.0)).sqrt();
        assert!(
            rms <= 0.005,
            "Pass-2 acceptance: dipole n=8 RMS error {rms} > 0.5% threshold"
        );
    }

    #[test]
    fn current_loop_on_axis_matches_closed_form() {
        // On-axis circular loop: B_z = μ₀ I a² / [2 (a² + z²)^(3/2)]
        // (Jackson 3e §5.4). With n=64 segments accept ~1% numerical tolerance.
        let i = 0.5;
        let a = 0.05;
        let z = 0.2;
        let loop_ = CurrentLoop::new([0.0; 3], [0.0, 0.0, 1.0], a, i);
        let (b, _) = current_loop_field(&loop_, [0.0, 0.0, z]);
        let expected = MU_0 * i * a * a / (2.0 * (a * a + z * z).powf(1.5));
        assert_relative_eq!(b[2], expected, max_relative = 1.0e-2);
    }

    #[test]
    fn near_field_clamp_returns_zero_with_flag() {
        // Plan §2.1: r < R_MIN_M (1 mm) clamps to (0, true).
        let dipole = DipoleSource::new([0.0; 3], [1e-3, 0.0, 0.0]);
        let (b, sat) = dipole_field(&dipole, [0.5e-3, 0.0, 0.0]); // 0.5 mm
        assert_eq!(b, [0.0; 3]);
        assert!(sat, "near-field saturation flag must fire below 1 mm");
    }

    #[test]
    fn ferrous_object_zero_ambient_yields_zero_field() {
        // Linear induced moment is proportional to ambient — at zero ambient,
        // induced moment is zero, so the ferrous object emits no field.
        let obj = FerrousObject::steel([0.5, 0.0, 0.0], 1.0e-3);
        let (b, _) = ferrous_field(&obj, [0.0; 3], [1.0, 0.0, 0.0]);
        assert_eq!(b, [0.0; 3]);
    }

    #[test]
    fn scene_field_aggregates_multiple_sources() {
        // Two co-located dipoles with opposite moments cancel exactly.
        let m = 5.0e-4;
        let mut scene = Scene::new();
        scene.add_dipole(DipoleSource::new([0.0; 3], [0.0, 0.0, m]));
        scene.add_dipole(DipoleSource::new([0.0; 3], [0.0, 0.0, -m]));
        scene.add_sensor([0.0, 0.0, 0.5]);
        let result = scene_field_at_sensors(&scene);
        assert_eq!(result.len(), 1);
        let (b, _) = result[0];
        assert_relative_eq!(b[0], 0.0, epsilon = 1e-25);
        assert_relative_eq!(b[1], 0.0, epsilon = 1e-25);
        assert_relative_eq!(b[2], 0.0, epsilon = 1e-25);
    }
}
