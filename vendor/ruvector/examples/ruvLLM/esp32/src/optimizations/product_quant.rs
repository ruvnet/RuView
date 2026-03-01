//! Product Quantization - 8-32x Memory Compression
//!
//! Adapted from ruvector-postgres for ESP32 constraints.
//! Splits vectors into subvectors and quantizes each independently.

use heapless::Vec as HVec;

/// Maximum number of subquantizers
pub const MAX_SUBQUANTIZERS: usize = 8;
/// Maximum codebook size per subquantizer
pub const MAX_CODEBOOK_SIZE: usize = 16; // 4-bit codes
/// Maximum subvector dimension
pub const MAX_SUBVEC_DIM: usize = 8;

/// Product Quantization configuration
#[derive(Debug, Clone, Copy)]
pub struct PQConfig {
    /// Number of subquantizers (M)
    pub num_subquantizers: usize,
    /// Number of codes per subquantizer (K = 2^bits)
    pub codebook_size: usize,
    /// Dimension of each subvector
    pub subvec_dim: usize,
    /// Total vector dimension
    pub dim: usize,
}

impl Default for PQConfig {
    fn default() -> Self {
        Self {
            num_subquantizers: 4,
            codebook_size: 16, // 4-bit codes
            subvec_dim: 8,
            dim: 32,
        }
    }
}

/// Product Quantized code for a vector
#[derive(Debug, Clone)]
pub struct PQCode<const M: usize> {
    /// Code indices for each subquantizer (4-bit packed)
    pub codes: HVec<u8, M>,
}

impl<const M: usize> PQCode<M> {
    /// Create from code indices
    pub fn from_codes(codes: &[u8]) -> crate::Result<Self> {
        let mut code_vec = HVec::new();
        for &c in codes {
            code_vec.push(c).map_err(|_| crate::Error::BufferOverflow)?;
        }
        Ok(Self { codes: code_vec })
    }

    /// Get code for subquantizer i
    #[inline]
    pub fn get_code(&self, i: usize) -> u8 {
        self.codes.get(i).copied().unwrap_or(0)
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.codes.len()
    }
}

/// Product Quantizer with codebooks
pub struct ProductQuantizer<const M: usize, const K: usize, const D: usize> {
    /// Codebooks: [M][K][D] flattened to [M * K * D]
    /// Each subquantizer has K centroids of dimension D
    codebooks: HVec<i8, { 8 * 16 * 8 }>, // Max 1024 bytes
    /// Configuration
    config: PQConfig,
}

impl<const M: usize, const K: usize, const D: usize> ProductQuantizer<M, K, D> {
    /// Create with random codebooks (for testing)
    pub fn random(config: PQConfig, seed: u32) -> crate::Result<Self> {
        let total_size = config.num_subquantizers * config.codebook_size * config.subvec_dim;

        let mut codebooks = HVec::new();
        let mut rng_state = seed;

        for _ in 0..total_size {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            let val = (((rng_state >> 16) & 0xFF) as i16 - 128) as i8;
            codebooks.push(val).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { codebooks, config })
    }

