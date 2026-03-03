//! Simplified AIMDS detection benchmarks

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use aimds_detection::DetectionService;
use aimds_core::PromptInput;

fn bench_detection_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_simple");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = DetectionService::new().unwrap();

    for size in [100, 500, 1000, 5000].iter() {
        group.throughput(Throughput::Bytes(*size as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                let input = PromptInput::new("a".repeat(size));

                b.iter(|| {
                    rt.block_on(async {
                        service.detect(black_box(&input)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

fn bench_detection_patterns(c: &mut Criterion) {
    let mut group = c.benchmark_group("detection_patterns");

    let rt = tokio::runtime::Runtime::new().unwrap();
    let service = DetectionService::new().unwrap();

    let test_inputs = vec![
        ("clean", "This is a normal input with no threats"),
        ("suspicious", "SELECT * FROM users WHERE id=1 OR 1=1"),
        ("malicious", "<script>alert('xss')</script>"),
        ("complex", "Admin password: P@ssw0rd! Email: admin@example.com IP: 192.168.1.1"),
    ];

    for (name, content) in test_inputs {
        group.bench_with_input(
            BenchmarkId::from_parameter(name),
            &content,
            |b, &content| {
                let input = PromptInput::new(content.to_string());

                b.iter(|| {
                    rt.block_on(async {
                        service.detect(black_box(&input)).await.unwrap()
                    })
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_detection_simple, bench_detection_patterns);
criterion_main!(benches);
