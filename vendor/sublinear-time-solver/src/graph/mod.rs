//! Graph representations and algorithms for the sublinear time solver
//!
//! This module provides efficient graph data structures optimized for
//! push-based algorithms and random walk computations.

pub mod adjacency;

use std::collections::HashMap;
use bit_set::BitSet;

/// A node identifier in the graph
pub type NodeId = usize;

/// Weight type for edges
pub type Weight = f64;

/// Graph trait for different implementations
pub trait Graph {
    /// Number of nodes in the graph
    fn num_nodes(&self) -> usize;
    
    /// Number of edges in the graph
    fn num_edges(&self) -> usize;
    
    /// Get neighbors of a node with their weights
    fn neighbors(&self, node: NodeId) -> Vec<(NodeId, Weight)>;
    
    /// Get the degree of a node (sum of edge weights)
    fn degree(&self, node: NodeId) -> Weight;
    
    /// Check if an edge exists between two nodes
    fn has_edge(&self, from: NodeId, to: NodeId) -> bool;
    
    /// Get the weight of an edge if it exists
    fn edge_weight(&self, from: NodeId, to: NodeId) -> Option<Weight>;
}

/// Sparse matrix representation using Compressed Sparse Row format
#[derive(Debug, Clone)]
pub struct CompressedSparseRow {
    /// Number of rows in the matrix
    pub nrows: usize,
    /// Number of columns in the matrix
    pub ncols: usize,
    /// Row pointers (length = nrows + 1)
    pub row_ptr: Vec<usize>,
    /// Column indices of non-zero elements
    pub col_indices: Vec<usize>,
    /// Values of non-zero elements
    pub values: Vec<f64>,
}

impl CompressedSparseRow {
    /// Create a new empty CSR matrix
    pub fn new(nrows: usize, ncols: usize) -> Self {
        Self {
            nrows,
            ncols,
            row_ptr: vec![0; nrows + 1],
            col_indices: Vec::new(),
            values: Vec::new(),
        }
    }
    
    /// Get the number of non-zero elements
    pub fn nnz(&self) -> usize {
        self.values.len()
    }
    
    /// Get neighbors of a row with their values
    pub fn row(&self, row: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let start = self.row_ptr[row];
        let end = self.row_ptr[row + 1];
        
        (start..end).map(move |idx| {
            (self.col_indices[idx], self.values[idx])
        })
    }
    
    /// Compute row sums (degrees for graphs)
    pub fn row_sums(&self) -> Vec<f64> {
        let mut sums = vec![0.0; self.nrows];
        for row in 0..self.nrows {
            for (_, value) in self.row(row) {
                sums[row] += value;
            }
        }
        sums
    }
    
    /// Transpose the matrix
    pub fn transpose(&self) -> Self {
        let mut col_counts = vec![0; self.ncols];
        
        // Count elements per column
        for &col in &self.col_indices {
            col_counts[col] += 1;
        }
        
        // Build row pointers for transposed matrix
        let mut new_row_ptr = vec![0; self.ncols + 1];
        for i in 0..self.ncols {
            new_row_ptr[i + 1] = new_row_ptr[i] + col_counts[i];
        }
        
        let mut new_col_indices = vec![0; self.nnz()];
        let mut new_values = vec![0.0; self.nnz()];
        let mut col_positions = new_row_ptr.clone();
        
        // Fill transposed matrix
        for row in 0..self.nrows {
            for (col, value) in self.row(row) {
                let pos = col_positions[col];
                new_col_indices[pos] = row;
                new_values[pos] = value;
                col_positions[col] += 1;
            }
        }
        
        Self {
            nrows: self.ncols,
            ncols: self.nrows,
            row_ptr: new_row_ptr,
            col_indices: new_col_indices,
            values: new_values,
        }
    }
}

/// Work queue for push algorithms with priority-based processing
#[derive(Debug)]
pub struct WorkQueue {
    /// Binary heap for priority queue
    heap: std::collections::BinaryHeap<WorkItem>,
    /// Bit set to track which nodes are in the queue
    in_queue: BitSet,
    /// Threshold for adding items to queue
    threshold: f64,
}

