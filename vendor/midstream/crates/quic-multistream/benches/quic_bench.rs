//! Comprehensive benchmarks for quic-multistream crate
//!
//! **NO MOCKS - Real QUIC operations using quinn library**
//!
//! Benchmarks cover:
//! - Stream throughput (target: >100 MB/s)
//! - Multiplexing performance (concurrent streams)
//! - Connection establishment latency
//! - 0-RTT handshake time (when possible)
//! - Priority queue performance
//! - Error recovery overhead
//!
//! Performance targets:
//! - Stream throughput: >100 MB/s
//! - Connection establishment: <10ms
//! - Concurrent streams: 100+ simultaneous
//! - Priority handling: <1ms overhead

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use midstreamer_quic::{QuicConnection, StreamPriority};
use quinn::{Endpoint, ServerConfig};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::time::Duration;

// ============================================================================
// Real QUIC Server Setup (NO MOCKS)
// ============================================================================

/// Create a real QUIC server using quinn
async fn create_test_server() -> Result<(Endpoint, SocketAddr), Box<dyn std::error::Error>> {
    // Generate self-signed certificate for testing
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let cert_der = cert.serialize_der()?;
    let priv_key = cert.serialize_private_key_der();

    // Create server TLS config
    let mut server_crypto = quinn::rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert_der.into()],
            quinn::rustls::pki_types::PrivatePkcs8KeyDer::from(priv_key).into(),
        )?;

    server_crypto.alpn_protocols = vec![b"h3".to_vec()];

    let mut server_config = ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(server_crypto)?,
    ));

    // Configure transport for high performance
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(1000u32.into());
    transport.max_concurrent_uni_streams(1000u32.into());
    transport.stream_receive_window(10_000_000u32.into());
    transport.receive_window(15_000_000u32.into());
    server_config.transport_config(Arc::new(transport));

    // Bind to localhost on random port
    let endpoint = Endpoint::server(server_config, "127.0.0.1:0".parse()?)?;
    let addr = endpoint.local_addr()?;

    Ok((endpoint, addr))
}

/// Run real QUIC server that echoes data back
async fn run_test_server(endpoint: Endpoint) {
    while let Some(incoming) = endpoint.accept().await {
        tokio::spawn(async move {
            if let Ok(connection) = incoming.await {
                // Handle bidirectional streams - echo data back
                while let Ok((mut send, mut recv)) = connection.accept_bi().await {
                    tokio::spawn(async move {
                        let mut buf = vec![0u8; 65536];
                        while let Ok(Some(n)) = recv.read(&mut buf).await {
                            if send.write_all(&buf[..n]).await.is_err() {
                                break;
                            }
                        }
                        let _ = send.finish();
                    });
                }
            }
        });
    }
}

// ============================================================================
// Data Generators
// ============================================================================

fn generate_data(size: usize) -> Vec<u8> {
    (0..size).map(|i| (i % 256) as u8).collect()
}

// ============================================================================
// Benchmark 1: Stream Throughput (Real QUIC)
// ============================================================================

fn bench_stream_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    // Start real QUIC server
    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("stream_throughput");

    for size in [1024, 65536, 1_048_576, 10_485_760].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("single_stream", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
                    let mut stream = connection.open_bi_stream().await.unwrap();

                    let data = generate_data(size);
                    let mut recv_buf = vec![0u8; size];

                    stream.send(&data).await.unwrap();
                    stream.recv(&mut recv_buf).await.unwrap();

                    black_box(recv_buf)
                });
            }
        );
    }

    group.finish();
}

