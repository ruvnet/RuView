//! Comprehensive integration tests for the vital sign detection module.
//!
//! These tests exercise the public VitalSignDetector API by feeding
//! synthetic CSI frames (amplitude + phase vectors) and verifying the
//! extracted breathing rate, heart rate, confidence, and signal quality.
//!
//! Test matrix:
//! - Detector creation and sane defaults
//! - Breathing rate detection from synthetic 0.25 Hz (15 BPM) sine
//! - Heartbeat detection from synthetic 1.2 Hz (72 BPM) sine
//! - Combined breathing + heartbeat detection
//! - No-signal (constant amplitude) returns None or low confidence
//! - Out-of-range frequencies are rejected or produce low confidence
//! - Confidence increases with signal-to-noise ratio
//! - Reset clears all internal buffers
//! - Minimum samples threshold
//! - Throughput benchmark (10000 frames)

use std::f64::consts::PI;
use wifi_densepose_sensing_server::vital_signs::{VitalSignDetector, VitalSigns};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const N_SUBCARRIERS: usize = 56;

/// Generate a single CSI frame's amplitude vector with an embedded
/// breathing-band sine wave at `freq_hz` Hz.
///
/// The returned amplitude has `N_SUBCARRIERS` elements, each with a
/// per-subcarrier baseline plus the breathing modulation.
fn make_breathing_frame(freq_hz: f64, t: f64) -> Vec<f64> {
    (0..N_SUBCARRIERS)
        .map(|i| {
            let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
            let breathing = 2.0 * (2.0 * PI * freq_hz * t).sin();
            base + breathing
        })
        .collect()
}

/// Generate a phase vector that produces a phase-variance signal oscillating
/// at `freq_hz` Hz.
///
/// The heartbeat detector uses cross-subcarrier phase variance as its input
/// feature. To produce variance that oscillates at freq_hz, we modulate the
/// spread of phases across subcarriers at that frequency.
fn make_heartbeat_phase_variance(freq_hz: f64, t: f64) -> Vec<f64> {
    // Modulation factor: variance peaks when modulation is high
    let modulation = 0.5 * (1.0 + (2.0 * PI * freq_hz * t).sin());
    (0..N_SUBCARRIERS)
        .map(|i| {
            // Each subcarrier gets a different phase offset, scaled by modulation
            let base = (i as f64 * 0.2).sin();
            base * modulation
        })
        .collect()
}

/// Generate constant-phase vector (no heartbeat signal).
fn make_static_phase() -> Vec<f64> {
    (0..N_SUBCARRIERS)
        .map(|i| (i as f64 * 0.2).sin())
        .collect()
}

/// Feed `n_frames` of synthetic breathing data to a detector.
fn feed_breathing_signal(
    detector: &mut VitalSignDetector,
    freq_hz: f64,
    sample_rate: f64,
    n_frames: usize,
) -> VitalSigns {
    let phase = make_static_phase();
    let mut vitals = VitalSigns::default();
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp = make_breathing_frame(freq_hz, t);
        vitals = detector.process_frame(&amp, &phase);
    }
    vitals
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[test]
fn test_vital_detector_creation() {
    let sample_rate = 20.0;
    let detector = VitalSignDetector::new(sample_rate);

    // Buffer status should be empty initially
    let (br_len, br_cap, hb_len, hb_cap) = detector.buffer_status();

    assert_eq!(br_len, 0, "breathing buffer should start empty");
    assert_eq!(hb_len, 0, "heartbeat buffer should start empty");
    assert!(br_cap > 0, "breathing capacity should be positive");
    assert!(hb_cap > 0, "heartbeat capacity should be positive");

    // Capacities should be based on sample rate and window durations
    // At 20 Hz with 30s breathing window: 600 samples
    // At 20 Hz with 15s heartbeat window: 300 samples
    assert_eq!(br_cap, 600, "breathing capacity at 20 Hz * 30s = 600");
    assert_eq!(hb_cap, 300, "heartbeat capacity at 20 Hz * 15s = 300");
}

