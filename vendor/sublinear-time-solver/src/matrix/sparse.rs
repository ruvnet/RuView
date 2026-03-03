//! Sparse matrix storage implementations.
//!
//! This module provides efficient storage formats for sparse matrices,
//! including CSR, CSC, COO, and graph adjacency representations.

use crate::types::{Precision, DimensionType, IndexType, NodeId};
use crate::error::{SolverError, Result};
use alloc::{vec::Vec, collections::BTreeMap};
use core::iter;

/// Compressed Sparse Row (CSR) storage format.
/// 
/// Efficient for row-wise operations and matrix-vector multiplication.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CSRStorage {
    /// Non-zero values in row-major order
    pub values: Vec<Precision>,
    /// Column indices corresponding to values
    pub col_indices: Vec<IndexType>,
    /// Row pointers: row_ptr[i] is the start of row i in values/col_indices
    pub row_ptr: Vec<IndexType>,
}

/// Compressed Sparse Column (CSC) storage format.
/// 
/// Efficient for column-wise operations.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CSCStorage {
    /// Non-zero values in column-major order
    pub values: Vec<Precision>,
    /// Row indices corresponding to values
    pub row_indices: Vec<IndexType>,
    /// Column pointers: col_ptr[j] is the start of column j in values/row_indices
    pub col_ptr: Vec<IndexType>,
}

/// Coordinate (COO) storage format.
/// 
/// Efficient for construction and random access patterns.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct COOStorage {
    /// Row indices
    pub row_indices: Vec<IndexType>,
    /// Column indices
    pub col_indices: Vec<IndexType>,
    /// Values
    pub values: Vec<Precision>,
}

/// Graph adjacency list storage format.
/// 
/// Optimized for graph algorithms like push methods.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphStorage {
    /// Outgoing edges for each node
    pub out_edges: Vec<Vec<GraphEdge>>,
    /// Incoming edges for each node (for backward push)
    pub in_edges: Vec<Vec<GraphEdge>>,
    /// Node degrees for normalization
    pub degrees: Vec<Precision>,
}

/// Graph edge representation.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GraphEdge {
    /// Target node
    pub target: NodeId,
    /// Edge weight
    pub weight: Precision,
}

// CSR Implementation
impl CSRStorage {
    /// Create CSR storage from COO format.
    pub fn from_coo(coo: &COOStorage, rows: DimensionType, cols: DimensionType) -> Result<Self> {
        if coo.is_empty() {
            return Ok(Self {
                values: Vec::new(),
                col_indices: Vec::new(),
                row_ptr: vec![0; rows + 1],
            });
        }
        
        // Sort by row, then by column
        let mut sorted_entries: Vec<_> = coo.row_indices.iter()
            .zip(&coo.col_indices)
            .zip(&coo.values)
            .map(|((&r, &c), &v)| (r as usize, c, v))
            .collect();
        sorted_entries.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        
        let mut values = Vec::new();
        let mut col_indices = Vec::new();
        let mut row_ptr = vec![0; rows + 1];
        
        let mut current_row = 0;
        let mut nnz_count = 0;
        
        for (row, col, value) in sorted_entries {
            // Skip zeros
            if value == 0.0 {
                continue;
            }
            
            // Update row pointers
            while current_row < row {
                current_row += 1;
                row_ptr[current_row] = nnz_count as IndexType;
            }
            
            values.push(value);
            col_indices.push(col);
            nnz_count += 1;
        }
        
        // Finalize remaining row pointers
        while current_row < rows {
            current_row += 1;
            row_ptr[current_row] = nnz_count as IndexType;
        }
        
        Ok(Self {
            values,
            col_indices,
            row_ptr,
        })
    }
    
    /// Create CSR storage from CSC format.
    pub fn from_csc(csc: &CSCStorage, rows: DimensionType, cols: DimensionType) -> Result<Self> {
        let triplets = csc.to_triplets()?;
        let coo = COOStorage::from_triplets(triplets)?;
        Self::from_coo(&coo, rows, cols)
    }
    
    /// Get matrix element at (row, col).
    pub fn get(&self, row: usize, col: usize) -> Option<Precision> {
        if row >= self.row_ptr.len() - 1 {
            return None;
        }
        
        let start = self.row_ptr[row] as usize;
        let end = self.row_ptr[row + 1] as usize;
        
        // Binary search for the column
        match self.col_indices[start..end].binary_search(&(col as IndexType)) {
            Ok(pos) => Some(self.values[start + pos]),
            Err(_) => None,
        }
    }
    
