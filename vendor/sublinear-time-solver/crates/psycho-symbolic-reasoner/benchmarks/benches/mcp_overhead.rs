use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use std::time::{Duration, Instant};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::collections::HashMap;
use serde_json::{json, Value};

#[derive(Debug, Clone)]
struct MCPInvocationResult {
    tool_name: String,
    invocation_time: Duration,
    response_size: usize,
    success: bool,
    error_message: Option<String>,
}

struct MCPOverheadProfiler {
    results: Arc<Mutex<Vec<MCPInvocationResult>>>,
}

impl MCPOverheadProfiler {
    fn new() -> Self {
        Self {
            results: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn record_invocation(&self, result: MCPInvocationResult) {
        if let Ok(mut results) = self.results.lock() {
            results.push(result);
        }
    }

    fn get_average_overhead(&self, tool_name: &str) -> Option<Duration> {
        if let Ok(results) = self.results.lock() {
            let filtered: Vec<_> = results.iter()
                .filter(|r| r.tool_name == tool_name && r.success)
                .collect();

            if filtered.is_empty() {
                return None;
            }

            let total_nanos: u64 = filtered.iter()
                .map(|r| r.invocation_time.as_nanos() as u64)
                .sum();

            Some(Duration::from_nanos(total_nanos / filtered.len() as u64))
        } else {
            None
        }
    }

    fn get_success_rate(&self, tool_name: &str) -> Option<f64> {
        if let Ok(results) = self.results.lock() {
            let filtered: Vec<_> = results.iter()
                .filter(|r| r.tool_name == tool_name)
                .collect();

            if filtered.is_empty() {
                return None;
            }

            let successful = filtered.iter().filter(|r| r.success).count();
            Some(successful as f64 / filtered.len() as f64)
        } else {
            None
        }
    }
}

// Mock MCP tool invocation for benchmarking
fn simulate_mcp_tool_invocation(tool_name: &str, payload: &Value) -> MCPInvocationResult {
    let start = Instant::now();

    // Simulate different types of MCP overhead
    let base_overhead = match tool_name {
        "claude-flow::swarm_init" => Duration::from_millis(50),
        "claude-flow::agent_spawn" => Duration::from_millis(30),
        "claude-flow::task_orchestrate" => Duration::from_millis(100),
        "claude-flow::swarm_status" => Duration::from_millis(20),
        "claude-flow::neural_train" => Duration::from_millis(200),
        "claude-flow::memory_usage" => Duration::from_millis(10),
        _ => Duration::from_millis(25),
    };

    // Simulate payload processing overhead
    let payload_size = payload.to_string().len();
    let payload_overhead = Duration::from_nanos(payload_size as u64 * 100); // 100ns per byte

    // Simulate network-like delays
    let network_jitter = Duration::from_millis(rand::random::<u64>() % 20);

    let total_overhead = base_overhead + payload_overhead + network_jitter;
    thread::sleep(total_overhead);

    let invocation_time = start.elapsed();

    // Simulate occasional failures
    let success = rand::random::<f64>() > 0.02; // 2% failure rate

    MCPInvocationResult {
        tool_name: tool_name.to_string(),
        invocation_time,
        response_size: 1024 + (payload_size / 2), // Simulate response size
        success,
        error_message: if success { None } else { Some("Simulated MCP error".to_string()) },
    }
}

fn bench_basic_mcp_tool_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("basic_mcp_overhead");

    let tools = vec![
        "claude-flow::swarm_init",
        "claude-flow::agent_spawn",
        "claude-flow::task_orchestrate",
        "claude-flow::swarm_status",
        "claude-flow::neural_train",
        "claude-flow::memory_usage",
    ];

    for tool in tools {
        let payload = json!({
            "test": true,
            "timestamp": "2023-01-01T00:00:00Z",
            "data": vec![1, 2, 3, 4, 5]
        });

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::new("tool_invocation", tool),
            &(tool, payload),
            |b, (tool, payload)| {
                b.iter(|| {
                    let result = simulate_mcp_tool_invocation(black_box(tool), black_box(payload));
                    black_box(result);
                });
            }
        );
    }

    group.finish();
}

