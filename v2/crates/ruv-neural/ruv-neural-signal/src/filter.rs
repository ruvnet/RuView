//! Digital filters for neural signal processing.
//!
//! Implements Butterworth IIR filters in second-order sections (SOS) form
//! for numerical stability. Supports bandpass, notch (band-reject),
//! highpass, and lowpass configurations.
//!
//! All filters implement the [`SignalProcessor`] trait for uniform usage.

use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// Trait for signal processing operations.
pub trait SignalProcessor {
    /// Apply the processor to a signal, returning the filtered output.
    fn process(&self, signal: &[f64]) -> Vec<f64>;
}

/// A single second-order section (biquad) with coefficients.
///
/// Transfer function: H(z) = (b0 + b1*z^-1 + b2*z^-2) / (1 + a1*z^-1 + a2*z^-2)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecondOrderSection {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a1: f64,
    pub a2: f64,
}

impl SecondOrderSection {
    /// Apply this biquad section to a signal using Direct Form II Transposed.
    fn apply(&self, signal: &[f64]) -> Vec<f64> {
        let n = signal.len();
        let mut output = vec![0.0; n];
        let mut w1 = 0.0;
        let mut w2 = 0.0;

        for i in 0..n {
            let x = signal[i];
            let y = self.b0 * x + w1;
            w1 = self.b1 * x - self.a1 * y + w2;
            w2 = self.b2 * x - self.a2 * y;
            output[i] = y;
        }

        output
    }
}

/// Apply a cascade of second-order sections to a signal (forward-backward
/// for zero-phase filtering).
fn apply_sos_filtfilt(sections: &[SecondOrderSection], signal: &[f64]) -> Vec<f64> {
    if signal.is_empty() {
        return Vec::new();
    }

    // Forward pass through all sections
    let mut result = signal.to_vec();
    for sos in sections {
        result = sos.apply(&result);
    }

    // Reverse
    result.reverse();

    // Backward pass through all sections
    for sos in sections {
        result = sos.apply(&result);
    }

    // Reverse back to original order
    result.reverse();

    result
}

/// Design Butterworth analog prototype poles for a given order.
/// Returns poles on the unit circle in the left half of the s-plane.
fn butterworth_poles(order: usize) -> Vec<(f64, f64)> {
    let mut poles = Vec::new();
    for k in 0..order {
        let theta = PI * (2 * k + order + 1) as f64 / (2 * order) as f64;
        poles.push((theta.cos(), theta.sin()));
    }
    poles
}

/// Prewarp a frequency from digital to analog domain.
fn prewarp(freq_hz: f64, sample_rate: f64) -> f64 {
    2.0 * sample_rate * (PI * freq_hz / sample_rate).tan()
}

/// Design a lowpass second-order section from analog prototype poles
/// using the bilinear transform.
fn design_lowpass_sos(pole_re: f64, pole_im: f64, wc: f64, fs: f64) -> SecondOrderSection {
    let t = 1.0 / (2.0 * fs);

    if pole_im.abs() < 1e-14 {
        // Real pole -> embed in SOS with b2=0, a2=0
        let s_re = wc * pole_re;
        let d = 1.0 - s_re * t;
        let n = -(s_re * t);
        SecondOrderSection {
            b0: n / d,
            b1: n / d,
            b2: 0.0,
            a1: -(1.0 + s_re * t) / d,
            a2: 0.0,
        }
    } else {
        // Complex conjugate pair
        let s_re = wc * pole_re;
        let s_im = wc * pole_im;
        let denom = (1.0 - s_re * t).powi(2) + (s_im * t).powi(2);
        let a1 = 2.0 * ((s_re * t).powi(2) + (s_im * t).powi(2) - 1.0) / denom;
        let a2 = ((1.0 + s_re * t).powi(2) + (s_im * t).powi(2)) / denom;
        let num_gain = (wc * t).powi(2) / denom;
        SecondOrderSection {
            b0: num_gain,
            b1: 2.0 * num_gain,
            b2: num_gain,
            a1,
            a2,
        }
    }
}

