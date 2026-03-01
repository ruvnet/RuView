//! Delta encoding strategies
//!
//! This module provides various encoding strategies for deltas,
//! optimizing for different access patterns and sparsity levels.

use alloc::vec::Vec;
use core::marker::PhantomData;

use crate::delta::{DeltaOp, DeltaValue, VectorDelta};
use crate::error::{DeltaError, Result};

/// Trait for delta encoding strategies
pub trait DeltaEncoding: Send + Sync {
    /// Encode a delta to bytes
    fn encode(&self, delta: &VectorDelta) -> Result<Vec<u8>>;

    /// Decode bytes to a delta
    fn decode(&self, bytes: &[u8]) -> Result<VectorDelta>;

    /// Get the encoding type identifier
    fn encoding_type(&self) -> EncodingType;
}

/// Encoding type identifiers
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum EncodingType {
    /// Dense encoding (all values stored)
    Dense = 0,
    /// Sparse encoding (only non-zero values)
    Sparse = 1,
    /// Run-length encoding
    RunLength = 2,
    /// Varint encoding
    Varint = 3,
    /// Hybrid encoding (automatic selection)
    Hybrid = 4,
}

impl TryFrom<u8> for EncodingType {
    type Error = DeltaError;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Dense),
            1 => Ok(Self::Sparse),
            2 => Ok(Self::RunLength),
            3 => Ok(Self::Varint),
            4 => Ok(Self::Hybrid),
            _ => Err(DeltaError::InvalidEncoding(alloc::format!(
                "Unknown encoding type: {}",
                value
            ))),
        }
    }
}

/// Dense encoding - stores all values
#[derive(Debug, Clone, Default)]
pub struct DenseEncoding;

impl DenseEncoding {
    /// Create a new dense encoding
    pub fn new() -> Self {
        Self
    }
}

impl DeltaEncoding for DenseEncoding {
    fn encode(&self, delta: &VectorDelta) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(4 + 4 + delta.dimensions * 4);

        // Header: encoding type (1 byte) + dimensions (4 bytes)
        bytes.push(EncodingType::Dense as u8);
        bytes.extend_from_slice(&(delta.dimensions as u32).to_le_bytes());

