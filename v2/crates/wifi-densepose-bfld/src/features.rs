//! Motion features from decoded BFI reports (ADR-365, plan feature path A).
//!
//! Turns the compressed feedback angles into a bounded, model-ready motion
//! signal without reconstructing the steering matrix `V` (which does not
//! recover the full channel `H`, so no range/AoA is claimed). Two pieces:
//!
//! - [`ReportFeatures`] — a per-report spatial summary (circular mean of the
//!   periodic `Phi`, mean of the bounded `Psi`, mean average SNR).
//! - [`motion_energy`] — the temporal signal: mean per-angle change between two
//!   consecutive reports, with `Phi` differenced on the circle and `Psi`
//!   linearly. This is the movement primitive a presence/motion model consumes.
//!
//! All values are `SYNTHETIC`-validated (unit tests); no motion-accuracy claim
//! is made until a real capture with ground truth exists.

#![cfg(feature = "std")]
// `phi`/`psi` are the IEEE 802.11 names; casts are of small bounded angle and
// stream counts where f64 precision loss cannot occur.
#![allow(clippy::similar_names, clippy::cast_precision_loss)]

use crate::capture::{CapturedReport, Report};

/// Bounded per-report spatial summary.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ReportFeatures {
    /// Circular mean of the `Phi` angles, radians in `[0, 2*pi)`.
    pub phi_circular_mean: f64,
    /// Circular concentration of `Phi` in `[0, 1]` (1 = all aligned).
    pub phi_concentration: f64,
    /// Mean of the `Psi` angles, radians in `[0, pi/2]`.
    pub psi_mean: f64,
    /// Mean average SNR across streams, in the report's raw 0.25 dB units.
    pub mean_snr: f64,
}

impl ReportFeatures {
    /// Summarize one decoded report (VHT or HE).
    #[must_use]
    pub fn from_report(r: &Report) -> Self {
        let (phi, psi, snr) = (r.phi(), r.psi(), r.avg_snr());
        let (mut sin_sum, mut cos_sum) = (0.0f64, 0.0f64);
        for &c in phi {
            let a = r.phi_radians(c);
            sin_sum += a.sin();
            cos_sum += a.cos();
        }
        let n = phi.len().max(1) as f64;
        let resultant = sin_sum.hypot(cos_sum) / n;
        let phi_mean = {
            let m = sin_sum.atan2(cos_sum);
            if m < 0.0 { m + core::f64::consts::TAU } else { m }
        };
        let psi_mean = if psi.is_empty() {
            0.0
        } else {
            psi.iter().map(|&c| r.psi_radians(c)).sum::<f64>() / psi.len() as f64
        };
        let mean_snr = if snr.is_empty() {
            0.0
        } else {
            snr.iter().map(|&s| f64::from(s)).sum::<f64>() / snr.len() as f64
        };
        Self { phi_circular_mean: phi_mean, phi_concentration: resultant, psi_mean, mean_snr }
    }
}

/// Mean per-angle change between two consecutive reports, in `[0, 1]`.
///
/// `Phi` is differenced on the circle (periodic), `Psi` linearly (bounded), each
/// normalized to its own range. Returns `None` when the reports have different
/// dimensions (a reconfiguration, not motion).
#[must_use]
pub fn motion_energy(prev: &Report, cur: &Report) -> Option<f64> {
    let (pphi, ppsi, cphi, cpsi) = (prev.phi(), prev.psi(), cur.phi(), cur.psi());
    if pphi.len() != cphi.len()
        || ppsi.len() != cpsi.len()
        || prev.phi_bits() != cur.phi_bits()
        || prev.psi_bits() != cur.psi_bits()
    {
        return None;
    }
    let total = pphi.len() + ppsi.len();
    if total == 0 {
        return Some(0.0);
    }

    let phi_span = f64::from(1u32 << prev.phi_bits());
    let phi_half = phi_span / 2.0;
    let mut acc = 0.0f64;
    for (&a, &b) in pphi.iter().zip(cphi) {
        let raw = f64::from(a) - f64::from(b);
        let d = raw.abs().min(phi_span - raw.abs()); // circular distance in code space
        acc += d / phi_half; // normalize to [0, 1]
    }
    let psi_span = f64::from(1u32 << prev.psi_bits());
    for (&a, &b) in ppsi.iter().zip(cpsi) {
        acc += (f64::from(a) - f64::from(b)).abs() / psi_span;
    }
    Some(acc / total as f64)
}

