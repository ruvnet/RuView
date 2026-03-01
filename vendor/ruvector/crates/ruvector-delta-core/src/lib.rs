//! # RuVector Delta Core
//!
//! Core delta types and traits for behavioral vector change tracking.
//! This crate provides the fundamental abstractions for computing, applying,
//! and composing deltas on vector data structures.
//!
//! ## Key Concepts
//!
//! - **Delta**: A representation of the change between two states
//! - **DeltaStream**: An ordered sequence of deltas for event sourcing
//! - **DeltaWindow**: Time-bounded aggregation of deltas
//! - **Encoding**: Sparse and dense delta representations
//! - **Compression**: Delta-specific compression strategies
//!
//! ## Example
//!
//! ```rust
//! use ruvector_delta_core::{Delta, VectorDelta, DeltaStream};
//!
//! // Compute delta between two vectors
//! let old = vec![1.0f32, 2.0, 3.0];
//! let new = vec![1.1f32, 2.0, 3.5];
//! let delta = VectorDelta::compute(&old, &new);
//!
//! // Apply delta to reconstruct
//! let mut reconstructed = old.clone();
//! delta.apply(&mut reconstructed).unwrap();
//! assert_eq!(reconstructed, new);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]
#![warn(clippy::all)]
#![deny(unsafe_op_in_unsafe_fn)]

extern crate alloc;

pub mod compression;
pub mod delta;
pub mod encoding;
pub mod error;
pub mod stream;
pub mod window;

// Re-exports
pub use compression::{CompressionCodec, CompressionLevel, DeltaCompressor};
pub use delta::{Delta, DeltaOp, DeltaValue, SparseDelta, VectorDelta};
pub use encoding::{
    DeltaEncoding, DenseEncoding, EncodingType, HybridEncoding, RunLengthEncoding, SparseEncoding,
};
pub use error::{DeltaError, Result};
pub use stream::{DeltaStream, DeltaStreamConfig, StreamCheckpoint};
pub use window::{DeltaWindow, WindowAggregator, WindowConfig, WindowResult, WindowType};

/// Prelude for convenient imports
pub mod prelude {
    pub use crate::compression::{CompressionCodec, DeltaCompressor};
    pub use crate::delta::{Delta, DeltaOp, DeltaValue, VectorDelta};
    pub use crate::encoding::{DeltaEncoding, DenseEncoding, SparseEncoding};
    pub use crate::error::Result;
    pub use crate::stream::{DeltaStream, StreamCheckpoint};
    pub use crate::window::{DeltaWindow, WindowAggregator};
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_delta() {
        let old = vec![1.0f32, 2.0, 3.0, 4.0];
        let new = vec![1.0f32, 2.5, 3.0, 4.5];

        let delta = VectorDelta::compute(&old, &new);

        let mut reconstructed = old.clone();
        delta.apply(&mut reconstructed).unwrap();

        for (a, b) in reconstructed.iter().zip(new.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_delta_composition() {
        let v1 = vec![1.0f32, 2.0, 3.0];
        let v2 = vec![1.5f32, 2.0, 3.5];
        let v3 = vec![2.0f32, 2.5, 4.0];

        let delta1 = VectorDelta::compute(&v1, &v2);
        let delta2 = VectorDelta::compute(&v2, &v3);

        let composed = delta1.compose(delta2);

        let mut result = v1.clone();
        composed.apply(&mut result).unwrap();

        for (a, b) in result.iter().zip(v3.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_delta_inverse() {
        let old = vec![1.0f32, 2.0, 3.0];
        let new = vec![1.5f32, 2.5, 3.5];

        let delta = VectorDelta::compute(&old, &new);
        let inverse = delta.inverse();

        let mut result = new.clone();
        inverse.apply(&mut result).unwrap();

        for (a, b) in result.iter().zip(old.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_identity_delta() {
        let v = vec![1.0f32, 2.0, 3.0];
        let delta = VectorDelta::compute(&v, &v);

        assert!(delta.is_identity());
    }
}
