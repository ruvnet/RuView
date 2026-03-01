//! Product Quantization (PQ) — 8-16x compression.
//!
//! Splits a vector into M subspaces, learns K centroids per subspace
//! via k-means, and encodes each sub-vector as a centroid index (1 byte
//! when K <= 256).
//!
//! Used for the **Warm** (Tier 1) tier.

use crate::tier::TemperatureTier;
use crate::traits::Quantizer;
use alloc::vec;
use alloc::vec::Vec;

/// Product quantizer parameters and codebooks.
#[derive(Clone, Debug)]
pub struct ProductQuantizer {
    /// Number of subspaces.
    pub m: usize,
    /// Number of centroids per subspace.
    pub k: usize,
    /// Dimensions per subspace.
    pub sub_dim: usize,
    /// Codebooks: `codebooks[subspace][centroid]` is a `Vec<f32>` of length `sub_dim`.
    pub codebooks: Vec<Vec<Vec<f32>>>,
}

impl ProductQuantizer {
    /// Train a product quantizer using k-means clustering per subspace.
    ///
    /// # Arguments
    ///
    /// - `vectors`: Training vectors (all must have the same dimensionality).
    /// - `m`: Number of subspaces.
    /// - `k`: Number of centroids per subspace (typically 64, 128, or 256).
    /// - `iterations`: Number of k-means iterations.
    ///
    /// # Panics
    ///
    /// Panics if the vector dimensionality is not divisible by `m`, if `vectors`
    /// is empty, or if `k` or `m` is zero.
    pub fn train(vectors: &[&[f32]], m: usize, k: usize, iterations: usize) -> Self {
        assert!(!vectors.is_empty(), "need training data");
        assert!(m > 0 && k > 0, "m and k must be > 0");
        let dim = vectors[0].len();
        assert!(
            dim.is_multiple_of(m),
            "dim ({dim}) must be divisible by m ({m})"
        );
        let sub_dim = dim / m;

        let mut codebooks = Vec::with_capacity(m);

        for sub in 0..m {
            let start = sub * sub_dim;
            let end = start + sub_dim;

            // Extract sub-vectors for this subspace.
            let sub_vecs: Vec<&[f32]> = vectors.iter().map(|v| &v[start..end]).collect();

            let centroids = kmeans(&sub_vecs, k, sub_dim, iterations);
            codebooks.push(centroids);
        }

        Self {
            m,
            k,
            sub_dim,
            codebooks,
        }
    }

    /// Encode a vector: for each subspace, find the nearest centroid index.
    pub fn encode_vec(&self, vector: &[f32]) -> Vec<u8> {
        assert_eq!(vector.len(), self.m * self.sub_dim);
        let mut codes = Vec::with_capacity(self.m);
        for sub in 0..self.m {
            let start = sub * self.sub_dim;
            let sub_vec = &vector[start..start + self.sub_dim];
            let idx = nearest_centroid(sub_vec, &self.codebooks[sub]);
            codes.push(idx as u8);
        }
        codes
    }

    /// Decode codes back to an approximate vector by concatenating centroids.
    pub fn decode_vec(&self, codes: &[u8]) -> Vec<f32> {
        assert_eq!(codes.len(), self.m);
        let mut vector = Vec::with_capacity(self.m * self.sub_dim);
        for (sub, &code) in codes.iter().enumerate() {
            vector.extend_from_slice(&self.codebooks[sub][code as usize]);
        }
        vector
    }

    /// Precompute distance tables for Asymmetric Distance Computation (ADC).
    ///
    /// Returns a table `[subspace][centroid]` where entry (s, c) is the
    /// squared L2 distance from the query sub-vector s to centroid c.
    pub fn compute_distance_tables(&self, query: &[f32]) -> Vec<Vec<f32>> {
        assert_eq!(query.len(), self.m * self.sub_dim);
        let mut tables = Vec::with_capacity(self.m);
        for sub in 0..self.m {
            let start = sub * self.sub_dim;
            let q_sub = &query[start..start + self.sub_dim];
            let mut table = Vec::with_capacity(self.k);
            for centroid in &self.codebooks[sub] {
                table.push(l2_squared(q_sub, centroid));
            }
            tables.push(table);
        }
        tables
    }

    /// Compute the ADC distance using precomputed tables.
    ///
    /// Sum of table lookups: `dist = sum over s of tables[s][codes[s]]`.
    /// Uses `get_unchecked` for the inner lookup since `code` is always
    /// a valid centroid index (0..k) produced by `encode_vec`.
    pub fn distance_adc(tables: &[Vec<f32>], codes: &[u8]) -> f32 {
        assert_eq!(tables.len(), codes.len());
        let mut dist = 0.0f32;
        for (table, &code) in tables.iter().zip(codes.iter()) {
            // Safety: code is always in [0, k) as produced by encode_vec,
            // and each table has exactly k entries. Bounds check with debug_assert.
            debug_assert!((code as usize) < table.len());
            unsafe {
                dist += *table.get_unchecked(code as usize);
            }
        }
        dist
    }
}

impl Quantizer for ProductQuantizer {
    fn encode(&self, vector: &[f32]) -> Vec<u8> {
        self.encode_vec(vector)
    }

    fn decode(&self, codes: &[u8]) -> Vec<f32> {
        self.decode_vec(codes)
    }

