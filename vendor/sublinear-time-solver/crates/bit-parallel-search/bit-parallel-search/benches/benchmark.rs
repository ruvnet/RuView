use bit_parallel_search::BitParallelMatcher;
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_vs_naive(c: &mut Criterion) {
    let text = b"The quick brown fox jumps over the lazy dog. The quick brown fox jumps over the lazy dog.";
    let patterns = vec![
        (b"quick" as &[u8], "short"),
        (b"jumps over the lazy" as &[u8], "medium"),
        (b"The quick brown fox jumps over the lazy dog" as &[u8], "long"),
    ];

    let mut group = c.benchmark_group("string_search");

    for (pattern, name) in patterns {
        group.bench_with_input(
            BenchmarkId::new("bit_parallel", name),
            &(text, pattern),
            |b, (text, pattern)| {
                b.iter(|| BitParallelMatcher::find(black_box(text), black_box(pattern)))
            },
        );

        group.bench_with_input(
            BenchmarkId::new("naive", name),
            &(text, pattern),
            |b, (text, pattern)| {
                b.iter(|| naive_search(black_box(text), black_box(pattern)))
            },
        );
    }

    group.finish();
}

fn naive_search(text: &[u8], pattern: &[u8]) -> Option<usize> {
    if pattern.is_empty() || pattern.len() > text.len() {
        return None;
    }

    for i in 0..=text.len() - pattern.len() {
        if &text[i..i + pattern.len()] == pattern {
            return Some(i);
        }
    }
    None
}

fn bench_count(c: &mut Criterion) {
    let text = b"ababababababababababababababababab";
    let pattern = b"aba";

    c.bench_function("bit_parallel_count", |b| {
        b.iter(|| BitParallelMatcher::count(black_box(text), black_box(pattern)))
    });
}

criterion_group!(benches, bench_vs_naive, bench_count);
criterion_main!(benches);