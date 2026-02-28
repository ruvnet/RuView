//! Model definition and loading for ESP32
//!
//! Supports tiny transformer models with INT8 quantization.

use crate::quantized::{QuantParams, QuantizationType};
use heapless::Vec as HVec;
use serde::{Deserialize, Serialize};

/// Maximum number of transformer layers
pub const MAX_LAYERS: usize = 2;
/// Maximum embedding table size (vocab * embed_dim bytes)
pub const MAX_EMBEDDING_SIZE: usize = 32 * 1024; // 32KB
/// Maximum weight size per layer
pub const MAX_LAYER_SIZE: usize = 16 * 1024; // 16KB

/// Model configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Vocabulary size
    pub vocab_size: usize,
    /// Embedding dimension
    pub embed_dim: usize,
    /// Hidden dimension in FFN
    pub hidden_dim: usize,
    /// Number of transformer layers
    pub num_layers: usize,
    /// Number of attention heads
    pub num_heads: usize,
    /// Maximum sequence length
    pub max_seq_len: usize,
    /// Quantization type
    pub quant_type: QuantizationType,
}

impl Default for ModelConfig {
    fn default() -> Self {
        // Tiny model suitable for ESP32
        Self {
            vocab_size: 256,
            embed_dim: 32,
            hidden_dim: 64,
            num_layers: 1,
            num_heads: 2,
            max_seq_len: 16,
            quant_type: QuantizationType::Int8,
        }
    }
}

impl ModelConfig {
    /// Validate configuration fits ESP32 constraints
    pub fn validate(&self, variant: crate::Esp32Variant) -> crate::Result<()> {
        let model_size = self.estimate_size();
        let max_ram = variant.max_model_ram();

        if model_size > max_ram {
            return Err(crate::Error::ModelTooLarge {
                required: model_size,
                available: max_ram,
            });
        }

        if self.embed_dim % self.num_heads != 0 {
            return Err(crate::Error::InvalidModel(
                "embed_dim must be divisible by num_heads"
            ));
        }

        if self.num_layers > MAX_LAYERS {
            return Err(crate::Error::InvalidModel("Too many layers"));
        }

        Ok(())
    }

    /// Estimate total model size in bytes
    pub fn estimate_size(&self) -> usize {
        let bytes_per_weight = match self.quant_type {
            QuantizationType::Int8 => 1,
            QuantizationType::Int4 => 1, // 2 weights per byte
            QuantizationType::Binary => 1, // 8 weights per byte
            QuantizationType::Fixed16 => 2,
        };

        let divisor = match self.quant_type {
            QuantizationType::Int4 => 2,
            QuantizationType::Binary => 8,
            _ => 1,
        };

        // Embedding table
        let embed_size = (self.vocab_size * self.embed_dim * bytes_per_weight) / divisor;

        // Per-layer weights
        let qkv_size = 3 * self.embed_dim * self.embed_dim * bytes_per_weight / divisor;
        let ffn_size = 3 * self.embed_dim * self.hidden_dim * bytes_per_weight / divisor;
        let layer_size = qkv_size + ffn_size;

        // Output projection
        let output_size = (self.vocab_size * self.embed_dim * bytes_per_weight) / divisor;

        embed_size + (layer_size * self.num_layers) + output_size
    }

    /// Get recommended config for variant
    pub fn for_variant(variant: crate::Esp32Variant) -> Self {
        match variant {
            crate::Esp32Variant::Esp32 | crate::Esp32Variant::Esp32S3 => {
                // ~300KB available, use larger model (but fits in stack)
                Self {
                    vocab_size: 256,
                    embed_dim: 64,
                    hidden_dim: 128,
                    num_layers: 2,
                    num_heads: 4,
                    max_seq_len: 32,
                    quant_type: QuantizationType::Int8,
                }
            }
            crate::Esp32Variant::Esp32S2 => {
                // ~120KB available, use smaller model
                Self {
                    vocab_size: 128,
                    embed_dim: 32,
                    hidden_dim: 64,
                    num_layers: 1,
                    num_heads: 2,
                    max_seq_len: 16,
                    quant_type: QuantizationType::Int8,
                }
            }
            crate::Esp32Variant::Esp32C3 | crate::Esp32Variant::Esp32C6 => {
                // ~200KB available
                Self {
                    vocab_size: 256,
                    embed_dim: 48,
                    hidden_dim: 96,
                    num_layers: 2,
                    num_heads: 3,
                    max_seq_len: 24,
                    quant_type: QuantizationType::Int8,
                }
            }
        }
    }
}

