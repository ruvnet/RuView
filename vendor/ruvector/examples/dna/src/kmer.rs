//! K-mer encoding and HNSW vector indexing for DNA sequences
//!
//! This module provides efficient k-mer based vector encoding for DNA sequences
//! with HNSW indexing for fast similarity search. Implements both k-mer frequency
//! vectors and MinHash sketching (Mash/sourmash algorithm).

use ruvector_core::{
    types::{DbOptions, DistanceMetric, HnswConfig, QuantizationConfig, SearchQuery},
    VectorDB, VectorEntry,
};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KmerError {
    #[error("Invalid k-mer length: {0}")]
    InvalidKmerLength(usize),
    #[error("Invalid DNA sequence: {0}")]
    InvalidSequence(String),
    #[error("Database error: {0}")]
    DatabaseError(#[from] ruvector_core::RuvectorError),
    #[error("Empty sequence")]
    EmptySequence,
}

type Result<T> = std::result::Result<T, KmerError>;

/// Nucleotide to 2-bit encoding: A=0, C=1, G=2, T=3
#[inline]
fn nucleotide_to_bits(nuc: u8) -> Option<u8> {
    match nuc.to_ascii_uppercase() {
        b'A' => Some(0),
        b'C' => Some(1),
        b'G' => Some(2),
        b'T' | b'U' => Some(3),
        _ => None,
    }
}

/// Returns the reverse complement of a DNA sequence
fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter()
        .rev()
        .map(|&nuc| match nuc.to_ascii_uppercase() {
            b'A' => b'T',
            b'T' | b'U' => b'A',
            b'C' => b'G',
            b'G' => b'C',
            n => n,
        })
        .collect()
}

/// Returns the canonical k-mer (lexicographically smaller of k-mer and its reverse complement)
pub fn canonical_kmer(kmer: &[u8]) -> Vec<u8> {
    let rc = reverse_complement(kmer);
    if kmer <= rc.as_slice() {
        kmer.to_vec()
    } else {
        rc
    }
}

/// K-mer encoder that converts DNA sequences into frequency vectors
pub struct KmerEncoder {
    k: usize,
    dimensions: usize,
}

impl KmerEncoder {
    /// Create a new k-mer encoder for k-mers of length k
    ///
    /// # Arguments
    /// * `k` - Length of k-mers (typical values: 21, 31)
    ///
    /// Uses feature hashing to limit dimensionality for large k
    pub fn new(k: usize) -> Result<Self> {
        if k == 0 || k > 32 {
            return Err(KmerError::InvalidKmerLength(k));
        }

        // Calculate dimensions: min(4^k, 1024) using feature hashing
        let max_kmers = 4_usize.saturating_pow(k as u32);
        let dimensions = max_kmers.min(1024);

        Ok(Self { k, dimensions })
    }

    /// Get the number of dimensions in the encoded vector
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Encode a DNA sequence into a k-mer frequency vector
    ///
    /// Uses canonical k-mer hashing (min of forward/reverse-complement hash)
    /// to count strand-agnostic k-mers, then normalizes to unit vector.
    pub fn encode_sequence(&self, seq: &[u8]) -> Result<Vec<f32>> {
        if seq.len() < self.k {
            return Err(KmerError::EmptySequence);
        }

        let mut counts = vec![0u32; self.dimensions];
        let mut total = 0u32;

        // Extract all k-mers using a sliding window
        // Avoid Vec allocation by hashing both strands and taking min
        for window in seq.windows(self.k) {
            let fwd_hash = Self::fnv1a_hash(window);
            let rc_hash = Self::fnv1a_hash_rc(window);
            let canonical_hash = fwd_hash.min(rc_hash);
            let index = canonical_hash % self.dimensions;

            counts[index] = counts[index].saturating_add(1);
            total = total.saturating_add(1);
        }

        // Normalize to frequency vector and then to unit vector
        let inv_total = 1.0 / total as f32;
        let mut vector: Vec<f32> = counts
            .iter()
            .map(|&count| count as f32 * inv_total)
            .collect();

        // L2 normalization
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            let inv_norm = 1.0 / norm;
            vector.iter_mut().for_each(|x| *x *= inv_norm);
        }

