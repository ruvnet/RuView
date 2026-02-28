//! WASM Kernel Pack System (ADR-005)
//!
//! This module implements the WebAssembly kernel pack infrastructure for
//! secure, sandboxed execution of ML compute kernels.
//!
//! # Architecture
//!
//! The kernel pack system provides:
//! - **Sandboxed Execution**: Wasmtime runtime with epoch-based interruption
//! - **Supply Chain Security**: Ed25519 signatures, SHA256 hash verification
//! - **Hot-Swappable Kernels**: Update kernels without service restart
//! - **Cross-Platform**: Same kernels run on servers and embedded devices
//!
//! # Kernel Categories
//!
//! - Positional: RoPE (Rotary Position Embeddings)
//! - Normalization: RMSNorm
//! - Activation: SwiGLU
//! - KV Cache: Quantization/Dequantization
//! - Adapter: LoRA delta application
//!
//! # Example
//!
//! ```rust,ignore
//! use ruvector_wasm::kernel::{KernelManager, KernelPackVerifier};
//!
//! // Load and verify kernel pack
//! let verifier = KernelPackVerifier::with_trusted_keys(keys);
//! let manager = KernelManager::new(runtime_config)?;
//! manager.load_pack("kernel-pack-v1.0.0", &verifier)?;
//!
//! // Execute kernel
//! let result = manager.execute("rope_f32", &descriptor)?;
//! ```

pub mod allowlist;
pub mod epoch;
pub mod error;
pub mod hash;
pub mod manifest;
pub mod memory;
pub mod runtime;
pub mod signature;

// Re-exports
pub use allowlist::TrustedKernelAllowlist;
pub use epoch::{EpochConfig, EpochController};
pub use error::{KernelError, VerifyError};
pub use hash::HashVerifier;
pub use manifest::{
    KernelCategory, KernelDescriptor, KernelInfo, KernelManifest, KernelParam, PlatformConfig,
    ResourceLimits, TensorSpec,
};
pub use memory::{KernelInvocationDescriptor, SharedMemoryProtocol};
pub use runtime::{KernelRuntime, RuntimeConfig, WasmKernelInstance};
pub use signature::KernelPackVerifier;

/// Current runtime version for compatibility checking
pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Maximum supported kernel manifest schema version
pub const MAX_MANIFEST_VERSION: &str = "1.0.0";

/// WASM page size in bytes (64KB)
pub const WASM_PAGE_SIZE: usize = 65536;

/// Default epoch tick interval in milliseconds
pub const DEFAULT_EPOCH_TICK_MS: u64 = 10;

/// Default epoch budget (ticks before interruption)
pub const DEFAULT_EPOCH_BUDGET: u64 = 1000;