/// Layer weights for a single transformer layer
#[derive(Clone)]
pub struct LayerWeights {
    /// Query projection weights [embed_dim, embed_dim]
    pub wq: HVec<i8, MAX_LAYER_SIZE>,
    /// Key projection weights
    pub wk: HVec<i8, MAX_LAYER_SIZE>,
    /// Value projection weights
    pub wv: HVec<i8, MAX_LAYER_SIZE>,
    /// Output projection weights
    pub wo: HVec<i8, MAX_LAYER_SIZE>,

    /// FFN up projection [embed_dim, hidden_dim]
    pub w_up: HVec<i8, MAX_LAYER_SIZE>,
    /// FFN gate projection
    pub w_gate: HVec<i8, MAX_LAYER_SIZE>,
    /// FFN down projection [hidden_dim, embed_dim]
    pub w_down: HVec<i8, MAX_LAYER_SIZE>,

    /// Quantization params
    pub q_params: QuantParams,
    pub k_params: QuantParams,
    pub v_params: QuantParams,
    pub o_params: QuantParams,
    pub up_params: QuantParams,
    pub gate_params: QuantParams,
    pub down_params: QuantParams,
}

impl Default for LayerWeights {
    fn default() -> Self {
        Self {
            wq: HVec::new(),
            wk: HVec::new(),
            wv: HVec::new(),
            wo: HVec::new(),
            w_up: HVec::new(),
            w_gate: HVec::new(),
            w_down: HVec::new(),
            q_params: QuantParams::default(),
            k_params: QuantParams::default(),
            v_params: QuantParams::default(),
            o_params: QuantParams::default(),
            up_params: QuantParams::default(),
            gate_params: QuantParams::default(),
            down_params: QuantParams::default(),
        }
    }
}

