//! Vital sign detection from WiFi CSI data.
//!
//! Implements breathing rate (0.1-0.5 Hz) and heart rate (0.8-2.0 Hz)
//! estimation using FFT-based spectral analysis on CSI amplitude and phase
//! time series. Designed per ADR-021 (rvdna vital sign pipeline).
//!
//! All math is pure Rust -- no external FFT crate required. Uses a radix-2
//! DIT FFT for buffers zero-padded to power-of-two length. A windowed-sinc
//! FIR bandpass filter isolates the frequency bands of interest before
//! spectral analysis.

use std::collections::VecDeque;
use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

// ── Configuration constants ────────────────────────────────────────────────

/// Breathing rate physiological band: 6-30 breaths per minute.
const BREATHING_MIN_HZ: f64 = 0.1; // 6 BPM
const BREATHING_MAX_HZ: f64 = 0.5; // 30 BPM

/// Heart rate physiological band: 40-120 beats per minute.
const HEARTBEAT_MIN_HZ: f64 = 0.667; // 40 BPM
const HEARTBEAT_MAX_HZ: f64 = 2.0; // 120 BPM

/// Minimum number of samples before attempting extraction.
const MIN_BREATHING_SAMPLES: usize = 40; // ~2s at 20 Hz
const MIN_HEARTBEAT_SAMPLES: usize = 30; // ~1.5s at 20 Hz

/// Peak-to-mean ratio threshold for confident detection.
const CONFIDENCE_THRESHOLD: f64 = 2.0;

// ── Output types ───────────────────────────────────────────────────────────

/// Vital sign readings produced each frame.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalSigns {
    /// Estimated breathing rate in breaths per minute, if detected.
    pub breathing_rate_bpm: Option<f64>,
    /// Estimated heart rate in beats per minute, if detected.
    pub heart_rate_bpm: Option<f64>,
    /// Confidence of breathing estimate (0.0 - 1.0).
    pub breathing_confidence: f64,
    /// Confidence of heartbeat estimate (0.0 - 1.0).
    pub heartbeat_confidence: f64,
    /// Overall signal quality metric (0.0 - 1.0).
    pub signal_quality: f64,
}

impl Default for VitalSigns {
    fn default() -> Self {
        Self {
            breathing_rate_bpm: None,
            heart_rate_bpm: None,
            breathing_confidence: 0.0,
            heartbeat_confidence: 0.0,
            signal_quality: 0.0,
        }
    }
}

// ── Detector ───────────────────────────────────────────────────────────────

/// Stateful vital sign detector. Maintains rolling buffers of CSI amplitude
/// data and extracts breathing and heart rate via spectral analysis.
#[allow(dead_code)]
pub struct VitalSignDetector {
    /// Rolling buffer of mean-amplitude samples for breathing detection.
    breathing_buffer: VecDeque<f64>,
    /// Rolling buffer of phase-variance samples for heartbeat detection.
    heartbeat_buffer: VecDeque<f64>,
    /// CSI frame arrival rate in Hz.
    sample_rate: f64,
    /// Window duration for breathing FFT in seconds.
    breathing_window_secs: f64,
    /// Window duration for heartbeat FFT in seconds.
    heartbeat_window_secs: f64,
    /// Maximum breathing buffer capacity (samples).
    breathing_capacity: usize,
    /// Maximum heartbeat buffer capacity (samples).
    heartbeat_capacity: usize,
    /// Running frame count for signal quality estimation.
    frame_count: u64,
}

impl VitalSignDetector {
    /// Create a new detector with the given CSI sample rate (Hz).
    ///
    /// Typical sample rates:
    /// - ESP32 CSI: 20-100 Hz
    /// - Windows WiFi RSSI: 2 Hz (insufficient for heartbeat)
    /// - Simulation: 2-20 Hz
    pub fn new(sample_rate: f64) -> Self {
        let breathing_window_secs = 30.0;
        let heartbeat_window_secs = 15.0;
        let breathing_capacity = (sample_rate * breathing_window_secs) as usize;
        let heartbeat_capacity = (sample_rate * heartbeat_window_secs) as usize;

        Self {
            breathing_buffer: VecDeque::with_capacity(breathing_capacity.max(1)),
            heartbeat_buffer: VecDeque::with_capacity(heartbeat_capacity.max(1)),
            sample_rate,
            breathing_window_secs,
            heartbeat_window_secs,
            breathing_capacity: breathing_capacity.max(1),
            heartbeat_capacity: heartbeat_capacity.max(1),
            frame_count: 0,
        }
    }

