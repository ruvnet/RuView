//! Quantization Techniques for iOS/Browser WASM
//!
//! Memory-efficient vector compression for mobile devices.
//! - Scalar Quantization: 4x compression (f32 → u8)
//! - Binary Quantization: 32x compression (f32 → 1 bit)
//! - Product Quantization: 8-16x compression

use std::vec::Vec;

// ============================================
// Scalar Quantization (4x compression)
// ============================================

/// Scalar-quantized vector (f32 → u8)
#[derive(Clone, Debug)]
pub struct ScalarQuantized {
    /// Quantized values
    pub data: Vec<u8>,
    /// Minimum value for reconstruction
    pub min: f32,
    /// Scale factor for reconstruction
    pub scale: f32,
}

impl ScalarQuantized {
    /// Quantize a float vector to u8
    pub fn quantize(vector: &[f32]) -> Self {
        if vector.is_empty() {
            return Self {
                data: vec![],
                min: 0.0,
                scale: 1.0,
            };
        }

        let min = vector.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = vector.iter().cloned().fold(f32::NEG_INFINITY, f32::max);

        let scale = if (max - min).abs() < f32::EPSILON {
            1.0
        } else {
            (max - min) / 255.0
        };

        let data = vector
            .iter()
            .map(|&v| ((v - min) / scale).round().clamp(0.0, 255.0) as u8)
            .collect();

        Self { data, min, scale }
    }

    /// Reconstruct approximate float vector
    pub fn reconstruct(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|&v| self.min + (v as f32) * self.scale)
            .collect()
    }

    /// Fast distance calculation in quantized space
    pub fn distance(&self, other: &Self) -> f32 {
        let mut sum = 0i32;
        for (&a, &b) in self.data.iter().zip(other.data.iter()) {
            let diff = a as i32 - b as i32;
            sum += diff * diff;
        }
        (sum as f32).sqrt() * self.scale.max(other.scale)
    }

    /// Asymmetric distance (query is float, database is quantized)
    pub fn asymmetric_distance(&self, query: &[f32]) -> f32 {
        let len = self.data.len().min(query.len());
        let mut sum = 0.0f32;

        for i in 0..len {
            let reconstructed = self.min + (self.data[i] as f32) * self.scale;
            let diff = reconstructed - query[i];
            sum += diff * diff;
        }

        sum.sqrt()
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.data.len() + 8 // data + min + scale
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(8 + self.data.len());
        bytes.extend_from_slice(&self.min.to_le_bytes());
        bytes.extend_from_slice(&self.scale.to_le_bytes());
        bytes.extend_from_slice(&self.data);
        bytes
    }

    /// Deserialize from bytes
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 8 {
            return None;
        }
        let min = f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let scale = f32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let data = bytes[8..].to_vec();
        Some(Self { data, min, scale })
    }

    /// Estimate serialized size
    pub fn serialized_size(&self) -> usize {
        8 + self.data.len()
    }
}

// ============================================
// Binary Quantization (32x compression)
// ============================================

/// Binary-quantized vector (f32 → 1 bit)
#[derive(Clone, Debug)]
pub struct BinaryQuantized {
    /// Packed bits (8 dimensions per byte)
    pub bits: Vec<u8>,
    /// Original dimension count
    pub dimensions: usize,
}

impl BinaryQuantized {
    /// Quantize float vector to binary (sign-based)
    pub fn quantize(vector: &[f32]) -> Self {
        let dimensions = vector.len();
        let num_bytes = (dimensions + 7) / 8;
        let mut bits = vec![0u8; num_bytes];

        for (i, &v) in vector.iter().enumerate() {
            if v > 0.0 {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits[byte_idx] |= 1 << bit_idx;
            }
        }

        Self { bits, dimensions }
    }

    /// Quantize with threshold (not just sign)
    pub fn quantize_with_threshold(vector: &[f32], threshold: f32) -> Self {
        let dimensions = vector.len();
        let num_bytes = (dimensions + 7) / 8;
        let mut bits = vec![0u8; num_bytes];

        for (i, &v) in vector.iter().enumerate() {
            if v > threshold {
                let byte_idx = i / 8;
                let bit_idx = i % 8;
                bits[byte_idx] |= 1 << bit_idx;
            }
        }

        Self { bits, dimensions }
    }

    /// Hamming distance between two binary vectors
    pub fn distance(&self, other: &Self) -> u32 {
        let mut distance = 0u32;
        for (&a, &b) in self.bits.iter().zip(other.bits.iter()) {
            distance += (a ^ b).count_ones();
        }
        distance
    }

    /// Asymmetric distance to float query
    pub fn asymmetric_distance(&self, query: &[f32]) -> f32 {
        let mut distance = 0u32;
        for (i, &q) in query.iter().take(self.dimensions).enumerate() {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let bit = (self.bits.get(byte_idx).unwrap_or(&0) >> bit_idx) & 1;

            let query_bit = if q > 0.0 { 1 } else { 0 };
            if bit != query_bit {
                distance += 1;
            }
        }
        distance as f32
    }

