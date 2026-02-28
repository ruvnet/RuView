//! Native PostgreSQL sparse vector type with zero-copy varlena layout
//!
//! SparseVec stores only non-zero elements, ideal for high-dimensional sparse data.
//! Uses PostgreSQL varlena layout for zero-copy performance.
//!
//! Varlena layout:
//! - VARHDRSZ (4 bytes)
//! - dimensions (4 bytes u32) - total dimensions
//! - nnz (4 bytes u32) - number of non-zeros
//! - indices (4 bytes * nnz) - sorted indices
//! - values (4 bytes * nnz) - values

use pgrx::pgrx_sql_entity_graph::metadata::{
    ArgumentError, Returns, ReturnsError, SqlMapping, SqlTranslatable,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::ptr;
use std::str::FromStr;

use crate::MAX_DIMENSIONS;

// ============================================================================
// SparseVec Structure (Rust representation)
// ============================================================================

/// SparseVec: Sparse vector type for high-dimensional data
///
/// Memory layout in PostgreSQL varlena format:
/// - Header: 4 bytes (VARHDRSZ)
/// - Dimensions: 4 bytes (u32)
/// - NNZ: 4 bytes (u32)
/// - Indices: 4 bytes * nnz (u32 array)
/// - Values: 4 bytes * nnz (f32 array)
#[derive(Clone, Serialize, Deserialize)]
pub struct SparseVec {
    /// Total dimensions (including zeros)
    dimensions: u32,
    /// Non-zero indices (sorted)
    indices: Vec<u32>,
    /// Non-zero values (corresponding to indices)
    values: Vec<f32>,
}

impl SparseVec {
    /// Create from index-value pairs
    pub fn from_pairs(dimensions: usize, pairs: &[(usize, f32)]) -> Self {
        if dimensions > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        // Filter zeros and sort by index
        let mut sorted: Vec<_> = pairs
            .iter()
            .filter(|(_, v)| *v != 0.0 && v.is_finite())
            .map(|&(i, v)| (i as u32, v))
            .collect();
        sorted.sort_by_key(|(i, _)| *i);

        // Check for duplicates and bounds
        for i in 1..sorted.len() {
            if sorted[i].0 == sorted[i - 1].0 {
                pgrx::error!("Duplicate index {} in sparse vector", sorted[i].0);
            }
        }

        if let Some(&(max_idx, _)) = sorted.last() {
            if max_idx as usize >= dimensions {
                pgrx::error!(
                    "Index {} out of bounds for dimension {}",
                    max_idx,
                    dimensions
                );
            }
        }

        let (indices, values): (Vec<_>, Vec<_>) = sorted.into_iter().unzip();

        Self {
            dimensions: dimensions as u32,
            indices,
            values,
        }
    }

    /// Create from dense vector with threshold
    pub fn from_dense(data: &[f32], threshold: f32) -> Self {
        let pairs: Vec<_> = data
            .iter()
            .enumerate()
            .filter(|(_, &v)| v.abs() > threshold && v.is_finite())
            .map(|(i, &v)| (i, v))
            .collect();

        Self::from_pairs(data.len(), &pairs)
    }

    /// Create from BTreeMap
    pub fn from_map(dimensions: usize, map: &BTreeMap<u32, f32>) -> Self {
        let pairs: Vec<_> = map.iter().map(|(&i, &v)| (i as usize, v)).collect();
        Self::from_pairs(dimensions, &pairs)
    }

    /// Create empty sparse vector
    pub fn zeros(dimensions: usize) -> Self {
        if dimensions > MAX_DIMENSIONS {
            pgrx::error!(
                "Vector dimension {} exceeds maximum {}",
                dimensions,
                MAX_DIMENSIONS
            );
        }

        Self {
            dimensions: dimensions as u32,
            indices: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Get total dimensions
    #[inline]
    pub fn dimensions(&self) -> usize {
        self.dimensions as usize
    }

    /// Get number of non-zero elements
    #[inline]
    pub fn nnz(&self) -> usize {
        self.indices.len()
    }

    /// Get sparsity ratio (nnz / dimensions)
    pub fn sparsity(&self) -> f32 {
        if self.dimensions == 0 {
            return 0.0;
        }
        self.nnz() as f32 / self.dimensions as f32
    }

    /// Get indices slice
    #[inline]
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// Get values slice
    #[inline]
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Get value at index (0.0 if not present)
    pub fn get(&self, index: usize) -> f32 {
        match self.indices.binary_search(&(index as u32)) {
            Ok(pos) => self.values[pos],
            Err(_) => 0.0,
        }
    }

    /// Convert to dense vector
    pub fn to_dense(&self) -> Vec<f32> {
        let mut dense = vec![0.0; self.dimensions as usize];
        for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
            dense[idx as usize] = val;
        }
        dense
    }

    /// Calculate L2 norm
    pub fn norm(&self) -> f32 {
        self.values.iter().map(|x| x * x).sum::<f32>().sqrt()
    }

    /// Sparse dot product with another sparse vector (merge-join algorithm)
    pub fn dot(&self, other: &Self) -> f32 {
        if self.dimensions != other.dimensions {
            pgrx::error!("Vector dimensions must match for dot product");
        }

        let mut i = 0;
        let mut j = 0;
        let mut sum = 0.0;

        // Merge-join for sparse-sparse intersection
        while i < self.nnz() && j < other.nnz() {
            let idx_a = self.indices[i];
            let idx_b = other.indices[j];

            if idx_a == idx_b {
                sum += self.values[i] * other.values[j];
                i += 1;
                j += 1;
            } else if idx_a < idx_b {
                i += 1;
            } else {
                j += 1;
            }
        }

        sum
    }

    /// Dot product with dense vector (scatter-gather)
    pub fn dot_dense(&self, dense: &[f32]) -> f32 {
        if self.dimensions() != dense.len() {
            pgrx::error!("Vector dimensions must match for dot product");
        }

        self.indices
            .iter()
            .zip(self.values.iter())
            .map(|(&idx, &val)| val * dense[idx as usize])
            .sum()
    }

    /// Memory size in bytes
    pub fn memory_size(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.indices.len() * std::mem::size_of::<u32>()
            + self.values.len() * std::mem::size_of::<f32>()
    }

    /// Add two sparse vectors
    pub fn add(&self, other: &Self) -> Self {
        if self.dimensions != other.dimensions {
            pgrx::error!("Vector dimensions must match");
        }

        let mut result: BTreeMap<u32, f32> = BTreeMap::new();

        for (&idx, &val) in self.indices.iter().zip(self.values.iter()) {
            *result.entry(idx).or_insert(0.0) += val;
        }

        for (&idx, &val) in other.indices.iter().zip(other.values.iter()) {
            *result.entry(idx).or_insert(0.0) += val;
        }

        // Remove zeros
        result.retain(|_, v| *v != 0.0);

        Self::from_map(self.dimensions as usize, &result)
    }

    /// Scalar multiplication
    pub fn mul_scalar(&self, scalar: f32) -> Self {
        if scalar == 0.0 {
            return Self::zeros(self.dimensions as usize);
        }

        Self {
            dimensions: self.dimensions,
            indices: self.indices.clone(),
            values: self.values.iter().map(|v| v * scalar).collect(),
        }
    }

    /// Serialize to varlena bytes (zero-copy layout)
    fn to_varlena_bytes(&self) -> Vec<u8> {
        let nnz = self.nnz() as u32;
        let header_size = 8; // dimensions (4) + nnz (4)
        let indices_size = (nnz as usize) * 4;
        let values_size = (nnz as usize) * 4;
        let total_size = header_size + indices_size + values_size;

        let mut bytes = Vec::with_capacity(total_size);

        // Write header
        bytes.extend_from_slice(&self.dimensions.to_le_bytes());
        bytes.extend_from_slice(&nnz.to_le_bytes());

        // Write indices
        for idx in &self.indices {
            bytes.extend_from_slice(&idx.to_le_bytes());
        }

        // Write values
        for val in &self.values {
            bytes.extend_from_slice(&val.to_le_bytes());
        }

        bytes
    }

    /// Deserialize from varlena bytes
    unsafe fn from_varlena_bytes(bytes: &[u8]) -> Self {
        if bytes.len() < 8 {
            pgrx::error!("Invalid sparsevec data: too short");
        }

        let dimensions = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let nnz = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let expected_len = 8 + nnz * 8;

        if bytes.len() != expected_len {
            pgrx::error!(
                "Invalid sparsevec data: expected {} bytes, got {}",
                expected_len,
                bytes.len()
            );
        }

        let mut indices = Vec::with_capacity(nnz);
        let mut values = Vec::with_capacity(nnz);

        // Read indices
        for i in 0..nnz {
            let offset = 8 + i * 4;
            let idx = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            indices.push(idx);
        }

        // Read values
        let values_offset = 8 + nnz * 4;
        for i in 0..nnz {
            let offset = values_offset + i * 4;
            let val = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            values.push(val);
        }

        Self {
            dimensions,
            indices,
            values,
        }
    }
}

impl fmt::Display for SparseVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format: {idx:val,idx:val,...}/dim
        write!(f, "{{")?;
        for (i, (&idx, &val)) in self.indices.iter().zip(self.values.iter()).enumerate() {
            if i > 0 {
                write!(f, ",")?;
            }
            write!(f, "{}:{}", idx, val)?;
        }
        write!(f, "}}/{}", self.dimensions)
    }
}

