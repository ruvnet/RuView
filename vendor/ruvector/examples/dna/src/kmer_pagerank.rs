//! K-mer Graph PageRank for DNA Sequence Ranking
//!
//! Builds a k-mer co-occurrence graph from DNA sequences and uses
//! ruvector-solver's Forward Push Personalized PageRank (PPR) to rank
//! sequences by structural centrality in the k-mer overlap network.
//!
//! This enables identifying the most "representative" sequences in a
//! collection — those whose k-mer profiles are most connected to others.

use ruvector_solver::forward_push::ForwardPushSolver;
use ruvector_solver::types::CsrMatrix;

/// Result of PageRank-based sequence ranking
#[derive(Debug, Clone)]
pub struct SequenceRank {
    /// Index of the sequence in the input collection
    pub index: usize,
    /// PageRank score (higher = more central)
    pub score: f64,
}

/// K-mer graph builder and PageRank ranker.
///
/// Constructs a weighted graph where:
/// - Nodes are sequences
/// - Edge weight(i, j) = number of shared k-mers between sequences i and j
///
/// Then uses Forward Push PPR to compute centrality scores.
pub struct KmerGraphRanker {
    k: usize,
    hash_dimensions: usize,
}

impl KmerGraphRanker {
    /// Create a new ranker with the given k-mer length.
    ///
    /// # Arguments
    /// * `k` - K-mer length (typical: 11-31)
    /// * `hash_dimensions` - Number of hash buckets for k-mer fingerprints (default: 256)
    pub fn new(k: usize, hash_dimensions: usize) -> Self {
        Self { k, hash_dimensions }
    }

    /// Build a k-mer fingerprint vector for a DNA sequence.
    ///
    /// Uses FNV-1a hashing with canonical k-mers (min of forward/reverse-complement)
    /// to produce a fixed-size frequency vector.
    fn fingerprint(&self, seq: &[u8]) -> Vec<f64> {
        if seq.len() < self.k {
            return vec![0.0; self.hash_dimensions];
        }

        let mut counts = vec![0u32; self.hash_dimensions];

        for window in seq.windows(self.k) {
            let fwd = Self::fnv1a_hash(window);
            let rc = Self::fnv1a_hash_rc(window);
            let canonical = fwd.min(rc);
            counts[canonical % self.hash_dimensions] += 1;
        }

        // Normalize to probability distribution
        let total: u32 = counts.iter().sum();
        if total == 0 {
            return vec![0.0; self.hash_dimensions];
        }
        let inv = 1.0 / total as f64;
        counts.iter().map(|&c| c as f64 * inv).collect()
    }

    /// Compute cosine similarity between two fingerprint vectors.
    fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
        let dot: f64 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f64 = a.iter().map(|x| x * x).sum::<f64>().sqrt();
        let norm_b: f64 = b.iter().map(|x| x * x).sum::<f64>().sqrt();