        Ok(vector)
    }

    /// FNV-1a hash of a byte slice
    #[inline]
    fn fnv1a_hash(data: &[u8]) -> usize {
        const FNV_OFFSET: u64 = 14695981039346656037;
        const FNV_PRIME: u64 = 1099511628211;
        let mut hash = FNV_OFFSET;
        for &byte in data {
            hash ^= byte as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash as usize
    }

    /// FNV-1a hash of reverse complement (avoids Vec allocation)
    #[inline]
    fn fnv1a_hash_rc(data: &[u8]) -> usize {
        const FNV_OFFSET: u64 = 14695981039346656037;
        const FNV_PRIME: u64 = 1099511628211;
        let mut hash = FNV_OFFSET;
        for &byte in data.iter().rev() {
            let comp = match byte.to_ascii_uppercase() {
                b'A' => b'T',
                b'T' | b'U' => b'A',
                b'C' => b'G',
                b'G' => b'C',
                n => n,
            };
            hash ^= comp as u64;
            hash = hash.wrapping_mul(FNV_PRIME);
        }
        hash as usize
    }

    /// Hash a k-mer to an index using FNV-1a hash
    fn hash_kmer(&self, kmer: &[u8]) -> usize {
        Self::fnv1a_hash(kmer)
    }
}

/// MinHash sketch for fast sequence similarity (Mash/sourmash algorithm)
pub struct MinHashSketch {
    num_hashes: usize,
    hashes: Vec<u64>,
}

impl MinHashSketch {
    /// Create a new MinHash sketch with the given number of hashes
    ///
    /// # Arguments
    /// * `num_hashes` - Number of hash values to keep (typically 1000)
    pub fn new(num_hashes: usize) -> Self {
        Self {
            num_hashes,
            hashes: Vec::new(),
        }
    }

    /// Compute MinHash signature for a DNA sequence
    pub fn sketch(&mut self, seq: &[u8], k: usize) -> Result<&[u64]> {
        if seq.len() < k {
            return Err(KmerError::EmptySequence);
        }

        let mut all_hashes = Vec::with_capacity(seq.len() - k + 1);

        // Hash all k-mers using dual-hash (no Vec allocation per k-mer)
        for window in seq.windows(k) {
            let fwd = Self::hash_kmer_64_slice(window);
            let rc = Self::hash_kmer_64_rc(window);
            all_hashes.push(fwd.min(rc));
        }

        // Sort and keep the smallest num_hashes values
        all_hashes.sort_unstable();
        all_hashes.truncate(self.num_hashes);
        self.hashes = all_hashes;

        Ok(&self.hashes)
    }

    /// Compute Jaccard distance between two MinHash sketches
    pub fn jaccard_distance(&self, other: &MinHashSketch) -> f32 {
        if self.hashes.is_empty() || other.hashes.is_empty() {
            return 1.0;
        }

        let mut intersection = 0;
        let mut i = 0;
        let mut j = 0;

        // Count intersection using sorted arrays
        while i < self.hashes.len() && j < other.hashes.len() {
            if self.hashes[i] == other.hashes[j] {
                intersection += 1;
                i += 1;
                j += 1;
            } else if self.hashes[i] < other.hashes[j] {
                i += 1;
            } else {
                j += 1;
            }
        }

        let union = self.hashes.len() + other.hashes.len() - intersection;
        if union == 0 {
            return 0.0;
        }

        let jaccard_similarity = intersection as f32 / union as f32;
        1.0 - jaccard_similarity
    }

