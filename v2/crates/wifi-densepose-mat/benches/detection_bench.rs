//! Performance benchmarks for wifi-densepose-mat detection algorithms.
//!
//! Run with: cargo bench --package wifi-densepose-mat
//!
//! Benchmarks cover:
//! - Breathing detection at various signal lengths
//! - Heartbeat detection performance
//! - Movement classification
//! - Full detection pipeline
//! - Localization algorithms (triangulation, depth estimation)
//! - Alert generation

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::f64::consts::PI;

use wifi_densepose_mat::{
    // Detection types
    BreathingDetector, BreathingDetectorConfig,
    HeartbeatDetector, HeartbeatDetectorConfig,
    MovementClassifier, MovementClassifierConfig,
    DetectionConfig, DetectionPipeline, VitalSignsDetector,
    // Localization types
    Triangulator, DepthEstimator,
    // Alerting types
    AlertGenerator,
    // Domain types exported at crate root
    BreathingPattern, BreathingType, VitalSignsReading,
    MovementProfile, ScanZoneId, Survivor,
};

// Types that need to be accessed from submodules
use wifi_densepose_mat::detection::CsiDataBuffer;
use wifi_densepose_mat::domain::{
    ConfidenceScore, SensorPosition, SensorType,
    DebrisProfile, DebrisMaterial, MoistureLevel, MetalContent,
};

use chrono::Utc;

// =============================================================================
// Test Data Generators
// =============================================================================

/// Generate a clean breathing signal at specified rate
fn generate_breathing_signal(rate_bpm: f64, sample_rate: f64, duration_secs: f64) -> Vec<f64> {
    let num_samples = (sample_rate * duration_secs) as usize;
    let freq = rate_bpm / 60.0;

    (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            (2.0 * PI * freq * t).sin()
        })
        .collect()
}

/// Generate a breathing signal with noise
fn generate_noisy_breathing_signal(
    rate_bpm: f64,
    sample_rate: f64,
    duration_secs: f64,
    noise_level: f64,
) -> Vec<f64> {
    let num_samples = (sample_rate * duration_secs) as usize;
    let freq = rate_bpm / 60.0;

    (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            let signal = (2.0 * PI * freq * t).sin();
            // Simple pseudo-random noise based on sample index
            let noise = ((i as f64 * 12345.6789).sin() * 2.0 - 1.0) * noise_level;
            signal + noise
        })
        .collect()
}

/// Generate heartbeat signal with micro-Doppler characteristics
fn generate_heartbeat_signal(rate_bpm: f64, sample_rate: f64, duration_secs: f64) -> Vec<f64> {
    let num_samples = (sample_rate * duration_secs) as usize;
    let freq = rate_bpm / 60.0;

    (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            let phase = 2.0 * PI * freq * t;
            // Heartbeat is more pulse-like than sinusoidal
            0.3 * phase.sin() + 0.1 * (2.0 * phase).sin() + 0.05 * (3.0 * phase).sin()
        })
        .collect()
}

/// Generate combined breathing + heartbeat signal
fn generate_combined_vital_signal(
    breathing_rate: f64,
    heart_rate: f64,
    sample_rate: f64,
    duration_secs: f64,
) -> (Vec<f64>, Vec<f64>) {
    let num_samples = (sample_rate * duration_secs) as usize;
    let br_freq = breathing_rate / 60.0;
    let hr_freq = heart_rate / 60.0;

    let amplitudes: Vec<f64> = (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            // Breathing dominates amplitude
            (2.0 * PI * br_freq * t).sin()
        })
        .collect();

    let phases: Vec<f64> = (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            // Phase captures both but heartbeat is more prominent
            let breathing = 0.3 * (2.0 * PI * br_freq * t).sin();
            let heartbeat = 0.5 * (2.0 * PI * hr_freq * t).sin();
            breathing + heartbeat
        })
        .collect();

    (amplitudes, phases)
}

