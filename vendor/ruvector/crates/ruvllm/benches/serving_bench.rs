#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Benchmarks comparing continuous batching to sequential serving
//!
//! Run with: cargo bench --bench serving_bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use ruvllm::backends::{GenerateParams, NoopBackend};
use ruvllm::serving::{
    ContinuousBatchScheduler, InferenceRequest, KvCachePoolConfig, RequestQueue, SchedulerConfig,
    ServingEngine, ServingEngineConfig,
};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Simulates sequential request processing (no batching)
fn sequential_process(requests: &[InferenceRequest]) -> Vec<Duration> {
    let mut latencies = Vec::with_capacity(requests.len());

    for request in requests {
        let start = Instant::now();

        // Simulate prefill
        let prefill_time = Duration::from_micros((request.prompt_len() * 100) as u64);
        std::thread::sleep(prefill_time);

        // Simulate decode (one token at a time)
        let decode_time = Duration::from_micros((request.params.max_tokens * 50) as u64);
        std::thread::sleep(decode_time);

        latencies.push(start.elapsed());
    }

    latencies
}

/// Simulates continuous batching with scheduler
fn continuous_batching_process(requests: Vec<InferenceRequest>) -> Vec<Duration> {
    let config = SchedulerConfig::default();
    let kv_config = KvCachePoolConfig {
        num_slots: 64,
        max_seq_len: 512,
        block_size: 16,
        total_blocks: 1024,
        num_kv_heads: 8,
        head_dim: 128,
        num_layers: 32,
    };

    let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);
    let mut queue = RequestQueue::new();
    let mut latencies = Vec::new();
    let request_times: std::collections::HashMap<_, _> =
        requests.iter().map(|r| (r.id, Instant::now())).collect();

    // Add all requests to queue
    for request in requests {
        queue.add(request);
    }

    // Process iterations until all complete
    let mut iteration = 0;
    let max_iterations = 1000;

    while !queue.is_empty() && iteration < max_iterations {
        let batch = scheduler.schedule(&mut queue);

        if batch.is_empty() {
            break;
        }

        // Simulate batch processing
        // Prefill tokens can be processed in parallel
        let prefill_tokens: usize = batch
            .requests
            .iter()
            .filter(|r| r.is_prefill)
            .map(|r| r.num_tokens())
            .sum();

        // Decode tokens are processed together
        let decode_count = batch.requests.iter().filter(|r| !r.is_prefill).count();

        // Batched prefill is much faster per token
        if prefill_tokens > 0 {
            let batch_prefill_time = Duration::from_micros((prefill_tokens * 20) as u64); // 5x faster
            std::thread::sleep(batch_prefill_time);
        }

        // Batched decode is faster per request
        if decode_count > 0 {
            let batch_decode_time = Duration::from_micros((decode_count * 30) as u64); // ~1.7x faster
            std::thread::sleep(batch_decode_time);

            // Mark completion for decode requests that finished
            for req in &batch.requests {
                if !req.is_prefill {
                    if let Some(running) = queue.running.get_mut(&req.request_id) {
                        running.add_token(0); // Simulate token generation

                        if running.is_complete() {
                            if let Some(start) = request_times.get(&req.request_id) {
                                latencies.push(start.elapsed());
                            }
                        }
                    }
                }
            }
        }

        iteration += 1;
    }

    latencies
}

fn create_test_requests(
    count: usize,
    prompt_len: usize,
    max_tokens: usize,
) -> Vec<InferenceRequest> {
    (0..count)
        .map(|_| {
            let prompt_tokens: Vec<u32> = (0..prompt_len as u32).collect();
            let params = GenerateParams::default().with_max_tokens(max_tokens);
            InferenceRequest::new(prompt_tokens, params)
        })
        .collect()
}

fn bench_scheduler_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("scheduler_overhead");

    for batch_size in [1, 4, 16, 64, 128] {
        group.throughput(Throughput::Elements(batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("schedule", batch_size),
            &batch_size,
            |b, &size| {
                let config = SchedulerConfig::default();
                let kv_config = KvCachePoolConfig::default();
                let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);

                b.iter(|| {
                    let mut queue = RequestQueue::new();
                    let requests = create_test_requests(size, 100, 50);
                    for request in requests {
                        queue.add(request);
                    }
                    let batch = scheduler.schedule(&mut queue);
                    black_box(batch)
                });
            },
        );
    }

    group.finish();
}

fn bench_batch_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("batch_creation");

    for num_requests in [1, 8, 32, 128] {
        group.bench_with_input(
            BenchmarkId::new("create_batch", num_requests),
            &num_requests,
            |b, &count| {
                let config = SchedulerConfig::default();
                let kv_config = KvCachePoolConfig {
                    num_slots: 256,
                    max_seq_len: 512,
                    block_size: 16,
                    total_blocks: 4096,
                    ..Default::default()
                };
                let mut scheduler = ContinuousBatchScheduler::new(config, kv_config);

                b.iter(|| {
                    let mut queue = RequestQueue::new();
                    let requests = create_test_requests(count, 64, 32);
                    for request in requests {
                        queue.add(request);
                    }
                    scheduler.schedule(&mut queue)
                });
            },
        );
    }

    group.finish();
}

