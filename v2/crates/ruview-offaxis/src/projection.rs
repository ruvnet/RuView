//! Clean-room generalized (off-axis) perspective projection.
//!
//! Implements the screen-corner formulation published by Robert Kooima,
//! "Generalized Perspective Projection" (2008): given a physical screen
//! described by three of its corners and a tracked eye position in the same
//! tracker space, produce the asymmetric frustum and view transform that make
//! the screen behave as a window into the virtual scene.
//!
//! No code from any existing implementation (including the unlicensed
//! `icurtis1/off-axis-sneaker` prior art referenced by ADR-324) was copied or
//! consulted while writing this module; the derivation follows the published
//! math only.
//!
//! ## Conventions
//!
//! - Right-handed tracker space, metres.
//! - `pa` = screen lower-left, `pb` = lower-right, `pc` = upper-left corner.
//! - The screen normal `vn = vr × vu` points toward the viewer's side; the
//!   eye must be on that side (`EyeBehindScreen` otherwise).
//! - Output matrices are column-major `f64` in the three.js / OpenGL layout
//!   (see [`crate::math::Mat4`]). NDC follows OpenGL: visible x/y/z in
//!   `[-1, 1]`, camera looking down `-z` in eye space.

use crate::math::{mul, Mat4, Vec3, IDENTITY};
use core::fmt;

/// Minimum eye-to-screen-plane distance (metres). Below this the frustum
/// degenerates (division by ~0); callers get a typed error instead of NaNs.
pub const MIN_EYE_DISTANCE: f64 = 1e-6;

/// Errors from screen construction or projection evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffAxisError {
    /// The three corners do not span a plane (coincident or collinear).
    DegenerateScreen,
    /// The eye is on or behind the screen plane (`distance <= MIN_EYE_DISTANCE`).
    EyeBehindScreen,
    /// `near`/`far` are not `0 < near < far`, or not finite.
    InvalidClipPlanes,
    /// A non-finite input coordinate was supplied.
    NonFiniteInput,
}

impl fmt::Display for OffAxisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OffAxisError::DegenerateScreen => {
                write!(f, "screen corners are coincident or collinear")
            }
            OffAxisError::EyeBehindScreen => {
                write!(f, "eye is on or behind the screen plane")
            }
            OffAxisError::InvalidClipPlanes => {
                write!(f, "clip planes must satisfy 0 < near < far and be finite")
            }
            OffAxisError::NonFiniteInput => write!(f, "input coordinate is not finite"),
        }
    }
}

impl std::error::Error for OffAxisError {}

/// A physical screen described by three corners, with its orthonormal basis
/// precomputed at construction (the basis is eye-independent, so caching it
/// keeps the per-frame [`off_axis`] call to the eye-dependent work only).
#[derive(Clone, Copy, Debug)]
pub struct Screen {
    pa: Vec3,
    pb: Vec3,
    pc: Vec3,
    vr: Vec3,
    vu: Vec3,
    vn: Vec3,
}

impl Screen {
    /// Build a screen from its lower-left (`pa`), lower-right (`pb`) and
    /// upper-left (`pc`) corners, in metres, in any orientation.
    pub fn new(pa: Vec3, pb: Vec3, pc: Vec3) -> Result<Self, OffAxisError> {
        for v in [pa, pb, pc] {
            if !(v.x.is_finite() && v.y.is_finite() && v.z.is_finite()) {
                return Err(OffAxisError::NonFiniteInput);
            }
        }
        let vr = (pb - pa)
            .normalize()
            .ok_or(OffAxisError::DegenerateScreen)?;
        let vu_raw = (pc - pa)
            .normalize()
            .ok_or(OffAxisError::DegenerateScreen)?;
        let vn = vr
            .cross(vu_raw)
            .normalize()
            .ok_or(OffAxisError::DegenerateScreen)?;
        // Orthonormalize vu: guarantees that {vr, vu, vn} forms an exact
        // right-handed orthonormal basis (zero shear / pure rotation matrix).
        let vu = vn
            .cross(vr)
            .normalize()
            .ok_or(OffAxisError::DegenerateScreen)?;
        Ok(Self {
            pa,
            pb,
            pc,
            vr,
            vu,
            vn,
        })
    }