    /// Iterate over non-zero elements in a row.
    pub fn row_iter(&self, row: usize) -> CSRRowIter {
        if row >= self.row_ptr.len() - 1 {
            return CSRRowIter {
                col_indices: &[],
                values: &[],
                pos: 0,
            };
        }
        
        let start = self.row_ptr[row] as usize;
        let end = self.row_ptr[row + 1] as usize;
        
        CSRRowIter {
            col_indices: &self.col_indices[start..end],
            values: &self.values[start..end],
            pos: 0,
        }
    }
    
    /// Iterate over non-zero elements in a column (slow for CSR).
    pub fn col_iter(&self, col: usize) -> CSRColIter {
        CSRColIter {
            storage: self,
            col: col as IndexType,
            row: 0,
        }
    }
    
    /// Matrix-vector multiplication: result = A * x
    pub fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.multiply_vector_add(x, result);
    }
    
    /// Matrix-vector multiplication with accumulation: result += A * x
    pub fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) {
        for (row, mut row_sum) in result.iter_mut().enumerate() {
            let start = self.row_ptr[row] as usize;
            let end = self.row_ptr[row + 1] as usize;
            
            for i in start..end {
                let col = self.col_indices[i] as usize;
                *row_sum += self.values[i] * x[col];
            }
        }
    }
    
    /// Get number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }
    
    /// Extract as coordinate triplets.
    pub fn to_triplets(&self) -> Result<Vec<(usize, usize, Precision)>> {
        let mut triplets = Vec::new();
        
        for row in 0..self.row_ptr.len() - 1 {
            let start = self.row_ptr[row] as usize;
            let end = self.row_ptr[row + 1] as usize;
            
            for i in start..end {
                let col = self.col_indices[i] as usize;
                let value = self.values[i];
                triplets.push((row, col, value));
            }
        }
        
        Ok(triplets)
    }
    
    /// Scale all values by a factor.
    pub fn scale(&mut self, factor: Precision) {
        for value in &mut self.values {
            *value *= factor;
        }
    }
    
    /// Add a value to the diagonal.
    pub fn add_diagonal(&mut self, alpha: Precision) {
        for row in 0..self.row_ptr.len() - 1 {
            let start = self.row_ptr[row] as usize;
            let end = self.row_ptr[row + 1] as usize;
            
            // Look for diagonal element
            if let Ok(pos) = self.col_indices[start..end].binary_search(&(row as IndexType)) {
                self.values[start + pos] += alpha;
            }
            // Note: If diagonal element doesn't exist, we'd need to restructure the matrix
        }
    }
}

/// Iterator over non-zero elements in a CSR row.
pub struct CSRRowIter<'a> {
    col_indices: &'a [IndexType],
    values: &'a [Precision],
    pos: usize,
}

impl<'a> Iterator for CSRRowIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.col_indices.len() {
            let col = self.col_indices[self.pos];
            let val = self.values[self.pos];
            self.pos += 1;
            Some((col, val))
        } else {
            None
        }
    }
}

/// Iterator over non-zero elements in a CSR column (inefficient).
pub struct CSRColIter<'a> {
    storage: &'a CSRStorage,
    col: IndexType,
    row: usize,
}

impl<'a> Iterator for CSRColIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.row < self.storage.row_ptr.len() - 1 {
            let start = self.storage.row_ptr[self.row] as usize;
            let end = self.storage.row_ptr[self.row + 1] as usize;
            
            if let Ok(pos) = self.storage.col_indices[start..end].binary_search(&self.col) {
                let value = self.storage.values[start + pos];
                let row = self.row as IndexType;
                self.row += 1;
                return Some((row, value));
            }
            
            self.row += 1;
        }
        None
    }
}