fn bench_sustained_throughput(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("sustained_throughput");
    group.sample_size(30);

    let data = generate_data(65536);

    group.bench_function("100_iterations_64kb", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut stream = connection.open_bi_stream().await.unwrap();
            let mut recv_buf = vec![0u8; 65536];

            for _ in 0..100 {
                stream.send(&data).await.unwrap();
                stream.recv(&mut recv_buf).await.unwrap();
            }

            black_box(recv_buf)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 2: Multiplexing Performance (Real Concurrent Streams)
// ============================================================================

fn bench_concurrent_streams(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("concurrent_streams");
    group.sample_size(20);

    for num_streams in [1, 10, 50, 100].iter() {
        group.throughput(Throughput::Elements(*num_streams as u64));

        group.bench_with_input(
            BenchmarkId::new("parallel_streams", num_streams),
            num_streams,
            |b, &n| {
                b.to_async(&rt).iter(|| async {
                    let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
                    let data = generate_data(4096);

                    let mut tasks = Vec::new();

                    for _ in 0..n {
                        let connection = &connection;
                        let data = data.clone();

                        let task = async move {
                            let mut stream = connection.open_bi_stream().await.unwrap();
                            let mut recv_buf = vec![0u8; 4096];
                            stream.send(&data).await.unwrap();
                            stream.recv(&mut recv_buf).await.unwrap();
                            recv_buf
                        };

                        tasks.push(task);
                    }

                    let results = futures::future::join_all(tasks).await;
                    black_box(results)
                });
            }
        );
    }

    group.finish();
}

fn bench_sequential_vs_parallel(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("sequential_vs_parallel");
    let data = generate_data(8192);

    group.bench_function("sequential_10_streams", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut results = Vec::new();

            for _ in 0..10 {
                let mut stream = connection.open_bi_stream().await.unwrap();
                let mut recv_buf = vec![0u8; 8192];
                stream.send(&data).await.unwrap();
                stream.recv(&mut recv_buf).await.unwrap();
                results.push(recv_buf);
            }

            black_box(results)
        });
    });

    group.bench_function("parallel_10_streams", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = Arc::new(QuicConnection::connect(&addr.to_string()).await.unwrap());

            let mut tasks = Vec::new();
            for _ in 0..10 {
                let connection = connection.clone();
                let data = data.clone();
                let task = async move {
                    let mut stream = connection.open_bi_stream().await.unwrap();
                    let mut recv_buf = vec![0u8; 8192];
                    stream.send(&data).await.unwrap();
                    stream.recv(&mut recv_buf).await.unwrap();
                    recv_buf
                };
                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;
            black_box(results)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 3: Connection Establishment Latency
// ============================================================================

fn bench_connection_establishment(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("connection_establishment");
    group.sample_size(50);

    group.bench_function("full_handshake", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            black_box(connection)
        });
    });

    group.bench_function("connect_and_stream", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let stream = connection.open_bi_stream().await.unwrap();
            black_box(stream)
        });
    });

    group.bench_function("connect_send_receive", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut stream = connection.open_bi_stream().await.unwrap();
            let data = generate_data(1024);
            let mut recv_buf = vec![0u8; 1024];

            stream.send(&data).await.unwrap();
            stream.recv(&mut recv_buf).await.unwrap();

            black_box(recv_buf)
        });
    });

    group.finish();
}

fn bench_connection_reuse(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("connection_reuse");

    group.bench_function("new_connection_per_request", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut stream = connection.open_bi_stream().await.unwrap();
            let data = generate_data(4096);
            let mut recv_buf = vec![0u8; 4096];

            stream.send(&data).await.unwrap();
            stream.recv(&mut recv_buf).await.unwrap();

            black_box(recv_buf)
        });
    });

    group.bench_function("reuse_connection_10_requests", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();

            for _ in 0..10 {
                let mut stream = connection.open_bi_stream().await.unwrap();
                let data = generate_data(4096);
                let mut recv_buf = vec![0u8; 4096];

                stream.send(&data).await.unwrap();
                stream.recv(&mut recv_buf).await.unwrap();
            }

            black_box(())
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 4: Priority Queue Performance
// ============================================================================

fn bench_stream_priorities(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("stream_priorities");

    let priorities = [
        StreamPriority::Critical,
        StreamPriority::High,
        StreamPriority::Normal,
        StreamPriority::Low,
    ];

    for priority in priorities.iter() {
        group.bench_with_input(
            BenchmarkId::new("priority", format!("{:?}", priority)),
            priority,
            |b, &p| {
                b.to_async(&rt).iter(|| async {
                    let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
                    let mut stream = connection.open_bi_stream_with_priority(p).await.unwrap();
                    let data = generate_data(8192);
                    let mut recv_buf = vec![0u8; 8192];

                    stream.send(&data).await.unwrap();
                    stream.recv(&mut recv_buf).await.unwrap();

                    black_box(recv_buf)
                });
            }
        );
    }

    group.bench_function("mixed_priorities", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = Arc::new(QuicConnection::connect(&addr.to_string()).await.unwrap());
            let data = generate_data(4096);

            let mut tasks = Vec::new();

            for (i, &priority) in priorities.iter().cycle().take(20).enumerate() {
                let connection = connection.clone();
                let data = data.clone();

                let task = async move {
                    let mut stream = connection.open_bi_stream_with_priority(priority).await.unwrap();
                    let mut recv_buf = vec![0u8; 4096];
                    stream.send(&data).await.unwrap();
                    stream.recv(&mut recv_buf).await.unwrap();
                    (i, recv_buf)
                };

                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;
            black_box(results)
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 5: Error Recovery Overhead
// ============================================================================

fn bench_error_recovery(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("error_recovery");
    group.sample_size(30);

    group.bench_function("stream_recreation_after_finish", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();

            // Open, use, and finish stream
            let mut stream1 = connection.open_bi_stream().await.unwrap();
            let data = generate_data(1024);
            stream1.send(&data).await.unwrap();
            stream1.finish().await.unwrap();

            // Open new stream on same connection
            let stream2 = connection.open_bi_stream().await.unwrap();

            black_box(stream2)
        });
    });

    group.bench_function("rapid_stream_cycling", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();

            for _ in 0..10 {
                let mut stream = connection.open_bi_stream().await.unwrap();
                let data = generate_data(512);
                stream.send(&data).await.unwrap();
                stream.finish().await.unwrap();
            }

            black_box(())
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 6: Statistics Collection
// ============================================================================

fn bench_stats_collection(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("stats_collection");

    group.bench_function("connection_stats", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let stats = connection.stats();
            black_box(stats)
        });
    });

    group.bench_function("stats_during_transfer", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut stream = connection.open_bi_stream().await.unwrap();
            let data = generate_data(65536);
            let mut recv_buf = vec![0u8; 65536];

            stream.send(&data).await.unwrap();
            let stats1 = connection.stats();
            stream.recv(&mut recv_buf).await.unwrap();
            let stats2 = connection.stats();

            black_box((stats1, stats2))
        });
    });

    group.finish();
}

