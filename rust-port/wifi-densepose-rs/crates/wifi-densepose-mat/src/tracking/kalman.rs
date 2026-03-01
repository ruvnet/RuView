//! Kalman filter for survivor position tracking.
//!
//! Implements a constant-velocity model in 3-D space.
//! State: [px, py, pz, vx, vy, vz] (metres, m/s)
//! Observation: [px, py, pz] (metres, from multi-AP triangulation)

/// 6×6 matrix type (row-major)
type Mat6 = [[f64; 6]; 6];
/// 3×3 matrix type (row-major)
type Mat3 = [[f64; 3]; 3];
/// 6-vector
type Vec6 = [f64; 6];
/// 3-vector
type Vec3 = [f64; 3];

/// Kalman filter state for a tracked survivor.
///
/// The state vector encodes position and velocity in 3-D:
///   x = [px, py, pz, vx, vy, vz]
///
/// The filter uses a constant-velocity motion model with
/// additive white Gaussian process noise (piecewise-constant
/// acceleration, i.e. the "Singer" / "white-noise jerk" discrete model).
#[derive(Debug, Clone)]
pub struct KalmanState {
    /// State estimate [px, py, pz, vx, vy, vz]
    pub x: Vec6,
    /// State covariance (6×6, symmetric positive-definite)
    pub p: Mat6,
    /// Process noise: σ_accel squared (m/s²)²
    process_noise_var: f64,
    /// Measurement noise: σ_obs squared (m)²
    obs_noise_var: f64,
}

impl KalmanState {
    /// Create new state from initial position observation.
    ///
    /// Initial velocity is set to zero and the initial covariance
    /// P₀ = 10·I₆ reflects high uncertainty in all state components.
    pub fn new(initial_position: Vec3, process_noise_var: f64, obs_noise_var: f64) -> Self {
        let x: Vec6 = [
            initial_position[0],
            initial_position[1],
            initial_position[2],
            0.0,
            0.0,
            0.0,
        ];

        // P₀ = 10 · I₆
        let mut p = [[0.0f64; 6]; 6];
        for i in 0..6 {
            p[i][i] = 10.0;
        }

        Self {
            x,
            p,
            process_noise_var,
            obs_noise_var,
        }
    }

    /// Predict forward by `dt_secs` using the constant-velocity model.
    ///
    /// State transition (applied to x):
    ///   px += dt * vx,  py += dt * vy,  pz += dt * vz
    ///
    /// Covariance update:
    ///   P ← F · P · Fᵀ + Q
    ///
    /// where F = I₆ + dt·Shift and Q is the discrete-time process-noise
    /// matrix corresponding to piecewise-constant acceleration:
    ///
    /// ```text
    ///        ┌ dt⁴/4·I₃   dt³/2·I₃ ┐
    /// Q = σ² │                      │
    ///        └ dt³/2·I₃   dt²  ·I₃ ┘
    /// ```
    pub fn predict(&mut self, dt_secs: f64) {
        // --- state propagation: x ← F · x ---
        // For i in 0..3: x[i] += dt * x[i+3]
        for i in 0..3 {
            self.x[i] += dt_secs * self.x[i + 3];
        }

        // --- build F explicitly (6×6) ---
        let mut f = mat6_identity();
        // upper-right 3×3 block = dt · I₃
        for i in 0..3 {
            f[i][i + 3] = dt_secs;
        }

        // --- covariance prediction: P ← F · P · Fᵀ + Q ---
        let ft = mat6_transpose(&f);
        let fp = mat6_mul(&f, &self.p);
        let fpft = mat6_mul(&fp, &ft);

        let q = build_process_noise(dt_secs, self.process_noise_var);
        self.p = mat6_add(&fpft, &q);
    }

