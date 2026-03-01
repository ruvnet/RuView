//! Hardware Normalizer — ADR-027 MERIDIAN Phase 1
//!
//! Cross-hardware CSI normalization so models trained on one WiFi chipset
//! generalize to others. The normalizer detects hardware from subcarrier
//! count, resamples to a canonical grid (default 56) via Catmull-Rom cubic
//! interpolation, z-score normalizes amplitude, and sanitizes phase
//! (unwrap + linear-trend removal).

use std::collections::HashMap;
use std::f64::consts::PI;
use thiserror::Error;

/// Errors from hardware normalization.
#[derive(Debug, Error)]
pub enum HardwareNormError {
    #[error("Empty CSI frame (amplitude len={amp}, phase len={phase})")]
    EmptyFrame { amp: usize, phase: usize },
    #[error("Amplitude/phase length mismatch ({amp} vs {phase})")]
    LengthMismatch { amp: usize, phase: usize },
    #[error("Unknown hardware for subcarrier count {0}")]
    UnknownHardware(usize),
    #[error("Invalid canonical subcarrier count: {0}")]
    InvalidCanonical(usize),
}

/// Known WiFi chipset families with their subcarrier counts and MIMO configs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HardwareType {
    /// ESP32-S3 with LWIP CSI: 64 subcarriers, 1x1 SISO
    Esp32S3,
    /// Intel 5300 NIC: 30 subcarriers, up to 3x3 MIMO
    Intel5300,
    /// Atheros (ath9k/ath10k): 56 subcarriers, up to 3x3 MIMO
    Atheros,
    /// Generic / unknown hardware
    Generic,
}

impl HardwareType {
    /// Expected subcarrier count for this hardware.
    pub fn subcarrier_count(&self) -> usize {
        match self {
            Self::Esp32S3 => 64,
            Self::Intel5300 => 30,
            Self::Atheros => 56,
            Self::Generic => 56,
        }
    }

    /// Maximum MIMO spatial streams.
    pub fn mimo_streams(&self) -> usize {
        match self {
            Self::Esp32S3 => 1,
            Self::Intel5300 => 3,
            Self::Atheros => 3,
            Self::Generic => 1,
        }
    }
}

/// Per-hardware amplitude statistics for z-score normalization.
#[derive(Debug, Clone)]
pub struct AmplitudeStats {
    pub mean: f64,
    pub std: f64,
}

impl Default for AmplitudeStats {
    fn default() -> Self {
        Self { mean: 0.0, std: 1.0 }
    }
}

/// A CSI frame normalized to a canonical representation.
#[derive(Debug, Clone)]
pub struct CanonicalCsiFrame {
    /// Z-score normalized amplitude (length = canonical_subcarriers).
    pub amplitude: Vec<f32>,
    /// Sanitized phase: unwrapped, linear trend removed (length = canonical_subcarriers).
    pub phase: Vec<f32>,
    /// Hardware type that produced the original frame.
    pub hardware_type: HardwareType,
}

/// Normalizes CSI frames from heterogeneous hardware into a canonical form.
#[derive(Debug)]
pub struct HardwareNormalizer {
    canonical_subcarriers: usize,
    hw_stats: HashMap<HardwareType, AmplitudeStats>,
}

impl HardwareNormalizer {
    /// Create a normalizer with default canonical subcarrier count (56).
    pub fn new() -> Self {
        Self { canonical_subcarriers: 56, hw_stats: HashMap::new() }
    }

    /// Create a normalizer with a custom canonical subcarrier count.
    pub fn with_canonical_subcarriers(count: usize) -> Result<Self, HardwareNormError> {
        if count == 0 {
            return Err(HardwareNormError::InvalidCanonical(count));
        }
        Ok(Self { canonical_subcarriers: count, hw_stats: HashMap::new() })
    }

    /// Register amplitude statistics for a specific hardware type.
    pub fn set_hw_stats(&mut self, hw: HardwareType, stats: AmplitudeStats) {
        self.hw_stats.insert(hw, stats);
    }

    /// Return the canonical subcarrier count.
    pub fn canonical_subcarriers(&self) -> usize {
        self.canonical_subcarriers
    }