    fn tier(&self) -> TemperatureTier {
        TemperatureTier::Warm
    }

    fn dim(&self) -> usize {
        self.m * self.sub_dim
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Squared L2 distance between two slices.
fn l2_squared(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| {
            let d = x - y;
            d * d
        })
        .sum()
}

/// Find the index of the nearest centroid to `point`.
fn nearest_centroid(point: &[f32], centroids: &[Vec<f32>]) -> usize {
    let mut best_idx = 0;
    let mut best_dist = f32::INFINITY;
    for (i, c) in centroids.iter().enumerate() {
        let d = l2_squared(point, c);
        if d < best_dist {
            best_dist = d;
            best_idx = i;
        }
    }
    best_idx
}

/// Simple k-means clustering.
///
/// Initializes centroids from the first K data points (wrapping if needed),
/// then runs Lloyd's algorithm for `iterations` rounds.
fn kmeans(data: &[&[f32]], k: usize, sub_dim: usize, iterations: usize) -> Vec<Vec<f32>> {
    let n = data.len();
    let actual_k = k.min(n); // can't have more centroids than data points

    // Initialize centroids from data.
    let mut centroids: Vec<Vec<f32>> = (0..actual_k).map(|i| data[i % n].to_vec()).collect();

    let mut assignments = vec![0usize; n];
    let mut counts = vec![0usize; actual_k];
    let mut sums = vec![vec![0.0f32; sub_dim]; actual_k];

    for _ in 0..iterations {
        // Assignment step.
        for (i, point) in data.iter().enumerate() {
            assignments[i] = nearest_centroid(point, &centroids);
        }

        // Update step.
        counts.fill(0);
        for s in &mut sums {
            for v in s.iter_mut() {
                *v = 0.0;
            }
        }

        for (i, point) in data.iter().enumerate() {
            let c = assignments[i];
            counts[c] += 1;
            for (d, &val) in point.iter().enumerate() {
                sums[c][d] += val;
            }
        }

        for c in 0..actual_k {
            if counts[c] > 0 {
                for d in 0..sub_dim {
                    centroids[c][d] = sums[c][d] / counts[c] as f32;
                }
            }
        }
    }

    // If we need more centroids than data points, duplicate the last centroid.
    while centroids.len() < k {
        centroids.push(centroids[centroids.len() - 1].clone());
    }

    centroids
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pq_data() -> Vec<Vec<f32>> {
        // 50 vectors of dim 16
        let mut vecs = Vec::new();
        for i in 0..50 {
            let v: Vec<f32> = (0..16)
                .map(|d| ((i * 7 + d * 13 + 3) % 200) as f32 / 100.0 - 1.0)
                .collect();
            vecs.push(v);
        }
        vecs
    }

    #[test]
    fn train_and_encode_decode() {
        let data = make_pq_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let pq = ProductQuantizer::train(&refs, 4, 8, 10);

        assert_eq!(pq.m, 4);
        assert_eq!(pq.k, 8);
        assert_eq!(pq.sub_dim, 4);
        assert_eq!(pq.codebooks.len(), 4);

        let codes = pq.encode_vec(&data[0]);
        assert_eq!(codes.len(), 4);
        for &c in &codes {
            assert!((c as usize) < 8);
        }

        let recon = pq.decode_vec(&codes);
        assert_eq!(recon.len(), 16);
    }

    #[test]
    fn adc_distance() {
        let data = make_pq_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let pq = ProductQuantizer::train(&refs, 4, 8, 10);

        let query = &data[0];
        let tables = pq.compute_distance_tables(query);
        assert_eq!(tables.len(), 4);

        let codes = pq.encode_vec(&data[1]);
        let dist = ProductQuantizer::distance_adc(&tables, &codes);
        assert!(dist >= 0.0);

        // Distance to self should be very small
        let self_codes = pq.encode_vec(query);
        let self_dist = ProductQuantizer::distance_adc(&tables, &self_codes);
        assert!(self_dist < dist || dist == 0.0);
    }

    #[test]
    fn pq_convergence() {
        // After training, reconstruction error should decrease with more iterations.
        let data = make_pq_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();

        let pq_1 = ProductQuantizer::train(&refs, 4, 8, 1);
        let pq_20 = ProductQuantizer::train(&refs, 4, 8, 20);

        let error_1: f32 = data
            .iter()
            .map(|v| {
                let codes = pq_1.encode_vec(v);
                let recon = pq_1.decode_vec(&codes);
                l2_squared(v, &recon)
            })
            .sum();

        let error_20: f32 = data
            .iter()
            .map(|v| {
                let codes = pq_20.encode_vec(v);
                let recon = pq_20.decode_vec(&codes);
                l2_squared(v, &recon)
            })
            .sum();

        assert!(
            error_20 <= error_1 + f32::EPSILON,
            "more iterations should not increase error: {error_1} vs {error_20}"
        );
    }

    #[test]
    fn quantizer_trait() {
        let data = make_pq_data();
        let refs: Vec<&[f32]> = data.iter().map(|v| v.as_slice()).collect();
        let pq = ProductQuantizer::train(&refs, 4, 8, 5);
        assert_eq!(pq.tier(), TemperatureTier::Warm);
        assert_eq!(pq.dim(), 16);
    }
}