        if norm_a < 1e-15 || norm_b < 1e-15 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }

    /// Build the k-mer overlap graph as a column-stochastic transition matrix.
    ///
    /// Edge weights are cosine similarities between k-mer fingerprints,
    /// normalized to form a stochastic matrix (columns sum to 1).
    fn build_transition_matrix(&self, sequences: &[&[u8]], threshold: f64) -> CsrMatrix<f64> {
        let n = sequences.len();
        let fingerprints: Vec<Vec<f64>> =
            sequences.iter().map(|seq| self.fingerprint(seq)).collect();

        // Build weighted adjacency with thresholding
        let mut col_sums = vec![0.0f64; n];
        let mut entries: Vec<(usize, usize, f64)> = Vec::new();

        for i in 0..n {
            for j in 0..n {
                if i == j {
                    continue;
                }
                let sim = Self::cosine_similarity(&fingerprints[i], &fingerprints[j]);
                if sim > threshold {
                    entries.push((i, j, sim));
                    col_sums[j] += sim;
                }
            }
        }

        // Normalize columns to make stochastic
        // Also add self-loops for isolated nodes
        let mut normalized: Vec<(usize, usize, f64)> = entries
            .into_iter()
            .map(|(i, j, w)| {
                let norm = if col_sums[j] > 1e-15 {
                    col_sums[j]
                } else {
                    1.0
                };
                (i, j, w / norm)
            })
            .collect();

        // Add self-loops for isolated nodes (dangling node handling)
        for j in 0..n {
            if col_sums[j] < 1e-15 {
                normalized.push((j, j, 1.0));
            }
        }

        CsrMatrix::<f64>::from_coo(n, n, normalized)
    }

    /// Rank sequences by PageRank centrality in the k-mer overlap graph.
    ///
    /// Uses ruvector-solver's Forward Push algorithm for sublinear-time
    /// Personalized PageRank computation.
    ///
    /// # Arguments
    /// * `sequences` - Collection of DNA sequences (as byte slices)
    /// * `alpha` - Teleportation probability (default: 0.15)
    /// * `epsilon` - PPR approximation tolerance (default: 1e-6)
    /// * `similarity_threshold` - Minimum cosine similarity to create an edge (default: 0.1)
    ///
    /// # Returns
    /// Sequences ranked by descending PageRank score
    pub fn rank_sequences(
        &self,
        sequences: &[&[u8]],
        alpha: f64,
        epsilon: f64,
        similarity_threshold: f64,
    ) -> Vec<SequenceRank> {
        let n = sequences.len();
        if n == 0 {
            return vec![];
        }
        if n == 1 {
            return vec![SequenceRank {
                index: 0,
                score: 1.0,
            }];
        }

        let matrix = self.build_transition_matrix(sequences, similarity_threshold);

        // Use Forward Push PPR from each node, accumulate global PageRank
        let solver = ForwardPushSolver::new(alpha, epsilon);
        let mut global_rank = vec![0.0f64; n];

        // Compute PPR from each node (or a representative subset for large graphs)
        let num_seeds = n.min(50); // Limit seeds for large collections
        let step = if n > num_seeds { n / num_seeds } else { 1 };

        for seed_idx in (0..n).step_by(step) {
            match solver.ppr_from_source(&matrix, seed_idx) {
                Ok(ppr_result) => {
                    for (node, score) in ppr_result {
                        if node < n {
                            global_rank[node] += score;
                        }
                    }
                }
                Err(_) => {
                    // If PPR fails for this seed, skip it
                    continue;
                }
            }
        }

        // Normalize
        let total: f64 = global_rank.iter().sum();
        if total > 1e-15 {
            let inv = 1.0 / total;
            for score in &mut global_rank {
                *score *= inv;
            }
        }

        // Build ranked results
        let mut results: Vec<SequenceRank> = global_rank
            .into_iter()
            .enumerate()
            .map(|(index, score)| SequenceRank { index, score })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        results
    }

    /// Compute pairwise PageRank similarity between two specific sequences
    /// within the context of a collection.
    ///
    /// Uses Forward Push PPR from the source sequence and returns the
    /// PPR score at the target sequence.
    pub fn pairwise_similarity(
        &self,
        sequences: &[&[u8]],
        source: usize,
        target: usize,
        alpha: f64,
        epsilon: f64,
        similarity_threshold: f64,
    ) -> f64 {
        if source >= sequences.len() || target >= sequences.len() {
            return 0.0;
        }

        let matrix = self.build_transition_matrix(sequences, similarity_threshold);
        let solver = ForwardPushSolver::new(alpha, epsilon);

        match solver.ppr_from_source(&matrix, source) {
            Ok(ppr_result) => ppr_result
                .into_iter()
                .find(|(node, _)| *node == target)
                .map(|(_, score)| score)
                .unwrap_or(0.0),
            Err(_) => 0.0,
        }
    }

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fingerprint() {
        let ranker = KmerGraphRanker::new(3, 64);
        let seq = b"ATCGATCGATCG";
        let fp = ranker.fingerprint(seq);
        assert_eq!(fp.len(), 64);

        // Should be a probability distribution (sums to ~1)
        let sum: f64 = fp.iter().sum();
        assert!((sum - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0, 2.0, 3.0];
        let b = vec![1.0, 2.0, 3.0];
        let sim = KmerGraphRanker::cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0];
        let b = vec![0.0, 1.0];
        let sim = KmerGraphRanker::cosine_similarity(&a, &b);
        assert!(sim.abs() < 1e-10);
    }

    #[test]
    fn test_rank_sequences_basic() {
        let ranker = KmerGraphRanker::new(3, 64);
        let seq1 = b"ATCGATCGATCGATCG";
        let seq2 = b"ATCGATCGATCGATCG"; // identical to seq1
        let seq3 = b"GCTAGCTAGCTAGCTA"; // different

        let sequences: Vec<&[u8]> = vec![seq1, seq2, seq3];
        let ranks = ranker.rank_sequences(&sequences, 0.15, 1e-4, 0.01);

        assert_eq!(ranks.len(), 3);

        // All ranks should sum to 1
        let total: f64 = ranks.iter().map(|r| r.score).sum();
        assert!((total - 1.0).abs() < 1e-5);

        // Identical sequences should have similar ranks
        let rank_0 = ranks.iter().find(|r| r.index == 0).unwrap().score;
        let rank_1 = ranks.iter().find(|r| r.index == 1).unwrap().score;
        assert!((rank_0 - rank_1).abs() < 0.3); // roughly similar
    }

    #[test]
    fn test_rank_empty() {
        let ranker = KmerGraphRanker::new(3, 64);
        let sequences: Vec<&[u8]> = vec![];
        let ranks = ranker.rank_sequences(&sequences, 0.15, 1e-4, 0.1);
        assert!(ranks.is_empty());
    }

    #[test]
    fn test_rank_single() {
        let ranker = KmerGraphRanker::new(3, 64);
        let sequences: Vec<&[u8]> = vec![b"ATCGATCG"];
        let ranks = ranker.rank_sequences(&sequences, 0.15, 1e-4, 0.1);
        assert_eq!(ranks.len(), 1);
        assert!((ranks[0].score - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_pairwise_similarity() {
        let ranker = KmerGraphRanker::new(3, 64);
        let seq1 = b"ATCGATCGATCGATCG";
        let seq2 = b"ATCGATCGATCGATCG";
        let seq3 = b"NNNNNNNNNNNNNNNN"; // very different

        let sequences: Vec<&[u8]> = vec![seq1, seq2, seq3];

        let sim_01 = ranker.pairwise_similarity(&sequences, 0, 1, 0.15, 1e-4, 0.01);
        let sim_02 = ranker.pairwise_similarity(&sequences, 0, 2, 0.15, 1e-4, 0.01);

        // Identical sequences should have higher similarity
        assert!(sim_01 >= sim_02);
    }
}
