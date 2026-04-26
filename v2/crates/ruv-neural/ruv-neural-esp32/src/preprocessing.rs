//! Lightweight edge preprocessing that runs on the ESP32 before data is sent
//! upstream to the RuVector backend.
//!
//! Includes fixed-point IIR filtering for integer-only ESP32 math paths and
//! floating-point downsampling / pipeline processing for `std` targets.

/// IIR filter coefficients for a second-order section (biquad).
///
/// Transfer function: `H(z) = (b0 + b1*z^-1 + b2*z^-2) / (a0 + a1*z^-1 + a2*z^-2)`
#[derive(Debug, Clone)]
pub struct IirCoeffs {
    /// Numerator coefficients `[b0, b1, b2]`.
    pub b: [f64; 3],
    /// Denominator coefficients `[a0, a1, a2]`.
    pub a: [f64; 3],
}

impl IirCoeffs {
    /// Create notch filter coefficients for a given frequency and sample rate.
    ///
    /// Uses a quality factor of 30 for a narrow rejection band.
    pub fn notch(freq_hz: f64, sample_rate_hz: f64) -> Self {
        let w0 = 2.0 * std::f64::consts::PI * freq_hz / sample_rate_hz;
        let q = 30.0;
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let b0 = 1.0;
        let b1 = -2.0 * cos_w0;
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * cos_w0;
        let a2 = 1.0 - alpha;

        // Normalize by a0
        Self {
            b: [b0 / a0, b1 / a0, b2 / a0],
            a: [1.0, a1 / a0, a2 / a0],
        }
    }

    /// Create a first-order high-pass filter (stored as second-order with
    /// zero padding).
    pub fn highpass(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = rc / (rc + dt);

        Self {
            b: [alpha, -alpha, 0.0],
            a: [1.0, -(1.0 - alpha), 0.0],
        }
    }

    /// Create a first-order low-pass filter (stored as second-order with
    /// zero padding).
    pub fn lowpass(cutoff_hz: f64, sample_rate_hz: f64) -> Self {
        let rc = 1.0 / (2.0 * std::f64::consts::PI * cutoff_hz);
        let dt = 1.0 / sample_rate_hz;
        let alpha = dt / (rc + dt);

        Self {
            b: [alpha, 0.0, 0.0],
            a: [1.0, -(1.0 - alpha), 0.0],
        }
    }
}

/// Minimal preprocessing pipeline that runs on the ESP32 before data is sent
/// upstream.
pub struct EdgePreprocessor {
    /// Apply a 50 Hz notch filter (mains power, EU/Asia).
    pub notch_50hz: bool,
    /// Apply a 60 Hz notch filter (mains power, Americas).
    pub notch_60hz: bool,
    /// High-pass cutoff frequency in Hz.
    pub highpass_hz: f64,
    /// Low-pass cutoff frequency in Hz.
    pub lowpass_hz: f64,
    /// Downsample factor (1 = no downsampling).
    pub downsample_factor: usize,
    /// Sample rate of the incoming data in Hz.
    pub sample_rate_hz: f64,
}

impl Default for EdgePreprocessor {
    fn default() -> Self {
        Self::new()
    }
}

impl EdgePreprocessor {
    /// Create a preprocessor with sensible defaults for neural sensing.
    pub fn new() -> Self {
        Self {
            notch_50hz: true,
            notch_60hz: true,
            highpass_hz: 0.5,
            lowpass_hz: 200.0,
            downsample_factor: 1,
            sample_rate_hz: 1000.0,
        }
    }

    /// Apply a second-order IIR filter using fixed-point arithmetic.
    ///
    /// Coefficients are scaled by 2^14 internally to use integer multiply/shift
    /// on the ESP32. The output is clipped to `i16` range.
    pub fn apply_iir_fixed(&self, samples: &[i16], coeffs: &IirCoeffs) -> Vec<i16> {
        const SCALE: i64 = 1 << 14;

        let b0 = (coeffs.b[0] * SCALE as f64) as i64;
        let b1 = (coeffs.b[1] * SCALE as f64) as i64;
        let b2 = (coeffs.b[2] * SCALE as f64) as i64;
        let a1 = (coeffs.a[1] * SCALE as f64) as i64;
        let a2 = (coeffs.a[2] * SCALE as f64) as i64;

        let mut out = Vec::with_capacity(samples.len());
        let mut x1: i64 = 0;
        let mut x2: i64 = 0;
        let mut y1: i64 = 0;
        let mut y2: i64 = 0;

        for &x0 in samples {
            let x0 = x0 as i64;
            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) >> 14;

            let clamped = y0.clamp(i16::MIN as i64, i16::MAX as i64) as i16;
            out.push(clamped);

            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
        }