        // Convert to dense and encode
        match &delta.value {
            DeltaValue::Identity => {
                // Write zeros
                bytes.extend(core::iter::repeat(0u8).take(delta.dimensions * 4));
            }
            DeltaValue::Sparse(ops) => {
                let mut values = vec![0.0f32; delta.dimensions];
                for op in ops {
                    if (op.index as usize) < delta.dimensions {
                        values[op.index as usize] = op.value;
                    }
                }
                for v in values {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => {
                for v in values {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }

        Ok(bytes)
    }

    fn decode(&self, bytes: &[u8]) -> Result<VectorDelta> {
        if bytes.len() < 5 {
            return Err(DeltaError::InvalidEncoding(
                "Buffer too small for header".into(),
            ));
        }

        let encoding_type = EncodingType::try_from(bytes[0])?;
        if encoding_type != EncodingType::Dense {
            return Err(DeltaError::InvalidEncoding("Not a dense encoding".into()));
        }

        let dimensions = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;

        let expected_len = 5 + dimensions * 4;
        if bytes.len() < expected_len {
            return Err(DeltaError::InvalidEncoding(alloc::format!(
                "Buffer too small: expected {}, got {}",
                expected_len,
                bytes.len()
            )));
        }

        let mut values = Vec::with_capacity(dimensions);
        for i in 0..dimensions {
            let offset = 5 + i * 4;
            let v = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            values.push(v);
        }

        Ok(VectorDelta::from_dense(values))
    }

    fn encoding_type(&self) -> EncodingType {
        EncodingType::Dense
    }
}

/// Sparse encoding - stores only non-zero values with their indices
#[derive(Debug, Clone, Default)]
pub struct SparseEncoding {
    /// Threshold for considering a value as zero
    pub epsilon: f32,
}

impl SparseEncoding {
    /// Create a new sparse encoding with default epsilon
    pub fn new() -> Self {
        Self { epsilon: 1e-7 }
    }

    /// Create with custom epsilon
    pub fn with_epsilon(epsilon: f32) -> Self {
        Self { epsilon }
    }
}

impl DeltaEncoding for SparseEncoding {
    fn encode(&self, delta: &VectorDelta) -> Result<Vec<u8>> {
        // Header: encoding type (1) + dimensions (4) + count (4)
        let mut bytes = Vec::new();

        bytes.push(EncodingType::Sparse as u8);
        bytes.extend_from_slice(&(delta.dimensions as u32).to_le_bytes());

        match &delta.value {
            DeltaValue::Identity => {
                // Zero entries
                bytes.extend_from_slice(&0u32.to_le_bytes());
            }
            DeltaValue::Sparse(ops) => {
                bytes.extend_from_slice(&(ops.len() as u32).to_le_bytes());
                for op in ops {
                    bytes.extend_from_slice(&op.index.to_le_bytes());
                    bytes.extend_from_slice(&op.value.to_le_bytes());
                }
            }
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => {
                let non_zero: Vec<_> = values
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| v.abs() > self.epsilon)
                    .collect();

                bytes.extend_from_slice(&(non_zero.len() as u32).to_le_bytes());
                for (i, v) in non_zero {
                    bytes.extend_from_slice(&(i as u32).to_le_bytes());
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
            }
        }

        Ok(bytes)
    }

    fn decode(&self, bytes: &[u8]) -> Result<VectorDelta> {
        if bytes.len() < 9 {
            return Err(DeltaError::InvalidEncoding(
                "Buffer too small for sparse header".into(),
            ));
        }

        let encoding_type = EncodingType::try_from(bytes[0])?;
        if encoding_type != EncodingType::Sparse {
            return Err(DeltaError::InvalidEncoding("Not a sparse encoding".into()));
        }

        let dimensions = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
        let count = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;

        let expected_len = 9 + count * 8;
        if bytes.len() < expected_len {
            return Err(DeltaError::InvalidEncoding(alloc::format!(
                "Buffer too small: expected {}, got {}",
                expected_len,
                bytes.len()
            )));
        }

        let mut ops = smallvec::SmallVec::new();
        for i in 0..count {
            let offset = 9 + i * 8;
            let index = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            let value = f32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]);
            ops.push(DeltaOp::new(index, value));
        }

        Ok(VectorDelta::from_sparse(ops, dimensions))
    }

    fn encoding_type(&self) -> EncodingType {
        EncodingType::Sparse
    }
}

/// Run-length encoding for consecutive identical deltas
#[derive(Debug, Clone, Default)]
pub struct RunLengthEncoding {
    /// Threshold for considering values equal
    pub epsilon: f32,
}

impl RunLengthEncoding {
    /// Create a new run-length encoding
    pub fn new() -> Self {
        Self { epsilon: 1e-7 }
    }

    /// Create with custom epsilon
    pub fn with_epsilon(epsilon: f32) -> Self {
        Self { epsilon }
    }

    /// Check if two values are approximately equal
    fn approx_eq(&self, a: f32, b: f32) -> bool {
        (a - b).abs() <= self.epsilon
    }
}

/// A run in RLE encoding
#[derive(Debug, Clone, Copy)]
struct Run {
    value: f32,
    count: u32,
}

impl DeltaEncoding for RunLengthEncoding {
    fn encode(&self, delta: &VectorDelta) -> Result<Vec<u8>> {
        let values = match &delta.value {
            DeltaValue::Identity => vec![0.0f32; delta.dimensions],
            DeltaValue::Sparse(ops) => {
                let mut v = vec![0.0f32; delta.dimensions];
                for op in ops {
                    if (op.index as usize) < delta.dimensions {
                        v[op.index as usize] = op.value;
                    }
                }
                v
            }
            DeltaValue::Dense(v) | DeltaValue::Replace(v) => v.clone(),
        };

        if values.is_empty() {
            let mut bytes = Vec::with_capacity(9);
            bytes.push(EncodingType::RunLength as u8);
            bytes.extend_from_slice(&(0u32).to_le_bytes());
            bytes.extend_from_slice(&(0u32).to_le_bytes());
            return Ok(bytes);
        }

        // Build runs
        let mut runs: Vec<Run> = Vec::new();
        let mut current_value = values[0];
        let mut current_count = 1u32;

        for &v in values.iter().skip(1) {
            if self.approx_eq(v, current_value) {
                current_count += 1;
            } else {
                runs.push(Run {
                    value: current_value,
                    count: current_count,
                });
                current_value = v;
                current_count = 1;
            }
        }
        runs.push(Run {
            value: current_value,
            count: current_count,
        });

        // Encode
        let mut bytes = Vec::with_capacity(9 + runs.len() * 8);
        bytes.push(EncodingType::RunLength as u8);
        bytes.extend_from_slice(&(delta.dimensions as u32).to_le_bytes());
        bytes.extend_from_slice(&(runs.len() as u32).to_le_bytes());

        for run in runs {
            bytes.extend_from_slice(&run.value.to_le_bytes());
            bytes.extend_from_slice(&run.count.to_le_bytes());
        }

        Ok(bytes)
    }