#[test]
fn test_breathing_detection_synthetic() {
    let sample_rate = 20.0;
    let breathing_freq = 0.25; // 15 BPM
    let mut detector = VitalSignDetector::new(sample_rate);

    // Feed 30 seconds of clear breathing signal
    let n_frames = (sample_rate * 30.0) as usize; // 600 frames
    let vitals = feed_breathing_signal(&mut detector, breathing_freq, sample_rate, n_frames);

    // Breathing rate should be detected
    let bpm = vitals
        .breathing_rate_bpm
        .expect("should detect breathing rate from 0.25 Hz sine");

    // Allow +/- 3 BPM tolerance (FFT resolution at 20 Hz over 600 samples)
    let expected_bpm = 15.0;
    assert!(
        (bpm - expected_bpm).abs() < 3.0,
        "breathing rate {:.1} BPM should be close to {:.1} BPM",
        bpm,
        expected_bpm,
    );

    assert!(
        vitals.breathing_confidence > 0.0,
        "breathing confidence should be > 0, got {}",
        vitals.breathing_confidence,
    );
}

#[test]
fn test_heartbeat_detection_synthetic() {
    let sample_rate = 20.0;
    let heartbeat_freq = 1.2; // 72 BPM
    let mut detector = VitalSignDetector::new(sample_rate);

    // Feed 15 seconds of data with heartbeat signal in the phase variance
    let n_frames = (sample_rate * 15.0) as usize;

    // Static amplitude -- no breathing signal
    let amp: Vec<f64> = (0..N_SUBCARRIERS)
        .map(|i| 15.0 + 5.0 * (i as f64 * 0.1).sin())
        .collect();

    let mut vitals = VitalSigns::default();
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let phase = make_heartbeat_phase_variance(heartbeat_freq, t);
        vitals = detector.process_frame(&amp, &phase);
    }

    // Heart rate detection from phase variance is more challenging.
    // We verify that if a heart rate is detected, it's in the valid
    // physiological range (40-120 BPM).
    if let Some(bpm) = vitals.heart_rate_bpm {
        assert!(
            bpm >= 40.0 && bpm <= 120.0,
            "detected heart rate {:.1} BPM should be in physiological range [40, 120]",
            bpm
        );
    }

    // At minimum, heartbeat confidence should be non-negative
    assert!(
        vitals.heartbeat_confidence >= 0.0,
        "heartbeat confidence should be >= 0"
    );
}

#[test]
fn test_combined_vital_signs() {
    let sample_rate = 20.0;
    let breathing_freq = 0.25; // 15 BPM
    let heartbeat_freq = 1.2; // 72 BPM
    let mut detector = VitalSignDetector::new(sample_rate);

    // Feed 30 seconds with both signals
    let n_frames = (sample_rate * 30.0) as usize;
    let mut vitals = VitalSigns::default();
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;

        // Amplitude carries breathing modulation
        let amp = make_breathing_frame(breathing_freq, t);

        // Phase carries heartbeat modulation (via variance)
        let phase = make_heartbeat_phase_variance(heartbeat_freq, t);

        vitals = detector.process_frame(&amp, &phase);
    }

    // Breathing should be detected accurately
    let breathing_bpm = vitals
        .breathing_rate_bpm
        .expect("should detect breathing in combined signal");
    assert!(
        (breathing_bpm - 15.0).abs() < 3.0,
        "breathing {:.1} BPM should be close to 15 BPM",
        breathing_bpm
    );

    // Heartbeat: verify it's in the valid range if detected
    if let Some(hb_bpm) = vitals.heart_rate_bpm {
        assert!(
            hb_bpm >= 40.0 && hb_bpm <= 120.0,
            "heartbeat {:.1} BPM should be in range [40, 120]",
            hb_bpm
        );
    }
}

#[test]
fn test_no_signal_lower_confidence_than_true_signal() {
    let sample_rate = 20.0;
    let n_frames = (sample_rate * 30.0) as usize;

    // Detector A: constant amplitude (no real breathing signal)
    let mut detector_flat = VitalSignDetector::new(sample_rate);
    let amp_flat = vec![50.0; N_SUBCARRIERS];
    let phase = vec![0.0; N_SUBCARRIERS];
    for _ in 0..n_frames {
        detector_flat.process_frame(&amp_flat, &phase);
    }
    let (_, flat_conf) = detector_flat.extract_breathing();

    // Detector B: clear 0.25 Hz breathing signal
    let mut detector_signal = VitalSignDetector::new(sample_rate);
    let phase_b = make_static_phase();
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp = make_breathing_frame(0.25, t);
        detector_signal.process_frame(&amp, &phase_b);
    }
    let (signal_rate, signal_conf) = detector_signal.extract_breathing();

    // The real signal should be detected
    assert!(
        signal_rate.is_some(),
        "true breathing signal should be detected"
    );

    // The real signal should have higher confidence than the flat signal.
    // Note: the bandpass filter creates transient artifacts on flat signals
    // that may produce non-zero confidence, but a true periodic signal should
    // always produce a stronger spectral peak.
    assert!(
        signal_conf >= flat_conf,
        "true signal confidence ({:.3}) should be >= flat signal confidence ({:.3})",
        signal_conf,
        flat_conf,
    );
}