// ============================================================================
// Benchmark 7: Unidirectional Streams
// ============================================================================

fn bench_unidirectional_streams(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("unidirectional_streams");

    for size in [1024, 65536, 1_048_576].iter() {
        group.throughput(Throughput::Bytes(*size as u64));

        group.bench_with_input(
            BenchmarkId::new("uni_stream", size),
            size,
            |b, &size| {
                b.to_async(&rt).iter(|| async {
                    let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
                    let mut stream = connection.open_uni_stream().await.unwrap();
                    let data = generate_data(size);

                    stream.send(&data).await.unwrap();
                    stream.finish().await.unwrap();

                    black_box(())
                });
            }
        );
    }

    group.finish();
}

// ============================================================================
// Benchmark 8: Realistic Workloads
// ============================================================================

fn bench_realistic_workloads(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let (endpoint, addr) = rt.block_on(create_test_server()).unwrap();

    rt.spawn(run_test_server(endpoint));
    std::thread::sleep(Duration::from_millis(100));

    let mut group = c.benchmark_group("realistic_workloads");
    group.sample_size(20);

    // Simulated HTTP/3 request pattern
    group.bench_function("http3_like_requests", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = Arc::new(QuicConnection::connect(&addr.to_string()).await.unwrap());

            // Simulate 5 parallel requests with different sizes
            let sizes = vec![512, 2048, 8192, 32768, 1024];
            let mut tasks = Vec::new();

            for size in sizes {
                let connection = connection.clone();
                let data = generate_data(size);

                let task = async move {
                    let mut stream = connection.open_bi_stream().await.unwrap();
                    let mut recv_buf = vec![0u8; size];
                    stream.send(&data).await.unwrap();
                    stream.recv(&mut recv_buf).await.unwrap();
                    recv_buf
                };

                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;
            black_box(results)
        });
    });

    // File transfer simulation
    group.bench_function("large_file_transfer_1mb", |b| {
        b.to_async(&rt).iter(|| async {
            let connection = QuicConnection::connect(&addr.to_string()).await.unwrap();
            let mut stream = connection.open_bi_stream().await.unwrap();

            let chunk_size = 65536;
            let total_chunks = 16; // 1 MB total
            let data = generate_data(chunk_size);
            let mut recv_buf = vec![0u8; chunk_size];

            for _ in 0..total_chunks {
                stream.send(&data).await.unwrap();
                stream.recv(&mut recv_buf).await.unwrap();
            }

            black_box(())
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Configuration
// ============================================================================

criterion_group! {
    name = throughput_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(10))
        .warm_up_time(Duration::from_secs(3));
    targets = bench_stream_throughput, bench_sustained_throughput
}

criterion_group! {
    name = multiplexing_benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(15));
    targets = bench_concurrent_streams, bench_sequential_vs_parallel
}

criterion_group! {
    name = connection_benches;
    config = Criterion::default()
        .sample_size(50)
        .measurement_time(Duration::from_secs(8));
    targets = bench_connection_establishment, bench_connection_reuse
}

criterion_group! {
    name = priority_benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(10));
    targets = bench_stream_priorities
}

criterion_group! {
    name = error_benches;
    config = Criterion::default()
        .sample_size(30)
        .measurement_time(Duration::from_secs(8));
    targets = bench_error_recovery
}

criterion_group! {
    name = stats_benches;
    config = Criterion::default()
        .sample_size(100);
    targets = bench_stats_collection
}

criterion_group! {
    name = uni_benches;
    config = Criterion::default()
        .sample_size(50);
    targets = bench_unidirectional_streams
}

criterion_group! {
    name = realistic_benches;
    config = Criterion::default()
        .sample_size(20)
        .measurement_time(Duration::from_secs(12));
    targets = bench_realistic_workloads
}

criterion_main!(
    throughput_benches,
    multiplexing_benches,
    connection_benches,
    priority_benches,
    error_benches,
    stats_benches,
    uni_benches,
    realistic_benches
);
