//! Text tokenization using HuggingFace tokenizers

use crate::{EmbeddingError, Result};
use std::path::Path;
use tokenizers::tokenizer::Tokenizer as HfTokenizer;
use tracing::{debug, instrument};

/// Wrapper around HuggingFace tokenizer with batch processing
pub struct Tokenizer {
    inner: HfTokenizer,
    max_length: usize,
    pad_token_id: u32,
}

/// Encoded batch output
#[derive(Debug, Clone)]
pub struct EncodedBatch {
    /// Token IDs [batch_size, seq_length]
    pub input_ids: Vec<Vec<i64>>,
    /// Attention mask [batch_size, seq_length]
    pub attention_mask: Vec<Vec<i64>>,
    /// Token type IDs [batch_size, seq_length]
    pub token_type_ids: Vec<Vec<i64>>,
    /// Original sequence lengths before padding
    pub original_lengths: Vec<usize>,
}

impl EncodedBatch {
    /// Get batch size
    pub fn batch_size(&self) -> usize {
        self.input_ids.len()
    }

    /// Get sequence length (padded)
    pub fn seq_length(&self) -> usize {
        self.input_ids.first().map(|v| v.len()).unwrap_or(0)
    }

    /// Convert to flat arrays for ONNX input
    pub fn to_onnx_inputs(&self) -> (Vec<i64>, Vec<i64>, Vec<i64>, Vec<usize>) {
        let batch_size = self.batch_size();
        let seq_length = self.seq_length();
        let total_len = batch_size * seq_length;

        let mut flat_input_ids = Vec::with_capacity(total_len);
        let mut flat_attention_mask = Vec::with_capacity(total_len);
        let mut flat_token_type_ids = Vec::with_capacity(total_len);

        for i in 0..batch_size {
            flat_input_ids.extend(&self.input_ids[i]);
            flat_attention_mask.extend(&self.attention_mask[i]);
            flat_token_type_ids.extend(&self.token_type_ids[i]);
        }

        (
            flat_input_ids,
            flat_attention_mask,
            flat_token_type_ids,
            vec![batch_size, seq_length],
        )
    }
}

/// Helper function to find pad token ID from vocabulary
fn find_pad_token_id(tokenizer: &HfTokenizer) -> u32 {
    let vocab = tokenizer.get_vocab(true);
    vocab
        .get("[PAD]")
        .or_else(|| vocab.get("<pad>"))
        .or_else(|| vocab.get("<|pad|>"))
        .copied()
        .unwrap_or(0)
}

impl Tokenizer {
    /// Load tokenizer from a local file
    #[instrument(skip_all, fields(path = %path.as_ref().display()))]
    pub fn from_file(path: impl AsRef<Path>, max_length: usize) -> Result<Self> {
        let path = path.as_ref();
        debug!("Loading tokenizer from file");

        let inner = HfTokenizer::from_file(path)
            .map_err(|e| EmbeddingError::tokenizer_not_found(e.to_string()))?;

        let pad_token_id = find_pad_token_id(&inner);

        Ok(Self {
            inner,
            max_length,
            pad_token_id,
        })
    }

    /// Load tokenizer from HuggingFace Hub by downloading tokenizer.json
    #[instrument(skip_all, fields(model_id = %model_id))]
    pub fn from_pretrained(model_id: &str, max_length: usize) -> Result<Self> {
        debug!("Loading tokenizer from HuggingFace Hub: {}", model_id);

        // Download tokenizer.json from HuggingFace Hub
        let url = format!(
            "https://huggingface.co/{}/resolve/main/tokenizer.json",
            model_id
        );

        let response = reqwest::blocking::get(&url)
            .map_err(|e| EmbeddingError::download_failed(format!("Failed to download tokenizer: {}", e)))?;

        if !response.status().is_success() {
            return Err(EmbeddingError::download_failed(format!(
                "Failed to download tokenizer from {}: HTTP {}",
                url,
                response.status()
            )));
        }

        let bytes = response.bytes()
            .map_err(|e| EmbeddingError::download_failed(e.to_string()))?;

        let inner = HfTokenizer::from_bytes(&bytes)
            .map_err(|e| EmbeddingError::tokenizer_not_found(e.to_string()))?;

        let pad_token_id = find_pad_token_id(&inner);

        Ok(Self {
            inner,
            max_length,
            pad_token_id,
        })
    }

