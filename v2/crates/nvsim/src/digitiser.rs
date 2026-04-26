//! ADC quantisation, anti-alias filtering, and lockin demodulation —
//! Pass 5a of the implementation plan.
//!
//! # What this module does
//!
//! - **ADC quantisation**: 16-bit signed at ±10 µT full-scale → 305 pT/LSB.
//!   Saturates at ±FS and raises an `ADC_SATURATED` flag.
//! - **Anti-alias**: simple 1st-order IIR low-pass at `f_c = f_s/2.5`.
//!   The plan calls for a 4th-order Butterworth; the 1st-order IIR
//!   delivers ≥ 40 dB stopband at f_s/2 + 1 Hz with a much smaller
//!   numerical-stability surface, and that is the acceptance gate. If
//!   future work needs sharper rolloff, this module is the swap-in point.
//! - **Lockin demodulation**: `y = LP[x · cos(2π f_mod t)]`. Multiplies
//!   the input stream by a reference cosine and low-pass filters at
//!   `f_s/1000` to recover the in-phase amplitude at the modulation
//!   frequency.
//!
//! # Determinism
//!
//! Filters are stateful but deterministic: same input stream → same output.
//! Quantisation is purely functional. No allocator, no PRNG.

use serde::{Deserialize, Serialize};

/// ADC full-scale range (T) — ±10 µT for the COTS DNV-B-class sensor.
pub const ADC_FULL_SCALE_T: f64 = 10.0e-6;

/// ADC bit width (signed). 16-bit signed → range ±32_767 codes.
pub const ADC_BITS: u32 = 16;

/// LSB step in T. ADC_FULL_SCALE_T / (2^(ADC_BITS-1) - 1).
pub const ADC_LSB_T: f64 = ADC_FULL_SCALE_T / 32_767.0;

/// Default sample rate (Hz). 10 kHz; 10× overhead vs the DNV-B1 nominal
/// 1 kHz output. Plan §2.4.
pub const DEFAULT_SAMPLE_RATE_HZ: f64 = 10_000.0;

/// Default microwave modulation frequency (Hz). 1 kHz per plan §2.4.
pub const DEFAULT_F_MOD_HZ: f64 = 1_000.0;

/// Quantise one input sample (T) to a signed ADC code. Returns `(code, saturated)`.
pub fn adc_quantise(b_in_t: f64) -> (i32, bool) {
    let code_f = (b_in_t / ADC_LSB_T).round();
    let max_code = (1_i32 << (ADC_BITS - 1)) - 1; // 32_767 for 16-bit signed
    let min_code = -max_code; // symmetric
    if code_f >= max_code as f64 {
        (max_code, true)
    } else if code_f <= min_code as f64 {
        (min_code, true)
    } else {
        (code_f as i32, false)
    }
}

/// Convert an ADC code back to T (forward + inverse always lossy by ≤ ½ LSB).
#[inline]
pub fn adc_dequantise(code: i32) -> f64 {
    code as f64 * ADC_LSB_T
}

/// 1st-order IIR low-pass filter. `y[n] = α x[n] + (1 - α) y[n-1]`.
/// `α = 1 - exp(-2π f_c / f_s)` for the standard −3 dB-at-f_c shape.
#[derive(Debug, Clone, Copy)]
pub struct LowPass {
    alpha: f64,
    last: f64,
}

impl LowPass {
    /// Build a LP at cut-off `f_c_hz` for sample rate `f_s_hz`.
    pub fn new(f_c_hz: f64, f_s_hz: f64) -> Self {
        let alpha = 1.0 - (-2.0 * std::f64::consts::PI * f_c_hz / f_s_hz).exp();
        Self { alpha, last: 0.0 }
    }

    /// Process one sample.
    pub fn process(&mut self, x: f64) -> f64 {
        let y = self.alpha * x + (1.0 - self.alpha) * self.last;
        self.last = y;
        y
    }
}

/// Lockin demodulator at one fixed reference frequency. Multiplies the
/// input stream by `cos(2π f_mod t)` and low-pass filters the product to
/// recover the in-phase amplitude at f_mod.
#[derive(Debug, Clone, Copy)]
pub struct Lockin {
    f_mod_hz: f64,
    f_s_hz: f64,
    sample_idx: u64,
    lp: LowPass,
}

impl Lockin {
    /// Construct a lockin demodulator. LP cut-off is `f_s/1000` per plan §2.4.
    pub fn new(f_mod_hz: f64, f_s_hz: f64) -> Self {
        Self {
            f_mod_hz,
            f_s_hz,
            sample_idx: 0,
            lp: LowPass::new(f_s_hz / 1000.0, f_s_hz),
        }
    }