// CSC Implementation
impl CSCStorage {
    /// Create CSC storage from COO format.
    pub fn from_coo(coo: &COOStorage, rows: DimensionType, cols: DimensionType) -> Result<Self> {
        if coo.is_empty() {
            return Ok(Self {
                values: Vec::new(),
                row_indices: Vec::new(),
                col_ptr: vec![0; cols + 1],
            });
        }
        
        // Sort by column, then by row
        let mut sorted_entries: Vec<_> = coo.row_indices.iter()
            .zip(&coo.col_indices)
            .zip(&coo.values)
            .map(|((&r, &c), &v)| (r, c as usize, v))
            .collect();
        sorted_entries.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
        
        let mut values = Vec::new();
        let mut row_indices = Vec::new();
        let mut col_ptr = vec![0; cols + 1];
        
        let mut current_col = 0;
        let mut nnz_count = 0;
        
        for (row, col, value) in sorted_entries {
            // Skip zeros
            if value == 0.0 {
                continue;
            }
            
            // Update column pointers
            while current_col < col {
                current_col += 1;
                col_ptr[current_col] = nnz_count as IndexType;
            }
            
            values.push(value);
            row_indices.push(row);
            nnz_count += 1;
        }
        
        // Finalize remaining column pointers
        while current_col < cols {
            current_col += 1;
            col_ptr[current_col] = nnz_count as IndexType;
        }
        
        Ok(Self {
            values,
            row_indices,
            col_ptr,
        })
    }
    
    /// Create CSC storage from CSR format.
    pub fn from_csr(csr: &CSRStorage, rows: DimensionType, cols: DimensionType) -> Result<Self> {
        let triplets = csr.to_triplets()?;
        let coo = COOStorage::from_triplets(triplets)?;
        Self::from_coo(&coo, rows, cols)
    }
    
    /// Get matrix element at (row, col).
    pub fn get(&self, row: usize, col: usize) -> Option<Precision> {
        if col >= self.col_ptr.len() - 1 {
            return None;
        }
        
        let start = self.col_ptr[col] as usize;
        let end = self.col_ptr[col + 1] as usize;
        
        // Binary search for the row
        match self.row_indices[start..end].binary_search(&(row as IndexType)) {
            Ok(pos) => Some(self.values[start + pos]),
            Err(_) => None,
        }
    }
    
    /// Iterate over non-zero elements in a row (slow for CSC).
    pub fn row_iter(&self, row: usize) -> CSCRowIter {
        CSCRowIter {
            storage: self,
            row: row as IndexType,
            col: 0,
        }
    }
    
    /// Iterate over non-zero elements in a column.
    pub fn col_iter(&self, col: usize) -> CSCColIter {
        if col >= self.col_ptr.len() - 1 {
            return CSCColIter {
                row_indices: &[],
                values: &[],
                pos: 0,
            };
        }
        
        let start = self.col_ptr[col] as usize;
        let end = self.col_ptr[col + 1] as usize;
        
        CSCColIter {
            row_indices: &self.row_indices[start..end],
            values: &self.values[start..end],
            pos: 0,
        }
    }
    
    /// Matrix-vector multiplication: result = A * x
    pub fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.multiply_vector_add(x, result);
    }
    
    /// Matrix-vector multiplication with accumulation: result += A * x
    pub fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) {
        for col in 0..self.col_ptr.len() - 1 {
            let x_col = x[col];
            if x_col == 0.0 {
                continue;
            }
            
            let start = self.col_ptr[col] as usize;
            let end = self.col_ptr[col + 1] as usize;
            
            for i in start..end {
                let row = self.row_indices[i] as usize;
                result[row] += self.values[i] * x_col;
            }
        }
    }
    
    /// Get number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }
    
    /// Extract as coordinate triplets.
    pub fn to_triplets(&self) -> Result<Vec<(usize, usize, Precision)>> {
        let mut triplets = Vec::new();
        
        for col in 0..self.col_ptr.len() - 1 {
            let start = self.col_ptr[col] as usize;
            let end = self.col_ptr[col + 1] as usize;
            
            for i in start..end {
                let row = self.row_indices[i] as usize;
                let value = self.values[i];
                triplets.push((row, col, value));
            }
        }
        
        Ok(triplets)
    }
    
    /// Scale all values by a factor.
    pub fn scale(&mut self, factor: Precision) {
        for value in &mut self.values {
            *value *= factor;
        }
    }
    
    /// Add a value to the diagonal.
    pub fn add_diagonal(&mut self, alpha: Precision) {
        for col in 0..self.col_ptr.len() - 1 {
            let start = self.col_ptr[col] as usize;
            let end = self.col_ptr[col + 1] as usize;
            
            // Look for diagonal element
            if let Ok(pos) = self.row_indices[start..end].binary_search(&(col as IndexType)) {
                self.values[start + pos] += alpha;
            }
        }
    }
}