    fn decode(&self, bytes: &[u8]) -> Result<VectorDelta> {
        if bytes.len() < 9 {
            return Err(DeltaError::InvalidEncoding(
                "Buffer too small for RLE header".into(),
            ));
        }

        let encoding_type = EncodingType::try_from(bytes[0])?;
        if encoding_type != EncodingType::RunLength {
            return Err(DeltaError::InvalidEncoding(
                "Not a run-length encoding".into(),
            ));
        }

        let dimensions = u32::from_le_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]) as usize;
        let run_count = u32::from_le_bytes([bytes[5], bytes[6], bytes[7], bytes[8]]) as usize;

        let expected_len = 9 + run_count * 8;
        if bytes.len() < expected_len {
            return Err(DeltaError::InvalidEncoding(alloc::format!(
                "Buffer too small: expected {}, got {}",
                expected_len,
                bytes.len()
            )));
        }

        let mut values = Vec::with_capacity(dimensions);
        for i in 0..run_count {
            let offset = 9 + i * 8;
            let value = f32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            let count = u32::from_le_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;

            for _ in 0..count {
                values.push(value);
            }
        }

        if values.len() != dimensions {
            return Err(DeltaError::InvalidEncoding(alloc::format!(
                "RLE decoded to {} values, expected {}",
                values.len(),
                dimensions
            )));
        }

        Ok(VectorDelta::from_dense(values))
    }

    fn encoding_type(&self) -> EncodingType {
        EncodingType::RunLength
    }
}

/// Hybrid encoding that automatically selects the best strategy
#[derive(Debug, Clone)]
pub struct HybridEncoding {
    /// Sparsity threshold for choosing sparse encoding
    pub sparsity_threshold: f32,
    /// RLE benefit threshold
    pub rle_threshold: f32,
    /// Epsilon for float comparisons
    pub epsilon: f32,
}

impl Default for HybridEncoding {
    fn default() -> Self {
        Self {
            sparsity_threshold: 0.7,
            rle_threshold: 0.5,
            epsilon: 1e-7,
        }
    }
}

impl HybridEncoding {
    /// Create a new hybrid encoding with default thresholds
    pub fn new() -> Self {
        Self::default()
    }

    /// Create with custom thresholds
    pub fn with_thresholds(sparsity: f32, rle: f32) -> Self {
        Self {
            sparsity_threshold: sparsity,
            rle_threshold: rle,
            epsilon: 1e-7,
        }
    }

    /// Determine the best encoding for a delta
    pub fn select_encoding(&self, delta: &VectorDelta) -> EncodingType {
        match &delta.value {
            DeltaValue::Identity => EncodingType::Sparse,
            DeltaValue::Sparse(ops) => {
                let sparsity = 1.0 - (ops.len() as f32 / delta.dimensions as f32);
                if sparsity > self.sparsity_threshold {
                    EncodingType::Sparse
                } else {
                    EncodingType::Dense
                }
            }
            DeltaValue::Dense(values) | DeltaValue::Replace(values) => {
                // Check sparsity
                let non_zero = values.iter().filter(|v| v.abs() > self.epsilon).count();
                let sparsity = 1.0 - (non_zero as f32 / values.len() as f32);

                if sparsity > self.sparsity_threshold {
                    return EncodingType::Sparse;
                }

                // Check RLE potential
                let mut runs = 1usize;
                let mut prev = values[0];
                for &v in values.iter().skip(1) {
                    if (v - prev).abs() > self.epsilon {
                        runs += 1;
                        prev = v;
                    }
                }

                let rle_ratio = runs as f32 / values.len() as f32;
                if rle_ratio < self.rle_threshold {
                    EncodingType::RunLength
                } else {
                    EncodingType::Dense
                }
            }
        }
    }
}

