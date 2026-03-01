# Benchmarking Strategy for Ruvector-Scipix OCR System

## Overview

This document outlines a comprehensive benchmarking strategy for the ruvector-scipix OCR system, covering performance metrics, accuracy metrics, datasets, baselines, and implementation details.

## 1. Performance Metrics

### 1.1 Latency Metrics

Measure end-to-end processing time from image input to LaTeX output:

- **P50 (Median)**: 50th percentile latency - typical processing time
- **P95**: 95th percentile latency - most requests complete within this time
- **P99**: 99th percentile latency - captures tail latency for SLA requirements
- **P99.9**: 99.9th percentile latency - extreme outliers

**Target Benchmarks:**
```
Single Image Processing:
- P50: < 200ms (small images, <1MB)
- P95: < 500ms
- P99: < 1000ms
- P99.9: < 2000ms

Batch Processing (10 images):
- P50: < 1500ms
- P95: < 3000ms
- P99: < 5000ms
```

**Component-Level Latency:**
- Image preprocessing: < 50ms
- Model inference: < 150ms (GPU), < 800ms (CPU)
- Post-processing/formatting: < 20ms
- NAPI overhead: < 10ms

### 1.2 Throughput Metrics

Measure processing capacity under sustained load:

- **Images per second (img/s)**: Single-threaded performance
- **Pages per minute (ppm)**: Batch processing performance
- **Concurrent requests**: Multi-threaded throughput
- **GPU utilization**: Percentage of GPU compute used

**Target Benchmarks:**
```
Single-threaded:
- GPU: 10-15 img/s
- CPU: 2-3 img/s

Multi-threaded (8 cores):
- GPU: 40-50 img/s
- CPU: 8-12 img/s

Batch Processing:
- GPU: 60-80 ppm
- CPU: 15-20 ppm
```

### 1.3 Memory Usage

Track memory consumption patterns:

- **Peak memory**: Maximum memory usage during processing
- **Average memory**: Typical memory footprint
- **Memory per image**: Incremental memory for each image
- **Memory leaks**: Long-running stability tests

**Target Benchmarks:**
```
Model Loading:
- Peak: < 2GB (GPU), < 1GB (CPU)

Per-Image Processing:
- Peak: < 500MB
- Average: < 200MB

Batch Processing (100 images):
- Peak: < 3GB
- Average: < 1.5GB
```

### 1.4 Model Loading Time

Measure initialization overhead:

- **Cold start**: First-time model loading
- **Warm start**: Cached model loading
- **Memory mapping**: mmap performance for large models

**Target Benchmarks:**
```
Cold Start:
- GPU: < 5s
- CPU: < 3s

Warm Start:
- GPU: < 1s
- CPU: < 500ms
```

## 2. Accuracy Metrics

### 2.1 Character Error Rate (CER)

Measures character-level accuracy:

```
CER = (Substitutions + Deletions + Insertions) / Total_Characters
```

**Target:** CER < 2% on standard datasets

**Implementation:**
```rust
fn calculate_cer(reference: &str, hypothesis: &str) -> f64 {
    let ref_chars: Vec<char> = reference.chars().collect();
    let hyp_chars: Vec<char> = hypothesis.chars().collect();

    let distance = levenshtein_distance(&ref_chars, &hyp_chars);
    distance as f64 / ref_chars.len() as f64
}
```

### 2.2 Word Error Rate (WER)

Measures word-level accuracy:

```
WER = (Substitutions + Deletions + Insertions) / Total_Words
```

**Target:** WER < 5% on standard datasets

**Implementation:**
```rust
fn calculate_wer(reference: &str, hypothesis: &str) -> f64 {
    let ref_words: Vec<&str> = reference.split_whitespace().collect();
    let hyp_words: Vec<&str> = hypothesis.split_whitespace().collect();

    let distance = levenshtein_distance(&ref_words, &hyp_words);
    distance as f64 / ref_words.len() as f64
}
```

### 2.3 BLEU Score for LaTeX Output

Measures LaTeX generation quality (0-100 scale):

```
BLEU = BP × exp(Σ wn × log(pn))
```

**Target:** BLEU > 85 on math expression datasets

**Implementation:**
```rust
fn calculate_bleu(reference: &str, hypothesis: &str, n: usize) -> f64 {
    let ref_ngrams = extract_ngrams(reference, n);
    let hyp_ngrams = extract_ngrams(hypothesis, n);

    let matches = count_matches(&ref_ngrams, &hyp_ngrams);
    let precision = matches as f64 / hyp_ngrams.len() as f64;

    let bp = brevity_penalty(reference.len(), hypothesis.len());
    bp * precision
}
```