/// Generate multi-person scenario with overlapping signals
fn generate_multi_person_signal(
    person_count: usize,
    sample_rate: f64,
    duration_secs: f64,
) -> Vec<f64> {
    let num_samples = (sample_rate * duration_secs) as usize;

    // Different breathing rates for each person
    let base_rates: Vec<f64> = (0..person_count)
        .map(|i| 12.0 + (i as f64 * 3.5)) // 12, 15.5, 19, 22.5... BPM
        .collect();

    (0..num_samples)
        .map(|i| {
            let t = i as f64 / sample_rate;
            base_rates.iter()
                .enumerate()
                .map(|(idx, &rate)| {
                    let freq = rate / 60.0;
                    let amplitude = 1.0 / (idx + 1) as f64; // Distance-based attenuation
                    let phase_offset = idx as f64 * PI / 4.0; // Different phases
                    amplitude * (2.0 * PI * freq * t + phase_offset).sin()
                })
                .sum::<f64>()
        })
        .collect()
}

/// Generate movement signal with specified characteristics
fn generate_movement_signal(
    movement_type: &str,
    sample_rate: f64,
    duration_secs: f64,
) -> Vec<f64> {
    let num_samples = (sample_rate * duration_secs) as usize;

    match movement_type {
        "gross" => {
            // Large, irregular movements
            let mut signal = vec![0.0; num_samples];
            for i in (num_samples / 4)..(num_samples / 2) {
                signal[i] = 2.0;
            }
            for i in (3 * num_samples / 4)..(4 * num_samples / 5) {
                signal[i] = -1.5;
            }
            signal
        }
        "tremor" => {
            // High-frequency tremor (8-12 Hz)
            (0..num_samples)
                .map(|i| {
                    let t = i as f64 / sample_rate;
                    0.3 * (2.0 * PI * 10.0 * t).sin()
                })
                .collect()
        }
        "periodic" => {
            // Low-frequency periodic (breathing-like)
            (0..num_samples)
                .map(|i| {
                    let t = i as f64 / sample_rate;
                    0.5 * (2.0 * PI * 0.25 * t).sin()
                })
                .collect()
        }
        _ => vec![0.0; num_samples], // No movement
    }
}

/// Create test sensor positions in a triangular configuration
fn create_test_sensors(count: usize) -> Vec<SensorPosition> {
    (0..count)
        .map(|i| {
            let angle = 2.0 * PI * i as f64 / count as f64;
            SensorPosition {
                id: format!("sensor_{}", i),
                x: 10.0 * angle.cos(),
                y: 10.0 * angle.sin(),
                z: 1.5,
                sensor_type: SensorType::Transceiver,
                is_operational: true,
            }
        })
        .collect()
}

/// Create test debris profile
fn create_test_debris() -> DebrisProfile {
    DebrisProfile {
        primary_material: DebrisMaterial::Mixed,
        void_fraction: 0.25,
        moisture_content: MoistureLevel::Dry,
        metal_content: MetalContent::Low,
    }
}

/// Create test survivor for alert generation
fn create_test_survivor() -> Survivor {
    let vitals = VitalSignsReading {
        breathing: Some(BreathingPattern {
            rate_bpm: 18.0,
            amplitude: 0.8,
            regularity: 0.9,
            pattern_type: BreathingType::Normal,
        }),
        heartbeat: None,
        movement: MovementProfile::default(),
        timestamp: Utc::now(),
        confidence: ConfidenceScore::new(0.85),
    };

    Survivor::new(ScanZoneId::new(), vitals, None)
}

// =============================================================================
// Breathing Detection Benchmarks
// =============================================================================

