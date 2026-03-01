//! Vector quantization for memory reduction
//!
//! Provides various quantization methods:
//! - Scalar (SQ8): 4x compression
//! - Product (PQ): 8-32x compression
//! - Binary: 32x compression

pub mod binary;
pub mod product;
pub mod scalar;

use std::sync::atomic::{AtomicUsize, Ordering};

/// Global quantization table memory tracking
static TABLE_MEMORY_BYTES: AtomicUsize = AtomicUsize::new(0);

/// Get quantization table memory in MB
pub fn get_table_memory_mb() -> f64 {
    TABLE_MEMORY_BYTES.load(Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

/// Track table memory allocation
pub fn track_table_allocation(bytes: usize) {
    TABLE_MEMORY_BYTES.fetch_add(bytes, Ordering::Relaxed);
}

/// Quantization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizationType {
    /// No quantization (full precision)
    None,
    /// Scalar quantization (f32 -> i8)
    Scalar,
    /// Product quantization (subspace division)
    Product,
    /// Binary quantization (f32 -> 1 bit)
    Binary,
}

impl std::fmt::Display for QuantizationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QuantizationType::None => write!(f, "none"),
            QuantizationType::Scalar => write!(f, "sq8"),
            QuantizationType::Product => write!(f, "pq"),
            QuantizationType::Binary => write!(f, "binary"),
        }
    }
}

impl std::str::FromStr for QuantizationType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "none" | "" => Ok(QuantizationType::None),
            "scalar" | "sq8" | "sq" => Ok(QuantizationType::Scalar),
            "product" | "pq" => Ok(QuantizationType::Product),
            "binary" | "bq" => Ok(QuantizationType::Binary),
            _ => Err(format!("Unknown quantization type: {}", s)),
        }
    }
}
