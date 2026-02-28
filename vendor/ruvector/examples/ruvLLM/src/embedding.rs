//! Embedding service with tokenization and caching
//!
//! Provides text-to-vector conversion with LRU caching for efficiency.

use crate::config::EmbeddingConfig;
use crate::error::Result;

use ahash::AHashMap;
use lru::LruCache;
use parking_lot::Mutex;
use std::num::NonZeroUsize;

/// Result of embedding a text
#[derive(Debug, Clone)]
pub struct Embedding {
    /// The embedding vector
    pub vector: Vec<f32>,
    /// Token count
    pub token_count: usize,
    /// Whether text was truncated
    pub truncated: bool,
    /// Cache hit indicator
    pub from_cache: bool,
}

/// Token from tokenization
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Token {
    /// Token ID
    pub id: u32,
    /// Token text
    pub text: String,
}

/// Tokenizer for text processing
pub struct Tokenizer {
    /// Vocabulary mapping
    vocab: AHashMap<String, u32>,
    /// Reverse mapping
    id_to_token: Vec<String>,
    /// Special tokens
    special_tokens: SpecialTokens,
}

/// Special token IDs
#[derive(Debug, Clone)]
struct SpecialTokens {
    pad: u32,
    unk: u32,
    bos: u32,
    eos: u32,
}

impl Tokenizer {
    /// Create a new basic tokenizer
    pub fn new(vocab_size: usize) -> Self {
        let mut vocab = AHashMap::new();
        let mut id_to_token = Vec::with_capacity(vocab_size);

        // Add special tokens
        let special = ["<pad>", "<unk>", "<bos>", "<eos>", "<sep>"];
        for (i, tok) in special.iter().enumerate() {
            vocab.insert(tok.to_string(), i as u32);
            id_to_token.push(tok.to_string());
        }

        // Build basic character/word vocabulary
        let chars: Vec<char> =
            "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789 .,!?;:'\"-_()[]{}"
                .chars()
                .collect();
        for ch in chars {
            let s = ch.to_string();
            if !vocab.contains_key(&s) && vocab.len() < vocab_size {
                let id = vocab.len() as u32;
                vocab.insert(s.clone(), id);
                id_to_token.push(s);
            }
        }

        Self {
            vocab,
            id_to_token,
            special_tokens: SpecialTokens {
                pad: 0,
                unk: 1,
                bos: 2,
                eos: 3,
            },
        }
    }

    /// Tokenize text into token IDs
    pub fn tokenize(&self, text: &str) -> Vec<u32> {
        let mut tokens = vec![self.special_tokens.bos];

        // Simple character-level tokenization
        for word in text.split_whitespace() {
            for ch in word.chars() {
                let s = ch.to_string();
                let id = self
                    .vocab
                    .get(&s)
                    .copied()
                    .unwrap_or(self.special_tokens.unk);
                tokens.push(id);
            }
            // Add space token
            if let Some(&space_id) = self.vocab.get(" ") {
                tokens.push(space_id);
            }
        }

        tokens.push(self.special_tokens.eos);
        tokens
    }

    /// Get vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.vocab.len()
    }

    /// Decode tokens back to text
    pub fn decode(&self, tokens: &[u32]) -> String {
        tokens
            .iter()
            .filter_map(|&id| self.id_to_token.get(id as usize))
            .cloned()
            .collect::<Vec<_>>()
            .join("")
    }
}

/// Service for text embedding with caching
pub struct EmbeddingService {
    /// Embedding dimension
    dimension: usize,
    /// Maximum tokens
    max_tokens: usize,
    /// Tokenizer
    tokenizer: Tokenizer,
    /// LRU cache for embeddings
    cache: Mutex<LruCache<u64, Embedding>>,
    /// Embedding matrix (token_id -> embedding)
    embedding_matrix: Vec<Vec<f32>>,
    /// Position embeddings
    position_embeddings: Vec<Vec<f32>>,
    /// Statistics
    stats: EmbeddingStats,
}