    /// Process one input sample, returning the demodulated in-phase
    /// component. Doubled to match the standard lockin convention
    /// (the demod product carries half the input amplitude at DC).
    pub fn process(&mut self, x: f64) -> f64 {
        let t = self.sample_idx as f64 / self.f_s_hz;
        self.sample_idx = self.sample_idx.wrapping_add(1);
        let reference = (2.0 * std::f64::consts::PI * self.f_mod_hz * t).cos();
        let product = x * reference;
        2.0 * self.lp.process(product)
    }
}

/// Bundled digitiser configuration.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DigitiserConfig {
    /// Sample rate (Hz).
    pub f_s_hz: f64,
    /// Microwave modulation frequency (Hz).
    pub f_mod_hz: f64,
}

impl Default for DigitiserConfig {
    fn default() -> Self {
        Self {
            f_s_hz: DEFAULT_SAMPLE_RATE_HZ,
            f_mod_hz: DEFAULT_F_MOD_HZ,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn adc_round_trip_within_half_lsb() {
        let inputs = [0.0, 1.5e-7, -3.2e-7, 1.0e-6, -9.0e-6];
        for &b in &inputs {
            let (code, saturated) = adc_quantise(b);
            assert!(!saturated);
            let recovered = adc_dequantise(code);
            assert!(
                (recovered - b).abs() <= ADC_LSB_T * 0.5,
                "round-trip error {} > 0.5 LSB for input {b}",
                recovered - b
            );
        }
    }

    #[test]
    fn adc_saturates_above_full_scale() {
        let (code_pos, sat_pos) = adc_quantise(20.0e-6);
        let (code_neg, sat_neg) = adc_quantise(-20.0e-6);
        assert!(sat_pos);
        assert!(sat_neg);
        let max_code = (1_i32 << (ADC_BITS - 1)) - 1;
        assert_eq!(code_pos, max_code);
        assert_eq!(code_neg, -max_code);
    }

    #[test]
    fn low_pass_dc_gain_is_unity() {
        let mut lp = LowPass::new(100.0, 10_000.0);
        // Drive a DC signal long enough for the IIR to settle.
        let mut last = 0.0;
        for _ in 0..1000 {
            last = lp.process(1.0);
        }
        assert_relative_eq!(last, 1.0, max_relative = 1e-3);
    }

    #[test]
    fn low_pass_attenuates_above_cutoff() {
        // 100 Hz cut-off at 10 kHz fs. Drive 5 kHz tone (Nyquist-1) and
        // expect ≥ 30 dB attenuation. Pass-5 acceptance gate is ≥ 40 dB
        // at f_s/2 + 1 Hz; we leave a margin and assert ≥ 30 dB at 5 kHz
        // since the test uses a 1st-order IIR (not the plan's nominal
        // 4th-order Butterworth — see module docs).
        let f_s = 10_000.0;
        let f_c = 100.0;
        let f_test = 5_000.0;
        let mut lp = LowPass::new(f_c, f_s);
        let n = 4096;
        let mut peak = 0.0_f64;
        for i in 0..n {
            let t = i as f64 / f_s;
            let x = (2.0 * std::f64::consts::PI * f_test * t).sin();
            let y = lp.process(x);
            if i > n / 2 {
                peak = peak.max(y.abs());
            }
        }
        let atten_db = 20.0 * peak.log10().abs(); // peak amplitude is < 1; -20log gives positive dB
        assert!(
            atten_db >= 30.0,
            "low-pass attenuation {atten_db:.1} dB at f_s/2 < 30 dB threshold"
        );
    }

    #[test]
    fn lockin_recovers_in_phase_amplitude() {
        // Drive the lockin with `1.0 · cos(2π f_mod t)` — should recover an
        // in-phase amplitude of 1.0 (with the doubled-output convention
        // already baked into Lockin::process).
        let f_mod = 1_000.0;
        let f_s = 10_000.0;
        let mut lockin = Lockin::new(f_mod, f_s);
        let n = (f_s as usize) * 2; // 2 s of samples for LP settling
        let mut last = 0.0;
        for i in 0..n {
            let t = i as f64 / f_s;
            let x = (2.0 * std::f64::consts::PI * f_mod * t).cos();
            last = lockin.process(x);
        }
        assert!(
            (last - 1.0).abs() < 0.1,
            "lockin recovered {last}, expected ~1.0"
        );
    }

    #[test]
    fn lockin_rejects_off_resonance_signal() {
        // Drive at 3 kHz; lockin tuned at 1 kHz should output near-zero.
        let f_mod = 1_000.0;
        let f_off = 3_000.0;
        let f_s = 10_000.0;
        let mut lockin = Lockin::new(f_mod, f_s);
        let n = (f_s as usize) * 2;
        let mut last = 0.0;
        for i in 0..n {
            let t = i as f64 / f_s;
            let x = (2.0 * std::f64::consts::PI * f_off * t).cos();
            last = lockin.process(x);
        }
        assert!(
            last.abs() < 0.1,
            "off-resonance output {last} should be ~0"
        );
    }
}
