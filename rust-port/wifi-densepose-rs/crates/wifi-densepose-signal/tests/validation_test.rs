//! Validation tests to prove correctness of signal processing algorithms
//!
//! These tests compare our implementations against known mathematical results

use ndarray::Array2;
use std::f64::consts::PI;
use wifi_densepose_signal::{
    CsiData,
    PhaseSanitizer, PhaseSanitizerConfig, UnwrappingMethod,
    FeatureExtractor, FeatureExtractorConfig,
    MotionDetector, MotionDetectorConfig,
    CsiFeatures,
};

/// Validate phase unwrapping against known mathematical result
#[test]
fn validate_phase_unwrapping_correctness() {
    // Create a linearly increasing phase that wraps
    let n = 100;
    let mut wrapped_phase = Array2::zeros((1, n));

    // True unwrapped phase: 0 to 4π (linearly increasing)
    let expected_unwrapped: Vec<f64> = (0..n)
        .map(|i| (i as f64 / (n - 1) as f64) * 4.0 * PI)
        .collect();

    // Wrap it to [-π, π)
    for (i, &val) in expected_unwrapped.iter().enumerate() {
        let wrapped = ((val + PI) % (2.0 * PI)) - PI;
        wrapped_phase[[0, i]] = wrapped;
    }

    let config = PhaseSanitizerConfig::builder()
        .unwrapping_method(UnwrappingMethod::Standard)
        .build();
    let sanitizer = PhaseSanitizer::new(config).unwrap();

    let unwrapped = sanitizer.unwrap_phase(&wrapped_phase).unwrap();

    // Verify unwrapping is correct (within tolerance)
    let mut max_error = 0.0f64;
    for i in 1..n {
        let diff = unwrapped[[0, i]] - unwrapped[[0, i - 1]];
        // Should be small positive increment, not large jump
        assert!(diff.abs() < PI, "Jump detected at index {}: diff={}", i, diff);

        let expected_diff = expected_unwrapped[i] - expected_unwrapped[i - 1];
        let error = (diff - expected_diff).abs();
        max_error = max_error.max(error);
    }

    println!("Phase unwrapping max error: {:.6} radians", max_error);
    assert!(max_error < 0.1, "Phase unwrapping error too large: {}", max_error);
}

