//! SIMD-Optimized CPU Inference Demo
//!
//! Demonstrates real local LLM inference using SIMD-optimized operations.

use ruvllm::{SimdGenerationConfig, SimdInferenceEngine};
use std::time::Instant;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║           RuvLLM SIMD-Optimized CPU Inference Demo                         ║");
    println!("║     Real Local LLM with AVX2/SSE4.1 SIMD Acceleration                      ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");

    // Detect SIMD capabilities
    println!("🔍 Detecting CPU SIMD capabilities...");
    #[cfg(target_arch = "x86_64")]
    {
        if is_x86_feature_detected!("avx2") {
            println!("   ✓ AVX2 detected - using 256-bit SIMD operations");
        } else if is_x86_feature_detected!("sse4.1") {
            println!("   ✓ SSE4.1 detected - using 128-bit SIMD operations");
        } else {
            println!("   ⚠ No SIMD detected - using scalar fallback");
        }
    }
    #[cfg(not(target_arch = "x86_64"))]
    println!("   ℹ Non-x86 architecture - using optimized scalar operations");

    // Initialize engine
    println!("\n📦 Initializing SIMD inference engine...");
    let start = Instant::now();
    let engine = SimdInferenceEngine::new_demo();
    let (vocab_size, num_layers) = engine.model_info();
    println!(
        "   ✓ Initialized in {:.2}ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    println!(
        "   ℹ Model: {} vocab, {} transformer layers",
        vocab_size, num_layers
    );
    println!("   ℹ Quantization: Q4 (4-bit weights, 4x memory reduction)");
    println!("   ℹ Architecture: RMSNorm + SiLU + Multi-Head Attention");

    // Test prompts
    let prompts = vec![
        "Hello, how are you?",
        "What is machine learning?",
        "Explain quantum computing",
        "Write code for fibonacci",
        "The meaning of life is",
    ];

    let config = SimdGenerationConfig {
        max_tokens: 32,
        temperature: 0.8,
        top_p: 0.9,
        top_k: 40,
        repeat_penalty: 1.1,
    };

    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║                        SIMD Inference Benchmarks                           ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!("║ Generation Config: max_tokens=32, temp=0.8, top_p=0.9, top_k=40           ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");

    let mut total_tokens = 0;
    let mut total_time = 0.0;

    for (i, prompt) in prompts.iter().enumerate() {
        println!("📝 Prompt {}: \"{}\"", i + 1, prompt);

        let (output, tokens, time_ms) = engine.generate(prompt, &config, None);

        println!(
            "   📤 Output: \"{}\"",
            output.chars().take(60).collect::<String>()
        );
        println!(
            "   ⏱  Tokens: {}, Time: {:.2}ms, Speed: {:.1} tok/s",
            tokens,
            time_ms,
            if time_ms > 0.0 {
                (tokens as f64 / time_ms) * 1000.0
            } else {
                0.0
            }
        );
        println!();

        total_tokens += tokens;
        total_time += time_ms;
    }

    // Session continuity test
    println!("╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║                      Session Continuity (KV Cache)                         ║");
    println!("╚═══════════════════════════════════════════════════════════════════════════╝\n");

    let session_id = "test-session";
    let conversation = vec!["Hello!", "Tell me more", "That's interesting"];

    for (i, msg) in conversation.iter().enumerate() {
        let (output, tokens, time_ms) = engine.generate(msg, &config, Some(session_id));
        println!(
            "Turn {}: \"{}\" → \"{}\" ({} tokens, {:.2}ms)",
            i + 1,
            msg,
            output.chars().take(40).collect::<String>(),
            tokens,
            time_ms
        );
    }

    // Summary
    println!("\n╔═══════════════════════════════════════════════════════════════════════════╗");
    println!("║                            Performance Summary                             ║");
    println!("╠═══════════════════════════════════════════════════════════════════════════╣");
    println!(
        "║ Total tokens generated: {:>6}                                            ║",
        total_tokens
    );
    println!(
        "║ Total inference time:   {:>6.2}ms                                          ║",
        total_time
    );
    if total_time > 0.0 {
        println!(
            "║ Average throughput:     {:>6.1} tokens/sec                                ║",
            (total_tokens as f64 / total_time) * 1000.0
        );
        println!(
            "║ Average latency:        {:>6.2}ms/token                                   ║",
            total_time / total_tokens as f64
        );
    }
    println!("╚═══════════════════════════════════════════════════════════════════════════╝");

    println!("\n✅ SIMD inference demo complete!");
    println!("\n📌 Note: This demo uses a small random-weight model for demonstration.");
    println!("   For production, connect to real LLM backends via the inference pool.");
}