    /// Detect hardware type from subcarrier count.
    pub fn detect_hardware(subcarrier_count: usize) -> HardwareType {
        match subcarrier_count {
            64 => HardwareType::Esp32S3,
            30 => HardwareType::Intel5300,
            56 => HardwareType::Atheros,
            _ => HardwareType::Generic,
        }
    }

    /// Normalize a raw CSI frame into canonical form.
    ///
    /// 1. Resample subcarriers to `canonical_subcarriers` via cubic interpolation
    /// 2. Z-score normalize amplitude (mean=0, std=1)
    /// 3. Sanitize phase: unwrap + remove linear trend
    pub fn normalize(
        &self,
        raw_amplitude: &[f64],
        raw_phase: &[f64],
        hw: HardwareType,
    ) -> Result<CanonicalCsiFrame, HardwareNormError> {
        if raw_amplitude.is_empty() || raw_phase.is_empty() {
            return Err(HardwareNormError::EmptyFrame {
                amp: raw_amplitude.len(),
                phase: raw_phase.len(),
            });
        }
        if raw_amplitude.len() != raw_phase.len() {
            return Err(HardwareNormError::LengthMismatch {
                amp: raw_amplitude.len(),
                phase: raw_phase.len(),
            });
        }

        let amp_resampled = resample_cubic(raw_amplitude, self.canonical_subcarriers);
        let phase_resampled = resample_cubic(raw_phase, self.canonical_subcarriers);
        let amp_normalized = zscore_normalize(&amp_resampled, self.hw_stats.get(&hw));
        let phase_sanitized = sanitize_phase(&phase_resampled);

        Ok(CanonicalCsiFrame {
            amplitude: amp_normalized.iter().map(|&v| v as f32).collect(),
            phase: phase_sanitized.iter().map(|&v| v as f32).collect(),
            hardware_type: hw,
        })
    }
}

impl Default for HardwareNormalizer {
    fn default() -> Self { Self::new() }
}

/// Resample a 1-D signal to `dst_len` using Catmull-Rom cubic interpolation.
/// Identity passthrough when `src.len() == dst_len`.
fn resample_cubic(src: &[f64], dst_len: usize) -> Vec<f64> {
    let n = src.len();
    if n == dst_len { return src.to_vec(); }
    if n == 0 || dst_len == 0 { return vec![0.0; dst_len]; }
    if n == 1 { return vec![src[0]; dst_len]; }

    let ratio = (n - 1) as f64 / (dst_len - 1).max(1) as f64;
    (0..dst_len)
        .map(|i| {
            let x = i as f64 * ratio;
            let idx = x.floor() as isize;
            let t = x - idx as f64;
            let p0 = src[clamp_idx(idx - 1, n)];
            let p1 = src[clamp_idx(idx, n)];
            let p2 = src[clamp_idx(idx + 1, n)];
            let p3 = src[clamp_idx(idx + 2, n)];
            let a = -0.5 * p0 + 1.5 * p1 - 1.5 * p2 + 0.5 * p3;
            let b = p0 - 2.5 * p1 + 2.0 * p2 - 0.5 * p3;
            let c = -0.5 * p0 + 0.5 * p2;
            a * t * t * t + b * t * t + c * t + p1
        })
        .collect()
}

fn clamp_idx(idx: isize, len: usize) -> usize {
    idx.max(0).min(len as isize - 1) as usize
}

/// Z-score normalize to mean=0, std=1. Uses per-hardware stats if available.
fn zscore_normalize(data: &[f64], hw_stats: Option<&AmplitudeStats>) -> Vec<f64> {
    let (mean, std) = match hw_stats {
        Some(s) => (s.mean, s.std),
        None => compute_mean_std(data),
    };
    let safe_std = if std.abs() < 1e-12 { 1.0 } else { std };
    data.iter().map(|&v| (v - mean) / safe_std).collect()
}

fn compute_mean_std(data: &[f64]) -> (f64, f64) {
    let n = data.len() as f64;
    if n < 1.0 { return (0.0, 1.0); }
    let mean = data.iter().sum::<f64>() / n;
    if n < 2.0 { return (mean, 1.0); }
    let var = data.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, var.sqrt())
}

