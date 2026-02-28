//! WASM Batch Operations and TypedArray Optimizations
//!
//! Optimizations specific to WebAssembly execution:
//! - Batch FFI calls to minimize overhead
//! - Pre-allocated WASM memory
//! - TypedArray bulk transfers
//! - Memory alignment for SIMD
//!
//! Target: 10x reduction in FFI overhead

use crate::graph::VertexId;
use std::collections::HashMap;

/// Configuration for WASM batch operations
#[derive(Debug, Clone)]
pub struct BatchConfig {
    /// Maximum batch size
    pub max_batch_size: usize,
    /// Pre-allocated buffer size in bytes
    pub buffer_size: usize,
    /// Alignment for SIMD operations
    pub alignment: usize,
    /// Enable memory pooling
    pub memory_pooling: bool,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_batch_size: 1024,
            buffer_size: 64 * 1024, // 64KB
            alignment: 64,          // AVX-512 alignment
            memory_pooling: true,
        }
    }
}

/// Batch operation types for minimizing FFI calls
#[derive(Debug, Clone)]
pub enum BatchOperation {
    /// Insert multiple edges
    InsertEdges(Vec<(VertexId, VertexId, f64)>),
    /// Delete multiple edges
    DeleteEdges(Vec<(VertexId, VertexId)>),
    /// Update multiple weights
    UpdateWeights(Vec<(VertexId, VertexId, f64)>),
    /// Query multiple distances
    QueryDistances(Vec<(VertexId, VertexId)>),
    /// Compute cuts for multiple partitions
    ComputeCuts(Vec<Vec<VertexId>>),
}

/// Result from batch operation
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// Operation type
    pub operation: String,
    /// Number of items processed
    pub items_processed: usize,
    /// Time taken in microseconds
    pub time_us: u64,
    /// Results (for queries)
    pub results: Vec<f64>,
    /// Error message if any
    pub error: Option<String>,
}

/// TypedArray transfer for efficient WASM memory access
///
/// Provides aligned memory buffers for bulk data transfer between
/// JavaScript and WASM.
#[repr(C, align(64))]
pub struct TypedArrayTransfer {
    /// Float64 buffer for weights/distances
    pub f64_buffer: Vec<f64>,
    /// Uint64 buffer for vertex IDs
    pub u64_buffer: Vec<u64>,
    /// Uint32 buffer for indices/counts
    pub u32_buffer: Vec<u32>,
    /// Byte buffer for raw data
    pub byte_buffer: Vec<u8>,
    /// Current position in buffers
    position: usize,
}

impl TypedArrayTransfer {
    /// Create new transfer with default buffer size
    pub fn new() -> Self {
        Self::with_capacity(1024)
    }

    /// Create with specific capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            f64_buffer: Vec::with_capacity(capacity),
            u64_buffer: Vec::with_capacity(capacity),
            u32_buffer: Vec::with_capacity(capacity * 2),
            byte_buffer: Vec::with_capacity(capacity * 8),
            position: 0,
        }
    }

    /// Reset buffers for reuse
    pub fn reset(&mut self) {
        self.f64_buffer.clear();
        self.u64_buffer.clear();
        self.u32_buffer.clear();
        self.byte_buffer.clear();
        self.position = 0;
    }

    /// Add edge to transfer buffer
    pub fn add_edge(&mut self, source: VertexId, target: VertexId, weight: f64) {
        self.u64_buffer.push(source);
        self.u64_buffer.push(target);
        self.f64_buffer.push(weight);
    }

    /// Add vertex to transfer buffer
    pub fn add_vertex(&mut self, vertex: VertexId) {
        self.u64_buffer.push(vertex);
    }

    /// Add distance result
    pub fn add_distance(&mut self, distance: f64) {
        self.f64_buffer.push(distance);
    }

    /// Get edges from buffer
    pub fn get_edges(&self) -> Vec<(VertexId, VertexId, f64)> {
        let mut edges = Vec::with_capacity(self.f64_buffer.len());

        for (i, &weight) in self.f64_buffer.iter().enumerate() {
            let source = self.u64_buffer.get(i * 2).copied().unwrap_or(0);
            let target = self.u64_buffer.get(i * 2 + 1).copied().unwrap_or(0);
            edges.push((source, target, weight));
        }

        edges
    }

    /// Get f64 buffer as raw pointer (for FFI)
    pub fn f64_ptr(&self) -> *const f64 {
        self.f64_buffer.as_ptr()
    }

    /// Get u64 buffer as raw pointer (for FFI)
    pub fn u64_ptr(&self) -> *const u64 {
        self.u64_buffer.as_ptr()
    }

    /// Get buffer lengths
    pub fn len(&self) -> (usize, usize, usize) {
        (
            self.f64_buffer.len(),
            self.u64_buffer.len(),
            self.u32_buffer.len(),
        )
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.f64_buffer.is_empty() && self.u64_buffer.is_empty()
    }
}