    /// Reconstruct to +1/-1 vector
    pub fn reconstruct(&self) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.dimensions);
        for i in 0..self.dimensions {
            let byte_idx = i / 8;
            let bit_idx = i % 8;
            let bit = (self.bits.get(byte_idx).unwrap_or(&0) >> bit_idx) & 1;
            result.push(if bit == 1 { 1.0 } else { -1.0 });
        }
        result
    }

    /// Get memory size in bytes
    pub fn memory_size(&self) -> usize {
        self.bits.len() + 8 // bits + dimensions (as usize)
    }

    /// Serialize to bytes
    pub fn serialize(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + self.bits.len());
        bytes.extend_from_slice(&(self.dimensions as u32).to_le_bytes());
        bytes.extend_from_slice(&self.bits);
        bytes
    }

    /// Deserialize from bytes
    pub fn deserialize(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let dimensions = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let bits = bytes[4..].to_vec();
        Some(Self { bits, dimensions })
    }

    /// Estimate serialized size
    pub fn serialized_size(&self) -> usize {
        4 + self.bits.len()
    }
}

// ============================================
// Simple Product Quantization (8-16x compression)
// ============================================

/// Product-quantized vector
#[derive(Clone, Debug)]
pub struct ProductQuantized {
    /// Quantized codes (one per subspace)
    pub codes: Vec<u8>,
    /// Number of subspaces
    pub num_subspaces: usize,
}

/// Product quantization codebook
#[derive(Clone, Debug)]
pub struct PQCodebook {
    /// Centroids for each subspace [subspace][centroid][dim]
    pub centroids: Vec<Vec<Vec<f32>>>,
    /// Number of subspaces
    pub num_subspaces: usize,
    /// Dimension per subspace
    pub subspace_dim: usize,
    /// Number of centroids (usually 256 for u8 codes)
    pub num_centroids: usize,
}

impl PQCodebook {
    /// Train a PQ codebook using k-means
    pub fn train(
        vectors: &[Vec<f32>],
        num_subspaces: usize,
        num_centroids: usize,
        iterations: usize,
    ) -> Self {
        if vectors.is_empty() {
            return Self {
                centroids: vec![],
                num_subspaces,
                subspace_dim: 0,
                num_centroids,
            };
        }

        let dim = vectors[0].len();
        let subspace_dim = dim / num_subspaces;
        let mut centroids = Vec::with_capacity(num_subspaces);

        // Train each subspace independently
        for s in 0..num_subspaces {
            let start = s * subspace_dim;
            let end = start + subspace_dim;

            // Extract subvectors
            let subvectors: Vec<Vec<f32>> = vectors
                .iter()
                .map(|v| v[start..end].to_vec())
                .collect();

            // Run k-means
            let subspace_centroids = kmeans(&subvectors, num_centroids, iterations);
            centroids.push(subspace_centroids);
        }

        Self {
            centroids,
            num_subspaces,
            subspace_dim,
            num_centroids,
        }
    }

    /// Encode a vector using this codebook
    pub fn encode(&self, vector: &[f32]) -> ProductQuantized {
        let mut codes = Vec::with_capacity(self.num_subspaces);

        for (s, subspace_centroids) in self.centroids.iter().enumerate() {
            let start = s * self.subspace_dim;
            let end = start + self.subspace_dim;
            let subvector = &vector[start..end];

            // Find nearest centroid
            let code = subspace_centroids
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let dist = euclidean_squared(subvector, c);
                    (i, dist)
                })
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(i, _)| i as u8)
                .unwrap_or(0);

            codes.push(code);
        }

        ProductQuantized {
            codes,
            num_subspaces: self.num_subspaces,
        }
    }

    /// Decode a PQ vector back to approximate floats
    pub fn decode(&self, pq: &ProductQuantized) -> Vec<f32> {
        let mut result = Vec::with_capacity(self.num_subspaces * self.subspace_dim);

        for (s, &code) in pq.codes.iter().enumerate() {
            if s < self.centroids.len() && (code as usize) < self.centroids[s].len() {
                result.extend_from_slice(&self.centroids[s][code as usize]);
            }
        }

        result
    }

    /// Compute distance using precomputed distance table (ADC)
    pub fn asymmetric_distance(&self, pq: &ProductQuantized, query: &[f32]) -> f32 {
        let mut dist = 0.0f32;

        for (s, &code) in pq.codes.iter().enumerate() {
            let start = s * self.subspace_dim;
            let end = start + self.subspace_dim;
            let query_sub = &query[start..end];

            if s < self.centroids.len() && (code as usize) < self.centroids[s].len() {
                let centroid = &self.centroids[s][code as usize];
                dist += euclidean_squared(query_sub, centroid);
            }
        }

        dist.sqrt()
    }
}