    /// Process one CSI frame and return updated vital signs.
    ///
    /// `amplitude` - per-subcarrier amplitude values for this frame.
    /// `phase` - per-subcarrier phase values for this frame.
    ///
    /// The detector extracts two aggregate features per frame:
    /// 1. Mean amplitude (breathing signal -- chest movement modulates path loss)
    /// 2. Phase variance across subcarriers (heartbeat signal -- subtle phase shifts)
    pub fn process_frame(&mut self, amplitude: &[f64], phase: &[f64]) -> VitalSigns {
        self.frame_count += 1;

        if amplitude.is_empty() {
            return VitalSigns::default();
        }

        // -- Feature 1: Mean amplitude for breathing detection --
        // Respiratory chest displacement (1-5 mm) modulates CSI amplitudes
        // across all subcarriers. Mean amplitude captures this well.
        let n = amplitude.len() as f64;
        let mean_amp: f64 = amplitude.iter().sum::<f64>() / n;

        self.breathing_buffer.push_back(mean_amp);
        while self.breathing_buffer.len() > self.breathing_capacity {
            self.breathing_buffer.pop_front();
        }

        // -- Feature 2: Phase variance for heartbeat detection --
        // Cardiac-induced body surface displacement is < 0.5 mm, producing
        // tiny phase changes. Cross-subcarrier phase variance captures this
        // more sensitively than amplitude alone.
        let phase_var = if phase.len() > 1 {
            let mean_phase: f64 = phase.iter().sum::<f64>() / phase.len() as f64;
            phase
                .iter()
                .map(|p| (p - mean_phase).powi(2))
                .sum::<f64>()
                / phase.len() as f64
        } else {
            // Fallback: use amplitude high-pass residual when phase is unavailable
            let half = amplitude.len() / 2;
            if half > 0 {
                let hi_mean: f64 =
                    amplitude[half..].iter().sum::<f64>() / (amplitude.len() - half) as f64;
                amplitude[half..]
                    .iter()
                    .map(|a| (a - hi_mean).powi(2))
                    .sum::<f64>()
                    / (amplitude.len() - half) as f64
            } else {
                0.0
            }
        };

        self.heartbeat_buffer.push_back(phase_var);
        while self.heartbeat_buffer.len() > self.heartbeat_capacity {
            self.heartbeat_buffer.pop_front();
        }

        // -- Extract vital signs --
        let (breathing_rate, breathing_confidence) = self.extract_breathing();
        let (heart_rate, heartbeat_confidence) = self.extract_heartbeat();

        // -- Signal quality --
        let signal_quality = self.compute_signal_quality(amplitude);

        VitalSigns {
            breathing_rate_bpm: breathing_rate,
            heart_rate_bpm: heart_rate,
            breathing_confidence,
            heartbeat_confidence,
            signal_quality,
        }
    }

    /// Extract breathing rate from the breathing buffer via FFT.
    /// Returns (rate_bpm, confidence).
    pub fn extract_breathing(&self) -> (Option<f64>, f64) {
        if self.breathing_buffer.len() < MIN_BREATHING_SAMPLES {
            return (None, 0.0);
        }

        let data: Vec<f64> = self.breathing_buffer.iter().copied().collect();
        let filtered = bandpass_filter(&data, BREATHING_MIN_HZ, BREATHING_MAX_HZ, self.sample_rate);
        self.compute_fft_peak(&filtered, BREATHING_MIN_HZ, BREATHING_MAX_HZ)
    }

