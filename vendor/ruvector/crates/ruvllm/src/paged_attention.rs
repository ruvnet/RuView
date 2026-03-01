//! Paged Attention Mechanism
//!
//! Implements efficient memory management for attention computation inspired by
//! mistral.rs and vLLM. Uses a page table to manage KV cache blocks, enabling
//! efficient memory utilization and dynamic sequence lengths.
//!
//! ## Architecture
//!
//! ```text
//! +-------------------+     +-------------------+
//! | Page Table        |---->| Page Blocks       |
//! | [seq_id -> pages] |     | [KV pairs]        |
//! +-------------------+     +-------------------+
//!         |                         |
//!         v                         v
//! +-------------------+     +-------------------+
//! | Block Allocator   |     | Attention Kernel  |
//! | (free list)       |     | (paged attention) |
//! +-------------------+     +-------------------+
//! ```

use crate::error::{Result, RuvLLMError};
use dashmap::DashMap;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Configuration for paged attention
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedAttentionConfig {
    /// Number of tokens per page
    pub page_size: usize,
    /// Maximum number of pages per sequence
    pub max_pages_per_sequence: usize,
    /// Total page table capacity
    pub page_table_capacity: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Head dimension
    pub head_dim: usize,
    /// Number of KV heads (for GQA)
    pub num_kv_heads: usize,
    /// Block allocation strategy
    pub allocation_strategy: AllocationStrategy,
}

impl Default for PagedAttentionConfig {
    fn default() -> Self {
        Self {
            page_size: 16,
            max_pages_per_sequence: 256,
            page_table_capacity: 4096,
            num_heads: 32,
            head_dim: 128,
            num_kv_heads: 8,
            allocation_strategy: AllocationStrategy::FirstFit,
        }
    }
}

/// Block allocation strategies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationStrategy {
    /// Allocate the first available block
    FirstFit,
    /// Allocate the best fitting block
    BestFit,
    /// Allocate blocks in a round-robin fashion
    RoundRobin,
}

impl Default for AllocationStrategy {
    fn default() -> Self {
        Self::FirstFit
    }
}

/// A single page block containing KV pairs
#[derive(Debug)]
pub struct PageBlock {
    /// Block ID
    pub block_id: usize,
    /// Key values (shape: [page_size, num_kv_heads, head_dim])
    pub keys: Vec<f32>,
    /// Value values (shape: [page_size, num_kv_heads, head_dim])
    pub values: Vec<f32>,
    /// Number of tokens currently stored
    pub num_tokens: usize,
    /// Reference count for copy-on-write
    pub ref_count: AtomicUsize,
}

impl Clone for PageBlock {
    fn clone(&self) -> Self {
        Self {
            block_id: self.block_id,
            keys: self.keys.clone(),
            values: self.values.clone(),
            num_tokens: self.num_tokens,
            ref_count: AtomicUsize::new(self.ref_count.load(Ordering::SeqCst)),
        }
    }
}

impl PageBlock {
    /// Create a new page block
    pub fn new(block_id: usize, page_size: usize, num_kv_heads: usize, head_dim: usize) -> Self {
        let capacity = page_size * num_kv_heads * head_dim;
        Self {
            block_id,
            keys: vec![0.0; capacity],
            values: vec![0.0; capacity],
            num_tokens: 0,
            ref_count: AtomicUsize::new(1),
        }
    }

    /// Check if the block is full
    pub fn is_full(&self, page_size: usize) -> bool {
        self.num_tokens >= page_size
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self, page_size: usize) -> usize {
        page_size.saturating_sub(self.num_tokens)
    }

