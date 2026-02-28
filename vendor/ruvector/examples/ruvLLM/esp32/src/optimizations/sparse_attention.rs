//! Sparse Attention Patterns for ESP32
//!
//! Reduces attention complexity from O(n²) to O(n) using:
//! - Sliding window attention
//! - Strided patterns
//! - Block-sparse attention

use heapless::Vec as HVec;

/// Maximum sequence length for sparse patterns
pub const MAX_SPARSE_SEQ: usize = 32;
/// Maximum window size
pub const MAX_WINDOW_SIZE: usize = 8;

/// Attention pattern types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttentionPattern {
    /// Full attention (O(n²)) - baseline
    Full,
    /// Sliding window attention (O(n * w))
    SlidingWindow { window_size: usize },
    /// Strided attention (O(n * n/s))
    Strided { stride: usize },
    /// Combined window + stride
    Longformer { window_size: usize, stride: usize },
    /// Block diagonal attention
    BlockDiagonal { block_size: usize },
    /// Local + global tokens
    BigBird { window_size: usize, global_tokens: usize },
}

impl Default for AttentionPattern {
    fn default() -> Self {
        // Sliding window is best for tiny models
        Self::SlidingWindow { window_size: 4 }
    }
}

/// Sparse attention implementation
pub struct SparseAttention {
    /// Pattern type
    pattern: AttentionPattern,
    /// Attention mask (true = attend, false = skip)
    /// Stored as bitmask for memory efficiency
    mask_data: HVec<u32, MAX_SPARSE_SEQ>,
    /// Sequence length
    seq_len: usize,
}

impl SparseAttention {
    /// Create sparse attention with given pattern
    pub fn new(pattern: AttentionPattern, seq_len: usize) -> crate::Result<Self> {
        if seq_len > MAX_SPARSE_SEQ {
            return Err(crate::Error::BufferOverflow);
        }

        let mut sa = Self {
            pattern,
            mask_data: HVec::new(),
            seq_len,
        };

        sa.build_mask()?;
        Ok(sa)
    }

    /// Build attention mask based on pattern
    fn build_mask(&mut self) -> crate::Result<()> {
        self.mask_data.clear();

        for i in 0..self.seq_len {
            let mut row_mask: u32 = 0;

            for j in 0..self.seq_len {
                if j <= i && self.should_attend(i, j) {
                    row_mask |= 1 << j;
                }
            }

            self.mask_data.push(row_mask).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(())
    }

    /// Check if position i should attend to position j
    fn should_attend(&self, i: usize, j: usize) -> bool {
        match self.pattern {
            AttentionPattern::Full => true,

            AttentionPattern::SlidingWindow { window_size } => {
                i.saturating_sub(window_size) <= j
            }

            AttentionPattern::Strided { stride } => {
                j % stride == 0 || i.saturating_sub(1) <= j
            }

            AttentionPattern::Longformer { window_size, stride } => {
                // Local window OR strided global
                i.saturating_sub(window_size) <= j || j % stride == 0
            }

            AttentionPattern::BlockDiagonal { block_size } => {
                // Same block
                i / block_size == j / block_size
            }

            AttentionPattern::BigBird { window_size, global_tokens } => {
                // Local window OR global tokens (first N positions)
                i.saturating_sub(window_size) <= j || j < global_tokens
            }
        }
    }

    /// Check if query position i should attend to key position j
    #[inline]
    pub fn should_attend_at(&self, i: usize, j: usize) -> bool {
        if i >= self.seq_len || j >= self.seq_len {
            return false;
        }
        (self.mask_data[i] >> j) & 1 == 1
    }

    /// Get mask row for position i (for vectorized attention)
    #[inline]
    pub fn get_mask_row(&self, i: usize) -> u32 {
        self.mask_data.get(i).copied().unwrap_or(0)
    }

    /// Apply sparse attention: scores = Q @ K^T, masked
    /// Only computes necessary positions
    pub fn sparse_qk(
        &self,
        query: &[i8],      // [dim]
        keys: &[&[i8]],    // [seq_len][dim]
        scores: &mut [i32], // [seq_len]
        query_pos: usize,
    ) {
        let mask = self.get_mask_row(query_pos);

        for (j, key) in keys.iter().enumerate() {
            if (mask >> j) & 1 == 1 {
                // Compute dot product
                let mut sum: i32 = 0;
                for (&q, &k) in query.iter().zip(key.iter()) {
                    sum += q as i32 * k as i32;
                }
                scores[j] = sum;
            } else {
                scores[j] = i32::MIN; // Will be zeroed by softmax
            }
        }
    }

    /// Count active attention positions
    pub fn active_positions(&self) -> usize {
        self.mask_data.iter().map(|m| m.count_ones() as usize).sum()
    }

    /// Theoretical vs actual computation ratio
    pub fn sparsity_ratio(&self) -> f32 {
        let full = self.seq_len * (self.seq_len + 1) / 2; // Lower triangular
        let sparse = self.active_positions();
        sparse as f32 / full as f32
    }

    /// Memory savings description
    pub fn memory_savings(&self) -> &'static str {
        match self.pattern {
            AttentionPattern::Full => "None (O(n²))",
            AttentionPattern::SlidingWindow { .. } => "O(n) - linear",
            AttentionPattern::Strided { .. } => "O(n) - linear",
            AttentionPattern::Longformer { .. } => "O(n) - linear",
            AttentionPattern::BlockDiagonal { .. } => "O(n) - block-linear",
            AttentionPattern::BigBird { .. } => "O(n) - linear",
        }
    }
}