/// Iterator over non-zero elements in a CSC row (inefficient).
pub struct CSCRowIter<'a> {
    storage: &'a CSCStorage,
    row: IndexType,
    col: usize,
}

impl<'a> Iterator for CSCRowIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.col < self.storage.col_ptr.len() - 1 {
            let start = self.storage.col_ptr[self.col] as usize;
            let end = self.storage.col_ptr[self.col + 1] as usize;
            
            if let Ok(pos) = self.storage.row_indices[start..end].binary_search(&self.row) {
                let value = self.storage.values[start + pos];
                let col = self.col as IndexType;
                self.col += 1;
                return Some((col, value));
            }
            
            self.col += 1;
        }
        None
    }
}

/// Iterator over non-zero elements in a CSC column.
pub struct CSCColIter<'a> {
    row_indices: &'a [IndexType],
    values: &'a [Precision],
    pos: usize,
}

impl<'a> Iterator for CSCColIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.row_indices.len() {
            let row = self.row_indices[self.pos];
            let val = self.values[self.pos];
            self.pos += 1;
            Some((row, val))
        } else {
            None
        }
    }
}

// COO Implementation
impl COOStorage {
    /// Create COO storage from triplets.
    pub fn from_triplets(triplets: Vec<(usize, usize, Precision)>) -> Result<Self> {
        let mut row_indices = Vec::new();
        let mut col_indices = Vec::new();
        let mut values = Vec::new();
        
        for (row, col, value) in triplets {
            if value != 0.0 {
                row_indices.push(row as IndexType);
                col_indices.push(col as IndexType);
                values.push(value);
            }
        }
        
        Ok(Self {
            row_indices,
            col_indices,
            values,
        })
    }
    
    /// Check if the storage is empty.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
    
    /// Get matrix element at (row, col) - O(n) search.
    pub fn get(&self, row: usize, col: usize) -> Option<Precision> {
        for i in 0..self.values.len() {
            if self.row_indices[i] as usize == row && self.col_indices[i] as usize == col {
                return Some(self.values[i]);
            }
        }
        None
    }
    
    /// Iterate over non-zero elements in a row.
    pub fn row_iter(&self, row: usize) -> COORowIter {
        COORowIter {
            storage: self,
            target_row: row as IndexType,
            pos: 0,
        }
    }
    
    /// Iterate over non-zero elements in a column.
    pub fn col_iter(&self, col: usize) -> COOColIter {
        COOColIter {
            storage: self,
            target_col: col as IndexType,
            pos: 0,
        }
    }
    
    /// Matrix-vector multiplication: result = A * x
    pub fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.multiply_vector_add(x, result);
    }
    
    /// Matrix-vector multiplication with accumulation: result += A * x
    pub fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) {
        for i in 0..self.values.len() {
            let row = self.row_indices[i] as usize;
            let col = self.col_indices[i] as usize;
            result[row] += self.values[i] * x[col];
        }
    }
    
    /// Get number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.values.len()
    }
    
    /// Extract as coordinate triplets.
    pub fn to_triplets(&self) -> Vec<(usize, usize, Precision)> {
        self.row_indices.iter()
            .zip(&self.col_indices)
            .zip(&self.values)
            .map(|((&r, &c), &v)| (r as usize, c as usize, v))
            .collect()
    }
    
    /// Scale all values by a factor.
    pub fn scale(&mut self, factor: Precision) {
        for value in &mut self.values {
            *value *= factor;
        }
    }
    
    /// Add a value to the diagonal.
    pub fn add_diagonal(&mut self, alpha: Precision, rows: DimensionType) {
        // For COO, we'd need to add new diagonal entries if they don't exist
        // This is a simplified implementation that only modifies existing diagonal entries
        for i in 0..self.values.len() {
            if self.row_indices[i] == self.col_indices[i] {
                self.values[i] += alpha;
            }
        }
    }
}

/// Iterator over non-zero elements in a COO row.
pub struct COORowIter<'a> {
    storage: &'a COOStorage,
    target_row: IndexType,
    pos: usize,
}

impl<'a> Iterator for COORowIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.storage.values.len() {
            if self.storage.row_indices[self.pos] == self.target_row {
                let col = self.storage.col_indices[self.pos];
                let val = self.storage.values[self.pos];
                self.pos += 1;
                return Some((col, val));
            }
            self.pos += 1;
        }
        None
    }
}

