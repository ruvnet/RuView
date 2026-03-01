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
//! Real model validation tests
//!
//! These tests require actual GGUF model files to run.
//! They are marked with `#[ignore]` by default and can be run with:
//!
//! ```bash
//! # Run with specific model path
//! TEST_MODEL_PATH=./test_models/tinyllama.gguf cargo test -p ruvllm --test real_model_test -- --ignored
//!
//! # Run with default test_models directory
//! cargo test -p ruvllm --test real_model_test -- --ignored
//! ```
//!
//! ## Recommended test models (small, fast)
//!
//! | Model | Size | Use Case |
//! |-------|------|----------|
//! | TinyLlama-1.1B-Chat-v1.0.Q4_K_M.gguf | ~700MB | Fast iteration |
//! | Qwen2-0.5B-Instruct.Q4_K_M.gguf | ~400MB | Smallest, fastest |
//! | Phi-3-mini-4k-instruct.Q4_K_M.gguf | ~2GB | Higher quality |
//!
//! ## Download test models
//!
//! ```bash
//! cargo run -p ruvllm --example download_test_model -- --model tinyllama
//! ```

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

// ============================================================================
// Test Utilities
// ============================================================================

/// Common search locations for test models
const MODEL_SEARCH_PATHS: &[&str] = &[
    "./test_models",
    "../test_models",
    "../../test_models",
    "./models",
    "../models",
    "~/.cache/ruvllm/models",
    "~/.cache/huggingface/hub",
];

/// Supported model file patterns for each architecture
const TINYLLAMA_PATTERNS: &[&str] = &["tinyllama*.gguf", "TinyLlama*.gguf", "*tinyllama*.gguf"];

const PHI3_PATTERNS: &[&str] = &["phi-3*.gguf", "Phi-3*.gguf", "*phi3*.gguf", "*phi-3*.gguf"];

const QWEN_PATTERNS: &[&str] = &["qwen*.gguf", "Qwen*.gguf", "*qwen*.gguf"];

/// Result type for test helpers (reserved for future use)
#[allow(dead_code)]
type TestResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Find a test model in common locations.
///
/// Search order:
/// 1. `TEST_MODEL_PATH` environment variable (exact path)
/// 2. `TEST_MODEL_DIR` environment variable (directory to search)
/// 3. Common locations in `MODEL_SEARCH_PATHS`
///
/// # Arguments
///
/// * `patterns` - Glob patterns to match model files
///
/// # Returns
///
/// Path to the first matching model file, or None if not found
pub fn find_test_model(patterns: &[&str]) -> Option<PathBuf> {
    // 1. Check TEST_MODEL_PATH for exact path
    if let Ok(path) = env::var("TEST_MODEL_PATH") {
        let path = PathBuf::from(path);
        if path.exists() && path.is_file() {
            return Some(path);
        }
    }

    // 2. Check TEST_MODEL_DIR for directory
    if let Ok(dir) = env::var("TEST_MODEL_DIR") {
        if let Some(found) = search_directory(&PathBuf::from(dir), patterns) {
            return Some(found);
        }
    }

    // 3. Search common locations
    for search_path in MODEL_SEARCH_PATHS {
        let expanded = expand_path(search_path);
        if expanded.exists() && expanded.is_dir() {
            if let Some(found) = search_directory(&expanded, patterns) {
                return Some(found);
            }
        }
    }

    None
}

/// Search a directory for files matching any of the given patterns
fn search_directory(dir: &Path, patterns: &[&str]) -> Option<PathBuf> {
    if !dir.exists() || !dir.is_dir() {
        return None;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return None,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let file_name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n.to_lowercase(),
            None => continue,
        };

        for pattern in patterns {
            if matches_glob_pattern(&file_name, &pattern.to_lowercase()) {
                return Some(path);
            }
        }
    }

    None
}

/// Simple glob pattern matching (supports * wildcard)
fn matches_glob_pattern(name: &str, pattern: &str) -> bool {
    if !pattern.contains('*') {
        return name == pattern;
    }

    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut remaining = name;

    // First part must be a prefix (if not empty)
    if !parts[0].is_empty() {
        if !remaining.starts_with(parts[0]) {
            return false;
        }
        remaining = &remaining[parts[0].len()..];
    }

    // Last part must be a suffix (if not empty)
    if parts.len() > 1 {
        let last = parts[parts.len() - 1];
        if !last.is_empty() && !remaining.ends_with(last) {
            return false;
        }
    }

    // Middle parts must appear in order
    for part in &parts[1..parts.len().saturating_sub(1)] {
        if part.is_empty() {
            continue;
        }
        match remaining.find(part) {
            Some(pos) => remaining = &remaining[pos + part.len()..],
            None => return false,
        }
    }

    true
}

