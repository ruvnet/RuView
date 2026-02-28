//! Tokenizer wrapper for WASM embedding generation

use crate::error::{Result, WasmEmbeddingError};
use tokenizers::Tokenizer;

/// Tokenizer wrapper that handles text encoding
pub struct WasmTokenizer {
    tokenizer: Tokenizer,
    max_length: usize,
}

/// Encoded text ready for model inference
#[derive(Debug, Clone)]
pub struct EncodedInput {
    pub input_ids: Vec<i64>,
    pub attention_mask: Vec<i64>,
    pub token_type_ids: Vec<i64>,
}

impl WasmTokenizer {
    /// Create a new tokenizer from JSON configuration
    pub fn from_json(json: &str, max_length: usize) -> Result<Self> {
        let tokenizer = Tokenizer::from_bytes(json.as_bytes())
            .map_err(|e| WasmEmbeddingError::tokenizer(e.to_string()))?;

        Ok(Self {
            tokenizer,
            max_length,
        })
    }

    /// Create tokenizer from raw bytes
    pub fn from_bytes(bytes: &[u8], max_length: usize) -> Result<Self> {
        let tokenizer = Tokenizer::from_bytes(bytes)
            .map_err(|e| WasmEmbeddingError::tokenizer(e.to_string()))?;

        Ok(Self {
            tokenizer,
            max_length,
        })
    }

    /// Encode a single text
    pub fn encode(&self, text: &str) -> Result<EncodedInput> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|e| WasmEmbeddingError::tokenizer(e.to_string()))?;

        let mut input_ids: Vec<i64> = encoding.get_ids().iter().map(|&id| id as i64).collect();
        let mut attention_mask: Vec<i64> =
            encoding.get_attention_mask().iter().map(|&m| m as i64).collect();
        let mut token_type_ids: Vec<i64> =
            encoding.get_type_ids().iter().map(|&t| t as i64).collect();

        // Truncate if necessary
        if input_ids.len() > self.max_length {
            input_ids.truncate(self.max_length);
            attention_mask.truncate(self.max_length);
            token_type_ids.truncate(self.max_length);
        }

        // Pad if necessary
        while input_ids.len() < self.max_length {
            input_ids.push(0);
            attention_mask.push(0);
            token_type_ids.push(0);
        }

        Ok(EncodedInput {
            input_ids,
            attention_mask,
            token_type_ids,
        })
    }

    /// Encode multiple texts with padding to the same length
    pub fn encode_batch(&self, texts: &[&str]) -> Result<Vec<EncodedInput>> {
        texts.iter().map(|text| self.encode(text)).collect()
    }

    /// Get the maximum sequence length
    pub fn max_length(&self) -> usize {
        self.max_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Basic tokenizer JSON for testing
    const TEST_TOKENIZER: &str = r#"{
        "version": "1.0",
        "truncation": null,
        "padding": null,
        "added_tokens": [],
        "normalizer": null,
        "pre_tokenizer": {"type": "Whitespace"},
        "post_processor": null,
        "decoder": null,
        "model": {
            "type": "WordLevel",
            "vocab": {"[PAD]": 0, "[UNK]": 1, "hello": 2, "world": 3},
            "unk_token": "[UNK]"
        }
    }"#;

    #[test]
    fn test_tokenizer_creation() {
        let tokenizer = WasmTokenizer::from_json(TEST_TOKENIZER, 128);
        assert!(tokenizer.is_ok());
    }
}
