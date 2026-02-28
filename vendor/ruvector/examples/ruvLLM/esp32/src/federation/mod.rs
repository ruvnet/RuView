//! Federation Module for Multi-ESP32 Distributed Inference
//!
//! Enables running larger models across multiple ESP32 chips:
//! - Pipeline parallelism: Each chip handles different layers
//! - Tensor parallelism: Split attention heads across chips
//! - Model sharding: Distribute embeddings/weights
//! - Speculative decoding: Draft on one chip, verify on others
//!
//! # Architecture Options
//!
//! ```text
//! 5-Chip Pipeline (recommended for latency):
//! ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐    ┌─────────┐
//! │ ESP32-0 │───▶│ ESP32-1 │───▶│ ESP32-2 │───▶│ ESP32-3 │───▶│ ESP32-4 │
//! │ Embed + │    │ Layer 1 │    │ Layer 2 │    │ Layer 3 │    │ Layer 4 │
//! │ Layer 0 │    │         │    │         │    │         │    │ + Head  │
//! └─────────┘    └─────────┘    └─────────┘    └─────────┘    └─────────┘
//!
//! 5-Chip Tensor Parallel (for throughput):
//! ┌─────────┐
//! │ ESP32-0 │ ◀──┐
//! │ Head 0  │    │
//! └─────────┘    │
//! ┌─────────┐    │    ┌─────────┐
//! │ ESP32-1 │ ◀──┼────│ ESP32-4 │
//! │ Head 1  │    │    │ Coord   │
//! └─────────┘    │    └─────────┘
//! ┌─────────┐    │
//! │ ESP32-2 │ ◀──┤
//! │ Head 2  │    │
//! └─────────┘    │
//! ┌─────────┐    │
//! │ ESP32-3 │ ◀──┘
//! │ Head 3  │
//! └─────────┘
//! ```

pub mod pipeline;
pub mod tensor_parallel;
pub mod sharding;
pub mod speculative;
pub mod protocol;
pub mod coordinator;
pub mod fastgrnn_router;
pub mod massive_scale;
pub mod medium_scale;

// Re-exports
pub use pipeline::{PipelineNode, PipelineConfig, PipelineRole};
pub use tensor_parallel::{TensorParallelNode, TPConfig};
pub use sharding::{ShardedEmbedding, ShardConfig};
pub use speculative::{SpeculativeDecoder, DraftVerifyConfig};
pub use protocol::{FederationMessage, MessageType, ChipId};
pub use coordinator::{FederationCoordinator, ClusterTopology};
pub use fastgrnn_router::{MicroFastGRNN, MicroGRNNConfig, RoutingFeatures};
pub use massive_scale::{
    MassiveTopology, MassiveScaleConfig, MassiveScaleSimulator, ScaleProjection,
    DistributedCoordinator, GossipProtocol, FaultTolerance,
};
pub use medium_scale::{
    MediumClusterConfig, ScaleComparison, MediumScaleAnalyzer,
    ModelCategory, HardwareConfig, BusType,
    MEDIUM_SCALE_MIN, MEDIUM_SCALE_MAX, MEDIUM_SCALE_OPTIMAL,
};

/// Maximum chips in small federation
pub const MAX_FEDERATION_SIZE: usize = 8;
/// Maximum chips in massive scale (theoretical)
pub const MAX_MASSIVE_SCALE: usize = 1_000_000;

/// Federation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FederationMode {
    /// Single chip (no federation)
    Standalone,
    /// Pipeline parallelism - each chip handles different layers
    Pipeline,
    /// Tensor parallelism - split heads across chips
    TensorParallel,
    /// Hybrid: pipeline + tensor parallel
    Hybrid,
    /// Speculative decoding with draft/verify
    Speculative,
    /// Mixture of Experts - each chip is an expert
    MixtureOfExperts,
}

/// Federation cluster configuration
#[derive(Debug, Clone)]
pub struct FederationConfig {
    /// Number of chips in cluster
    pub num_chips: usize,
    /// This chip's ID (0-indexed)
    pub chip_id: ChipId,
    /// Federation mode
    pub mode: FederationMode,
    /// Communication bus type
    pub bus: CommunicationBus,
    /// Layers per chip (for pipeline mode)
    pub layers_per_chip: usize,
    /// Heads per chip (for tensor parallel mode)
    pub heads_per_chip: usize,
    /// Enable pipelining (process next token while current finishes)
    pub enable_pipelining: bool,
}

impl Default for FederationConfig {
    fn default() -> Self {
        Self {
            num_chips: 5,
            chip_id: ChipId(0),
            mode: FederationMode::Pipeline,
            bus: CommunicationBus::Spi,
            layers_per_chip: 2,
            heads_per_chip: 1,
            enable_pipelining: true,
        }
    }
}