    /// Append KV pairs to the block
    pub fn append(
        &mut self,
        keys: &[f32],
        values: &[f32],
        num_kv_heads: usize,
        head_dim: usize,
    ) -> Result<usize> {
        let stride = num_kv_heads * head_dim;
        let num_tokens = keys.len() / stride;

        if keys.len() != values.len() {
            return Err(RuvLLMError::PagedAttention(
                "Key and value lengths must match".to_string(),
            ));
        }

        let start_offset = self.num_tokens * stride;
        let end_offset = start_offset + keys.len();

        if end_offset > self.keys.len() {
            return Err(RuvLLMError::PagedAttention("Block overflow".to_string()));
        }

        self.keys[start_offset..end_offset].copy_from_slice(keys);
        self.values[start_offset..end_offset].copy_from_slice(values);
        self.num_tokens += num_tokens;

        Ok(num_tokens)
    }
}

/// Page table entry for a sequence
#[derive(Debug, Clone)]
pub struct PageTableEntry {
    /// Sequence ID
    pub sequence_id: String,
    /// Block IDs in order
    pub block_ids: Vec<usize>,
    /// Total number of tokens
    pub total_tokens: usize,
}

/// Page table managing sequence-to-block mappings
#[derive(Debug)]
pub struct PageTable {
    /// Configuration
    config: PagedAttentionConfig,
    /// Sequence to page table entry mapping
    entries: DashMap<String, PageTableEntry>,
    /// All page blocks
    blocks: RwLock<Vec<PageBlock>>,
    /// Free block list
    free_blocks: RwLock<VecDeque<usize>>,
    /// Next block ID
    next_block_id: AtomicUsize,
}

impl PageTable {
    /// Create a new page table
    pub fn new(config: PagedAttentionConfig) -> Self {
        let mut blocks = Vec::with_capacity(config.page_table_capacity);
        let mut free_blocks = VecDeque::with_capacity(config.page_table_capacity);

        // Pre-allocate blocks
        for i in 0..config.page_table_capacity {
            blocks.push(PageBlock::new(
                i,
                config.page_size,
                config.num_kv_heads,
                config.head_dim,
            ));
            free_blocks.push_back(i);
        }

        Self {
            next_block_id: AtomicUsize::new(config.page_table_capacity),
            config,
            entries: DashMap::new(),
            blocks: RwLock::new(blocks),
            free_blocks: RwLock::new(free_blocks),
        }
    }

    /// Allocate a new block for a sequence
    pub fn allocate_block(&self, sequence_id: &str) -> Result<usize> {
        let mut free_blocks = self.free_blocks.write();

        let block_id = match self.config.allocation_strategy {
            AllocationStrategy::FirstFit => free_blocks.pop_front(),
            AllocationStrategy::BestFit | AllocationStrategy::RoundRobin => free_blocks.pop_front(),
        };

        let block_id = block_id
            .ok_or_else(|| RuvLLMError::OutOfMemory("No free blocks available".to_string()))?;

        // Update page table entry
        self.entries
            .entry(sequence_id.to_string())
            .or_insert_with(|| PageTableEntry {
                sequence_id: sequence_id.to_string(),
                block_ids: Vec::new(),
                total_tokens: 0,
            })
            .block_ids
            .push(block_id);

        Ok(block_id)
    }

    /// Free a block
    pub fn free_block(&self, block_id: usize) -> Result<()> {
        let mut blocks = self.blocks.write();
        let mut free_blocks = self.free_blocks.write();

        if block_id >= blocks.len() {
            return Err(RuvLLMError::PagedAttention(format!(
                "Invalid block ID: {}",
                block_id
            )));
        }

        // Reset the block
        blocks[block_id].num_tokens = 0;
        blocks[block_id].ref_count.store(1, Ordering::SeqCst);

        free_blocks.push_back(block_id);
        Ok(())
    }

    /// Free all blocks for a sequence
    pub fn free_sequence(&self, sequence_id: &str) -> Result<()> {
        if let Some((_, entry)) = self.entries.remove(sequence_id) {
            for block_id in entry.block_ids {
                self.free_block(block_id)?;
            }
        }
        Ok(())
    }

