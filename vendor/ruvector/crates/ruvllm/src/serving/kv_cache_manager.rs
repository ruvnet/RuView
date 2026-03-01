//! KV Cache Pool Management for Continuous Batching
//!
//! This module provides efficient KV cache slot allocation and management
//! for the continuous batching scheduler. It handles allocation, extension,
//! and freeing of cache slots for requests.

use super::request::RequestId;
use crate::error::{Result, RuvLLMError};
use crate::kv_cache::{KvCacheConfig, TwoTierKvCache};
use parking_lot::RwLock;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

/// Configuration for the KV cache pool
#[derive(Debug, Clone)]
pub struct KvCachePoolConfig {
    /// Number of slots in the pool
    pub num_slots: usize,
    /// Maximum sequence length per slot
    pub max_seq_len: usize,
    /// Block size for paged attention (tokens per block)
    pub block_size: usize,
    /// Total blocks available in the pool
    pub total_blocks: usize,
    /// Number of KV heads
    pub num_kv_heads: usize,
    /// Head dimension
    pub head_dim: usize,
    /// Number of layers
    pub num_layers: usize,
}

impl Default for KvCachePoolConfig {
    fn default() -> Self {
        Self {
            num_slots: 256,
            max_seq_len: 4096,
            block_size: 16,
            total_blocks: 4096,
            num_kv_heads: 8,
            head_dim: 128,
            num_layers: 32,
        }
    }
}

impl KvCachePoolConfig {
    /// Calculate blocks needed for a sequence length
    pub fn blocks_for_seq_len(&self, seq_len: usize) -> usize {
        (seq_len + self.block_size - 1) / self.block_size
    }

    /// Calculate memory per block in bytes
    pub fn bytes_per_block(&self) -> usize {
        // 2 for K and V, 4 bytes per f32 (or 2 for f16)
        2 * self.num_kv_heads * self.head_dim * self.block_size * self.num_layers * 2
    }

    /// Total pool memory in bytes
    pub fn total_memory(&self) -> usize {
        self.total_blocks * self.bytes_per_block()
    }
}

/// Allocation information for a request's KV cache
#[derive(Debug, Clone)]
pub struct KvCacheAllocation {
    /// Slot ID in the cache pool
    pub slot_id: usize,
    /// Current number of tokens in cache
    pub current_length: usize,
    /// Maximum allowed length
    pub max_length: usize,
    /// Allocated block indices
    pub block_table: Vec<usize>,
    /// Number of blocks allocated
    pub num_blocks: usize,
    /// Request ID that owns this allocation
    pub request_id: RequestId,
    /// Whether the allocation is active
    pub is_active: bool,
}

impl KvCacheAllocation {
    /// Create a new allocation
    pub fn new(slot_id: usize, request_id: RequestId, max_length: usize) -> Self {
        Self {
            slot_id,
            current_length: 0,
            max_length,
            block_table: Vec::new(),
            num_blocks: 0,
            request_id,
            is_active: true,
        }
    }

    /// Calculate remaining capacity
    pub fn remaining(&self) -> usize {
        self.max_length.saturating_sub(self.current_length)
    }

    /// Check if allocation can accommodate more tokens
    pub fn can_extend(&self, additional_tokens: usize) -> bool {
        self.current_length + additional_tokens <= self.max_length
    }
}

/// Manager for KV cache allocations
#[derive(Debug)]
pub struct KvCacheManager {
    /// Configuration
    config: KvCachePoolConfig,
    /// Request ID to allocation mapping
    allocations: RwLock<HashMap<RequestId, KvCacheAllocation>>,
    /// Free slot indices
    free_slots: RwLock<VecDeque<usize>>,
    /// Free block indices
    free_blocks: RwLock<VecDeque<usize>>,
    /// Number of active allocations
    active_allocations: AtomicUsize,
    /// Total allocated blocks
    allocated_blocks: AtomicUsize,
    /// Underlying KV cache storage (per slot)
    caches: Vec<Arc<TwoTierKvCache>>,
    /// Swapped out cache data (for preemption with swap mode)
    swap_space: RwLock<HashMap<RequestId, SwappedCache>>,
}

/// Swapped out cache data
#[derive(Debug, Clone)]
pub struct SwappedCache {
    /// Request ID
    pub request_id: RequestId,
    /// Original slot ID
    pub original_slot: usize,
    /// Keys
    pub keys: Vec<f32>,
    /// Values
    pub values: Vec<f32>,
    /// Sequence length when swapped
    pub seq_len: usize,
    /// Block table
    pub block_table: Vec<usize>,
}