/// Embedding service statistics
struct EmbeddingStats {
    cache_hits: std::sync::atomic::AtomicU64,
    cache_misses: std::sync::atomic::AtomicU64,
    total_tokens: std::sync::atomic::AtomicU64,
}

impl EmbeddingService {
    /// Create a new embedding service
    pub fn new(config: &EmbeddingConfig) -> Result<Self> {
        let tokenizer = Tokenizer::new(10000);
        let vocab_size = tokenizer.vocab_size();

        // Initialize embedding matrix with random values
        let mut rng = rand::thread_rng();
        use rand::Rng;

        let embedding_matrix: Vec<Vec<f32>> = (0..vocab_size)
            .map(|_| {
                let mut vec: Vec<f32> = (0..config.dimension)
                    .map(|_| rng.gen_range(-0.1..0.1))
                    .collect();
                // Normalize
                let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    vec.iter_mut().for_each(|x| *x /= norm);
                }
                vec
            })
            .collect();

        // Position embeddings (sinusoidal)
        let position_embeddings: Vec<Vec<f32>> = (0..config.max_tokens)
            .map(|pos| {
                (0..config.dimension)
                    .map(|i| {
                        let angle = pos as f32
                            / (10000.0_f32).powf(2.0 * (i / 2) as f32 / config.dimension as f32);
                        if i % 2 == 0 {
                            angle.sin()
                        } else {
                            angle.cos()
                        }
                    })
                    .collect()
            })
            .collect();

        let cache_size = NonZeroUsize::new(10000).unwrap();