    /// Hash a k-mer using MurmurHash3-like algorithm (forward strand)
    #[inline]
    fn hash_kmer_64_slice(kmer: &[u8]) -> u64 {
        const C1: u64 = 0x87c37b91114253d5;
        const C2: u64 = 0x4cf5ad432745937f;
        let mut h = 0u64;
        for &byte in kmer {
            let mut k = byte as u64;
            k = k.wrapping_mul(C1);
            k = k.rotate_left(31);
            k = k.wrapping_mul(C2);
            h ^= k;
            h = h.rotate_left(27);
            h = h.wrapping_mul(5).wrapping_add(0x52dce729);
        }
        h ^ kmer.len() as u64
    }

    /// Hash reverse complement of a k-mer (no Vec allocation)
    #[inline]
    fn hash_kmer_64_rc(kmer: &[u8]) -> u64 {
        const C1: u64 = 0x87c37b91114253d5;
        const C2: u64 = 0x4cf5ad432745937f;
        let mut h = 0u64;
        for &byte in kmer.iter().rev() {
            let comp = match byte.to_ascii_uppercase() {
                b'A' => b'T',
                b'T' | b'U' => b'A',
                b'C' => b'G',
                b'G' => b'C',
                n => n,
            };
            let mut k = comp as u64;
            k = k.wrapping_mul(C1);
            k = k.rotate_left(31);
            k = k.wrapping_mul(C2);
            h ^= k;
            h = h.rotate_left(27);
            h = h.wrapping_mul(5).wrapping_add(0x52dce729);
        }
        h ^ kmer.len() as u64
    }

    /// Get the hashes
    pub fn hashes(&self) -> &[u64] {
        &self.hashes
    }
}

/// Search result for k-mer index queries
#[derive(Debug, Clone)]
pub struct KmerSearchResult {
    pub id: String,
    pub score: f32,
    pub distance: f32,
}

/// K-mer index wrapping VectorDB for sequence similarity search
pub struct KmerIndex {
    db: VectorDB,
    encoder: KmerEncoder,
    k: usize,
}

impl KmerIndex {
    /// Create a new k-mer index
    ///
    /// # Arguments
    /// * `k` - K-mer length
    /// * `dimensions` - Vector dimensions (should match encoder dimensions)
    pub fn new(k: usize, dimensions: usize) -> Result<Self> {
        let encoder = KmerEncoder::new(k)?;

        // Verify dimensions match
        if encoder.dimensions() != dimensions {
            return Err(KmerError::InvalidKmerLength(k));
        }

        let options = DbOptions {
            dimensions,
            distance_metric: DistanceMetric::Cosine,
            storage_path: format!("./kmer_index_k{}.db", k),
            hnsw_config: Some(HnswConfig {
                m: 32,
                ef_construction: 200,
                ef_search: 100,
                max_elements: 1_000_000,
            }),
            quantization: Some(QuantizationConfig::Scalar),
        };

        let db = VectorDB::new(options)?;

        Ok(Self { db, encoder, k })
    }

    /// Index a single DNA sequence
    pub fn index_sequence(&self, id: &str, sequence: &[u8]) -> Result<()> {
        let vector = self.encoder.encode_sequence(sequence)?;

        let entry = VectorEntry {
            id: Some(id.to_string()),
            vector,
            metadata: Some({
                let mut meta = HashMap::new();
                meta.insert("length".to_string(), serde_json::json!(sequence.len()));
                meta.insert("k".to_string(), serde_json::json!(self.k));
                meta
            }),
        };

        self.db.insert(entry)?;
        Ok(())
    }

    /// Index multiple sequences in a batch
    pub fn index_batch(&self, sequences: Vec<(&str, &[u8])>) -> Result<()> {
        let entries: Result<Vec<VectorEntry>> = sequences
            .into_iter()
            .map(|(id, seq)| {
                let vector = self.encoder.encode_sequence(seq)?;
                Ok(VectorEntry {
                    id: Some(id.to_string()),
                    vector,
                    metadata: Some({
                        let mut meta = HashMap::new();
                        meta.insert("length".to_string(), serde_json::json!(seq.len()));
                        meta.insert("k".to_string(), serde_json::json!(self.k));
                        meta
                    }),
                })
            })
            .collect();

        self.db.insert_batch(entries?)?;
        Ok(())
    }