impl KvCacheManager {
    /// Create a new KV cache manager
    pub fn new(config: KvCachePoolConfig) -> Self {
        // Initialize free slots
        let free_slots: VecDeque<usize> = (0..config.num_slots).collect();

        // Initialize free blocks
        let free_blocks: VecDeque<usize> = (0..config.total_blocks).collect();

        // Create underlying caches for each slot
        let kv_config = KvCacheConfig {
            tail_length: 256,
            max_tokens: config.max_seq_len,
            num_kv_heads: config.num_kv_heads,
            head_dim: config.head_dim,
            ..Default::default()
        };

        let caches: Vec<_> = (0..config.num_slots)
            .map(|_| Arc::new(TwoTierKvCache::new(kv_config.clone())))
            .collect();

        Self {
            config,
            allocations: RwLock::new(HashMap::new()),
            free_slots: RwLock::new(free_slots),
            free_blocks: RwLock::new(free_blocks),
            active_allocations: AtomicUsize::new(0),
            allocated_blocks: AtomicUsize::new(0),
            caches,
            swap_space: RwLock::new(HashMap::new()),
        }
    }

    /// Allocate a KV cache slot for a request
    pub fn allocate(&mut self, request_id: RequestId, max_tokens: usize) -> Result<usize> {
        let mut free_slots = self.free_slots.write();

        let slot_id = free_slots.pop_front().ok_or_else(|| {
            RuvLLMError::OutOfMemory("No free KV cache slots available".to_string())
        })?;

        // Calculate blocks needed
        let blocks_needed = self.config.blocks_for_seq_len(max_tokens);
        let mut free_blocks = self.free_blocks.write();

        if free_blocks.len() < blocks_needed {
            // Put slot back and return error
            free_slots.push_front(slot_id);
            return Err(RuvLLMError::OutOfMemory(format!(
                "Not enough blocks: need {}, have {}",
                blocks_needed,
                free_blocks.len()
            )));
        }

        // Allocate blocks
        let block_table: Vec<usize> = (0..blocks_needed)
            .filter_map(|_| free_blocks.pop_front())
            .collect();

        // Create allocation
        let mut allocation = KvCacheAllocation::new(slot_id, request_id, max_tokens);
        allocation.block_table = block_table.clone();
        allocation.num_blocks = blocks_needed;

        // Store allocation
        self.allocations.write().insert(request_id, allocation);
        self.active_allocations.fetch_add(1, Ordering::Relaxed);
        self.allocated_blocks
            .fetch_add(blocks_needed, Ordering::Relaxed);

        // Clear the cache slot
        self.caches[slot_id].clear();

        Ok(slot_id)
    }

    /// Extend an existing allocation with more tokens
    pub fn extend(&mut self, request_id: RequestId, new_tokens: usize) -> Result<()> {
        let mut allocations = self.allocations.write();

        let allocation = allocations.get_mut(&request_id).ok_or_else(|| {
            RuvLLMError::NotFound(format!("No allocation for request {}", request_id))
        })?;

        let new_length = allocation.current_length + new_tokens;

        if new_length > allocation.max_length {
            return Err(RuvLLMError::OutOfMemory(format!(
                "Cannot extend: {} + {} > {}",
                allocation.current_length, new_tokens, allocation.max_length
            )));
        }

        // Check if we need more blocks
        let current_blocks = allocation.num_blocks;
        let needed_blocks = self.config.blocks_for_seq_len(new_length);

        if needed_blocks > current_blocks {
            let additional_blocks = needed_blocks - current_blocks;
            let mut free_blocks = self.free_blocks.write();

            if free_blocks.len() < additional_blocks {
                return Err(RuvLLMError::OutOfMemory(format!(
                    "Not enough blocks to extend: need {}, have {}",
                    additional_blocks,
                    free_blocks.len()
                )));
            }

            // Allocate additional blocks
            for _ in 0..additional_blocks {
                if let Some(block) = free_blocks.pop_front() {
                    allocation.block_table.push(block);
                }
            }

            allocation.num_blocks = needed_blocks;
            self.allocated_blocks
                .fetch_add(additional_blocks, Ordering::Relaxed);
        }

        allocation.current_length = new_length;

        Ok(())
    }

    /// Free a KV cache allocation
    pub fn free(&mut self, request_id: RequestId) {
        let mut allocations = self.allocations.write();

        if let Some(allocation) = allocations.remove(&request_id) {
            // Return slot to free list
            self.free_slots.write().push_back(allocation.slot_id);

            // Return blocks to free list
            let mut free_blocks = self.free_blocks.write();
            for block in allocation.block_table {
                free_blocks.push_back(block);
            }

            self.active_allocations.fetch_sub(1, Ordering::Relaxed);
            self.allocated_blocks
                .fetch_sub(allocation.num_blocks, Ordering::Relaxed);

            // Clear the cache
            self.caches[allocation.slot_id].clear();
        }
    }

