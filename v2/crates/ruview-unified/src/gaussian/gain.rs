//! Channel-gain queries against the Gaussian map, and the inverse update
//! that makes the map *learn* from measured links (ADR-275 §4).
//!
//! Physics model: free-space Friis amplitude with Beer–Lambert extinction
//! through the Gaussians,
//!
//! ```text
//! H(tx,rx,f) = (λ / 4πd) · e^{-j·2πd/λ} · exp(-Σ_g occ_g · I_g)
//! ```
//!
//! where `I_g = ∫₀ᴸ exp(-½·q_g(o + t·u)) dt` is the closed-form line
//! integral of Gaussian `g`'s unnormalized density along the TX→RX segment
//! (a 1-D Gaussian integral, evaluated with `erf`). Two exactness anchors
//! make this testable: an **empty map returns exact Friis**, and adding an
//! absorber strictly, monotonically reduces gain.

use num_complex::Complex64;

use super::map::GaussianMap;
use super::primitive::{Provenance, RfGaussian};
use crate::math::erf;

const C: f64 = 299_792_458.0;

/// Closed-form line integral of `exp(-½·(x-μ)ᵀΣ⁻¹(x-μ))` along the segment
/// `o → o + L·u` (`u` unit).
///
/// With `a = uᵀΣ⁻¹u`, `b = uᵀΣ⁻¹(μ−o)`, `c = (μ−o)ᵀΣ⁻¹(μ−o)`:
/// `q(t) = a·(t − b/a)² + (c − b²/a)`, so
/// `I = e^{-(c−b²/a)/2} · √(π/2a) · [erf(√(a/2)(L−t₀)) + erf(√(a/2)·t₀)]`.
#[must_use]
pub fn line_integral(g: &RfGaussian, o: [f64; 3], u: [f64; 3], len: f64) -> f64 {
    let si = g.inv_covariance();
    let mo = [g.position[0] - o[0], g.position[1] - o[1], g.position[2] - o[2]];
    let mut a = 0.0;
    let mut b = 0.0;
    let mut c = 0.0;
    for i in 0..3 {
        for j in 0..3 {
            a += u[i] * si[i][j] * u[j];
            b += u[i] * si[i][j] * mo[j];
            c += mo[i] * si[i][j] * mo[j];
        }
    }
    if a <= 0.0 {
        return 0.0;
    }
    let t0 = b / a;
    let peak = (-0.5 * (c - b * b / a)).exp();
    let s = (a / 2.0).sqrt();
    peak * (std::f64::consts::PI / (2.0 * a)).sqrt() * (erf(s * (len - t0)) + erf(s * t0))
}

/// Total optical depth (nepers) of the map along a TX→RX segment, plus the
/// per-Gaussian integrals for gradient use: `(τ, [(idx, I_g)])`.
#[must_use]
pub fn optical_depth(map: &GaussianMap, tx: [f64; 3], rx: [f64; 3]) -> (f64, Vec<(usize, f64)>) {
    let d = [rx[0] - tx[0], rx[1] - tx[1], rx[2] - tx[2]];
    let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
    if len < 1e-9 {
        return (0.0, Vec::new());
    }
    let u = [d[0] / len, d[1] / len, d[2] / len];
    // Candidate set: Gaussians within a 3 m corridor of the segment (3σ for
    // σ ≤ 1 m primitives), gathered by walking only the hash cells along
    // the link instead of a ball around its midpoint — see
    // `GaussianMap::query_near_segment` for the measured effect.
    let candidates = map.query_near_segment(tx, rx, 3.0);
    let mut tau = 0.0;
    let mut parts = Vec::new();
    for i in candidates {
        let g = &map.gaussians()[i];
        let integral = line_integral(g, tx, u, len);
        if integral > 1e-12 {
            tau += g.occupancy * integral;
            parts.push((i, integral));
        }
    }
    (tau, parts)
}

/// Complex channel gain between two points at carrier `freq_hz`.
///
/// Empty map ⇒ exact free-space Friis amplitude `λ/(4πd)` with propagation
/// phase `e^{-j2πd/λ}`.
#[must_use]
pub fn channel_gain(map: &GaussianMap, tx: [f64; 3], rx: [f64; 3], freq_hz: f64) -> Complex64 {
    let d = ((rx[0] - tx[0]).powi(2) + (rx[1] - tx[1]).powi(2) + (rx[2] - tx[2]).powi(2)).sqrt();
    if d < 1e-9 {
        return Complex64::new(1.0, 0.0);
    }
    let lambda = C / freq_hz;
    let amp = lambda / (4.0 * std::f64::consts::PI * d);
    let phase = -2.0 * std::f64::consts::PI * d / lambda;
    let (tau, _) = optical_depth(map, tx, rx);
    Complex64::from_polar(amp * (-tau).exp(), phase)
}

