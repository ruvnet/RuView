// RuvLLM ESP32 - Tiny LLM Inference Demo
// This example shows how to run a tiny language model on ESP32

use ruvllm_esp32::prelude::*;
use ruvllm_esp32::ruvector::{MicroRAG, RAGConfig};

fn main() {
    println!("=== RuvLLM ESP32 Demo ===");
    println!("Initializing Tiny LLM Engine...");

    // Create configuration for ESP32 variant
    let config = ModelConfig::for_variant(Esp32Variant::Esp32);
    println!("Model Configuration:");
    println!("  Vocab Size: {}", config.vocab_size);
    println!("  Embed Dim: {}", config.embed_dim);
    println!("  Layers: {}", config.num_layers);
    println!("  Heads: {}", config.num_heads);
    println!("  Max Seq Len: {}", config.max_seq_len);

    // Initialize the tiny model
    match TinyModel::new(config) {
        Ok(model) => {
            println!("✓ Model initialized successfully");

            // Create the inference engine
            match MicroEngine::new(model) {
                Ok(mut engine) => {
                    println!("✓ Inference engine ready");

                    // Initialize RAG for knowledge-grounded responses
                    let mut rag = MicroRAG::new(RAGConfig::default());
                    println!("✓ RAG system initialized");

                    // Simple embedding function for demo
                    let embed = |text: &str| -> [i8; 64] {
                        let mut embedding = [0i8; 64];
                        // Simple hash-based embedding for demo
                        for (i, byte) in text.bytes().enumerate() {
                            if i < 64 {
                                embedding[i] = (byte as i8) % 127;
                            }
                        }
                        embedding
                    };

                    // Add knowledge to RAG
                    println!("\nAdding knowledge to RAG system:");
                    let knowledge_entries = [
                        "The kitchen light is called 'main light'",
                        "The ESP32 has 520KB of SRAM",
                        "RuvLLM supports INT8 quantization",
                        "The model uses transformer architecture",
                    ];

                    for entry in knowledge_entries.iter() {
                        let embedding = embed(entry);
                        match rag.add_knowledge(entry, &embedding) {
                            Ok(_) => println!("  ✓ {}", entry),
                            Err(e) => println!("  ✗ Failed: {:?}", e),
                        }
                    }

                    // Run inference demo
                    println!("\n=== Running Inference Demo ===");

                    // Example input tokens
                    let input_tokens = [1u16, 2, 3, 4, 5];
                    println!("Input tokens: {:?}", input_tokens);

                    // Configure inference
                    let inference_config = InferenceConfig {
                        max_tokens: 10,
                        greedy: true,
                        temperature: 1.0,
                        seed: 42,
                        top_k: 50,
                    };

                    // Generate tokens
                    match engine.generate(&input_tokens, &inference_config) {
                        Ok(result) => {
                            println!("\n✓ Inference successful!");
                            println!("Generated {} tokens in {} us",
                                     result.tokens.len(),
                                     result.inference_time_us);
                            println!("Output tokens: {:?}", result.tokens);
                        }
                        Err(e) => {
                            println!("\n✗ Inference failed: {:?}", e);
                        }
                    }

                    // Query RAG system
                    println!("\n=== RAG Query Demo ===");
                    let query = "What is the kitchen light?";
                    println!("Query: {}", query);

                    let query_embed = embed(query);
                    let rag_result = rag.retrieve(&query_embed);

                    println!("RAG Results:");
                    println!("  Context: {:?}", rag_result.context);
                    println!("  Source IDs: {:?}", rag_result.source_ids);
                    println!("  Scores: {:?}", rag_result.scores);
                    println!("  Truncated: {}", rag_result.truncated);

                    println!("\n=== Demo Complete ===");
                    println!("RuvLLM ESP32 is ready for deployment!");
                }
                Err(e) => {
                    println!("✗ Failed to create engine: {:?}", e);
                }
            }
        }
        Err(e) => {
            println!("✗ Failed to create model: {:?}", e);
        }
    }
}