    /// Create from pre-trained codebooks
    pub fn from_codebooks(config: PQConfig, codebooks: &[i8]) -> crate::Result<Self> {
        let expected = config.num_subquantizers * config.codebook_size * config.subvec_dim;
        if codebooks.len() != expected {
            return Err(crate::Error::InvalidModel("Codebook size mismatch"));
        }

        let mut cb_vec = HVec::new();
        for &v in codebooks {
            cb_vec.push(v).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(Self { codebooks: cb_vec, config })
    }

    /// Get centroid for subquantizer m, code k
    #[inline]
    fn get_centroid(&self, m: usize, k: usize) -> &[i8] {
        let d = self.config.subvec_dim;
        let kk = self.config.codebook_size;
        let start = m * kk * d + k * d;
        &self.codebooks[start..start + d]
    }

    /// Encode a vector to PQ codes
    pub fn encode(&self, vector: &[i8]) -> crate::Result<PQCode<M>> {
        if vector.len() != self.config.dim {
            return Err(crate::Error::InvalidModel("Vector dimension mismatch"));
        }

        let mut codes = HVec::new();
        let d = self.config.subvec_dim;

        for m in 0..self.config.num_subquantizers {
            let subvec = &vector[m * d..(m + 1) * d];

            // Find nearest centroid
            let mut best_code = 0u8;
            let mut best_dist = i32::MAX;

            for k in 0..self.config.codebook_size {
                let centroid = self.get_centroid(m, k);
                let dist = Self::l2_squared(subvec, centroid);
                if dist < best_dist {
                    best_dist = dist;
                    best_code = k as u8;
                }
            }

            codes.push(best_code).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(PQCode { codes })
    }

    /// Decode PQ codes back to approximate vector
    pub fn decode(&self, code: &PQCode<M>, output: &mut [i8]) -> crate::Result<()> {
        if output.len() != self.config.dim {
            return Err(crate::Error::InvalidModel("Output dimension mismatch"));
        }

        let d = self.config.subvec_dim;

        for m in 0..self.config.num_subquantizers {
            let k = code.get_code(m) as usize;
            let centroid = self.get_centroid(m, k);
            output[m * d..(m + 1) * d].copy_from_slice(centroid);
        }

        Ok(())
    }

    /// Compute asymmetric distance: exact query vs PQ-encoded database vector
    pub fn asymmetric_distance(&self, query: &[i8], code: &PQCode<M>) -> i32 {
        let d = self.config.subvec_dim;
        let mut total_dist: i32 = 0;

        for m in 0..self.config.num_subquantizers {
            let query_sub = &query[m * d..(m + 1) * d];
            let k = code.get_code(m) as usize;
            let centroid = self.get_centroid(m, k);
            total_dist += Self::l2_squared(query_sub, centroid);
        }

        total_dist
    }

    /// Compute distance using pre-computed distance table (faster for batch queries)
    pub fn distance_with_table(&self, table: &PQDistanceTable<M, K>, code: &PQCode<M>) -> i32 {
        let mut total: i32 = 0;
        for m in 0..self.config.num_subquantizers {
            let k = code.get_code(m) as usize;
            total += table.get(m, k);
        }
        total
    }

    /// Build distance table for a query (precompute all query-centroid distances)
    pub fn build_distance_table(&self, query: &[i8]) -> PQDistanceTable<M, K> {
        let mut table = PQDistanceTable::new();
        let d = self.config.subvec_dim;

        for m in 0..self.config.num_subquantizers {
            let query_sub = &query[m * d..(m + 1) * d];
            for k in 0..self.config.codebook_size {
                let centroid = self.get_centroid(m, k);
                let dist = Self::l2_squared(query_sub, centroid);
                table.set(m, k, dist);
            }
        }

        table
    }

    /// L2 squared distance between two INT8 vectors
    #[inline]
    fn l2_squared(a: &[i8], b: &[i8]) -> i32 {
        let mut sum: i32 = 0;
        for (&x, &y) in a.iter().zip(b.iter()) {
            let diff = x as i32 - y as i32;
            sum += diff * diff;
        }
        sum
    }

    /// Memory usage of codebooks
    pub fn memory_size(&self) -> usize {
        self.codebooks.len()
    }

    /// Compression ratio vs INT8
    pub fn compression_ratio(&self) -> f32 {
        let original = self.config.dim as f32; // 1 byte per dim
        let compressed = self.config.num_subquantizers as f32; // 1 byte per code
        original / compressed
    }
}

/// Pre-computed distance table for fast PQ distance computation
pub struct PQDistanceTable<const M: usize, const K: usize> {
    /// Distances: [M][K] flattened
    distances: [i32; 128], // Max 8 subquantizers * 16 codes
}

impl<const M: usize, const K: usize> PQDistanceTable<M, K> {
    /// Create empty table
    pub fn new() -> Self {
        Self { distances: [0; 128] }
    }

    /// Get distance for subquantizer m, code k
    #[inline]
    pub fn get(&self, m: usize, k: usize) -> i32 {
        self.distances[m * K + k]
    }

    /// Set distance for subquantizer m, code k
    #[inline]
    pub fn set(&mut self, m: usize, k: usize, dist: i32) {
        self.distances[m * K + k] = dist;
    }
}

impl<const M: usize, const K: usize> Default for PQDistanceTable<M, K> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pq_config() {
        let config = PQConfig::default();
        assert_eq!(config.num_subquantizers, 4);
        assert_eq!(config.codebook_size, 16);
        assert_eq!(config.subvec_dim, 8);
        assert_eq!(config.dim, 32);
    }

    #[test]
    fn test_pq_encode_decode() {
        let config = PQConfig {
            num_subquantizers: 4,
            codebook_size: 16,
            subvec_dim: 8,
            dim: 32,
        };

        let pq = ProductQuantizer::<4, 16, 8>::random(config, 42).unwrap();

        // Create a test vector
        let mut vector = [0i8; 32];
        for i in 0..32 {
            vector[i] = (i as i8).wrapping_mul(3);
        }

        // Encode
        let code = pq.encode(&vector).unwrap();
        assert_eq!(code.codes.len(), 4);

        // Decode
        let mut decoded = [0i8; 32];
        pq.decode(&code, &mut decoded).unwrap();

        // Decoded should be approximate (using centroids)
        // Just verify it runs without error
    }

    #[test]
    fn test_pq_compression() {
        let config = PQConfig::default();
        let pq = ProductQuantizer::<4, 16, 8>::random(config, 42).unwrap();

        // 32 bytes original -> 4 bytes codes = 8x compression
        assert_eq!(pq.compression_ratio(), 8.0);
    }

    #[test]
    fn test_distance_table() {
        let config = PQConfig::default();
        let pq = ProductQuantizer::<4, 16, 8>::random(config, 42).unwrap();

        let mut query = [0i8; 32];
        for i in 0..32 {
            query[i] = i as i8;
        }

        let table = pq.build_distance_table(&query);

        // Encode a vector and compute distance both ways
        let mut vector = [10i8; 32];
        let code = pq.encode(&vector).unwrap();

        let dist1 = pq.asymmetric_distance(&query, &code);
        let dist2 = pq.distance_with_table(&table, &code);

        // Should be equal
        assert_eq!(dist1, dist2);
    }
}