/// Design a highpass second-order section from analog prototype poles.
fn design_highpass_sos(pole_re: f64, pole_im: f64, wc: f64, fs: f64) -> SecondOrderSection {
    let t = 1.0 / (2.0 * fs);

    if pole_im.abs() < 1e-14 {
        // Real pole
        let alpha = wc / (-pole_re);
        let d = 1.0 + alpha * t;
        SecondOrderSection {
            b0: 1.0 / d,
            b1: -1.0 / d,
            b2: 0.0,
            a1: -(1.0 - alpha * t) / d,
            a2: 0.0,
        }
    } else {
        // Complex conjugate pair: HP transform s -> wc/s
        let mag_sq = pole_re.powi(2) + pole_im.powi(2);
        let hp_re = wc * pole_re / mag_sq;
        let hp_im = -wc * pole_im / mag_sq;

        let denom = (1.0 - hp_re * t).powi(2) + (hp_im * t).powi(2);
        let a1 = 2.0 * ((hp_re * t).powi(2) + (hp_im * t).powi(2) - 1.0) / denom;
        let a2 = ((1.0 + hp_re * t).powi(2) + (hp_im * t).powi(2)) / denom;
        let num_gain = 1.0 / denom;
        SecondOrderSection {
            b0: num_gain,
            b1: -2.0 * num_gain,
            b2: num_gain,
            a1,
            a2,
        }
    }
}

/// Design Butterworth lowpass filter as cascade of second-order sections.
fn design_butterworth_lowpass(order: usize, cutoff_hz: f64, sample_rate: f64) -> Vec<SecondOrderSection> {
    let wc = prewarp(cutoff_hz, sample_rate);
    let poles = butterworth_poles(order);
    let mut sections = Vec::new();

    let mut i = 0;
    while i < poles.len() {
        if poles[i].1.abs() < 1e-14 {
            sections.push(design_lowpass_sos(poles[i].0, 0.0, wc, sample_rate));
            i += 1;
        } else {
            sections.push(design_lowpass_sos(poles[i].0, poles[i].1, wc, sample_rate));
            i += 2;
        }
    }

    sections
}

/// Design Butterworth highpass filter as cascade of second-order sections.
fn design_butterworth_highpass(order: usize, cutoff_hz: f64, sample_rate: f64) -> Vec<SecondOrderSection> {
    let wc = prewarp(cutoff_hz, sample_rate);
    let poles = butterworth_poles(order);
    let mut sections = Vec::new();

    let mut i = 0;
    while i < poles.len() {
        if poles[i].1.abs() < 1e-14 {
            sections.push(design_highpass_sos(poles[i].0, 0.0, wc, sample_rate));
            i += 1;
        } else {
            sections.push(design_highpass_sos(poles[i].0, poles[i].1, wc, sample_rate));
            i += 2;
        }
    }

    sections
}

/// Butterworth IIR bandpass filter using cascaded second-order sections.
///
/// Applies a zero-phase (forward-backward) filter for no phase distortion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandpassFilter {
    /// Filter order (per lowpass/highpass stage).
    pub order: usize,
    /// Lower cutoff frequency in Hz.
    pub low_hz: f64,
    /// Upper cutoff frequency in Hz.
    pub high_hz: f64,
    /// Sampling rate in Hz.
    pub sample_rate: f64,
    /// Highpass SOS sections (for low_hz cutoff).
    hp_sections: Vec<SecondOrderSection>,
    /// Lowpass SOS sections (for high_hz cutoff).
    lp_sections: Vec<SecondOrderSection>,
}

impl BandpassFilter {
    /// Create a new Butterworth bandpass filter.
    ///
    /// # Arguments
    /// * `order` - Filter order (typically 2-6)
    /// * `low_hz` - Lower cutoff frequency in Hz
    /// * `high_hz` - Upper cutoff frequency in Hz
    /// * `sample_rate` - Sampling rate in Hz
    pub fn new(order: usize, low_hz: f64, high_hz: f64, sample_rate: f64) -> Self {
        let hp_sections = design_butterworth_highpass(order, low_hz, sample_rate);
        let lp_sections = design_butterworth_lowpass(order, high_hz, sample_rate);
        Self {
            order,
            low_hz,
            high_hz,
            sample_rate,
            hp_sections,
            lp_sections,
        }
    }

    /// Apply the bandpass filter to a signal.
    pub fn apply(&self, signal: &[f64]) -> Vec<f64> {
        let hp_out = apply_sos_filtfilt(&self.hp_sections, signal);
        apply_sos_filtfilt(&self.lp_sections, &hp_out)
    }
}