    /// Convenience: an axis-aligned screen of `width_m × height_m` metres,
    /// centered at the origin in the `z = 0` plane, normal facing `+z`
    /// (the viewer side). This matches the usual desktop-demo setup where
    /// the tracker origin is the screen center.
    pub fn centered(width_m: f64, height_m: f64) -> Result<Self, OffAxisError> {
        if !(width_m.is_finite() && height_m.is_finite()) {
            return Err(OffAxisError::NonFiniteInput);
        }
        if width_m <= 0.0 || height_m <= 0.0 {
            return Err(OffAxisError::DegenerateScreen);
        }
        let hw = width_m / 2.0;
        let hh = height_m / 2.0;
        Screen::new(
            Vec3::new(-hw, -hh, 0.0),
            Vec3::new(hw, -hh, 0.0),
            Vec3::new(-hw, hh, 0.0),
        )
    }

    /// Lower-left corner.
    pub fn pa(&self) -> Vec3 {
        self.pa
    }

    /// Lower-right corner.
    pub fn pb(&self) -> Vec3 {
        self.pb
    }

    /// Upper-left corner.
    pub fn pc(&self) -> Vec3 {
        self.pc
    }

    /// The implied upper-right corner `pb + (pc - pa)`.
    pub fn pd(&self) -> Vec3 {
        self.pb + self.pc - self.pa
    }

    /// Unit right vector along the screen's bottom edge.
    pub fn vr(&self) -> Vec3 {
        self.vr
    }

    /// Unit up vector along the screen's left edge.
    pub fn vu(&self) -> Vec3 {
        self.vu
    }

    /// Unit normal, pointing toward the viewer side.
    pub fn vn(&self) -> Vec3 {
        self.vn
    }

    /// Signed distance from `eye` to the screen plane along the normal
    /// (positive when the eye is on the viewer side).
    pub fn eye_distance(&self, eye: Vec3) -> f64 {
        // va = pa - pe; d = -(va · vn)
        -(self.pa - eye).dot(self.vn)
    }
}

/// The result of a generalized projection evaluation.
///
/// `projection` is the asymmetric frustum; `view` is the rigid transform
/// (screen-basis rotation + eye translation) taking tracker space into eye
/// space. For three.js, either:
///
/// - set `camera.projectionMatrix` from `projection` and position/orient the
///   camera from the eye and screen basis yourself, or
/// - use [`OffAxis::view_projection`] as a single combined matrix when you
///   manage matrices manually.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OffAxis {
    /// Asymmetric perspective frustum (column-major).
    pub projection: Mat4,
    /// Screen-aligned view matrix: rotation into the screen basis composed
    /// with translation by the negated eye position (column-major).
    pub view: Mat4,
}

impl OffAxis {
    /// `projection * view` — the full tracker-space-to-clip-space matrix.
    pub fn view_projection(&self) -> Mat4 {
        mul(&self.projection, &self.view)
    }
}

/// OpenGL-convention asymmetric frustum matrix (column-major).
#[inline]
fn frustum(l: f64, r: f64, b: f64, t: f64, n: f64, f: f64) -> Mat4 {
    let mut m = [0.0; 16];
    m[0] = 2.0 * n / (r - l);
    m[5] = 2.0 * n / (t - b);
    m[8] = (r + l) / (r - l);
    m[9] = (t + b) / (t - b);
    m[10] = -(f + n) / (f - n);
    m[11] = -1.0;
    m[14] = -2.0 * f * n / (f - n);
    m
}

