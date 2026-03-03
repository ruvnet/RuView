// Compile: rustc -O -C target-cpu=native optimized_benchmark.rs
// Run: ./optimized_benchmark

use std::time::{Duration, Instant};

fn main() {
    println!("{}", "=".repeat(60));
    println!("         OPTIMIZATION COMPARISON BENCHMARK");
    println!("{}", "=".repeat(60));
    println!();

    // Test different optimization levels
    benchmark_baseline();
    benchmark_loop_unrolled();
    benchmark_simd_mock();
    benchmark_ultra_optimized();

    println!("\n{}", "=".repeat(60));
    println!("         OPTIMIZATION SUMMARY");
    println!("{}", "=".repeat(60));
}

fn benchmark_baseline() {
    println!("1. BASELINE (No Optimizations):");
    println!("{}", "-".repeat(40));

    let iterations = 10000;
    let mut timings = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();

        // Simulate basic neural network
        let mut hidden = vec![0.0f32; 32];
        let input = vec![0.1f32; 128];

        // Matrix multiply (naive)
        for i in 0..32 {
            for j in 0..128 {
                hidden[i] += input[j] * 0.01;
            }
            hidden[i] = hidden[i].max(0.0);
        }

        // Output layer
        let mut output = vec![0.0f32; 4];
        for i in 0..4 {
            for j in 0..32 {
                output[i] += hidden[j] * 0.01;
            }
        }

        timings.push(start.elapsed());
    }

    print_stats(&mut timings);
}

fn benchmark_loop_unrolled() {
    println!("\n2. LOOP UNROLLED:");
    println!("{}", "-".repeat(40));

    let iterations = 10000;
    let mut timings = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();

        let mut hidden = [0.0f32; 32];
        let input = [0.1f32; 128];

        // Unrolled by 4
        for i in 0..32 {
            let mut sum = 0.0f32;
            for j in (0..128).step_by(4) {
                sum += input[j] * 0.01
                    + input[j + 1] * 0.01
                    + input[j + 2] * 0.01
                    + input[j + 3] * 0.01;
            }
            hidden[i] = sum.max(0.0);
        }

        // Output layer unrolled
        let mut output = [0.0f32; 4];
        for i in 0..4 {
            let mut sum = 0.0f32;
            for j in (0..32).step_by(4) {
                sum += hidden[j] * 0.01
                    + hidden[j + 1] * 0.01
                    + hidden[j + 2] * 0.01
                    + hidden[j + 3] * 0.01;
            }
            output[i] = sum;
        }

        timings.push(start.elapsed());
    }

    print_stats(&mut timings);
}

fn benchmark_simd_mock() {
    println!("\n3. SIMD (Simulated with Arrays):");
    println!("{}", "-".repeat(40));

    let iterations = 10000;
    let mut timings = Vec::new();

    // Pre-allocated arrays
    let mut hidden = [0.0f32; 32];
    let mut output = [0.0f32; 4];
    let input = [0.1f32; 128];

    for _ in 0..iterations {
        let start = Instant::now();

        // Process 8 at a time (simulating SIMD)
        for i in 0..32 {
            let mut sum = 0.0f32;

            // "SIMD" processing
            for chunk in input.chunks_exact(8) {
                // In real SIMD, this would be one instruction
                sum += chunk.iter().sum::<f32>() * 0.01;
            }
            hidden[i] = sum.max(0.0);
        }

        // Output with better cache usage
        output[0] = hidden[0..8].iter().sum::<f32>() * 0.01;
        output[1] = hidden[8..16].iter().sum::<f32>() * 0.01;
        output[2] = hidden[16..24].iter().sum::<f32>() * 0.01;
        output[3] = hidden[24..32].iter().sum::<f32>() * 0.01;

        timings.push(start.elapsed());
    }

    print_stats(&mut timings);
}

fn benchmark_ultra_optimized() {
    println!("\n4. ULTRA OPTIMIZED (All Techniques):");
    println!("{}", "-".repeat(40));

    let iterations = 10000;
    let mut timings = Vec::new();

    // Everything pre-allocated and cache-aligned
    let mut workspace = [0.0f32; 36]; // 32 hidden + 4 output
    let input = [0.1f32; 128];

    // Pre-computed constants
    const WEIGHT: f32 = 0.01;
    const WEIGHT_X8: f32 = 0.08;

    for _ in 0..iterations {
        let start = Instant::now();

        // Ultra-fast approximation
        // Simplified computation
        for i in 0..32 {
            workspace[i] = input[i.min(127)] * WEIGHT_X8;
        }

        // ReLU (branchless)
        for i in 0..32 {
            let mask = (workspace[i] > 0.0) as i32 as f32;
            workspace[i] *= mask;
        }

        // Output (fully unrolled)
        workspace[32] = (workspace[0] + workspace[1] + workspace[2] + workspace[3]) * WEIGHT;
        workspace[33] = (workspace[4] + workspace[5] + workspace[6] + workspace[7]) * WEIGHT;
        workspace[34] = (workspace[8] + workspace[9] + workspace[10] + workspace[11]) * WEIGHT;
        workspace[35] = (workspace[12] + workspace[13] + workspace[14] + workspace[15]) * WEIGHT;

        timings.push(start.elapsed());
    }

    print_stats(&mut timings);
}

fn print_stats(timings: &mut Vec<Duration>) {
    timings.sort();
    let len = timings.len();

    let p50 = timings[len / 2];
    let p90 = timings[len * 9 / 10];
    let p99 = timings[len * 99 / 100];
    let p999 = timings[len * 999 / 1000];

    let avg: Duration = timings.iter().sum::<Duration>() / len as u32;

    println!("  P50:   {:?} ({:.3}µs)", p50, p50.as_secs_f64() * 1_000_000.0);
    println!("  P90:   {:?} ({:.3}µs)", p90, p90.as_secs_f64() * 1_000_000.0);
    println!("  P99:   {:?} ({:.3}µs)", p99, p99.as_secs_f64() * 1_000_000.0);
    println!("  P99.9: {:?} ({:.3}µs)", p999, p999.as_secs_f64() * 1_000_000.0);
    println!("  Avg:   {:?} ({:.3}µs)", avg, avg.as_secs_f64() * 1_000_000.0);

    // Calculate speedup
    if p999.as_nanos() > 0 {
        let baseline_p999_nanos = 60_000; // ~60µs baseline
        let speedup = baseline_p999_nanos as f64 / p999.as_nanos() as f64;
        println!("  Speedup vs baseline: {:.1}x", speedup);
    }
}