/// Sanitize phase: unwrap 2-pi discontinuities then remove linear trend.
/// Mirrors `PhaseSanitizer::unwrap_1d` logic, adds least-squares detrend.
fn sanitize_phase(phase: &[f64]) -> Vec<f64> {
    if phase.is_empty() { return Vec::new(); }

    // Unwrap
    let mut uw = phase.to_vec();
    let mut correction = 0.0;
    let mut prev = uw[0];
    for i in 1..uw.len() {
        let diff = phase[i] - prev;
        if diff > PI { correction -= 2.0 * PI; }
        else if diff < -PI { correction += 2.0 * PI; }
        uw[i] = phase[i] + correction;
        prev = phase[i];
    }

    // Remove linear trend: y = slope*x + intercept
    let n = uw.len() as f64;
    let xm = (n - 1.0) / 2.0;
    let ym = uw.iter().sum::<f64>() / n;
    let (mut num, mut den) = (0.0, 0.0);
    for (i, &y) in uw.iter().enumerate() {
        let dx = i as f64 - xm;
        num += dx * (y - ym);
        den += dx * dx;
    }
    let slope = if den.abs() > 1e-12 { num / den } else { 0.0 };
    let intercept = ym - slope * xm;
    uw.iter().enumerate().map(|(i, &y)| y - (slope * i as f64 + intercept)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_hardware_and_properties() {
        assert_eq!(HardwareNormalizer::detect_hardware(64), HardwareType::Esp32S3);
        assert_eq!(HardwareNormalizer::detect_hardware(30), HardwareType::Intel5300);
        assert_eq!(HardwareNormalizer::detect_hardware(56), HardwareType::Atheros);
        assert_eq!(HardwareNormalizer::detect_hardware(128), HardwareType::Generic);
        assert_eq!(HardwareType::Esp32S3.subcarrier_count(), 64);
        assert_eq!(HardwareType::Esp32S3.mimo_streams(), 1);
        assert_eq!(HardwareType::Intel5300.subcarrier_count(), 30);
        assert_eq!(HardwareType::Intel5300.mimo_streams(), 3);
        assert_eq!(HardwareType::Atheros.subcarrier_count(), 56);
        assert_eq!(HardwareType::Atheros.mimo_streams(), 3);
        assert_eq!(HardwareType::Generic.subcarrier_count(), 56);
        assert_eq!(HardwareType::Generic.mimo_streams(), 1);
    }

    #[test]
    fn resample_identity_56_to_56() {
        let input: Vec<f64> = (0..56).map(|i| i as f64 * 0.1).collect();
        let output = resample_cubic(&input, 56);
        for (a, b) in input.iter().zip(output.iter()) {
            assert!((a - b).abs() < 1e-12, "Identity resampling must be passthrough");
        }
    }

    #[test]
    fn resample_64_to_56() {
        let input: Vec<f64> = (0..64).map(|i| (i as f64 * 0.1).sin()).collect();
        let out = resample_cubic(&input, 56);
        assert_eq!(out.len(), 56);
        assert!((out[0] - input[0]).abs() < 1e-6);
        assert!((out[55] - input[63]).abs() < 0.1);
    }

    #[test]
    fn resample_30_to_56() {
        let input: Vec<f64> = (0..30).map(|i| (i as f64 * 0.2).cos()).collect();
        let out = resample_cubic(&input, 56);
        assert_eq!(out.len(), 56);
        assert!((out[0] - input[0]).abs() < 1e-6);
        assert!((out[55] - input[29]).abs() < 0.1);
    }

    #[test]
    fn resample_preserves_constant() {
        for &v in &resample_cubic(&vec![3.14; 64], 56) {
            assert!((v - 3.14).abs() < 1e-10);
        }
    }

    #[test]
    fn zscore_produces_zero_mean_unit_std() {
        let data: Vec<f64> = (0..100).map(|i| 50.0 + 10.0 * (i as f64 * 0.1).sin()).collect();
        let z = zscore_normalize(&data, None);
        let n = z.len() as f64;
        let mean = z.iter().sum::<f64>() / n;
        let std = (z.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)).sqrt();
        assert!(mean.abs() < 1e-10, "Mean should be ~0, got {mean}");
        assert!((std - 1.0).abs() < 1e-10, "Std should be ~1, got {std}");
    }

    #[test]
    fn zscore_with_hw_stats_and_constant() {
        let z = zscore_normalize(&[10.0, 20.0, 30.0], Some(&AmplitudeStats { mean: 20.0, std: 10.0 }));
        assert!((z[0] + 1.0).abs() < 1e-12);
        assert!(z[1].abs() < 1e-12);
        assert!((z[2] - 1.0).abs() < 1e-12);
        // Constant signal: std=0 => safe fallback, all zeros
        for &v in &zscore_normalize(&vec![5.0; 50], None) { assert!(v.abs() < 1e-12); }
    }

    #[test]
    fn phase_sanitize_removes_linear_trend() {
        let san = sanitize_phase(&(0..56).map(|i| 0.5 * i as f64).collect::<Vec<_>>());
        assert_eq!(san.len(), 56);
        for &v in &san { assert!(v.abs() < 1e-10, "Detrended should be ~0, got {v}"); }
    }

    #[test]
    fn phase_sanitize_unwrap() {
        let raw: Vec<f64> = (0..40).map(|i| {
            let mut w = (i as f64 * 0.4) % (2.0 * PI);
            if w > PI { w -= 2.0 * PI; }
            w
        }).collect();
        let san = sanitize_phase(&raw);
        for i in 1..san.len() {
            assert!((san[i] - san[i - 1]).abs() < 1.0, "Phase jump at {i}");
        }
    }

    #[test]
    fn phase_sanitize_edge_cases() {
        assert!(sanitize_phase(&[]).is_empty());
        assert!(sanitize_phase(&[1.5])[0].abs() < 1e-12);
    }

    #[test]
    fn normalize_esp32_64_to_56() {
        let norm = HardwareNormalizer::new();
        let amp: Vec<f64> = (0..64).map(|i| 20.0 + 5.0 * (i as f64 * 0.1).sin()).collect();
        let ph: Vec<f64> = (0..64).map(|i| (i as f64 * 0.05).sin() * 0.5).collect();
        let r = norm.normalize(&amp, &ph, HardwareType::Esp32S3).unwrap();
        assert_eq!(r.amplitude.len(), 56);
        assert_eq!(r.phase.len(), 56);
        assert_eq!(r.hardware_type, HardwareType::Esp32S3);
        let mean: f64 = r.amplitude.iter().map(|&v| v as f64).sum::<f64>() / 56.0;
        assert!(mean.abs() < 0.1, "Mean should be ~0, got {mean}");
    }

    #[test]
    fn normalize_intel5300_30_to_56() {
        let r = HardwareNormalizer::new().normalize(
            &(0..30).map(|i| 15.0 + 3.0 * (i as f64 * 0.2).cos()).collect::<Vec<_>>(),
            &(0..30).map(|i| (i as f64 * 0.1).sin() * 0.3).collect::<Vec<_>>(),
            HardwareType::Intel5300,
        ).unwrap();
        assert_eq!(r.amplitude.len(), 56);
        assert_eq!(r.hardware_type, HardwareType::Intel5300);
    }

    #[test]
    fn normalize_atheros_passthrough_count() {
        let r = HardwareNormalizer::new().normalize(
            &(0..56).map(|i| 10.0 + 2.0 * i as f64).collect::<Vec<_>>(),
            &(0..56).map(|i| (i as f64 * 0.05).sin()).collect::<Vec<_>>(),
            HardwareType::Atheros,
        ).unwrap();
        assert_eq!(r.amplitude.len(), 56);
    }

    #[test]
    fn normalize_errors_and_custom_canonical() {
        let n = HardwareNormalizer::new();
        assert!(n.normalize(&[], &[], HardwareType::Generic).is_err());
        assert!(matches!(n.normalize(&[1.0, 2.0], &[1.0], HardwareType::Generic),
            Err(HardwareNormError::LengthMismatch { .. })));
        assert!(matches!(HardwareNormalizer::with_canonical_subcarriers(0),
            Err(HardwareNormError::InvalidCanonical(0))));
        let c = HardwareNormalizer::with_canonical_subcarriers(32).unwrap();
        let r = c.normalize(
            &(0..64).map(|i| i as f64).collect::<Vec<_>>(),
            &(0..64).map(|i| (i as f64 * 0.1).sin()).collect::<Vec<_>>(),
            HardwareType::Esp32S3,
        ).unwrap();
        assert_eq!(r.amplitude.len(), 32);
    }
}