// ============================================
// Helper Functions
// ============================================

fn euclidean_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

fn kmeans(vectors: &[Vec<f32>], k: usize, iterations: usize) -> Vec<Vec<f32>> {
    if vectors.is_empty() || k == 0 {
        return vec![];
    }

    let dim = vectors[0].len();

    // Initialize centroids (first k vectors or random subset)
    let mut centroids: Vec<Vec<f32>> = vectors.iter().take(k).cloned().collect();

    // Pad if not enough vectors
    while centroids.len() < k {
        centroids.push(vec![0.0; dim]);
    }

    for _ in 0..iterations {
        // Assign vectors to clusters
        let mut assignments: Vec<Vec<Vec<f32>>> = vec![vec![]; k];

        for vector in vectors {
            let nearest = centroids
                .iter()
                .enumerate()
                .map(|(i, c)| (i, euclidean_squared(vector, c)))
                .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
                .map(|(i, _)| i)
                .unwrap_or(0);

            assignments[nearest].push(vector.clone());
        }

        // Update centroids
        for (centroid, assigned) in centroids.iter_mut().zip(assignments.iter()) {
            if !assigned.is_empty() {
                for (i, c) in centroid.iter_mut().enumerate() {
                    *c = assigned.iter().map(|v| v[i]).sum::<f32>() / assigned.len() as f32;
                }
            }
        }
    }

    centroids
}

// ============================================
// WASM Exports
// ============================================

/// Scalar quantize a vector
#[no_mangle]
pub extern "C" fn scalar_quantize(
    input_ptr: *const f32,
    len: u32,
    out_data: *mut u8,
    out_min: *mut f32,
    out_scale: *mut f32,
) {
    unsafe {
        let input = core::slice::from_raw_parts(input_ptr, len as usize);
        let sq = ScalarQuantized::quantize(input);

        let out = core::slice::from_raw_parts_mut(out_data, sq.data.len());
        out.copy_from_slice(&sq.data);

        *out_min = sq.min;
        *out_scale = sq.scale;
    }
}

/// Binary quantize a vector
#[no_mangle]
pub extern "C" fn binary_quantize(
    input_ptr: *const f32,
    len: u32,
    out_bits: *mut u8,
) -> u32 {
    unsafe {
        let input = core::slice::from_raw_parts(input_ptr, len as usize);
        let bq = BinaryQuantized::quantize(input);

        let out = core::slice::from_raw_parts_mut(out_bits, bq.bits.len());
        out.copy_from_slice(&bq.bits);

        bq.bits.len() as u32
    }
}

/// Hamming distance between two binary vectors
#[no_mangle]
pub extern "C" fn hamming_distance(
    a_ptr: *const u8,
    b_ptr: *const u8,
    len: u32,
) -> u32 {
    unsafe {
        let a = core::slice::from_raw_parts(a_ptr, len as usize);
        let b = core::slice::from_raw_parts(b_ptr, len as usize);

        let mut distance = 0u32;
        for (&x, &y) in a.iter().zip(b.iter()) {
            distance += (x ^ y).count_ones();
        }
        distance
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scalar_quantization() {
        let v = vec![0.0, 0.5, 1.0, 0.25, 0.75];
        let sq = ScalarQuantized::quantize(&v);
        let reconstructed = sq.reconstruct();

        for (orig, recon) in v.iter().zip(reconstructed.iter()) {
            assert!((orig - recon).abs() < 0.01);
        }
    }

    #[test]
    fn test_binary_quantization() {
        let v = vec![1.0, -1.0, 0.5, -0.5];
        let bq = BinaryQuantized::quantize(&v);

        assert_eq!(bq.dimensions, 4);
        assert_eq!(bq.bits.len(), 1);
        assert_eq!(bq.bits[0], 0b0101); // positions 0 and 2 are positive
    }

    #[test]
    fn test_hamming_distance() {
        let v1 = vec![1.0, 1.0, 1.0, 1.0];
        let v2 = vec![1.0, -1.0, 1.0, -1.0];

        let bq1 = BinaryQuantized::quantize(&v1);
        let bq2 = BinaryQuantized::quantize(&v2);

        assert_eq!(bq1.distance(&bq2), 2);
    }

    #[test]
    fn test_pq_encode_decode() {
        let vectors: Vec<Vec<f32>> = (0..100)
            .map(|i| vec![i as f32 / 100.0; 8])
            .collect();

        let codebook = PQCodebook::train(&vectors, 2, 16, 10);
        let pq = codebook.encode(&vectors[50]);
        let decoded = codebook.decode(&pq);

        assert_eq!(decoded.len(), 8);
    }
}
