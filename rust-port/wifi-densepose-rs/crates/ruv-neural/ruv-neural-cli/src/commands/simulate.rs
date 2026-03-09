//! Simulate neural sensor data and write to JSON or stdout.

use std::f64::consts::PI;
use std::fs;

use ruv_neural_core::signal::MultiChannelTimeSeries;

/// Run the simulate command.
///
/// Generates synthetic multi-channel neural data with configurable alpha,
/// beta, and gamma oscillations plus realistic noise.
pub fn run(
    channels: usize,
    duration: f64,
    sample_rate: f64,
    output: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let num_samples = (duration * sample_rate) as usize;
    if num_samples == 0 {
        return Err("Duration and sample rate must produce at least one sample".into());
    }

    tracing::info!(
        channels,
        num_samples,
        sample_rate,
        duration,
        "Generating simulated neural data"
    );

    let data = generate_neural_data(channels, num_samples, sample_rate);

    let ts = MultiChannelTimeSeries::new(data.clone(), sample_rate, 0.0).map_err(|e| {
        Box::<dyn std::error::Error>::from(format!("Failed to create time series: {e}"))
    })?;

    // Compute summary statistics.
    let mut channel_rms = Vec::with_capacity(channels);
    for ch in 0..channels {
        let rms = (data[ch].iter().map(|x| x * x).sum::<f64>() / num_samples as f64).sqrt();
        channel_rms.push(rms);
    }
    let mean_rms = channel_rms.iter().sum::<f64>() / channels as f64;

    println!("=== rUv Neural — Simulation Complete ===");
    println!();
    println!("  Channels:      {channels}");
    println!("  Samples:       {num_samples}");
    println!("  Duration:      {duration:.2} s");
    println!("  Sample rate:   {sample_rate:.1} Hz");
    println!("  Mean RMS:      {mean_rms:.4} fT");
    println!();

    // Show frequency content summary.
    println!("  Frequency content:");
    println!("    Alpha (8-13 Hz):   10 Hz sinusoid, 50 fT amplitude");
    println!("    Beta  (13-30 Hz):  20 Hz sinusoid, 30 fT amplitude");
    println!("    Gamma (30-100 Hz): 40 Hz sinusoid, 15 fT amplitude");
    println!("    Noise floor:       ~10 fT/sqrt(Hz) white noise");
    println!();

    match output {
        Some(ref path) => {
            let json = serde_json::to_string_pretty(&ts)?;
            fs::write(path, json)?;
            println!("  Output written to: {path}");
        }
        None => {
            println!("  (Use -o <file> to save output to JSON)");
        }
    }

    Ok(())
}

/// Generate synthetic neural data with realistic oscillations and noise.
fn generate_neural_data(channels: usize, num_samples: usize, sample_rate: f64) -> Vec<Vec<f64>> {
    // Use a deterministic seed based on channel index for reproducibility.
    let mut data = Vec::with_capacity(channels);

    for ch in 0..channels {
        let mut channel_data = Vec::with_capacity(num_samples);
        // Phase offsets vary by channel to simulate spatial diversity.
        let phase_offset = (ch as f64) * PI / (channels as f64);

        // Simple LCG for deterministic pseudo-random noise per channel.
        let mut rng_state: u64 = (ch as u64).wrapping_mul(6364136223846793005).wrapping_add(1);

        for i in 0..num_samples {
            let t = i as f64 / sample_rate;

            // Alpha rhythm: 10 Hz, 50 fT
            let alpha = 50.0 * (2.0 * PI * 10.0 * t + phase_offset).sin();

            // Beta rhythm: 20 Hz, 30 fT
            let beta = 30.0 * (2.0 * PI * 20.0 * t + phase_offset * 1.3).sin();

            // Gamma rhythm: 40 Hz, 15 fT
            let gamma = 15.0 * (2.0 * PI * 40.0 * t + phase_offset * 0.7).sin();

            // White noise (~10 fT/sqrt(Hz) density).
            // Approximate Gaussian via Box-Muller with LCG.
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u1 = (rng_state >> 11) as f64 / (1u64 << 53) as f64;
            rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let u2 = (rng_state >> 11) as f64 / (1u64 << 53) as f64;

            let noise_amplitude = 10.0 * (sample_rate / 2.0).sqrt();
            let gaussian = if u1 > 1e-15 {
                (-2.0 * u1.ln()).sqrt() * (2.0 * PI * u2).cos()
            } else {
                0.0
            };
            let noise = noise_amplitude * gaussian / (num_samples as f64).sqrt() * 0.1;

            channel_data.push(alpha + beta + gamma + noise);
        }

        data.push(channel_data);
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_correct_shape() {
        let data = generate_neural_data(8, 500, 1000.0);
        assert_eq!(data.len(), 8);
        for ch in &data {
            assert_eq!(ch.len(), 500);
        }
    }

    #[test]
    fn simulate_produces_output() {
        let result = run(4, 1.0, 500.0, None);
        assert!(result.is_ok());
    }

    #[test]
    fn simulate_writes_json() {
        let dir = std::env::temp_dir();
        let path = dir.join("ruv_neural_test_sim.json");
        let path_str = path.to_string_lossy().to_string();
        let result = run(2, 0.5, 250.0, Some(path_str.clone()));
        assert!(result.is_ok());
        assert!(path.exists());
        let contents = std::fs::read_to_string(&path).unwrap();
        let _ts: MultiChannelTimeSeries = serde_json::from_str(&contents).unwrap();
        std::fs::remove_file(&path).ok();
    }
}