### 2.4 Expression Recognition Rate (ERR)

Measures mathematical expression correctness:

```
ERR = Correct_Expressions / Total_Expressions
```

**Target:** ERR > 90% on complex mathematical expressions

**Categories:**
- Simple expressions: 2+2, x^2
- Fractions: \frac{a}{b}
- Matrices: \begin{bmatrix}...\end{bmatrix}
- Complex equations: integrals, summations, limits

## 3. Benchmark Datasets

### 3.1 Im2latex-100k

**Source:** https://zenodo.org/record/56198

**Description:**
- 100,000 LaTeX formula images
- Rendered from arXiv papers
- Variety of mathematical expressions

**Usage:**
```bash
# Download dataset
wget https://zenodo.org/record/56198/files/im2latex-100k.tar.gz
tar -xzf im2latex-100k.tar.gz

# Structure:
# im2latex-100k/
#   ├── images/
#   └── formulas.txt
```

**Benchmark Focus:**
- General mathematical notation
- Diversity of expressions
- Standard baseline comparison

### 3.2 Im2latex-230k

**Source:** Extended Im2latex dataset

**Description:**
- 230,000 LaTeX formula images
- More complex expressions
- Better coverage of mathematical domains

**Usage:**
```bash
# Download extended dataset
wget https://zenodo.org/record/1234567/files/im2latex-230k.tar.gz
tar -xzf im2latex-230k.tar.gz
```

**Benchmark Focus:**
- Complex mathematical expressions
- Edge cases and rare symbols
- Stress testing

### 3.3 CROHME (Handwritten Math)

**Source:** https://www.isical.ac.in/~crohme/

**Description:**
- Competition on Recognition of Online Handwritten Mathematical Expressions
- Handwritten formulas (not typed/rendered)
- Real-world use case

**Usage:**
```bash
# Download CROHME dataset
wget http://www.isical.ac.in/~crohme/CROHME2019.zip
unzip CROHME2019.zip
```

**Benchmark Focus:**
- Handwritten formula recognition
- Real-world variability
- Robustness testing

### 3.4 Custom Ruvector Test Set

**Description:**
- Curated test set specific to ruvector use cases
- Real user submissions
- Edge cases discovered in production

**Structure:**
```
ruvector-testset/
├── easy/          # Simple expressions (100 samples)
├── medium/        # Moderate complexity (200 samples)
├── hard/          # Complex expressions (150 samples)
├── edge_cases/    # Known difficult cases (50 samples)
└── ground_truth.json
```

**Creation Script:**
```rust
// examples/scipix/benches/create_testset.rs
use std::fs;
use serde_json::json;

fn create_testset() {
    let testset = json!({
        "easy": [
            {"image": "easy/001.png", "latex": "x^2 + 2x + 1"},
            {"image": "easy/002.png", "latex": "\\frac{1}{2}"},
        ],
        "medium": [
            {"image": "medium/001.png", "latex": "\\int_{0}^{\\infty} e^{-x} dx"},
        ],
        "hard": [
            {"image": "hard/001.png", "latex": "\\sum_{n=1}^{\\infty} \\frac{1}{n^2} = \\frac{\\pi^2}{6}"},
        ]
    });

    fs::write("ground_truth.json", testset.to_string()).unwrap();
}
```

## 4. Comparison Baselines

### 4.1 Scipix API (Commercial Baseline)

**Website:** https://scipix.com/

**Metrics to Compare:**
- Accuracy (CER, WER, BLEU)
- Latency (API roundtrip time)
- Cost per image
- Supported formats

**Benchmark Script:**
```rust
async fn benchmark_scipix(image_path: &str) -> BenchmarkResult {
    let client = ScipixClient::new(api_key);

    let start = Instant::now();
    let result = client.ocr_image(image_path).await?;
    let latency = start.elapsed();

    BenchmarkResult {
        provider: "Scipix API",
        latency,
        latex: result.latex,
        confidence: result.confidence,
    }
}
```

### 4.2 pix2tex/LaTeX-OCR

**Repository:** https://github.com/lukas-blecher/LaTeX-OCR

**Description:**
- Open-source Python implementation
- Transformer-based model
- Academic baseline

**Benchmark Script:**
```python
# benchmark_pix2tex.py
import time
from pix2tex.cli import LatexOCR

model = LatexOCR()

def benchmark_pix2tex(image_path):
    start = time.time()
    latex = model(image_path)
    latency = time.time() - start

    return {
        'provider': 'pix2tex',
        'latency': latency,
        'latex': latex
    }
```