    /// Get the number of available slots
    pub fn available_slots(&self) -> usize {
        self.free_slots.read().len()
    }

    /// Get the number of available blocks
    pub fn available_blocks(&self) -> usize {
        self.free_blocks.read().len()
    }

    /// Check if there's capacity for a request
    pub fn can_allocate(&self, max_tokens: usize) -> bool {
        let slots_available = !self.free_slots.read().is_empty();
        let blocks_needed = self.config.blocks_for_seq_len(max_tokens);
        let blocks_available = self.free_blocks.read().len() >= blocks_needed;
        slots_available && blocks_available
    }

    /// Get allocation for a request
    pub fn get_allocation(&self, request_id: RequestId) -> Option<KvCacheAllocation> {
        self.allocations.read().get(&request_id).cloned()
    }

    /// Get the block table for a request
    pub fn get_block_table(&self, request_id: RequestId) -> Option<Vec<usize>> {
        self.allocations
            .read()
            .get(&request_id)
            .map(|a| a.block_table.clone())
    }

    /// Update the current length of an allocation
    pub fn set_length(&mut self, request_id: RequestId, length: usize) -> Result<()> {
        let mut allocations = self.allocations.write();

        let allocation = allocations.get_mut(&request_id).ok_or_else(|| {
            RuvLLMError::NotFound(format!("No allocation for request {}", request_id))
        })?;

        allocation.current_length = length;
        Ok(())
    }

    /// Swap out a request's KV cache to CPU memory
    pub fn swap_out(&mut self, request_id: RequestId) -> Result<()> {
        let allocation = {
            let allocations = self.allocations.read();
            allocations.get(&request_id).cloned().ok_or_else(|| {
                RuvLLMError::NotFound(format!("No allocation for request {}", request_id))
            })?
        };

        // Read KV data from cache
        let (keys, values) = self.caches[allocation.slot_id].get_all_kv();

        // Store in swap space
        let swapped = SwappedCache {
            request_id,
            original_slot: allocation.slot_id,
            keys,
            values,
            seq_len: allocation.current_length,
            block_table: allocation.block_table.clone(),
        };

        self.swap_space.write().insert(request_id, swapped);

        // Free the slot but keep the allocation record
        self.caches[allocation.slot_id].clear();
        self.free_slots.write().push_back(allocation.slot_id);

        // Return blocks
        let mut free_blocks = self.free_blocks.write();
        for block in &allocation.block_table {
            free_blocks.push_back(*block);
        }

        // Mark allocation as inactive
        if let Some(alloc) = self.allocations.write().get_mut(&request_id) {
            alloc.is_active = false;
        }

        Ok(())
    }

    /// Swap in a request's KV cache from CPU memory
    pub fn swap_in(&mut self, request_id: RequestId) -> Result<usize> {
        let swapped = self.swap_space.write().remove(&request_id).ok_or_else(|| {
            RuvLLMError::NotFound(format!("No swapped cache for request {}", request_id))
        })?;

        // Allocate a new slot
        let slot_id = {
            let mut free_slots = self.free_slots.write();
            free_slots
                .pop_front()
                .ok_or_else(|| RuvLLMError::OutOfMemory("No free slots for swap in".to_string()))?
        };

        // Allocate blocks
        let blocks_needed = self.config.blocks_for_seq_len(swapped.seq_len);
        let block_table = {
            let mut free_blocks = self.free_blocks.write();
            if free_blocks.len() < blocks_needed {
                // Put slot back
                self.free_slots.write().push_front(slot_id);
                return Err(RuvLLMError::OutOfMemory(
                    "Not enough blocks for swap in".to_string(),
                ));
            }

            (0..blocks_needed)
                .filter_map(|_| free_blocks.pop_front())
                .collect::<Vec<_>>()
        };

        // Restore KV data
        self.caches[slot_id].append(&swapped.keys, &swapped.values)?;

        // Update allocation
        if let Some(alloc) = self.allocations.write().get_mut(&request_id) {
            alloc.slot_id = slot_id;
            alloc.block_table = block_table;
            alloc.num_blocks = blocks_needed;
            alloc.is_active = true;
        }

        Ok(slot_id)
    }

    /// Check if a request has swapped cache
    pub fn is_swapped(&self, request_id: RequestId) -> bool {
        self.swap_space.read().contains_key(&request_id)
    }