/// Precomputed attention patterns for different sequence lengths
pub struct AttentionPatternCache {
    /// Cached patterns for common lengths
    patterns: [Option<SparseAttention>; 4],
}

impl AttentionPatternCache {
    /// Create cache with sliding window patterns
    pub fn new_sliding(window_size: usize) -> Self {
        let pattern = AttentionPattern::SlidingWindow { window_size };

        Self {
            patterns: [
                SparseAttention::new(pattern, 8).ok(),
                SparseAttention::new(pattern, 16).ok(),
                SparseAttention::new(pattern, 24).ok(),
                SparseAttention::new(pattern, 32).ok(),
            ],
        }
    }

    /// Get pattern for sequence length
    pub fn get(&self, seq_len: usize) -> Option<&SparseAttention> {
        let idx = match seq_len {
            1..=8 => 0,
            9..=16 => 1,
            17..=24 => 2,
            25..=32 => 3,
            _ => return None,
        };
        self.patterns[idx].as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sliding_window() {
        let sa = SparseAttention::new(
            AttentionPattern::SlidingWindow { window_size: 2 },
            8,
        ).unwrap();

        // Position 0: should only attend to 0
        assert!(sa.should_attend_at(0, 0));
        assert!(!sa.should_attend_at(0, 1));

        // Position 4: should attend to 2, 3, 4
        assert!(!sa.should_attend_at(4, 1));
        assert!(sa.should_attend_at(4, 2));
        assert!(sa.should_attend_at(4, 3));
        assert!(sa.should_attend_at(4, 4));
        assert!(!sa.should_attend_at(4, 5)); // Future
    }

    #[test]
    fn test_strided() {
        let sa = SparseAttention::new(
            AttentionPattern::Strided { stride: 4 },
            16,
        ).unwrap();

        // Position 10: attends to 0, 4, 8, 9, 10
        assert!(sa.should_attend_at(10, 0));   // stride
        assert!(sa.should_attend_at(10, 4));   // stride
        assert!(sa.should_attend_at(10, 8));   // stride
        assert!(sa.should_attend_at(10, 9));   // local
        assert!(sa.should_attend_at(10, 10));  // self
        assert!(!sa.should_attend_at(10, 1));  // not stride, not local
    }

    #[test]
    fn test_sparsity() {
        let full = SparseAttention::new(AttentionPattern::Full, 16).unwrap();
        let sparse = SparseAttention::new(
            AttentionPattern::SlidingWindow { window_size: 4 },
            16,
        ).unwrap();

        // Full should have all positions
        assert!(full.sparsity_ratio() > 0.99);

        // Sparse should save computation
        assert!(sparse.sparsity_ratio() < full.sparsity_ratio());
    }

    #[test]
    fn test_block_diagonal() {
        let sa = SparseAttention::new(
            AttentionPattern::BlockDiagonal { block_size: 4 },
            16,
        ).unwrap();

        // Position 5 (block 1): attends to 4, 5 only
        assert!(!sa.should_attend_at(5, 3)); // Block 0
        assert!(sa.should_attend_at(5, 4));  // Block 1
        assert!(sa.should_attend_at(5, 5));  // Block 1, self
        assert!(!sa.should_attend_at(5, 6)); // Block 1, future
        assert!(!sa.should_attend_at(5, 8)); // Block 2
    }

    #[test]
    fn test_bigbird() {
        let sa = SparseAttention::new(
            AttentionPattern::BigBird { window_size: 2, global_tokens: 2 },
            16,
        ).unwrap();

        // Position 10: attends to 0, 1 (global), 8, 9, 10 (window)
        assert!(sa.should_attend_at(10, 0));   // global
        assert!(sa.should_attend_at(10, 1));   // global
        assert!(!sa.should_attend_at(10, 5));  // neither
        assert!(sa.should_attend_at(10, 8));   // window
        assert!(sa.should_attend_at(10, 10));  // self
    }
}