### 4.3 ocrs (Rust Native)

**Repository:** https://github.com/robertknight/ocrs

**Description:**
- Rust-native OCR
- General text OCR (not math-specific)
- Performance baseline

**Benchmark:**
```rust
use ocrs::{OcrEngine, OcrEngineParams};

fn benchmark_ocrs(image_path: &str) -> BenchmarkResult {
    let engine = OcrEngine::new(OcrEngineParams::default())?;

    let start = Instant::now();
    let result = engine.ocr_image(image_path)?;
    let latency = start.elapsed();

    BenchmarkResult {
        provider: "ocrs",
        latency,
        text: result.text,
    }
}
```

### 4.4 Tesseract

**Website:** https://github.com/tesseract-ocr/tesseract

**Description:**
- Industry standard OCR
- Not math-specific
- CPU performance baseline

**Benchmark:**
```rust
use tesseract::Tesseract;

fn benchmark_tesseract(image_path: &str) -> BenchmarkResult {
    let start = Instant::now();
    let text = Tesseract::new(None, Some("eng"))?
        .set_image(image_path)?
        .get_text()?;
    let latency = start.elapsed();

    BenchmarkResult {
        provider: "Tesseract",
        latency,
        text,
    }
}
```

## 5. Benchmark Implementation

### 5.1 Criterion.rs Setup

**Dependencies:**
```toml
# Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["html_reports"] }
serde_json = "1.0"
image = "0.24"

[[bench]]
name = "scipix_ocr"
harness = false
```

### 5.2 Basic Benchmark Template

```rust
// benches/scipix_ocr.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::path::Path;

fn benchmark_single_image(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let image_path = "testdata/simple_equation.png";

    c.bench_function("ocr_simple_equation", |b| {
        b.iter(|| {
            ocr.process_image(black_box(image_path))
        });
    });
}

fn benchmark_image_sizes(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let mut group = c.benchmark_group("image_sizes");

    for size in ["small", "medium", "large"].iter() {
        let image_path = format!("testdata/{}_image.png", size);

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &image_path,
            |b, path| {
                b.iter(|| ocr.process_image(black_box(path)));
            },
        );
    }

    group.finish();
}

fn benchmark_batch_processing(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let images: Vec<String> = (0..10)
        .map(|i| format!("testdata/batch_{}.png", i))
        .collect();

    c.bench_function("ocr_batch_10_images", |b| {
        b.iter(|| {
            ocr.process_batch(black_box(&images))
        });
    });
}

criterion_group!(benches, benchmark_single_image, benchmark_image_sizes, benchmark_batch_processing);
criterion_main!(benches);
```

### 5.3 Advanced Benchmark with Metrics

```rust
// benches/comprehensive_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId, Throughput};
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::time::Duration;

fn benchmark_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Configure for throughput measurement
    group.sample_size(100);
    group.measurement_time(Duration::from_secs(30));

    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    for batch_size in [1, 5, 10, 20, 50].iter() {
        let images: Vec<String> = (0..*batch_size)
            .map(|i| format!("testdata/image_{}.png", i % 10))
            .collect();

        group.throughput(Throughput::Elements(*batch_size as u64));

        group.bench_with_input(
            BenchmarkId::new("batch_processing", batch_size),
            &images,
            |b, imgs| {
                b.iter(|| ocr.process_batch(imgs));
            },
        );
    }

    group.finish();
}

fn benchmark_memory_usage(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    let config = OCRConfig::default();

    group.bench_function("model_loading", |b| {
        b.iter(|| {
            let _ocr = ScipixOCR::new(config.clone()).unwrap();
            // Model automatically dropped, measuring allocation overhead
        });
    });

    group.finish();
}

fn benchmark_latency_percentiles(c: &mut Criterion) {
    let mut group = c.benchmark_group("latency_percentiles");

    // Large sample size for accurate percentile calculation
    group.sample_size(1000);

    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let test_images = vec![
        "testdata/simple.png",
        "testdata/complex.png",
        "testdata/matrix.png",
    ];

    for image_path in test_images {
        group.bench_with_input(
            BenchmarkId::from_parameter(Path::new(image_path).file_stem().unwrap().to_str().unwrap()),
            &image_path,
            |b, path| {
                b.iter(|| ocr.process_image(path));
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    benchmark_throughput,
    benchmark_memory_usage,
    benchmark_latency_percentiles
);
criterion_main!(benches);
```