    /// Get blocks for a sequence
    pub fn get_blocks(&self, sequence_id: &str) -> Option<Vec<usize>> {
        self.entries.get(sequence_id).map(|e| e.block_ids.clone())
    }

    /// Append KV pairs to a sequence
    pub fn append_kv(&self, sequence_id: &str, keys: &[f32], values: &[f32]) -> Result<()> {
        let stride = self.config.num_kv_heads * self.config.head_dim;
        let num_tokens = keys.len() / stride;

        if keys.len() != values.len() {
            return Err(RuvLLMError::PagedAttention(
                "Key and value lengths must match".to_string(),
            ));
        }

        let mut remaining_tokens = num_tokens;
        let mut offset = 0;

        while remaining_tokens > 0 {
            // Get or allocate a block
            let block_id = {
                let entry = self.entries.get(sequence_id);
                match entry {
                    Some(e) if !e.block_ids.is_empty() => {
                        // SAFETY: We just checked !e.block_ids.is_empty()
                        let last_block_id = *e.block_ids.last().expect("block_ids is non-empty");
                        let blocks = self.blocks.read();
                        if blocks[last_block_id].is_full(self.config.page_size) {
                            drop(blocks);
                            drop(e);
                            self.allocate_block(sequence_id)?
                        } else {
                            last_block_id
                        }
                    }
                    _ => {
                        drop(entry);
                        self.allocate_block(sequence_id)?
                    }
                }
            };

            // Calculate how many tokens we can append
            let blocks = self.blocks.read();
            let capacity = blocks[block_id].remaining_capacity(self.config.page_size);
            drop(blocks);

            let tokens_to_append = remaining_tokens.min(capacity);
            let slice_size = tokens_to_append * stride;

            // Append to the block
            let mut blocks = self.blocks.write();
            blocks[block_id].append(
                &keys[offset..offset + slice_size],
                &values[offset..offset + slice_size],
                self.config.num_kv_heads,
                self.config.head_dim,
            )?;
            drop(blocks);

            // Update entry
            if let Some(mut entry) = self.entries.get_mut(sequence_id) {
                entry.total_tokens += tokens_to_append;
            }

            offset += slice_size;
            remaining_tokens -= tokens_to_append;
        }

        Ok(())
    }

    /// Get statistics
    pub fn stats(&self) -> PageTableStats {
        let free_blocks = self.free_blocks.read();
        PageTableStats {
            total_blocks: self.config.page_table_capacity,
            free_blocks: free_blocks.len(),
            active_sequences: self.entries.len(),
        }
    }
}

/// Page table statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PageTableStats {
    /// Total number of blocks
    pub total_blocks: usize,
    /// Number of free blocks
    pub free_blocks: usize,
    /// Number of active sequences
    pub active_sequences: usize,
}

/// Paged attention implementation
#[derive(Debug)]
pub struct PagedAttention {
    /// Configuration
    config: PagedAttentionConfig,
    /// Page table
    page_table: PageTable,
}

impl PagedAttention {
    /// Create a new paged attention instance
    pub fn new(config: PagedAttentionConfig) -> Self {
        let page_table = PageTable::new(config.clone());
        Self { config, page_table }
    }

    /// Allocate pages for a new sequence
    pub fn allocate_sequence(&self, sequence_id: &str, num_tokens: usize) -> Result<()> {
        let num_pages = (num_tokens + self.config.page_size - 1) / self.config.page_size;

        for _ in 0..num_pages {
            self.page_table.allocate_block(sequence_id)?;
        }

        Ok(())
    }

    /// Free a sequence's pages
    pub fn free_sequence(&self, sequence_id: &str) -> Result<()> {
        self.page_table.free_sequence(sequence_id)
    }

    /// Append KV pairs for a sequence
    pub fn append_kv(&self, sequence_id: &str, keys: &[f32], values: &[f32]) -> Result<()> {
        self.page_table.append_kv(sequence_id, keys, values)
    }