fn bench_breathing_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("breathing_detection");

    let detector = BreathingDetector::with_defaults();
    let sample_rate = 100.0; // 100 Hz

    // Benchmark different signal lengths
    for duration in [5.0, 10.0, 30.0, 60.0] {
        let signal = generate_breathing_signal(16.0, sample_rate, duration);
        let num_samples = signal.len();

        group.throughput(Throughput::Elements(num_samples as u64));
        group.bench_with_input(
            BenchmarkId::new("clean_signal", format!("{}s", duration as u32)),
            &signal,
            |b, signal| {
                b.iter(|| detector.detect(black_box(signal), black_box(sample_rate)))
            },
        );
    }

    // Benchmark different noise levels
    for noise_level in [0.0, 0.1, 0.3, 0.5] {
        let signal = generate_noisy_breathing_signal(16.0, sample_rate, 30.0, noise_level);

        group.bench_with_input(
            BenchmarkId::new("noisy_signal", format!("noise_{}", (noise_level * 10.0) as u32)),
            &signal,
            |b, signal| {
                b.iter(|| detector.detect(black_box(signal), black_box(sample_rate)))
            },
        );
    }

    // Benchmark different breathing rates
    for rate in [8.0, 16.0, 25.0, 35.0] {
        let signal = generate_breathing_signal(rate, sample_rate, 30.0);

        group.bench_with_input(
            BenchmarkId::new("rate_variation", format!("{}bpm", rate as u32)),
            &signal,
            |b, signal| {
                b.iter(|| detector.detect(black_box(signal), black_box(sample_rate)))
            },
        );
    }

    // Benchmark with custom config (high sensitivity)
    let high_sensitivity_config = BreathingDetectorConfig {
        min_rate_bpm: 2.0,
        max_rate_bpm: 50.0,
        min_amplitude: 0.05,
        window_size: 1024,
        window_overlap: 0.75,
        confidence_threshold: 0.2,
    };
    let sensitive_detector = BreathingDetector::new(high_sensitivity_config);
    let signal = generate_noisy_breathing_signal(16.0, sample_rate, 30.0, 0.3);

    group.bench_with_input(
        BenchmarkId::new("high_sensitivity", "30s_noisy"),
        &signal,
        |b, signal| {
            b.iter(|| sensitive_detector.detect(black_box(signal), black_box(sample_rate)))
        },
    );

    group.finish();
}

// =============================================================================
// Heartbeat Detection Benchmarks
// =============================================================================

fn bench_heartbeat_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("heartbeat_detection");

    let detector = HeartbeatDetector::with_defaults();
    let sample_rate = 1000.0; // 1 kHz for micro-Doppler

    // Benchmark different signal lengths
    for duration in [5.0, 10.0, 30.0] {
        let signal = generate_heartbeat_signal(72.0, sample_rate, duration);
        let num_samples = signal.len();

        group.throughput(Throughput::Elements(num_samples as u64));
        group.bench_with_input(
            BenchmarkId::new("clean_signal", format!("{}s", duration as u32)),
            &signal,
            |b, signal| {
                b.iter(|| detector.detect(black_box(signal), black_box(sample_rate), None))
            },
        );
    }

    // Benchmark with known breathing rate (improves filtering)
    let signal = generate_heartbeat_signal(72.0, sample_rate, 30.0);
    group.bench_with_input(
        BenchmarkId::new("with_breathing_rate", "72bpm_known_br"),
        &signal,
        |b, signal| {
            b.iter(|| {
                detector.detect(
                    black_box(signal),
                    black_box(sample_rate),
                    black_box(Some(16.0)), // Known breathing rate
                )
            })
        },
    );

    // Benchmark different heart rates
    for rate in [50.0, 72.0, 100.0, 150.0] {
        let signal = generate_heartbeat_signal(rate, sample_rate, 10.0);

        group.bench_with_input(
            BenchmarkId::new("rate_variation", format!("{}bpm", rate as u32)),
            &signal,
            |b, signal| {
                b.iter(|| detector.detect(black_box(signal), black_box(sample_rate), None))
            },
        );
    }

    // Benchmark enhanced processing config
    let enhanced_config = HeartbeatDetectorConfig {
        min_rate_bpm: 30.0,
        max_rate_bpm: 200.0,
        min_signal_strength: 0.02,
        window_size: 2048,
        enhanced_processing: true,
        confidence_threshold: 0.3,
    };
    let enhanced_detector = HeartbeatDetector::new(enhanced_config);
    let signal = generate_heartbeat_signal(72.0, sample_rate, 10.0);

    group.bench_with_input(
        BenchmarkId::new("enhanced_processing", "2048_window"),
        &signal,
        |b, signal| {
            b.iter(|| enhanced_detector.detect(black_box(signal), black_box(sample_rate), None))
        },
    );

    group.finish();
}

