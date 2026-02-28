// Performance validation tests
//
// Tests latency, memory usage, throughput, and ensures no memory leaks

use super::*;
use std::time::{Duration, Instant};
use tokio;

#[tokio::test]
async fn test_performance_latency_within_bounds() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let image = images::generate_simple_equation("x + y");
    image.save("/tmp/perf_latency.png").unwrap();

    // Measure latency
    let start = Instant::now();
    let result = test_server
        .process_image("/tmp/perf_latency.png", OutputFormat::LaTeX)
        .await
        .expect("Processing failed");
    let latency = start.elapsed();

    println!("Latency: {:?}", latency);
    println!("Confidence: {}", result.confidence);

    // Assert latency is within bounds (<100ms for simple equation)
    assert!(latency.as_millis() < 100, "Latency too high: {:?}", latency);

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_memory_usage_limits() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Get initial memory usage
    let initial_memory = get_memory_usage();

    // Process multiple images
    for i in 0..100 {
        let eq = format!("x + {}", i);
        let image = images::generate_simple_equation(&eq);
        let path = format!("/tmp/perf_mem_{}.png", i);
        image.save(&path).unwrap();

        test_server
            .process_image(&path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        // Clean up
        std::fs::remove_file(&path).unwrap();
    }

    // Get final memory usage
    let final_memory = get_memory_usage();
    let memory_increase = final_memory - initial_memory;

    println!("Memory increase: {} MB", memory_increase / 1024 / 1024);

    // Assert memory usage is reasonable (<100MB increase)
    assert!(
        memory_increase < 100 * 1024 * 1024,
        "Memory usage too high: {} bytes",
        memory_increase
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_no_memory_leaks() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let image = images::generate_simple_equation("leak test");
    image.save("/tmp/leak_test.png").unwrap();

    // Process same image many times
    let iterations = 1000;
    let mut memory_samples = Vec::new();

    for i in 0..iterations {
        test_server
            .process_image("/tmp/leak_test.png", OutputFormat::LaTeX)
            .await
            .expect("Processing failed");

        // Sample memory every 100 iterations
        if i % 100 == 0 {
            memory_samples.push(get_memory_usage());
        }
    }

    // Check for linear memory growth (leak indicator)
    let first_sample = memory_samples[0];
    let last_sample = *memory_samples.last().unwrap();
    let growth_rate = (last_sample - first_sample) as f64 / iterations as f64;

    println!("Memory growth rate: {} bytes/iteration", growth_rate);
    println!("Samples: {:?}", memory_samples);

    // Growth rate should be minimal (<1KB per iteration)
    assert!(
        growth_rate < 1024.0,
        "Possible memory leak detected: {} bytes/iteration",
        growth_rate
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_throughput() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create test images
    let image_count = 50;
    for i in 0..image_count {
        let eq = format!("throughput_{}", i);
        let image = images::generate_simple_equation(&eq);
        image.save(&format!("/tmp/throughput_{}.png", i)).unwrap();
    }

    // Measure throughput
    let start = Instant::now();

    for i in 0..image_count {
        test_server
            .process_image(&format!("/tmp/throughput_{}.png", i), OutputFormat::LaTeX)
            .await
            .expect("Processing failed");
    }

    let duration = start.elapsed();
    let throughput = image_count as f64 / duration.as_secs_f64();

    println!("Throughput: {:.2} images/second", throughput);
    println!("Total time: {:?} for {} images", duration, image_count);

    // Assert reasonable throughput (>5 images/second)
    assert!(
        throughput > 5.0,
        "Throughput too low: {:.2} images/s",
        throughput
    );

    // Cleanup
    for i in 0..image_count {
        std::fs::remove_file(&format!("/tmp/throughput_{}.png", i)).unwrap();
    }

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_concurrent_throughput() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create test image
    let image = images::generate_simple_equation("concurrent");
    image.save("/tmp/concurrent_throughput.png").unwrap();

    let concurrent_requests = 20;
    let start = Instant::now();

    // Spawn concurrent requests
    let mut handles = vec![];
    for _ in 0..concurrent_requests {
        let server = test_server.clone();
        let handle = tokio::spawn(async move {
            server
                .process_image("/tmp/concurrent_throughput.png", OutputFormat::LaTeX)
                .await
        });
        handles.push(handle);
    }

    // Wait for all to complete
    let results = futures::future::join_all(handles).await;
    let duration = start.elapsed();

    let success_count = results.iter().filter(|r| r.is_ok()).count();
    let throughput = concurrent_requests as f64 / duration.as_secs_f64();

    println!("Concurrent throughput: {:.2} req/second", throughput);
    println!("Success rate: {}/{}", success_count, concurrent_requests);

    assert!(
        success_count == concurrent_requests,
        "All requests should succeed"
    );
    assert!(
        throughput > 10.0,
        "Concurrent throughput too low: {:.2}",
        throughput
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_latency_percentiles() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    let iterations = 100;
    let mut latencies = Vec::new();

    for i in 0..iterations {
        let eq = format!("p{}", i);
        let image = images::generate_simple_equation(&eq);
        let path = format!("/tmp/percentile_{}.png", i);
        image.save(&path).unwrap();

        let start = Instant::now();
        test_server
            .process_image(&path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");
        let latency = start.elapsed();

        latencies.push(latency.as_micros());

        std::fs::remove_file(&path).unwrap();
    }

    // Sort latencies
    latencies.sort();

    // Calculate percentiles
    let p50 = latencies[50];
    let p95 = latencies[95];
    let p99 = latencies[99];

    println!("Latency percentiles:");
    println!("  P50: {} μs ({} ms)", p50, p50 / 1000);
    println!("  P95: {} μs ({} ms)", p95, p95 / 1000);
    println!("  P99: {} μs ({} ms)", p99, p99 / 1000);

    // Assert percentile targets
    assert!(p50 < 100_000, "P50 latency too high: {} μs", p50); // <100ms
    assert!(p95 < 200_000, "P95 latency too high: {} μs", p95); // <200ms
    assert!(p99 < 500_000, "P99 latency too high: {} μs", p99); // <500ms

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_batch_efficiency() {
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");

    // Create test images
    let batch_size = 10;
    let mut paths = Vec::new();

    for i in 0..batch_size {
        let eq = format!("batch_{}", i);
        let image = images::generate_simple_equation(&eq);
        let path = format!("/tmp/batch_eff_{}.png", i);
        image.save(&path).unwrap();
        paths.push(path);
    }

    // Measure sequential processing
    let start_sequential = Instant::now();
    for path in &paths {
        test_server
            .process_image(path, OutputFormat::LaTeX)
            .await
            .expect("Processing failed");
    }
    let sequential_time = start_sequential.elapsed();

    // Measure batch processing
    let start_batch = Instant::now();
    test_server
        .process_batch(
            &paths.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            OutputFormat::LaTeX,
        )
        .await
        .expect("Batch processing failed");
    let batch_time = start_batch.elapsed();

    println!("Sequential time: {:?}", sequential_time);
    println!("Batch time: {:?}", batch_time);
    println!(
        "Speedup: {:.2}x",
        sequential_time.as_secs_f64() / batch_time.as_secs_f64()
    );

    // Batch should be faster
    assert!(
        batch_time < sequential_time,
        "Batch processing should be faster"
    );

    // Cleanup
    for path in paths {
        std::fs::remove_file(&path).unwrap();
    }

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_performance_cold_start_warmup() {
    // Measure cold start
    let start_cold = Instant::now();
    let test_server = TestServer::start()
        .await
        .expect("Failed to start test server");
    let cold_start_time = start_cold.elapsed();

    println!("Cold start time: {:?}", cold_start_time);

    // First request (warmup)
    let image = images::generate_simple_equation("warmup");
    image.save("/tmp/warmup.png").unwrap();

    let start_first = Instant::now();
    test_server
        .process_image("/tmp/warmup.png", OutputFormat::LaTeX)
        .await
        .expect("Processing failed");
    let first_request_time = start_first.elapsed();

    // Second request (warmed up)
    let start_second = Instant::now();
    test_server
        .process_image("/tmp/warmup.png", OutputFormat::LaTeX)
        .await
        .expect("Processing failed");
    let second_request_time = start_second.elapsed();

    println!("First request time: {:?}", first_request_time);
    println!("Second request time: {:?}", second_request_time);

    // Cold start should be reasonable (<5s)
    assert!(
        cold_start_time.as_secs() < 5,
        "Cold start too slow: {:?}",
        cold_start_time
    );

    // Second request should be faster (model loaded)
    assert!(
        second_request_time < first_request_time,
        "Warmed up request should be faster"
    );

    test_server.shutdown().await;
}

// Helper function to get current memory usage
fn get_memory_usage() -> usize {
    #[cfg(target_os = "linux")]
    {
        // Read from /proc/self/statm
        if let Ok(content) = std::fs::read_to_string("/proc/self/statm") {
            if let Some(rss) = content.split_whitespace().nth(1) {
                if let Ok(pages) = rss.parse::<usize>() {
                    // Convert pages to bytes (assuming 4KB pages)
                    return pages * 4096;
                }
            }
        }
    }

    // Fallback for other platforms or if reading fails
    0
}