impl Default for TypedArrayTransfer {
    fn default() -> Self {
        Self::new()
    }
}

/// WASM batch operations executor
pub struct WasmBatchOps {
    config: BatchConfig,
    /// Transfer buffer
    transfer: TypedArrayTransfer,
    /// Pending operations
    pending: Vec<BatchOperation>,
    /// Statistics
    total_ops: u64,
    total_items: u64,
    total_time_us: u64,
}

impl WasmBatchOps {
    /// Create new batch executor with default config
    pub fn new() -> Self {
        Self::with_config(BatchConfig::default())
    }

    /// Create with custom config
    pub fn with_config(config: BatchConfig) -> Self {
        Self {
            transfer: TypedArrayTransfer::with_capacity(config.buffer_size / 8),
            config,
            pending: Vec::new(),
            total_ops: 0,
            total_items: 0,
            total_time_us: 0,
        }
    }

    /// Queue edge insertions for batch processing
    pub fn queue_insert_edges(&mut self, edges: Vec<(VertexId, VertexId, f64)>) {
        if edges.len() > self.config.max_batch_size {
            // Split into multiple batches
            for chunk in edges.chunks(self.config.max_batch_size) {
                self.pending
                    .push(BatchOperation::InsertEdges(chunk.to_vec()));
            }
        } else {
            self.pending.push(BatchOperation::InsertEdges(edges));
        }
    }

    /// Queue edge deletions for batch processing
    pub fn queue_delete_edges(&mut self, edges: Vec<(VertexId, VertexId)>) {
        if edges.len() > self.config.max_batch_size {
            for chunk in edges.chunks(self.config.max_batch_size) {
                self.pending
                    .push(BatchOperation::DeleteEdges(chunk.to_vec()));
            }
        } else {
            self.pending.push(BatchOperation::DeleteEdges(edges));
        }
    }

    /// Queue distance queries for batch processing
    pub fn queue_distance_queries(&mut self, pairs: Vec<(VertexId, VertexId)>) {
        if pairs.len() > self.config.max_batch_size {
            for chunk in pairs.chunks(self.config.max_batch_size) {
                self.pending
                    .push(BatchOperation::QueryDistances(chunk.to_vec()));
            }
        } else {
            self.pending.push(BatchOperation::QueryDistances(pairs));
        }
    }

    /// Execute all pending operations
    pub fn execute_batch(&mut self) -> Vec<BatchResult> {
        let _start = std::time::Instant::now();

        // Drain pending operations to avoid borrow conflict
        let pending_ops: Vec<_> = self.pending.drain(..).collect();
        let mut results = Vec::with_capacity(pending_ops.len());

        for op in pending_ops {
            let op_start = std::time::Instant::now();
            let result = self.execute_operation(op);
            let elapsed = op_start.elapsed().as_micros() as u64;

            self.total_ops += 1;
            self.total_items += result.items_processed as u64;
            self.total_time_us += elapsed;

            results.push(result);
        }

        self.transfer.reset();
        results
    }