// =============================================================================
// Movement Classification Benchmarks
// =============================================================================

fn bench_movement_classification(c: &mut Criterion) {
    let mut group = c.benchmark_group("movement_classification");

    let classifier = MovementClassifier::with_defaults();
    let sample_rate = 100.0;

    // Benchmark different movement types
    for movement_type in ["none", "gross", "tremor", "periodic"] {
        let signal = generate_movement_signal(movement_type, sample_rate, 10.0);
        let num_samples = signal.len();

        group.throughput(Throughput::Elements(num_samples as u64));
        group.bench_with_input(
            BenchmarkId::new("movement_type", movement_type),
            &signal,
            |b, signal| {
                b.iter(|| classifier.classify(black_box(signal), black_box(sample_rate)))
            },
        );
    }

    // Benchmark different signal lengths
    for duration in [2.0, 5.0, 10.0, 30.0] {
        let signal = generate_movement_signal("gross", sample_rate, duration);

        group.bench_with_input(
            BenchmarkId::new("signal_length", format!("{}s", duration as u32)),
            &signal,
            |b, signal| {
                b.iter(|| classifier.classify(black_box(signal), black_box(sample_rate)))
            },
        );
    }

    // Benchmark with custom sensitivity
    let sensitive_config = MovementClassifierConfig {
        movement_threshold: 0.05,
        gross_movement_threshold: 0.3,
        window_size: 200,
        periodicity_threshold: 0.2,
    };
    let sensitive_classifier = MovementClassifier::new(sensitive_config);
    let signal = generate_movement_signal("tremor", sample_rate, 10.0);

    group.bench_with_input(
        BenchmarkId::new("high_sensitivity", "tremor_detection"),
        &signal,
        |b, signal| {
            b.iter(|| sensitive_classifier.classify(black_box(signal), black_box(sample_rate)))
        },
    );

    group.finish();
}

// =============================================================================
// Full Detection Pipeline Benchmarks
// =============================================================================

fn bench_detection_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_pipeline");
    group.sample_size(50); // Reduce sample size for slower benchmarks

    let sample_rate = 100.0;

    // Standard pipeline (breathing + movement)
    let standard_config = DetectionConfig {
        sample_rate,
        enable_heartbeat: false,
        min_confidence: 0.3,
        ..Default::default()
    };
    let standard_pipeline = DetectionPipeline::new(standard_config);

    // Full pipeline (breathing + heartbeat + movement)
    let full_config = DetectionConfig {
        sample_rate: 1000.0,
        enable_heartbeat: true,
        min_confidence: 0.3,
        ..Default::default()
    };
    let full_pipeline = DetectionPipeline::new(full_config);

    // Benchmark standard pipeline at different data sizes
    for duration in [5.0, 10.0, 30.0] {
        let (amplitudes, phases) = generate_combined_vital_signal(16.0, 72.0, sample_rate, duration);
        let mut buffer = CsiDataBuffer::new(sample_rate);
        buffer.add_samples(&amplitudes, &phases);

        group.throughput(Throughput::Elements(amplitudes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("standard_pipeline", format!("{}s", duration as u32)),
            &buffer,
            |b, buffer| {
                b.iter(|| standard_pipeline.detect(black_box(buffer)))
            },
        );
    }

    // Benchmark full pipeline
    for duration in [5.0, 10.0] {
        let (amplitudes, phases) = generate_combined_vital_signal(16.0, 72.0, 1000.0, duration);
        let mut buffer = CsiDataBuffer::new(1000.0);
        buffer.add_samples(&amplitudes, &phases);

        group.bench_with_input(
            BenchmarkId::new("full_pipeline", format!("{}s", duration as u32)),
            &buffer,
            |b, buffer| {
                b.iter(|| full_pipeline.detect(black_box(buffer)))
            },
        );
    }

    // Benchmark multi-person scenarios
    for person_count in [1, 2, 3, 5] {
        let signal = generate_multi_person_signal(person_count, sample_rate, 30.0);
        let mut buffer = CsiDataBuffer::new(sample_rate);
        buffer.add_samples(&signal, &signal);

        group.bench_with_input(
            BenchmarkId::new("multi_person", format!("{}_people", person_count)),
            &buffer,
            |b, buffer| {
                b.iter(|| standard_pipeline.detect(black_box(buffer)))
            },
        );
    }

    group.finish();
}

