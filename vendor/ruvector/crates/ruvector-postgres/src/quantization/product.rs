//! Product Quantization (PQ)
//!
//! Compresses vectors by dividing into subspaces and quantizing each
//! independently. Achieves 8-32x compression with precomputed distance tables.

use rand::prelude::SliceRandom;
use rand::Rng;

/// Product Quantization configuration
#[derive(Debug, Clone)]
pub struct PQConfig {
    /// Number of subspaces (subvectors)
    pub m: usize,
    /// Number of centroids per subspace (typically 256 for 8-bit codes)
    pub k: usize,
    /// Random seed
    pub seed: u64,
}

impl Default for PQConfig {
    fn default() -> Self {
        Self {
            m: 8,   // 8 subspaces
            k: 256, // 256 centroids (8-bit codes)
            seed: 42,
        }
    }
}

/// Product Quantization index
pub struct ProductQuantizer {
    /// Configuration
    config: PQConfig,
    /// Dimensions per subspace
    dims_per_subspace: usize,
    /// Total dimensions
    dimensions: usize,
    /// Centroids for each subspace: [m][k][dims_per_subspace]
    centroids: Vec<Vec<Vec<f32>>>,
    /// Whether trained
    trained: bool,
}

impl ProductQuantizer {
    /// Create a new product quantizer
    pub fn new(dimensions: usize, config: PQConfig) -> Self {
        assert!(
            dimensions % config.m == 0,
            "Dimensions must be divisible by number of subspaces"
        );

        let dims_per_subspace = dimensions / config.m;

        Self {
            config,
            dims_per_subspace,
            dimensions,
            centroids: Vec::new(),
            trained: false,
        }
    }

    /// Train the quantizer on sample vectors
    pub fn train(&mut self, vectors: &[Vec<f32>]) {
        use rand::prelude::*;
        use rand_chacha::ChaCha8Rng;

        let mut rng = ChaCha8Rng::seed_from_u64(self.config.seed);

        self.centroids = Vec::with_capacity(self.config.m);

        for subspace in 0..self.config.m {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;

            // Extract subvectors
            let subvectors: Vec<Vec<f32>> =
                vectors.iter().map(|v| v[start..end].to_vec()).collect();

            // Run k-means on this subspace
            let centroids = self.kmeans(&subvectors, self.config.k, 10, &mut rng);
            self.centroids.push(centroids);
        }

        self.trained = true;
    }

    /// K-means clustering
    fn kmeans<R: Rng>(
        &self,
        vectors: &[Vec<f32>],
        k: usize,
        iterations: usize,
        rng: &mut R,
    ) -> Vec<Vec<f32>> {
        if vectors.is_empty() || k == 0 {
            return Vec::new();
        }

        let dims = vectors[0].len();
        let k = k.min(vectors.len());

        // Initialize centroids randomly
        let mut indices: Vec<usize> = (0..vectors.len()).collect();
        indices.shuffle(rng);

        let mut centroids: Vec<Vec<f32>> = indices
            .iter()
            .take(k)
            .map(|&i| vectors[i].clone())
            .collect();

        for _ in 0..iterations {
            // Assign vectors to nearest centroid
            let mut assignments: Vec<Vec<usize>> = vec![Vec::new(); k];

            for (i, v) in vectors.iter().enumerate() {
                let nearest = self.find_nearest(v, &centroids);
                assignments[nearest].push(i);
            }

            // Update centroids
            for (c, assigned) in assignments.iter().enumerate() {
                if assigned.is_empty() {
                    continue;
                }

                let mut new_centroid = vec![0.0f32; dims];
                for &i in assigned {
                    for (j, &val) in vectors[i].iter().enumerate() {
                        new_centroid[j] += val;
                    }
                }

                let count = assigned.len() as f32;
                for val in &mut new_centroid {
                    *val /= count;
                }

                centroids[c] = new_centroid;
            }
        }

        centroids
    }

    /// Find nearest centroid index
    fn find_nearest(&self, vector: &[f32], centroids: &[Vec<f32>]) -> usize {
        let mut best = 0;
        let mut best_dist = f32::MAX;

        for (i, c) in centroids.iter().enumerate() {
            let dist: f32 = vector
                .iter()
                .zip(c.iter())
                .map(|(a, b)| (a - b).powi(2))
                .sum();

            if dist < best_dist {
                best_dist = dist;
                best = i;
            }
        }

        best
    }

    /// Encode a vector to PQ codes
    pub fn encode(&self, vector: &[f32]) -> Vec<u8> {
        assert!(self.trained, "Quantizer must be trained");
        assert_eq!(vector.len(), self.dimensions);

        let mut codes = Vec::with_capacity(self.config.m);

        for subspace in 0..self.config.m {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let subvector = &vector[start..end];

            let nearest = self.find_nearest(subvector, &self.centroids[subspace]);
            codes.push(nearest as u8);
        }

        codes
    }

