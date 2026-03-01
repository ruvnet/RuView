//! Embedding operations for ESP32
//!
//! Provides efficient token embedding lookup and positional encoding.

use heapless::Vec as HVec;

/// Maximum embedding dimension
pub const MAX_EMBED_DIM: usize = 128;
/// Maximum vocabulary size for stack allocation
pub const MAX_VOCAB: usize = 2048;

/// Embedding table with INT8 quantization
pub struct EmbeddingTable<const VOCAB: usize, const DIM: usize> {
    /// Flattened embedding weights [VOCAB * DIM]
    weights: HVec<i8, { 64 * 1024 }>, // Max 64KB
    /// Vocabulary size
    vocab_size: usize,
    /// Embedding dimension
    embed_dim: usize,
    /// Scale factor for dequantization
    scale: f32,
}

impl<const VOCAB: usize, const DIM: usize> EmbeddingTable<VOCAB, DIM> {
    /// Create new embedding table from weights
    pub fn new(weights: &[i8], vocab_size: usize, embed_dim: usize) -> crate::Result<Self> {
        if weights.len() != vocab_size * embed_dim {
            return Err(crate::Error::InvalidModel("Weight size mismatch"));
        }

        let mut table_weights = HVec::new();
        for &w in weights {
            table_weights.push(w).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            weights: table_weights,
            vocab_size,
            embed_dim,
            scale: 1.0 / 127.0,
        })
    }

    /// Create random embedding table for testing
    pub fn random(vocab_size: usize, embed_dim: usize, seed: u32) -> crate::Result<Self> {
        let mut weights = HVec::new();
        let mut rng_state = seed;

        for _ in 0..(vocab_size * embed_dim) {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let val = ((rng_state >> 16) & 0xFF) as i8;
            weights.push(val).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            weights,
            vocab_size,
            embed_dim,
            scale: 1.0 / 127.0,
        })
    }

    /// Look up embedding for a token
    #[inline]
    pub fn lookup(&self, token_id: u16, output: &mut [i8]) -> crate::Result<()> {
        let id = token_id as usize;
        if id >= self.vocab_size {
            return Err(crate::Error::InvalidModel("Token ID out of range"));
        }

        let start = id * self.embed_dim;
        let end = start + self.embed_dim;

        if output.len() < self.embed_dim {
            return Err(crate::Error::BufferOverflow);
        }

        output[..self.embed_dim].copy_from_slice(&self.weights[start..end]);
        Ok(())
    }

    /// Look up embedding and add to existing buffer (for accumulation)
    #[inline]
    pub fn lookup_add(&self, token_id: u16, output: &mut [i32]) -> crate::Result<()> {
        let id = token_id as usize;
        if id >= self.vocab_size {
            return Err(crate::Error::InvalidModel("Token ID out of range"));
        }

        let start = id * self.embed_dim;

        for i in 0..self.embed_dim {
            output[i] += self.weights[start + i] as i32;
        }
        Ok(())
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.weights.len()
    }
}

/// Rotary Position Embedding (RoPE) for ESP32
///
/// Uses fixed-point arithmetic for sin/cos computation.
pub struct RotaryEmbedding {
    /// Dimension (must be even)
    dim: usize,
    /// Base frequency
    base: u32,
    /// Precomputed sin values (fixed-point, scaled by 128)
    sin_cache: [i8; MAX_EMBED_DIM],
    /// Precomputed cos values (fixed-point, scaled by 128)
    cos_cache: [i8; MAX_EMBED_DIM],
    /// Maximum cached position
    max_cached_pos: usize,
}

impl RotaryEmbedding {
    /// Create new RoPE with given dimension
    pub fn new(dim: usize, base: u32) -> Self {
        Self {
            dim,
            base,
            sin_cache: [0i8; MAX_EMBED_DIM],
            cos_cache: [0i8; MAX_EMBED_DIM],
            max_cached_pos: 0,
        }
    }

    /// Update cache for new position
    pub fn update_cache(&mut self, pos: usize) {
        if pos <= self.max_cached_pos {
            return;
        }

        // Compute frequency for each dimension pair
        for i in 0..(self.dim / 2) {
            // freq = 1 / (base^(2i/dim))
            // For INT8, we approximate using lookup table or simple formula

            // Simplified: use position-dependent rotation
            // angle = pos / (base^(i / (dim/2)))
            let freq_scale = ((i * 256) / (self.dim / 2)) as u32;
            let angle = ((pos as u32 * 256) / (self.base + freq_scale)) as i32;

            // Approximate sin/cos using polynomial
            // sin(x) ≈ x - x³/6 for small x (scaled)
            // cos(x) ≈ 1 - x²/2 for small x (scaled)
            let x = (angle % 256) as i32 - 128; // Center around 0

            // Simple quadrant-based approximation
            let sin_val = (x * 127 / 128).clamp(-127, 127) as i8;
            let cos_val = ((128 - x.abs()) * 127 / 128).clamp(-127, 127) as i8;

            self.sin_cache[i] = sin_val;
            self.cos_cache[i] = cos_val;
            self.sin_cache[i + self.dim / 2] = sin_val;
            self.cos_cache[i + self.dim / 2] = cos_val;
        }

        self.max_cached_pos = pos;
    }