fn bench_payload_size_impact(c: &mut Criterion) {
    let mut group = c.benchmark_group("payload_size_impact");

    let payload_sizes = [100, 1000, 10000, 100000]; // bytes
    let tool_name = "claude-flow::task_orchestrate";

    for &size in payload_sizes.iter() {
        // Generate payload of specified size
        let large_data: Vec<u32> = (0..size/4).collect(); // Approximate size
        let payload = json!({
            "task": "large_data_processing",
            "data": large_data,
            "metadata": {
                "size": size,
                "type": "benchmark"
            }
        });

        group.throughput(Throughput::Bytes(size as u64));
        group.bench_with_input(
            BenchmarkId::new("payload_size", size),
            &payload,
            |b, payload| {
                b.iter(|| {
                    let result = simulate_mcp_tool_invocation(
                        black_box(tool_name),
                        black_box(payload)
                    );
                    black_box(result);
                });
            }
        );
    }

    group.finish();
}

fn bench_concurrent_mcp_invocations(c: &mut Criterion) {
    let mut group = c.benchmark_group("concurrent_mcp_invocations");

    let concurrency_levels = [1, 2, 4, 8, 16];

    for &concurrency in concurrency_levels.iter() {
        let payload = json!({
            "concurrent_test": true,
            "thread_count": concurrency
        });

        group.throughput(Throughput::Elements(concurrency as u64));
        group.bench_with_input(
            BenchmarkId::new("concurrent_calls", concurrency),
            &concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    let handles: Vec<_> = (0..concurrency).map(|i| {
                        let payload_clone = payload.clone();
                        let tool_name = match i % 3 {
                            0 => "claude-flow::swarm_status",
                            1 => "claude-flow::agent_spawn",
                            _ => "claude-flow::memory_usage",
                        };

                        thread::spawn(move || {
                            simulate_mcp_tool_invocation(tool_name, &payload_clone)
                        })
                    }).collect();

                    let results: Vec<_> = handles.into_iter()
                        .map(|h| h.join().unwrap())
                        .collect();

                    black_box(results);
                });
            }
        );
    }

    group.finish();
}

fn bench_mcp_tool_chain_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_tool_chain_overhead");

    // Simulate common MCP tool chains
    let tool_chains = vec![
        vec!["claude-flow::swarm_init", "claude-flow::agent_spawn", "claude-flow::task_orchestrate"],
        vec!["claude-flow::memory_usage", "claude-flow::neural_train", "claude-flow::swarm_status"],
        vec!["claude-flow::swarm_init", "claude-flow::agent_spawn", "claude-flow::agent_spawn", "claude-flow::task_orchestrate", "claude-flow::swarm_status"],
    ];

    for (i, chain) in tool_chains.iter().enumerate() {
        group.throughput(Throughput::Elements(chain.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("tool_chain", i),
            chain,
            |b, chain| {
                b.iter(|| {
                    let mut results = Vec::new();

                    for (j, &tool) in chain.iter().enumerate() {
                        let payload = json!({
                            "chain_step": j,
                            "tool": tool,
                            "previous_results": results.len()
                        });

                        let result = simulate_mcp_tool_invocation(
                            black_box(tool),
                            black_box(&payload)
                        );
                        results.push(result);
                    }

                    black_box(results);
                });
            }
        );
    }

    group.finish();
}

fn bench_mcp_error_handling_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_error_handling");

    // Test with different error rates
    let error_scenarios = vec![
        ("normal_operation", 0.02),  // 2% error rate
        ("high_error_rate", 0.20),   // 20% error rate
        ("failing_service", 0.50),   // 50% error rate
    ];

    for (scenario_name, error_rate) in error_scenarios {
        group.bench_with_input(
            BenchmarkId::new("error_scenario", scenario_name),
            &error_rate,
            |b, &error_rate| {
                b.iter(|| {
                    let payload = json!({
                        "error_simulation": true,
                        "error_rate": error_rate
                    });

                    // Simulate error-prone invocations
                    let mut results = Vec::new();
                    for i in 0..10 {
                        let success_override = rand::random::<f64>() > error_rate;
                        let mut result = simulate_mcp_tool_invocation(
                            "claude-flow::task_orchestrate",
                            &payload
                        );

                        // Override success based on error rate
                        if !success_override {
                            result.success = false;
                            result.error_message = Some(format!("Simulated error {}", i));
                        }

                        results.push(result);

                        // Simulate retry logic overhead for failed calls
                        if !result.success {
                            thread::sleep(Duration::from_millis(10)); // Retry delay
                        }
                    }

                    black_box(results);
                });
            }
        );
    }

    group.finish();
}