        out
    }

    /// Apply a second-order IIR filter using floating-point arithmetic.
    fn apply_iir_float(&self, samples: &[f64], coeffs: &IirCoeffs) -> Vec<f64> {
        let mut out = Vec::with_capacity(samples.len());
        let mut x1 = 0.0_f64;
        let mut x2 = 0.0_f64;
        let mut y1 = 0.0_f64;
        let mut y2 = 0.0_f64;

        for &x0 in samples {
            let y0 = coeffs.b[0] * x0 + coeffs.b[1] * x1 + coeffs.b[2] * x2
                - coeffs.a[1] * y1
                - coeffs.a[2] * y2;

            out.push(y0);

            x2 = x1;
            x1 = x0;
            y2 = y1;
            y1 = y0;
        }

        out
    }

    /// Downsample by block-averaging groups of `factor` consecutive samples.
    ///
    /// If the input length is not a multiple of `factor`, the trailing samples
    /// are averaged as a shorter block.
    pub fn downsample(&self, samples: &[f64], factor: usize) -> Vec<f64> {
        if factor <= 1 || samples.is_empty() {
            return samples.to_vec();
        }

        samples
            .chunks(factor)
            .map(|chunk| {
                let sum: f64 = chunk.iter().sum();
                sum / chunk.len() as f64
            })
            .collect()
    }

    /// Run the full edge preprocessing pipeline on multi-channel data.
    ///
    /// Steps (in order):
    /// 1. High-pass filter (remove DC offset / drift)
    /// 2. Notch filter at 50 Hz (if enabled)
    /// 3. Notch filter at 60 Hz (if enabled)
    /// 4. Low-pass filter (anti-alias before downsampling)
    /// 5. Downsample
    pub fn process(&self, raw_data: &[Vec<f64>]) -> Vec<Vec<f64>> {
        let sr = self.sample_rate_hz;

        let hp_coeffs = IirCoeffs::highpass(self.highpass_hz, sr);
        let lp_coeffs = IirCoeffs::lowpass(self.lowpass_hz, sr);
        let notch_50 = IirCoeffs::notch(50.0, sr);
        let notch_60 = IirCoeffs::notch(60.0, sr);

        raw_data
            .iter()
            .map(|channel| {
                let mut data = self.apply_iir_float(channel, &hp_coeffs);

                if self.notch_50hz {
                    data = self.apply_iir_float(&data, &notch_50);
                }
                if self.notch_60hz {
                    data = self.apply_iir_float(&data, &notch_60);
                }

                data = self.apply_iir_float(&data, &lp_coeffs);

                self.downsample(&data, self.downsample_factor)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_downsample_factor_2() {
        let pre = EdgePreprocessor::new();
        let input: Vec<f64> = (0..10).map(|x| x as f64).collect();
        let result = pre.downsample(&input, 2);
        assert_eq!(result.len(), 5);
        // [0,1] -> 0.5, [2,3] -> 2.5, ...
        assert!((result[0] - 0.5).abs() < 1e-10);
        assert!((result[1] - 2.5).abs() < 1e-10);
        assert!((result[4] - 8.5).abs() < 1e-10);
    }

    #[test]
    fn test_downsample_factor_1_is_identity() {
        let pre = EdgePreprocessor::new();
        let input = vec![1.0, 2.0, 3.0];
        let result = pre.downsample(&input, 1);
        assert_eq!(result, input);
    }

    #[test]
    fn test_downsample_non_multiple() {
        let pre = EdgePreprocessor::new();
        let input: Vec<f64> = (0..7).map(|x| x as f64).collect();
        let result = pre.downsample(&input, 3);
        // [0,1,2]->1, [3,4,5]->4, [6]->6
        assert_eq!(result.len(), 3);
        assert!((result[2] - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_process_output_length() {
        let mut pre = EdgePreprocessor::new();
        pre.downsample_factor = 4;
        pre.sample_rate_hz = 1000.0;
        let raw = vec![vec![0.0; 1000], vec![0.0; 1000]];
        let result = pre.process(&raw);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].len(), 250);
        assert_eq!(result[1].len(), 250);
    }

    #[test]
    fn test_iir_fixed_passthrough_dc() {
        // Identity-ish filter: b=[1,0,0], a=[1,0,0] should pass through
        let pre = EdgePreprocessor::new();
        let coeffs = IirCoeffs {
            b: [1.0, 0.0, 0.0],
            a: [1.0, 0.0, 0.0],
        };
        let input: Vec<i16> = vec![100, 200, 300, 400, 500];
        let output = pre.apply_iir_fixed(&input, &coeffs);
        assert_eq!(output.len(), 5);
        // With identity filter, output should match input
        for (i, &v) in output.iter().enumerate() {
            assert_eq!(v, input[i], "mismatch at index {i}");
        }
    }

    #[test]
    fn test_notch_coefficients_valid() {
        let coeffs = IirCoeffs::notch(50.0, 1000.0);
        // a[0] should be normalized to 1.0
        assert!((coeffs.a[0] - 1.0).abs() < 1e-10);
        // b[0] and b[2] should be equal for a notch
        assert!((coeffs.b[0] - coeffs.b[2]).abs() < 1e-10);
    }
}