/// Iterator over non-zero elements in a COO column.
pub struct COOColIter<'a> {
    storage: &'a COOStorage,
    target_col: IndexType,
    pos: usize,
}

impl<'a> Iterator for COOColIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        while self.pos < self.storage.values.len() {
            if self.storage.col_indices[self.pos] == self.target_col {
                let row = self.storage.row_indices[self.pos];
                let val = self.storage.values[self.pos];
                self.pos += 1;
                return Some((row, val));
            }
            self.pos += 1;
        }
        None
    }
}

// Graph Implementation
impl GraphStorage {
    /// Create graph storage from triplets.
    pub fn from_triplets(triplets: Vec<(usize, usize, Precision)>, nodes: DimensionType) -> Result<Self> {
        let mut out_edges = vec![Vec::new(); nodes];
        let mut in_edges = vec![Vec::new(); nodes];
        let mut degrees = vec![0.0; nodes];
        
        for (row, col, weight) in triplets {
            if weight != 0.0 && row < nodes && col < nodes {
                out_edges[row].push(GraphEdge {
                    target: col as NodeId,
                    weight,
                });
                
                if row != col { // Don't double-count self-loops for in_edges
                    in_edges[col].push(GraphEdge {
                        target: row as NodeId,
                        weight,
                    });
                }
                
                degrees[row] += weight.abs();
            }
        }
        
        Ok(Self {
            out_edges,
            in_edges,
            degrees,
        })
    }
    
    /// Get matrix element at (row, col).
    pub fn get(&self, row: usize, col: usize) -> Option<Precision> {
        if row >= self.out_edges.len() {
            return None;
        }
        
        for edge in &self.out_edges[row] {
            if edge.target as usize == col {
                return Some(edge.weight);
            }
        }
        None
    }
    
    /// Iterate over non-zero elements in a row.
    pub fn row_iter(&self, row: usize) -> GraphRowIter {
        if row >= self.out_edges.len() {
            GraphRowIter {
                edges: &[],
                pos: 0,
            }
        } else {
            GraphRowIter {
                edges: &self.out_edges[row],
                pos: 0,
            }
        }
    }
    
    /// Iterate over non-zero elements in a column.
    pub fn col_iter(&self, col: usize) -> GraphColIter {
        if col >= self.in_edges.len() {
            GraphColIter {
                edges: &[],
                pos: 0,
            }
        } else {
            GraphColIter {
                edges: &self.in_edges[col],
                pos: 0,
            }
        }
    }
    
    /// Matrix-vector multiplication: result = A * x
    pub fn multiply_vector(&self, x: &[Precision], result: &mut [Precision]) {
        result.fill(0.0);
        self.multiply_vector_add(x, result);
    }
    
    /// Matrix-vector multiplication with accumulation: result += A * x
    pub fn multiply_vector_add(&self, x: &[Precision], result: &mut [Precision]) {
        for (row, edges) in self.out_edges.iter().enumerate() {
            for edge in edges {
                let col = edge.target as usize;
                if col < x.len() {
                    result[row] += edge.weight * x[col];
                }
            }
        }
    }
    
    /// Get number of non-zero elements.
    pub fn nnz(&self) -> usize {
        self.out_edges.iter().map(|edges| edges.len()).sum()
    }
    
    /// Extract as coordinate triplets.
    pub fn to_triplets(&self) -> Result<Vec<(usize, usize, Precision)>> {
        let mut triplets = Vec::new();
        
        for (row, edges) in self.out_edges.iter().enumerate() {
            for edge in edges {
                triplets.push((row, edge.target as usize, edge.weight));
            }
        }
        
        Ok(triplets)
    }
    
    /// Scale all edge weights by a factor.
    pub fn scale(&mut self, factor: Precision) {
        for edges in &mut self.out_edges {
            for edge in edges {
                edge.weight *= factor;
            }
        }
        
        for edges in &mut self.in_edges {
            for edge in edges {
                edge.weight *= factor;
            }
        }
        
        for degree in &mut self.degrees {
            *degree *= factor.abs();
        }
    }
    
    /// Add a value to the diagonal.
    pub fn add_diagonal(&mut self, alpha: Precision) {
        for (node, edges) in self.out_edges.iter_mut().enumerate() {
            // Look for self-loop
            let mut found = false;
            for edge in edges.iter_mut() {
                if edge.target as usize == node {
                    edge.weight += alpha;
                    found = true;
                    break;
                }
            }
            
            // Add self-loop if it doesn't exist
            if !found && alpha != 0.0 {
                edges.push(GraphEdge {
                    target: node as NodeId,
                    weight: alpha,
                });
            }
            
            // Update degree
            self.degrees[node] += alpha.abs();
        }
    }
    