    /// Execute a single operation
    fn execute_operation(&mut self, op: BatchOperation) -> BatchResult {
        match op {
            BatchOperation::InsertEdges(edges) => {
                let count = edges.len();

                // Prepare transfer buffer
                self.transfer.reset();
                for (u, v, w) in &edges {
                    self.transfer.add_edge(*u, *v, *w);
                }

                // In WASM, this would call the native insert function
                // For now, we simulate the batch operation
                BatchResult {
                    operation: "InsertEdges".to_string(),
                    items_processed: count,
                    time_us: 0,
                    results: Vec::new(),
                    error: None,
                }
            }

            BatchOperation::DeleteEdges(edges) => {
                let count = edges.len();

                self.transfer.reset();
                for (u, v) in &edges {
                    self.transfer.add_vertex(*u);
                    self.transfer.add_vertex(*v);
                }

                BatchResult {
                    operation: "DeleteEdges".to_string(),
                    items_processed: count,
                    time_us: 0,
                    results: Vec::new(),
                    error: None,
                }
            }

            BatchOperation::UpdateWeights(updates) => {
                let count = updates.len();

                self.transfer.reset();
                for (u, v, w) in &updates {
                    self.transfer.add_edge(*u, *v, *w);
                }

                BatchResult {
                    operation: "UpdateWeights".to_string(),
                    items_processed: count,
                    time_us: 0,
                    results: Vec::new(),
                    error: None,
                }
            }

            BatchOperation::QueryDistances(pairs) => {
                let count = pairs.len();

                self.transfer.reset();
                for (u, v) in &pairs {
                    self.transfer.add_vertex(*u);
                    self.transfer.add_vertex(*v);
                }

                // Simulate distance results
                let results: Vec<f64> = pairs
                    .iter()
                    .map(|(u, v)| if u == v { 0.0 } else { 1.0 })
                    .collect();

                BatchResult {
                    operation: "QueryDistances".to_string(),
                    items_processed: count,
                    time_us: 0,
                    results,
                    error: None,
                }
            }

            BatchOperation::ComputeCuts(partitions) => {
                let count = partitions.len();

                BatchResult {
                    operation: "ComputeCuts".to_string(),
                    items_processed: count,
                    time_us: 0,
                    results: vec![0.0; count],
                    error: None,
                }
            }
        }
    }

    /// Get number of pending operations
    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    /// Get statistics
    pub fn stats(&self) -> BatchStats {
        BatchStats {
            total_operations: self.total_ops,
            total_items: self.total_items,
            total_time_us: self.total_time_us,
            avg_items_per_op: if self.total_ops > 0 {
                self.total_items as f64 / self.total_ops as f64
            } else {
                0.0
            },
            avg_time_per_item_us: if self.total_items > 0 {
                self.total_time_us as f64 / self.total_items as f64
            } else {
                0.0
            },
        }
    }

    /// Clear pending operations
    pub fn clear(&mut self) {
        self.pending.clear();
        self.transfer.reset();
    }
}

impl Default for WasmBatchOps {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for batch operations
#[derive(Debug, Clone, Default)]
pub struct BatchStats {
    /// Total operations executed
    pub total_operations: u64,
    /// Total items processed
    pub total_items: u64,
    /// Total time in microseconds
    pub total_time_us: u64,
    /// Average items per operation
    pub avg_items_per_op: f64,
    /// Average time per item in microseconds
    pub avg_time_per_item_us: f64,
}

/// Pre-allocated WASM memory region
#[repr(C, align(64))]
pub struct WasmMemoryRegion {
    /// Raw memory
    data: Vec<u8>,
    /// Capacity in bytes
    capacity: usize,
    /// Current offset
    offset: usize,
}

impl WasmMemoryRegion {
    /// Create new memory region
    pub fn new(size: usize) -> Self {
        // Round up to alignment
        let aligned_size = (size + 63) & !63;
        Self {
            data: vec![0u8; aligned_size],
            capacity: aligned_size,
            offset: 0,
        }
    }

    /// Allocate bytes from region, returns the offset
    ///
    /// Returns the starting offset of the allocated region.
    /// Use `get_slice` to access the allocated memory safely.
    pub fn alloc(&mut self, size: usize, align: usize) -> Option<usize> {
        // Align offset
        let aligned_offset = (self.offset + align - 1) & !(align - 1);

        if aligned_offset + size > self.capacity {
            return None;
        }

        let result = aligned_offset;
        self.offset = aligned_offset + size;
        Some(result)
    }

