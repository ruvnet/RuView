//! Hilbert transform for instantaneous phase and amplitude extraction.
//!
//! Computes the analytic signal via FFT-based Hilbert transform:
//!   1. FFT the real signal
//!   2. Zero negative frequencies, double positive frequencies
//!   3. IFFT to obtain the analytic signal
//!
//! The instantaneous amplitude is |analytic(t)| and the instantaneous
//! phase is arg(analytic(t)).

use num_complex::Complex;
use rustfft::FftPlanner;
use std::cell::RefCell;

thread_local! {
    static FFT_PLANNER: RefCell<FftPlanner<f64>> = RefCell::new(FftPlanner::new());
}

/// Compute the analytic signal via FFT-based Hilbert transform.
///
/// Given a real signal x(t), returns the analytic signal z(t) = x(t) + j * H[x](t),
/// where H[x] is the Hilbert transform of x.
///
/// Uses a thread-local cached FftPlanner to avoid re-creating plans on every call.
pub fn hilbert_transform(signal: &[f64]) -> Vec<Complex<f64>> {
    let n = signal.len();
    if n == 0 {
        return Vec::new();
    }

    let (fft_forward, fft_inverse) = FFT_PLANNER.with(|planner| {
        let mut planner = planner.borrow_mut();
        let fwd = planner.plan_fft_forward(n);
        let inv = planner.plan_fft_inverse(n);
        (fwd, inv)
    });

    // Forward FFT
    let mut spectrum: Vec<Complex<f64>> = signal.iter().map(|&x| Complex::new(x, 0.0)).collect();
    fft_forward.process(&mut spectrum);

    // Build the analytic signal in the frequency domain:
    // - DC component (k=0): multiply by 1
    // - Positive frequencies (k=1..n/2-1): multiply by 2
    // - Nyquist (k=n/2, if n is even): multiply by 1
    // - Negative frequencies (k=n/2+1..n-1): multiply by 0
    if n > 1 {
        let half = n / 2;
        for k in 1..half {
            spectrum[k] *= 2.0;
        }
        // Nyquist bin stays at 1x if n is even (already correct)
        for k in (half + 1)..n {
            spectrum[k] = Complex::new(0.0, 0.0);
        }
    }

    // Inverse FFT
    fft_inverse.process(&mut spectrum);

    // Normalize by N (rustfft does unnormalized transforms)
    let inv_n = 1.0 / n as f64;
    for s in &mut spectrum {
        *s *= inv_n;
    }

    spectrum
}

/// Compute the instantaneous phase of a signal via the Hilbert transform.
///
/// Returns phase values in radians in the range (-pi, pi].
pub fn instantaneous_phase(signal: &[f64]) -> Vec<f64> {
    hilbert_transform(signal)
        .iter()
        .map(|z| z.im.atan2(z.re))
        .collect()
}

/// Compute the instantaneous amplitude (envelope) of a signal via the Hilbert transform.
///
/// Returns |analytic(t)| for each sample.
pub fn instantaneous_amplitude(signal: &[f64]) -> Vec<f64> {
    hilbert_transform(signal)
        .iter()
        .map(|z| z.norm())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_abs_diff_eq;
    use std::f64::consts::PI;

    #[test]
    fn hilbert_of_cosine_gives_sine() {
        // For cos(2*pi*f*t), the Hilbert transform is sin(2*pi*f*t).
        // The analytic signal is cos + j*sin = exp(j*2*pi*f*t).
        // So the imaginary part of the analytic signal should be sin.
        let n = 256;
        let f = 5.0;
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                (2.0 * PI * f * t).cos()
            })
            .collect();

        let analytic = hilbert_transform(&signal);

        // Check imaginary part ≈ sin(2*pi*f*t) for interior samples
        // (edge effects make first/last few samples less accurate)
        for i in 10..(n - 10) {
            let t = i as f64 / n as f64;
            let expected_sin = (2.0 * PI * f * t).sin();
            assert_abs_diff_eq!(analytic[i].im, expected_sin, epsilon = 0.05);
        }
    }

    #[test]
    fn instantaneous_amplitude_of_constant_frequency() {
        // A pure cosine has constant amplitude = 1.0
        let n = 256;
        let f = 10.0;
        let signal: Vec<f64> = (0..n)
            .map(|i| {
                let t = i as f64 / n as f64;
                (2.0 * PI * f * t).cos()
            })
            .collect();

        let amp = instantaneous_amplitude(&signal);

        // Interior samples should have amplitude close to 1.0
        for &a in &amp[10..(n - 10)] {
            assert_abs_diff_eq!(a, 1.0, epsilon = 0.05);
        }
    }

    #[test]
    fn empty_signal() {
        let result = hilbert_transform(&[]);
        assert!(result.is_empty());
    }
}
