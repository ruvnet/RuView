use std::collections::VecDeque;

/// Estimate a real temporal motion frequency from recent CSI amplitude history.
///
/// Each frame is reduced to its mean amplitude, detrended, Hann windowed,
/// and evaluated on exact DFT bins.
pub fn estimate_temporal_dominant_frequency_hz(
    frame_history: &VecDeque<Vec<f64>>,
    sample_rate_hz: f64,
) -> f64 {
    if !sample_rate_hz.is_finite() || sample_rate_hz <= 0.0 {
        return 0.0;
    }

    let recent_frames: Vec<&Vec<f64>> = frame_history.iter().rev().take(128).collect();
    if recent_frames
        .iter()
        .any(|frame| frame.is_empty() || frame.iter().any(|value| !value.is_finite()))
    {
        return 0.0;
    }

    let samples: Vec<f64> = recent_frames
        .into_iter()
        .rev()
        .map(|frame| frame.iter().sum::<f64>() / frame.len() as f64)
        .collect();

    let n = samples.len();
    if n < 16 {
        return 0.0;
    }

    let mean = samples.iter().sum::<f64>() / n as f64;
    let upper_hz = 4.0_f64.min(sample_rate_hz * 0.45);
    if upper_hz < 0.1 {
        return 0.0;
    }

    let mut powers = Vec::new();
    for bin in 1..=(n / 2) {
        let frequency_hz = bin as f64 * sample_rate_hz / n as f64;
        if !(0.1..=upper_hz).contains(&frequency_hz) {
            continue;
        }

        let mut real = 0.0;
        let mut imag = 0.0;
        let angle_step = 2.0 * std::f64::consts::PI * bin as f64 / n as f64;
        let (step_sin, step_cos) = angle_step.sin_cos();
        let mut angle_cos = 1.0;
        let mut angle_sin = 0.0;

        for (index, sample) in samples.iter().enumerate() {
            let window =
                0.5 * (1.0 - (2.0 * std::f64::consts::PI * index as f64 / (n - 1) as f64).cos());
            let value = (sample - mean) * window;
            real += value * angle_cos;
            imag -= value * angle_sin;
            let next_cos = angle_cos * step_cos - angle_sin * step_sin;
            angle_sin = angle_sin * step_cos + angle_cos * step_sin;
            angle_cos = next_cos;
        }

        powers.push((frequency_hz, real * real + imag * imag));
    }

    if powers.is_empty() {
        return 0.0;
    }

    let mean_power = powers.iter().map(|(_, power)| power).sum::<f64>() / powers.len() as f64;
    let (peak_hz, peak_power) = powers
        .into_iter()
        .max_by(|left, right| {
            left.1
                .partial_cmp(&right.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or((0.0, 0.0));

    if mean_power <= f64::EPSILON || peak_power / mean_power < 2.0 {
        0.0
    } else {
        peak_hz
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovers_temporal_frequency_in_hertz() {
        let sample_rate_hz = 20.0;
        let target_hz = 1.2;
        let mut history = VecDeque::new();
        for sample in 0..100 {
            let t = sample as f64 / sample_rate_hz;
            let value = 20.0 + 2.0 * (2.0 * std::f64::consts::PI * target_hz * t).sin();
            history.push_back(vec![value; 56]);
        }
        let measured = estimate_temporal_dominant_frequency_hz(&history, sample_rate_hz);
        assert!((measured - target_hz).abs() <= 0.21, "measured {measured} Hz");
    }

    #[test]
    fn abstains_on_short_or_flat_history() {
        let short = VecDeque::from(vec![vec![20.0; 56]; 15]);
        assert_eq!(estimate_temporal_dominant_frequency_hz(&short, 20.0), 0.0);
        let flat = VecDeque::from(vec![vec![20.0; 56]; 100]);
        assert_eq!(estimate_temporal_dominant_frequency_hz(&flat, 20.0), 0.0);
        let mut corrupt = flat;
        corrupt[50][4] = f64::NAN;
        assert_eq!(estimate_temporal_dominant_frequency_hz(&corrupt, 20.0), 0.0);
    }
}

