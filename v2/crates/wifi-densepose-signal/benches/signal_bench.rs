//! Comprehensive benchmarks for WiFi-DensePose signal processing
//!
//! Run with: cargo bench --package wifi-densepose-signal

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use ndarray::Array2;
use std::time::Duration;

// Import from the crate
use wifi_densepose_signal::{
    CsiProcessor, CsiProcessorConfig, CsiData,
    PhaseSanitizer, PhaseSanitizerConfig,
    FeatureExtractor, FeatureExtractorConfig,
    MotionDetector, MotionDetectorConfig,
};

/// Create realistic test CSI data
fn create_csi_data(antennas: usize, subcarriers: usize) -> CsiData {
    use std::f64::consts::PI;

    let mut amplitude = Array2::zeros((antennas, subcarriers));
    let mut phase = Array2::zeros((antennas, subcarriers));

    for i in 0..antennas {
        for j in 0..subcarriers {
            // Realistic amplitude: combination of path loss and multipath
            let base_amp = 0.5 + 0.3 * ((j as f64 / subcarriers as f64) * PI).sin();
            let noise = 0.05 * ((i * 17 + j * 31) as f64 * 0.1).sin();
            amplitude[[i, j]] = base_amp + noise;

            // Realistic phase: linear component + multipath distortion
            let linear_phase = (j as f64 / subcarriers as f64) * 2.0 * PI;
            let multipath = 0.3 * ((j as f64 * 0.2 + i as f64 * 0.5).sin());
            phase[[i, j]] = (linear_phase + multipath) % (2.0 * PI) - PI;
        }
    }

    CsiData::builder()
        .amplitude(amplitude)
        .phase(phase)
        .build()
        .unwrap()
}

/// Benchmark CSI preprocessing pipeline
fn bench_csi_preprocessing(c: &mut Criterion) {
    let mut group = c.benchmark_group("CSI Preprocessing");
    group.measurement_time(Duration::from_secs(5));

    for &(antennas, subcarriers) in &[(4, 64), (4, 128), (8, 256)] {
        let config = CsiProcessorConfig::default();
        let processor = CsiProcessor::new(config).unwrap();
        let csi_data = create_csi_data(antennas, subcarriers);

        group.throughput(Throughput::Elements((antennas * subcarriers) as u64));
        group.bench_with_input(
            BenchmarkId::new("preprocess", format!("{}x{}", antennas, subcarriers)),
            &csi_data,
            |b, data| {
                b.iter(|| {
                    processor.preprocess(black_box(data)).unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark phase sanitization
fn bench_phase_sanitization(c: &mut Criterion) {
    let mut group = c.benchmark_group("Phase Sanitization");
    group.measurement_time(Duration::from_secs(5));

    for &size in &[64, 128, 256, 512] {
        let config = PhaseSanitizerConfig::default();
        let mut sanitizer = PhaseSanitizer::new(config).unwrap();

        // Create wrapped phase data with discontinuities
        let mut phase_data = Array2::zeros((4, size));
        for i in 0..4 {
            for j in 0..size {
                let t = j as f64 / size as f64;
                // Create phase with wrapping
                phase_data[[i, j]] = (t * 8.0 * std::f64::consts::PI) % (2.0 * std::f64::consts::PI) - std::f64::consts::PI;
            }
        }

        group.throughput(Throughput::Elements((4 * size) as u64));
        group.bench_with_input(
            BenchmarkId::new("sanitize", format!("4x{}", size)),
            &phase_data,
            |b, data| {
                b.iter(|| {
                    sanitizer.sanitize_phase(&black_box(data.clone())).unwrap()
                });
            },
        );
    }

    group.finish();
}

/// Benchmark feature extraction
fn bench_feature_extraction(c: &mut Criterion) {
    let mut group = c.benchmark_group("Feature Extraction");
    group.measurement_time(Duration::from_secs(5));

    for &subcarriers in &[64, 128, 256] {
        let config = FeatureExtractorConfig::default();
        let extractor = FeatureExtractor::new(config);
        let csi_data = create_csi_data(4, subcarriers);

        group.throughput(Throughput::Elements(subcarriers as u64));
        group.bench_with_input(
            BenchmarkId::new("extract", format!("4x{}", subcarriers)),
            &csi_data,
            |b, data| {
                b.iter(|| {
                    extractor.extract(black_box(data))
                });
            },
        );
    }

    group.finish();
}

/// Benchmark motion detection
fn bench_motion_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("Motion Detection");
    group.measurement_time(Duration::from_secs(5));

    let config = MotionDetectorConfig::builder()
        .motion_threshold(0.3)
        .history_size(10)
        .build();
    let detector = MotionDetector::new(config);

    let extractor = FeatureExtractor::new(FeatureExtractorConfig::default());

    // Pre-extract features for benchmark
    let csi_data = create_csi_data(4, 64);
    let features = extractor.extract(&csi_data);

    group.throughput(Throughput::Elements(1));
    group.bench_function("analyze_motion", |b| {
        b.iter(|| {
            detector.analyze_motion(black_box(&features))
        });
    });

    group.finish();
}

/// Benchmark full pipeline
fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("Full Pipeline");
    group.measurement_time(Duration::from_secs(10));

    let processor_config = CsiProcessorConfig::default();
    let processor = CsiProcessor::new(processor_config).unwrap();

    let sanitizer_config = PhaseSanitizerConfig::default();
    let mut sanitizer = PhaseSanitizer::new(sanitizer_config).unwrap();

    let extractor_config = FeatureExtractorConfig::default();
    let extractor = FeatureExtractor::new(extractor_config);

    let detector_config = MotionDetectorConfig::builder()
        .motion_threshold(0.3)
        .history_size(10)
        .build();
    let detector = MotionDetector::new(detector_config);

    let csi_data = create_csi_data(4, 64);

    group.throughput(Throughput::Elements(1));
    group.bench_function("complete_signal_pipeline", |b| {
        b.iter(|| {
            // 1. Preprocess CSI
            let processed = processor.preprocess(black_box(&csi_data)).unwrap();

            // 2. Sanitize phase
            let sanitized = sanitizer.sanitize_phase(&black_box(processed.phase.clone())).unwrap();

            // 3. Extract features
            let features = extractor.extract(black_box(&csi_data));

            // 4. Detect motion
            let _motion = detector.analyze_motion(black_box(&features));

            sanitized
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_csi_preprocessing,
    bench_phase_sanitization,
    bench_feature_extraction,
    bench_motion_detection,
    bench_full_pipeline,
);
criterion_main!(benches);