    /// Extract heart rate from the heartbeat buffer via FFT.
    /// Returns (rate_bpm, confidence).
    pub fn extract_heartbeat(&self) -> (Option<f64>, f64) {
        if self.heartbeat_buffer.len() < MIN_HEARTBEAT_SAMPLES {
            return (None, 0.0);
        }

        let data: Vec<f64> = self.heartbeat_buffer.iter().copied().collect();
        let filtered = bandpass_filter(&data, HEARTBEAT_MIN_HZ, HEARTBEAT_MAX_HZ, self.sample_rate);
        self.compute_fft_peak(&filtered, HEARTBEAT_MIN_HZ, HEARTBEAT_MAX_HZ)
    }

    /// Find the dominant frequency in `buffer` within the [min_hz, max_hz] band
    /// using FFT. Returns (frequency_as_bpm, confidence).
    pub fn compute_fft_peak(
        &self,
        buffer: &[f64],
        min_hz: f64,
        max_hz: f64,
    ) -> (Option<f64>, f64) {
        if buffer.len() < 4 {
            return (None, 0.0);
        }

        // Zero-pad to next power of two for radix-2 FFT
        let fft_len = buffer.len().next_power_of_two();
        let mut signal = vec![0.0; fft_len];
        signal[..buffer.len()].copy_from_slice(buffer);

        // Apply Hann window to reduce spectral leakage
        for i in 0..buffer.len() {
            let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (buffer.len() as f64 - 1.0)).cos());
            signal[i] *= w;
        }

        // Compute FFT magnitude spectrum
        let spectrum = fft_magnitude(&signal);

        // Frequency resolution
        let freq_res = self.sample_rate / fft_len as f64;

        // Find bin range for our band of interest
        let min_bin = (min_hz / freq_res).ceil() as usize;
        let max_bin = ((max_hz / freq_res).floor() as usize).min(spectrum.len().saturating_sub(1));

        if min_bin >= max_bin || min_bin >= spectrum.len() {
            return (None, 0.0);
        }

        // Find peak magnitude and its bin index within the band
        let mut peak_mag = 0.0f64;
        let mut peak_bin = min_bin;
        let mut band_sum = 0.0f64;
        let mut band_count = 0usize;

        for bin in min_bin..=max_bin {
            let mag = spectrum[bin];
            band_sum += mag;
            band_count += 1;
            if mag > peak_mag {
                peak_mag = mag;
                peak_bin = bin;
            }
        }

        if band_count == 0 || band_sum < f64::EPSILON {
            return (None, 0.0);
        }

        let band_mean = band_sum / band_count as f64;

        // Confidence: ratio of peak to band mean, normalized to 0-1
        let peak_ratio = if band_mean > f64::EPSILON {
            peak_mag / band_mean
        } else {
            0.0
        };

        // Parabolic interpolation for sub-bin frequency accuracy
        let peak_freq = if peak_bin > min_bin && peak_bin < max_bin {
            let alpha = spectrum[peak_bin - 1];
            let beta = spectrum[peak_bin];
            let gamma = spectrum[peak_bin + 1];
            let denom = alpha - 2.0 * beta + gamma;
            if denom.abs() > f64::EPSILON {
                let p = 0.5 * (alpha - gamma) / denom;
                (peak_bin as f64 + p) * freq_res
            } else {
                peak_bin as f64 * freq_res
            }
        } else {
            peak_bin as f64 * freq_res
        };

        let bpm = peak_freq * 60.0;

        // Confidence mapping: peak_ratio >= CONFIDENCE_THRESHOLD maps to high confidence
        let confidence = if peak_ratio >= CONFIDENCE_THRESHOLD {
            ((peak_ratio - 1.0) / (CONFIDENCE_THRESHOLD * 2.0 - 1.0)).clamp(0.0, 1.0)
        } else {
            ((peak_ratio - 1.0) / (CONFIDENCE_THRESHOLD - 1.0) * 0.5).clamp(0.0, 0.5)
        };

        if confidence > 0.05 {
            (Some(bpm), confidence)
        } else {
            (None, confidence)
        }
    }

    /// Overall signal quality based on amplitude statistics.
    fn compute_signal_quality(&self, amplitude: &[f64]) -> f64 {
        if amplitude.is_empty() {
            return 0.0;
        }

        let n = amplitude.len() as f64;
        let mean = amplitude.iter().sum::<f64>() / n;

        if mean < f64::EPSILON {
            return 0.0;
        }

        let variance = amplitude.iter().map(|a| (a - mean).powi(2)).sum::<f64>() / n;
        let cv = variance.sqrt() / mean; // coefficient of variation

        // Good signal: moderate CV (some variation from body motion, not pure noise).
        // - Too low CV (~0) = static, no person present
        // - Too high CV (>1) = noisy/unstable signal
        // Sweet spot around 0.05-0.3
        let quality = if cv < 0.01 {
            cv / 0.01 * 0.3 // very low variation => low quality
        } else if cv < 0.3 {
            0.3 + 0.7 * (1.0 - ((cv - 0.15) / 0.15).abs()).max(0.0) // peak around 0.15
        } else {
            (1.0 - (cv - 0.3) / 0.7).clamp(0.1, 0.5) // too noisy
        };

        // Factor in buffer fill level (need enough history for reliable estimates)
        let fill =
            (self.breathing_buffer.len() as f64) / (self.breathing_capacity as f64).max(1.0);
        let fill_factor = fill.clamp(0.0, 1.0);

        (quality * (0.3 + 0.7 * fill_factor)).clamp(0.0, 1.0)
    }

    /// Clear all internal buffers and reset state.
    pub fn reset(&mut self) {
        self.breathing_buffer.clear();
        self.heartbeat_buffer.clear();
        self.frame_count = 0;
    }

    /// Current buffer fill levels for diagnostics.
    /// Returns (breathing_len, breathing_capacity, heartbeat_len, heartbeat_capacity).
    pub fn buffer_status(&self) -> (usize, usize, usize, usize) {
        (
            self.breathing_buffer.len(),
            self.breathing_capacity,
            self.heartbeat_buffer.len(),
            self.heartbeat_capacity,
        )
    }
}

