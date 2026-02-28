//! RuvLLM ESP32 Demo Application
//!
//! Demonstrates tiny LLM inference on ESP32 microcontrollers.

#![cfg_attr(feature = "no_std", no_std)]
#![cfg_attr(feature = "no_std", no_main)]

#[cfg(feature = "esp32-std")]
use esp_idf_svc::hal::prelude::*;

#[cfg(feature = "no_std")]
extern crate alloc;

// For host testing, import from crate
#[cfg(feature = "host-test")]
use ruvllm_esp32::prelude::*;
#[cfg(feature = "host-test")]
use ruvllm_esp32::model::ModelConfig;
#[cfg(feature = "host-test")]
use ruvllm_esp32::embedding::SimpleTokenizer;

// For ESP32 builds
#[cfg(feature = "esp32-std")]
use ruvllm_esp32::prelude::*;
#[cfg(feature = "esp32-std")]
use ruvllm_esp32::model::ModelConfig;
#[cfg(feature = "esp32-std")]
use ruvllm_esp32::embedding::SimpleTokenizer;

#[cfg(feature = "esp32-std")]
fn main() -> anyhow::Result<()> {
    // Initialize ESP-IDF
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    log::info!("=== RuvLLM ESP32 Demo ===");
    log::info!("Initializing...");

    // Detect ESP32 variant and create appropriate model
    let variant = detect_variant();
    log::info!("Detected variant: {:?}", variant);
    log::info!("Available RAM: {} KB", variant.sram_bytes() / 1024);
    log::info!("Max model RAM: {} KB", variant.max_model_ram() / 1024);

    // Create model config for this variant
    let config = ModelConfig::for_variant(variant);
    log::info!("Model config:");
    log::info!("  Vocab size: {}", config.vocab_size);
    log::info!("  Embed dim: {}", config.embed_dim);
    log::info!("  Hidden dim: {}", config.hidden_dim);
    log::info!("  Layers: {}", config.num_layers);
    log::info!("  Heads: {}", config.num_heads);
    log::info!("  Estimated size: {} KB", config.estimate_size() / 1024);

    // Create the model
    log::info!("Creating model...");
    let model = TinyModel::new(config)?;
    log::info!("Model created, actual size: {} KB", model.memory_size() / 1024);

    // Create inference engine
    log::info!("Creating inference engine...");
    let mut engine = MicroEngine::new(model)?;

    let usage = engine.memory_usage();
    log::info!("Memory usage breakdown:");
    log::info!("  Model weights: {} KB", usage.model_weights / 1024);
    log::info!("  Activation buffers: {} KB", usage.activation_buffers / 1024);
    log::info!("  KV cache: {} KB", usage.kv_cache / 1024);
    log::info!("  Total: {} KB", usage.total / 1024);

    // Run inference benchmark
    log::info!("Running inference benchmark...");
    run_benchmark(&mut engine)?;

    // Interactive demo (if UART available)
    log::info!("Starting interactive demo...");
    run_interactive(&mut engine)?;

    Ok(())
}

// Host test main function
#[cfg(feature = "host-test")]
fn main() -> anyhow::Result<()> {
    println!("=== RuvLLM ESP32 Demo (Host Simulation) ===");
    println!("Initializing...");

    // Detect ESP32 variant (simulated)
    let variant = Esp32Variant::Esp32;
    println!("Simulating variant: {:?}", variant);
    println!("Available RAM: {} KB", variant.sram_bytes() / 1024);
    println!("Max model RAM: {} KB", variant.max_model_ram() / 1024);

    // Create model config for this variant
    let config = ModelConfig::for_variant(variant);
    println!("Model config:");
    println!("  Vocab size: {}", config.vocab_size);
    println!("  Embed dim: {}", config.embed_dim);
    println!("  Hidden dim: {}", config.hidden_dim);
    println!("  Layers: {}", config.num_layers);
    println!("  Heads: {}", config.num_heads);
    println!("  Estimated size: {} KB", config.estimate_size() / 1024);

    // Create the model
    println!("Creating model...");
    let model = TinyModel::new(config)?;
    println!("Model created, actual size: {} KB", model.memory_size() / 1024);

    // Create inference engine
    println!("Creating inference engine...");
    let mut engine = MicroEngine::new(model)?;

    let usage = engine.memory_usage();
    println!("Memory usage breakdown:");
    println!("  Model weights: {} KB", usage.model_weights / 1024);
    println!("  Activation buffers: {} KB", usage.activation_buffers / 1024);
    println!("  KV cache: {} KB", usage.kv_cache / 1024);
    println!("  Total: {} KB", usage.total / 1024);

    // Run inference benchmark
    println!("\nRunning inference benchmark...");
    run_benchmark_host(&mut engine)?;

    // Interactive demo
    println!("\nStarting interactive demo...");
    run_interactive_host(&mut engine)?;

    Ok(())
}

