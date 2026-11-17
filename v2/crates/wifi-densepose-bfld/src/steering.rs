//! Steering-matrix (V) reconstruction from decoded beamforming angles.
//!
//! ADR-365, plan feature-path B. Inverse Givens rotation per IEEE 802.11
//! §19.3.11.10.2, ported from the `WiPiCap` reference `inverse_givens_rotation`.
//!
//! Reconstructed V is the beamformee's compressed feedback matrix, NOT the full
//! channel H — so no calibrated range or coherent angle-of-arrival is claimed. Its
//! value here is twofold: it is feature-path B (temporal changes in V track
//! motion), and its **unitarity** (V^H V = I) is a decoder correctness oracle —
//! if the angle bit widths, ordering, and dequantization are right, V
//! reconstructed from any valid angle codes is unitary. That is checked on
//! `SYNTHETIC` angles here and is the same gate that promotes a real capture
//! from `CLAIMED` to `MEASURED`.

#![cfg(feature = "std")]
// phi/psi and nr/nc are the 802.11 names.
#![allow(clippy::similar_names, clippy::many_single_char_names)]

use crate::cbr::angles_per_subcarrier;

/// Minimal complex number for the small (<=4x4) reconstruction matrices.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Complex {
    /// Real part.
    pub re: f64,
    /// Imaginary part.
    pub im: f64,
}

impl Complex {
    const ZERO: Self = Self { re: 0.0, im: 0.0 };
    const ONE: Self = Self { re: 1.0, im: 0.0 };

    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }
    /// `e^{i·theta}`.
    fn expi(theta: f64) -> Self {
        Self { re: theta.cos(), im: theta.sin() }
    }
    fn add(self, o: Self) -> Self {
        Self { re: self.re + o.re, im: self.im + o.im }
    }
    fn mul(self, o: Self) -> Self {
        Self { re: self.re.mul_add(o.re, -(self.im * o.im)), im: self.re.mul_add(o.im, self.im * o.re) }
    }
    fn conj(self) -> Self {
        Self { re: self.re, im: -self.im }
    }
}

type Mat = Vec<Vec<Complex>>;

fn eye(rows: usize, cols: usize) -> Mat {
    (0..rows)
        .map(|r| (0..cols).map(|c| if r == c { Complex::ONE } else { Complex::ZERO }).collect())
        .collect()
}

/// `a^T · b` (plain transpose, matching the real Givens `.T` in `WiPiCap`).
fn matmul_tn(a: &Mat, b: &Mat) -> Mat {
    let (n, k, m) = (a[0].len(), a.len(), b[0].len());
    let mut out = vec![vec![Complex::ZERO; m]; n];
    for (i, row) in out.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            let mut acc = Complex::ZERO;
            for p in 0..k {
                acc = acc.add(a[p][i].mul(b[p][j])); // a^T[i][p] = a[p][i]
            }
            *cell = acc;
        }
    }
    out
}

#[derive(Clone, Copy)]
enum Kind {
    Phi,
    Psi,
}

/// Regenerate the ordered Givens angle specs (kind + matrix position) for an
/// (Nr, Nc) feedback matrix, matching the decode order used by `cbr`.
fn angle_specs(nr: usize, nc: usize) -> Vec<(Kind, usize, usize)> {
    let (nr8, nc8) = (u8::try_from(nr).expect("nr <= 4"), u8::try_from(nc).expect("nc <= 4"));
    let total = angles_per_subcarrier(nr8, nc8) * 2;
    let mut specs = Vec::with_capacity(total);
    let (mut phi0, mut phi1) = (0usize, 0usize);
    let (mut psi0, mut psi1) = (1usize, 0usize);
    let mut cnt = nr - 1;
    while specs.len() < total && cnt > 0 {
        for i in 0..cnt {
            specs.push((Kind::Phi, phi0 + i, phi1));
        }
        phi0 += 1;
        phi1 += 1;
        for i in 0..cnt {
            specs.push((Kind::Psi, psi0 + i, psi1));
        }
        psi0 += 1;
        psi1 += 1;
        cnt -= 1;
    }
    specs
}