### 5.4 Accuracy Benchmark

```rust
// benches/accuracy_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion};
use ruvector_scipix::{ScipixOCR, OCRConfig};
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Deserialize, Serialize)]
struct GroundTruth {
    image: String,
    latex: String,
}

fn load_ground_truth(path: &str) -> Vec<GroundTruth> {
    let content = fs::read_to_string(path).expect("Failed to read ground truth");
    serde_json::from_str(&content).expect("Failed to parse ground truth")
}

fn calculate_cer(reference: &str, hypothesis: &str) -> f64 {
    // Implement Levenshtein distance
    let ref_chars: Vec<char> = reference.chars().collect();
    let hyp_chars: Vec<char> = hypothesis.chars().collect();

    let mut dp = vec![vec![0; hyp_chars.len() + 1]; ref_chars.len() + 1];

    for i in 0..=ref_chars.len() {
        dp[i][0] = i;
    }
    for j in 0..=hyp_chars.len() {
        dp[0][j] = j;
    }

    for i in 1..=ref_chars.len() {
        for j in 1..=hyp_chars.len() {
            let cost = if ref_chars[i - 1] == hyp_chars[j - 1] { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1)
                .min(dp[i - 1][j - 1] + cost);
        }
    }

    dp[ref_chars.len()][hyp_chars.len()] as f64 / ref_chars.len() as f64
}

fn benchmark_accuracy(c: &mut Criterion) {
    let config = OCRConfig::default();
    let ocr = ScipixOCR::new(config).expect("Failed to initialize OCR");

    let ground_truth = load_ground_truth("testdata/ground_truth.json");

    c.bench_function("accuracy_evaluation", |b| {
        b.iter(|| {
            let mut total_cer = 0.0;
            let mut count = 0;

            for gt in &ground_truth {
                if let Ok(result) = ocr.process_image(&gt.image) {
                    let cer = calculate_cer(&gt.latex, &result.latex);
                    total_cer += cer;
                    count += 1;
                }
            }

            let avg_cer = if count > 0 { total_cer / count as f64 } else { 1.0 };
            println!("Average CER: {:.4}", avg_cer);
        });
    });
}

criterion_group!(benches, benchmark_accuracy);
criterion_main!(benches);
```

### 5.5 Automated Benchmark Runner

```rust
// examples/scipix/src/benchmark_runner.rs
use std::process::Command;
use std::fs::{self, File};
use std::io::Write;
use serde_json::json;

pub struct BenchmarkRunner {
    output_dir: String,
}

impl BenchmarkRunner {
    pub fn new(output_dir: &str) -> Self {
        fs::create_dir_all(output_dir).expect("Failed to create output directory");
        Self {
            output_dir: output_dir.to_string(),
        }
    }

    pub fn run_all_benchmarks(&self) -> Result<(), Box<dyn std::error::Error>> {
        println!("Running comprehensive benchmarks...");

        // Run Criterion benchmarks
        let criterion_output = Command::new("cargo")
            .args(&["bench", "--bench", "scipix_ocr"])
            .output()?;

        self.save_output("criterion_output.txt", &criterion_output.stdout)?;

        // Run accuracy benchmarks
        let accuracy_output = Command::new("cargo")
            .args(&["bench", "--bench", "accuracy_benchmark"])
            .output()?;

        self.save_output("accuracy_output.txt", &accuracy_output.stdout)?;

        // Run memory profiling
        self.run_memory_profiling()?;

        // Generate summary report
        self.generate_summary_report()?;

        Ok(())
    }

    fn run_memory_profiling(&self) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
            let output = Command::new("valgrind")
                .args(&[
                    "--tool=massif",
                    "--massif-out-file=massif.out",
                    "cargo", "bench", "--bench", "scipix_ocr"
                ])
                .output()?;

            self.save_output("memory_profile.txt", &output.stdout)?;
        }

        Ok(())
    }

    fn save_output(&self, filename: &str, content: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
        let path = format!("{}/{}", self.output_dir, filename);
        let mut file = File::create(path)?;
        file.write_all(content)?;
        Ok(())
    }

    fn generate_summary_report(&self) -> Result<(), Box<dyn std::error::Error>> {
        let report = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "benchmarks": {
                "performance": "See criterion_output.txt",
                "accuracy": "See accuracy_output.txt",
                "memory": "See memory_profile.txt"
            },
            "results_dir": self.output_dir
        });

        let path = format!("{}/summary.json", self.output_dir);
        let mut file = File::create(path)?;
        file.write_all(serde_json::to_string_pretty(&report)?.as_bytes())?;

        println!("Benchmark summary saved to {}/summary.json", self.output_dir);

        Ok(())
    }
}

// Main entry point
fn main() {
    let runner = BenchmarkRunner::new("benchmark_results");

    match runner.run_all_benchmarks() {
        Ok(_) => println!("All benchmarks completed successfully"),
        Err(e) => eprintln!("Benchmark failed: {}", e),
    }
}
```

