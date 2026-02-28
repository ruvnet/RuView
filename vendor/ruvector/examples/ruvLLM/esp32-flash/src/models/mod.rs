//! Model Zoo - Pre-quantized Models for RuvLLM ESP32
//!
//! Ready-to-use language models optimized for ESP32 microcontrollers.
//!
//! # Available Models
//!
//! | Model | Size | RAM | Tokens/sec | Use Case |
//! |-------|------|-----|------------|----------|
//! | TinyStories | 8KB | 20KB | ~50 | Story generation |
//! | MicroChat | 16KB | 32KB | ~30 | Simple chatbot |
//! | NanoEmbed | 4KB | 8KB | ~100 | Embeddings only |
//! | TinyQA | 12KB | 24KB | ~40 | Question answering |

use heapless::Vec;

/// Model metadata
#[derive(Clone)]
pub struct ModelInfo {
    /// Model name
    pub name: &'static str,
    /// Model version
    pub version: &'static str,
    /// Model size in bytes
    pub size_bytes: u32,
    /// Required RAM in bytes
    pub ram_bytes: u32,
    /// Vocabulary size
    pub vocab_size: u16,
    /// Hidden dimension
    pub hidden_dim: u16,
    /// Number of layers
    pub num_layers: u8,
    /// Number of attention heads
    pub num_heads: u8,
    /// Maximum sequence length
    pub max_seq_len: u16,
    /// Quantization bits (8 = INT8, 4 = INT4, 1 = binary)
    pub quant_bits: u8,
    /// Description
    pub description: &'static str,
}

/// Available pre-quantized models
pub const MODELS: &[ModelInfo] = &[
    ModelInfo {
        name: "tinystories-1m",
        version: "1.0.0",
        size_bytes: 8 * 1024,      // 8KB
        ram_bytes: 20 * 1024,      // 20KB
        vocab_size: 256,
        hidden_dim: 64,
        num_layers: 2,
        num_heads: 2,
        max_seq_len: 64,
        quant_bits: 8,
        description: "Tiny model for simple story generation",
    },
    ModelInfo {
        name: "microchat-2m",
        version: "1.0.0",
        size_bytes: 16 * 1024,     // 16KB
        ram_bytes: 32 * 1024,      // 32KB
        vocab_size: 512,
        hidden_dim: 96,
        num_layers: 3,
        num_heads: 3,
        max_seq_len: 128,
        quant_bits: 8,
        description: "Simple chatbot for basic conversations",
    },
    ModelInfo {
        name: "nanoembed-500k",
        version: "1.0.0",
        size_bytes: 4 * 1024,      // 4KB
        ram_bytes: 8 * 1024,       // 8KB
        vocab_size: 256,
        hidden_dim: 32,
        num_layers: 1,
        num_heads: 1,
        max_seq_len: 32,
        quant_bits: 8,
        description: "Ultra-light embedding model for semantic search",
    },
    ModelInfo {
        name: "tinyqa-1.5m",
        version: "1.0.0",
        size_bytes: 12 * 1024,     // 12KB
        ram_bytes: 24 * 1024,      // 24KB
        vocab_size: 384,
        hidden_dim: 80,
        num_layers: 2,
        num_heads: 2,
        max_seq_len: 96,
        quant_bits: 8,
        description: "Question-answering model for simple queries",
    },
    ModelInfo {
        name: "binary-embed-250k",
        version: "1.0.0",
        size_bytes: 2 * 1024,      // 2KB
        ram_bytes: 4 * 1024,       // 4KB
        vocab_size: 128,
        hidden_dim: 64,
        num_layers: 1,
        num_heads: 1,
        max_seq_len: 16,
        quant_bits: 1,             // Binary quantization
        description: "Binary quantized embeddings (32x compression)",
    },
];

/// Model selection by use case
#[derive(Debug, Clone, Copy)]
pub enum UseCase {
    /// Story/text generation
    Generation,
    /// Conversational AI
    Chat,
    /// Semantic embeddings
    Embedding,
    /// Question answering
    QA,
    /// Minimum memory footprint
    MinMemory,
}

/// Get recommended model for use case
pub fn recommend_model(use_case: UseCase, max_ram_kb: u32) -> Option<&'static ModelInfo> {
    let max_ram = max_ram_kb * 1024;

    let candidates: Vec<&ModelInfo, 8> = MODELS
        .iter()
        .filter(|m| m.ram_bytes <= max_ram)
        .collect();

    match use_case {
        UseCase::Generation => candidates
            .iter()
            .find(|m| m.name.contains("stories"))
            .copied(),
        UseCase::Chat => candidates
            .iter()
            .find(|m| m.name.contains("chat"))
            .copied(),
        UseCase::Embedding => candidates
            .iter()
            .find(|m| m.name.contains("embed"))
            .copied(),
        UseCase::QA => candidates
            .iter()
            .find(|m| m.name.contains("qa"))
            .copied(),
        UseCase::MinMemory => candidates
            .iter()
            .min_by_key(|m| m.ram_bytes)
            .copied(),
    }
}

/// Get model by name
pub fn get_model(name: &str) -> Option<&'static ModelInfo> {
    MODELS.iter().find(|m| m.name == name)
}

/// List all models
pub fn list_models() -> &'static [ModelInfo] {
    MODELS
}

/// Calculate tokens per second estimate for model on given chip
pub fn estimate_performance(model: &ModelInfo, chip: &str) -> u32 {
    let base_speed = match chip {
        "esp32s3" => 60,  // SIMD acceleration
        "esp32" => 40,
        "esp32s2" => 35,
        "esp32c3" => 30,
        "esp32c6" => 35,
        _ => 30,
    };

    // Adjust for model complexity
    let complexity_factor = 1.0 / (model.num_layers as f32 * 0.3 + 1.0);
    let quant_factor = if model.quant_bits == 1 { 2.0 } else { 1.0 };

    (base_speed as f32 * complexity_factor * quant_factor) as u32
}

/// Print model info table
pub fn print_model_table() -> heapless::String<1024> {
    let mut output = heapless::String::new();

    let _ = output.push_str("Available Models:\n");
    let _ = output.push_str("─────────────────────────────────────────────────\n");
    let _ = output.push_str("Name              Size    RAM     Quant  Use Case\n");
    let _ = output.push_str("─────────────────────────────────────────────────\n");

    for model in MODELS {
        let _ = core::fmt::write(
            &mut output,
            format_args!(
                "{:<17} {:>4}KB  {:>4}KB  INT{:<2}  {}\n",
                model.name,
                model.size_bytes / 1024,
                model.ram_bytes / 1024,
                model.quant_bits,
                model.description.chars().take(20).collect::<heapless::String<20>>()
            )
        );
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_lookup() {
        let model = get_model("tinystories-1m");
        assert!(model.is_some());
        assert_eq!(model.unwrap().vocab_size, 256);
    }

    #[test]
    fn test_recommend_model() {
        let model = recommend_model(UseCase::MinMemory, 10);
        assert!(model.is_some());
        assert_eq!(model.unwrap().name, "binary-embed-250k");
    }

    #[test]
    fn test_performance_estimate() {
        let model = get_model("nanoembed-500k").unwrap();
        let speed = estimate_performance(model, "esp32s3");
        assert!(speed > 0);
    }
}