    /// Apply rotary embedding to query/key vectors
    #[inline]
    pub fn apply(&self, x: &mut [i8], _pos: usize) {
        let half_dim = self.dim / 2;

        // Process pairs of dimensions
        for i in 0..half_dim {
            let x1 = x[i] as i32;
            let x2 = x[i + half_dim] as i32;

            let sin = self.sin_cache[i] as i32;
            let cos = self.cos_cache[i] as i32;

            // Rotation: [cos, -sin; sin, cos] @ [x1, x2]
            let new_x1 = (x1 * cos - x2 * sin) >> 7;
            let new_x2 = (x1 * sin + x2 * cos) >> 7;

            x[i] = new_x1.clamp(-128, 127) as i8;
            x[i + half_dim] = new_x2.clamp(-128, 127) as i8;
        }
    }
}

/// Simple positional encoding using learned embeddings
pub struct LearnedPositionalEmbedding<const MAX_LEN: usize, const DIM: usize> {
    /// Position embeddings [MAX_LEN * DIM]
    embeddings: HVec<i8, { 8 * 1024 }>, // Max 8KB for positions
    /// Maximum sequence length
    max_len: usize,
    /// Embedding dimension
    dim: usize,
}

impl<const MAX_LEN: usize, const DIM: usize> LearnedPositionalEmbedding<MAX_LEN, DIM> {
    /// Create random positional embeddings
    pub fn random(max_len: usize, dim: usize, seed: u32) -> crate::Result<Self> {
        let mut embeddings = HVec::new();
        let mut rng_state = seed;

        for _ in 0..(max_len * dim) {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            // Smaller values for positional embeddings
            let val = (((rng_state >> 16) & 0x3F) as i8) - 32;
            embeddings.push(val).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self {
            embeddings,
            max_len,
            dim,
        })
    }

    /// Add positional embedding to input
    #[inline]
    pub fn add_to(&self, input: &mut [i8], pos: usize) -> crate::Result<()> {
        if pos >= self.max_len {
            return Err(crate::Error::BufferOverflow);
        }

        let start = pos * self.dim;
        for i in 0..self.dim {
            let sum = input[i] as i32 + self.embeddings[start + i] as i32;
            input[i] = sum.clamp(-128, 127) as i8;
        }
        Ok(())
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.embeddings.len()
    }
}

/// Byte-Pair Encoding tokenizer (simplified)
///
/// For ESP32, we use a simple character-level or small vocabulary tokenizer.
pub struct SimpleTokenizer {
    /// Character to token ID mapping
    char_to_id: [u16; 256],
    /// Token ID to character mapping
    id_to_char: [u8; 256],
    /// Vocabulary size
    vocab_size: usize,
}

impl SimpleTokenizer {
    /// Create ASCII tokenizer (vocabulary = 128)
    pub fn ascii() -> Self {
        let mut char_to_id = [0u16; 256];
        let mut id_to_char = [0u8; 256];

        for i in 0..128 {
            char_to_id[i] = i as u16;
            id_to_char[i] = i as u8;
        }

        // Map non-ASCII to UNK (127)
        for i in 128..256 {
            char_to_id[i] = 127;
        }

        Self {
            char_to_id,
            id_to_char,
            vocab_size: 128,
        }
    }

    /// Tokenize a string
    pub fn encode(&self, text: &str) -> HVec<u16, 128> {
        let mut tokens = HVec::new();
        for byte in text.bytes() {
            let _ = tokens.push(self.char_to_id[byte as usize]);
        }
        tokens
    }

    /// Decode tokens to string
    pub fn decode(&self, tokens: &[u16]) -> HVec<u8, 128> {
        let mut chars = HVec::new();
        for &token in tokens {
            if (token as usize) < self.vocab_size {
                let _ = chars.push(self.id_to_char[token as usize]);
            }
        }
        chars
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_lookup() {
        let embed: EmbeddingTable<256, 64> = EmbeddingTable::random(256, 64, 42).unwrap();

        let mut output = [0i8; 64];
        embed.lookup(10, &mut output).unwrap();

        // Should be non-zero
        assert!(output.iter().any(|&x| x != 0));
    }

    #[test]
    fn test_rotary_embedding() {
        let mut rope = RotaryEmbedding::new(32, 10000);
        rope.update_cache(10);

        let mut x = [64i8; 32];
        rope.apply(&mut x, 5);

        // Values should change after rotation
        assert!(x.iter().any(|&v| v != 64));
    }

    #[test]
    fn test_tokenizer() {
        let tokenizer = SimpleTokenizer::ascii();

        let tokens = tokenizer.encode("Hello");
        assert_eq!(tokens.len(), 5);

        let decoded = tokenizer.decode(&tokens);
        assert_eq!(&decoded[..], b"Hello");
    }
}