/// Power gain in dB.
#[must_use]
pub fn gain_db(map: &GaussianMap, tx: [f64; 3], rx: [f64; 3], freq_hz: f64) -> f64 {
    20.0 * channel_gain(map, tx, rx, freq_hz).norm().log10()
}

/// One inverse-update step from a measured link amplitude (ADR-275 §4.2 —
/// the physics-informed incremental mapping move: when the environment
/// changes, adjust or add a *compact* set of Gaussians instead of remapping).
///
/// The measured optical depth is `τ* = ln(friis_amp / measured_amp)`; the
/// map's depth is `τ = Σ occ_g·I_g`. A projected-gradient step moves the
/// intersected occupancies toward the residual `r = τ* − τ`:
///
/// `occ_g += lr · r · I_g / (Σ I_g² + ε)`, clamped at 0 —
///
/// which is exact Newton along this link's direction at `lr = 1`. When no
/// Gaussian meaningfully intersects the segment and the residual demands
/// attenuation, a new isotropic Gaussian is spawned at the segment midpoint
/// sized to explain the residual. Returns the post-update residual (nepers).
pub fn observe_link(
    map: &mut GaussianMap,
    tx: [f64; 3],
    rx: [f64; 3],
    freq_hz: f64,
    measured_amp: f64,
    lr: f64,
    timestamp_ns: u64,
) -> f64 {
    assert!(measured_amp > 0.0, "measured amplitude must be positive");
    let d = ((rx[0] - tx[0]).powi(2) + (rx[1] - tx[1]).powi(2) + (rx[2] - tx[2]).powi(2)).sqrt();
    let lambda = C / freq_hz;
    let friis = lambda / (4.0 * std::f64::consts::PI * d);
    let tau_target = (friis / measured_amp).ln();

    let (tau, parts) = optical_depth(map, tx, rx);
    let residual = tau_target - tau;

    let sum_i2: f64 = parts.iter().map(|(_, i)| i * i).sum();
    if sum_i2 > 1e-9 {
        for (idx, integral) in &parts {
            let g = &mut map.gaussians_mut()[*idx];
            g.occupancy = (g.occupancy + lr * residual * integral / sum_i2).max(0.0);
            g.timestamp_ns = g.timestamp_ns.max(timestamp_ns);
        }
        map.rebuild_grid();
    } else if residual > 0.05 {
        // Nothing on the path can explain the attenuation: spawn a compact
        // absorber at the midpoint sized to close the residual exactly.
        let mid = [(tx[0] + rx[0]) / 2.0, (tx[1] + rx[1]) / 2.0, (tx[2] + rx[2]) / 2.0];
        let mut g = RfGaussian::new(
            mid,
            [0.3, 0.3, 0.3],
            [1.0, 0.0, 0.0, 0.0],
            0.0,
            0.6,
            timestamp_ns,
            300.0,
            Provenance { device_id: "gain-inverse".into(), model_version: 1, synthetic: false },
        )
        .expect("spawn parameters are statically valid");
        let dvec = [rx[0] - tx[0], rx[1] - tx[1], rx[2] - tx[2]];
        let u = [dvec[0] / d, dvec[1] / d, dvec[2] / d];
        let unit_integral = line_integral(&g, tx, u, d);
        if unit_integral > 1e-9 {
            g.occupancy = residual / unit_integral;
            map.insert(g);
        }
    }

    let (tau_after, _) = optical_depth(map, tx, rx);
    tau_target - tau_after
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gaussian::primitive::Provenance;

    fn prov() -> Provenance {
        Provenance { device_id: "gain-test".into(), model_version: 1, synthetic: true }
    }

    /// Free space is the free-space Friis term, with and without a map.
    ///
    /// An empty map has no absorption at all: `optical_depth` short-circuits
    /// to `tau = 0` before the loop that calls `line_integral`/`erf`, so the
    /// first half of this test is the free-space term and nothing else. The
    /// closed-form path through `line_integral` is covered by
    /// `line_integral_matches_numeric_quadrature` (RuView#1438), so this test
    /// is not cited as validating it. The second half repeats the same
    /// expectation with a transparent Gaussian in the map, which does go
    /// through the erf/Beer-Lambert path and must give the same answer.
    #[test]
    fn free_space_friis_holds_with_and_without_a_transparent_map() {
        let f = 2.437e9;
        let tx = [0.0, 0.0, 1.0];
        let rx = [4.0, 3.0, 1.0]; // d = 5
        let lambda = C / f;
        let expected_amp = lambda / (4.0 * std::f64::consts::PI * 5.0);
        let expected_phase = -2.0 * std::f64::consts::PI * 5.0 / lambda;

        // Empty map: no absorption, so the gain is the free-space Friis term.
        let map = GaussianMap::new(1.0);
        let h = channel_gain(&map, tx, rx, f);
        assert!((h.norm() - expected_amp).abs() < 1e-15, "amplitude must be exact Friis");
        // Compare phasors (phase is only defined mod 2π).
        let diff = (h / Complex64::from_polar(expected_amp, expected_phase)) - 1.0;
        assert!(diff.norm() < 1e-9, "propagation phase must match");

        // A map holding only a transparent Gaussian goes through the
        // erf/Beer-Lambert path and must give the same free-space answer.
        let mut transparent = GaussianMap::new(1.0);
        let g = RfGaussian::new(
            [2.0, 1.5, 1.0],
            [0.4, 0.4, 0.4],
            [1.0, 0.0, 0.0, 0.0],
            0.0,
            0.9,
            0,
            300.0,
            prov(),
        )
        .expect("valid");
        transparent.insert(g);
        let h = channel_gain(&transparent, tx, rx, f);
        assert!(
            (h.norm() - expected_amp).abs() < 1e-12,
            "a zero-occupancy map must not attenuate: got {} dB",
            gain_db(&transparent, tx, rx, f)
        );
    }

    #[test]
    fn absorber_on_path_reduces_gain_monotonically() {
        let f = 5.18e9;
        let tx = [0.0, 0.0, 1.0];
        let rx = [6.0, 0.0, 1.0];
        let mut last = f64::INFINITY;
        for occ in [0.0, 0.2, 0.5, 1.0, 2.0] {
            let mut map = GaussianMap::new(1.0);
            let g = RfGaussian::new(
                [3.0, 0.0, 1.0],
                [0.4, 0.4, 0.4],
                [1.0, 0.0, 0.0, 0.0],
                occ,
                0.9,
                0,
                300.0,
                prov(),
            )
            .expect("valid");
            map.insert(g);
            let db = gain_db(&map, tx, rx, f);
            assert!(db < last + 1e-12, "gain must fall as occupancy rises");
            last = db;
        }
    }

    #[test]
    fn off_path_absorber_barely_matters() {
        let f = 5.18e9;
        let tx = [0.0, 0.0, 1.0];
        let rx = [6.0, 0.0, 1.0];
        let mut map = GaussianMap::new(1.0);
        map.insert(
            RfGaussian::new(
                [3.0, 4.0, 1.0], // 4 m off the LoS, σ = 0.4 ⇒ 10σ away
                [0.4, 0.4, 0.4],
                [1.0, 0.0, 0.0, 0.0],
                2.0,
                0.9,
                0,
                300.0,
                prov(),
            )
            .expect("valid"),
        );
        let clean = gain_db(&GaussianMap::new(1.0), tx, rx, f);
        let with = gain_db(&map, tx, rx, f);
        assert!((clean - with).abs() < 1e-6, "off-path matter must not attenuate LoS");
    }

    #[test]
    fn line_integral_matches_numeric_quadrature() {
        let g = RfGaussian::new(
            [2.0, 0.5, 1.0],
            [0.5, 0.8, 0.3],
            [0.9, 0.1, 0.2, 0.1], // arbitrary rotation
            1.0,
            0.9,
            0,
            300.0,
            prov(),
        )
        .expect("valid");
        let o = [0.0, 0.0, 1.0];
        let u = [1.0, 0.0, 0.0];
        let len = 6.0;
        // Trapezoid quadrature at 1 mm steps as ground truth.
        let n = 6000;
        let mut acc = 0.0;
        for i in 0..=n {
            let t = len * i as f64 / n as f64;
            let w = if i == 0 || i == n { 0.5 } else { 1.0 };
            acc += w * g.density_at([o[0] + t * u[0], o[1] + t * u[1], o[2] + t * u[2]]);
        }
        acc *= len / n as f64;
        let closed = line_integral(&g, o, u, len);
        assert!(
            (closed - acc).abs() < 1e-6,
            "closed form {closed} vs quadrature {acc}"
        );
    }

    /// One link, one unknown: the occupancy of a single absorber converges
    /// from a repeated measurement of the same link.
    ///
    /// This is a convergence check of the `observe_link` update rule -- each
    /// call is a damped Newton step and the residual shrinks geometrically --
    /// and not evidence of learning from diverse observations (RuView#1438).
    /// `two_distinct_links_resolve_two_distinct_absorbers` is the one that
    /// resolves different unknowns.
    #[test]
    fn single_link_occupancy_converges_to_the_measured_absorption() {
        let f = 2.437e9;
        let tx = [0.0, 0.0, 1.0];
        let rx = [6.0, 0.0, 1.0];
        let lambda = C / f;
        let friis = lambda / (4.0 * std::f64::consts::PI * 6.0);
        // Ground truth: an unseen absorber imposes τ = 0.7 nepers (≈ 6.1 dB).
        let measured = friis * (-0.7f64).exp();

        let mut map = GaussianMap::new(1.0);
        let mut residual = f64::INFINITY;
        for step in 0..20 {
            residual = observe_link(&mut map, tx, rx, f, measured, 0.7, step);
        }
        assert!(!map.is_empty(), "an absorber must have been spawned");
        assert!(
            residual.abs() < 0.06,
            "residual after convergence must be < 0.06 nepers (≈0.5 dB), got {residual}"
        );
        let predicted_db = gain_db(&map, tx, rx, f);
        let measured_db = 20.0 * measured.log10();
        assert!(
            (predicted_db - measured_db).abs() < 0.5,
            "map prediction {predicted_db:.2} dB must match measurement {measured_db:.2} dB"
        );
    }

    /// RuView#1438 (item 3): several distinct links must resolve genuinely
    /// different unknowns.
    ///
    /// The single-link test above converges one scalar from one repeated
    /// measurement. Here the map gets two links with different geometry and
    /// different true attenuations, and each link has to end up explained by
    /// its own absorber -- the one it spawned -- rather than by the other.
    #[test]
    fn two_distinct_links_resolve_two_distinct_absorbers() {
        let f = 2.437e9;
        // Two links out of the same corner, in different directions.
        let (tx_a, rx_a) = ([0.0, 0.0, 1.0], [6.0, 0.0, 1.0]);
        let (tx_b, rx_b) = ([0.0, 0.0, 1.0], [0.0, 6.0, 1.0]);

        // Ground truth: one absorber on each path, with different optical
        // depths (0.7 and 0.3 nepers).
        let mut truth = GaussianMap::new(1.0);
        for (pos, occ) in [([3.0, 0.0, 1.0], 0.7), ([0.0, 3.0, 1.0], 0.3)] {
            let g = RfGaussian::new(
                pos,
                [0.3, 0.3, 0.3],
                [1.0, 0.0, 0.0, 0.0],
                occ,
                0.9,
                0,
                300.0,
                prov(),
            )
            .expect("valid");
            truth.insert(g);
        }
        let measured_a = channel_gain(&truth, tx_a, rx_a, f).norm();
        let measured_b = channel_gain(&truth, tx_b, rx_b, f).norm();

        // Learn from an empty map, alternating the two links.
        let mut map = GaussianMap::new(1.0);
        let mut res_a = f64::INFINITY;
        let mut res_b = f64::INFINITY;
        for step in 0..20 {
            res_a = observe_link(&mut map, tx_a, rx_a, f, measured_a, 0.7, step);
            res_b = observe_link(&mut map, tx_b, rx_b, f, measured_b, 0.7, step);
        }
        assert!(res_a.abs() < 0.06, "link A residual {res_a} must converge");
        assert!(res_b.abs() < 0.06, "link B residual {res_b} must converge");
        assert_eq!(
            map.len(),
            2,
            "one absorber per link is expected, got {}",
            map.len()
        );

        // Each link must be explained by the absorber on its own path.
        let occ_a = map
            .gaussians()
            .iter()
            .filter(|g| g.position[1] < 1.5)
            .map(|g| g.occupancy)
            .fold(0.0f64, f64::max);
        let occ_b = map
            .gaussians()
            .iter()
            .filter(|g| g.position[0] < 1.5)
            .map(|g| g.occupancy)
            .fold(0.0f64, f64::max);
        assert!(
            (occ_a - 0.7).abs() < 0.05,
            "link A must resolve its own absorber (occupancy {occ_a:.3}, expected 0.7)"
        );
        assert!(
            (occ_b - 0.3).abs() < 0.05,
            "link B must resolve its own absorber (occupancy {occ_b:.3}, expected 0.3)"
        );
        assert!(
            (occ_a - occ_b).abs() > 0.2,
            "the two links resolved different unknowns, got {occ_a:.3} and {occ_b:.3}"
        );
    }
}