// =============================================================================
// Triangulation Benchmarks
// =============================================================================

fn bench_triangulation(c: &mut Criterion) {
    let mut group = c.benchmark_group("triangulation");

    let triangulator = Triangulator::with_defaults();

    // Benchmark with different sensor counts
    for sensor_count in [3, 4, 5, 8, 12] {
        let sensors = create_test_sensors(sensor_count);

        // Generate RSSI values (simulate target at center)
        let rssi_values: Vec<(String, f64)> = sensors.iter()
            .map(|s| {
                let distance = (s.x * s.x + s.y * s.y).sqrt();
                let rssi = -30.0 - 20.0 * distance.log10(); // Path loss model
                (s.id.clone(), rssi)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("rssi_position", format!("{}_sensors", sensor_count)),
            &(sensors.clone(), rssi_values.clone()),
            |b, (sensors, rssi)| {
                b.iter(|| {
                    triangulator.estimate_position(black_box(sensors), black_box(rssi))
                })
            },
        );
    }

    // Benchmark ToA-based positioning
    for sensor_count in [3, 4, 5, 8] {
        let sensors = create_test_sensors(sensor_count);

        // Generate ToA values (time in nanoseconds)
        let toa_values: Vec<(String, f64)> = sensors.iter()
            .map(|s| {
                let distance = (s.x * s.x + s.y * s.y).sqrt();
                // Round trip time: 2 * distance / speed_of_light
                let toa_ns = 2.0 * distance / 299_792_458.0 * 1e9;
                (s.id.clone(), toa_ns)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("toa_position", format!("{}_sensors", sensor_count)),
            &(sensors.clone(), toa_values.clone()),
            |b, (sensors, toa)| {
                b.iter(|| {
                    triangulator.estimate_from_toa(black_box(sensors), black_box(toa))
                })
            },
        );
    }

    // Benchmark with noisy measurements
    let sensors = create_test_sensors(5);
    for noise_pct in [0, 5, 10, 20] {
        let rssi_values: Vec<(String, f64)> = sensors.iter()
            .enumerate()
            .map(|(i, s)| {
                let distance = (s.x * s.x + s.y * s.y).sqrt();
                let rssi = -30.0 - 20.0 * distance.log10();
                // Add noise based on index for determinism
                let noise = (i as f64 / 10.0) * noise_pct as f64 / 100.0 * 10.0;
                (s.id.clone(), rssi + noise)
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("noisy_rssi", format!("{}pct_noise", noise_pct)),
            &(sensors.clone(), rssi_values.clone()),
            |b, (sensors, rssi)| {
                b.iter(|| {
                    triangulator.estimate_position(black_box(sensors), black_box(rssi))
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Depth Estimation Benchmarks
// =============================================================================

fn bench_depth_estimation(c: &mut Criterion) {
    let mut group = c.benchmark_group("depth_estimation");

    let estimator = DepthEstimator::with_defaults();
    let debris = create_test_debris();

    // Benchmark single-path depth estimation
    for attenuation in [10.0, 20.0, 40.0, 60.0] {
        group.bench_with_input(
            BenchmarkId::new("single_path", format!("{}dB", attenuation as u32)),
            &attenuation,
            |b, &attenuation| {
                b.iter(|| {
                    estimator.estimate_depth(
                        black_box(attenuation),
                        black_box(5.0), // 5m horizontal distance
                        black_box(&debris),
                    )
                })
            },
        );
    }

    // Benchmark different debris types
    let debris_types = [
        ("snow", DebrisMaterial::Snow),
        ("wood", DebrisMaterial::Wood),
        ("light_concrete", DebrisMaterial::LightConcrete),
        ("heavy_concrete", DebrisMaterial::HeavyConcrete),
        ("mixed", DebrisMaterial::Mixed),
    ];

    for (name, material) in debris_types {
        let debris = DebrisProfile {
            primary_material: material,
            void_fraction: 0.25,
            moisture_content: MoistureLevel::Dry,
            metal_content: MetalContent::Low,
        };

        group.bench_with_input(
            BenchmarkId::new("debris_type", name),
            &debris,
            |b, debris| {
                b.iter(|| {
                    estimator.estimate_depth(
                        black_box(30.0),
                        black_box(5.0),
                        black_box(debris),
                    )
                })
            },
        );
    }

    // Benchmark multipath depth estimation
    for path_count in [1, 2, 4, 8] {
        let reflected_paths: Vec<(f64, f64)> = (0..path_count)
            .map(|i| {
                (
                    30.0 + i as f64 * 5.0, // attenuation
                    1e-9 * (i + 1) as f64, // delay in seconds
                )
            })
            .collect();

        group.bench_with_input(
            BenchmarkId::new("multipath", format!("{}_paths", path_count)),
            &reflected_paths,
            |b, paths| {
                b.iter(|| {
                    estimator.estimate_from_multipath(
                        black_box(25.0),
                        black_box(paths),
                        black_box(&debris),
                    )
                })
            },
        );
    }

    // Benchmark debris profile estimation
    for (variance, multipath, moisture) in [
        (0.2, 0.3, 0.2),
        (0.5, 0.5, 0.5),
        (0.7, 0.8, 0.8),
    ] {
        group.bench_with_input(
            BenchmarkId::new("profile_estimation", format!("v{}_m{}", (variance * 10.0) as u32, (multipath * 10.0) as u32)),
            &(variance, multipath, moisture),
            |b, &(v, m, mo)| {
                b.iter(|| {
                    estimator.estimate_debris_profile(
                        black_box(v),
                        black_box(m),
                        black_box(mo),
                    )
                })
            },
        );
    }

    group.finish();
}

// =============================================================================
// Alert Generation Benchmarks
// =============================================================================

fn bench_alert_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("alert_generation");

    // Benchmark basic alert generation
    let generator = AlertGenerator::new();
    let survivor = create_test_survivor();

    group.bench_function("generate_basic_alert", |b| {
        b.iter(|| generator.generate(black_box(&survivor)))
    });

    // Benchmark escalation alert
    group.bench_function("generate_escalation_alert", |b| {
        b.iter(|| {
            generator.generate_escalation(
                black_box(&survivor),
                black_box("Vital signs deteriorating"),
            )
        })
    });

    // Benchmark status change alert
    use wifi_densepose_mat::domain::TriageStatus;
    group.bench_function("generate_status_change_alert", |b| {
        b.iter(|| {
            generator.generate_status_change(
                black_box(&survivor),
                black_box(&TriageStatus::Minor),
            )
        })
    });

    // Benchmark with zone registration
    let mut generator_with_zones = AlertGenerator::new();
    for i in 0..100 {
        generator_with_zones.register_zone(ScanZoneId::new(), format!("Zone {}", i));
    }

    group.bench_function("generate_with_zones_lookup", |b| {
        b.iter(|| generator_with_zones.generate(black_box(&survivor)))
    });

    // Benchmark batch alert generation
    let survivors: Vec<Survivor> = (0..10).map(|_| create_test_survivor()).collect();

    group.bench_function("batch_generate_10_alerts", |b| {
        b.iter(|| {
            survivors.iter()
                .map(|s| generator.generate(black_box(s)))
                .collect::<Vec<_>>()
        })
    });

    group.finish();
}

// =============================================================================
// CSI Buffer Operations Benchmarks
// =============================================================================

fn bench_csi_buffer(c: &mut Criterion) {
    let mut group = c.benchmark_group("csi_buffer");

    let sample_rate = 100.0;

    // Benchmark buffer creation and addition
    for sample_count in [1000, 5000, 10000, 30000] {
        let amplitudes: Vec<f64> = (0..sample_count)
            .map(|i| (i as f64 / 100.0).sin())
            .collect();
        let phases: Vec<f64> = (0..sample_count)
            .map(|i| (i as f64 / 50.0).cos())
            .collect();

        group.throughput(Throughput::Elements(sample_count as u64));
        group.bench_with_input(
            BenchmarkId::new("add_samples", format!("{}_samples", sample_count)),
            &(amplitudes.clone(), phases.clone()),
            |b, (amp, phase)| {
                b.iter(|| {
                    let mut buffer = CsiDataBuffer::new(sample_rate);
                    buffer.add_samples(black_box(amp), black_box(phase));
                    buffer
                })
            },
        );
    }

    // Benchmark incremental addition (simulating real-time data)
    let chunk_size = 100;
    let total_samples = 10000;
    let amplitudes: Vec<f64> = (0..chunk_size).map(|i| (i as f64 / 100.0).sin()).collect();
    let phases: Vec<f64> = (0..chunk_size).map(|i| (i as f64 / 50.0).cos()).collect();

    group.bench_function("incremental_add_100_chunks", |b| {
        b.iter(|| {
            let mut buffer = CsiDataBuffer::new(sample_rate);
            for _ in 0..(total_samples / chunk_size) {
                buffer.add_samples(black_box(&amplitudes), black_box(&phases));
            }
            buffer
        })
    });

    // Benchmark has_sufficient_data check
    let mut buffer = CsiDataBuffer::new(sample_rate);
    let amplitudes: Vec<f64> = (0..3000).map(|i| (i as f64 / 100.0).sin()).collect();
    let phases: Vec<f64> = (0..3000).map(|i| (i as f64 / 50.0).cos()).collect();
    buffer.add_samples(&amplitudes, &phases);

    group.bench_function("check_sufficient_data", |b| {
        b.iter(|| buffer.has_sufficient_data(black_box(10.0)))
    });

    group.bench_function("calculate_duration", |b| {
        b.iter(|| black_box(&buffer).duration())
    });

    group.finish();
}

// =============================================================================
// Criterion Groups and Main
// =============================================================================

criterion_group!(
    name = detection_benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2));
    targets =
        bench_breathing_detection,
        bench_heartbeat_detection,
        bench_movement_classification
);

criterion_group!(
    name = pipeline_benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(3))
        .sample_size(50);
    targets = bench_detection_pipeline
);

criterion_group!(
    name = localization_benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(500))
        .measurement_time(std::time::Duration::from_secs(2));
    targets =
        bench_triangulation,
        bench_depth_estimation
);

criterion_group!(
    name = alerting_benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(300))
        .measurement_time(std::time::Duration::from_secs(1));
    targets = bench_alert_generation
);

criterion_group!(
    name = buffer_benches;
    config = Criterion::default()
        .warm_up_time(std::time::Duration::from_millis(300))
        .measurement_time(std::time::Duration::from_secs(1));
    targets = bench_csi_buffer
);

criterion_main!(
    detection_benches,
    pipeline_benches,
    localization_benches,
    alerting_benches,
    buffer_benches
);
