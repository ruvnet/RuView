use bit_parallel_search::BitParallelSearcher;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

// Real-world test data
const HTTP_HEADER: &[u8] = include_bytes!("../test_data/http_header.txt");
const LOG_FILE: &[u8] = include_bytes!("../test_data/server.log");
const JSON_RESPONSE: &[u8] = include_bytes!("../test_data/api_response.json");

fn bench_http_parsing(c: &mut Criterion) {
    let headers = b"Host: example.com\r\nUser-Agent: Mozilla/5.0\r\nContent-Type: application/json\r\nContent-Length: 1234\r\nAuthorization: Bearer abc123\r\nAccept: application/json\r\nConnection: keep-alive\r\n\r\n".repeat(100);

    let searchers = vec![
        (BitParallelSearcher::new(b"Content-Type:"), "Content-Type"),
        (BitParallelSearcher::new(b"Content-Length:"), "Content-Length"),
        (BitParallelSearcher::new(b"Authorization:"), "Authorization"),
        (BitParallelSearcher::new(b"User-Agent:"), "User-Agent"),
    ];

    let mut group = c.benchmark_group("http_header_parsing");
    group.throughput(Throughput::Bytes(headers.len() as u64));

    for (searcher, name) in searchers {
        group.bench_with_input(
            BenchmarkId::new("bit_parallel", name),
            &headers,
            |b, headers| {
                b.iter(|| searcher.find_in(black_box(headers)))
            },
        );

        // Compare with naive approach
        group.bench_with_input(
            BenchmarkId::new("naive_find", name),
            &(&headers, searcher.pattern_len()),
            |b, (headers, pattern_len)| {
                let pattern: &[u8] = match name {
                    "Content-Type" => b"Content-Type:",
                    "Content-Length" => b"Content-Length:",
                    "Authorization" => b"Authorization:",
                    "User-Agent" => b"User-Agent:",
                    _ => unreachable!(),
                };
                b.iter(|| {
                    headers.windows(pattern.len())
                        .position(|window| window == pattern)
                })
            },
        );
    }

    group.finish();
}

fn bench_log_analysis(c: &mut Criterion) {
    // Simulate server log file
    let log_entries: &[&[u8]] = &[
        b"2024-01-01 10:00:01 INFO Starting server on port 8080",
        b"2024-01-01 10:00:02 ERROR Failed to connect to database",
        b"2024-01-01 10:00:03 WARN High memory usage detected",
        b"2024-01-01 10:00:04 INFO Request processed successfully",
        b"2024-01-01 10:00:05 ERROR Authentication failed for user",
        b"2024-01-01 10:00:06 DEBUG SQL query executed in 15ms",
    ];

    let log_data: Vec<u8> = log_entries.iter()
        .cycle()
        .take(10000)
        .flat_map(|entry| entry.iter().copied().chain(b"\n".iter().copied()))
        .collect();

    let error_searcher = BitParallelSearcher::new(b"ERROR");
    let warn_searcher = BitParallelSearcher::new(b"WARN");

    let mut group = c.benchmark_group("log_analysis");
    group.throughput(Throughput::Bytes(log_data.len() as u64));

    group.bench_function("count_errors", |b| {
        b.iter(|| error_searcher.count_in(black_box(&log_data)))
    });

    group.bench_function("count_warnings", |b| {
        b.iter(|| warn_searcher.count_in(black_box(&log_data)))
    });

    // Compare with regex for context
    let re = regex::bytes::Regex::new(r"ERROR").unwrap();
    group.bench_function("regex_count_errors", |b| {
        b.iter(|| re.find_iter(black_box(&log_data)).count())
    });

    group.finish();
}

