//! Benchmarks for detection layer performance

use aimds_detection::{DetectionConfig, DetectionEngine};
use aimds_core::{ThreatLevel, ThreatPattern};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_pattern_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("pattern_matching");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for pattern_count in [1, 5, 10, 20, 50].iter() {
        group.throughput(Throughput::Elements(*pattern_count as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(pattern_count),
            pattern_count,
            |b, &count| {
                let config = DetectionConfig::default();
                let mut engine = DetectionEngine::new(config).unwrap();

                // Add patterns
                for i in 0..count {
                    engine.add_pattern(ThreatPattern {
                        name: format!("Pattern {}", i),
                        signature: format!("threat signature {}", i),
                        severity: ThreatLevel::Medium,
                        confidence: 0.8,
                    });
                }

                let input = "This is a test input with some threat signature 5 content";

                b.iter(|| {
                    rt.block_on(async {
                        engine.detect(black_box(input)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_sanitization(c: &mut Criterion) {
    let mut group = c.benchmark_group("sanitization");

    let rt = tokio::runtime::Runtime::new().unwrap();

    for input_size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Bytes(*input_size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(input_size),
            input_size,
            |b, &size| {
                let config = DetectionConfig {
                    enable_sanitization: true,
                    enable_pii_detection: false,
                    ..Default::default()
                };
                let engine = DetectionEngine::new(config).unwrap();
                let input = "a".repeat(size);

                b.iter(|| {
                    rt.block_on(async {
                        engine.detect(black_box(&input)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_pii_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("pii_detection");

    let rt = tokio::runtime::Runtime::new().unwrap();

    let inputs = vec![
        ("no_pii", "This is normal text without any PII"),
        ("with_email", "Contact us at support@example.com for help"),
        ("with_phone", "Call me at 555-123-4567 tomorrow"),
        ("with_multiple", "Email: user@test.com, Phone: 555-1234, IP: 192.168.1.1"),
    ];

    for (name, input) in inputs {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &input,
            |b, &input| {
                let config = DetectionConfig {
                    enable_pii_detection: true,
                    enable_sanitization: false,
                    ..Default::default()
                };
                let engine = DetectionEngine::new(config).unwrap();

                b.iter(|| {
                    rt.block_on(async {
                        engine.detect(black_box(input)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_pipeline");
    group.sample_size(100);

    let rt = tokio::runtime::Runtime::new().unwrap();

    let config = DetectionConfig {
        window_size: 50,
        max_pattern_length: 1000,
        confidence_threshold: 0.75,
        enable_pii_detection: true,
        enable_sanitization: true,
    };
    let mut engine = DetectionEngine::new(config).unwrap();

    // Add realistic threat patterns
    engine.add_pattern(ThreatPattern {
        name: "SQL Injection".to_string(),
        signature: "SELECT * FROM users WHERE".to_string(),
        severity: ThreatLevel::Critical,
        confidence: 0.95,
    });

    engine.add_pattern(ThreatPattern {
        name: "XSS Attack".to_string(),
        signature: "<script>alert('xss')</script>".to_string(),
        severity: ThreatLevel::High,
        confidence: 0.9,
    });

    engine.add_pattern(ThreatPattern {
        name: "Path Traversal".to_string(),
        signature: "../../../etc/passwd".to_string(),
        severity: ThreatLevel::High,
        confidence: 0.85,
    });

    let input = "User input: admin@example.com with IP 192.168.1.1";

    group.bench_function("realistic_input", |b| {
        b.iter(|| {
            rt.block_on(async {
                engine.detect(black_box(input)).await.unwrap()
            })
        });
    });

    group.finish();
}

fn bench_scheduling(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduling");

    let rt = tokio::runtime::Runtime::new().unwrap();

    use aimds_detection::ThreatScheduler;

    let scheduler = ThreatScheduler::new();

    for threat_level in [
        ThreatLevel::None,
        ThreatLevel::Low,
        ThreatLevel::Medium,
        ThreatLevel::High,
        ThreatLevel::Critical,
    ] {
        group.bench_with_input(
            BenchmarkId::from_parameter(format!("{:?}", threat_level)),
            &threat_level,
            |b, &level| {
                b.iter(|| {
                    rt.block_on(async {
                        scheduler.prioritize_threat(black_box(level)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_pattern_matching,
    bench_sanitization,
    bench_pii_detection,
    bench_full_pipeline,
    bench_scheduling,
);

criterion_main!(benches);
