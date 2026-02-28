//! GPU acceleration module for Prime-Radiant coherence engine.
//!
//! This module provides GPU-accelerated computation using wgpu for:
//! - Parallel residual calculations across large graphs
//! - Matrix operations for restriction maps
//! - Energy aggregation with atomic operations
//! - Spectral analysis via power iteration
//!
//! # Architecture
//!
//! ```text
//! +------------------+     +------------------+     +------------------+
//! |   GpuDevice      |---->|   GpuBuffer      |---->|   GpuDispatcher  |
//! |  (Init/Queue)    |     |  (Alloc/Transfer)|     |  (Kernels/Sync)  |
//! +------------------+     +------------------+     +------------------+
//!          |                        |                        |
//!          v                        v                        v
//! +------------------+     +------------------+     +------------------+
//! | Instance/Adapter |     | BufferPool       |     | PipelineCache    |
//! | Device/Queue     |     | Read/Write       |     | BindGroups       |
//! +------------------+     +------------------+     +------------------+
//! ```
//!
//! # Feature Flag
//!
//! This module requires the `gpu` feature flag:
//! ```toml
//! [dependencies]
//! prime-radiant = { version = "0.1", features = ["gpu"] }
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use prime_radiant::gpu::{GpuDevice, GpuBuffer, GpuDispatcher, ComputePipeline};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Initialize GPU device
//!     let device = GpuDevice::new().await?;
//!
//!     // Create storage buffer with data
//!     let input_data: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0];
//!     let input_buffer = GpuBuffer::new_storage(device.device(), &input_data, false);
//!
//!     // Create output buffer
//!     let output_buffer = GpuBuffer::new_storage_uninit::<f32>(
//!         device.device(),
//!         input_data.len(),
//!         true,
//!     );
//!
//!     // Create compute pipeline
//!     let pipeline = ComputePipeline::from_shader(
//!         device.device(),
//!         include_str!("shaders/compute_residuals.wgsl"),
//!         "main",
//!         &[BindingDesc::storage_readonly(), BindingDesc::storage_readwrite()],
//!     )?;
//!
//!     // Create dispatcher and execute
//!     let dispatcher = GpuDispatcher::new(Arc::new(device));
//!     let bind_group = pipeline.create_bind_group(
//!         dispatcher.device().device(),
//!         &[&input_buffer, &output_buffer],
//!     )?;
//!     dispatcher.dispatch(&pipeline, &bind_group, [4, 1, 1]).await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # GPU Kernels
//!
//! The following WGSL compute shaders are implemented:
//!
//! 1. **compute_residuals.wgsl** - Parallel residual computation for all edges
//! 2. **compute_energy.wgsl** - Parallel energy aggregation with tree reduction
//! 3. **sheaf_attention.wgsl** - Batched attention: A_ij = exp(-beta * E_ij) / Z
//! 4. **token_routing.wgsl** - Parallel lane assignment based on energy thresholds
//!
//! # Performance Targets
//!
//! | Operation | Target | Notes |
//! |-----------|--------|-------|
//! | Buffer allocation | < 1ms | Pooled for hot paths |
//! | Kernel dispatch | < 100us | Excludes GPU execution |
//! | Residual (10K edges) | < 1ms | GPU parallel |
//! | Energy aggregation | < 500us | Atomic reduction |

mod buffer;
mod device;
mod dispatch;
mod engine;
mod error;
mod kernels;
mod pipeline;

// Core exports
pub use buffer::{
    BufferKey, BufferUsage, BufferUsageFlags, GpuBuffer, GpuBufferManager, GpuBufferPool,
};
pub use device::{GpuDevice, GpuDeviceInfo, GpuDeviceOptions};
pub use dispatch::{DispatchBuilder, DispatchConfig, GpuDispatcher};
pub use error::{GpuError, GpuResult};
pub use pipeline::{BindingDesc, BindingType, ComputePipeline, PipelineCache};

// Re-export buffer types
pub use buffer::{GpuEdge, GpuNodeState, GpuParams, GpuRestrictionMap};

// Re-export engine types
pub use engine::{GpuCapabilities, GpuCoherenceEnergy, GpuCoherenceEngine, GpuConfig};

/// Synchronous API for GPU coherence engine (uses pollster)
pub mod sync {
    pub use super::engine::sync::*;
}

// Re-export kernel types
pub use kernels::{
    AttentionWeight, ComputeEnergyKernel, ComputeResidualsKernel, EnergyParams, LaneStats,
    RoutingDecision, SheafAttentionKernel, Token, TokenRoutingKernel,
};

/// Default workgroup size for compute shaders
pub const DEFAULT_WORKGROUP_SIZE: u32 = 256;

/// Maximum buffer size for a single allocation (256MB)
pub const MAX_BUFFER_SIZE: u64 = 256 * 1024 * 1024;

/// Default pool capacity for buffer reuse
pub const DEFAULT_POOL_CAPACITY: usize = 32;

/// Shader source code embedded at compile time
pub mod shaders {
    /// Compute residuals shader for parallel edge residual computation
    pub const COMPUTE_RESIDUALS: &str = include_str!("shaders/compute_residuals.wgsl");
    /// Compute energy shader for parallel reduction
    pub const COMPUTE_ENERGY: &str = include_str!("shaders/compute_energy.wgsl");
    /// Sheaf attention shader for attention weight computation
    pub const SHEAF_ATTENTION: &str = include_str!("shaders/sheaf_attention.wgsl");
    /// Token routing shader for lane assignment
    pub const TOKEN_ROUTING: &str = include_str!("shaders/token_routing.wgsl");
}

/// GPU workgroup size constants
pub mod workgroup {
    /// Default workgroup size for 1D compute
    pub const SIZE_1D: u32 = 256;
    /// Default workgroup size for 2D compute (x dimension)
    pub const SIZE_2D_X: u32 = 16;
    /// Default workgroup size for 2D compute (y dimension)
    pub const SIZE_2D_Y: u32 = 16;
    /// Maximum state vector dimension for GPU kernels
    pub const MAX_STATE_DIM: u32 = 512;
}