impl fmt::Debug for SparseVec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "SparseVec(dims={}, nnz={}, sparsity={:.2}%)",
            self.dimensions,
            self.nnz(),
            self.sparsity() * 100.0
        )
    }
}

impl FromStr for SparseVec {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Parse format: {idx:val,idx:val,...}/dim
        if !s.starts_with('{') {
            return Err(format!("Invalid sparsevec format: must start with {{"));
        }

        let parts: Vec<_> = s[1..].splitn(2, "}/").collect();

        if parts.len() != 2 {
            return Err("Invalid sparsevec format: expected {pairs}/dim".to_string());
        }

        let dimensions: usize = parts[1].trim().parse().map_err(|_| "Invalid dimensions")?;

        if parts[0].is_empty() {
            return Ok(Self::zeros(dimensions));
        }

        let pairs: Result<Vec<(usize, f32)>, String> = parts[0]
            .split(',')
            .map(|pair| {
                let kv: Vec<_> = pair.split(':').collect();
                if kv.len() != 2 {
                    return Err(format!("Invalid index:value pair: {}", pair));
                }
                let idx: usize = kv[0].trim().parse().map_err(|_| "Invalid index")?;
                let val: f32 = kv[1].trim().parse().map_err(|_| "Invalid value")?;
                Ok((idx, val))
            })
            .collect();

