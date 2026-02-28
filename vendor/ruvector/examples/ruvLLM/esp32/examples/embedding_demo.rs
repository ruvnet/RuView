//! Embedding Demo for ESP32
//!
//! Demonstrates embedding lookup and similarity computation.

use ruvllm_esp32::prelude::*;
use ruvllm_esp32::embedding::{EmbeddingTable, SimpleTokenizer};

fn main() {
    println!("=== ESP32 Embedding Demo ===\n");

    // Create tokenizer
    let tokenizer = SimpleTokenizer::ascii();

    // Create embedding table
    let embed: EmbeddingTable<256, 64> = EmbeddingTable::random(256, 64, 42).unwrap();

    println!("Embedding table created:");
    println!("  Vocab size: 256");
    println!("  Embed dim: 64");
    println!("  Memory: {} bytes\n", embed.memory_size());

    // Tokenize some text
    let texts = ["hello", "world", "esp32"];

    for text in &texts {
        let tokens = tokenizer.encode(text);
        println!("Text: '{}' -> tokens: {:?}", text, tokens.as_slice());

        // Get embedding for first token
        let mut embedding = [0i8; 64];
        embed.lookup(tokens[0], &mut embedding).unwrap();

        // Compute L2 norm (simplified)
        let norm: i32 = embedding.iter().map(|&x| (x as i32) * (x as i32)).sum();
        println!("  First token embedding norm²: {}", norm);
    }

    // Compute similarity between embeddings
    println!("\n=== Similarity Demo ===\n");

    let mut embed1 = [0i8; 64];
    let mut embed2 = [0i8; 64];

    embed.lookup('h' as u16, &mut embed1).unwrap();
    embed.lookup('H' as u16, &mut embed2).unwrap();

    // Dot product similarity
    let similarity: i32 = embed1.iter()
        .zip(embed2.iter())
        .map(|(&a, &b)| a as i32 * b as i32)
        .sum();

    println!("Similarity('h', 'H'): {}", similarity);

    embed.lookup('a' as u16, &mut embed2).unwrap();
    let similarity2: i32 = embed1.iter()
        .zip(embed2.iter())
        .map(|(&a, &b)| a as i32 * b as i32)
        .sum();

    println!("Similarity('h', 'a'): {}", similarity2);

    println!("\nDemo complete!");
}