    /// Update the filter with a 3-D position observation.
    ///
    /// Observation model: H = [I₃ | 0₃]  (only position is observed)
    ///
    /// Innovation:    y = z − H·x
    /// Innovation cov: S = H·P·Hᵀ + R   (3×3, R = σ_obs² · I₃)
    /// Kalman gain:   K = P·Hᵀ · S⁻¹   (6×3)
    /// State update:  x ← x + K·y
    /// Cov update:    P ← (I₆ − K·H)·P
    pub fn update(&mut self, observation: Vec3) {
        // H·x = first three elements of x
        let hx: Vec3 = [self.x[0], self.x[1], self.x[2]];

        // Innovation: y = z - H·x
        let y = vec3_sub(observation, hx);

        // P·Hᵀ = first 3 columns of P  (6×3 matrix)
        let ph_t = mat6x3_from_cols(&self.p);

        // H·P·Hᵀ = top-left 3×3 of P
        let hpht = mat3_from_top_left(&self.p);

        // S = H·P·Hᵀ + R  where R = obs_noise_var · I₃
        let mut s = hpht;
        for i in 0..3 {
            s[i][i] += self.obs_noise_var;
        }

        // S⁻¹ (3×3 analytical inverse)
        let s_inv = match mat3_inv(&s) {
            Some(m) => m,
            // If S is singular (degenerate geometry), skip update.
            None => return,
        };

        // K = P·Hᵀ · S⁻¹  (6×3)
        let k = mat6x3_mul_mat3(&ph_t, &s_inv);

        // x ← x + K · y  (6-vector update)
        let kv = mat6x3_mul_vec3(&k, y);
        self.x = vec6_add(self.x, kv);

        // P ← (I₆ − K·H) · P
        // K·H is a 6×6 matrix; since H = [I₃|0₃], (K·H)ᵢⱼ = K[i][j] for j<3, else 0.
        let mut kh = [[0.0f64; 6]; 6];
        for i in 0..6 {
            for j in 0..3 {
                kh[i][j] = k[i][j];
            }
        }
        let i_minus_kh = mat6_sub(&mat6_identity(), &kh);
        self.p = mat6_mul(&i_minus_kh, &self.p);
    }

    /// Squared Mahalanobis distance of `observation` to the predicted measurement.
    ///
    /// d² = (z − H·x)ᵀ · S⁻¹ · (z − H·x)
    ///
    /// where S = H·P·Hᵀ + R.
    ///
    /// Returns `f64::INFINITY` if S is singular.
    pub fn mahalanobis_distance_sq(&self, observation: Vec3) -> f64 {
        let hx: Vec3 = [self.x[0], self.x[1], self.x[2]];
        let y = vec3_sub(observation, hx);

        let hpht = mat3_from_top_left(&self.p);
        let mut s = hpht;
        for i in 0..3 {
            s[i][i] += self.obs_noise_var;
        }

        let s_inv = match mat3_inv(&s) {
            Some(m) => m,
            None => return f64::INFINITY,
        };

        // d² = yᵀ · S⁻¹ · y
        let s_inv_y = mat3_mul_vec3(&s_inv, y);
        s_inv_y[0] * y[0] + s_inv_y[1] * y[1] + s_inv_y[2] * y[2]
    }

    /// Current position estimate [px, py, pz].
    pub fn position(&self) -> Vec3 {
        [self.x[0], self.x[1], self.x[2]]
    }

    /// Current velocity estimate [vx, vy, vz].
    pub fn velocity(&self) -> Vec3 {
        [self.x[3], self.x[4], self.x[5]]
    }

    /// Scalar position uncertainty: trace of the top-left 3×3 of P.
    ///
    /// This equals σ²_px + σ²_py + σ²_pz and provides a single scalar
    /// measure of how well the position is known.
    pub fn position_uncertainty(&self) -> f64 {
        self.p[0][0] + self.p[1][1] + self.p[2][2]
    }
}

// ---------------------------------------------------------------------------
// Private math helpers
// ---------------------------------------------------------------------------