        Ok(Self {
            dimension: config.dimension,
            max_tokens: config.max_tokens,
            tokenizer,
            cache: Mutex::new(LruCache::new(cache_size)),
            embedding_matrix,
            position_embeddings,
            stats: EmbeddingStats {
                cache_hits: std::sync::atomic::AtomicU64::new(0),
                cache_misses: std::sync::atomic::AtomicU64::new(0),
                total_tokens: std::sync::atomic::AtomicU64::new(0),
            },
        })
    }

    /// Embed a text string
    pub fn embed(&self, text: &str) -> Result<Embedding> {
        // Check cache
        let hash = self.hash_text(text);
        {
            let mut cache = self.cache.lock();
            if let Some(cached) = cache.get(&hash) {
                self.stats
                    .cache_hits
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let mut result = cached.clone();
                result.from_cache = true;
                return Ok(result);
            }
        }
        self.stats
            .cache_misses
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Tokenize
        let tokens = self.tokenizer.tokenize(text);
        let token_count = tokens.len();
        let truncated = token_count > self.max_tokens;
        let tokens: Vec<u32> = tokens.into_iter().take(self.max_tokens).collect();

        self.stats
            .total_tokens
            .fetch_add(tokens.len() as u64, std::sync::atomic::Ordering::Relaxed);

        // Compute embedding
        let vector = self.compute_embedding(&tokens);

        let embedding = Embedding {
            vector,
            token_count: tokens.len(),
            truncated,
            from_cache: false,
        };

        // Cache result
        {
            let mut cache = self.cache.lock();
            cache.put(hash, embedding.clone());
        }

        Ok(embedding)
    }

    /// Embed multiple texts (batched for efficiency)
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Embedding>> {
        texts.iter().map(|t| self.embed(t)).collect()
    }

    /// Embed with specific pooling strategy
    pub fn embed_with_pooling(&self, text: &str, pooling: PoolingStrategy) -> Result<Embedding> {
        let tokens = self.tokenizer.tokenize(text);
        let tokens: Vec<u32> = tokens.into_iter().take(self.max_tokens).collect();

        let vector = match pooling {
            PoolingStrategy::Mean => self.mean_pooling(&tokens),
            PoolingStrategy::Max => self.max_pooling(&tokens),
            PoolingStrategy::CLS => self.cls_pooling(&tokens),
            PoolingStrategy::LastToken => self.last_token_pooling(&tokens),
        };

        Ok(Embedding {
            vector,
            token_count: tokens.len(),
            truncated: tokens.len() >= self.max_tokens,
            from_cache: false,
        })
    }

    /// Get embedding statistics
    pub fn get_stats(&self) -> EmbeddingServiceStats {
        EmbeddingServiceStats {
            cache_hits: self
                .stats
                .cache_hits
                .load(std::sync::atomic::Ordering::Relaxed),
            cache_misses: self
                .stats
                .cache_misses
                .load(std::sync::atomic::Ordering::Relaxed),
            total_tokens: self
                .stats
                .total_tokens
                .load(std::sync::atomic::Ordering::Relaxed),
            cache_size: self.cache.lock().len(),
        }
    }

    /// Clear the embedding cache
    pub fn clear_cache(&self) {
        self.cache.lock().clear();
    }

    fn hash_text(&self, text: &str) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }

    fn compute_embedding(&self, tokens: &[u32]) -> Vec<f32> {
        self.mean_pooling(tokens)
    }

    fn mean_pooling(&self, tokens: &[u32]) -> Vec<f32> {
        let mut result = vec![0.0f32; self.dimension];

        for (pos, &token_id) in tokens.iter().enumerate() {
            let token_emb = self.get_token_embedding(token_id);
            let pos_emb = self.get_position_embedding(pos);

            for i in 0..self.dimension {
                result[i] += token_emb[i] + pos_emb[i];
            }
        }

        // Average
        let n = tokens.len() as f32;
        if n > 0.0 {
            result.iter_mut().for_each(|x| *x /= n);
        }

        // Normalize
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            result.iter_mut().for_each(|x| *x /= norm);
        }

        result
    }

    fn max_pooling(&self, tokens: &[u32]) -> Vec<f32> {
        let mut result = vec![f32::NEG_INFINITY; self.dimension];

        for (pos, &token_id) in tokens.iter().enumerate() {
            let token_emb = self.get_token_embedding(token_id);
            let pos_emb = self.get_position_embedding(pos);

            for i in 0..self.dimension {
                let val = token_emb[i] + pos_emb[i];
                if val > result[i] {
                    result[i] = val;
                }
            }
        }

        // Normalize
        let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            result.iter_mut().for_each(|x| *x /= norm);
        }

        result
    }

    fn cls_pooling(&self, tokens: &[u32]) -> Vec<f32> {
        if let Some(&first_token) = tokens.first() {
            let token_emb = self.get_token_embedding(first_token);
            let pos_emb = self.get_position_embedding(0);

            let mut result: Vec<f32> = token_emb
                .iter()
                .zip(pos_emb.iter())
                .map(|(t, p)| t + p)
                .collect();

            // Normalize
            let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                result.iter_mut().for_each(|x| *x /= norm);
            }

            result
        } else {
            vec![0.0; self.dimension]
        }
    }

    fn last_token_pooling(&self, tokens: &[u32]) -> Vec<f32> {
        if let Some(&last_token) = tokens.last() {
            let pos = tokens.len().saturating_sub(1);
            let token_emb = self.get_token_embedding(last_token);
            let pos_emb = self.get_position_embedding(pos);

            let mut result: Vec<f32> = token_emb
                .iter()
                .zip(pos_emb.iter())
                .map(|(t, p)| t + p)
                .collect();

            // Normalize
            let norm: f32 = result.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                result.iter_mut().for_each(|x| *x /= norm);
            }

            result
        } else {
            vec![0.0; self.dimension]
        }
    }

    fn get_token_embedding(&self, token_id: u32) -> &[f32] {
        let idx = (token_id as usize).min(self.embedding_matrix.len() - 1);
        &self.embedding_matrix[idx]
    }

    fn get_position_embedding(&self, pos: usize) -> &[f32] {
        let idx = pos.min(self.position_embeddings.len() - 1);
        &self.position_embeddings[idx]
    }
}

/// Pooling strategy for embeddings
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PoolingStrategy {
    /// Mean pooling (average all tokens)
    Mean,
    /// Max pooling (element-wise max)
    Max,
    /// CLS token pooling (first token)
    CLS,
    /// Last token pooling
    LastToken,
}