fn bench_mcp_memory_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_memory_overhead");

    group.bench_function("memory_accumulation", |b| {
        b.iter_custom(|iters| {
            let profiler = MCPOverheadProfiler::new();
            let start = Instant::now();

            for iteration in 0..iters {
                // Simulate memory accumulation over many MCP calls
                let mut accumulated_results = Vec::new();

                for i in 0..100 {
                    let payload = json!({
                        "iteration": iteration,
                        "call_number": i,
                        "large_data": vec![0u8; 1024] // 1KB per call
                    });

                    let result = simulate_mcp_tool_invocation(
                        "claude-flow::memory_usage",
                        &payload
                    );

                    profiler.record_invocation(result.clone());
                    accumulated_results.push(result);
                }

                // Simulate periodic cleanup
                if iteration % 10 == 0 {
                    accumulated_results.clear();
                }

                black_box(accumulated_results);
            }

            start.elapsed()
        });
    });

    group.finish();
}

fn bench_mcp_serialization_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_serialization_overhead");

    let data_types = vec![
        ("simple_object", json!({"key": "value", "number": 42})),
        ("nested_object", json!({
            "level1": {
                "level2": {
                    "level3": {
                        "data": vec![1, 2, 3, 4, 5],
                        "metadata": {"type": "nested"}
                    }
                }
            }
        })),
        ("large_array", json!({"data": vec![0; 10000]})),
        ("mixed_types", json!({
            "string": "test",
            "number": 123.456,
            "boolean": true,
            "null": null,
            "array": [1, "two", 3.0, true, null],
            "object": {"nested": {"value": 42}}
        })),
    ];

    for (data_type, payload) in data_types {
        group.bench_with_input(
            BenchmarkId::new("serialization", data_type),
            &payload,
            |b, payload| {
                b.iter(|| {
                    // Measure serialization overhead
                    let start = Instant::now();
                    let serialized = serde_json::to_string(black_box(payload)).unwrap();
                    let serialization_time = start.elapsed();

                    // Simulate MCP call
                    let mcp_start = Instant::now();
                    let result = simulate_mcp_tool_invocation(
                        "claude-flow::task_orchestrate",
                        payload
                    );
                    let mcp_time = mcp_start.elapsed();

                    // Measure deserialization overhead
                    let deser_start = Instant::now();
                    let _deserialized: Value = serde_json::from_str(&serialized).unwrap();
                    let deserialization_time = deser_start.elapsed();

                    black_box((
                        serialization_time,
                        mcp_time,
                        deserialization_time,
                        result
                    ));
                });
            }
        );
    }

    group.finish();
}

fn bench_mcp_batching_efficiency(c: &mut Criterion) {
    let mut group = c.benchmark_group("mcp_batching_efficiency");

    let batch_sizes = [1, 5, 10, 20, 50];

    for &batch_size in batch_sizes.iter() {
        group.throughput(Throughput::Elements(batch_size as u64));

        // Individual calls
        group.bench_with_input(
            BenchmarkId::new("individual_calls", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let mut results = Vec::new();

                    for i in 0..batch_size {
                        let payload = json!({
                            "batch_item": i,
                            "data": format!("item_{}", i)
                        });

                        let result = simulate_mcp_tool_invocation(
                            "claude-flow::agent_spawn",
                            &payload
                        );
                        results.push(result);
                    }

                    black_box(results);
                });
            }
        );

        // Batched call
        group.bench_with_input(
            BenchmarkId::new("batched_call", batch_size),
            &batch_size,
            |b, &batch_size| {
                b.iter(|| {
                    let batch_payload = json!({
                        "batch": true,
                        "items": (0..batch_size).map(|i| json!({
                            "item_id": i,
                            "data": format!("item_{}", i)
                        })).collect::<Vec<_>>()
                    });

                    // Simulate batched processing (typically more efficient)
                    let start = Instant::now();
                    let batch_overhead = Duration::from_millis(50 + (batch_size as u64 * 2));
                    thread::sleep(batch_overhead);

                    let result = MCPInvocationResult {
                        tool_name: "claude-flow::batch_agent_spawn".to_string(),
                        invocation_time: start.elapsed(),
                        response_size: batch_size * 512,
                        success: true,
                        error_message: None,
                    };

                    black_box(result);
                });
            }
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_basic_mcp_tool_overhead,
    bench_payload_size_impact,
    bench_concurrent_mcp_invocations,
    bench_mcp_tool_chain_overhead,
    bench_mcp_error_handling_overhead,
    bench_mcp_memory_overhead,
    bench_mcp_serialization_overhead,
    bench_mcp_batching_efficiency
);

criterion_main!(benches);