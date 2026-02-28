//! Sparse mask utilities for attention patterns

use std::collections::HashSet;

/// Sparse mask for attention patterns
#[derive(Clone, Debug)]
pub struct AttentionMask {
    /// Sparse indices as (row, col) pairs
    pub indices: Vec<(usize, usize)>,
    /// Shape of the full attention matrix
    pub shape: (usize, usize),
    /// Set for O(1) lookup
    lookup: HashSet<(usize, usize)>,
}

impl AttentionMask {
    /// Create a new sparse mask from indices
    pub fn new(indices: Vec<(usize, usize)>, shape: (usize, usize)) -> Self {
        let lookup: HashSet<_> = indices.iter().copied().collect();
        Self {
            indices,
            shape,
            lookup,
        }
    }

    /// Check if position is masked (should attend)
    #[inline]
    pub fn is_attended(&self, row: usize, col: usize) -> bool {
        self.lookup.contains(&(row, col))
    }

    /// Apply mask to attention scores (set non-attended to -inf)
    pub fn apply(&self, scores: &mut [f32], seq_len: usize) {
        for i in 0..seq_len {
            for j in 0..seq_len {
                if !self.is_attended(i, j) {
                    scores[i * seq_len + j] = f32::NEG_INFINITY;
                }
            }
        }
    }

    /// Create a local window mask
    pub fn local_window(n: usize, window_size: usize) -> Self {
        let mut indices = Vec::new();
        let half_window = window_size / 2;

        for i in 0..n {
            let start = i.saturating_sub(half_window);
            let end = (i + half_window + 1).min(n);
            for j in start..end {
                indices.push((i, j));
            }
        }

        Self::new(indices, (n, n))
    }

    /// Create a causal mask (lower triangular)
    pub fn causal(n: usize) -> Self {
        let mut indices = Vec::new();
        for i in 0..n {
            for j in 0..=i {
                indices.push((i, j));
            }
        }
        Self::new(indices, (n, n))
    }

    /// Create a strided mask
    pub fn strided(n: usize, stride: usize) -> Self {
        let mut indices = Vec::new();
        for i in 0..n {
            for j in (0..n).step_by(stride) {
                indices.push((i, j));
            }
            // Always attend to self
            indices.push((i, i));
        }
        let mut indices: Vec<_> = indices
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        indices.sort();
        Self::new(indices, (n, n))
    }

    /// Number of non-zero entries
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Sparsity ratio (0 = all zeros, 1 = all ones)
    pub fn density(&self) -> f32 {
        self.nnz() as f32 / (self.shape.0 * self.shape.1) as f32
    }
}

/// Builder for creating sparse masks
pub struct SparseMaskBuilder {
    n: usize,
    indices: Vec<(usize, usize)>,
}

impl SparseMaskBuilder {
    pub fn new(n: usize) -> Self {
        Self {
            n,
            indices: Vec::new(),
        }
    }

    /// Add local window pattern
    pub fn with_local_window(mut self, window_size: usize) -> Self {
        let half_window = window_size / 2;
        for i in 0..self.n {
            let start = i.saturating_sub(half_window);
            let end = (i + half_window + 1).min(self.n);
            for j in start..end {
                self.indices.push((i, j));
            }
        }
        self
    }

    /// Add global tokens (all positions attend to these)
    pub fn with_global_tokens(mut self, global_indices: &[usize]) -> Self {
        for i in 0..self.n {
            for &g in global_indices {
                if g < self.n {
                    self.indices.push((i, g));
                    self.indices.push((g, i));
                }
            }
        }
        self
    }

    /// Add causal masking
    pub fn with_causal(mut self) -> Self {
        for i in 0..self.n {
            for j in 0..=i {
                self.indices.push((i, j));
            }
        }
        self
    }

    /// Build the mask
    pub fn build(self) -> AttentionMask {
        let mut indices: Vec<_> = self
            .indices
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        indices.sort();
        AttentionMask::new(indices, (self.n, self.n))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_window_mask() {
        let mask = AttentionMask::local_window(10, 3);

        // Position 5 should attend to positions 4, 5, 6
        assert!(mask.is_attended(5, 4));
        assert!(mask.is_attended(5, 5));
        assert!(mask.is_attended(5, 6));

        // Position 5 should not attend to position 0
        assert!(!mask.is_attended(5, 0));
    }

    #[test]
    fn test_causal_mask() {
        let mask = AttentionMask::causal(5);

        // Lower triangle should be attended
        assert!(mask.is_attended(2, 0));
        assert!(mask.is_attended(2, 1));
        assert!(mask.is_attended(2, 2));

        // Upper triangle should not
        assert!(!mask.is_attended(2, 3));
        assert!(!mask.is_attended(2, 4));
    }

    #[test]
    fn test_builder() {
        let mask = SparseMaskBuilder::new(10)
            .with_local_window(3)
            .with_global_tokens(&[0])
            .build();

        // All positions should attend to global token 0
        for i in 0..10 {
            assert!(mask.is_attended(i, 0));
        }
    }
}