#[derive(Debug, PartialEq, PartialOrd)]
struct WorkItem {
    /// Priority value (higher = more important)
    priority: OrderedFloat,
    /// Node identifier
    node_id: usize,
}

/// Wrapper for f64 to enable ordering
#[derive(Debug, PartialEq, PartialOrd)]
struct OrderedFloat(f64);

impl Eq for OrderedFloat {}
impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl WorkQueue {
    /// Create a new work queue with given threshold
    pub fn new(threshold: f64) -> Self {
        Self {
            heap: std::collections::BinaryHeap::new(),
            in_queue: BitSet::new(),
            threshold,
        }
    }
    
    /// Push a node if it meets the threshold
    pub fn push_if_threshold(&mut self, node: usize, residual: f64, degree: f64) {
        let priority = if degree > 0.0 { residual / degree } else { residual };
        
        if priority >= self.threshold && !self.in_queue.contains(node) {
            self.heap.push(WorkItem {
                priority: OrderedFloat(priority),
                node_id: node,
            });
            self.in_queue.insert(node);
        }
    }
    
    /// Pop the highest priority item
    pub fn pop(&mut self) -> Option<(usize, f64)> {
        if let Some(item) = self.heap.pop() {
            self.in_queue.remove(item.node_id);
            Some((item.node_id, item.priority.0))
        } else {
            None
        }
    }
    
    /// Check if queue is empty
    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }
    
    /// Get current queue size
    pub fn len(&self) -> usize {
        self.heap.len()
    }
    
    /// Adaptively adjust threshold based on queue size
    pub fn adaptive_threshold(&mut self, max_queue_size: usize, min_queue_size: usize) {
        let current_size = self.len();
        
        if current_size > max_queue_size {
            self.threshold *= 1.1;  // Increase threshold to reduce queue size
        } else if current_size < min_queue_size && self.threshold > 1e-12 {
            self.threshold *= 0.9;  // Decrease threshold to increase queue size
        }
    }
}

/// Tracker for visited nodes during push operations
#[derive(Debug)]
pub struct VisitedTracker {
    /// Bit set for fast membership testing
    visited: BitSet,
    /// Order in which nodes were visited
    visit_order: Vec<usize>,
    /// Timestamps for each node
    timestamps: Vec<u32>,
    /// Current timestamp
    current_time: u32,
}

impl VisitedTracker {
    /// Create a new visited tracker for n nodes
    pub fn new(n: usize) -> Self {
        Self {
            visited: BitSet::new(),
            visit_order: Vec::new(),
            timestamps: vec![0; n],
            current_time: 1,
        }
    }
    
    /// Mark a node as visited, returns true if newly visited
    pub fn mark_visited(&mut self, node: usize) -> bool {
        if !self.visited.contains(node) {
            self.visited.insert(node);
            self.visit_order.push(node);
            if node < self.timestamps.len() {
                self.timestamps[node] = self.current_time;
            }
            true
        } else {
            false
        }
    }
    
    /// Check if a node has been visited
    pub fn is_visited(&self, node: usize) -> bool {
        self.visited.contains(node)
    }
    
    /// Get all visited nodes in order
    pub fn visited_nodes(&self) -> &[usize] {
        &self.visit_order
    }
    
    /// Reset for a new query (incremental timestamp)
    pub fn reset_for_new_query(&mut self) {
        self.current_time += 1;
        self.visit_order.clear();
        
        // Full reset if timestamp overflow
        if self.current_time == u32::MAX {
            self.full_reset();
        }
    }
    
    /// Complete reset of all tracking data
    fn full_reset(&mut self) {
        self.visited.clear();
        self.visit_order.clear();
        self.timestamps.fill(0);
        self.current_time = 1;
    }
    
    /// Get number of visited nodes
    pub fn num_visited(&self) -> usize {
        self.visit_order.len()
    }
}

pub use adjacency::AdjacencyList;
