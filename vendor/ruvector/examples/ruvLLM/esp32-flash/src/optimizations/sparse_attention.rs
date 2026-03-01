//! Sparse Attention Patterns for ESP32

use heapless::Vec as HVec;

pub const MAX_SPARSE_SEQ: usize = 32;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttentionPattern {
    Full,
    SlidingWindow { window_size: usize },
    Strided { stride: usize },
    Longformer { window_size: usize, stride: usize },
    BlockDiagonal { block_size: usize },
    BigBird { window_size: usize, global_tokens: usize },
}

impl Default for AttentionPattern {
    fn default() -> Self { Self::SlidingWindow { window_size: 4 } }
}

pub struct SparseAttention {
    pattern: AttentionPattern,
    mask_data: HVec<u32, MAX_SPARSE_SEQ>,
    seq_len: usize,
}

impl SparseAttention {
    pub fn new(pattern: AttentionPattern, seq_len: usize) -> crate::Result<Self> {
        if seq_len > MAX_SPARSE_SEQ { return Err(crate::Error::BufferOverflow); }
        let mut sa = Self { pattern, mask_data: HVec::new(), seq_len };
        sa.build_mask()?;
        Ok(sa)
    }

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

    fn should_attend(&self, i: usize, j: usize) -> bool {
        match self.pattern {
            AttentionPattern::Full => true,
            AttentionPattern::SlidingWindow { window_size } => i.saturating_sub(window_size) <= j,
            AttentionPattern::Strided { stride } => j % stride == 0 || i.saturating_sub(1) <= j,
            AttentionPattern::Longformer { window_size, stride } =>
                i.saturating_sub(window_size) <= j || j % stride == 0,
            AttentionPattern::BlockDiagonal { block_size } => i / block_size == j / block_size,
            AttentionPattern::BigBird { window_size, global_tokens } =>
                i.saturating_sub(window_size) <= j || j < global_tokens,
        }
    }

    #[inline]
    pub fn should_attend_at(&self, i: usize, j: usize) -> bool {
        if i >= self.seq_len || j >= self.seq_len { return false; }
        (self.mask_data[i] >> j) & 1 == 1
    }

    #[inline]
    pub fn get_mask_row(&self, i: usize) -> u32 {
        self.mask_data.get(i).copied().unwrap_or(0)
    }

    pub fn sparse_qk(&self, query: &[i8], keys: &[&[i8]], scores: &mut [i32], query_pos: usize) {
        let mask = self.get_mask_row(query_pos);
        for (j, key) in keys.iter().enumerate() {
            if (mask >> j) & 1 == 1 {
                scores[j] = query.iter().zip(key.iter()).map(|(&q, &k)| q as i32 * k as i32).sum();
            } else {
                scores[j] = i32::MIN;
            }
        }
    }

    pub fn active_positions(&self) -> usize {
        self.mask_data.iter().map(|m| m.count_ones() as usize).sum()
    }

    pub fn sparsity_ratio(&self) -> f32 {
        let full = self.seq_len * (self.seq_len + 1) / 2;
        self.active_positions() as f32 / full as f32
    }
}

pub struct AttentionPatternCache {
    patterns: [Option<SparseAttention>; 4],
}

impl AttentionPatternCache {
    pub fn new_sliding(window: usize) -> Self {
        let p = AttentionPattern::SlidingWindow { window_size: window };
        Self {
            patterns: [
                SparseAttention::new(p, 8).ok(),
                SparseAttention::new(p, 16).ok(),
                SparseAttention::new(p, 24).ok(),
                SparseAttention::new(p, 32).ok(),
            ],
        }
    }

    pub fn get(&self, seq_len: usize) -> Option<&SparseAttention> {
        match seq_len {
            1..=8 => self.patterns[0].as_ref(),
            9..=16 => self.patterns[1].as_ref(),
            17..=24 => self.patterns[2].as_ref(),
            25..=32 => self.patterns[3].as_ref(),
            _ => None,
        }
    }
}