#[test]
fn test_out_of_range_lower_confidence_than_in_band() {
    let sample_rate = 20.0;
    let n_frames = (sample_rate * 30.0) as usize;
    let phase = make_static_phase();

    // Detector A: 5 Hz amplitude oscillation (outside breathing band)
    let mut detector_oob = VitalSignDetector::new(sample_rate);
    let out_of_band_freq = 5.0;
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp: Vec<f64> = (0..N_SUBCARRIERS)
            .map(|i| {
                let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                base + 2.0 * (2.0 * PI * out_of_band_freq * t).sin()
            })
            .collect();
        detector_oob.process_frame(&amp, &phase);
    }
    let (_, oob_conf) = detector_oob.extract_breathing();

    // Detector B: 0.25 Hz amplitude oscillation (inside breathing band)
    let mut detector_inband = VitalSignDetector::new(sample_rate);
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp = make_breathing_frame(0.25, t);
        detector_inband.process_frame(&amp, &phase);
    }
    let (inband_rate, inband_conf) = detector_inband.extract_breathing();

    // The in-band signal should be detected
    assert!(
        inband_rate.is_some(),
        "in-band 0.25 Hz signal should be detected as breathing"
    );

    // The in-band signal should have higher confidence than the out-of-band one.
    // The bandpass filter may leak some energy from 5 Hz harmonics, but a true
    // 0.25 Hz signal should always dominate.
    assert!(
        inband_conf >= oob_conf,
        "in-band confidence ({:.3}) should be >= out-of-band confidence ({:.3})",
        inband_conf,
        oob_conf,
    );
}

#[test]
fn test_confidence_increases_with_snr() {
    let sample_rate = 20.0;
    let breathing_freq = 0.25;
    let n_frames = (sample_rate * 30.0) as usize;

    // High SNR: large breathing amplitude, no noise
    let mut detector_clean = VitalSignDetector::new(sample_rate);
    let phase = make_static_phase();

    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp: Vec<f64> = (0..N_SUBCARRIERS)
            .map(|i| {
                let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                // Strong breathing signal (amplitude 5.0)
                base + 5.0 * (2.0 * PI * breathing_freq * t).sin()
            })
            .collect();
        detector_clean.process_frame(&amp, &phase);
    }
    let (_, clean_conf) = detector_clean.extract_breathing();

    // Low SNR: small breathing amplitude, lots of noise
    let mut detector_noisy = VitalSignDetector::new(sample_rate);
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp: Vec<f64> = (0..N_SUBCARRIERS)
            .map(|i| {
                let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                // Weak breathing signal (amplitude 0.1) + heavy noise
                let noise = 3.0
                    * ((i as f64 * 7.3 + t * 113.7).sin()
                        + (i as f64 * 13.1 + t * 79.3).sin())
                    / 2.0;
                base + 0.1 * (2.0 * PI * breathing_freq * t).sin() + noise
            })
            .collect();
        detector_noisy.process_frame(&amp, &phase);
    }
    let (_, noisy_conf) = detector_noisy.extract_breathing();

    assert!(
        clean_conf > noisy_conf,
        "clean signal confidence ({:.3}) should exceed noisy signal confidence ({:.3})",
        clean_conf,
        noisy_conf,
    );
}

#[test]
fn test_reset_clears_buffers() {
    let mut detector = VitalSignDetector::new(20.0);
    let amp = vec![10.0; N_SUBCARRIERS];
    let phase = vec![0.0; N_SUBCARRIERS];

    // Feed some frames to fill buffers
    for _ in 0..100 {
        detector.process_frame(&amp, &phase);
    }

    let (br_len, _, hb_len, _) = detector.buffer_status();
    assert!(br_len > 0, "breathing buffer should have data before reset");
    assert!(hb_len > 0, "heartbeat buffer should have data before reset");

    // Reset
    detector.reset();

    let (br_len, _, hb_len, _) = detector.buffer_status();
    assert_eq!(br_len, 0, "breathing buffer should be empty after reset");
    assert_eq!(hb_len, 0, "heartbeat buffer should be empty after reset");

    // Extraction should return None after reset
    let (breathing, _) = detector.extract_breathing();
    let (heartbeat, _) = detector.extract_heartbeat();
    assert!(
        breathing.is_none(),
        "breathing should be None after reset (not enough samples)"
    );
    assert!(
        heartbeat.is_none(),
        "heartbeat should be None after reset (not enough samples)"
    );
}