    /// Search for similar sequences
    pub fn search_similar(&self, query: &[u8], top_k: usize) -> Result<Vec<KmerSearchResult>> {
        let query_vector = self.encoder.encode_sequence(query)?;

        let search_query = SearchQuery {
            vector: query_vector,
            k: top_k,
            filter: None,
            ef_search: None,
        };

        let results = self.db.search(search_query)?;

        Ok(results
            .into_iter()
            .map(|r| KmerSearchResult {
                id: r.id,
                score: r.score,
                distance: r.score,
            })
            .collect())
    }

    /// Search for sequences with similarity above a threshold
    pub fn search_with_threshold(
        &self,
        query: &[u8],
        threshold: f32,
    ) -> Result<Vec<KmerSearchResult>> {
        // Search with a larger k to ensure we get all candidates
        let results = self.search_similar(query, 100)?;

        Ok(results
            .into_iter()
            .filter(|r| r.distance <= threshold)
            .collect())
    }

    /// Get the k-mer length
    pub fn k(&self) -> usize {
        self.k
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nucleotide_encoding() {
        assert_eq!(nucleotide_to_bits(b'A'), Some(0));
        assert_eq!(nucleotide_to_bits(b'C'), Some(1));
        assert_eq!(nucleotide_to_bits(b'G'), Some(2));
        assert_eq!(nucleotide_to_bits(b'T'), Some(3));
        assert_eq!(nucleotide_to_bits(b'a'), Some(0));
        assert_eq!(nucleotide_to_bits(b'N'), None);
    }

    #[test]
    fn test_reverse_complement() {
        let seq = b"ATCG";
        let rc = reverse_complement(seq);
        assert_eq!(rc, b"CGAT");
    }

    #[test]
    fn test_canonical_kmer() {
        let kmer1 = b"ATCG";
        let kmer2 = b"CGAT"; // reverse complement

        let canon1 = canonical_kmer(kmer1);
        let canon2 = canonical_kmer(kmer2);

        assert_eq!(canon1, canon2);
    }

    #[test]
    fn test_kmer_encoder_creation() {
        let encoder = KmerEncoder::new(3).unwrap();
        assert_eq!(encoder.k, 3);
        assert_eq!(encoder.dimensions(), 64);
    }

    #[test]
    fn test_kmer_encoder_large_k() {
        let encoder = KmerEncoder::new(21).unwrap();
        assert_eq!(encoder.k, 21);
        assert_eq!(encoder.dimensions(), 1024); // Capped by feature hashing
    }

    #[test]
    fn test_encode_sequence() {
        let encoder = KmerEncoder::new(3).unwrap();
        let seq = b"ATCGATCG";
        let vector = encoder.encode_sequence(seq).unwrap();

        assert_eq!(vector.len(), encoder.dimensions());

        // Check L2 normalization
        let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5);
    }

    #[test]
    fn test_minhash_sketch() {
        let mut sketch = MinHashSketch::new(100);
        let seq = b"ATCGATCGATCGATCGATCG";

        sketch.sketch(seq, 5).unwrap();
        assert!(sketch.hashes().len() <= 100);
    }

    #[test]
    fn test_jaccard_distance() {
        let mut sketch1 = MinHashSketch::new(100);
        let mut sketch2 = MinHashSketch::new(100);

        let seq1 = b"ATCGATCGATCGATCGATCG";
        let seq2 = b"ATCGATCGATCGATCGATCG"; // Identical

        sketch1.sketch(seq1, 5).unwrap();
        sketch2.sketch(seq2, 5).unwrap();

        let distance = sketch1.jaccard_distance(&sketch2);
        assert!(distance < 0.01); // Should be very similar
    }
}