/// Motion-energy time series from a decoded capture, one value per consecutive
/// report pair whose dimensions match. Pairs across a reconfiguration are
/// skipped (they carry no motion meaning).
#[must_use]
pub fn motion_series(reports: &[CapturedReport]) -> Vec<(u64, f64)> {
    reports
        .windows(2)
        .filter_map(|w| motion_energy(&w[0].report, &w[1].report).map(|e| (w[1].ts_us, e)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{motion_energy, motion_series, ReportFeatures};
    use crate::capture::{CapturedReport, Report};
    use crate::cbr::parse_vht_action;

    /// Build a decoded 2x2/80MHz/Ng1/SU-cb0 report with all angle codes = `code`.
    fn report(code: u16) -> Report {
        let v: u32 = (2 - 1) | ((2 - 1) << 3) | (2 << 6) | (1 << 18);
        let ctl = &v.to_le_bytes()[..3];
        let ns = 234usize;
        let (phi_bits, psi_bits) = (4u32, 2u32); // SU codebook 0
        let mut acc = 0u32;
        let mut held = 0u32;
        let mut stream = Vec::new();
        let mut push = |val: u16, w: u32| {
            acc |= (u32::from(val) & ((1 << w) - 1)) << held;
            held += w;
            while held >= 8 {
                stream.push(u8::try_from(acc & 0xff).unwrap());
                acc >>= 8;
                held -= 8;
            }
        };
        for _ in 0..ns {
            push(code % (1 << phi_bits), phi_bits);
            push(code % (1 << psi_bits), psi_bits);
        }
        if held > 0 {
            stream.push(u8::try_from(acc & 0xff).unwrap());
        }
        let mut body = vec![21u8, 0];
        body.extend_from_slice(ctl);
        body.extend_from_slice(&[0, 0]);
        body.extend_from_slice(&stream);
        Report::Vht(parse_vht_action(&body).unwrap())
    }

    #[test]
    fn identical_reports_have_zero_motion() {
        let a = report(10);
        let b = report(10);
        assert_eq!(motion_energy(&a, &b), Some(0.0));
    }

    #[test]
    fn changed_angles_produce_motion() {
        let a = report(0);
        let b = report(20);
        let e = motion_energy(&a, &b).unwrap();
        assert!(e > 0.0 && e <= 1.0, "motion {e} out of range");
    }

    #[test]
    fn phi_difference_is_circular() {
        // phi_bits=4 (SU cb0), so phi codes span 0..16. Hold psi constant by
        // choosing codes that are all 0 mod 4 (psi = code % 4). Code 12 is only
        // 4 steps from 0 across the wrap; code 8 is a half-circle away.
        let near_wrap = motion_energy(&report(0), &report(12)).unwrap();
        let half = motion_energy(&report(0), &report(8)).unwrap();
        assert!(near_wrap < half, "wrap {near_wrap} should be < half-circle {half}");
    }

    #[test]
    fn report_features_bounded() {
        let f = ReportFeatures::from_report(&report(30));
        assert!(f.phi_circular_mean >= 0.0 && f.phi_circular_mean < core::f64::consts::TAU);
        assert!(f.phi_concentration >= 0.0 && f.phi_concentration <= 1.0 + 1e-9);
        assert!(f.psi_mean >= 0.0 && f.psi_mean <= core::f64::consts::FRAC_PI_2);
    }

    #[test]
    fn series_skips_dimension_changes() {
        let mk = |ts: u64, code: u16| CapturedReport { ts_us: ts, sequence: 0, report: report(code) };
        let reports = vec![mk(0, 5), mk(1000, 25), mk(2000, 25)];
        let series = motion_series(&reports);
        assert_eq!(series.len(), 2);
        assert_eq!(series[1], (2000, 0.0)); // last pair identical → 0 motion
    }
}
