use bit_parallel_search::BitParallelSearcher;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Test data
#[allow(dead_code)]
const SMALL_TEXT: &[u8] = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps.";
#[allow(dead_code)]
const MEDIUM_TEXT: &[u8] = include_bytes!("../README.md");

fn generate_text(size: usize) -> Vec<u8> {
    let mut text = Vec::with_capacity(size);
    let pattern = b"The quick brown fox jumps over the lazy dog. ";
    while text.len() < size {
        text.extend_from_slice(pattern);
    }
    text.truncate(size);
    text
}

fn bench_pattern_lengths(c: &mut Criterion) {
    let text = generate_text(10_000);
    let patterns = vec![
        (b"fox".to_vec(), "3_bytes"),
        (b"quick".to_vec(), "5_bytes"),
        (b"jumps over".to_vec(), "10_bytes"),
        (b"The quick brown fox".to_vec(), "19_bytes"),
        (b"The quick brown fox jumps over the lazy dog".to_vec(), "44_bytes"),
        (b"x".repeat(64), "64_bytes"),
        (b"x".repeat(65), "65_bytes_FALLBACK"),
        (b"x".repeat(128), "128_bytes_FALLBACK"),
    ];

    let mut group = c.benchmark_group("pattern_length_comparison");
    group.throughput(Throughput::Bytes(text.len() as u64));

    for (pattern, name) in patterns {
        // Bit-parallel
        group.bench_with_input(
            BenchmarkId::new("bit_parallel", name),
            &(&text, &pattern),
            |b, (text, pattern)| {
                let searcher = BitParallelSearcher::new(pattern);
                b.iter(|| searcher.find_in(black_box(text)))
            },
        );

        // Standard library
        group.bench_with_input(
            BenchmarkId::new("std_find", name),
            &(&text, &pattern),
            |b, (text, pattern)| {
                b.iter(|| {
                    text.windows(pattern.len())
                        .position(|window| window == pattern.as_slice())
                })
            },
        );

        // Naive implementation
        group.bench_with_input(
            BenchmarkId::new("naive", name),
            &(&text, &pattern),
            |b, (text, pattern)| {
                b.iter(|| naive_search(black_box(text), black_box(pattern)))
            },
        );
    }

    group.finish();
}

fn bench_text_sizes(c: &mut Criterion) {
    let pattern = b"brown fox";
    let sizes = vec![100, 1_000, 10_000, 100_000];

    let mut group = c.benchmark_group("text_size_scaling");

    for size in sizes {
        let text = generate_text(size);
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_with_input(
            BenchmarkId::new("bit_parallel", size),
            &text,
            |b, text| {
                let searcher = BitParallelSearcher::new(pattern);
                b.iter(|| searcher.find_in(black_box(text)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("std_find", size),
            &text,
            |b, text| {
                b.iter(|| {
                    text.windows(pattern.len())
                        .position(|window| window == pattern)
                })
            },
        );
    }

    group.finish();
}

fn bench_reuse_vs_recreate(c: &mut Criterion) {
    let texts: Vec<Vec<u8>> = (0..100).map(|_| generate_text(1000)).collect();
    let pattern = b"fox";

    let mut group = c.benchmark_group("searcher_reuse");

    // Reusing searcher (GOOD)
    group.bench_function("reuse_searcher", |b| {
        let searcher = BitParallelSearcher::new(pattern);
        b.iter(|| {
            for text in &texts {
                black_box(searcher.find_in(text));
            }
        })
    });

    // Recreating searcher each time (BAD)
    group.bench_function("recreate_searcher", |b| {
        b.iter(|| {
            for text in &texts {
                let searcher = BitParallelSearcher::new(pattern);
                black_box(searcher.find_in(text));
            }
        })
    });

    group.finish();
}

fn bench_count_performance(c: &mut Criterion) {
    let text = b"ab".repeat(1000);
    let pattern = b"ab";

    let mut group = c.benchmark_group("count_occurrences");

    group.bench_function("bit_parallel_count", |b| {
        let searcher = BitParallelSearcher::new(pattern);
        b.iter(|| searcher.count_in(black_box(&text)))
    });

    #[cfg(feature = "std")]
    group.bench_function("find_all_count", |b| {
        let searcher = BitParallelSearcher::new(pattern);
        b.iter(|| searcher.find_all_in(black_box(&text)).count())
    });

    group.bench_function("naive_count", |b| {
        b.iter(|| {
            let mut count = 0;
            let mut pos = 0;
            while pos <= text.len() - pattern.len() {
                if &text[pos..pos + pattern.len()] == pattern {
                    count += 1;
                }
                pos += 1;
            }
            count
        })
    });

    group.finish();
}

fn bench_against_memchr(c: &mut Criterion) {
    let text = generate_text(10_000);
    let byte = b'x';

    let mut group = c.benchmark_group("single_byte_comparison");
    group.throughput(Throughput::Bytes(text.len() as u64));

    group.bench_function("bit_parallel", |b| {
        let searcher = BitParallelSearcher::new(&[byte]);
        b.iter(|| searcher.find_in(black_box(&text)))
    });

    group.bench_function("memchr", |b| {
        b.iter(|| memchr::memchr(byte, black_box(&text)))
    });

    group.finish();
}

// Comparison with regex for completeness
fn bench_vs_regex(c: &mut Criterion) {
    use regex::bytes::Regex;

    let text = generate_text(10_000);
    let pattern = b"quick.*fox";
    let literal_pattern = b"quick brown fox";

    let mut group = c.benchmark_group("regex_comparison");

    // Regex pattern
    let re = Regex::new(std::str::from_utf8(pattern).unwrap()).unwrap();
    group.bench_function("regex_pattern", |b| {
        b.iter(|| re.find(black_box(&text)))
    });

    // Regex literal (for fair comparison)
    let re_literal = Regex::new(std::str::from_utf8(literal_pattern).unwrap()).unwrap();
    group.bench_function("regex_literal", |b| {
        b.iter(|| re_literal.find(black_box(&text)))
    });

    // Bit-parallel
    group.bench_function("bit_parallel", |b| {
        let searcher = BitParallelSearcher::new(literal_pattern);
        b.iter(|| searcher.find_in(black_box(&text)))
    });

    group.finish();
}

fn naive_search(text: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > text.len() {
        return None;
    }

    for i in 0..=text.len() - pattern.len() {
        let mut matches = true;
        for j in 0..pattern.len() {
            if text[i + j] != pattern[j] {
                matches = false;
                break;
            }
        }
        if matches {
            return Some(i);
        }
    }
    None
}

criterion_group!(
    benches,
    bench_pattern_lengths,
    bench_text_sizes,
    bench_reuse_vs_recreate,
    bench_count_performance,
    bench_against_memchr,
    bench_vs_regex
);

criterion_main!(benches);