#[cfg(feature = "host-test")]
fn run_benchmark_host(engine: &mut MicroEngine) -> anyhow::Result<()> {
    use std::time::Instant;

    let config = InferenceConfig {
        max_tokens: 10,
        greedy: true,
        ..Default::default()
    };

    // Warmup
    println!("Warmup run...");
    let prompt = [1u16, 2, 3, 4, 5];
    let _ = engine.generate(&prompt, &config)?;
    engine.reset();

    // Benchmark runs
    const NUM_RUNS: usize = 10;
    let mut total_time_us = 0u64;
    let mut total_tokens = 0usize;

    println!("Running {} benchmark iterations...", NUM_RUNS);

    for i in 0..NUM_RUNS {
        let start = Instant::now();
        let result = engine.generate(&prompt, &config)?;
        let elapsed = start.elapsed();

        total_time_us += elapsed.as_micros() as u64;
        total_tokens += result.tokens.len();

        println!(
            "  Run {}: {} tokens in {} us ({:.1} tok/s)",
            i + 1,
            result.tokens.len(),
            elapsed.as_micros(),
            result.tokens.len() as f32 / elapsed.as_secs_f32()
        );

        engine.reset();
    }

    let avg_time_us = total_time_us / NUM_RUNS as u64;
    let avg_tokens = total_tokens / NUM_RUNS;
    let tokens_per_sec = (avg_tokens as f32 * 1_000_000.0) / avg_time_us as f32;

    println!("=== Benchmark Results ===");
    println!("Average time: {} us", avg_time_us);
    println!("Average tokens: {}", avg_tokens);
    println!("Throughput: {:.1} tokens/sec", tokens_per_sec);
    println!("Latency per token: {:.1} us", avg_time_us as f32 / avg_tokens.max(1) as f32);

    // Estimate ESP32 performance (roughly 15x slower)
    let esp32_time_us = avg_time_us * 15;
    let esp32_tokens_per_sec = tokens_per_sec / 15.0;
    println!("\nEstimated ESP32 performance:");
    println!("  Time: {} us ({:.2} ms)", esp32_time_us, esp32_time_us as f32 / 1000.0);
    println!("  Throughput: {:.1} tokens/sec", esp32_tokens_per_sec);

    // Performance counters
    let counters = engine.perf_counters();
    println!("\nPerformance counters:");
    println!("  Embeddings: {}", counters.embeddings);
    println!("  Attention ops: {}", counters.attention_ops);
    println!("  FFN ops: {}", counters.ffn_ops);

    Ok(())
}

#[cfg(feature = "host-test")]
fn run_interactive_host(engine: &mut MicroEngine) -> anyhow::Result<()> {
    let tokenizer = SimpleTokenizer::ascii();
    let config = InferenceConfig {
        max_tokens: 20,
        greedy: true,
        ..Default::default()
    };

    // Simple demo prompts
    let prompts = [
        "Hello",
        "The quick brown",
        "1 + 1 =",
    ];

    for prompt in &prompts {
        println!("Prompt: '{}'", prompt);

        let tokens = tokenizer.encode(prompt);
        let prompt_ids: heapless::Vec<u16, 64> = tokens.iter().copied().collect();

        engine.reset();
        let result = engine.generate(&prompt_ids, &config)?;

        let output = tokenizer.decode(&result.tokens);
        let output_str = core::str::from_utf8(&output).unwrap_or("<invalid>");

        println!("Generated: '{}'", output_str);
        println!("Tokens: {:?}", result.tokens.as_slice());
        println!("---");
    }

    Ok(())
}

