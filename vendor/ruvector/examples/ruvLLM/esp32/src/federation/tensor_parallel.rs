//! Tensor Parallelism - Distributed Attention Heads
//!
//! Splits attention heads across chips for parallel computation.
//! Each chip handles a subset of heads, then results are combined.

use heapless::Vec as HVec;
use super::protocol::{ChipId, FederationMessage};

/// Maximum heads per chip
pub const MAX_HEADS_PER_CHIP: usize = 4;

/// Tensor parallel configuration
#[derive(Debug, Clone)]
pub struct TPConfig {
    /// Number of chips
    pub num_chips: usize,
    /// This chip's ID
    pub chip_id: ChipId,
    /// Total attention heads
    pub total_heads: usize,
    /// Heads handled by this chip
    pub my_heads: HVec<usize, MAX_HEADS_PER_CHIP>,
    /// Embedding dimension per head
    pub head_dim: usize,
}

impl TPConfig {
    /// Create config distributing heads across chips
    pub fn distribute_heads(
        chip_id: usize,
        num_chips: usize,
        total_heads: usize,
        head_dim: usize,
    ) -> Self {
        let mut my_heads = HVec::new();

        // Assign heads round-robin style
        for h in 0..total_heads {
            if h % num_chips == chip_id {
                let _ = my_heads.push(h);
            }
        }

        Self {
            num_chips,
            chip_id: ChipId(chip_id as u8),
            total_heads,
            my_heads,
            head_dim,
        }
    }
}

/// Tensor parallel attention node
pub struct TensorParallelNode {
    config: TPConfig,
    /// Partial attention outputs from each head
    partial_outputs: HVec<HVec<i32, 64>, MAX_HEADS_PER_CHIP>,
    /// Combined output buffer
    output_buffer: HVec<i32, 256>,
}

impl TensorParallelNode {
    pub fn new(config: TPConfig) -> Self {
        Self {
            config,
            partial_outputs: HVec::new(),
            output_buffer: HVec::new(),
        }
    }

    /// Get heads this chip handles
    pub fn my_heads(&self) -> &[usize] {
        &self.config.my_heads
    }

    /// Compute partial attention for assigned heads
    pub fn compute_partial_attention(
        &mut self,
        query: &[i8],
        keys: &[&[i8]],
        values: &[&[i8]],
    ) -> crate::Result<()> {
        self.partial_outputs.clear();

        for &head_idx in &self.config.my_heads {
            let mut head_output = HVec::new();

            // Compute Q @ K^T for this head
            let head_start = head_idx * self.config.head_dim;
            let head_end = head_start + self.config.head_dim;

            // Simplified attention: just dot product for now
            for &val in &values[0][head_start..head_end.min(values[0].len())] {
                head_output.push(val as i32).map_err(|_| crate::Error::BufferOverflow)?;
            }

            self.partial_outputs.push(head_output).map_err(|_| crate::Error::BufferOverflow)?;
        }

        Ok(())
    }

    /// Create message with partial results
    pub fn create_partial_result_message(&self, dst: ChipId, seq: u16) -> crate::Result<FederationMessage> {
        let mut data: Vec<i8> = Vec::new();

        for partial in &self.partial_outputs {
            for &val in partial {
                data.push((val >> 8) as i8); // Scale down
            }
        }

        FederationMessage::activation(
            self.config.chip_id,
            dst,
            seq,
            0, // Not layer-based
            0,
            &data,
        )
    }

    /// Memory saved vs single-chip
    pub fn memory_reduction(&self) -> f32 {
        self.config.num_chips as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_head_distribution() {
        // 4 heads across 5 chips
        let config0 = TPConfig::distribute_heads(0, 5, 4, 16);
        let config1 = TPConfig::distribute_heads(1, 5, 4, 16);

        // Chip 0 gets head 0, chip 1 gets head 1, etc.
        assert_eq!(config0.my_heads.as_slice(), &[0]);
        assert_eq!(config1.my_heads.as_slice(), &[1]);
    }
}