// ── Bandpass filter ────────────────────────────────────────────────────────

/// Simple FIR bandpass filter using a windowed-sinc design.
///
/// Constructs a bandpass by subtracting two lowpass filters (LPF_high - LPF_low)
/// with a Hamming window. This is a zero-external-dependency implementation
/// suitable for the buffer sizes we encounter (up to ~600 samples).
pub fn bandpass_filter(data: &[f64], low_hz: f64, high_hz: f64, sample_rate: f64) -> Vec<f64> {
    if data.len() < 3 || sample_rate < f64::EPSILON {
        return data.to_vec();
    }

    // Normalized cutoff frequencies (0 to 0.5)
    let low_norm = low_hz / sample_rate;
    let high_norm = high_hz / sample_rate;

    if low_norm >= high_norm || low_norm >= 0.5 || high_norm <= 0.0 {
        return data.to_vec();
    }

    // FIR filter order: ~3 cycles of the lowest frequency, clamped to [5, 127]
    let filter_order = ((3.0 / low_norm).ceil() as usize).clamp(5, 127);
    // Ensure odd for type-I FIR symmetry
    let filter_order = if filter_order % 2 == 0 {
        filter_order + 1
    } else {
        filter_order
    };

    let half = filter_order / 2;
    let mut coeffs = vec![0.0f64; filter_order];

    // BPF = LPF(high_norm) - LPF(low_norm) with Hamming window
    for i in 0..filter_order {
        let n = i as f64 - half as f64;
        let lp_high = if n.abs() < f64::EPSILON {
            2.0 * high_norm
        } else {
            (2.0 * PI * high_norm * n).sin() / (PI * n)
        };
        let lp_low = if n.abs() < f64::EPSILON {
            2.0 * low_norm
        } else {
            (2.0 * PI * low_norm * n).sin() / (PI * n)
        };

        // Hamming window
        let w = 0.54 - 0.46 * (2.0 * PI * i as f64 / (filter_order as f64 - 1.0)).cos();
        coeffs[i] = (lp_high - lp_low) * w;
    }

    // Normalize filter to unit gain at center frequency
    let center_freq = (low_norm + high_norm) / 2.0;
    let gain: f64 = coeffs
        .iter()
        .enumerate()
        .map(|(i, &c)| c * (2.0 * PI * center_freq * i as f64).cos())
        .sum();
    if gain.abs() > f64::EPSILON {
        for c in coeffs.iter_mut() {
            *c /= gain;
        }
    }

    // Apply filter via convolution
    let mut output = vec![0.0f64; data.len()];
    for i in 0..data.len() {
        let mut sum = 0.0;
        for (j, &coeff) in coeffs.iter().enumerate() {
            let idx = i as isize - half as isize + j as isize;
            if idx >= 0 && (idx as usize) < data.len() {
                sum += data[idx as usize] * coeff;
            }
        }
        output[i] = sum;
    }

    output
}