/// Expand ~ to home directory
fn expand_path(path: &str) -> PathBuf {
    if path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&path[2..]);
        }
    }
    PathBuf::from(path)
}

/// Skip test gracefully if no model is available
///
/// Returns the model path if found, or prints a skip message and returns None
pub fn skip_if_no_model(patterns: &[&str], model_name: &str) -> Option<PathBuf> {
    match find_test_model(patterns) {
        Some(path) => {
            println!("Using model: {}", path.display());
            Some(path)
        }
        None => {
            println!("SKIPPED: No {} model found.", model_name);
            println!("To run this test:");
            println!("  1. Download the model:");
            println!(
                "     cargo run -p ruvllm --example download_test_model -- --model {}",
                model_name.to_lowercase().replace(' ', "")
            );
            println!("  2. Or set TEST_MODEL_PATH environment variable");
            println!("  3. Or place model in ./test_models/ directory");
            None
        }
    }
}

/// Measure tokens per second during generation
pub struct GenerationMetrics {
    pub total_tokens: usize,
    pub total_duration: Duration,
    pub first_token_latency: Duration,
    pub token_latencies: Vec<Duration>,
}

impl GenerationMetrics {
    pub fn tokens_per_second(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.total_tokens as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn latency_p50(&self) -> Duration {
        self.percentile_latency(50)
    }

    pub fn latency_p95(&self) -> Duration {
        self.percentile_latency(95)
    }

    pub fn latency_p99(&self) -> Duration {
        self.percentile_latency(99)
    }

    fn percentile_latency(&self, p: usize) -> Duration {
        if self.token_latencies.is_empty() {
            return Duration::ZERO;
        }

        let mut sorted = self.token_latencies.clone();
        sorted.sort();

        let idx = (p * sorted.len() / 100).min(sorted.len() - 1);
        sorted[idx]
    }

    pub fn summary(&self) -> String {
        format!(
            "Tokens: {}, Duration: {:.2}s, Speed: {:.2} tok/s, TTFT: {:.2}ms, P50: {:.2}ms, P95: {:.2}ms, P99: {:.2}ms",
            self.total_tokens,
            self.total_duration.as_secs_f64(),
            self.tokens_per_second(),
            self.first_token_latency.as_secs_f64() * 1000.0,
            self.latency_p50().as_secs_f64() * 1000.0,
            self.latency_p95().as_secs_f64() * 1000.0,
            self.latency_p99().as_secs_f64() * 1000.0,
        )
    }
}

// ============================================================================
// GGUF File Validation Tests
// ============================================================================

/// Test that we can read and validate a GGUF file header
#[test]
#[ignore = "Requires model file - run with --ignored"]
fn test_gguf_file_validation() {
    // Try to find any GGUF model
    let all_patterns = ["*.gguf"];
    let model_path = match skip_if_no_model(&all_patterns, "any GGUF") {
        Some(p) => p,
        None => return,
    };

    // Read and validate the file header
    let file = std::fs::File::open(&model_path).expect("Failed to open model file");
    let mut reader = std::io::BufReader::new(file);

    // Read magic number (first 4 bytes should be "GGUF")
    use std::io::Read;
    let mut magic = [0u8; 4];
    reader.read_exact(&mut magic).expect("Failed to read magic");

    // GGUF magic is "GGUF" in little-endian: 0x46554747
    assert_eq!(&magic, b"GGUF", "Invalid GGUF magic number");

    // Read version (4 bytes, little-endian u32)
    let mut version_bytes = [0u8; 4];
    reader
        .read_exact(&mut version_bytes)
        .expect("Failed to read version");
    let version = u32::from_le_bytes(version_bytes);

    // GGUF versions 2 and 3 are common
    assert!(
        version >= 2 && version <= 3,
        "Unexpected GGUF version: {}",
        version
    );

    println!("GGUF file validated:");
    println!("  Path: {}", model_path.display());
    println!("  Magic: GGUF");
    println!("  Version: {}", version);
}

// ============================================================================
// TinyLlama Tests
// ============================================================================

/// Test loading TinyLlama model
#[test]
#[ignore = "Requires TinyLlama model file"]
fn test_tinyllama_load() {
    let model_path = match skip_if_no_model(TINYLLAMA_PATTERNS, "TinyLlama") {
        Some(p) => p,
        None => return,
    };

    // This test verifies the model can be loaded without errors
    // In a real implementation, you would use the RuvLLM API
    println!("Would load TinyLlama from: {}", model_path.display());

    // Verify file is readable and has reasonable size
    let metadata = std::fs::metadata(&model_path).expect("Failed to get file metadata");
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    println!("Model size: {:.2} MB", size_mb);

    // TinyLlama Q4_K_M should be ~500-800MB
    assert!(
        size_mb > 100.0 && size_mb < 2000.0,
        "Unexpected model size: {:.2} MB (expected 100-2000 MB for TinyLlama)",
        size_mb
    );
}

/// Test text generation with TinyLlama
#[test]
#[ignore = "Requires TinyLlama model file"]
fn test_tinyllama_generation() {
    let model_path = match skip_if_no_model(TINYLLAMA_PATTERNS, "TinyLlama") {
        Some(p) => p,
        None => return,
    };

    println!(
        "Testing generation with TinyLlama: {}",
        model_path.display()
    );

    // Placeholder for actual generation test
    // In real implementation:
    //
    // let mut backend = CandleBackend::new().expect("Failed to create backend");
    // let config = ModelConfig {
    //     architecture: ModelArchitecture::Llama,
    //     quantization: Some(Quantization::Q4K),
    //     ..Default::default()
    // };
    // backend.load_model(model_path.to_str().unwrap(), config).expect("Failed to load model");
    //
    // let params = GenerateParams::default()
    //     .with_max_tokens(50)
    //     .with_temperature(0.7);
    //
    // let response = backend.generate("Hello, I am", params).expect("Generation failed");
    // assert!(!response.is_empty(), "Empty response from model");
    // println!("Generated: {}", response);

    println!("TinyLlama generation test placeholder - implement with actual backend");
}

/// Test streaming generation with TinyLlama
#[test]
#[ignore = "Requires TinyLlama model file"]
fn test_tinyllama_streaming() {
    let model_path = match skip_if_no_model(TINYLLAMA_PATTERNS, "TinyLlama") {
        Some(p) => p,
        None => return,
    };

    println!("Testing streaming with TinyLlama: {}", model_path.display());

    // Placeholder for streaming test
    // In real implementation:
    //
    // let stream = backend.generate_stream_v2("Once upon a time", params)?;
    // let mut token_count = 0;
    // for event in stream {
    //     match event? {
    //         StreamEvent::Token(token) => {
    //             print!("{}", token.text);
    //             token_count += 1;
    //         }
    //         StreamEvent::Done { tokens_per_second, .. } => {
    //             println!("\nSpeed: {:.2} tok/s", tokens_per_second);
    //         }
    //         StreamEvent::Error(e) => panic!("Streaming error: {}", e),
    //     }
    // }
    // assert!(token_count > 0, "No tokens generated");

    println!("TinyLlama streaming test placeholder - implement with actual backend");
}

// ============================================================================
// Phi-3 Tests
// ============================================================================

/// Test loading Phi-3 model
#[test]
#[ignore = "Requires Phi-3 model file"]
fn test_phi3_load() {
    let model_path = match skip_if_no_model(PHI3_PATTERNS, "Phi-3") {
        Some(p) => p,
        None => return,
    };

    println!("Would load Phi-3 from: {}", model_path.display());

    let metadata = std::fs::metadata(&model_path).expect("Failed to get file metadata");
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    println!("Model size: {:.2} MB", size_mb);

    // Phi-3 mini Q4_K_M should be ~2-3GB
    assert!(
        size_mb > 500.0 && size_mb < 5000.0,
        "Unexpected model size: {:.2} MB (expected 500-5000 MB for Phi-3)",
        size_mb
    );
}

/// Test text generation with Phi-3
#[test]
#[ignore = "Requires Phi-3 model file"]
fn test_phi3_generation() {
    let model_path = match skip_if_no_model(PHI3_PATTERNS, "Phi-3") {
        Some(p) => p,
        None => return,
    };

    println!("Testing generation with Phi-3: {}", model_path.display());
    println!("Phi-3 generation test placeholder - implement with actual backend");
}

/// Test Phi-3 with code completion prompt
#[test]
#[ignore = "Requires Phi-3 model file"]
fn test_phi3_code_completion() {
    let model_path = match skip_if_no_model(PHI3_PATTERNS, "Phi-3") {
        Some(p) => p,
        None => return,
    };

    println!(
        "Testing code completion with Phi-3: {}",
        model_path.display()
    );

    // Code completion prompts test the model's ability to understand code context
    let _prompts = [
        "def fibonacci(n):\n    \"\"\"Calculate the nth Fibonacci number.\"\"\"\n    ",
        "// Function to reverse a string in Rust\nfn reverse_string(s: &str) -> String {\n    ",
        "# Python function to check if a number is prime\ndef is_prime(n):\n    ",
    ];

    println!("Phi-3 code completion test placeholder - implement with actual backend");
}

// ============================================================================
// Qwen Tests
// ============================================================================

/// Test loading Qwen model
#[test]
#[ignore = "Requires Qwen model file"]
fn test_qwen_load() {
    let model_path = match skip_if_no_model(QWEN_PATTERNS, "Qwen") {
        Some(p) => p,
        None => return,
    };

    println!("Would load Qwen from: {}", model_path.display());

    let metadata = std::fs::metadata(&model_path).expect("Failed to get file metadata");
    let size_mb = metadata.len() as f64 / (1024.0 * 1024.0);

    println!("Model size: {:.2} MB", size_mb);

    // Qwen2-0.5B Q4_K_M should be ~300-500MB
    assert!(
        size_mb > 50.0 && size_mb < 1000.0,
        "Unexpected model size: {:.2} MB (expected 50-1000 MB for Qwen-0.5B)",
        size_mb
    );
}

/// Test text generation with Qwen
#[test]
#[ignore = "Requires Qwen model file"]
fn test_qwen_generation() {
    let model_path = match skip_if_no_model(QWEN_PATTERNS, "Qwen") {
        Some(p) => p,
        None => return,
    };

    println!("Testing generation with Qwen: {}", model_path.display());
    println!("Qwen generation test placeholder - implement with actual backend");
}

/// Test Qwen multilingual capability
#[test]
#[ignore = "Requires Qwen model file"]
fn test_qwen_multilingual() {
    let model_path = match skip_if_no_model(QWEN_PATTERNS, "Qwen") {
        Some(p) => p,
        None => return,
    };

    println!("Testing multilingual with Qwen: {}", model_path.display());

    // Qwen is known for good multilingual support
    let _prompts = [
        "Hello, how are you today?",      // English
        "Bonjour, comment allez-vous?",   // French
        "Hallo, wie geht es Ihnen?",      // German
        "Translate 'hello' to Chinese: ", // Translation task
    ];

    println!("Qwen multilingual test placeholder - implement with actual backend");
}

// ============================================================================
// Performance Benchmarks
// ============================================================================

/// Benchmark token generation speed
#[test]
#[ignore = "Requires model file - run with --ignored"]
fn test_benchmark_generation_speed() {
    // Try to find any available model
    let patterns = ["*.gguf"];
    let model_path = match skip_if_no_model(&patterns, "any GGUF") {
        Some(p) => p,
        None => return,
    };

    println!(
        "Benchmarking generation speed with: {}",
        model_path.display()
    );

    // Benchmark parameters
    let warmup_iterations = 3;
    let benchmark_iterations = 10;
    let max_tokens = 50;

    println!("Warmup: {} iterations", warmup_iterations);
    println!("Benchmark: {} iterations", benchmark_iterations);
    println!("Max tokens per generation: {}", max_tokens);

    // Placeholder for actual benchmark
    // In real implementation:
    //
    // // Warmup
    // for _ in 0..warmup_iterations {
    //     backend.generate("Hello", params.clone())?;
    // }
    //
    // // Benchmark
    // let mut speeds = Vec::new();
    // for i in 0..benchmark_iterations {
    //     let start = Instant::now();
    //     let stream = backend.generate_stream_v2("Hello", params.clone())?;
    //     let mut tokens = 0;
    //     for event in stream {
    //         if let StreamEvent::Token(_) = event? {
    //             tokens += 1;
    //         }
    //     }
    //     let elapsed = start.elapsed();
    //     let speed = tokens as f64 / elapsed.as_secs_f64();
    //     speeds.push(speed);
    //     println!("  Iteration {}: {:.2} tok/s", i + 1, speed);
    // }
    //
    // let avg_speed = speeds.iter().sum::<f64>() / speeds.len() as f64;
    // println!("\nAverage speed: {:.2} tok/s", avg_speed);

    println!("Benchmark placeholder - implement with actual backend");
}

/// Test memory usage during inference
#[test]
#[ignore = "Requires model file"]
fn test_memory_usage() {
    let patterns = ["*.gguf"];
    let model_path = match skip_if_no_model(&patterns, "any GGUF") {
        Some(p) => p,
        None => return,
    };

    println!("Testing memory usage with: {}", model_path.display());

    // Get initial memory usage (platform-specific)
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok();

        if let Some(output) = output {
            if let Ok(rss) = String::from_utf8_lossy(&output.stdout)
                .trim()
                .parse::<u64>()
            {
                println!("Initial RSS: {} KB", rss);
            }
        }
    }