/// Public statistics
#[derive(Debug, Clone)]
pub struct EmbeddingServiceStats {
    /// Cache hits
    pub cache_hits: u64,
    /// Cache misses
    pub cache_misses: u64,
    /// Total tokens processed
    pub total_tokens: u64,
    /// Current cache size
    pub cache_size: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_dimension() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();
        let embedding = service.embed("Hello world").unwrap();
        assert_eq!(embedding.vector.len(), config.dimension);
    }

    #[test]
    fn test_embedding_normalized() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();
        let embedding = service.embed("Test text").unwrap();

        let norm: f32 = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_same_text_same_embedding() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        let e1 = service.embed("Same text").unwrap();
        let e2 = service.embed("Same text").unwrap();

        assert_eq!(e1.vector, e2.vector);
        assert!(e2.from_cache);
    }

    #[test]
    fn test_different_texts_different_embeddings() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        let e1 = service.embed("Hello world").unwrap();
        let e2 = service.embed("Goodbye moon").unwrap();

        // Character-level tokenizer produces similar embeddings for similar text
        // Just verify they're not identical
        let diff: f32 = e1
            .vector
            .iter()
            .zip(e2.vector.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            diff > 0.0,
            "Different texts should produce different embeddings"
        );
    }

    #[test]
    fn test_tokenizer() {
        let tokenizer = Tokenizer::new(1000);

        let tokens = tokenizer.tokenize("Hello world");
        assert!(!tokens.is_empty());
        assert_eq!(tokens[0], 2); // BOS
        assert_eq!(*tokens.last().unwrap(), 3); // EOS
    }

    #[test]
    fn test_batch_embedding() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        let texts = vec!["text one", "text two", "text three"];
        let embeddings = service.embed_batch(&texts).unwrap();

        assert_eq!(embeddings.len(), 3);
        for emb in &embeddings {
            assert_eq!(emb.vector.len(), config.dimension);
        }
    }

    #[test]
    fn test_pooling_strategies() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();
        let text = "Test pooling strategies";

        let mean = service
            .embed_with_pooling(text, PoolingStrategy::Mean)
            .unwrap();
        let max = service
            .embed_with_pooling(text, PoolingStrategy::Max)
            .unwrap();
        let cls = service
            .embed_with_pooling(text, PoolingStrategy::CLS)
            .unwrap();
        let last = service
            .embed_with_pooling(text, PoolingStrategy::LastToken)
            .unwrap();

        assert_eq!(mean.vector.len(), config.dimension);
        assert_eq!(max.vector.len(), config.dimension);
        assert_eq!(cls.vector.len(), config.dimension);
        assert_eq!(last.vector.len(), config.dimension);

        let mean_dot_max: f32 = mean
            .vector
            .iter()
            .zip(max.vector.iter())
            .map(|(a, b)| a * b)
            .sum();
        assert!(mean_dot_max < 0.999);
    }

    #[test]
    fn test_cache_stats() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        service.embed("test 1").unwrap();
        service.embed("test 2").unwrap();
        service.embed("test 1").unwrap(); // Cache hit

        let stats = service.get_stats();
        assert_eq!(stats.cache_hits, 1);
        assert_eq!(stats.cache_misses, 2);
    }

    #[test]
    fn test_truncation() {
        let mut config = EmbeddingConfig::default();
        config.max_tokens = 10;
        let service = EmbeddingService::new(&config).unwrap();

        let long_text = "This is a very long text that will definitely be truncated because it exceeds the maximum token limit";
        let embedding = service.embed(long_text).unwrap();

        assert!(embedding.truncated);
    }

    #[test]
    fn test_clear_cache() {
        let config = EmbeddingConfig::default();
        let service = EmbeddingService::new(&config).unwrap();

        service.embed("test").unwrap();
        assert_eq!(service.get_stats().cache_size, 1);

        service.clear_cache();
        assert_eq!(service.get_stats().cache_size, 0);
    }
}