### 5.6 CI/CD Integration

```yaml
# .github/workflows/benchmarks.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]
  schedule:
    - cron: '0 2 * * *'  # Run daily at 2 AM

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          override: true

      - name: Cache cargo registry
        uses: actions/cache@v3
        with:
          path: ~/.cargo/registry
          key: ${{ runner.os }}-cargo-registry-${{ hashFiles('**/Cargo.lock') }}

      - name: Download test datasets
        run: |
          mkdir -p testdata
          # Download sample images for benchmarking
          wget -O testdata/simple.png https://example.com/test-images/simple.png

      - name: Run benchmarks
        run: cargo bench --bench scipix_ocr

      - name: Run accuracy benchmarks
        run: cargo bench --bench accuracy_benchmark

      - name: Upload benchmark results
        uses: actions/upload-artifact@v3
        with:
          name: benchmark-results
          path: target/criterion/

      - name: Compare with baseline
        run: |
          cargo install critcmp
          critcmp --group ".*" baseline current

      - name: Check for regressions
        run: |
          python scripts/check_regression.py \
            --baseline benchmark_baseline.json \
            --current target/criterion/results.json \
            --threshold 0.10  # Alert if >10% regression
```

## 6. Profiling Tools

### 6.1 perf and Flamegraph

**Installation:**
```bash
# Install perf (Linux)
sudo apt-get install linux-tools-common linux-tools-generic

# Install flamegraph
cargo install flamegraph
```

**CPU Profiling:**
```bash
# Profile a benchmark with perf
perf record -F 99 -g cargo bench --bench scipix_ocr

# Generate flamegraph
perf script | stackcollapse-perf.pl | flamegraph.pl > flamegraph.svg

# Or use cargo-flamegraph directly
cargo flamegraph --bench scipix_ocr
```

**Analysis Script:**
```rust
// scripts/analyze_perf.rs
use std::process::Command;

fn main() {
    // Run perf stat for detailed metrics
    let output = Command::new("perf")
        .args(&[
            "stat",
            "-e", "cycles,instructions,cache-misses,branch-misses",
            "cargo", "bench", "--bench", "scipix_ocr"
        ])
        .output()
        .expect("Failed to run perf stat");

    println!("Perf statistics:");
    println!("{}", String::from_utf8_lossy(&output.stderr));
}
```

### 6.2 Memory Profiling

**Valgrind/Massif:**
```bash
# Profile memory usage
valgrind --tool=massif \
         --massif-out-file=massif.out \
         cargo bench --bench scipix_ocr

# Visualize with massif-visualizer
massif-visualizer massif.out

# Or use ms_print
ms_print massif.out > memory_report.txt
```

**Heaptrack (Linux):**
```bash
# Install heaptrack
sudo apt-get install heaptrack

# Profile memory allocations
heaptrack cargo bench --bench scipix_ocr

# Analyze results
heaptrack_gui heaptrack.cargo.*.gz
```

**Custom Memory Tracker:**
```rust
// src/memory_tracker.rs
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct TrackingAllocator;

static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for TrackingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        ALLOCATED.fetch_add(size, Ordering::SeqCst);
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let size = layout.size();
        DEALLOCATED.fetch_add(size, Ordering::SeqCst);
        System.dealloc(ptr, layout);
    }
}

#[global_allocator]
static GLOBAL: TrackingAllocator = TrackingAllocator;

pub fn get_memory_stats() -> (usize, usize, usize) {
    let allocated = ALLOCATED.load(Ordering::SeqCst);
    let deallocated = DEALLOCATED.load(Ordering::SeqCst);
    let current = allocated - deallocated;
    (allocated, deallocated, current)
}

// Usage in benchmark:
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_memory() {
        let (before_alloc, _, _) = get_memory_stats();

        // Run OCR operation
        let ocr = ScipixOCR::new(OCRConfig::default()).unwrap();
        ocr.process_image("test.png").unwrap();

        let (after_alloc, _, current) = get_memory_stats();

        println!("Memory allocated: {} bytes", after_alloc - before_alloc);
        println!("Current memory usage: {} bytes", current);
    }
}
```

