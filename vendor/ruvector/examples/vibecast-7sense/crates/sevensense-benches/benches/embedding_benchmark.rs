//! Embedding Benchmark Suite for 7sense
//!
//! Performance targets from ADR-004:
//! - Embedding inference: >100 segments/second
//! - Mel spectrogram compute: <20ms per segment
//! - Embedding normalization: <5ms per segment
//! - Batch ingestion: 1M vectors/minute

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::time::Duration;

use sevensense_benches::*;

/// Audio segment parameters (5 seconds at 32kHz)
const AUDIO_SAMPLE_RATE: usize = 32_000;
const SEGMENT_DURATION_SECS: f32 = 5.0;
const SEGMENT_SAMPLES: usize = (AUDIO_SAMPLE_RATE as f32 * SEGMENT_DURATION_SECS) as usize;

/// Mel spectrogram parameters
const N_MELS: usize = 128;
const N_FFT: usize = 2048;
const HOP_LENGTH: usize = 512;
const MEL_FRAMES: usize = (SEGMENT_SAMPLES / HOP_LENGTH) + 1;

// ============================================================================
// Simulated Audio Processing
// ============================================================================

/// Generate synthetic audio samples for benchmarking
fn generate_audio_segment() -> Vec<f32> {
    let mut samples = Vec::with_capacity(SEGMENT_SAMPLES);
    let mut seed = 12345u64;

    for i in 0..SEGMENT_SAMPLES {
        // Simple synthetic audio with multiple frequencies
        let t = i as f32 / AUDIO_SAMPLE_RATE as f32;
        let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.3
            + (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.2
            + (2.0 * std::f32::consts::PI * 1320.0 * t).sin() * 0.1;

        // Add some noise
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let noise = ((seed >> 33) as f32 / u32::MAX as f32) * 0.1 - 0.05;

        samples.push(sample + noise);
    }

    samples
}

/// Simulated mel spectrogram computation
/// In production, this would use actual FFT and mel filterbank
fn compute_mel_spectrogram(audio: &[f32]) -> Vec<Vec<f32>> {
    let num_frames = (audio.len() / HOP_LENGTH) + 1;
    let mut spectrogram = Vec::with_capacity(num_frames);

    for frame_idx in 0..num_frames {
        let start = frame_idx * HOP_LENGTH;
        let end = (start + N_FFT).min(audio.len());

        // Simulated FFT and mel filterbank
        let mut mel_frame = vec![0.0f32; N_MELS];
        for (i, &sample) in audio[start..end].iter().enumerate() {
            let bin = i % N_MELS;
            mel_frame[bin] += sample.abs();
        }

        // Apply log scaling
        for val in mel_frame.iter_mut() {
            *val = (*val + 1e-10).ln();
        }

        spectrogram.push(mel_frame);
    }

    spectrogram
}

/// Simulated embedding inference (mock ONNX model)
/// In production, this would use the actual Perch 2.0 model
fn compute_embedding(spectrogram: &[Vec<f32>]) -> Vec<f32> {
    let mut embedding = vec![0.0f32; PERCH_EMBEDDING_DIM];

    // Simulated neural network computation
    for (i, frame) in spectrogram.iter().enumerate() {
        for (j, &mel) in frame.iter().enumerate() {
            let embed_idx = (i * N_MELS + j) % PERCH_EMBEDDING_DIM;
            embedding[embed_idx] += mel * 0.01;
        }
    }

    // Normalize to unit length
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    }

    embedding
}

/// L2 normalize an embedding vector
fn normalize_embedding(embedding: &mut [f32]) {
    let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in embedding.iter_mut() {
            *x /= norm;
        }
    }
}

// ============================================================================
// Spectrogram Generation Benchmarks
// ============================================================================