/// 6×6 matrix multiply: C = A · B.
fn mat6_mul(a: &Mat6, b: &Mat6) -> Mat6 {
    let mut c = [[0.0f64; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            for k in 0..6 {
                c[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    c
}

/// 6×6 matrix element-wise add.
fn mat6_add(a: &Mat6, b: &Mat6) -> Mat6 {
    let mut c = [[0.0f64; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            c[i][j] = a[i][j] + b[i][j];
        }
    }
    c
}

/// 6×6 matrix element-wise subtract: A − B.
fn mat6_sub(a: &Mat6, b: &Mat6) -> Mat6 {
    let mut c = [[0.0f64; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            c[i][j] = a[i][j] - b[i][j];
        }
    }
    c
}

/// 6×6 identity matrix.
fn mat6_identity() -> Mat6 {
    let mut m = [[0.0f64; 6]; 6];
    for i in 0..6 {
        m[i][i] = 1.0;
    }
    m
}

/// Transpose of a 6×6 matrix.
fn mat6_transpose(a: &Mat6) -> Mat6 {
    let mut t = [[0.0f64; 6]; 6];
    for i in 0..6 {
        for j in 0..6 {
            t[j][i] = a[i][j];
        }
    }
    t
}

/// Analytical inverse of a 3×3 matrix via cofactor expansion.
///
/// Returns `None` if |det| < 1e-12 (singular or near-singular).
fn mat3_inv(m: &Mat3) -> Option<Mat3> {
    // Cofactors (signed minors)
    let c00 = m[1][1] * m[2][2] - m[1][2] * m[2][1];
    let c01 = -(m[1][0] * m[2][2] - m[1][2] * m[2][0]);
    let c02 = m[1][0] * m[2][1] - m[1][1] * m[2][0];

    let c10 = -(m[0][1] * m[2][2] - m[0][2] * m[2][1]);
    let c11 = m[0][0] * m[2][2] - m[0][2] * m[2][0];
    let c12 = -(m[0][0] * m[2][1] - m[0][1] * m[2][0]);

    let c20 = m[0][1] * m[1][2] - m[0][2] * m[1][1];
    let c21 = -(m[0][0] * m[1][2] - m[0][2] * m[1][0]);
    let c22 = m[0][0] * m[1][1] - m[0][1] * m[1][0];

    // det = first row · first column of cofactor matrix (cofactor expansion)
    let det = m[0][0] * c00 + m[0][1] * c01 + m[0][2] * c02;

    if det.abs() < 1e-12 {
        return None;
    }

    let inv_det = 1.0 / det;

    // M⁻¹ = (1/det) · Cᵀ  (transpose of cofactor matrix)
    Some([
        [c00 * inv_det, c10 * inv_det, c20 * inv_det],
        [c01 * inv_det, c11 * inv_det, c21 * inv_det],
        [c02 * inv_det, c12 * inv_det, c22 * inv_det],
    ])
}

/// First 3 columns of a 6×6 matrix as a 6×3 matrix.
///
/// Because H = [I₃ | 0₃], P·Hᵀ equals the first 3 columns of P.
fn mat6x3_from_cols(p: &Mat6) -> [[f64; 3]; 6] {
    let mut out = [[0.0f64; 3]; 6];
    for i in 0..6 {
        for j in 0..3 {
            out[i][j] = p[i][j];
        }
    }
    out
}

/// Top-left 3×3 sub-matrix of a 6×6 matrix.
///
/// Because H = [I₃ | 0₃], H·P·Hᵀ equals the top-left 3×3 of P.
fn mat3_from_top_left(p: &Mat6) -> Mat3 {
    let mut out = [[0.0f64; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            out[i][j] = p[i][j];
        }
    }
    out
}

/// Element-wise add of two 6-vectors.
fn vec6_add(a: Vec6, b: Vec6) -> Vec6 {
    [
        a[0] + b[0],
        a[1] + b[1],
        a[2] + b[2],
        a[3] + b[3],
        a[4] + b[4],
        a[5] + b[5],
    ]
}

/// Multiply a 6×3 matrix by a 3-vector, yielding a 6-vector.
fn mat6x3_mul_vec3(m: &[[f64; 3]; 6], v: Vec3) -> Vec6 {
    let mut out = [0.0f64; 6];
    for i in 0..6 {
        for j in 0..3 {
            out[i] += m[i][j] * v[j];
        }
    }
    out
}

/// Multiply a 3×3 matrix by a 3-vector, yielding a 3-vector.
fn mat3_mul_vec3(m: &Mat3, v: Vec3) -> Vec3 {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

/// Element-wise subtract of two 3-vectors.
fn vec3_sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// Multiply a 6×3 matrix by a 3×3 matrix, yielding a 6×3 matrix.
fn mat6x3_mul_mat3(a: &[[f64; 3]; 6], b: &Mat3) -> [[f64; 3]; 6] {
    let mut out = [[0.0f64; 3]; 6];
    for i in 0..6 {
        for j in 0..3 {
            for k in 0..3 {
                out[i][j] += a[i][k] * b[k][j];
            }
        }
    }
    out
}

/// Build the discrete-time process-noise matrix Q.
///
/// Corresponds to piecewise-constant acceleration (white-noise acceleration)
/// integrated over a time step dt:
///
/// ```text
///        ┌ dt⁴/4·I₃   dt³/2·I₃ ┐
/// Q = σ² │                      │
///        └ dt³/2·I₃   dt²  ·I₃ ┘
/// ```
fn build_process_noise(dt: f64, q_a: f64) -> Mat6 {
    let dt2 = dt * dt;
    let dt3 = dt2 * dt;
    let dt4 = dt3 * dt;

    let qpp = dt4 / 4.0 * q_a; // position–position diagonal
    let qpv = dt3 / 2.0 * q_a; // position–velocity cross term
    let qvv = dt2 * q_a;        // velocity–velocity diagonal

    let mut q = [[0.0f64; 6]; 6];
    for i in 0..3 {
        q[i][i] = qpp;
        q[i + 3][i + 3] = qvv;
        q[i][i + 3] = qpv;
        q[i + 3][i] = qpv;
    }
    q
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A stationary filter (velocity = 0) should not move after a predict step.
    #[test]
    fn test_kalman_stationary() {
        let initial = [1.0, 2.0, 3.0];
        let mut state = KalmanState::new(initial, 0.01, 1.0);

        // No update — initial velocity is zero, so position should barely move.
        state.predict(0.5);

        let pos = state.position();
        assert!(
            (pos[0] - 1.0).abs() < 0.01,
            "px should remain near 1.0, got {}",
            pos[0]
        );
        assert!(
            (pos[1] - 2.0).abs() < 0.01,
            "py should remain near 2.0, got {}",
            pos[1]
        );
        assert!(
            (pos[2] - 3.0).abs() < 0.01,
            "pz should remain near 3.0, got {}",
            pos[2]
        );
    }

    /// With repeated predict + update cycles toward [5, 0, 0], the filter
    /// should converge so that px is within 2.0 of the target after 10 steps.
    #[test]
    fn test_kalman_update_converges() {
        let mut state = KalmanState::new([0.0, 0.0, 0.0], 1.0, 1.0);
        let target = [5.0, 0.0, 0.0];

        for _ in 0..10 {
            state.predict(0.5);
            state.update(target);
        }

        let pos = state.position();
        assert!(
            (pos[0] - 5.0).abs() < 2.0,
            "px should converge toward 5.0, got {}",
            pos[0]
        );
    }

    /// An observation equal to the current position estimate should give a
    /// very small Mahalanobis distance.
    #[test]
    fn test_mahalanobis_close_observation() {
        let state = KalmanState::new([3.0, 4.0, 5.0], 0.1, 0.5);
        let obs = state.position(); // observation = current estimate

        let d2 = state.mahalanobis_distance_sq(obs);
        assert!(
            d2 < 1.0,
            "Mahalanobis distance² for the current position should be < 1.0, got {}",
            d2
        );
    }

    /// An observation 100 m from the current position should yield a large
    /// Mahalanobis distance (far outside the uncertainty ellipsoid).
    #[test]
    fn test_mahalanobis_far_observation() {
        // Use small obs_noise_var so the uncertainty ellipsoid is tight.
        let state = KalmanState::new([0.0, 0.0, 0.0], 0.01, 0.01);
        let far_obs = [100.0, 0.0, 0.0];

        let d2 = state.mahalanobis_distance_sq(far_obs);
        assert!(
            d2 > 9.0,
            "Mahalanobis distance² for a 100 m observation should be >> 9, got {}",
            d2
        );
    }
}