fn bench_protocol_parsing(c: &mut Criterion) {
    // Simulate network protocol parsing
    let packets: &[&[u8]] = &[
        b"GET /api/users HTTP/1.1\r\nHost: api.example.com\r\n\r\n",
        b"POST /api/login HTTP/1.1\r\nContent-Type: application/json\r\n\r\n",
        b"PUT /api/users/123 HTTP/1.1\r\nAuthorization: Bearer token\r\n\r\n",
        b"DELETE /api/users/123 HTTP/1.1\r\nX-Requested-With: XMLHttpRequest\r\n\r\n",
    ];

    let protocol_data: Vec<u8> = packets.iter()
        .cycle()
        .take(5000)
        .flat_map(|packet| packet.iter().copied())
        .collect();

    let http_methods = vec![
        (BitParallelSearcher::new(b"GET "), "GET"),
        (BitParallelSearcher::new(b"POST "), "POST"),
        (BitParallelSearcher::new(b"PUT "), "PUT"),
        (BitParallelSearcher::new(b"DELETE "), "DELETE"),
    ];

    let mut group = c.benchmark_group("protocol_parsing");
    group.throughput(Throughput::Bytes(protocol_data.len() as u64));

    for (searcher, method) in http_methods {
        group.bench_with_input(
            BenchmarkId::new("method_detection", method),
            &protocol_data,
            |b, data| {
                b.iter(|| searcher.count_in(black_box(data)))
            },
        );
    }

    group.finish();
}

fn bench_bioinformatics(c: &mut Criterion) {
    // DNA sequence analysis
    let bases = b"ATCGATCGATCGATCG";
    let dna_sequence: Vec<u8> = bases.iter()
        .cycle()
        .take(100_000)
        .copied()
        .collect();

    let patterns = vec![
        (b"ATCG".to_vec(), "ATCG_motif"),
        (b"GCTA".to_vec(), "GCTA_motif"),
        (b"AAAA".to_vec(), "poly_A"),
        (b"CCCC".to_vec(), "poly_C"),
        (b"ATCGATCG".to_vec(), "repeat_8bp"),
    ];

    let mut group = c.benchmark_group("bioinformatics");
    group.throughput(Throughput::Bytes(dna_sequence.len() as u64));

    for (pattern, name) in patterns {
        let searcher = BitParallelSearcher::new(&pattern);

        group.bench_with_input(
            BenchmarkId::new("motif_search", name),
            &dna_sequence,
            |b, sequence| {
                b.iter(|| searcher.count_in(black_box(sequence)))
            },
        );
    }

    group.finish();
}

fn bench_text_processing(c: &mut Criterion) {
    // Real text processing scenario
    let text = b"The quick brown fox jumps over the lazy dog. ".repeat(10000);

    let search_terms = vec![
        (b"the".to_vec(), "the"),
        (b"fox".to_vec(), "fox"),
        (b"quick brown".to_vec(), "quick_brown"),
        (b"jumps over".to_vec(), "jumps_over"),
        (b"lazy dog".to_vec(), "lazy_dog"),
    ];

    let mut group = c.benchmark_group("text_processing");
    group.throughput(Throughput::Bytes(text.len() as u64));

    for (pattern, name) in search_terms {
        let searcher = BitParallelSearcher::new(&pattern);

        group.bench_with_input(
            BenchmarkId::new("word_search", name),
            &text,
            |b, text| {
                b.iter(|| searcher.count_in(black_box(text)))
            },
        );
    }

    group.finish();
}

fn bench_config_parsing(c: &mut Criterion) {
    // Configuration file parsing
    let config = b"server.port=8080\nserver.host=localhost\ndatabase.url=postgres://localhost/db\ndatabase.pool_size=10\nredis.url=redis://localhost\nlogging.level=info\n".repeat(1000);

    let config_keys = vec![
        (BitParallelSearcher::new(b"server.port="), "server_port"),
        (BitParallelSearcher::new(b"database.url="), "database_url"),
        (BitParallelSearcher::new(b"redis.url="), "redis_url"),
        (BitParallelSearcher::new(b"logging.level="), "logging_level"),
    ];

    let mut group = c.benchmark_group("config_parsing");
    group.throughput(Throughput::Bytes(config.len() as u64));

    for (searcher, key_name) in config_keys {
        group.bench_with_input(
            BenchmarkId::new("config_key_search", key_name),
            &config,
            |b, config| {
                b.iter(|| searcher.find_in(black_box(config)))
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_http_parsing,
    bench_log_analysis,
    bench_protocol_parsing,
    bench_bioinformatics,
    bench_text_processing,
    bench_config_parsing
);

criterion_main!(benches);