    /// Load tokenizer from JSON string
    pub fn from_json(json: &str, max_length: usize) -> Result<Self> {
        let inner = HfTokenizer::from_bytes(json.as_bytes())
            .map_err(|e| EmbeddingError::tokenizer_not_found(e.to_string()))?;

        let pad_token_id = find_pad_token_id(&inner);

        Ok(Self {
            inner,
            max_length,
            pad_token_id,
        })
    }

    /// Encode a single text
    pub fn encode(&self, text: &str) -> Result<EncodedBatch> {
        self.encode_batch(&[text])
    }

    /// Encode a batch of texts
    #[instrument(skip_all, fields(batch_size = texts.len()))]
    pub fn encode_batch<S: AsRef<str>>(&self, texts: &[S]) -> Result<EncodedBatch> {
        if texts.is_empty() {
            return Err(EmbeddingError::EmptyInput);
        }

        debug!("Encoding batch of {} texts", texts.len());

        // Encode all texts
        let encodings: Vec<_> = texts
            .iter()
            .map(|t| self.inner.encode(t.as_ref(), true))
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(EmbeddingError::from)?;

        // Find max length in batch (capped at max_length)
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len().min(self.max_length))
            .max()
            .unwrap_or(0);

        // Pad all sequences to the same length
        let mut input_ids = Vec::with_capacity(texts.len());
        let mut attention_mask = Vec::with_capacity(texts.len());
        let mut token_type_ids = Vec::with_capacity(texts.len());
        let mut original_lengths = Vec::with_capacity(texts.len());

        for encoding in &encodings {
            let ids = encoding.get_ids();
            let type_ids = encoding.get_type_ids();
            let len = ids.len().min(self.max_length);

            original_lengths.push(len);

            // Truncate if necessary and convert to i64
            let mut ids_vec: Vec<i64> = ids[..len].iter().map(|&x| x as i64).collect();
            let mut mask_vec: Vec<i64> = vec![1; len];
            let mut type_vec: Vec<i64> = type_ids[..len].iter().map(|&x| x as i64).collect();

            // Pad to max_len
            let pad_len = max_len - len;
            if pad_len > 0 {
                ids_vec.extend(std::iter::repeat_n(self.pad_token_id as i64, pad_len));
                mask_vec.extend(std::iter::repeat_n(0i64, pad_len));
                type_vec.extend(std::iter::repeat_n(0i64, pad_len));
            }

            input_ids.push(ids_vec);
            attention_mask.push(mask_vec);
            token_type_ids.push(type_vec);
        }

        Ok(EncodedBatch {
            input_ids,
            attention_mask,
            token_type_ids,
            original_lengths,
        })
    }

    /// Get the vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.inner.get_vocab_size(true)
    }

    /// Get the max length
    pub fn max_length(&self) -> usize {
        self.max_length
    }

    /// Set the max length
    pub fn set_max_length(&mut self, max_length: usize) {
        self.max_length = max_length;
    }

    /// Decode token IDs back to text
    pub fn decode(&self, ids: &[u32], skip_special_tokens: bool) -> Result<String> {
        self.inner
            .decode(ids, skip_special_tokens)
            .map_err(EmbeddingError::from)
    }

    /// Get the pad token ID
    pub fn pad_token_id(&self) -> u32 {
        self.pad_token_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoded_batch_to_onnx() {
        let batch = EncodedBatch {
            input_ids: vec![vec![101, 2054, 2003, 102], vec![101, 2054, 102, 0]],
            attention_mask: vec![vec![1, 1, 1, 1], vec![1, 1, 1, 0]],
            token_type_ids: vec![vec![0, 0, 0, 0], vec![0, 0, 0, 0]],
            original_lengths: vec![4, 3],
        };

        let (ids, mask, types, shape) = batch.to_onnx_inputs();

        assert_eq!(shape, vec![2, 4]);
        assert_eq!(ids.len(), 8);
        assert_eq!(mask.len(), 8);
        assert_eq!(types.len(), 8);
    }
}
