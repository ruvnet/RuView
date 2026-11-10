use std::collections::VecDeque;

/// Candidate breathing frequencies evaluation band: 0.1 to 0.5 Hz (6 to 30 breaths/min).
const BREATHING_FREQ_LOW: f64 = 0.1;
const BREATHING_FREQ_HIGH: f64 = 0.5;
const N_CANDIDATES: usize = 9;

/// Estimate breathing rate in Hz from recent CSI amplitude history.
///
/// Uses Goertzel power calculation across physiological candidate frequencies.
/// If energy is meaningfully above background noise (> 3x average power),
/// returns the peak candidate frequency clamped to [0.1, 0.5] Hz.
pub fn estimate_breathing_rate_hz(
    frame_history: &VecDeque<Vec<f64>>,
    sample_rate_hz: f64,
) -> f64 {
    let n = frame_history.len();
    if n < 6 || sample_rate_hz <= 0.0 {
        return 0.0;
    }

    let series: Vec<f64> = frame_history
        .iter()
        .map(|amps| {
            if amps.is_empty() {
                0.0
            } else {
                amps.iter().sum::<f64>() / amps.len() as f64
            }
        })
        .collect();

    let mean_s = series.iter().sum::<f64>() / n as f64;
    let detrended: Vec<f64> = series.iter().map(|x| x - mean_s).collect();

    let mut best_freq = 0.0f64;
    let mut best_power = 0.0f64;
    let mut total_power = 0.0f64;

    for i in 0..N_CANDIDATES {
        let freq = BREATHING_FREQ_LOW
            + (BREATHING_FREQ_HIGH - BREATHING_FREQ_LOW) * i as f64
                / (N_CANDIDATES - 1).max(1) as f64;
        let omega = 2.0 * std::f64::consts::PI * freq / sample_rate_hz;
        let coeff = 2.0 * omega.cos();

        let mut s_prev2 = 0.0f64;
        let mut s_prev1 = 0.0f64;
        for &x in &detrended {
            let s = x + coeff * s_prev1 - s_prev2;
            s_prev2 = s_prev1;
            s_prev1 = s;
        }

        let power = s_prev2 * s_prev2 + s_prev1 * s_prev1 - coeff * s_prev1 * s_prev2;
        total_power += power;

        if power > best_power {
            best_power = power;
            best_freq = freq;
        }
    }

    let avg_power = total_power / N_CANDIDATES as f64;
    if best_power > avg_power * 3.0 {
        best_freq.clamp(BREATHING_FREQ_LOW, BREATHING_FREQ_HIGH)
    } else {
        0.0
    }
}