impl DeltaEncoding for HybridEncoding {
    fn encode(&self, delta: &VectorDelta) -> Result<Vec<u8>> {
        let selected = self.select_encoding(delta);

        match selected {
            EncodingType::Dense => DenseEncoding.encode(delta),
            EncodingType::Sparse => SparseEncoding::with_epsilon(self.epsilon).encode(delta),
            EncodingType::RunLength => RunLengthEncoding::with_epsilon(self.epsilon).encode(delta),
            _ => DenseEncoding.encode(delta),
        }
    }

    fn decode(&self, bytes: &[u8]) -> Result<VectorDelta> {
        if bytes.is_empty() {
            return Err(DeltaError::InvalidEncoding("Empty buffer".into()));
        }

        let encoding_type = EncodingType::try_from(bytes[0])?;

        match encoding_type {
            EncodingType::Dense => DenseEncoding.decode(bytes),
            EncodingType::Sparse => SparseEncoding::with_epsilon(self.epsilon).decode(bytes),
            EncodingType::RunLength => RunLengthEncoding::with_epsilon(self.epsilon).decode(bytes),
            EncodingType::Hybrid => Err(DeltaError::InvalidEncoding(
                "Hybrid type should not appear in encoded data".into(),
            )),
            EncodingType::Varint => Err(DeltaError::InvalidEncoding(
                "Varint encoding not yet implemented".into(),
            )),
        }
    }

    fn encoding_type(&self) -> EncodingType {
        EncodingType::Hybrid
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delta::Delta;
    use alloc::vec;

    #[test]
    fn test_dense_encoding_roundtrip() {
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0, 4.0]);

        let encoding = DenseEncoding::new();
        let bytes = encoding.encode(&delta).unwrap();
        let decoded = encoding.decode(&bytes).unwrap();

        assert_eq!(delta.dimensions, decoded.dimensions);
    }

    #[test]
    fn test_sparse_encoding_roundtrip() {
        let mut ops = smallvec::SmallVec::new();
        ops.push(DeltaOp::new(5, 1.5));
        ops.push(DeltaOp::new(10, 2.5));
        let delta = VectorDelta::from_sparse(ops, 100);

        let encoding = SparseEncoding::new();
        let bytes = encoding.encode(&delta).unwrap();
        let decoded = encoding.decode(&bytes).unwrap();

        assert_eq!(delta.dimensions, decoded.dimensions);
        assert_eq!(delta.value.nnz(), decoded.value.nnz());
    }

    #[test]
    fn test_rle_encoding_roundtrip() {
        // Create a delta with runs
        let values = vec![1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0, 3.0, 3.0];
        let delta = VectorDelta::from_dense(values.clone());

        let encoding = RunLengthEncoding::new();
        let bytes = encoding.encode(&delta).unwrap();
        let decoded = encoding.decode(&bytes).unwrap();

        assert_eq!(delta.dimensions, decoded.dimensions);
    }

    #[test]
    fn test_hybrid_encoding_selects_sparse() {
        // Very sparse delta
        let mut ops = smallvec::SmallVec::new();
        ops.push(DeltaOp::new(5, 1.5));
        let delta = VectorDelta::from_sparse(ops, 1000);

        let encoding = HybridEncoding::new();
        assert_eq!(encoding.select_encoding(&delta), EncodingType::Sparse);
    }

    #[test]
    fn test_hybrid_encoding_roundtrip() {
        let delta = VectorDelta::from_dense(vec![1.0, 2.0, 3.0, 4.0]);

        let encoding = HybridEncoding::new();
        let bytes = encoding.encode(&delta).unwrap();
        let decoded = encoding.decode(&bytes).unwrap();

        assert_eq!(delta.dimensions, decoded.dimensions);
    }

    #[test]
    fn test_identity_encoding() {
        let delta = VectorDelta::new(100);
        assert!(delta.is_identity());

        let encoding = SparseEncoding::new();
        let bytes = encoding.encode(&delta).unwrap();
        let decoded = encoding.decode(&bytes).unwrap();

        assert!(decoded.is_identity());
    }
}
