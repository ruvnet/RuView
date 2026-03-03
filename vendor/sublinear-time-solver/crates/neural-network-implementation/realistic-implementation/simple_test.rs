// Simple standalone test to show real neural network performance
// Compile with: rustc -O simple_test.rs

use std::time::{Duration, Instant};

fn relu(x: f32) -> f32 {
    if x > 0.0 { x } else { 0.0 }
}

// Simple 2-layer neural network
struct SimpleNN {
    w1: Vec<Vec<f32>>, // 32x128 weight matrix
    b1: Vec<f32>,      // 32 bias vector
    w2: Vec<Vec<f32>>, // 4x32 weight matrix
    b2: Vec<f32>,      // 4 bias vector
}

impl SimpleNN {
    fn new() -> Self {
        // Initialize with small random weights
        let mut w1 = vec![vec![0.0; 128]; 32];
        let mut w2 = vec![vec![0.0; 32]; 4];

        for i in 0..32 {
            for j in 0..128 {
                w1[i][j] = ((i * j) as f32 * 0.01) % 0.1 - 0.05;
            }
        }

        for i in 0..4 {
            for j in 0..32 {
                w2[i][j] = ((i * j) as f32 * 0.01) % 0.1 - 0.05;
            }
        }

        SimpleNN {
            w1,
            b1: vec![0.0; 32],
            w2,
            b2: vec![0.0; 4],
        }
    }

    fn forward(&self, input: &[f32]) -> Vec<f32> {
        // Layer 1: input (128) -> hidden (32)
        let mut hidden = vec![0.0; 32];
        for i in 0..32 {
            let mut sum = self.b1[i];
            for j in 0..128 {
                sum += self.w1[i][j] * input[j];
            }
            hidden[i] = relu(sum);
        }

        // Layer 2: hidden (32) -> output (4)
        let mut output = vec![0.0; 4];
        for i in 0..4 {
            let mut sum = self.b2[i];
            for j in 0..32 {
                sum += self.w2[i][j] * hidden[j];
            }
            output[i] = sum;
        }

        output
    }
}

fn main() {
    println!("=== Realistic Neural Network Performance Test ===\n");
    println!("Architecture: 128 -> 32 (ReLU) -> 4");
    println!("Pure Rust, no external dependencies");
    println!("Optimized release build\n");

    let nn = SimpleNN::new();
    let input = vec![0.1; 128];

    // Warmup
    for _ in 0..1000 {
        let _ = nn.forward(&input);
    }

    // Actual benchmark
    let iterations = 10000;
    let mut timings = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();
        let output = nn.forward(&input);
        let duration = start.elapsed();

        // Prevent optimization
        if output[0] > 1000.0 {
            println!("Unexpected output");
        }

        timings.push(duration);
    }

    // Sort for percentiles
    timings.sort();

    let p50 = timings[iterations / 2];
    let p90 = timings[iterations * 9 / 10];
    let p99 = timings[iterations * 99 / 100];
    let p999 = timings[iterations * 999 / 1000];

    println!("Results from {} iterations:", iterations);
    println!("  P50:   {:?}", p50);
    println!("  P90:   {:?}", p90);
    println!("  P99:   {:?}", p99);
    println!("  P99.9: {:?}", p999);

    let avg: Duration = timings.iter().sum::<Duration>() / iterations as u32;
    println!("  Average: {:?}", avg);

    println!("\n=== Analysis ===");

    if p999.as_micros() < 900 {
        println!("✅ Sub-0.9ms achieved at P99.9!");
    } else {
        println!("❌ Sub-0.9ms NOT achieved");
        println!("   P99.9 = {:.3}ms", p999.as_secs_f64() * 1000.0);
        println!("   This is REALISTIC for CPU inference");
    }

    println!("\nOperations per inference:");
    println!("  Layer 1: {} multiply-adds", 128 * 32);
    println!("  Layer 2: {} multiply-adds", 32 * 4);
    println!("  Total: {} operations", 128 * 32 + 32 * 4);

    let ops_per_sec = (128 * 32 + 32 * 4) as f64 * iterations as f64
        / timings.iter().sum::<Duration>().as_secs_f64();
    println!("\nThroughput: {:.0} ops/second", ops_per_sec);
}