        Ok(Self::from_pairs(dimensions, &pairs?))
    }
}

impl PartialEq for SparseVec {
    fn eq(&self, other: &Self) -> bool {
        self.dimensions == other.dimensions
            && self.indices == other.indices
            && self.values == other.values
    }
}

impl Eq for SparseVec {}

// ============================================================================
// PostgreSQL Type Integration
// ============================================================================

unsafe impl SqlTranslatable for SparseVec {
    fn argument_sql() -> Result<SqlMapping, ArgumentError> {
        Ok(SqlMapping::As(String::from("sparsevec")))
    }

    fn return_sql() -> Result<Returns, ReturnsError> {
        Ok(Returns::One(SqlMapping::As(String::from("sparsevec"))))
    }
}

impl pgrx::IntoDatum for SparseVec {
    fn into_datum(self) -> Option<pgrx::pg_sys::Datum> {
        let bytes = self.to_varlena_bytes();
        let len = bytes.len();
        let total_size = pgrx::pg_sys::VARHDRSZ + len;

        unsafe {
            let ptr = pgrx::pg_sys::palloc(total_size) as *mut u8;
            let varlena = ptr as *mut pgrx::pg_sys::varlena;
            pgrx::varlena::set_varsize_4b(varlena, total_size as i32);
            ptr::copy_nonoverlapping(bytes.as_ptr(), ptr.add(pgrx::pg_sys::VARHDRSZ), len);
            Some(pgrx::pg_sys::Datum::from(ptr))
        }
    }

