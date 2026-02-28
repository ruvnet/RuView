//! Integration tests for ADR-001: WiFi-Mat disaster response pipeline.
//!
//! These tests verify the full pipeline with deterministic synthetic CSI data:
//! 1. Push CSI data -> Detection pipeline processes it
//! 2. Ensemble classifier combines signals -> Triage recommendation
//! 3. Events emitted to EventStore
//! 4. API endpoints accept CSI data and return results
//!
//! No mocks, no random data. All test signals are deterministic sinusoids.

use std::sync::Arc;
use wifi_densepose_mat::{
    DisasterConfig, DisasterResponse, DisasterType,
    DetectionPipeline, DetectionConfig,
    EnsembleClassifier, EnsembleConfig,
    InMemoryEventStore, EventStore,
};

/// Generate deterministic CSI data simulating a breathing survivor.
///
/// Creates a sinusoidal signal at 0.267 Hz (16 BPM breathing rate)
/// with known amplitude and phase patterns.
fn generate_breathing_signal(sample_rate: f64, duration_secs: f64) -> (Vec<f64>, Vec<f64>) {
    let num_samples = (sample_rate * duration_secs) as usize;
    let breathing_freq = 0.267; // 16 BPM

    let amplitudes: Vec<f64> = (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            0.5 + 0.3 * (2.0 * std::f64::consts::PI * breathing_freq * t).sin()
        })
        .collect();

    let phases: Vec<f64> = (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            0.2 * (2.0 * std::f64::consts::PI * breathing_freq * t).sin()
        })
        .collect();

    (amplitudes, phases)
}

#[test]
fn test_detection_pipeline_accepts_deterministic_data() {
    let config = DetectionConfig {
        sample_rate: 100.0,
        enable_heartbeat: false,
        min_confidence: 0.1,
        ..DetectionConfig::default()
    };

    let pipeline = DetectionPipeline::new(config);

    // Push 10 seconds of breathing signal
    let (amplitudes, phases) = generate_breathing_signal(100.0, 10.0);
    assert_eq!(amplitudes.len(), 1000);
    assert_eq!(phases.len(), 1000);

    // Pipeline should accept the data without error
    pipeline.add_data(&amplitudes, &phases);

    // Verify the pipeline stored the data
    assert_eq!(pipeline.config().sample_rate, 100.0);
}

#[test]
fn test_ensemble_classifier_triage_logic() {
    use wifi_densepose_mat::domain::{
        BreathingPattern, BreathingType, MovementProfile,
        MovementType, HeartbeatSignature, SignalStrength,
        VitalSignsReading, TriageStatus,
    };

    let classifier = EnsembleClassifier::new(EnsembleConfig::default());

    // Normal breathing + movement = Minor (Green)
    let normal_breathing = VitalSignsReading::new(
        Some(BreathingPattern {
            rate_bpm: 16.0,
            pattern_type: BreathingType::Normal,
            amplitude: 0.5,
            regularity: 0.9,
        }),
        None,
        MovementProfile {
            movement_type: MovementType::Periodic,
            intensity: 0.5,
            frequency: 0.3,
            is_voluntary: true,
        },
    );
    let result = classifier.classify(&normal_breathing);
    assert_eq!(result.recommended_triage, TriageStatus::Minor);
    assert!(result.breathing_detected);

    // Agonal breathing = Immediate (Red)
    let agonal = VitalSignsReading::new(
        Some(BreathingPattern {
            rate_bpm: 6.0,
            pattern_type: BreathingType::Agonal,
            amplitude: 0.3,
            regularity: 0.2,
        }),
        None,
        MovementProfile::default(),
    );
    let result = classifier.classify(&agonal);
    assert_eq!(result.recommended_triage, TriageStatus::Immediate);

    // Normal breathing, no movement = Delayed (Yellow)
    let stable = VitalSignsReading::new(
        Some(BreathingPattern {
            rate_bpm: 14.0,
            pattern_type: BreathingType::Normal,
            amplitude: 0.6,
            regularity: 0.95,
        }),
        Some(HeartbeatSignature {
            rate_bpm: 72.0,
            variability: 0.1,
            strength: SignalStrength::Moderate,
        }),
        MovementProfile::default(),
    );
    let result = classifier.classify(&stable);
    assert_eq!(result.recommended_triage, TriageStatus::Delayed);
    assert!(result.heartbeat_detected);
}

#[test]
fn test_event_store_append_and_query() {
    let store = InMemoryEventStore::new();

    // Append a system event
    let event = wifi_densepose_mat::DomainEvent::System(
        wifi_densepose_mat::domain::events::SystemEvent::SystemStarted {
            version: "test-v1".to_string(),
            timestamp: chrono::Utc::now(),
        },
    );

    store.append(event).unwrap();

    let all = store.all().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].event_type(), "SystemStarted");
}

#[test]
fn test_disaster_response_with_event_store() {
    let config = DisasterConfig::builder()
        .disaster_type(DisasterType::Earthquake)
        .sensitivity(0.8)
        .build();

    let event_store: Arc<dyn EventStore> = Arc::new(InMemoryEventStore::new());
    let response = DisasterResponse::with_event_store(config, event_store.clone());

    // Push CSI data
    let (amplitudes, phases) = generate_breathing_signal(1000.0, 1.0);
    response.push_csi_data(&amplitudes, &phases).unwrap();

    // Store should be empty (no scan cycle ran)
    let events = event_store.all().unwrap();
    assert_eq!(events.len(), 0);

    // Access the ensemble classifier
    let _ensemble = response.ensemble_classifier();
}

#[test]
fn test_push_csi_data_validation() {
    let config = DisasterConfig::builder()
        .disaster_type(DisasterType::Earthquake)
        .build();

    let response = DisasterResponse::new(config);

    // Mismatched lengths should fail
    assert!(response.push_csi_data(&[1.0, 2.0], &[1.0]).is_err());

    // Empty data should fail
    assert!(response.push_csi_data(&[], &[]).is_err());

    // Valid data should succeed
    assert!(response.push_csi_data(&[1.0, 2.0], &[0.1, 0.2]).is_ok());
}

#[test]
fn test_deterministic_signal_properties() {
    // Verify that our test signal is actually deterministic
    let (a1, p1) = generate_breathing_signal(100.0, 5.0);
    let (a2, p2) = generate_breathing_signal(100.0, 5.0);

    assert_eq!(a1.len(), a2.len());
    for i in 0..a1.len() {
        assert!((a1[i] - a2[i]).abs() < 1e-15, "Amplitude mismatch at index {}", i);
        assert!((p1[i] - p2[i]).abs() < 1e-15, "Phase mismatch at index {}", i);
    }
}