/// Benchmark mel spectrogram generation
fn benchmark_spectrogram_generation(c: &mut Criterion) {
    let mut group = c.benchmark_group("spectrogram_generation");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    let audio = generate_audio_segment();

    group.throughput(Throughput::Elements(1));
    group.bench_function("single_segment", |b| {
        b.iter(|| black_box(compute_mel_spectrogram(&audio)));
    });

    // Batch spectrogram computation
    let batch_sizes = [10, 50, 100];
    for &batch_size in &batch_sizes {
        let audio_batch: Vec<Vec<f32>> = (0..batch_size).map(|_| generate_audio_segment()).collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    for audio in &audio_batch {
                        black_box(compute_mel_spectrogram(audio));
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Embedding Inference Benchmarks
// ============================================================================

/// Benchmark embedding inference (mock ONNX)
fn benchmark_embedding_inference(c: &mut Criterion) {
    let mut group = c.benchmark_group("embedding_inference");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    // Pre-compute spectrogram
    let audio = generate_audio_segment();
    let spectrogram = compute_mel_spectrogram(&audio);

    group.throughput(Throughput::Elements(1));
    group.bench_function("single_inference", |b| {
        b.iter(|| black_box(compute_embedding(&spectrogram)));
    });

    // Batch inference
    let batch_sizes = [10, 32, 64, 128];
    for &batch_size in &batch_sizes {
        let spectrograms: Vec<Vec<Vec<f32>>> = (0..batch_size)
            .map(|_| {
                let audio = generate_audio_segment();
                compute_mel_spectrogram(&audio)
            })
            .collect();

        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, _| {
                b.iter(|| {
                    for spec in &spectrograms {
                        black_box(compute_embedding(spec));
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark full pipeline: audio -> spectrogram -> embedding
fn benchmark_full_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_embedding_pipeline");
    group.sample_size(50);
    group.measurement_time(Duration::from_secs(15));

    group.throughput(Throughput::Elements(1));
    group.bench_function("single_segment", |b| {
        b.iter(|| {
            let audio = generate_audio_segment();
            let spectrogram = compute_mel_spectrogram(&audio);
            let embedding = compute_embedding(&spectrogram);
            black_box(embedding)
        });
    });

    // Batch pipeline
    for &batch_size in &[10, 50, 100] {
        group.throughput(Throughput::Elements(batch_size as u64));
        group.bench_with_input(
            BenchmarkId::new("batch", batch_size),
            &batch_size,
            |b, &size| {
                b.iter(|| {
                    for _ in 0..size {
                        let audio = generate_audio_segment();
                        let spectrogram = compute_mel_spectrogram(&audio);
                        let embedding = compute_embedding(&spectrogram);
                        black_box(embedding);
                    }
                });
            },
        );
    }

    group.finish();
}

// ============================================================================
// Normalization Benchmarks
// ============================================================================

/// Benchmark embedding normalization
fn benchmark_normalization(c: &mut Criterion) {
    let mut group = c.benchmark_group("normalization");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(5));

    // Generate random unnormalized embeddings
    let embeddings: Vec<Vec<f32>> = (0..1000)
        .map(|i| {
            let mut vec = vec![0.0f32; PERCH_EMBEDDING_DIM];
            let mut seed = (i as u64).wrapping_mul(6364136223846793005).wrapping_add(1);
            for v in vec.iter_mut() {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                *v = ((seed >> 33) as f32 / u32::MAX as f32) * 2.0 - 1.0;
            }
            vec
        })
        .collect();

    group.throughput(Throughput::Elements(1));
    group.bench_function("single", |b| {
        let mut embedding = embeddings[0].clone();
        b.iter(|| {
            normalize_embedding(&mut embedding);
            black_box(&embedding);
        });
    });

    group.throughput(Throughput::Elements(1000));
    group.bench_function("batch_1000", |b| {
        let mut batch = embeddings.clone();
        b.iter(|| {
            for embedding in batch.iter_mut() {
                normalize_embedding(embedding);
            }
            black_box(&batch);
        });
    });

    group.finish();
}

// ============================================================================
// Quantization Benchmarks
// ============================================================================

/// Benchmark scalar quantization (float32 -> int8)
fn benchmark_quantization(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization");
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(10));

    // Generate embeddings and calibrate quantizer
    let embeddings = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);
    let mut quantizer = ScalarQuantizer::new(PERCH_EMBEDDING_DIM);
    quantizer.calibrate(&embeddings);

    // Benchmark quantization
    group.throughput(Throughput::Elements(1));
    group.bench_function("quantize_single", |b| {
        let embedding = &embeddings[0];
        b.iter(|| black_box(quantizer.quantize(embedding)));
    });

    // Batch quantization
    group.throughput(Throughput::Elements(1000));
    group.bench_function("quantize_batch_1000", |b| {
        b.iter(|| {
            for embedding in &embeddings {
                black_box(quantizer.quantize(embedding));
            }
        });
    });

    // Benchmark dequantization
    let quantized: Vec<Vec<u8>> = embeddings.iter().map(|e| quantizer.quantize(e)).collect();

    group.throughput(Throughput::Elements(1));
    group.bench_function("dequantize_single", |b| {
        let q = &quantized[0];
        b.iter(|| black_box(quantizer.dequantize(q)));
    });

    group.throughput(Throughput::Elements(1000));
    group.bench_function("dequantize_batch_1000", |b| {
        b.iter(|| {
            for q in &quantized {
                black_box(quantizer.dequantize(q));
            }
        });
    });

    group.finish();
}

/// Benchmark quantization error measurement
fn benchmark_quantization_error(c: &mut Criterion) {
    let mut group = c.benchmark_group("quantization_error");
    group.sample_size(50);

    let embeddings = generate_random_vectors(100, PERCH_EMBEDDING_DIM);
    let mut quantizer = ScalarQuantizer::new(PERCH_EMBEDDING_DIM);
    quantizer.calibrate(&embeddings);

    group.bench_function("measure_error", |b| {
        b.iter(|| {
            let mut total_error = 0.0f32;
            let mut max_error = 0.0f32;

            for embedding in &embeddings {
                let quantized = quantizer.quantize(embedding);
                let dequantized = quantizer.dequantize(&quantized);

                let error: f32 = embedding
                    .iter()
                    .zip(dequantized.iter())
                    .map(|(a, b)| (a - b).abs())
                    .sum();

                total_error += error;
                max_error = max_error.max(error);
            }

            black_box((total_error / embeddings.len() as f32, max_error))
        });
    });

    group.finish();
}

// ============================================================================
// Throughput Analysis
// ============================================================================

/// Analyze embedding throughput against targets
fn analyze_embedding_throughput() {
    use std::time::Instant;

    println!("\n=== Embedding Throughput Analysis ===\n");

    // Target: 100 segments/second
    let target_segments_per_sec = targets::EMBEDDING_SEGMENTS_PER_SECOND;
    let num_segments = 100;

    let start = Instant::now();

    for _ in 0..num_segments {
        let audio = generate_audio_segment();
        let spectrogram = compute_mel_spectrogram(&audio);
        let _embedding = compute_embedding(&spectrogram);
    }

    let elapsed = start.elapsed();
    let throughput = num_segments as f64 / elapsed.as_secs_f64();

    println!("Processed {} segments in {:?}", num_segments, elapsed);
    println!("Throughput: {:.1} segments/sec", throughput);
    println!(
        "Target: {} segments/sec ({})",
        target_segments_per_sec,
        if throughput >= target_segments_per_sec as f64 {
            "PASS"
        } else {
            "FAIL"
        }
    );
}

// ============================================================================
// Half-Precision (float16) Simulation
// ============================================================================

/// Simulate float16 quantization for warm tier storage
fn simulate_float16(embedding: &[f32]) -> Vec<u16> {
    embedding
        .iter()
        .map(|&v| half::f16::from_f32(v).to_bits())
        .collect()
}

/// Benchmark float16 conversion
fn benchmark_float16_conversion(c: &mut Criterion) {
    let mut group = c.benchmark_group("float16_conversion");
    group.sample_size(100);

    let embeddings = generate_random_vectors(1000, PERCH_EMBEDDING_DIM);

    group.throughput(Throughput::Elements(1000));
    group.bench_function("to_float16", |b| {
        b.iter(|| {
            for embedding in &embeddings {
                black_box(simulate_float16(embedding));
            }
        });
    });

    // Benchmark float16 -> float32 conversion
    let float16_embeddings: Vec<Vec<u16>> = embeddings.iter().map(|e| simulate_float16(e)).collect();

    group.bench_function("from_float16", |b| {
        b.iter(|| {
            for embedding in &float16_embeddings {
                let restored: Vec<f32> = embedding
                    .iter()
                    .map(|&bits| half::f16::from_bits(bits).to_f32())
                    .collect();
                black_box(restored);
            }
        });
    });

    group.finish();
}

// ============================================================================
// Criterion Groups
// ============================================================================

criterion_group!(
    name = spectrogram_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_spectrogram_generation
);

criterion_group!(
    name = inference_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_embedding_inference, benchmark_full_pipeline
);

criterion_group!(
    name = normalization_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_normalization
);

criterion_group!(
    name = quantization_benches;
    config = Criterion::default().with_output_color(true);
    targets = benchmark_quantization, benchmark_quantization_error, benchmark_float16_conversion
);

criterion_main!(
    spectrogram_benches,
    inference_benches,
    normalization_benches,
    quantization_benches
);

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_generation() {
        let audio = generate_audio_segment();
        assert_eq!(audio.len(), SEGMENT_SAMPLES);

        // Check samples are in reasonable range
        for &sample in &audio {
            assert!(sample.abs() < 2.0);
        }
    }

    #[test]
    fn test_spectrogram_generation() {
        let audio = generate_audio_segment();
        let spectrogram = compute_mel_spectrogram(&audio);

        assert!(!spectrogram.is_empty());
        assert_eq!(spectrogram[0].len(), N_MELS);
    }

    #[test]
    fn test_embedding_computation() {
        let audio = generate_audio_segment();
        let spectrogram = compute_mel_spectrogram(&audio);
        let embedding = compute_embedding(&spectrogram);

        assert_eq!(embedding.len(), PERCH_EMBEDDING_DIM);

        // Check normalization
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_quantization_roundtrip() {
        let embeddings = generate_random_vectors(100, PERCH_EMBEDDING_DIM);
        let mut quantizer = ScalarQuantizer::new(PERCH_EMBEDDING_DIM);
        quantizer.calibrate(&embeddings);

        for embedding in &embeddings {
            let quantized = quantizer.quantize(embedding);
            let dequantized = quantizer.dequantize(&quantized);

            // Check dimensions preserved
            assert_eq!(dequantized.len(), embedding.len());

            // Check error is bounded
            let max_error: f32 = embedding
                .iter()
                .zip(dequantized.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f32::max);

            // Max error per dimension should be small
            assert!(max_error < 0.1, "Max error {} too large", max_error);
        }
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored --nocapture
    fn run_throughput_analysis() {
        analyze_embedding_throughput();
    }
}