    println!("Memory usage test placeholder - implement with actual backend");
}

// ============================================================================
// Model Comparison Tests
// ============================================================================

/// Compare generation quality across different models
#[test]
#[ignore = "Requires multiple model files"]
fn test_model_comparison() {
    println!("Model comparison test");

    let test_prompts = [
        "What is the capital of France?",
        "Write a haiku about programming.",
        "Explain quantum computing in simple terms.",
    ];

    // Find all available models
    let models: Vec<(&str, Option<PathBuf>)> = vec![
        ("TinyLlama", find_test_model(TINYLLAMA_PATTERNS)),
        ("Phi-3", find_test_model(PHI3_PATTERNS)),
        ("Qwen", find_test_model(QWEN_PATTERNS)),
    ];

    let available: Vec<_> = models.iter().filter(|(_, path)| path.is_some()).collect();

    if available.is_empty() {
        println!("SKIPPED: No models available for comparison");
        return;
    }

    println!("Available models for comparison:");
    for (name, path) in &available {
        if let Some(p) = path {
            println!("  - {}: {}", name, p.display());
        }
    }

    println!("\nTest prompts:");
    for (i, prompt) in test_prompts.iter().enumerate() {
        println!("  {}. {}", i + 1, prompt);
    }

    println!("\nModel comparison placeholder - implement with actual backend");
}