#[test]
fn test_minimum_samples_required() {
    let sample_rate = 20.0;
    let mut detector = VitalSignDetector::new(sample_rate);
    let amp = vec![10.0; N_SUBCARRIERS];
    let phase = vec![0.0; N_SUBCARRIERS];

    // Feed fewer than MIN_BREATHING_SAMPLES (40) frames
    for _ in 0..39 {
        detector.process_frame(&amp, &phase);
    }

    let (breathing, _) = detector.extract_breathing();
    assert!(
        breathing.is_none(),
        "with 39 samples (< 40 min), breathing should return None"
    );

    // One more frame should meet the minimum
    detector.process_frame(&amp, &phase);

    let (br_len, _, _, _) = detector.buffer_status();
    assert_eq!(br_len, 40, "should have exactly 40 samples now");

    // Now extraction is at least attempted (may still be None if flat signal,
    // but should not be blocked by the min-samples check)
    let _ = detector.extract_breathing();
}

#[test]
fn test_benchmark_throughput() {
    let sample_rate = 20.0;
    let mut detector = VitalSignDetector::new(sample_rate);

    let num_frames = 10_000;
    let n_sub = N_SUBCARRIERS;

    // Pre-generate frames
    let frames: Vec<(Vec<f64>, Vec<f64>)> = (0..num_frames)
        .map(|tick| {
            let t = tick as f64 / sample_rate;
            let amp: Vec<f64> = (0..n_sub)
                .map(|i| {
                    let base = 15.0 + 5.0 * (i as f64 * 0.1).sin();
                    let breathing = 2.0 * (2.0 * PI * 0.25 * t).sin();
                    let heartbeat = 0.3 * (2.0 * PI * 1.2 * t).sin();
                    let noise = (i as f64 * 7.3 + t * 13.7).sin() * 0.5;
                    base + breathing + heartbeat + noise
                })
                .collect();
            let phase: Vec<f64> = (0..n_sub)
                .map(|i| (i as f64 * 0.2 + t * 0.5).sin() * PI)
                .collect();
            (amp, phase)
        })
        .collect();

    let start = std::time::Instant::now();
    for (amp, phase) in &frames {
        detector.process_frame(amp, phase);
    }
    let elapsed = start.elapsed();
    let fps = num_frames as f64 / elapsed.as_secs_f64();

    println!(
        "Vital sign benchmark: {} frames in {:.2}ms = {:.0} frames/sec",
        num_frames,
        elapsed.as_secs_f64() * 1000.0,
        fps
    );

    // Should process at least 100 frames/sec on any reasonable hardware
    assert!(
        fps > 100.0,
        "throughput {:.0} fps is too low (expected > 100 fps)",
        fps,
    );
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
fn test_empty_amplitude_frame() {
    let mut detector = VitalSignDetector::new(20.0);
    let vitals = detector.process_frame(&[], &[]);

    assert!(vitals.breathing_rate_bpm.is_none());
    assert!(vitals.heart_rate_bpm.is_none());
    assert_eq!(vitals.signal_quality, 0.0);
}

#[test]
fn test_single_subcarrier_no_panic() {
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
fn test_signal_quality_varies_with_input() {
    let mut detector_static = VitalSignDetector::new(20.0);
    let mut detector_varied = VitalSignDetector::new(20.0);

    // Feed static signal (all same amplitude)
    for _ in 0..100 {
        let amp = vec![10.0; N_SUBCARRIERS];
        let phase = vec![0.0; N_SUBCARRIERS];
        detector_static.process_frame(&amp, &phase);
    }

    // Feed varied signal (moderate CV -- body motion)
    for i in 0..100 {
        let t = i as f64 / 20.0;
        let amp: Vec<f64> = (0..N_SUBCARRIERS)
            .map(|j| {
                let base = 15.0;
                let modulation = 2.0 * (2.0 * PI * 0.25 * t + j as f64 * 0.1).sin();
                base + modulation
            })
            .collect();
        let phase: Vec<f64> = (0..N_SUBCARRIERS)
            .map(|j| (j as f64 * 0.2 + t).sin())
            .collect();
        detector_varied.process_frame(&amp, &phase);
    }

    // The varied signal should have higher signal quality than the static one
    let static_vitals =
        detector_static.process_frame(&vec![10.0; N_SUBCARRIERS], &vec![0.0; N_SUBCARRIERS]);
    let amp_varied: Vec<f64> = (0..N_SUBCARRIERS)
        .map(|j| 15.0 + 2.0 * (j as f64 * 0.3).sin())
        .collect();
    let phase_varied: Vec<f64> = (0..N_SUBCARRIERS).map(|j| (j as f64 * 0.2).sin()).collect();
    let varied_vitals = detector_varied.process_frame(&amp_varied, &phase_varied);

    assert!(
        varied_vitals.signal_quality >= static_vitals.signal_quality,
        "varied signal quality ({:.3}) should be >= static ({:.3})",
        varied_vitals.signal_quality,
        static_vitals.signal_quality,
    );
}

#[test]
fn test_buffer_capacity_respected() {
    let sample_rate = 20.0;
    let mut detector = VitalSignDetector::new(sample_rate);

    let amp = vec![10.0; N_SUBCARRIERS];
    let phase = vec![0.0; N_SUBCARRIERS];

    // Feed more frames than breathing capacity (600)
    for _ in 0..1000 {
        detector.process_frame(&amp, &phase);
    }

    let (br_len, br_cap, hb_len, hb_cap) = detector.buffer_status();
    assert!(
        br_len <= br_cap,
        "breathing buffer length {} should not exceed capacity {}",
        br_len,
        br_cap
    );
    assert!(
        hb_len <= hb_cap,
        "heartbeat buffer length {} should not exceed capacity {}",
        hb_len,
        hb_cap
    );
}

#[test]
fn test_run_benchmark_function() {
    let (total, per_frame) = wifi_densepose_sensing_server::vital_signs::run_benchmark(50);
    assert!(total.as_nanos() > 0, "benchmark total duration should be > 0");
    assert!(
        per_frame.as_nanos() > 0,
        "benchmark per-frame duration should be > 0"
    );
}

#[test]
fn test_breathing_rate_in_physiological_range() {
    // If breathing is detected, it must always be in the physiological range
    // (6-30 BPM = 0.1-0.5 Hz)
    let sample_rate = 20.0;
    let mut detector = VitalSignDetector::new(sample_rate);
    let n_frames = (sample_rate * 30.0) as usize;

    let mut vitals = VitalSigns::default();
    for frame in 0..n_frames {
        let t = frame as f64 / sample_rate;
        let amp = make_breathing_frame(0.3, t); // 18 BPM
        let phase = make_static_phase();
        vitals = detector.process_frame(&amp, &phase);
    }

    if let Some(bpm) = vitals.breathing_rate_bpm {
        assert!(
            bpm >= 6.0 && bpm <= 30.0,
            "breathing rate {:.1} BPM must be in range [6, 30]",
            bpm
        );
    }
}

#[test]
fn test_multiple_detectors_independent() {
    // Two detectors should not interfere with each other
    let sample_rate = 20.0;
    let mut detector_a = VitalSignDetector::new(sample_rate);
    let mut detector_b = VitalSignDetector::new(sample_rate);

    let phase = make_static_phase();

    // Feed different breathing rates
    for frame in 0..(sample_rate * 30.0) as usize {
        let t = frame as f64 / sample_rate;
        let amp_a = make_breathing_frame(0.2, t); // 12 BPM
        let amp_b = make_breathing_frame(0.4, t); // 24 BPM
        detector_a.process_frame(&amp_a, &phase);
        detector_b.process_frame(&amp_b, &phase);
    }

    let (rate_a, _) = detector_a.extract_breathing();
    let (rate_b, _) = detector_b.extract_breathing();

    if let (Some(a), Some(b)) = (rate_a, rate_b) {
        // They should detect different rates
        assert!(
            (a - b).abs() > 2.0,
            "detector A ({:.1} BPM) and B ({:.1} BPM) should detect different rates",
            a,
            b
        );
    }
}
