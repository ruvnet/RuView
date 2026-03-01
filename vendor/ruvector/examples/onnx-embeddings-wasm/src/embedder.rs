//! Main WASM embedder implementation

use crate::error::{Result, WasmEmbeddingError};
use crate::model::TractModel;
use crate::pooling::{cosine_similarity, normalize_l2, PoolingStrategy};
use crate::tokenizer::WasmTokenizer;
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// Configuration for the WASM embedder
#[wasm_bindgen]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmEmbedderConfig {
    /// Maximum sequence length
    #[wasm_bindgen(skip)]
    pub max_length: usize,
    /// Pooling strategy
    #[wasm_bindgen(skip)]
    pub pooling: PoolingStrategy,
    /// Whether to L2 normalize embeddings
    #[wasm_bindgen(skip)]
    pub normalize: bool,
}

#[wasm_bindgen]
impl WasmEmbedderConfig {
    /// Create a new configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum sequence length
    #[wasm_bindgen(js_name = setMaxLength)]
    pub fn set_max_length(mut self, max_length: usize) -> Self {
        self.max_length = max_length;
        self
    }

    /// Set whether to normalize embeddings
    #[wasm_bindgen(js_name = setNormalize)]
    pub fn set_normalize(mut self, normalize: bool) -> Self {
        self.normalize = normalize;
        self
    }

    /// Set pooling strategy (0=Mean, 1=Cls, 2=Max, 3=MeanSqrtLen, 4=LastToken)
    #[wasm_bindgen(js_name = setPooling)]
    pub fn set_pooling(mut self, pooling: u8) -> Self {
        self.pooling = match pooling {
            0 => PoolingStrategy::Mean,
            1 => PoolingStrategy::Cls,
            2 => PoolingStrategy::Max,
            3 => PoolingStrategy::MeanSqrtLen,
            4 => PoolingStrategy::LastToken,
            _ => PoolingStrategy::Mean,
        };
        self
    }
}

impl Default for WasmEmbedderConfig {
    fn default() -> Self {
        Self {
            max_length: 256,
            pooling: PoolingStrategy::Mean,
            normalize: true,
        }
    }
}

/// WASM-compatible embedder using Tract for inference
#[wasm_bindgen]
pub struct WasmEmbedder {
    model: TractModel,
    tokenizer: WasmTokenizer,
    config: WasmEmbedderConfig,
    hidden_size: usize,
}

#[wasm_bindgen]
impl WasmEmbedder {
    /// Create a new embedder from model and tokenizer bytes
    ///
    /// # Arguments
    /// * `model_bytes` - ONNX model file bytes
    /// * `tokenizer_json` - Tokenizer JSON configuration
    #[wasm_bindgen(constructor)]
    pub fn new(model_bytes: &[u8], tokenizer_json: &str) -> std::result::Result<WasmEmbedder, JsValue> {
        Self::with_config(model_bytes, tokenizer_json, WasmEmbedderConfig::default())
    }

    /// Create embedder with custom configuration
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config(
        model_bytes: &[u8],
        tokenizer_json: &str,
        config: WasmEmbedderConfig,
    ) -> std::result::Result<WasmEmbedder, JsValue> {
        let model = TractModel::from_bytes(model_bytes, config.max_length)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let tokenizer = WasmTokenizer::from_json(tokenizer_json, config.max_length)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let hidden_size = model.hidden_size();

        Ok(Self {
            model,
            tokenizer,
            config,
            hidden_size,
        })
    }

    /// Generate embedding for a single text
    #[wasm_bindgen(js_name = embedOne)]
    pub fn embed_one(&mut self, text: &str) -> std::result::Result<Vec<f32>, JsValue> {
        self.embed_one_internal(text)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Generate embeddings for multiple texts
    #[wasm_bindgen(js_name = embedBatch)]
    pub fn embed_batch(&mut self, texts: Vec<String>) -> std::result::Result<Vec<f32>, JsValue> {
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        self.embed_batch_internal(&refs)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Compute similarity between two texts
    #[wasm_bindgen]
    pub fn similarity(&mut self, text1: &str, text2: &str) -> std::result::Result<f32, JsValue> {
        let emb1 = self.embed_one_internal(text1)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
        let emb2 = self.embed_one_internal(text2)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        Ok(cosine_similarity(&emb1, &emb2))
    }

    /// Get the embedding dimension
    #[wasm_bindgen]
    pub fn dimension(&self) -> usize {
        self.hidden_size
    }

    /// Get maximum sequence length
    #[wasm_bindgen(js_name = maxLength)]
    pub fn max_length(&self) -> usize {
        self.config.max_length
    }
}

// Internal implementation
impl WasmEmbedder {
    fn embed_one_internal(&mut self, text: &str) -> Result<Vec<f32>> {
        // Tokenize
        let encoded = self.tokenizer.encode(text)?;
        let attention_mask = encoded.attention_mask.clone();

        // Run inference
        let raw_output = self.model.run(&encoded)?;

        // Determine hidden size from output
        let seq_len = self.config.max_length;
        if raw_output.len() >= seq_len {
            let detected_hidden = raw_output.len() / seq_len;
            if detected_hidden != self.hidden_size && detected_hidden > 0 {
                self.hidden_size = detected_hidden;
                self.model.set_hidden_size(detected_hidden);
            }
        }

        // Apply pooling
        let mut embedding = self.config.pooling.apply(
            &raw_output,
            &attention_mask,
            self.hidden_size,
        );

        // Normalize if configured
        if self.config.normalize {
            normalize_l2(&mut embedding);
        }

        Ok(embedding)
    }

    fn embed_batch_internal(&mut self, texts: &[&str]) -> Result<Vec<f32>> {
        let mut all_embeddings = Vec::with_capacity(texts.len() * self.hidden_size);

        for text in texts {
            let embedding = self.embed_one_internal(text)?;
            all_embeddings.extend(embedding);
        }

        Ok(all_embeddings)
    }
}

/// Compute cosine similarity between two embedding vectors (JS-friendly)
#[wasm_bindgen(js_name = cosineSimilarity)]
pub fn js_cosine_similarity(a: Vec<f32>, b: Vec<f32>) -> f32 {
    cosine_similarity(&a, &b)
}

/// L2 normalize an embedding vector (JS-friendly)
#[wasm_bindgen(js_name = normalizeL2)]
pub fn js_normalize_l2(mut embedding: Vec<f32>) -> Vec<f32> {
    normalize_l2(&mut embedding);
    embedding
}