/// Evaluate the generalized off-axis projection for `screen` as seen from
/// `eye`, with the given clip planes.
///
/// Errors when the eye is on/behind the screen plane, when clip planes are
/// invalid, or when inputs are non-finite. Never returns NaN-bearing
/// matrices: every failure mode is a typed error (renderer-facing code must
/// hold the last good matrix on error, not draw garbage).
pub fn off_axis(screen: &Screen, eye: Vec3, near: f64, far: f64) -> Result<OffAxis, OffAxisError> {
    if !(eye.x.is_finite() && eye.y.is_finite() && eye.z.is_finite()) {
        return Err(OffAxisError::NonFiniteInput);
    }
    if !(near.is_finite() && far.is_finite()) || near <= 0.0 || far <= near {
        return Err(OffAxisError::InvalidClipPlanes);
    }

    let (vr, vu, vn) = (screen.vr, screen.vu, screen.vn);

    // Vectors from the eye to each screen corner.
    let va = screen.pa - eye;
    let vb = screen.pb - eye;
    let vc = screen.pc - eye;

    // Distance from the eye to the screen plane.
    let d = -va.dot(vn);
    if d <= MIN_EYE_DISTANCE {
        return Err(OffAxisError::EyeBehindScreen);
    }

    // Frustum extents on the near plane.
    let nd = near / d;
    let l = vr.dot(va) * nd;
    let r = vr.dot(vb) * nd;
    let b = vu.dot(va) * nd;
    let t = vu.dot(vc) * nd;

    let projection = frustum(l, r, b, t, near, far);

    // View = screen-basis rotation (rows vr/vu/vn) * translation by -eye.
    // Composed directly: the translation column is -R^T-rotated eye.
    let mut view = IDENTITY;
    view[0] = vr.x;
    view[4] = vr.y;
    view[8] = vr.z;
    view[1] = vu.x;
    view[5] = vu.y;
    view[9] = vu.z;
    view[2] = vn.x;
    view[6] = vn.y;
    view[10] = vn.z;
    view[12] = -vr.dot(eye);
    view[13] = -vu.dot(eye);
    view[14] = -vn.dot(eye);

    Ok(OffAxis { projection, view })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::project_point;

    const EPS: f64 = 1e-9;

    fn assert_close(a: f64, b: f64, eps: f64, what: &str) {
        assert!((a - b).abs() < eps, "{what}: {a} vs {b}");
    }

    /// The defining invariant of off-axis projection: for ANY valid eye
    /// position, the physical screen corners land exactly on the NDC corners
    /// — pa→(-1,-1), pb→(1,-1), pc→(-1,1), pd→(1,1).
    #[test]
    fn screen_corners_map_to_ndc_corners_for_all_eyes() {
        let screen = Screen::centered(0.6, 0.34).unwrap();
        let mut checked = 0;
        for xi in -4..=4 {
            for yi in -3..=3 {
                for zi in 1..=6 {
                    let eye = Vec3::new(xi as f64 * 0.25, yi as f64 * 0.2, zi as f64 * 0.35);
                    let oa = off_axis(&screen, eye, 0.05, 100.0).unwrap();
                    let m = oa.view_projection();
                    for (corner, ex, ey) in [
                        (screen.pa(), -1.0, -1.0),
                        (screen.pb(), 1.0, -1.0),
                        (screen.pc(), -1.0, 1.0),
                        (screen.pd(), 1.0, 1.0),
                    ] {
                        let ndc = project_point(&m, corner).unwrap();
                        assert_close(ndc.x, ex, EPS, "ndc.x");
                        assert_close(ndc.y, ey, EPS, "ndc.y");
                    }
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, 9 * 7 * 6);
    }

    /// The invariant survives an arbitrarily rotated + translated screen:
    /// the math is coordinate-frame independent.
    #[test]
    fn corner_invariant_holds_for_tilted_screen() {
        // A screen rotated ~30° about y and ~15° about x, shifted off origin.
        let (cy, sy) = (30f64.to_radians().cos(), 30f64.to_radians().sin());
        let (cx, sx) = (15f64.to_radians().cos(), 15f64.to_radians().sin());
        let rot = |v: Vec3| {
            // Ry then Rx.
            let v = Vec3::new(cy * v.x + sy * v.z, v.y, -sy * v.x + cy * v.z);
            Vec3::new(v.x, cx * v.y - sx * v.z, sx * v.y + cx * v.z)
        };
        let shift = Vec3::new(0.4, -0.2, 1.1);
        let pa = rot(Vec3::new(-0.3, -0.17, 0.0)) + shift;
        let pb = rot(Vec3::new(0.3, -0.17, 0.0)) + shift;
        let pc = rot(Vec3::new(-0.3, 0.17, 0.0)) + shift;
        let screen = Screen::new(pa, pb, pc).unwrap();

        // An eye on the viewer side of the tilted plane.
        let eye = shift + rot(Vec3::new(0.1, 0.05, 0.8));
        let m = off_axis(&screen, eye, 0.01, 50.0)
            .unwrap()
            .view_projection();
        for (corner, ex, ey) in [
            (screen.pa(), -1.0, -1.0),
            (screen.pb(), 1.0, -1.0),
            (screen.pc(), -1.0, 1.0),
            (screen.pd(), 1.0, 1.0),
        ] {
            let ndc = project_point(&m, corner).unwrap();
            assert_close(ndc.x, ex, EPS, "tilted ndc.x");
            assert_close(ndc.y, ey, EPS, "tilted ndc.y");
        }
    }

    /// A centered eye must reduce to the ordinary symmetric perspective
    /// frustum: l == -r, b == -t, and the projection matrix has no skew
    /// terms (m[8] == m[9] == 0).
    #[test]
    fn centered_eye_is_symmetric_perspective() {
        let screen = Screen::centered(0.6, 0.34).unwrap();
        let d = 0.7;
        let near = 0.05;
        let oa = off_axis(&screen, Vec3::new(0.0, 0.0, d), near, 100.0).unwrap();
        assert_close(oa.projection[8], 0.0, EPS, "skew x");
        assert_close(oa.projection[9], 0.0, EPS, "skew y");
        // fovy check: m[5] == 1/tan(fovy/2), tan(fovy/2) = (h/2)/d.
        let expected_m5 = d / (0.34 / 2.0);
        assert_close(oa.projection[5], expected_m5, 1e-9, "m[5] focal");
        // aspect: m[0] == m[5]/aspect.
        let expected_m0 = d / (0.6 / 2.0);
        assert_close(oa.projection[0], expected_m0, 1e-9, "m[0] focal");
    }

    /// Depth convention: a point at `near` in front of the eye maps to NDC
    /// z = -1; a point at `far` maps to +1 (OpenGL convention).
    #[test]
    fn near_and_far_map_to_ndc_depth_bounds() {
        let screen = Screen::centered(0.6, 0.34).unwrap();
        let eye = Vec3::new(0.12, -0.05, 0.65);
        let (near, far) = (0.05, 40.0);
        let m = off_axis(&screen, eye, near, far).unwrap().view_projection();
        // Looking direction in tracker space is -vn.
        let toward = screen.vn().scale(-1.0);
        let p_near = eye + toward.scale(near);
        let p_far = eye + toward.scale(far);
        assert_close(project_point(&m, p_near).unwrap().z, -1.0, 1e-7, "near z");
        assert_close(project_point(&m, p_far).unwrap().z, 1.0, 1e-7, "far z");
    }

    /// A point exactly on the screen plane keeps the same NDC (x, y) for
    /// every eye position — this is the "window" property: the screen
    /// surface itself is the fixed point of the illusion.
    #[test]
    fn screen_plane_points_are_eye_invariant() {
        let screen = Screen::centered(0.5, 0.3).unwrap();
        // A point 30% right / 20% up from screen center, on the plane.
        let p = Vec3::new(0.5 * 0.3, 0.3 * 0.2, 0.0);
        let mut first: Option<(f64, f64)> = None;
        for eye in [
            Vec3::new(0.0, 0.0, 0.6),
            Vec3::new(0.3, 0.1, 0.4),
            Vec3::new(-0.5, -0.2, 1.2),
        ] {
            let m = off_axis(&screen, eye, 0.05, 100.0)
                .unwrap()
                .view_projection();
            let ndc = project_point(&m, p).unwrap();
            match first {
                None => first = Some((ndc.x, ndc.y)),
                Some((fx, fy)) => {
                    assert_close(ndc.x, fx, EPS, "plane-point ndc.x eye-invariance");
                    assert_close(ndc.y, fy, EPS, "plane-point ndc.y eye-invariance");
                }
            }
        }
    }

    #[test]
    fn error_paths_are_typed() {
        let screen = Screen::centered(0.6, 0.34).unwrap();
        let ok_eye = Vec3::new(0.0, 0.0, 0.5);
        // Degenerate screens.
        let p = Vec3::new(0.0, 0.0, 0.0);
        assert_eq!(
            Screen::new(p, p, Vec3::new(0.0, 1.0, 0.0)).unwrap_err(),
            OffAxisError::DegenerateScreen
        );
        assert_eq!(
            Screen::new(p, Vec3::new(1.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)).unwrap_err(),
            OffAxisError::DegenerateScreen
        );
        // Eye behind / on the plane.
        assert_eq!(
            off_axis(&screen, Vec3::new(0.0, 0.0, -0.5), 0.05, 10.0).unwrap_err(),
            OffAxisError::EyeBehindScreen
        );
        assert_eq!(
            off_axis(&screen, Vec3::new(0.2, 0.1, 0.0), 0.05, 10.0).unwrap_err(),
            OffAxisError::EyeBehindScreen
        );
        // Bad clip planes.
        assert_eq!(
            off_axis(&screen, ok_eye, 0.0, 10.0).unwrap_err(),
            OffAxisError::InvalidClipPlanes
        );
        assert_eq!(
            off_axis(&screen, ok_eye, 1.0, 1.0).unwrap_err(),
            OffAxisError::InvalidClipPlanes
        );
        assert_eq!(
            off_axis(&screen, ok_eye, -1.0, 10.0).unwrap_err(),
            OffAxisError::InvalidClipPlanes
        );
        // Non-finite input.
        assert_eq!(
            off_axis(&screen, Vec3::new(f64::NAN, 0.0, 0.5), 0.05, 10.0).unwrap_err(),
            OffAxisError::NonFiniteInput
        );
        assert_eq!(
            off_axis(&screen, ok_eye, f64::INFINITY, f64::INFINITY).unwrap_err(),
            OffAxisError::InvalidClipPlanes
        );
    }

    #[test]
    fn screen_basis_is_strictly_orthonormal_under_skewed_corners() {
        // pa, pb, pc where pc is intentionally slightly skewed (85 degrees instead of 90 degrees)
        let pa = Vec3::new(0.0, 0.0, 0.0);
        let pb = Vec3::new(1.0, 0.0, 0.0);
        let pc = Vec3::new(0.1, 0.9, 0.0);
        let screen = Screen::new(pa, pb, pc).unwrap();

        let vr = screen.vr();
        let vu = screen.vu();
        let vn = screen.vn();

        // Check unit lengths
        assert_close(vr.dot(vr), 1.0, EPS, "vr length");
        assert_close(vu.dot(vu), 1.0, EPS, "vu length");
        assert_close(vn.dot(vn), 1.0, EPS, "vn length");

        // Check pairwise orthogonality
        assert_close(vr.dot(vu), 0.0, EPS, "vr . vu");
        assert_close(vr.dot(vn), 0.0, EPS, "vr . vn");
        assert_close(vu.dot(vn), 0.0, EPS, "vu . vn");

        // Check right-handed orientation: vr x vu == vn
        let cross = vr.cross(vu);
        assert_close(cross.x, vn.x, EPS, "cross.x");
        assert_close(cross.y, vn.y, EPS, "cross.y");
        assert_close(cross.z, vn.z, EPS, "cross.z");
    }
}