    /// Compute paged attention
    ///
    /// This is a simplified version - production would use optimized kernels
    pub fn forward(&self, query: &[f32], sequence_id: &str, scale: f32) -> Result<Vec<f32>> {
        let blocks = self.page_table.get_blocks(sequence_id).ok_or_else(|| {
            RuvLLMError::PagedAttention(format!("Sequence not found: {}", sequence_id))
        })?;

        if blocks.is_empty() {
            return Ok(vec![0.0; query.len()]);
        }

        // Simplified attention computation
        // In production, this would use optimized paged attention kernels
        let head_dim = self.config.head_dim;
        let num_heads = self.config.num_heads;
        let num_kv_heads = self.config.num_kv_heads;
        let gqa_ratio = num_heads / num_kv_heads;

        let mut output = vec![0.0; query.len()];

        // For each head
        for h in 0..num_heads {
            let kv_head = h / gqa_ratio;
            let q_offset = h * head_dim;
            let q_slice = &query[q_offset..q_offset + head_dim];

            let mut scores = Vec::new();
            let mut all_values = Vec::new();

            // Compute attention scores across all blocks
            let blocks_guard = self.page_table.blocks.read();
            for &block_id in &blocks {
                let block = &blocks_guard[block_id];
                for t in 0..block.num_tokens {
                    let kv_offset = (t * num_kv_heads + kv_head) * head_dim;
                    let k_slice = &block.keys[kv_offset..kv_offset + head_dim];
                    let v_slice = &block.values[kv_offset..kv_offset + head_dim];

                    // Dot product for attention score
                    let score: f32 = q_slice
                        .iter()
                        .zip(k_slice.iter())
                        .map(|(q, k)| q * k * scale)
                        .sum();

                    scores.push(score);
                    all_values.push(v_slice.to_vec());
                }
            }
            drop(blocks_guard);

            if scores.is_empty() {
                continue;
            }

            // Softmax
            let max_score = scores.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let exp_scores: Vec<f32> = scores.iter().map(|s| (s - max_score).exp()).collect();
            let sum_exp: f32 = exp_scores.iter().sum();
            let attn_weights: Vec<f32> = exp_scores.iter().map(|e| e / sum_exp).collect();

            // Weighted sum of values
            for (weight, values) in attn_weights.iter().zip(all_values.iter()) {
                for (i, v) in values.iter().enumerate() {
                    output[q_offset + i] += weight * v;
                }
            }
        }

        Ok(output)
    }

    /// Get page table statistics
    pub fn stats(&self) -> PageTableStats {
        self.page_table.stats()
    }

    /// Get the configuration
    pub fn config(&self) -> &PagedAttentionConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_block() {
        let mut block = PageBlock::new(0, 16, 8, 128);
        assert_eq!(block.num_tokens, 0);
        assert!(!block.is_full(16));
        assert_eq!(block.remaining_capacity(16), 16);
    }

    #[test]
    fn test_page_table() {
        let config = PagedAttentionConfig::default();
        let page_table = PageTable::new(config.clone());

        // Allocate a block
        let block_id = page_table.allocate_block("seq-1").unwrap();
        assert!(block_id < config.page_table_capacity);

        // Free the block
        page_table.free_block(block_id).unwrap();
    }

    #[test]
    fn test_paged_attention() {
        let config = PagedAttentionConfig {
            page_size: 4,
            num_heads: 2,
            head_dim: 4,
            num_kv_heads: 2,
            ..Default::default()
        };

        let attention = PagedAttention::new(config);

        // Append some KV pairs
        let keys = vec![1.0; 2 * 4]; // 1 token, 2 kv_heads, 4 head_dim
        let values = vec![1.0; 2 * 4];
        attention.append_kv("seq-1", &keys, &values).unwrap();

        // Forward pass
        let query = vec![1.0; 2 * 4]; // 2 heads, 4 head_dim
        let output = attention.forward(&query, "seq-1", 0.5).unwrap();
        assert_eq!(output.len(), 8);
    }
}