### 6.3 GPU Utilization

**NVIDIA GPU Profiling:**
```bash
# Install NVIDIA profiling tools
# Nsight Systems for timeline profiling
nsys profile --trace=cuda,nvtx cargo bench --bench scipix_ocr

# Nsight Compute for kernel analysis
ncu --set full cargo bench --bench scipix_ocr
```

**GPU Monitoring Script:**
```rust
// src/gpu_monitor.rs
use std::process::Command;
use std::time::{Duration, Instant};
use std::thread;

pub struct GPUMonitor {
    monitoring: bool,
    samples: Vec<GPUSample>,
}

#[derive(Debug, Clone)]
pub struct GPUSample {
    timestamp: Instant,
    utilization: u32,
    memory_used: u64,
    memory_total: u64,
    temperature: u32,
}

impl GPUMonitor {
    pub fn new() -> Self {
        Self {
            monitoring: false,
            samples: Vec::new(),
        }
    }

    pub fn start(&mut self) {
        self.monitoring = true;
        self.samples.clear();

        while self.monitoring {
            if let Ok(sample) = self.collect_sample() {
                self.samples.push(sample);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn stop(&mut self) {
        self.monitoring = false;
    }

    fn collect_sample(&self) -> Result<GPUSample, Box<dyn std::error::Error>> {
        let output = Command::new("nvidia-smi")
            .args(&[
                "--query-gpu=utilization.gpu,memory.used,memory.total,temperature.gpu",
                "--format=csv,noheader,nounits"
            ])
            .output()?;

        let data = String::from_utf8(output.stdout)?;
        let parts: Vec<&str> = data.trim().split(',').collect();

        Ok(GPUSample {
            timestamp: Instant::now(),
            utilization: parts[0].trim().parse()?,
            memory_used: parts[1].trim().parse()?,
            memory_total: parts[2].trim().parse()?,
            temperature: parts[3].trim().parse()?,
        })
    }

    pub fn get_statistics(&self) -> GPUStatistics {
        if self.samples.is_empty() {
            return GPUStatistics::default();
        }

        let avg_utilization = self.samples.iter()
            .map(|s| s.utilization)
            .sum::<u32>() as f64 / self.samples.len() as f64;

        let max_utilization = self.samples.iter()
            .map(|s| s.utilization)
            .max()
            .unwrap_or(0);

        let avg_memory = self.samples.iter()
            .map(|s| s.memory_used)
            .sum::<u64>() as f64 / self.samples.len() as f64;

        GPUStatistics {
            avg_utilization,
            max_utilization,
            avg_memory_mb: avg_memory / 1024.0,
            sample_count: self.samples.len(),
        }
    }
}

#[derive(Debug, Default)]
pub struct GPUStatistics {
    pub avg_utilization: f64,
    pub max_utilization: u32,
    pub avg_memory_mb: f64,
    pub sample_count: usize,
}
```

### 6.4 Integrated Profiling Benchmark

```rust
// benches/profiling_benchmark.rs
use criterion::{criterion_group, criterion_main, Criterion};
use ruvector_scipix::{ScipixOCR, OCRConfig};
use std::sync::{Arc, Mutex};
use std::thread;

fn benchmark_with_profiling(c: &mut Criterion) {
    let mut group = c.benchmark_group("profiled");

    group.bench_function("ocr_with_memory_tracking", |b| {
        b.iter_custom(|iters| {
            let config = OCRConfig::default();
            let ocr = ScipixOCR::new(config).unwrap();

            let (start_alloc, _, _) = get_memory_stats();
            let start_time = std::time::Instant::now();

            for _ in 0..iters {
                ocr.process_image("testdata/sample.png").unwrap();
            }

            let duration = start_time.elapsed();
            let (end_alloc, _, current) = get_memory_stats();

            println!("Memory delta: {} bytes", end_alloc - start_alloc);
            println!("Current usage: {} bytes", current);

            duration
        });
    });

    group.bench_function("ocr_with_gpu_monitoring", |b| {
        let monitor = Arc::new(Mutex::new(GPUMonitor::new()));
        let monitor_clone = monitor.clone();

        // Start GPU monitoring in background thread
        let handle = thread::spawn(move || {
            monitor_clone.lock().unwrap().start();
        });

        b.iter(|| {
            let config = OCRConfig::default();
            let ocr = ScipixOCR::new(config).unwrap();
            ocr.process_image("testdata/sample.png").unwrap();
        });

        // Stop monitoring
        monitor.lock().unwrap().stop();
        handle.join().unwrap();

        let stats = monitor.lock().unwrap().get_statistics();
        println!("GPU Statistics: {:?}", stats);
    });

    group.finish();
}

criterion_group!(benches, benchmark_with_profiling);
criterion_main!(benches);
```