    /// Decode PQ codes back to approximate vector
    pub fn decode(&self, codes: &[u8]) -> Vec<f32> {
        assert!(self.trained, "Quantizer must be trained");
        assert_eq!(codes.len(), self.config.m);

        let mut vector = Vec::with_capacity(self.dimensions);

        for (subspace, &code) in codes.iter().enumerate() {
            let centroid = &self.centroids[subspace][code as usize];
            vector.extend_from_slice(centroid);
        }

        vector
    }

    /// Compute asymmetric distance (query to encoded vector)
    /// More accurate than symmetric but slower
    pub fn asymmetric_distance(&self, query: &[f32], codes: &[u8]) -> f32 {
        assert_eq!(query.len(), self.dimensions);
        assert_eq!(codes.len(), self.config.m);

        let mut distance_sq = 0.0f32;

        for (subspace, &code) in codes.iter().enumerate() {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let query_sub = &query[start..end];
            let centroid = &self.centroids[subspace][code as usize];

            for (q, c) in query_sub.iter().zip(centroid.iter()) {
                distance_sq += (q - c).powi(2);
            }
        }

        distance_sq.sqrt()
    }

    /// Precompute distance table for a query
    /// Returns: [m][k] distances from query subvector to each centroid
    pub fn precompute_distance_table(&self, query: &[f32]) -> Vec<Vec<f32>> {
        assert_eq!(query.len(), self.dimensions);

        let mut table = Vec::with_capacity(self.config.m);

        for subspace in 0..self.config.m {
            let start = subspace * self.dims_per_subspace;
            let end = start + self.dims_per_subspace;
            let query_sub = &query[start..end];

            let distances: Vec<f32> = self.centroids[subspace]
                .iter()
                .map(|c| {
                    query_sub
                        .iter()
                        .zip(c.iter())
                        .map(|(q, v)| (q - v).powi(2))
                        .sum::<f32>()
                })
                .collect();

            table.push(distances);
        }

        table
    }

    /// Fast distance using precomputed table
    pub fn table_distance(&self, table: &[Vec<f32>], codes: &[u8]) -> f32 {
        let mut distance_sq = 0.0f32;

        for (subspace, &code) in codes.iter().enumerate() {
            distance_sq += table[subspace][code as usize];
        }

        distance_sq.sqrt()
    }

    /// Memory per encoded vector in bytes
    pub fn bytes_per_vector(&self) -> usize {
        self.config.m // One byte per subspace
    }

    /// Compression ratio
    pub fn compression_ratio(&self) -> f32 {
        (self.dimensions * 4) as f32 / self.config.m as f32
    }
}

/// Encoded vector with its codes
#[derive(Debug, Clone)]
pub struct PQVector {
    pub codes: Vec<u8>,
}

impl PQVector {
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>() + self.codes.len()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use rand::prelude::*;
    use rand_chacha::ChaCha8Rng;

    fn random_vectors(n: usize, dims: usize, seed: u64) -> Vec<Vec<f32>> {
        let mut rng = ChaCha8Rng::seed_from_u64(seed);
        (0..n)
            .map(|_| (0..dims).map(|_| rng.gen_range(-1.0..1.0)).collect())
            .collect()
    }

    #[test]
    fn test_train_and_encode() {
        let dims = 128;
        let config = PQConfig {
            m: 8,
            k: 64,
            seed: 42,
        };

        let mut pq = ProductQuantizer::new(dims, config);

        let training = random_vectors(1000, dims, 42);
        pq.train(&training);

        // Encode a vector
        let vector = random_vectors(1, dims, 123)[0].clone();
        let codes = pq.encode(&vector);

        assert_eq!(codes.len(), 8);

        // Decode and check distance
        let decoded = pq.decode(&codes);
        let error: f32 = vector
            .iter()
            .zip(decoded.iter())
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f32>()
            .sqrt();

        // Error should be reasonable
        assert!(error < 2.0, "Reconstruction error too high: {}", error);
    }

    #[test]
    fn test_distance_table() {
        let dims = 64;
        let config = PQConfig {
            m: 4,
            k: 16,
            seed: 42,
        };

        let mut pq = ProductQuantizer::new(dims, config);
        let training = random_vectors(500, dims, 42);
        pq.train(&training);

        let query = random_vectors(1, dims, 123)[0].clone();
        let target = random_vectors(1, dims, 456)[0].clone();
        let codes = pq.encode(&target);

        // Compare asymmetric and table distances
        let asym_dist = pq.asymmetric_distance(&query, &codes);

        let table = pq.precompute_distance_table(&query);
        let table_dist = pq.table_distance(&table, &codes);

        assert!((asym_dist - table_dist).abs() < 0.001);
    }

    #[test]
    fn test_compression_ratio() {
        let dims = 1536;
        let config = PQConfig {
            m: 48,
            k: 256,
            seed: 42,
        };

        let pq = ProductQuantizer::new(dims, config);

        // Original: 1536 * 4 = 6144 bytes
        // Compressed: 48 bytes
        // Ratio: 128x
        assert_eq!(pq.bytes_per_vector(), 48);
        assert!((pq.compression_ratio() - 128.0).abs() < 0.1);
    }
}