/// Communication bus between chips
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommunicationBus {
    /// SPI bus (fastest, 10-80 MHz)
    Spi,
    /// I2C bus (slower, 400 kHz - 1 MHz)
    I2c,
    /// UART (flexible, up to 5 Mbps)
    Uart,
    /// ESP-NOW (wireless, ~1 Mbps)
    EspNow,
    /// Custom parallel bus
    Parallel,
}

impl CommunicationBus {
    /// Estimated bandwidth in bytes/second
    pub const fn bandwidth_bytes_per_sec(&self) -> usize {
        match self {
            Self::Spi => 10_000_000,      // 10 MB/s at 80 MHz
            Self::I2c => 100_000,          // 100 KB/s at 1 MHz
            Self::Uart => 500_000,         // 500 KB/s at 5 Mbps
            Self::EspNow => 125_000,       // ~1 Mbps
            Self::Parallel => 20_000_000,  // Custom 8-bit parallel
        }
    }

    /// Latency overhead in microseconds
    pub const fn latency_us(&self) -> usize {
        match self {
            Self::Spi => 10,
            Self::I2c => 50,
            Self::Uart => 20,
            Self::EspNow => 500,  // Wireless overhead
            Self::Parallel => 5,
        }
    }
}

/// Calculate optimal federation configuration for given model
pub fn calculate_optimal_config(
    model_size_bytes: usize,
    num_layers: usize,
    num_heads: usize,
    num_chips: usize,
    per_chip_ram: usize,
) -> FederationConfig {
    let model_per_chip = model_size_bytes / num_chips;

    // Check if model fits with pipeline parallelism
    if model_per_chip <= per_chip_ram {
        let layers_per_chip = (num_layers + num_chips - 1) / num_chips;
        return FederationConfig {
            num_chips,
            chip_id: ChipId(0),
            mode: FederationMode::Pipeline,
            bus: CommunicationBus::Spi,
            layers_per_chip,
            heads_per_chip: num_heads,
            enable_pipelining: true,
        };
    }

    // Try tensor parallelism
    let heads_per_chip = (num_heads + num_chips - 1) / num_chips;
    FederationConfig {
        num_chips,
        chip_id: ChipId(0),
        mode: FederationMode::TensorParallel,
        bus: CommunicationBus::Spi,
        layers_per_chip: num_layers,
        heads_per_chip,
        enable_pipelining: false,
    }
}

/// Estimate performance improvement from federation
pub fn estimate_speedup(config: &FederationConfig) -> FederationSpeedup {
    let n = config.num_chips as f32;

    match config.mode {
        FederationMode::Standalone => FederationSpeedup {
            throughput_multiplier: 1.0,
            latency_reduction: 1.0,
            memory_per_chip_reduction: 1.0,
        },
        FederationMode::Pipeline => FederationSpeedup {
            // Pipeline: n-way throughput, slightly higher latency
            throughput_multiplier: n * 0.85, // 85% efficiency due to bubble
            latency_reduction: 1.0 / (1.0 + 0.1 * (n - 1.0)), // Slight increase
            memory_per_chip_reduction: n,
        },
        FederationMode::TensorParallel => FederationSpeedup {
            // TP: near-linear speedup on attention
            throughput_multiplier: n * 0.7, // Communication overhead
            latency_reduction: n * 0.7,
            memory_per_chip_reduction: n * 0.8, // Some duplication
        },
        FederationMode::Hybrid => FederationSpeedup {
            throughput_multiplier: n * 0.75,
            latency_reduction: (n / 2.0) * 0.8,
            memory_per_chip_reduction: n * 0.9,
        },
        FederationMode::Speculative => FederationSpeedup {
            // Speculative: 2-4x speedup typical
            throughput_multiplier: 2.5,
            latency_reduction: 2.0,
            memory_per_chip_reduction: 1.0, // Full model on draft chip
        },
        FederationMode::MixtureOfExperts => FederationSpeedup {
            throughput_multiplier: n * 0.9, // Excellent scaling
            latency_reduction: 1.5,
            memory_per_chip_reduction: n,
        },
    }
}

/// Performance improvement estimates
#[derive(Debug, Clone)]
pub struct FederationSpeedup {
    /// Throughput improvement (tokens/sec multiplier)
    pub throughput_multiplier: f32,
    /// Latency reduction (time per token)
    pub latency_reduction: f32,
    /// Memory reduction per chip
    pub memory_per_chip_reduction: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_config() {
        let config = calculate_optimal_config(
            500 * 1024,  // 500 KB model
            10,          // 10 layers
            4,           // 4 heads
            5,           // 5 chips
            120 * 1024,  // 120 KB per chip
        );

        assert_eq!(config.mode, FederationMode::Pipeline);
        assert_eq!(config.layers_per_chip, 2);
    }

    #[test]
    fn test_speedup_estimate() {
        let config = FederationConfig {
            num_chips: 5,
            mode: FederationMode::Pipeline,
            ..Default::default()
        };

        let speedup = estimate_speedup(&config);

        assert!(speedup.throughput_multiplier > 4.0);
        assert!(speedup.memory_per_chip_reduction >= 5.0);
    }
}
