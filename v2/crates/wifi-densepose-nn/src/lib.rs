//! # WiFi-DensePose Neural Network Crate
//!
//! This crate provides neural network inference capabilities for the WiFi-DensePose
//! pose estimation system. It supports multiple backends including ONNX Runtime,
//! tch-rs (PyTorch), and Candle for flexible deployment.
//!
//! ## Features
//!
//! - **DensePose Head**: Body part segmentation and UV coordinate regression
//! - **Modality Translator**: CSI to visual feature space translation
//! - **Multi-Backend Support**: ONNX, PyTorch (tch), and Candle backends
//! - **Inference Optimization**: Batching, GPU acceleration, and model caching
//!
//! ## Example
//!
//! ```rust,ignore
//! use wifi_densepose_nn::{InferenceEngine, DensePoseConfig, OnnxBackend};
//!
//! // Create inference engine with ONNX backend
//! let config = DensePoseConfig::default();
//! let backend = OnnxBackend::from_file("model.onnx")?;
//! let engine = InferenceEngine::new(backend, config)?;
//!
//! // Run inference
//! let input = ndarray::Array4::zeros((1, 256, 64, 64));
//! let output = engine.infer(&input)?;
//! ```

#![warn(missing_docs)]
#![warn(rustdoc::missing_doc_code_examples)]
#![deny(unsafe_code)]

pub mod densepose;
pub mod error;
pub mod inference;
#[cfg(feature = "onnx")]
pub mod onnx;
pub mod tensor;
pub mod translator;

// Re-exports for convenience
pub use densepose::{DensePoseConfig, DensePoseHead, DensePoseOutput};
pub use error::{NnError, NnResult};
pub use inference::{Backend, InferenceEngine, InferenceOptions};
#[cfg(feature = "onnx")]
pub use onnx::{OnnxBackend, OnnxSession};
pub use tensor::{Tensor, TensorShape};
pub use translator::{ModalityTranslator, TranslatorConfig, TranslatorOutput};

/// Prelude module for convenient imports
pub mod prelude {
    pub use crate::densepose::{DensePoseConfig, DensePoseHead, DensePoseOutput};
    pub use crate::error::{NnError, NnResult};
    pub use crate::inference::{Backend, InferenceEngine, InferenceOptions};
    #[cfg(feature = "onnx")]
    pub use crate::onnx::{OnnxBackend, OnnxSession};
    pub use crate::tensor::{Tensor, TensorShape};
    pub use crate::translator::{ModalityTranslator, TranslatorConfig, TranslatorOutput};
}

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Number of body parts in DensePose model (standard configuration)
pub const NUM_BODY_PARTS: usize = 24;

/// Number of UV coordinates (U and V)
pub const NUM_UV_COORDINATES: usize = 2;

/// Default hidden channel sizes for networks
pub const DEFAULT_HIDDEN_CHANNELS: &[usize] = &[256, 128, 64];