    /// Get cache statistics
    pub fn stats(&self) -> KvCacheManagerStats {
        KvCacheManagerStats {
            total_slots: self.config.num_slots,
            free_slots: self.available_slots(),
            active_allocations: self.active_allocations.load(Ordering::Relaxed),
            total_blocks: self.config.total_blocks,
            free_blocks: self.available_blocks(),
            allocated_blocks: self.allocated_blocks.load(Ordering::Relaxed),
            swapped_requests: self.swap_space.read().len(),
            block_size: self.config.block_size,
            bytes_per_block: self.config.bytes_per_block(),
            total_memory: self.config.total_memory(),
        }
    }

    /// Get reference to the underlying cache for a slot
    pub fn get_cache(&self, slot_id: usize) -> Option<&Arc<TwoTierKvCache>> {
        self.caches.get(slot_id)
    }

    /// Get the configuration
    pub fn config(&self) -> &KvCachePoolConfig {
        &self.config
    }
}

/// Statistics for KV cache manager
#[derive(Debug, Clone, Default)]
pub struct KvCacheManagerStats {
    /// Total number of slots
    pub total_slots: usize,
    /// Number of free slots
    pub free_slots: usize,
    /// Number of active allocations
    pub active_allocations: usize,
    /// Total number of blocks
    pub total_blocks: usize,
    /// Number of free blocks
    pub free_blocks: usize,
    /// Number of allocated blocks
    pub allocated_blocks: usize,
    /// Number of swapped requests
    pub swapped_requests: usize,
    /// Tokens per block
    pub block_size: usize,
    /// Bytes per block
    pub bytes_per_block: usize,
    /// Total pool memory
    pub total_memory: usize,
}

impl KvCacheManagerStats {
    /// Calculate utilization as a ratio
    pub fn slot_utilization(&self) -> f64 {
        if self.total_slots > 0 {
            self.active_allocations as f64 / self.total_slots as f64
        } else {
            0.0
        }
    }

    /// Calculate block utilization as a ratio
    pub fn block_utilization(&self) -> f64 {
        if self.total_blocks > 0 {
            self.allocated_blocks as f64 / self.total_blocks as f64
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_manager() -> KvCacheManager {
        let config = KvCachePoolConfig {
            num_slots: 4,
            max_seq_len: 128,
            block_size: 16,
            total_blocks: 32,
            num_kv_heads: 2,
            head_dim: 64,
            num_layers: 4,
        };
        KvCacheManager::new(config)
    }

    #[test]
    fn test_allocation() {
        let mut manager = create_test_manager();
        let request_id = RequestId::new();

        let slot = manager.allocate(request_id, 64).unwrap();
        assert!(slot < 4);

        let allocation = manager.get_allocation(request_id).unwrap();
        assert_eq!(allocation.slot_id, slot);
        assert_eq!(allocation.max_length, 64);
        assert_eq!(allocation.current_length, 0);
    }

    #[test]
    fn test_extend() {
        let mut manager = create_test_manager();
        let request_id = RequestId::new();

        manager.allocate(request_id, 64).unwrap();
        manager.extend(request_id, 32).unwrap();

        let allocation = manager.get_allocation(request_id).unwrap();
        assert_eq!(allocation.current_length, 32);
    }

    #[test]
    fn test_free() {
        let mut manager = create_test_manager();
        let request_id = RequestId::new();

        let initial_slots = manager.available_slots();
        manager.allocate(request_id, 64).unwrap();
        assert_eq!(manager.available_slots(), initial_slots - 1);

        manager.free(request_id);
        assert_eq!(manager.available_slots(), initial_slots);
        assert!(manager.get_allocation(request_id).is_none());
    }

    #[test]
    fn test_out_of_slots() {
        let mut manager = create_test_manager();

        // Allocate all 4 slots
        for i in 0..4 {
            let id = RequestId::from_uuid(uuid::Uuid::from_u128(i as u128));
            manager.allocate(id, 32).unwrap();
        }

        // Fifth allocation should fail
        let result = manager.allocate(RequestId::new(), 32);
        assert!(result.is_err());
    }

    #[test]
    fn test_can_allocate() {
        let mut manager = create_test_manager();

        assert!(manager.can_allocate(64));

        // Allocate all slots
        for i in 0..4 {
            let id = RequestId::from_uuid(uuid::Uuid::from_u128(i as u128));
            manager.allocate(id, 32).unwrap();
        }

        assert!(!manager.can_allocate(64));
    }

    #[test]
    fn test_stats() {
        let mut manager = create_test_manager();
        let request_id = RequestId::new();

        manager.allocate(request_id, 64).unwrap();

        let stats = manager.stats();
        assert_eq!(stats.total_slots, 4);
        assert_eq!(stats.free_slots, 3);
        assert_eq!(stats.active_allocations, 1);
    }
}