/// Reconstruct the `Nr x Nc` steering matrix V for one subcarrier from its
/// per-level Phi and Psi angle codes (each length [`angles_per_subcarrier`]).
///
/// `phi`/`psi` are the raw codes for this subcarrier in level order (as `cbr`
/// stores them); `phi_bits`/`psi_bits` are their quantization widths.
#[must_use]
pub fn reconstruct_v(
    nr: u8,
    nc: u8,
    phi: &[u16],
    psi: &[u16],
    phi_bits: u32,
    psi_bits: u32,
) -> Mat {
    let (nrz, ncz) = (usize::from(nr), usize::from(nc));
    let specs = angle_specs(nrz, ncz);

    // Dequantize into spec order (phi then psi per level), matching angle_specs.
    let phi_rad = |c: u16| (f64::from(c) + 0.5) * core::f64::consts::PI / f64::from(1u32 << (phi_bits - 1));
    let psi_rad = |c: u16| (f64::from(c) + 0.5) * core::f64::consts::PI / f64::from(1u32 << (psi_bits + 1));
    let mut angles = Vec::with_capacity(specs.len());
    let last = core::cmp::min(nc, nr.saturating_sub(1));
    let (mut pi, mut si) = (0usize, 0usize);
    for i in 1..=last {
        let cnt = usize::from(nr - i);
        for _ in 0..cnt {
            angles.push(phi_rad(phi[pi]));
            pi += 1;
        }
        for _ in 0..cnt {
            angles.push(psi_rad(psi[si]));
            si += 1;
        }
    }

    let mut mat_e = eye(nrz, ncz);
    let mut d_li = eye(nrz, nrz);
    let mut d_count = 0usize;
    let mut d_patience = 1usize;
    for idx in (0..angles.len()).rev() {
        let (kind, a0, a1) = specs[idx];
        match kind {
            Kind::Phi => {
                d_li[a0][a0] = Complex::expi(angles[idx]);
                d_count += 1;
            }
            Kind::Psi => {
                let (c, s) = (angles[idx].cos(), angles[idx].sin());
                let mut g = eye(nrz, nrz);
                g[a1][a1] = Complex::new(c, 0.0);
                g[a1][a0] = Complex::new(s, 0.0);
                g[a0][a1] = Complex::new(-s, 0.0);
                g[a0][a0] = Complex::new(c, 0.0);
                mat_e = matmul_tn(&g, &mat_e);
            }
        }
        if d_count == d_patience {
            mat_e = matmul_tn(&d_li, &mat_e);
            d_patience += 1;
            d_count = 0;
            d_li = eye(nrz, nrz);
        }
    }
    mat_e
}

/// Maximum deviation of `V^H V` from the `Nc x Nc` identity (0 == perfectly
/// unitary columns). The decoder correctness oracle.
#[must_use]
pub fn unitarity_error(v: &Mat) -> f64 {
    let nc = v[0].len();
    let mut worst = 0.0f64;
    for i in 0..nc {
        for j in 0..nc {
            // (V^H V)[i][j] = sum_r conj(V[r][i]) * V[r][j]
            let mut acc = Complex::ZERO;
            for row in v {
                acc = acc.add(row[i].conj().mul(row[j]));
            }
            let expect = if i == j { 1.0 } else { 0.0 };
            worst = worst.max((acc.re - expect).abs()).max(acc.im.abs());
        }
    }
    worst
}

#[cfg(test)]
mod tests {
    use super::{reconstruct_v, unitarity_error};

    /// V reconstructed from ANY valid angle codes must be unitary — this
    /// validates the bit widths, ordering, and dequantization together.
    #[test]
    fn reconstructed_v_is_unitary_across_configs() {
        // (nr, nc, phi_bits, psi_bits)
        let cfgs = [(2u8, 1u8), (2, 2), (3, 1), (3, 2), (3, 3), (4, 2), (4, 4)];
        for (nr, nc) in cfgs {
            let per = crate::cbr::angles_per_subcarrier(nr, nc);
            // deterministic pseudo-random codes within range
            let code = |k: usize, m: u16, b: u16, modn: u16| (u16::try_from(k).unwrap() * m + b) % modn;
            let phi: Vec<u16> = (0..per).map(|k| code(k, 37, 11, 16)).collect();
            let psi: Vec<u16> = (0..per).map(|k| code(k, 19, 3, 4)).collect();
            let v = reconstruct_v(nr, nc, &phi, &psi, 4, 2);
            assert_eq!(v.len(), usize::from(nr));
            assert_eq!(v[0].len(), usize::from(nc));
            let err = unitarity_error(&v);
            assert!(err < 1e-9, "Nr{nr}xNc{nc}: unitarity error {err}");
        }
    }

    #[test]
    fn different_angles_give_different_v() {
        let a = reconstruct_v(2, 2, &[3], &[1], 4, 2);
        let b = reconstruct_v(2, 2, &[10], &[2], 4, 2);
        let diff: f64 = a
            .iter()
            .zip(&b)
            .flat_map(|(ra, rb)| ra.iter().zip(rb))
            .map(|(x, y)| (x.re - y.re).abs() + (x.im - y.im).abs())
            .sum();
        assert!(diff > 1e-6, "distinct angle codes should give distinct V");
    }
}