    /// Get a slice at the given offset
    pub fn get_slice(&self, offset: usize, len: usize) -> Option<&[u8]> {
        if offset + len <= self.capacity {
            Some(&self.data[offset..offset + len])
        } else {
            None
        }
    }

    /// Get a mutable slice at the given offset
    pub fn get_slice_mut(&mut self, offset: usize, len: usize) -> Option<&mut [u8]> {
        if offset + len <= self.capacity {
            Some(&mut self.data[offset..offset + len])
        } else {
            None
        }
    }

    /// Reset region for reuse
    pub fn reset(&mut self) {
        self.offset = 0;
        // Optional: zero memory
        // self.data.fill(0);
    }

    /// Get remaining capacity
    pub fn remaining(&self) -> usize {
        self.capacity - self.offset
    }

    /// Get used bytes
    pub fn used(&self) -> usize {
        self.offset
    }

    /// Get raw pointer
    pub fn as_ptr(&self) -> *const u8 {
        self.data.as_ptr()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_typed_array_transfer() {
        let mut transfer = TypedArrayTransfer::new();

        transfer.add_edge(1, 2, 1.0);
        transfer.add_edge(2, 3, 2.0);

        let edges = transfer.get_edges();
        assert_eq!(edges.len(), 2);
        assert_eq!(edges[0], (1, 2, 1.0));
        assert_eq!(edges[1], (2, 3, 2.0));
    }

    #[test]
    fn test_batch_queue() {
        let mut batch = WasmBatchOps::new();

        let edges = vec![(1, 2, 1.0), (2, 3, 2.0)];
        batch.queue_insert_edges(edges);

        assert_eq!(batch.pending_count(), 1);
    }

    #[test]
    fn test_batch_execute() {
        let mut batch = WasmBatchOps::new();

        batch.queue_insert_edges(vec![(1, 2, 1.0)]);
        batch.queue_delete_edges(vec![(3, 4)]);

        let results = batch.execute_batch();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].operation, "InsertEdges");
        assert_eq!(results[1].operation, "DeleteEdges");
        assert_eq!(batch.pending_count(), 0);
    }

    #[test]
    fn test_batch_splitting() {
        let mut batch = WasmBatchOps::with_config(BatchConfig {
            max_batch_size: 10,
            ..Default::default()
        });

        // Queue 25 edges
        let edges: Vec<_> = (0..25).map(|i| (i, i + 1, 1.0)).collect();
        batch.queue_insert_edges(edges);

        // Should be split into 3 batches
        assert_eq!(batch.pending_count(), 3);
    }

    #[test]
    fn test_distance_queries() {
        let mut batch = WasmBatchOps::new();

        batch.queue_distance_queries(vec![(1, 2), (2, 3), (1, 1)]);

        let results = batch.execute_batch();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].results.len(), 3);
        assert_eq!(results[0].results[2], 0.0); // Same vertex
    }

    #[test]
    fn test_wasm_memory_region() {
        let mut region = WasmMemoryRegion::new(1024);

        // Allocate 64-byte aligned
        let offset1 = region.alloc(100, 64);
        assert!(offset1.is_some());
        assert_eq!(offset1.unwrap() % 64, 0);

        let offset2 = region.alloc(200, 64);
        assert!(offset2.is_some());

        // Verify we can get slices
        let slice1 = region.get_slice(offset1.unwrap(), 100);
        assert!(slice1.is_some());

        assert!(region.used() > 0);
        assert!(region.remaining() < 1024);

        region.reset();
        assert_eq!(region.used(), 0);
    }

    #[test]
    fn test_batch_stats() {
        let mut batch = WasmBatchOps::new();

        batch.queue_insert_edges(vec![(1, 2, 1.0), (2, 3, 2.0)]);
        let _ = batch.execute_batch();

        let stats = batch.stats();
        assert_eq!(stats.total_operations, 1);
        assert_eq!(stats.total_items, 2);
    }

    #[test]
    fn test_transfer_reset() {
        let mut transfer = TypedArrayTransfer::new();

        transfer.add_edge(1, 2, 1.0);
        assert!(!transfer.is_empty());

        transfer.reset();
        assert!(transfer.is_empty());
    }
}