impl SignalProcessor for BandpassFilter {
    fn process(&self, signal: &[f64]) -> Vec<f64> {
        self.apply(signal)
    }
}

/// Notch (band-reject) filter for removing line noise (50/60 Hz).
///
/// Implements a second-order IIR notch filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotchFilter {
    /// Center frequency to reject in Hz.
    pub center_hz: f64,
    /// Rejection bandwidth in Hz.
    pub bandwidth_hz: f64,
    /// Sampling rate in Hz.
    pub sample_rate: f64,
    /// The notch filter section.
    section: SecondOrderSection,
}

impl NotchFilter {
    /// Create a new notch filter.
    ///
    /// # Arguments
    /// * `center_hz` - Center frequency to reject (e.g., 50.0 or 60.0)
    /// * `bandwidth_hz` - Width of the rejection band in Hz (e.g., 2.0)
    /// * `sample_rate` - Sampling rate in Hz
    pub fn new(center_hz: f64, bandwidth_hz: f64, sample_rate: f64) -> Self {
        let w0 = 2.0 * PI * center_hz / sample_rate;
        let bw = 2.0 * PI * bandwidth_hz / sample_rate;
        let q = w0.sin() / bw;
        let alpha = w0.sin() / (2.0 * q);

        let a0 = 1.0 + alpha;
        let section = SecondOrderSection {
            b0: 1.0 / a0,
            b1: -2.0 * w0.cos() / a0,
            b2: 1.0 / a0,
            a1: -2.0 * w0.cos() / a0,
            a2: (1.0 - alpha) / a0,
        };

        Self {
            center_hz,
            bandwidth_hz,
            sample_rate,
            section,
        }
    }

    /// Apply the notch filter to a signal (zero-phase).
    pub fn apply(&self, signal: &[f64]) -> Vec<f64> {
        apply_sos_filtfilt(&[self.section.clone()], signal)
    }
}

impl SignalProcessor for NotchFilter {
    fn process(&self, signal: &[f64]) -> Vec<f64> {
        self.apply(signal)
    }
}

/// Butterworth highpass filter using second-order sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighpassFilter {
    /// Filter order.
    pub order: usize,
    /// Cutoff frequency in Hz.
    pub cutoff_hz: f64,
    /// Sampling rate in Hz.
    pub sample_rate: f64,
    /// SOS sections.
    sections: Vec<SecondOrderSection>,
}

impl HighpassFilter {
    /// Create a new Butterworth highpass filter.
    pub fn new(order: usize, cutoff_hz: f64, sample_rate: f64) -> Self {
        let sections = design_butterworth_highpass(order, cutoff_hz, sample_rate);
        Self {
            order,
            cutoff_hz,
            sample_rate,
            sections,
        }
    }

    /// Apply the highpass filter to a signal (zero-phase).
    pub fn apply(&self, signal: &[f64]) -> Vec<f64> {
        apply_sos_filtfilt(&self.sections, signal)
    }
}

impl SignalProcessor for HighpassFilter {
    fn process(&self, signal: &[f64]) -> Vec<f64> {
        self.apply(signal)
    }
}

/// Butterworth lowpass filter using second-order sections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LowpassFilter {
    /// Filter order.
    pub order: usize,
    /// Cutoff frequency in Hz.
    pub cutoff_hz: f64,
    /// Sampling rate in Hz.
    pub sample_rate: f64,
    /// SOS sections.
    sections: Vec<SecondOrderSection>,
}

impl LowpassFilter {
    /// Create a new Butterworth lowpass filter.
    pub fn new(order: usize, cutoff_hz: f64, sample_rate: f64) -> Self {
        let sections = design_butterworth_lowpass(order, cutoff_hz, sample_rate);
        Self {
            order,
            cutoff_hz,
            sample_rate,
            sections,
        }
    }

    /// Apply the lowpass filter to a signal (zero-phase).
    pub fn apply(&self, signal: &[f64]) -> Vec<f64> {
        apply_sos_filtfilt(&self.sections, signal)
    }
}