// ── FFT implementation ─────────────────────────────────────────────────────

/// Compute the magnitude spectrum of a real-valued signal using radix-2 DIT FFT.
///
/// Input must be power-of-2 length (caller should zero-pad).
/// Returns magnitudes for bins 0..N/2+1.
fn fft_magnitude(signal: &[f64]) -> Vec<f64> {
    let n = signal.len();
    debug_assert!(n.is_power_of_two(), "FFT input must be power-of-2 length");

    if n <= 1 {
        return signal.to_vec();
    }

    // Convert to complex (imaginary = 0)
    let mut real = signal.to_vec();
    let mut imag = vec![0.0f64; n];

    // Bit-reversal permutation
    bit_reverse_permute(&mut real, &mut imag);

    // Cooley-Tukey radix-2 DIT butterfly
    let mut size = 2;
    while size <= n {
        let half = size / 2;
        let angle_step = -2.0 * PI / size as f64;

        for start in (0..n).step_by(size) {
            for k in 0..half {
                let angle = angle_step * k as f64;
                let wr = angle.cos();
                let wi = angle.sin();

                let i = start + k;
                let j = start + k + half;

                let tr = wr * real[j] - wi * imag[j];
                let ti = wr * imag[j] + wi * real[j];

                real[j] = real[i] - tr;
                imag[j] = imag[i] - ti;
                real[i] += tr;
                imag[i] += ti;
            }
        }

        size *= 2;
    }

    // Compute magnitudes for positive frequencies (0..N/2+1)
    let out_len = n / 2 + 1;
    let mut magnitudes = Vec::with_capacity(out_len);
    for i in 0..out_len {
        magnitudes.push((real[i] * real[i] + imag[i] * imag[i]).sqrt());
    }

    magnitudes
}

/// In-place bit-reversal permutation for FFT.
fn bit_reverse_permute(real: &mut [f64], imag: &mut [f64]) {
    let n = real.len();
    let bits = (n as f64).log2() as u32;

    for i in 0..n {
        let j = reverse_bits(i as u32, bits) as usize;
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
    }
}

/// Reverse the lower `bits` bits of `val`.
fn reverse_bits(val: u32, bits: u32) -> u32 {
    let mut result = 0u32;
    let mut v = val;
    for _ in 0..bits {
        result = (result << 1) | (v & 1);
        v >>= 1;
    }
    result
}

// ── Benchmark ──────────────────────────────────────────────────────────────

