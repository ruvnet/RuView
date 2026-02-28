//! Quick benchmark to compare NEON SIMD vs scalar performance on Apple Silicon
//!
//! Run with: cargo run --example neon_benchmark --release -p ruvector-core

use std::time::Instant;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     NEON SIMD Benchmark for Apple Silicon (M4 Pro)        ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    // Test parameters
    let dimensions = 128; // Common embedding dimension
    let num_vectors = 10_000;
    let num_queries = 1_000;

    // Generate test data
    let vectors: Vec<Vec<f32>> = (0..num_vectors)
        .map(|i| {
            (0..dimensions)
                .map(|j| ((i * j) % 1000) as f32 / 1000.0)
                .collect()
        })
        .collect();

    let queries: Vec<Vec<f32>> = (0..num_queries)
        .map(|i| {
            (0..dimensions)
                .map(|j| ((i * j + 500) % 1000) as f32 / 1000.0)
                .collect()
        })
        .collect();

    println!("Configuration:");
    println!("  - Dimensions: {}", dimensions);
    println!("  - Vectors: {}", num_vectors);
    println!("  - Queries: {}", num_queries);
    println!(
        "  - Total distance calculations: {}\n",
        num_vectors * num_queries
    );

    #[cfg(target_arch = "aarch64")]
    println!("Platform: ARM64 (Apple Silicon) - NEON enabled ✓\n");

    #[cfg(target_arch = "x86_64")]
    println!("Platform: x86_64 - AVX2 detection enabled\n");

    // Benchmark Euclidean distance (SIMD)
    println!("═══════════════════════════════════════════════════════════════");
    println!("Euclidean Distance:");
    println!("═══════════════════════════════════════════════════════════════");

    let start = Instant::now();
    let mut simd_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            simd_sum += euclidean_simd(query, vec);
        }
    }
    let simd_time = start.elapsed();
    println!(
        "  SIMD:   {:>8.2} ms  (checksum: {:.4})",
        simd_time.as_secs_f64() * 1000.0,
        simd_sum
    );

    let start = Instant::now();
    let mut scalar_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            scalar_sum += euclidean_scalar(query, vec);
        }
    }
    let scalar_time = start.elapsed();
    println!(
        "  Scalar: {:>8.2} ms  (checksum: {:.4})",
        scalar_time.as_secs_f64() * 1000.0,
        scalar_sum
    );

    let speedup = scalar_time.as_secs_f64() / simd_time.as_secs_f64();
    println!("  Speedup: {:.2}x\n", speedup);

    // Benchmark Dot Product (SIMD)
    println!("═══════════════════════════════════════════════════════════════");
    println!("Dot Product:");
    println!("═══════════════════════════════════════════════════════════════");

    let start = Instant::now();
    let mut simd_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            simd_sum += dot_simd(query, vec);
        }
    }
    let simd_time = start.elapsed();
    println!(
        "  SIMD:   {:>8.2} ms  (checksum: {:.4})",
        simd_time.as_secs_f64() * 1000.0,
        simd_sum
    );

    let start = Instant::now();
    let mut scalar_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            scalar_sum += dot_scalar(query, vec);
        }
    }
    let scalar_time = start.elapsed();
    println!(
        "  Scalar: {:>8.2} ms  (checksum: {:.4})",
        scalar_time.as_secs_f64() * 1000.0,
        scalar_sum
    );

    let speedup = scalar_time.as_secs_f64() / simd_time.as_secs_f64();
    println!("  Speedup: {:.2}x\n", speedup);

    // Benchmark Cosine Similarity (SIMD)
    println!("═══════════════════════════════════════════════════════════════");
    println!("Cosine Similarity:");
    println!("═══════════════════════════════════════════════════════════════");

    let start = Instant::now();
    let mut simd_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            simd_sum += cosine_simd(query, vec);
        }
    }
    let simd_time = start.elapsed();
    println!(
        "  SIMD:   {:>8.2} ms  (checksum: {:.4})",
        simd_time.as_secs_f64() * 1000.0,
        simd_sum
    );

    let start = Instant::now();
    let mut scalar_sum = 0.0f32;
    for query in &queries {
        for vec in &vectors {
            scalar_sum += cosine_scalar(query, vec);
        }
    }
    let scalar_time = start.elapsed();
    println!(
        "  Scalar: {:>8.2} ms  (checksum: {:.4})",
        scalar_time.as_secs_f64() * 1000.0,
        scalar_sum
    );

    let speedup = scalar_time.as_secs_f64() / simd_time.as_secs_f64();
    println!("  Speedup: {:.2}x\n", speedup);

    println!("═══════════════════════════════════════════════════════════════");
    println!("Benchmark complete!");
}

// SIMD implementations (use the crate's SIMD functions)
#[cfg(target_arch = "aarch64")]
use std::arch::aarch64::*;

#[inline]
fn euclidean_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let len = a.len();
        let mut sum = vdupq_n_f32(0.0);
        let chunks = len / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = vld1q_f32(a.as_ptr().add(idx));
            let vb = vld1q_f32(b.as_ptr().add(idx));
            let diff = vsubq_f32(va, vb);
            sum = vfmaq_f32(sum, diff, diff);
        }
        let mut total = vaddvq_f32(sum);
        for i in (chunks * 4)..len {
            let diff = a[i] - b[i];
            total += diff * diff;
        }
        total.sqrt()
    }
    #[cfg(not(target_arch = "aarch64"))]
    euclidean_scalar(a, b)
}

#[inline]
fn euclidean_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f32>()
        .sqrt()
}

#[inline]
fn dot_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let len = a.len();
        let mut sum = vdupq_n_f32(0.0);
        let chunks = len / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = vld1q_f32(a.as_ptr().add(idx));
            let vb = vld1q_f32(b.as_ptr().add(idx));
            sum = vfmaq_f32(sum, va, vb);
        }
        let mut total = vaddvq_f32(sum);
        for i in (chunks * 4)..len {
            total += a[i] * b[i];
        }
        total
    }
    #[cfg(not(target_arch = "aarch64"))]
    dot_scalar(a, b)
}

#[inline]
fn dot_scalar(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[inline]
fn cosine_simd(a: &[f32], b: &[f32]) -> f32 {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        let len = a.len();
        let mut dot = vdupq_n_f32(0.0);
        let mut norm_a = vdupq_n_f32(0.0);
        let mut norm_b = vdupq_n_f32(0.0);
        let chunks = len / 4;
        for i in 0..chunks {
            let idx = i * 4;
            let va = vld1q_f32(a.as_ptr().add(idx));
            let vb = vld1q_f32(b.as_ptr().add(idx));
            dot = vfmaq_f32(dot, va, vb);
            norm_a = vfmaq_f32(norm_a, va, va);
            norm_b = vfmaq_f32(norm_b, vb, vb);
        }
        let mut dot_sum = vaddvq_f32(dot);
        let mut norm_a_sum = vaddvq_f32(norm_a);
        let mut norm_b_sum = vaddvq_f32(norm_b);
        for i in (chunks * 4)..len {
            dot_sum += a[i] * b[i];
            norm_a_sum += a[i] * a[i];
            norm_b_sum += b[i] * b[i];
        }
        dot_sum / (norm_a_sum.sqrt() * norm_b_sum.sqrt())
    }
    #[cfg(not(target_arch = "aarch64"))]
    cosine_scalar(a, b)
}

#[inline]
fn cosine_scalar(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
