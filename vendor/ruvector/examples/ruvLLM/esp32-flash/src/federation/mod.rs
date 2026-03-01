//! Federation Module for Multi-Chip Distributed Inference
//!
//! Supports:
//! - Pipeline parallelism (layers across chips)
//! - Tensor parallelism (attention heads across chips)
//! - Speculative decoding (draft/verify)
//! - SPI/I2C/UART/ESP-NOW communication

pub mod protocol;
pub mod pipeline;
pub mod speculative;

pub use protocol::{
    ChipId, MessageType, MessageHeader, FederationMessage, CommStats,
    MAX_ACTIVATION_SIZE, MAX_PAYLOAD_SIZE,
};
pub use pipeline::{
    PipelineNode, PipelineConfig, PipelineRole, PipelineState, PipelineStats,
    InFlightToken, calculate_pipeline_efficiency,
    MAX_LAYERS_PER_CHIP, MAX_PIPELINE_DEPTH,
};
pub use speculative::{
    SpeculativeDecoder, DraftVerifyConfig, DraftResult, VerifyResult, SpecStats,
    MAX_DRAFT_TOKENS,
};

/// Maximum chips in federation
pub const MAX_FEDERATION_SIZE: usize = 8;

/// Federation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FederationMode {
    Standalone,
    Pipeline,
    TensorParallel,
    Hybrid,
    Speculative,
    MixtureOfExperts,
}

/// Communication bus type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CommunicationBus {
    Spi,
    I2c,
    Uart,
    EspNow,
    Parallel,
}

impl CommunicationBus {
    pub const fn bandwidth_bytes_per_sec(&self) -> usize {
        match self {
            Self::Spi => 10_000_000,
            Self::I2c => 100_000,
            Self::Uart => 500_000,
            Self::EspNow => 125_000,
            Self::Parallel => 20_000_000,
        }
    }

    pub const fn latency_us(&self) -> usize {
        match self {
            Self::Spi => 10,
            Self::I2c => 50,
            Self::Uart => 20,
            Self::EspNow => 500,
            Self::Parallel => 5,
        }
    }
}

/// Federation configuration
#[derive(Debug, Clone)]
pub struct FederationConfig {
    pub num_chips: usize,
    pub chip_id: ChipId,
    pub mode: FederationMode,
    pub bus: CommunicationBus,
    pub layers_per_chip: usize,
    pub heads_per_chip: usize,
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

/// Calculate optimal federation config
pub fn calculate_optimal_config(
    model_size: usize,
    num_layers: usize,
    num_heads: usize,
    num_chips: usize,
    per_chip_ram: usize,
) -> FederationConfig {
    let model_per_chip = model_size / num_chips;

    if model_per_chip <= per_chip_ram {
        let layers_per_chip = (num_layers + num_chips - 1) / num_chips;
        FederationConfig {
            num_chips,
            chip_id: ChipId(0),
            mode: FederationMode::Pipeline,
            bus: CommunicationBus::Spi,
            layers_per_chip,
            heads_per_chip: num_heads,
            enable_pipelining: true,
        }
    } else {
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
}

/// Federation speedup estimates
#[derive(Debug, Clone)]
pub struct FederationSpeedup {
    pub throughput_multiplier: f32,
    pub latency_reduction: f32,
    pub memory_per_chip_reduction: f32,
}

pub fn estimate_speedup(config: &FederationConfig) -> FederationSpeedup {
    let n = config.num_chips as f32;
    match config.mode {
        FederationMode::Standalone => FederationSpeedup {
            throughput_multiplier: 1.0,
            latency_reduction: 1.0,
            memory_per_chip_reduction: 1.0,
        },
        FederationMode::Pipeline => FederationSpeedup {
            throughput_multiplier: n * 0.85,
            latency_reduction: 1.0 / (1.0 + 0.1 * (n - 1.0)),
            memory_per_chip_reduction: n,
        },
        FederationMode::TensorParallel => FederationSpeedup {
            throughput_multiplier: n * 0.7,
            latency_reduction: n * 0.7,
            memory_per_chip_reduction: n * 0.8,
        },
        FederationMode::Hybrid => FederationSpeedup {
            throughput_multiplier: n * 0.75,
            latency_reduction: (n / 2.0) * 0.8,
            memory_per_chip_reduction: n * 0.9,
        },
        FederationMode::Speculative => FederationSpeedup {
            throughput_multiplier: 2.5,
            latency_reduction: 2.0,
            memory_per_chip_reduction: 1.0,
        },
        FederationMode::MixtureOfExperts => FederationSpeedup {
            throughput_multiplier: n * 0.9,
            latency_reduction: 1.5,
            memory_per_chip_reduction: n,
        },
    }
}