impl LayerWeights {
    /// Initialize with random weights (for testing)
    pub fn random(config: &ModelConfig, seed: u32) -> crate::Result<Self> {
        let mut layer = Self::default();

        let embed_dim = config.embed_dim;
        let hidden_dim = config.hidden_dim;

        // Simple LCG random number generator
        let mut rng_state = seed;
        let mut next_rand = || {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            // Get value in range 0-127, then map to -64 to 63
            (((rng_state >> 16) & 0x7F) as i16 - 64) as i8
        };

        // QKV projections [embed_dim, embed_dim]
        let qkv_size = embed_dim * embed_dim;
        for _ in 0..qkv_size {
            layer.wq.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            layer.wk.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            layer.wv.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            layer.wo.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        // FFN projections
        let up_size = embed_dim * hidden_dim;
        for _ in 0..up_size {
            layer.w_up.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
            layer.w_gate.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        let down_size = hidden_dim * embed_dim;
        for _ in 0..down_size {
            layer.w_down.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        // Initialize quant params with reasonable defaults
        let scale = 1.0 / 64.0; // For weights in range [-64, 63]
        layer.q_params = QuantParams { scale, zero_point: 0.0, min_val: -1.0, max_val: 1.0 };
        layer.k_params = layer.q_params;
        layer.v_params = layer.q_params;
        layer.o_params = layer.q_params;
        layer.up_params = layer.q_params;
        layer.gate_params = layer.q_params;
        layer.down_params = layer.q_params;

        Ok(layer)
    }

    /// Memory size of this layer
    pub fn memory_size(&self) -> usize {
        self.wq.len() + self.wk.len() + self.wv.len() + self.wo.len()
            + self.w_up.len() + self.w_gate.len() + self.w_down.len()
    }
}

/// Complete tiny model
pub struct TinyModel {
    /// Model configuration
    pub config: ModelConfig,
    /// Embedding table [vocab_size, embed_dim]
    pub embedding_table: HVec<i8, MAX_EMBEDDING_SIZE>,
    /// Transformer layers
    pub layers: [LayerWeights; MAX_LAYERS],
    /// Output projection [embed_dim, vocab_size]
    pub output_proj: HVec<i8, MAX_EMBEDDING_SIZE>,
    /// Input quantization params
    pub input_params: QuantParams,
    /// Output quantization params
    pub output_params: QuantParams,
}

impl TinyModel {
    /// Create a new model with random weights
    pub fn new(config: ModelConfig) -> crate::Result<Self> {
        config.validate(crate::Esp32Variant::Esp32)?;

        let mut embedding_table = HVec::new();
        let mut output_proj = HVec::new();

        // Initialize embedding table
        let embed_size = config.vocab_size * config.embed_dim;
        let mut rng_state = 12345u32;
        let mut next_rand = || {
            rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
            // Get value in range 0-255, then map to -128 to 127
            (((rng_state >> 16) & 0xFF) as i16 - 128) as i8
        };

        for _ in 0..embed_size {
            embedding_table.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        // Initialize output projection
        for _ in 0..embed_size {
            output_proj.push(next_rand()).map_err(|_| crate::Error::BufferOverflow)?;
        }

        // Initialize layers
        let mut layers: [LayerWeights; MAX_LAYERS] = Default::default();
        for i in 0..config.num_layers {
            layers[i] = LayerWeights::random(&config, (i * 1000) as u32)?;
        }

        Ok(Self {
            config,
            embedding_table,
            layers,
            output_proj,
            input_params: QuantParams::default(),
            output_params: QuantParams::default(),
        })
    }

    /// Total memory size of model
    pub fn memory_size(&self) -> usize {
        let mut size = self.embedding_table.len();
        size += self.output_proj.len();
        for i in 0..self.config.num_layers {
            size += self.layers[i].memory_size();
        }
        size
    }

    /// Load model from bytes (e.g., from flash)
    pub fn from_bytes(data: &[u8]) -> crate::Result<Self> {
        // Parse header
        if data.len() < 32 {
            return Err(crate::Error::InvalidModel("Data too small"));
        }

        // Magic number check
        if &data[0..4] != b"RUVM" {
            return Err(crate::Error::InvalidModel("Invalid magic number"));
        }

        // Parse config from header
        let vocab_size = u16::from_le_bytes([data[4], data[5]]) as usize;
        let embed_dim = u16::from_le_bytes([data[6], data[7]]) as usize;
        let hidden_dim = u16::from_le_bytes([data[8], data[9]]) as usize;
        let num_layers = data[10] as usize;
        let num_heads = data[11] as usize;
        let max_seq_len = data[12] as usize;
        let quant_type = match data[13] {
            0 => QuantizationType::Int8,
            1 => QuantizationType::Int4,
            2 => QuantizationType::Binary,
            3 => QuantizationType::Fixed16,
            _ => return Err(crate::Error::InvalidModel("Unknown quantization type")),
        };

        let config = ModelConfig {
            vocab_size,
            embed_dim,
            hidden_dim,
            num_layers,
            num_heads,
            max_seq_len,
            quant_type,
        };

        config.validate(crate::Esp32Variant::Esp32)?;

        // For now, create random weights - real implementation would parse from data
        Self::new(config)
    }

    /// Export model to bytes
    pub fn to_bytes(&self) -> HVec<u8, 256> {
        let mut header: HVec<u8, 256> = HVec::new();

        // Magic number
        let _ = header.extend_from_slice(b"RUVM");

        // Config
        let _ = header.extend_from_slice(&(self.config.vocab_size as u16).to_le_bytes());
        let _ = header.extend_from_slice(&(self.config.embed_dim as u16).to_le_bytes());
        let _ = header.extend_from_slice(&(self.config.hidden_dim as u16).to_le_bytes());
        let _ = header.push(self.config.num_layers as u8);
        let _ = header.push(self.config.num_heads as u8);
        let _ = header.push(self.config.max_seq_len as u8);
        let _ = header.push(match self.config.quant_type {
            QuantizationType::Int8 => 0,
            QuantizationType::Int4 => 1,
            QuantizationType::Binary => 2,
            QuantizationType::Fixed16 => 3,
        });

        // Padding to 32 bytes
        while header.len() < 32 {
            let _ = header.push(0);
        }

        header
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ModelConfig::default();
        assert!(config.validate(crate::Esp32Variant::Esp32S2).is_ok());

        let size = config.estimate_size();
        println!("Default model size: {} bytes ({:.1} KB)", size, size as f32 / 1024.0);
        assert!(size < 50 * 1024); // < 50KB for testing
    }

    #[test]
    fn test_variant_configs() {
        for variant in [
            crate::Esp32Variant::Esp32,
            crate::Esp32Variant::Esp32S2,
            crate::Esp32Variant::Esp32S3,
            crate::Esp32Variant::Esp32C3,
            crate::Esp32Variant::Esp32C6,
        ] {
            let config = ModelConfig::for_variant(variant);
            assert!(config.validate(variant).is_ok());

            let size = config.estimate_size();
            println!("{:?}: {} bytes ({:.1} KB)", variant, size, size as f32 / 1024.0);
        }
    }

    #[test]
    fn test_model_creation() {
        let config = ModelConfig::default();
        let model = TinyModel::new(config).unwrap();

        let size = model.memory_size();
        println!("Actual model size: {} bytes ({:.1} KB)", size, size as f32 / 1024.0);
    }

    #[test]
    fn test_serialization() {
        let config = ModelConfig::default();
        let model = TinyModel::new(config).unwrap();

        let header = model.to_bytes();
        assert_eq!(&header[0..4], b"RUVM");
    }
}