fn bench_kv_cache_allocation(c: &mut Criterion) {
    use ruvllm::serving::{KvCacheManager, RequestId};

    let mut group = c.benchmark_group("kv_cache_allocation");

    for max_seq_len in [128, 512, 2048, 4096] {
        group.bench_with_input(
            BenchmarkId::new("allocate", max_seq_len),
            &max_seq_len,
            |b, &seq_len| {
                let config = KvCachePoolConfig {
                    num_slots: 128,
                    max_seq_len: seq_len,
                    block_size: 16,
                    total_blocks: 8192,
                    ..Default::default()
                };
                let mut manager = KvCacheManager::new(config);

                b.iter(|| {
                    let request_id = RequestId::new();
                    let slot = manager.allocate(request_id, seq_len);
                    if let Ok(_) = slot {
                        manager.free(request_id);
                    }
                    black_box(slot)
                });
            },
        );
    }

    group.finish();
}

fn bench_request_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("request_throughput");
    group.measurement_time(Duration::from_secs(5));

    for num_requests in [10, 50, 100] {
        group.throughput(Throughput::Elements(num_requests as u64));

        group.bench_with_input(
            BenchmarkId::new("continuous_batching", num_requests),
            &num_requests,
            |b, &count| {
                b.iter(|| {
                    let requests = create_test_requests(count, 32, 16);
                    continuous_batching_process(requests)
                });
            },
        );
    }

    group.finish();
}

fn bench_serving_engine(c: &mut Criterion) {
    let mut group = c.benchmark_group("serving_engine");

    group.bench_function("submit_request", |b| {
        let backend = Arc::new(NoopBackend);
        let config = ServingEngineConfig {
            kv_cache: KvCachePoolConfig {
                num_slots: 64,
                max_seq_len: 256,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = ServingEngine::new(backend, config);

        b.iter(|| {
            let params = GenerateParams::default().with_max_tokens(10);
            let request = InferenceRequest::new(vec![1, 2, 3, 4, 5], params);
            engine.submit(request)
        });
    });

    group.bench_function("run_iteration", |b| {
        let backend = Arc::new(NoopBackend);
        let config = ServingEngineConfig {
            kv_cache: KvCachePoolConfig {
                num_slots: 64,
                max_seq_len: 256,
                ..Default::default()
            },
            ..Default::default()
        };
        let engine = ServingEngine::new(backend, config);

        // Pre-populate with some requests
        for _ in 0..10 {
            let params = GenerateParams::default().with_max_tokens(5);
            let request = InferenceRequest::new(vec![1, 2, 3], params);
            let _ = engine.submit(request);
        }

        b.iter(|| engine.run_iteration());
    });

    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    group.measurement_time(Duration::from_secs(3));

    // Simulate realistic mixed workload
    group.bench_function("short_prompts_long_gen", |b| {
        b.iter(|| {
            let requests: Vec<_> = (0..20)
                .map(|_| {
                    let prompt_tokens: Vec<u32> = (0..16).collect();
                    let params = GenerateParams::default().with_max_tokens(128);
                    InferenceRequest::new(prompt_tokens, params)
                })
                .collect();
            continuous_batching_process(requests)
        });
    });

    group.bench_function("long_prompts_short_gen", |b| {
        b.iter(|| {
            let requests: Vec<_> = (0..20)
                .map(|_| {
                    let prompt_tokens: Vec<u32> = (0..256).collect();
                    let params = GenerateParams::default().with_max_tokens(16);
                    InferenceRequest::new(prompt_tokens, params)
                })
                .collect();
            continuous_batching_process(requests)
        });
    });

    group.bench_function("mixed_lengths", |b| {
        b.iter(|| {
            let mut requests = Vec::new();

            // Mix of short, medium, and long prompts
            for i in 0..30 {
                let prompt_len = match i % 3 {
                    0 => 16,
                    1 => 64,
                    _ => 256,
                };
                let max_tokens = match i % 3 {
                    0 => 100,
                    1 => 50,
                    _ => 20,
                };

                let prompt_tokens: Vec<u32> = (0..prompt_len).collect();
                let params = GenerateParams::default().with_max_tokens(max_tokens);
                requests.push(InferenceRequest::new(prompt_tokens, params));
            }

            continuous_batching_process(requests)
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_scheduler_overhead,
    bench_batch_creation,
    bench_kv_cache_allocation,
    bench_request_throughput,
    bench_serving_engine,
    bench_mixed_workload,
);

criterion_main!(benches);
