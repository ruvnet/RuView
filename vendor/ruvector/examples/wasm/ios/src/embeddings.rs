//! Content Embedding Module for iOS WASM
//!
//! Lightweight embedding generation for content recommendations.
//! Optimized for minimal binary size and sub-100ms latency on iPhone 12+.

/// Maximum embedding dimensions (memory budget constraint)
pub const MAX_EMBEDDING_DIM: usize = 256;

/// Default embedding dimension for content
pub const DEFAULT_DIM: usize = 64;

/// Content metadata for embedding generation
#[derive(Clone, Debug)]
pub struct ContentMetadata {
    /// Content identifier
    pub id: u64,
    /// Content type (0=video, 1=audio, 2=image, 3=text)
    pub content_type: u8,
    /// Duration in seconds (for video/audio)
    pub duration_secs: u32,
    /// Category tags (bit flags)
    pub category_flags: u32,
    /// Popularity score (0.0 - 1.0)
    pub popularity: f32,
    /// Recency score (0.0 - 1.0)
    pub recency: f32,
}

impl Default for ContentMetadata {
    fn default() -> Self {
        Self {
            id: 0,
            content_type: 0,
            duration_secs: 0,
            category_flags: 0,
            popularity: 0.5,
            recency: 0.5,
        }
    }
}

/// Lightweight content embedder optimized for iOS
pub struct ContentEmbedder {
    dim: usize,
    // Pre-computed projection weights (random but deterministic)
    projection: Vec<f32>,
}

impl ContentEmbedder {
    /// Create a new embedder with specified dimension
    pub fn new(dim: usize) -> Self {
        let dim = dim.min(MAX_EMBEDDING_DIM);

        // Initialize deterministic pseudo-random projection
        // Using simple LCG for reproducibility without rand crate
        let mut projection = Vec::with_capacity(dim * 8);
        let mut seed: u32 = 12345;

        for _ in 0..(dim * 8) {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let val = ((seed >> 16) as f32 / 32768.0) - 1.0;
            projection.push(val * 0.1); // Scale factor
        }

        Self { dim, projection }
    }

    /// Embed content metadata into a vector
    #[inline]
    pub fn embed(&self, content: &ContentMetadata) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.dim];

        // Feature extraction with projection
        let features = [
            content.content_type as f32 / 4.0,
            (content.duration_secs as f32).ln_1p() / 10.0,
            (content.category_flags as f32).sqrt() / 64.0,
            content.popularity,
            content.recency,
            content.id as f32 % 1000.0 / 1000.0,
            ((content.id >> 10) as f32 % 1000.0) / 1000.0,
            ((content.id >> 20) as f32 % 1000.0) / 1000.0,
        ];

        // Project features to embedding space
        for (i, e) in embedding.iter_mut().enumerate() {
            for (j, &feat) in features.iter().enumerate() {
                let proj_idx = i * 8 + j;
                if proj_idx < self.projection.len() {
                    *e += feat * self.projection[proj_idx];
                }
            }
        }

        // L2 normalize
        self.normalize(&mut embedding);

        embedding
    }

    /// Embed raw feature vector
    #[inline]
    pub fn embed_features(&self, features: &[f32]) -> Vec<f32> {
        let mut embedding = vec![0.0f32; self.dim];

        for (i, e) in embedding.iter_mut().enumerate() {
            for (j, &feat) in features.iter().take(8).enumerate() {
                let proj_idx = i * 8 + j;
                if proj_idx < self.projection.len() {
                    *e += feat * self.projection[proj_idx];
                }
            }
        }

        self.normalize(&mut embedding);
        embedding
    }

    /// L2 normalize a vector in place
    #[inline]
    fn normalize(&self, vec: &mut [f32]) {
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for x in vec.iter_mut() {
                *x /= norm;
            }
        }
    }

    /// Compute cosine similarity between two embeddings
    #[inline]
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() {
            return 0.0;
        }

        a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
    }

    /// Get embedding dimension
    pub fn dim(&self) -> usize {
        self.dim
    }
}

/// User vibe/preference state for personalized recommendations
#[derive(Clone, Debug, Default)]
pub struct VibeState {
    /// Energy level (0.0 = calm, 1.0 = energetic)
    pub energy: f32,
    /// Mood valence (-1.0 = negative, 1.0 = positive)
    pub mood: f32,
    /// Focus level (0.0 = relaxed, 1.0 = focused)
    pub focus: f32,
    /// Time of day preference (0.0 = morning, 1.0 = night)
    pub time_context: f32,
    /// Custom preference weights
    pub preferences: [f32; 4],
}

impl VibeState {
    /// Convert vibe state to embedding
    pub fn to_embedding(&self, embedder: &ContentEmbedder) -> Vec<f32> {
        let features = [
            self.energy,
            (self.mood + 1.0) / 2.0, // Normalize to 0-1
            self.focus,
            self.time_context,
            self.preferences[0],
            self.preferences[1],
            self.preferences[2],
            self.preferences[3],
        ];

        embedder.embed_features(&features)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedder_creation() {
        let embedder = ContentEmbedder::new(64);
        assert_eq!(embedder.dim(), 64);
    }

    #[test]
    fn test_embedding_normalized() {
        let embedder = ContentEmbedder::new(64);
        let content = ContentMetadata::default();
        let embedding = embedder.embed(&content);

        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_similarity_range() {
        let embedder = ContentEmbedder::new(64);

        let c1 = ContentMetadata { id: 1, ..Default::default() };
        let c2 = ContentMetadata { id: 2, ..Default::default() };

        let e1 = embedder.embed(&c1);
        let e2 = embedder.embed(&c2);

        let sim = ContentEmbedder::similarity(&e1, &e2);
        assert!(sim >= -1.0 && sim <= 1.0);
    }
}