impl SignalProcessor for LowpassFilter {
    fn process(&self, signal: &[f64]) -> Vec<f64> {
        self.apply(signal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    fn sine_wave(freq_hz: f64, sample_rate: f64, duration_s: f64) -> Vec<f64> {
        let n = (sample_rate * duration_s) as usize;
        (0..n)
            .map(|i| {
                let t = i as f64 / sample_rate;
                (2.0 * PI * freq_hz * t).sin()
            })
            .collect()
    }

    fn rms(signal: &[f64]) -> f64 {
        let sum_sq: f64 = signal.iter().map(|x| x * x).sum();
        (sum_sq / signal.len() as f64).sqrt()
    }

    #[test]
    fn bandpass_passes_correct_frequency() {
        let sr = 1000.0;
        let dur = 2.0;
        let in_band = sine_wave(20.0, sr, dur);
        let out_band = sine_wave(200.0, sr, dur);
        let signal: Vec<f64> = in_band.iter().zip(&out_band).map(|(a, b)| a + b).collect();

        let filter = BandpassFilter::new(4, 10.0, 50.0, sr);
        let filtered = filter.apply(&signal);

        let in_rms = rms(&in_band);
        let filtered_rms = rms(&filtered[200..filtered.len() - 200]);

        assert!(
            (filtered_rms - in_rms).abs() / in_rms < 0.3,
            "Bandpass should preserve in-band signal: filtered_rms={filtered_rms}, in_rms={in_rms}"
        );
    }

    #[test]
    fn bandpass_rejects_out_of_band() {
        let sr = 1000.0;
        let dur = 2.0;
        let signal = sine_wave(200.0, sr, dur);

        let filter = BandpassFilter::new(4, 10.0, 50.0, sr);
        let filtered = filter.apply(&signal);

        let orig_rms = rms(&signal);
        let filtered_rms = rms(&filtered[200..filtered.len() - 200]);

        assert!(
            filtered_rms / orig_rms < 0.1,
            "Bandpass should reject out-of-band: ratio={}",
            filtered_rms / orig_rms
        );
    }

    #[test]
    fn notch_removes_target_frequency() {
        let sr = 1000.0;
        let dur = 2.0;
        let keep = sine_wave(10.0, sr, dur);
        let remove = sine_wave(50.0, sr, dur);
        let signal: Vec<f64> = keep.iter().zip(&remove).map(|(a, b)| a + b).collect();

        let filter = NotchFilter::new(50.0, 2.0, sr);
        let filtered = filter.apply(&signal);

        let keep_rms = rms(&keep);
        let filtered_rms = rms(&filtered[200..filtered.len() - 200]);

        assert!(
            (filtered_rms - keep_rms).abs() / keep_rms < 0.3,
            "Notch should preserve nearby: filtered_rms={filtered_rms}, keep_rms={keep_rms}"
        );
    }

    #[test]
    fn lowpass_passes_low_frequency() {
        let sr = 1000.0;
        let dur = 2.0;
        let low = sine_wave(5.0, sr, dur);
        let high = sine_wave(100.0, sr, dur);
        let signal: Vec<f64> = low.iter().zip(&high).map(|(a, b)| a + b).collect();

        let filter = LowpassFilter::new(4, 20.0, sr);
        let filtered = filter.apply(&signal);

        let low_rms = rms(&low);
        let filtered_rms = rms(&filtered[200..filtered.len() - 200]);

        assert!(
            (filtered_rms - low_rms).abs() / low_rms < 0.3,
            "Lowpass should preserve low freq"
        );
    }

    #[test]
    fn highpass_passes_high_frequency() {
        let sr = 1000.0;
        let dur = 2.0;
        let low = sine_wave(1.0, sr, dur);
        let high = sine_wave(50.0, sr, dur);
        let signal: Vec<f64> = low.iter().zip(&high).map(|(a, b)| a + b).collect();

        let filter = HighpassFilter::new(4, 10.0, sr);
        let filtered = filter.apply(&signal);

        let high_rms = rms(&high);
        let filtered_rms = rms(&filtered[200..filtered.len() - 200]);

        assert!(
            (filtered_rms - high_rms).abs() / high_rms < 0.3,
            "Highpass should preserve high freq"
        );
    }

    #[test]
    fn empty_signal_returns_empty() {
        let filter = BandpassFilter::new(2, 1.0, 50.0, 1000.0);
        assert!(filter.apply(&[]).is_empty());
    }
}