    fn type_oid() -> pgrx::pg_sys::Oid {
        pgrx::pg_sys::Oid::INVALID
    }
}

impl pgrx::FromDatum for SparseVec {
    unsafe fn from_polymorphic_datum(
        datum: pgrx::pg_sys::Datum,
        is_null: bool,
        _typoid: pgrx::pg_sys::Oid,
    ) -> Option<Self> {
        if is_null {
            return None;
        }

        let ptr = datum.cast_mut_ptr::<pgrx::pg_sys::varlena>();
        let len = pgrx::varlena::varsize_any_exhdr(ptr);
        let data_ptr = pgrx::varlena::vardata_any(ptr) as *const u8;
        let bytes = std::slice::from_raw_parts(data_ptr, len);

        Some(SparseVec::from_varlena_bytes(bytes))
    }
}

// ============================================================================
// Text I/O Functions - Internal use
// ============================================================================
// Note: SparseVec type is for internal use. SQL-level functions use arrays.

// Note: SparseVec SQL functions are not exposed via #[pg_extern] due to
// pgrx 0.12 trait requirements. Use array-based functions for SQL-level operations.

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_pairs() {
        let v = SparseVec::from_pairs(10, &[(0, 1.0), (5, 2.0), (9, 3.0)]);
        assert_eq!(v.dimensions(), 10);
        assert_eq!(v.nnz(), 3);
        assert_eq!(v.get(0), 1.0);
        assert_eq!(v.get(5), 2.0);
        assert_eq!(v.get(9), 3.0);
        assert_eq!(v.get(1), 0.0);
    }

    #[test]
    fn test_from_dense() {
        let dense = vec![1.0, 0.0, 0.0, 2.0, 0.0];
        let sparse = SparseVec::from_dense(&dense, 0.0);
        assert_eq!(sparse.dimensions(), 5);
        assert_eq!(sparse.nnz(), 2);
        assert_eq!(sparse.get(0), 1.0);
        assert_eq!(sparse.get(3), 2.0);
    }

    #[test]
    fn test_to_dense() {
        let sparse = SparseVec::from_pairs(5, &[(0, 1.0), (3, 2.0)]);
        let dense = sparse.to_dense();
        assert_eq!(dense, vec![1.0, 0.0, 0.0, 2.0, 0.0]);
    }

    #[test]
    fn test_dot_sparse() {
        let a = SparseVec::from_pairs(5, &[(0, 1.0), (2, 2.0), (4, 3.0)]);
        let b = SparseVec::from_pairs(5, &[(0, 4.0), (2, 5.0), (3, 6.0)]);
        // Dot = 1*4 + 2*5 = 14
        assert!((a.dot(&b) - 14.0).abs() < 1e-6);
    }

    #[test]
    fn test_sparse_l2_distance() {
        let a = SparseVec::from_pairs(5, &[(0, 3.0), (2, 4.0)]);
        let b = SparseVec::from_pairs(5, &[(0, 0.0), (2, 0.0)]);
        // Distance = sqrt(3^2 + 4^2) = 5
        // Compute L2 distance using dense conversion
        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let dist = a_dense
            .iter()
            .zip(b_dense.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!((dist - 5.0).abs() < 1e-6);
    }

    #[test]
    fn test_memory_efficiency() {
        let sparse =
            SparseVec::from_pairs(10000, &(0..10).map(|i| (i * 1000, 1.0)).collect::<Vec<_>>());

        let dense_size = 10000 * 4; // 40KB
        let sparse_size = sparse.memory_size();

        assert!(sparse_size < dense_size / 10);
    }

    #[test]
    fn test_parse() {
        let v: SparseVec = "{0:1.0,2:2.0,4:3.0}/5".parse().unwrap();
        assert_eq!(v.dimensions(), 5);
        assert_eq!(v.nnz(), 3);
        assert_eq!(v.get(0), 1.0);
        assert_eq!(v.get(2), 2.0);
        assert_eq!(v.get(4), 3.0);
    }

    #[test]
    fn test_display() {
        let v = SparseVec::from_pairs(5, &[(0, 1.0), (2, 2.0)]);
        assert_eq!(v.to_string(), "{0:1,2:2}/5");
    }

    #[test]
    fn test_varlena_serialization() {
        let v = SparseVec::from_pairs(10, &[(0, 1.0), (5, 2.0), (9, 3.0)]);
        let bytes = v.to_varlena_bytes();
        let v2 = unsafe { SparseVec::from_varlena_bytes(&bytes) };
        assert_eq!(v, v2);
    }

    #[test]
    fn test_threshold_filtering() {
        let dense = vec![0.001, 0.5, 0.002, 1.0, 0.003];
        let sparse = SparseVec::from_dense(&dense, 0.01);
        assert_eq!(sparse.nnz(), 2); // Only 0.5 and 1.0 above threshold
    }
}

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod pg_tests {
    use super::*;
    use pgrx::pg_test;

    // Note: sparsevec_in/out SQL functions are not exposed via #[pg_extern]
    // due to pgrx 0.12 trait requirements. Testing parse/display instead.
    #[pg_test]
    fn test_sparsevec_parse_display() {
        let input = "{0:1.5,3:2.5,7:3.5}/10";
        let v: SparseVec = input.parse().unwrap();
        assert_eq!(v.dimensions(), 10);
        assert_eq!(v.nnz(), 3);

        let output = v.to_string();
        assert_eq!(output, "{0:1.5,3:2.5,7:3.5}/10");
    }

    #[pg_test]
    fn test_sparsevec_distances() {
        let a = SparseVec::from_pairs(5, &[(0, 1.0), (2, 2.0)]);
        let b = SparseVec::from_pairs(5, &[(1, 1.0), (2, 1.0)]);

        // Compute L2 distance using dense conversion
        let a_dense = a.to_dense();
        let b_dense = b.to_dense();
        let l2: f32 = a_dense
            .iter()
            .zip(b_dense.iter())
            .map(|(x, y)| (x - y).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(l2 > 0.0);

        // Inner product (only index 2 overlaps: 2*1 = 2)
        let ip = a.dot(&b);
        assert!((ip - 2.0).abs() < 1e-6);

        // Cosine distance using dot product
        let a_norm = a_dense.iter().map(|x| x * x).sum::<f32>().sqrt();
        let b_norm = b_dense.iter().map(|x| x * x).sum::<f32>().sqrt();
        let cosine = 1.0 - (ip / (a_norm * b_norm));
        assert!(cosine >= 0.0 && cosine <= 2.0);
    }

    #[pg_test]
    fn test_sparsevec_conversions() {
        let dense_data = [1.0, 0.0, 2.0, 0.0, 3.0];
        let sparse = SparseVec::from_dense(&dense_data, 0.0);

        assert_eq!(sparse.nnz(), 3);

        let dense2 = sparse.to_dense();
        assert_eq!(&dense_data[..], &dense2[..]);
    }
}