/// Run a benchmark: process `n_frames` synthetic frames and report timing.
///
/// Generates frames with embedded breathing (0.25 Hz / 15 BPM) and heartbeat
/// (1.2 Hz / 72 BPM) signals on 56 subcarriers at 20 Hz sample rate.
///
/// Returns (total_duration, per_frame_duration).
pub fn run_benchmark(n_frames: usize) -> (std::time::Duration, std::time::Duration) {
    use std::time::Instant;

    let sample_rate = 20.0;
    let mut detector = VitalSignDetector::new(sample_rate);

    // Pre-generate synthetic CSI data (56 subcarriers, matching simulation mode)
    let n_sub = 56;
    let frames: Vec<(Vec<f64>, Vec<f64>)> = (0..n_frames)
        .map(|tick| {
            let t = tick as f64 / sample_rate;
            let mut amp = Vec::with_capacity(n_sub);
            let mut phase = Vec::with_capacity(n_sub);
            for i in 0..n_sub {
                // Embedded breathing at 0.25 Hz (15 BPM) and heartbeat at 1.2 Hz (72 BPM)
                let breathing = 2.0 * (2.0 * PI * 0.25 * t).sin();
                let heartbeat = 0.3 * (2.0 * PI * 1.2 * t).sin();
                let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                let noise = (i as f64 * 7.3 + t * 13.7).sin() * 0.5;
                amp.push(base + breathing + heartbeat + noise);
                phase.push((i as f64 * 0.2 + t * 0.5).sin() * PI + heartbeat * 0.1);
            }
            (amp, phase)
        })
        .collect();

    let start = Instant::now();
    let mut last_vital = VitalSigns::default();
    for (amp, phase) in &frames {
        last_vital = detector.process_frame(amp, phase);
    }
    let total = start.elapsed();
    let per_frame = total / n_frames as u32;

    eprintln!("=== Vital Sign Detection Benchmark ===");
    eprintln!("Frames processed:       {}", n_frames);
    eprintln!("Sample rate:            {} Hz", sample_rate);
    eprintln!("Subcarriers:            {}", n_sub);
    eprintln!("Total time:             {:?}", total);
    eprintln!("Per-frame time:         {:?}", per_frame);
    eprintln!(
        "Throughput:             {:.0} frames/sec",
        n_frames as f64 / total.as_secs_f64()
    );
    eprintln!();
    eprintln!("Final vital signs:");
    eprintln!(
        "  Breathing rate:       {:?} BPM",
        last_vital.breathing_rate_bpm
    );
    eprintln!("  Heart rate:           {:?} BPM", last_vital.heart_rate_bpm);
    eprintln!(
        "  Breathing confidence: {:.3}",
        last_vital.breathing_confidence
    );
    eprintln!(
        "  Heartbeat confidence: {:.3}",
        last_vital.heartbeat_confidence
    );
    eprintln!(
        "  Signal quality:       {:.3}",
        last_vital.signal_quality
    );

    (total, per_frame)
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fft_magnitude_dc() {
        let signal = vec![1.0; 8];
        let mag = fft_magnitude(&signal);
        // DC bin should be 8.0 (sum), all others near zero
        assert!((mag[0] - 8.0).abs() < 1e-10);
        for m in &mag[1..] {
            assert!(*m < 1e-10, "non-DC bin should be near zero, got {m}");
        }
    }

    #[test]
    fn test_fft_magnitude_sine() {
        // 16-point signal with a single sinusoid at bin 2
        let n = 16;
        let mut signal = vec![0.0; n];
        for i in 0..n {
            signal[i] = (2.0 * PI * 2.0 * i as f64 / n as f64).sin();
        }
        let mag = fft_magnitude(&signal);
        // Peak should be at bin 2
        let peak_bin = mag
            .iter()
            .enumerate()
            .skip(1) // skip DC
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(peak_bin, 2);
    }

    #[test]
    fn test_bit_reverse() {
        assert_eq!(reverse_bits(0b000, 3), 0b000);
        assert_eq!(reverse_bits(0b001, 3), 0b100);
        assert_eq!(reverse_bits(0b110, 3), 0b011);
    }

    #[test]
    fn test_bandpass_filter_passthrough() {
        // A sine at the center of the passband should mostly pass through
        let sr = 20.0;
        let freq = 0.25; // center of breathing band
        let n = 200;
        let data: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        let filtered = bandpass_filter(&data, 0.1, 0.5, sr);
        // Check that the filtered signal has significant energy
        let energy: f64 = filtered.iter().map(|x| x * x).sum::<f64>() / n as f64;
        assert!(
            energy > 0.01,
            "passband signal should pass through, energy={energy}"
        );
    }

    #[test]
    fn test_bandpass_filter_rejects_out_of_band() {
        // A sine well outside the passband should be attenuated
        let sr = 20.0;
        let freq = 5.0; // way above breathing band
        let n = 200;
        let data: Vec<f64> = (0..n)
            .map(|i| (2.0 * PI * freq * i as f64 / sr).sin())
            .collect();
        let in_energy: f64 = data.iter().map(|x| x * x).sum::<f64>() / n as f64;
        let filtered = bandpass_filter(&data, 0.1, 0.5, sr);
        let out_energy: f64 = filtered.iter().map(|x| x * x).sum::<f64>() / n as f64;
        let attenuation = out_energy / in_energy;
        assert!(
            attenuation < 0.3,
            "out-of-band signal should be attenuated, ratio={attenuation}"
        );
    }

    #[test]
    fn test_vital_sign_detector_breathing() {
        let sr = 20.0;
        let mut detector = VitalSignDetector::new(sr);
        let target_bpm = 15.0; // 0.25 Hz
        let target_hz = target_bpm / 60.0;

        // Feed 30 seconds of data with a clear breathing signal
        let n_frames = (sr * 30.0) as usize;
        let mut vitals = VitalSigns::default();
        for frame in 0..n_frames {
            let t = frame as f64 / sr;
            let amp: Vec<f64> = (0..56)
                .map(|i| {
                    let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                    let breathing = 3.0 * (2.0 * PI * target_hz * t).sin();
                    base + breathing
                })
                .collect();
            let phase: Vec<f64> = (0..56).map(|i| (i as f64 * 0.2).sin()).collect();
            vitals = detector.process_frame(&amp, &phase);
        }

        // After 30s, breathing should be detected
        assert!(
            vitals.breathing_rate_bpm.is_some(),
            "breathing should be detected after 30s"
        );
        if let Some(rate) = vitals.breathing_rate_bpm {
            let error = (rate - target_bpm).abs();
            assert!(
                error < 3.0,
                "breathing rate {rate:.1} BPM should be near {target_bpm} BPM (error={error:.1})"
            );
        }
    }

    #[test]
    fn test_vital_sign_detector_reset() {
        let mut detector = VitalSignDetector::new(20.0);
        let amp = vec![10.0; 56];
        let phase = vec![0.0; 56];
        for _ in 0..100 {
            detector.process_frame(&amp, &phase);
        }
        let (br_len, _, hb_len, _) = detector.buffer_status();
        assert!(br_len > 0);
        assert!(hb_len > 0);

        detector.reset();
        let (br_len, _, hb_len, _) = detector.buffer_status();
        assert_eq!(br_len, 0);
        assert_eq!(hb_len, 0);
    }

    #[test]
    fn test_vital_signs_default() {
        let vs = VitalSigns::default();
        assert!(vs.breathing_rate_bpm.is_none());
        assert!(vs.heart_rate_bpm.is_none());
        assert_eq!(vs.breathing_confidence, 0.0);
        assert_eq!(vs.heartbeat_confidence, 0.0);
        assert_eq!(vs.signal_quality, 0.0);
    }

    #[test]
    fn test_empty_amplitude() {
        let mut detector = VitalSignDetector::new(20.0);
        let vs = detector.process_frame(&[], &[]);
        assert!(vs.breathing_rate_bpm.is_none());
        assert!(vs.heart_rate_bpm.is_none());
    }

    #[test]
    fn test_single_subcarrier() {
        let mut detector = VitalSignDetector::new(20.0);
        // Single subcarrier should not crash
        for i in 0..100 {
            let t = i as f64 / 20.0;
            let amp = vec![10.0 + (2.0 * PI * 0.25 * t).sin()];
            let phase = vec![0.0];
            let _ = detector.process_frame(&amp, &phase);
        }
    }

    #[test]
    fn test_benchmark_runs() {
        let (total, per_frame) = run_benchmark(100);
        assert!(total.as_nanos() > 0);
        assert!(per_frame.as_nanos() > 0);
    }
}
