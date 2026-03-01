//! Classification Demo for ESP32
//!
//! Demonstrates simple text classification using the tiny model.

use ruvllm_esp32::prelude::*;
use ruvllm_esp32::model::ModelConfig;
use ruvllm_esp32::embedding::SimpleTokenizer;

fn main() {
    println!("=== ESP32 Classification Demo ===\n");

    // Create model
    let config = ModelConfig::for_variant(Esp32Variant::Esp32);
    println!("Model configuration:");
    println!("  Vocab size: {}", config.vocab_size);
    println!("  Embed dim: {}", config.embed_dim);
    println!("  Hidden dim: {}", config.hidden_dim);
    println!("  Layers: {}", config.num_layers);
    println!("  Estimated size: {} bytes\n", config.estimate_size());

    let model = TinyModel::new(config).unwrap();
    let mut engine = MicroEngine::new(model).unwrap();

    // Tokenizer
    let tokenizer = SimpleTokenizer::ascii();

    // Classification examples
    let examples = [
        ("hello world", "greeting"),
        ("buy now", "spam"),
        ("the cat sat", "narrative"),
        ("2 + 2 = 4", "math"),
    ];

    println!("Classification Demo:");
    println!("(Note: Uses random weights, so classifications are random)\n");

    for (text, _expected) in &examples {
        let tokens = tokenizer.encode(text);
        let prompt: heapless::Vec<u16, 64> = tokens.iter().copied().collect();

        engine.reset();

        // Run single forward pass to get logits
        for &token in &prompt {
            let _ = engine.forward_one(token);
        }

        // Get predicted class from output (using token ID as proxy)
        let gen_config = InferenceConfig {
            max_tokens: 1,
            greedy: true,
            ..Default::default()
        };

        engine.reset();
        let result = engine.generate(&prompt, &gen_config).unwrap();

        let predicted_class = if result.tokens.is_empty() {
            0
        } else {
            result.tokens[0] % 4  // Map to 4 classes
        };

        let class_names = ["greeting", "spam", "narrative", "math"];
        println!(
            "  '{}' -> predicted: {} (class {})",
            text,
            class_names[predicted_class as usize],
            predicted_class
        );
    }

    // Memory usage
    let usage = engine.memory_usage();
    println!("\nMemory usage:");
    println!("  Model: {} bytes", usage.model_weights);
    println!("  Buffers: {} bytes", usage.activation_buffers);
    println!("  KV cache: {} bytes", usage.kv_cache);
    println!("  Total: {} bytes ({:.1} KB)", usage.total, usage.total as f32 / 1024.0);

    println!("\nDemo complete!");
}