    /// Get outgoing edges for a node.
    pub fn out_neighbors(&self, node: usize) -> &[GraphEdge] {
        if node < self.out_edges.len() {
            &self.out_edges[node]
        } else {
            &[]
        }
    }
    
    /// Get incoming edges for a node.
    pub fn in_neighbors(&self, node: usize) -> &[GraphEdge] {
        if node < self.in_edges.len() {
            &self.in_edges[node]
        } else {
            &[]
        }
    }
    
    /// Get node degree.
    pub fn degree(&self, node: usize) -> Precision {
        if node < self.degrees.len() {
            self.degrees[node]
        } else {
            0.0
        }
    }
}

/// Iterator over non-zero elements in a graph row.
pub struct GraphRowIter<'a> {
    edges: &'a [GraphEdge],
    pos: usize,
}

impl<'a> Iterator for GraphRowIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.edges.len() {
            let edge = self.edges[self.pos];
            self.pos += 1;
            Some((edge.target, edge.weight))
        } else {
            None
        }
    }
}

/// Iterator over non-zero elements in a graph column.
pub struct GraphColIter<'a> {
    edges: &'a [GraphEdge],
    pos: usize,
}

impl<'a> Iterator for GraphColIter<'a> {
    type Item = (IndexType, Precision);
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.edges.len() {
            let edge = self.edges[self.pos];
            self.pos += 1;
            Some((edge.target, edge.weight))
        } else {
            None
        }
    }
}

#[cfg(all(test, feature = "std"))]
mod tests {
    use super::*;
    
    #[test]
    fn test_csr_creation() {
        let triplets = vec![(0, 0, 1.0), (0, 2, 2.0), (1, 1, 3.0), (2, 0, 4.0), (2, 2, 5.0)];
        let coo = COOStorage::from_triplets(triplets).unwrap();
        let csr = CSRStorage::from_coo(&coo, 3, 3).unwrap();
        
        assert_eq!(csr.nnz(), 5);
        assert_eq!(csr.get(0, 0), Some(1.0));
        assert_eq!(csr.get(0, 2), Some(2.0));
        assert_eq!(csr.get(1, 1), Some(3.0));
        assert_eq!(csr.get(0, 1), None);
    }
    
    #[test]
    fn test_csr_matrix_vector_multiply() {
        let triplets = vec![(0, 0, 2.0), (0, 1, 1.0), (1, 0, 1.0), (1, 1, 3.0)];
        let coo = COOStorage::from_triplets(triplets).unwrap();
        let csr = CSRStorage::from_coo(&coo, 2, 2).unwrap();
        
        let x = vec![1.0, 2.0];
        let mut result = vec![0.0; 2];
        
        csr.multiply_vector(&x, &mut result);
        assert_eq!(result, vec![4.0, 7.0]); // [2*1+1*2, 1*1+3*2]
    }
    
    #[test]
    fn test_graph_storage() {
        let triplets = vec![(0, 1, 0.5), (1, 0, 0.3), (1, 2, 0.7), (2, 1, 0.2)];
        let graph = GraphStorage::from_triplets(triplets, 3).unwrap();
        
        assert_eq!(graph.nnz(), 4);
        assert_eq!(graph.out_neighbors(1).len(), 2);
        assert_eq!(graph.in_neighbors(1).len(), 2);
        assert!(graph.degree(1) > 0.0);
    }
    
    #[test]
    fn test_format_conversions() {
        let triplets = vec![(0, 0, 1.0), (0, 2, 2.0), (1, 1, 3.0)];
        
        // COO -> CSR -> CSC -> COO roundtrip
        let coo1 = COOStorage::from_triplets(triplets.clone()).unwrap();
        let csr = CSRStorage::from_coo(&coo1, 2, 3).unwrap();
        let csc = CSCStorage::from_csr(&csr, 2, 3).unwrap();
        let triplets2 = csc.to_triplets().unwrap();
        
        // Sort both for comparison
        let mut t1 = triplets.clone();
        let mut t2 = triplets2;
        t1.sort();
        t2.sort();
        
        assert_eq!(t1, t2);
    }
}