// ============================================================================
// Unit Tests for Helpers
// ============================================================================

#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn test_glob_pattern_matching() {
        assert!(matches_glob_pattern("tinyllama.gguf", "*.gguf"));
        assert!(matches_glob_pattern("tinyllama.gguf", "tinyllama*"));
        assert!(matches_glob_pattern(
            "tinyllama-1.1b.gguf",
            "*tinyllama*.gguf"
        ));
        assert!(matches_glob_pattern("model.gguf", "model.gguf"));
        assert!(!matches_glob_pattern("tinyllama.bin", "*.gguf"));
        assert!(!matches_glob_pattern("other.gguf", "tinyllama*"));
    }

    #[test]
    fn test_expand_path_no_tilde() {
        let path = expand_path("/usr/local/models");
        assert_eq!(path, PathBuf::from("/usr/local/models"));
    }

    #[test]
    fn test_expand_path_relative() {
        let path = expand_path("./models");
        assert_eq!(path, PathBuf::from("./models"));
    }

    #[test]
    fn test_metrics_percentile() {
        let metrics = GenerationMetrics {
            total_tokens: 100,
            total_duration: Duration::from_secs(10),
            first_token_latency: Duration::from_millis(50),
            token_latencies: (0..100).map(|i| Duration::from_millis(i as u64)).collect(),
        };

        assert_eq!(metrics.tokens_per_second(), 10.0);
        assert!(metrics.latency_p50() >= Duration::from_millis(49));
        assert!(metrics.latency_p50() <= Duration::from_millis(51));
        assert!(metrics.latency_p99() >= Duration::from_millis(98));
    }

    #[test]
    fn test_metrics_empty_latencies() {
        let metrics = GenerationMetrics {
            total_tokens: 0,
            total_duration: Duration::ZERO,
            first_token_latency: Duration::ZERO,
            token_latencies: vec![],
        };

        assert_eq!(metrics.tokens_per_second(), 0.0);
        assert_eq!(metrics.latency_p50(), Duration::ZERO);
    }
}