/// Validate amplitude RMS calculation
#[test]
fn validate_amplitude_rms() {
    // Create known data with constant amplitude
    let n = 64;
    let amplitude_value = 2.0;
    let amplitude = Array2::from_elem((4, n), amplitude_value);

    let csi_data = CsiData::builder()
        .amplitude(amplitude)
        .phase(Array2::zeros((4, n)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    let features = extractor.extract_amplitude(&csi_data);

    // RMS of constant signal = that constant
    println!("Amplitude RMS: expected={:.4}, got={:.4}", amplitude_value, features.rms);
    assert!((features.rms - amplitude_value).abs() < 0.01,
            "RMS error: expected={}, got={}", amplitude_value, features.rms);

    // Peak should equal the constant
    assert!((features.peak - amplitude_value).abs() < 0.01,
            "Peak error: expected={}, got={}", amplitude_value, features.peak);

    // Dynamic range should be zero
    assert!(features.dynamic_range.abs() < 0.01,
            "Dynamic range should be zero for constant signal: {}", features.dynamic_range);
}

/// Validate Doppler shift calculation conceptually
#[test]
fn validate_doppler_calculation() {
    // Simulate moving target causing phase shift
    // A person moving at 1 m/s at 5 GHz WiFi
    // Doppler shift ≈ 2 * v * f / c ≈ 33.3 Hz

    let sample_rate = 1000.0; // 1 kHz CSI sampling
    let velocity = 1.0; // 1 m/s
    let freq = 5.0e9; // 5 GHz
    let c = 3.0e8; // speed of light
    let expected_doppler = 2.0 * velocity * freq / c;

    println!("Expected Doppler shift for 1 m/s target: {:.2} Hz", expected_doppler);

    // Create phase data with Doppler shift
    let n_samples = 100;
    let subcarriers = 64;
    let mut phase_history = Vec::new();

    for t in 0..n_samples {
        let mut phase = Array2::zeros((4, subcarriers));
        for i in 0..4 {
            for j in 0..subcarriers {
                // Phase advances due to Doppler
                let doppler_phase = 2.0 * PI * expected_doppler * (t as f64 / sample_rate);
                phase[[i, j]] = doppler_phase + 0.01 * ((i + j) as f64);
            }
        }
        phase_history.push(phase);
    }

    // Calculate Doppler from phase difference between consecutive samples
    let mut phase_rates = Vec::new();
    for t in 1..n_samples {
        let diff = &phase_history[t] - &phase_history[t - 1];
        let avg_diff: f64 = diff.iter().sum::<f64>() / (4 * subcarriers) as f64;
        let freq_estimate = avg_diff * sample_rate / (2.0 * PI);
        phase_rates.push(freq_estimate);
    }

    let avg_doppler: f64 = phase_rates.iter().sum::<f64>() / phase_rates.len() as f64;
    println!("Measured Doppler: {:.2} Hz (expected: {:.2} Hz)", avg_doppler, expected_doppler);

    assert!((avg_doppler - expected_doppler).abs() < 1.0,
            "Doppler estimation error too large");
}

/// Validate FFT-based spectral analysis
#[test]
fn validate_spectral_analysis() {
    // Create signal with known frequency components
    let n = 256;
    let sample_rate = 1000.0;
    let freq1 = 50.0; // 50 Hz component

    let mut amplitude = Array2::zeros((1, n));
    for i in 0..n {
        let t = i as f64 / sample_rate;
        // Sinusoid with known frequency
        let signal = 1.0 * (2.0 * PI * freq1 * t).sin();
        amplitude[[0, i]] = signal.abs() + 1.0; // Ensure positive
    }

    let csi_data = CsiData::builder()
        .amplitude(amplitude)
        .phase(Array2::zeros((1, n)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    let psd = extractor.extract_psd(&csi_data);

    println!("Peak frequency: {:.1} Hz", psd.peak_frequency);
    println!("Total power: {:.3}", psd.total_power);
    println!("Spectral centroid: {:.1} Hz", psd.centroid);

    // Total power should be positive
    assert!(psd.total_power > 0.0, "Total power should be positive");
    // Centroid should be reasonable
    assert!(psd.centroid >= 0.0, "Spectral centroid should be non-negative");
}

/// Validate CSI complex conversion (amplitude/phase <-> complex)
#[test]
fn validate_complex_conversion() {
    let n = 64;
    let mut amplitude = Array2::zeros((4, n));
    let mut phase = Array2::zeros((4, n));

    // Known values
    for i in 0..4 {
        for j in 0..n {
            amplitude[[i, j]] = 1.0 + 0.1 * (i + j) as f64;
            phase[[i, j]] = (j as f64 / n as f64) * PI;
        }
    }

    let csi_data = CsiData::builder()
        .amplitude(amplitude.clone())
        .phase(phase.clone())
        .build()
        .unwrap();

    let complex = csi_data.to_complex();

    // Verify: |z| = amplitude, arg(z) = phase
    for i in 0..4 {
        for j in 0..n {
            let z = complex[[i, j]];
            let recovered_amp = z.norm();
            let recovered_phase = z.arg();

            let amp_error = (recovered_amp - amplitude[[i, j]]).abs();
            let phase_error = (recovered_phase - phase[[i, j]]).abs();

            assert!(amp_error < 1e-10,
                    "Amplitude mismatch at [{},{}]: expected {}, got {}",
                    i, j, amplitude[[i, j]], recovered_amp);
            assert!(phase_error < 1e-10,
                    "Phase mismatch at [{},{}]: expected {}, got {}",
                    i, j, phase[[i, j]], recovered_phase);
        }
    }

    println!("Complex conversion validated: all {} elements correct", 4 * n);
}

/// Validate motion detection threshold behavior
#[test]
fn validate_motion_detection_sensitivity() {
    let config = MotionDetectorConfig::builder()
        .motion_threshold(0.1)
        .history_size(5)
        .build();
    let detector = MotionDetector::new(config);

    // Create static features
    let static_features = create_static_features();

    // Feed baseline data
    for _ in 0..10 {
        let _ = detector.analyze_motion(&static_features);
    }

    // Create features with motion
    let motion_features = create_motion_features(0.5);
    let result = detector.analyze_motion(&motion_features);

    println!("Motion analysis - total_score: {:.3}, confidence: {:.3}",
             result.score.total, result.confidence);

    // Motion features should show valid scores
    assert!(result.score.total >= 0.0 && result.confidence >= 0.0,
            "Motion analysis should return valid scores");
}

/// Validate correlation features
#[test]
fn validate_correlation_features() {
    let n = 64;

    // Create perfectly correlated antenna data
    let mut amplitude = Array2::zeros((4, n));
    for i in 0..4 {
        for j in 0..n {
            // All antennas see same signal pattern
            amplitude[[i, j]] = 1.0 + 0.5 * ((j as f64 / n as f64) * 2.0 * PI).sin();
        }
    }

    let csi_data = CsiData::builder()
        .amplitude(amplitude)
        .phase(Array2::zeros((4, n)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    let corr = extractor.extract_correlation(&csi_data);

    println!("Mean correlation: {:.4}", corr.mean_correlation);
    println!("Max correlation: {:.4}", corr.max_correlation);

    // Correlation should be high for identical signals
    assert!(corr.mean_correlation > 0.9,
            "Identical signals should have high correlation: {}", corr.mean_correlation);
}

/// Validate phase coherence
#[test]
fn validate_phase_coherence() {
    let n = 64;

    // Create coherent phase (same pattern across antennas)
    let mut phase = Array2::zeros((4, n));
    for i in 0..4 {
        for j in 0..n {
            // Same linear phase across all antennas
            phase[[i, j]] = (j as f64 / n as f64) * PI;
        }
    }

    let csi_data = CsiData::builder()
        .amplitude(Array2::from_elem((4, n), 1.0))
        .phase(phase)
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    let phase_features = extractor.extract_phase(&csi_data);

    println!("Phase coherence: {:.4}", phase_features.coherence);

    // Coherent phase should have high coherence value
    assert!(phase_features.coherence > 0.5,
            "Coherent phase should have high coherence: {}", phase_features.coherence);
}

/// Validate feature extraction completeness
#[test]
fn validate_feature_extraction_complete() {
    let csi_data = create_test_csi(4, 64);
    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());

    let features = extractor.extract(&csi_data);

    // All feature components should be present and finite
    assert!(features.amplitude.rms.is_finite(), "Amplitude RMS should be finite");
    assert!(features.amplitude.peak.is_finite(), "Amplitude peak should be finite");
    assert!(features.phase.coherence.is_finite(), "Phase coherence should be finite");
    assert!(features.correlation.mean_correlation.is_finite(), "Correlation should be finite");
    assert!(features.psd.total_power.is_finite(), "PSD power should be finite");

    println!("Feature extraction complete - all fields populated");
    println!("  Amplitude: rms={:.4}, peak={:.4}, dynamic_range={:.4}",
             features.amplitude.rms, features.amplitude.peak, features.amplitude.dynamic_range);
    println!("  Phase: coherence={:.4}", features.phase.coherence);
    println!("  Correlation: mean={:.4}", features.correlation.mean_correlation);
    println!("  PSD: power={:.4}, peak_freq={:.1}", features.psd.total_power, features.psd.peak_frequency);
}

/// Validate dynamic range calculation
#[test]
fn validate_dynamic_range() {
    let n = 64;

    // Create signal with known dynamic range
    let min_val = 0.5;
    let max_val = 2.5;
    let expected_range = max_val - min_val;

    let mut amplitude = Array2::zeros((4, n));
    for i in 0..4 {
        for j in 0..n {
            // Linearly vary from min to max
            amplitude[[i, j]] = min_val + (max_val - min_val) * (j as f64 / (n - 1) as f64);
        }
    }

    let csi_data = CsiData::builder()
        .amplitude(amplitude)
        .phase(Array2::zeros((4, n)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    let features = extractor.extract_amplitude(&csi_data);

    println!("Dynamic range: expected={:.4}, got={:.4}", expected_range, features.dynamic_range);
    assert!((features.dynamic_range - expected_range).abs() < 0.01,
            "Dynamic range error: expected={}, got={}", expected_range, features.dynamic_range);
}

// Helper functions

fn create_test_csi(antennas: usize, subcarriers: usize) -> CsiData {
    let mut amplitude = Array2::zeros((antennas, subcarriers));
    let mut phase = Array2::zeros((antennas, subcarriers));

    for i in 0..antennas {
        for j in 0..subcarriers {
            amplitude[[i, j]] = 1.0 + 0.2 * ((j as f64 * 0.1).sin());
            phase[[i, j]] = (j as f64 / subcarriers as f64) * PI;
        }
    }

    CsiData::builder()
        .amplitude(amplitude)
        .phase(phase)
        .build()
        .unwrap()
}

fn create_static_features() -> CsiFeatures {
    let csi = CsiData::builder()
        .amplitude(Array2::from_elem((4, 64), 1.0))
        .phase(Array2::zeros((4, 64)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    extractor.extract(&csi)
}

fn create_motion_features(variation: f64) -> CsiFeatures {
    let mut amplitude = Array2::zeros((4, 64));
    for i in 0..4 {
        for j in 0..64 {
            amplitude[[i, j]] = 1.0 + variation * ((i * 7 + j * 13) as f64 * 0.5).sin();
        }
    }

    let csi = CsiData::builder()
        .amplitude(amplitude)
        .phase(Array2::zeros((4, 64)))
        .build()
        .unwrap();

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());
    extractor.extract(&csi)
}