## 7. Regression Testing

### 7.1 Performance Baseline Tracking

**Baseline Storage Structure:**
```json
{
  "commit": "a1b2c3d4",
  "timestamp": "2024-01-15T10:30:00Z",
  "benchmarks": {
    "ocr_simple_equation": {
      "mean": 185.4,
      "std_dev": 12.3,
      "p50": 182.1,
      "p95": 210.5,
      "p99": 225.8
    },
    "ocr_batch_10_images": {
      "mean": 1420.6,
      "std_dev": 85.2,
      "throughput": 7.04
    }
  },
  "accuracy": {
    "cer": 0.0185,
    "wer": 0.0432,
    "bleu": 87.3
  }
}
```

**Baseline Manager:**
```rust
// src/baseline_manager.rs
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;

#[derive(Serialize, Deserialize, Clone)]
pub struct BenchmarkBaseline {
    pub commit: String,
    pub timestamp: String,
    pub benchmarks: HashMap<String, BenchmarkMetrics>,
    pub accuracy: AccuracyMetrics,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct BenchmarkMetrics {
    pub mean: f64,
    pub std_dev: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct AccuracyMetrics {
    pub cer: f64,
    pub wer: f64,
    pub bleu: f64,
}

pub struct BaselineManager {
    baseline_path: String,
}

impl BaselineManager {
    pub fn new(baseline_path: &str) -> Self {
        Self {
            baseline_path: baseline_path.to_string(),
        }
    }

    pub fn load_baseline(&self) -> Result<BenchmarkBaseline, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(&self.baseline_path)?;
        Ok(serde_json::from_str(&content)?)
    }

    pub fn save_baseline(&self, baseline: &BenchmarkBaseline) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(baseline)?;
        fs::write(&self.baseline_path, json)?;
        Ok(())
    }

    pub fn compare_with_baseline(
        &self,
        current: &BenchmarkBaseline,
        threshold: f64
    ) -> Vec<RegressionAlert> {
        let baseline = match self.load_baseline() {
            Ok(b) => b,
            Err(_) => return vec![],
        };

        let mut alerts = Vec::new();

        for (name, current_metrics) in &current.benchmarks {
            if let Some(baseline_metrics) = baseline.benchmarks.get(name) {
                let regression = (current_metrics.mean - baseline_metrics.mean) / baseline_metrics.mean;

                if regression > threshold {
                    alerts.push(RegressionAlert {
                        benchmark: name.clone(),
                        metric: "mean".to_string(),
                        baseline_value: baseline_metrics.mean,
                        current_value: current_metrics.mean,
                        regression_percent: regression * 100.0,
                        severity: if regression > threshold * 2.0 { Severity::High } else { Severity::Medium },
                    });
                }
            }
        }

        // Check accuracy regressions
        if current.accuracy.cer > baseline.accuracy.cer * (1.0 + threshold) {
            alerts.push(RegressionAlert {
                benchmark: "accuracy".to_string(),
                metric: "cer".to_string(),
                baseline_value: baseline.accuracy.cer,
                current_value: current.accuracy.cer,
                regression_percent: ((current.accuracy.cer - baseline.accuracy.cer) / baseline.accuracy.cer) * 100.0,
                severity: Severity::High,
            });
        }

        alerts
    }
}

#[derive(Debug)]
pub struct RegressionAlert {
    pub benchmark: String,
    pub metric: String,
    pub baseline_value: f64,
    pub current_value: f64,
    pub regression_percent: f64,
    pub severity: Severity,
}

#[derive(Debug)]
pub enum Severity {
    Low,
    Medium,
    High,
}
```

### 7.2 Automated Regression Detection

