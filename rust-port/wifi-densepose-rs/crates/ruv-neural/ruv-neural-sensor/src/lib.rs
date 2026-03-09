//! rUv Neural Sensor -- sensor data acquisition for NV diamond, OPM, EEG,
//! and simulated sources.
//!
//! This crate provides uniform sensor interfaces via the [`SensorSource`] trait
//! from `ruv-neural-core`. Each sensor backend is feature-gated:
//!
//! | Feature       | Module         | Sensor Type                        |
//! |---------------|----------------|------------------------------------|
//! | `simulator`   | [`simulator`]  | Synthetic test data                |
//! | `nv_diamond`  | [`nv_diamond`] | Nitrogen-vacancy diamond magnetometer |
//! | `opm`         | [`opm`]        | Optically pumped magnetometer      |
//! | `eeg`         | [`eeg`]        | Electroencephalography             |
//!
//! The [`calibration`] and [`quality`] modules are always available.

#[cfg(feature = "simulator")]
pub mod simulator;

#[cfg(feature = "nv_diamond")]
pub mod nv_diamond;

#[cfg(feature = "opm")]
pub mod opm;

#[cfg(feature = "eeg")]
pub mod eeg;

pub mod calibration;
pub mod quality;

// Re-exports from core for convenience.
pub use ruv_neural_core::signal::MultiChannelTimeSeries;
pub use ruv_neural_core::traits::SensorSource;
pub use ruv_neural_core::{SensorArray, SensorChannel, SensorType};

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "simulator")]
    #[test]
    fn simulator_produces_correct_shape() {
        let mut sim = simulator::SimulatedSensorArray::new(16, 1000.0);
        let data = sim.read_chunk(500).expect("read_chunk failed");
        assert_eq!(data.num_channels, 16);
        assert_eq!(data.num_samples, 500);
        assert_eq!(data.sample_rate_hz, 1000.0);
    }

    #[cfg(feature = "simulator")]
    #[test]
    fn simulator_sensor_type() {
        let sim = simulator::SimulatedSensorArray::new(8, 500.0);
        assert_eq!(sim.sensor_type(), SensorType::NvDiamond);
    }

    #[cfg(feature = "simulator")]
    #[test]
    fn simulator_alpha_rhythm_frequency() {
        // Generate 2 seconds of data at 1000 Hz to verify alpha peak near 10 Hz.
        let mut sim = simulator::SimulatedSensorArray::new(1, 1000.0);
        sim.inject_alpha(100.0); // 100 fT amplitude
        let data = sim.read_chunk(2000).expect("read_chunk failed");
        let ch = &data.data[0];

        // Simple DFT at the alpha frequency bin.
        let n = ch.len();
        let sample_rate = 1000.0_f64;
        let target_freq = 10.0_f64;
        let bin = (target_freq * n as f64 / sample_rate).round() as usize;

        let power_at = |freq_bin: usize| -> f64 {
            let mut re = 0.0_f64;
            let mut im = 0.0_f64;
            for (t, &val) in ch.iter().enumerate() {
                let angle =
                    -2.0 * std::f64::consts::PI * freq_bin as f64 * t as f64 / n as f64;
                re += val * angle.cos();
                im += val * angle.sin();
            }
            (re * re + im * im).sqrt() / n as f64
        };

        let alpha_power = power_at(bin);
        let noise_bin = (37.0 * n as f64 / sample_rate).round() as usize;
        let noise_power = power_at(noise_bin);

        assert!(
            alpha_power > noise_power * 3.0,
            "Alpha power ({alpha_power}) should be >> noise power ({noise_power})"
        );
    }

    #[cfg(feature = "simulator")]
    #[test]
    fn simulator_noise_floor() {
        let noise_density = 15.0; // fT/sqrt(Hz)
        let sample_rate = 1000.0;
        let mut sim = simulator::SimulatedSensorArray::new(1, sample_rate)
            .with_noise(noise_density);
        let data = sim.read_chunk(10000).expect("read_chunk failed");
        let ch = &data.data[0];
        let rms = (ch.iter().map(|x| x * x).sum::<f64>() / ch.len() as f64).sqrt();

        // Expected RMS = noise_density * sqrt(sample_rate / 2) for white noise.
        let expected_rms = noise_density * (sample_rate / 2.0).sqrt();

        // Allow generous tolerance due to randomness.
        assert!(
            rms > expected_rms * 0.4 && rms < expected_rms * 1.6,
            "RMS {rms} not within tolerance of expected {expected_rms}"
        );
    }

    #[cfg(feature = "simulator")]
    #[test]
    fn simulator_inject_event() {
        let mut sim = simulator::SimulatedSensorArray::new(4, 1000.0);
        sim.inject_event(simulator::SensorEvent::Spike {
            channel: 0,
            amplitude_ft: 500.0,
            sample_offset: 100,
        });
        let data = sim.read_chunk(200).expect("read_chunk failed");
        // The spike should cause a large value near sample 100 in channel 0.
        let ch0 = &data.data[0];
        let max_val = ch0.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            max_val > 400.0,
            "Spike amplitude should be visible, got max {max_val}"
        );
    }

    #[test]
    fn calibration_apply_gain_offset() {
        let cal = calibration::CalibrationData {
            gains: vec![2.0, 0.5],
            offsets: vec![10.0, -5.0],
            noise_floors: vec![1.0, 2.0],
        };
        let corrected = calibration::calibrate_channel(100.0, 0, &cal);
        // (100.0 - 10.0) * 2.0 = 180.0
        assert!((corrected - 180.0).abs() < 1e-10);
    }

    #[test]
    fn calibration_noise_floor_estimate() {
        let quiet = vec![1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
        let nf = calibration::estimate_noise_floor(&quiet);
        // RMS of alternating +/-1 = 1.0
        assert!((nf - 1.0).abs() < 1e-10);
    }

    #[test]
    fn calibration_cross_calibrate() {
        let reference = vec![10.0, 20.0, 30.0, 40.0];
        let target = vec![5.0, 10.0, 15.0, 20.0];
        let (gain, offset) = calibration::cross_calibrate(&reference, &target);
        // target * gain + offset should approximate reference.
        // 5*2+0=10, 10*2+0=20, etc.
        assert!((gain - 2.0).abs() < 1e-10);
        assert!(offset.abs() < 1e-10);
    }

    #[test]
    fn quality_detects_low_snr() {
        let mut monitor = quality::QualityMonitor::new(2);

        // Channel 0: strong signal.
        let good_signal: Vec<f64> = (0..1000)
            .map(|i| 100.0 * (2.0 * std::f64::consts::PI * 10.0 * i as f64 / 1000.0).sin())
            .collect();

        // Channel 1: high-frequency noise (alternating values = maximum first-difference noise).
        let bad_signal: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();

        let qualities = monitor.check_quality(&[&good_signal, &bad_signal]);
        assert_eq!(qualities.len(), 2);
        // Smooth sinusoid should have higher SNR than alternating noise.
        assert!(
            qualities[0].snr_db > qualities[1].snr_db,
            "Good SNR ({}) should be > bad SNR ({})",
            qualities[0].snr_db,
            qualities[1].snr_db,
        );
    }

    #[test]
    fn quality_saturation_detection() {
        let mut monitor = quality::QualityMonitor::new(1);

        // A signal that clips at max value for many samples.
        let saturated: Vec<f64> = (0..1000)
            .map(|i| if i % 2 == 0 { 1e6 } else { -1e6 })
            .collect();

        let qualities = monitor.check_quality(&[&saturated]);
        assert!(qualities[0].saturated);
    }

    #[test]
    fn quality_alert_thresholds() {
        let q_good = quality::SignalQuality {
            snr_db: 10.0,
            artifact_probability: 0.1,
            saturated: false,
        };
        assert!(!q_good.below_threshold());

        let q_bad = quality::SignalQuality {
            snr_db: 2.0,
            artifact_probability: 0.6,
            saturated: false,
        };
        assert!(q_bad.below_threshold());
    }

    #[cfg(feature = "simulator")]
    #[test]
    fn sensor_source_trait_works() {
        let mut sim = simulator::SimulatedSensorArray::new(4, 500.0);
        let source: &mut dyn SensorSource = &mut sim;
        assert_eq!(source.num_channels(), 4);
        assert_eq!(source.sample_rate_hz(), 500.0);
        let data = source.read_chunk(100).expect("read_chunk failed");
        assert_eq!(data.num_channels, 4);
        assert_eq!(data.num_samples, 100);
    }

    #[cfg(feature = "nv_diamond")]
    #[test]
    fn nv_diamond_sensor_source() {
        let config = nv_diamond::NvDiamondConfig::default();
        let mut nv = nv_diamond::NvDiamondArray::new(config);
        assert_eq!(nv.sensor_type(), SensorType::NvDiamond);
        let data = nv.read_chunk(100).expect("read_chunk failed");
        assert_eq!(data.num_channels, nv.num_channels());
    }

    #[cfg(feature = "opm")]
    #[test]
    fn opm_sensor_source() {
        let config = opm::OpmConfig::default();
        let mut opm_arr = opm::OpmArray::new(config);
        assert_eq!(opm_arr.sensor_type(), SensorType::Opm);
        let data = opm_arr.read_chunk(100).expect("read_chunk failed");
        assert_eq!(data.num_channels, opm_arr.num_channels());
    }

    #[cfg(feature = "eeg")]
    #[test]
    fn eeg_sensor_source() {
        let config = eeg::EegConfig::default();
        let mut eeg_arr = eeg::EegArray::new(config);
        assert_eq!(eeg_arr.sensor_type(), SensorType::Eeg);
        let data = eeg_arr.read_chunk(100).expect("read_chunk failed");
        assert_eq!(data.num_channels, eeg_arr.num_channels());
    }
}