#[cfg(not(any(feature = "host-test", feature = "esp32-std")))]
#[no_mangle]
pub extern "C" fn main() -> ! {
    // Bare-metal entry point
    // Initialize heap, etc.
    loop {}
}

/// Detect ESP32 variant at runtime
fn detect_variant() -> Esp32Variant {
    // In real code, this would check chip ID
    // For now, default to ESP32
    #[cfg(feature = "esp32s3-simd")]
    return Esp32Variant::Esp32S3;

    #[cfg(not(feature = "esp32s3-simd"))]
    Esp32Variant::Esp32
}

/// Run inference benchmark
#[cfg(feature = "std")]
fn run_benchmark(engine: &mut MicroEngine) -> anyhow::Result<()> {
    use std::time::Instant;

    let config = InferenceConfig {
        max_tokens: 10,
        greedy: true,
        ..Default::default()
    };

    // Warmup
    log::info!("Warmup run...");
    let prompt = [1u16, 2, 3, 4, 5];
    let _ = engine.generate(&prompt, &config)?;
    engine.reset();

    // Benchmark runs
    const NUM_RUNS: usize = 10;
    let mut total_time_us = 0u64;
    let mut total_tokens = 0usize;

    log::info!("Running {} benchmark iterations...", NUM_RUNS);

    for i in 0..NUM_RUNS {
        let start = Instant::now();
        let result = engine.generate(&prompt, &config)?;
        let elapsed = start.elapsed();

        total_time_us += elapsed.as_micros() as u64;
        total_tokens += result.tokens.len();

        log::info!(
            "  Run {}: {} tokens in {} us ({:.1} tok/s)",
            i + 1,
            result.tokens.len(),
            elapsed.as_micros(),
            result.tokens.len() as f32 / elapsed.as_secs_f32()
        );

        engine.reset();
    }

    let avg_time_us = total_time_us / NUM_RUNS as u64;
    let avg_tokens = total_tokens / NUM_RUNS;
    let tokens_per_sec = (avg_tokens as f32 * 1_000_000.0) / avg_time_us as f32;

    log::info!("=== Benchmark Results ===");
    log::info!("Average time: {} us", avg_time_us);
    log::info!("Average tokens: {}", avg_tokens);
    log::info!("Throughput: {:.1} tokens/sec", tokens_per_sec);
    log::info!("Latency per token: {:.1} us", avg_time_us as f32 / avg_tokens as f32);

    // Memory stats
    let counters = engine.perf_counters();
    log::info!("Performance counters:");
    log::info!("  Embeddings: {}", counters.embeddings);
    log::info!("  Attention ops: {}", counters.attention_ops);
    log::info!("  FFN ops: {}", counters.ffn_ops);

    Ok(())
}

/// Run interactive text generation
#[cfg(feature = "std")]
fn run_interactive(engine: &mut MicroEngine) -> anyhow::Result<()> {
    let tokenizer = SimpleTokenizer::ascii();
    let config = InferenceConfig {
        max_tokens: 20,
        greedy: true,
        ..Default::default()
    };

    // Simple demo prompts
    let prompts = [
        "Hello",
        "The quick brown",
        "1 + 1 =",
    ];

    for prompt in &prompts {
        log::info!("Prompt: '{}'", prompt);

        let tokens = tokenizer.encode(prompt);
        let prompt_ids: heapless::Vec<u16, 64> = tokens.iter().copied().collect();

        engine.reset();
        let result = engine.generate(&prompt_ids, &config)?;

        let output = tokenizer.decode(&result.tokens);
        let output_str = core::str::from_utf8(&output).unwrap_or("<invalid>");

        log::info!("Generated: '{}'", output_str);
        log::info!("Tokens: {:?}", result.tokens.as_slice());
        log::info!("---");
    }

    Ok(())
}

// Panic handler for no_std
#[cfg(all(feature = "no_std", not(test)))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