```rust
// scripts/detect_regression.rs
use ruvector_scipix::baseline_manager::{BaselineManager, BenchmarkBaseline};
use std::env;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: detect_regression <baseline.json> <current.json>");
        process::exit(1);
    }

    let baseline_path = &args[1];
    let current_path = &args[2];
    let threshold = 0.10; // 10% regression threshold

    let manager = BaselineManager::new(baseline_path);

    // Load current results
    let current: BenchmarkBaseline = {
        let content = std::fs::read_to_string(current_path)
            .expect("Failed to read current results");
        serde_json::from_str(&content)
            .expect("Failed to parse current results")
    };

    // Compare with baseline
    let alerts = manager.compare_with_baseline(&current, threshold);

    if alerts.is_empty() {
        println!("✅ No performance regressions detected");
        process::exit(0);
    } else {
        println!("⚠️  Performance regressions detected:");

        let mut has_high_severity = false;

        for alert in &alerts {
            let severity_icon = match alert.severity {
                Severity::Low => "🟡",
                Severity::Medium => "🟠",
                Severity::High => "🔴",
            };

            if matches!(alert.severity, Severity::High) {
                has_high_severity = true;
            }

            println!(
                "{} {} / {}: {:.2}ms → {:.2}ms ({:+.1}%)",
                severity_icon,
                alert.benchmark,
                alert.metric,
                alert.baseline_value,
                alert.current_value,
                alert.regression_percent
            );
        }

        if has_high_severity {
            process::exit(1);
        } else {
            process::exit(0);
        }
    }
}
```

### 7.3 GitHub Actions Integration

```yaml
# .github/workflows/regression_check.yml
name: Performance Regression Check

on:
  pull_request:
    branches: [main]

jobs:
  regression-check:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v3
        with:
          fetch-depth: 0

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Download baseline
        run: |
          # Download baseline from releases or artifacts
          gh release download baseline --pattern 'benchmark_baseline.json'
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}

      - name: Run benchmarks
        run: |
          cargo bench --bench scipix_ocr -- --save-baseline current_baseline.json

      - name: Detect regressions
        run: |
          cargo run --bin detect_regression -- benchmark_baseline.json current_baseline.json

      - name: Comment on PR
        if: failure()
        uses: actions/github-script@v6
        with:
          script: |
            github.rest.issues.createComment({
              issue_number: context.issue.number,
              owner: context.repo.owner,
              repo: context.repo.repo,
              body: '⚠️ Performance regression detected! Please review benchmark results.'
            })
```

### 7.4 Continuous Baseline Updates

```rust
// scripts/update_baseline.rs
use ruvector_scipix::baseline_manager::{BaselineManager, BenchmarkBaseline};
use std::process::Command;

fn main() {
    // Get current git commit
    let commit = Command::new("git")
        .args(&["rev-parse", "HEAD"])
        .output()
        .expect("Failed to get git commit")
        .stdout;
    let commit = String::from_utf8(commit).unwrap().trim().to_string();

    // Run benchmarks
    let benchmark_output = Command::new("cargo")
        .args(&["bench", "--bench", "scipix_ocr", "--", "--save-baseline", "temp.json"])
        .output()
        .expect("Failed to run benchmarks");

    if !benchmark_output.status.success() {
        eprintln!("Benchmark failed");
        std::process::exit(1);
    }

    // Load benchmark results
    let baseline: BenchmarkBaseline = {
        let content = std::fs::read_to_string("temp.json")
            .expect("Failed to read benchmark results");
        let mut baseline: BenchmarkBaseline = serde_json::from_str(&content)
            .expect("Failed to parse benchmark results");

        baseline.commit = commit;
        baseline.timestamp = chrono::Utc::now().to_rfc3339();
        baseline
    };

    // Save as new baseline
    let manager = BaselineManager::new("benchmark_baseline.json");
    manager.save_baseline(&baseline)
        .expect("Failed to save baseline");

    println!("✅ Baseline updated successfully");
    println!("Commit: {}", baseline.commit);
    println!("Timestamp: {}", baseline.timestamp);
}
```

## Summary

This benchmarking strategy provides:

1. **Comprehensive Performance Metrics**: Latency, throughput, memory, and model loading benchmarks
2. **Accuracy Validation**: CER, WER, BLEU, and ERR metrics with industry-standard datasets
3. **Competitive Analysis**: Baseline comparisons with Scipix, pix2tex, ocrs, and Tesseract
4. **Production-Ready Implementation**: Criterion.rs benchmarks with CI/CD integration
5. **Advanced Profiling**: CPU, memory, and GPU profiling tools
6. **Regression Protection**: Automated detection and alerting for performance degradation

**Next Steps:**

1. Set up test datasets (Im2latex, CROHME)
2. Implement core benchmark suite
3. Establish performance baselines
4. Integrate into CI/CD pipeline
5. Configure alerting for regressions
6. Regular benchmark reviews and optimization

**Benchmark Execution:**
```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench scipix_ocr

# Run with profiling
cargo flamegraph --bench scipix_ocr

# Check for regressions
cargo run --bin detect_regression -- baseline.json current.json

# Update baseline
cargo run --bin update_baseline
```
