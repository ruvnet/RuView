//! Quantizers for 3/5/7-bit precision lanes
//!
//! Implements pack/unpack operations for each precision lane with
//! per-block or per-channel scaling.

use super::lanes::PrecisionLane;
use serde::{Deserialize, Serialize};

/// Quantized block with scale factor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizedBlock {
    /// Quantized data
    pub data: Vec<i8>,
    /// Scale factor for dequantization
    pub scale: f32,
    /// Zero point offset
    pub zero_point: i8,
    /// Block size
    pub block_size: usize,
    /// Precision lane
    pub lane: PrecisionLane,
}

impl QuantizedBlock {
    /// Create a new quantized block
    pub fn new(lane: PrecisionLane, block_size: usize) -> Self {
        Self {
            data: Vec::with_capacity(block_size),
            scale: lane.default_scale(),
            zero_point: 0,
            block_size,
            lane,
        }
    }

    /// Dequantize to f32 values
    pub fn dequantize(&self) -> Vec<f32> {
        self.data
            .iter()
            .map(|&q| ((q as i32 - self.zero_point as i32) as f32) * self.scale)
            .collect()
    }

    /// Get memory size in bytes
    pub fn size_bytes(&self) -> usize {
        self.data.len() + 4 + 1 // data + scale + zero_point
    }
}

/// 3-bit quantizer for reflex signals
///
/// Uses signed int4 container with values restricted to -4..3.
/// Optimized for LUT-based activation.
#[derive(Debug, Clone)]
pub struct Quantizer3Bit {
    /// Per-block scale factors
    pub scales: Vec<f32>,
    /// Block size (typically 32)
    pub block_size: usize,
    /// LUT for activation (optional)
    pub activation_lut: Option<[f32; 8]>,
}

impl Quantizer3Bit {
    /// Create a new 3-bit quantizer
    pub fn new(block_size: usize) -> Self {
        Self {
            scales: Vec::new(),
            block_size,
            activation_lut: None,
        }
    }

    /// Set activation LUT (e.g., for ReLU)
    pub fn with_activation_lut(mut self, lut: [f32; 8]) -> Self {
        self.activation_lut = Some(lut);
        self
    }

    /// Quantize f32 values to 3-bit
    pub fn quantize(&mut self, values: &[f32]) -> Vec<u8> {
        let num_blocks = (values.len() + self.block_size - 1) / self.block_size;
        self.scales = Vec::with_capacity(num_blocks);

        let mut result = Vec::with_capacity((values.len() + 1) / 2); // Pack 2 values per byte

        for block in values.chunks(self.block_size) {
            // Find scale for this block
            let max_abs = block.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
            let scale = if max_abs > 0.0 { max_abs / 3.0 } else { 1.0 }; // 3-bit max is 3
            self.scales.push(scale);

            // Quantize values
            for pair in block.chunks(2) {
                let q0 = Self::quantize_value(pair[0], scale);
                let q1 = if pair.len() > 1 {
                    Self::quantize_value(pair[1], scale)
                } else {
                    0
                };
                // Pack two 4-bit values into one byte
                result.push(((q1 as u8) << 4) | (q0 as u8 & 0x0F));
            }
        }

        result
    }

    /// Quantize single value to 3-bit
    fn quantize_value(value: f32, scale: f32) -> i8 {
        let scaled = (value / scale).round() as i8;
        scaled.clamp(-4, 3)
    }

    /// Dequantize 3-bit values to f32
    pub fn dequantize(&self, data: &[u8], num_values: usize) -> Vec<f32> {
        let mut result = Vec::with_capacity(num_values);
        let mut value_idx = 0;
        let mut block_idx = 0;

        for &byte in data {
            if value_idx >= num_values {
                break;
            }

            let scale = self.scales.get(block_idx).copied().unwrap_or(1.0);

            // Unpack first value (lower 4 bits)
            let q0 = (byte & 0x0F) as i8;
            let q0 = if q0 > 7 { q0 - 16 } else { q0 }; // Sign extend
            let v0 = (q0 as f32) * scale;

            // Apply activation LUT if present
            let v0 = if let Some(ref lut) = self.activation_lut {
                lut[(q0 + 4) as usize]
            } else {
                v0
            };

            result.push(v0);
            value_idx += 1;

            if value_idx >= num_values {
                break;
            }

            // Unpack second value (upper 4 bits)
            let q1 = ((byte >> 4) & 0x0F) as i8;
            let q1 = if q1 > 7 { q1 - 16 } else { q1 };
            let v1 = (q1 as f32) * scale;

            let v1 = if let Some(ref lut) = self.activation_lut {
                lut[(q1 + 4) as usize]
            } else {
                v1
            };

            result.push(v1);
            value_idx += 1;

            // Update block index
            if value_idx % self.block_size == 0 {
                block_idx += 1;
            }
        }

        result
    }
}

/// 5-bit quantizer for streaming embeddings
///
/// Uses signed int8 container with values in -16..15.
/// Per-channel or per-block scale for stable streaming updates.
#[derive(Debug, Clone)]
pub struct Quantizer5Bit {
    /// Per-block scale factors
    pub scales: Vec<f32>,
    /// Block size
    pub block_size: usize,
    /// Use per-channel scaling (instead of per-block)
    pub per_channel: bool,
}

impl Quantizer5Bit {
    /// Create a new 5-bit quantizer
    pub fn new(block_size: usize) -> Self {
        Self {
            scales: Vec::new(),
            block_size,
            per_channel: false,
        }
    }

    /// Enable per-channel scaling
    pub fn with_per_channel(mut self) -> Self {
        self.per_channel = true;
        self
    }

    /// Quantize f32 values to 5-bit (stored in int8)
    pub fn quantize(&mut self, values: &[f32]) -> Vec<i8> {
        if self.per_channel {
            self.quantize_per_channel(values)
        } else {
            self.quantize_per_block(values)
        }
    }

    fn quantize_per_block(&mut self, values: &[f32]) -> Vec<i8> {
        let num_blocks = (values.len() + self.block_size - 1) / self.block_size;
        self.scales = Vec::with_capacity(num_blocks);

        let mut result = Vec::with_capacity(values.len());

        for block in values.chunks(self.block_size) {
            let max_abs = block.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
            let scale = if max_abs > 0.0 { max_abs / 15.0 } else { 1.0 }; // 5-bit max is 15
            self.scales.push(scale);

            for &value in block {
                let q = (value / scale).round() as i8;
                result.push(q.clamp(-16, 15));
            }
        }

        result
    }

    fn quantize_per_channel(&mut self, values: &[f32]) -> Vec<i8> {
        self.scales = Vec::with_capacity(values.len());

        values
            .iter()
            .map(|&value| {
                let max_abs = value.abs();
                let scale = if max_abs > 0.0 { max_abs / 15.0 } else { 1.0 };
                self.scales.push(scale);
                let q = (value / scale).round() as i8;
                q.clamp(-16, 15)
            })
            .collect()
    }

    /// Dequantize 5-bit values to f32
    pub fn dequantize(&self, data: &[i8]) -> Vec<f32> {
        if self.per_channel {
            data.iter()
                .zip(self.scales.iter())
                .map(|(&q, &scale)| (q as f32) * scale)
                .collect()
        } else {
            let mut result = Vec::with_capacity(data.len());
            let mut block_idx = 0;

            for (i, &q) in data.iter().enumerate() {
                let scale = self.scales.get(block_idx).copied().unwrap_or(1.0);
                result.push((q as f32) * scale);

                if (i + 1) % self.block_size == 0 {
                    block_idx += 1;
                }
            }

            result
        }
    }
}

/// 7-bit quantizer for reasoning
///
/// Uses signed int8 container with values in -64..63.
/// Stable accumulators, close to int8 quality.
#[derive(Debug, Clone)]
pub struct Quantizer7Bit {
    /// Per-block scale factors
    pub scales: Vec<f32>,
    /// Block size
    pub block_size: usize,
}

impl Quantizer7Bit {
    /// Create a new 7-bit quantizer
    pub fn new(block_size: usize) -> Self {
        Self {
            scales: Vec::new(),
            block_size,
        }
    }

    /// Quantize f32 values to 7-bit (stored in int8)
    pub fn quantize(&mut self, values: &[f32]) -> Vec<i8> {
        let num_blocks = (values.len() + self.block_size - 1) / self.block_size;
        self.scales = Vec::with_capacity(num_blocks);

        let mut result = Vec::with_capacity(values.len());

        for block in values.chunks(self.block_size) {
            let max_abs = block.iter().map(|x| x.abs()).fold(0.0f32, f32::max);
            let scale = if max_abs > 0.0 { max_abs / 63.0 } else { 1.0 }; // 7-bit max is 63
            self.scales.push(scale);

            for &value in block {
                let q = (value / scale).round() as i8;
                result.push(q.clamp(-64, 63));
            }
        }

        result
    }

    /// Dequantize 7-bit values to f32
    pub fn dequantize(&self, data: &[i8]) -> Vec<f32> {
        let mut result = Vec::with_capacity(data.len());
        let mut block_idx = 0;

        for (i, &q) in data.iter().enumerate() {
            let scale = self.scales.get(block_idx).copied().unwrap_or(1.0);
            result.push((q as f32) * scale);

            if (i + 1) % self.block_size == 0 {
                block_idx += 1;
            }
        }

        result
    }

    /// Apply micro-LoRA delta (in 7-bit precision)
    pub fn apply_lora_delta(&mut self, base: &[i8], delta: &[i8], alpha: f32) -> Vec<i8> {
        base.iter()
            .zip(delta.iter())
            .map(|(&b, &d)| {
                let result = (b as f32) + (d as f32) * alpha;
                (result.round() as i8).clamp(-64, 63)
            })
            .collect()
    }
}

/// Unified quantizer that selects appropriate implementation
#[derive(Debug, Clone)]
pub enum LaneQuantizer {
    Bit3(Quantizer3Bit),
    Bit5(Quantizer5Bit),
    Bit7(Quantizer7Bit),
}

impl LaneQuantizer {
    /// Create quantizer for a specific lane
    pub fn for_lane(lane: PrecisionLane, block_size: usize) -> Self {
        match lane {
            PrecisionLane::Bit3 => Self::Bit3(Quantizer3Bit::new(block_size)),
            PrecisionLane::Bit5 => Self::Bit5(Quantizer5Bit::new(block_size)),
            PrecisionLane::Bit7 => Self::Bit7(Quantizer7Bit::new(block_size)),
            PrecisionLane::Float32 => Self::Bit7(Quantizer7Bit::new(block_size)), // Fallback
        }
    }

    /// Get the precision lane
    pub fn lane(&self) -> PrecisionLane {
        match self {
            Self::Bit3(_) => PrecisionLane::Bit3,
            Self::Bit5(_) => PrecisionLane::Bit5,
            Self::Bit7(_) => PrecisionLane::Bit7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_3bit_roundtrip() {
        let mut quantizer = Quantizer3Bit::new(32);
        let values: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();

        let quantized = quantizer.quantize(&values);
        let dequantized = quantizer.dequantize(&quantized, values.len());

        assert_eq!(dequantized.len(), values.len());

        // Check error is bounded (3-bit is very lossy - only 8 levels)
        // With range ~6.4 (-3.2 to 3.2), each level is ~0.8, so max error is ~0.4
        // But with grouping, it can be higher
        for (orig, deq) in values.iter().zip(dequantized.iter()) {
            let error = (orig - deq).abs();
            assert!(error < 1.0, "Error too large: {} vs {}", orig, deq);
        }
    }

    #[test]
    fn test_5bit_roundtrip() {
        let mut quantizer = Quantizer5Bit::new(32);
        let values: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();

        let quantized = quantizer.quantize(&values);
        let dequantized = quantizer.dequantize(&quantized);

        assert_eq!(dequantized.len(), values.len());

        for (orig, deq) in values.iter().zip(dequantized.iter()) {
            let error = (orig - deq).abs();
            assert!(error < 0.2, "Error too large: {} vs {}", orig, deq);
        }
    }

    #[test]
    fn test_7bit_roundtrip() {
        let mut quantizer = Quantizer7Bit::new(32);
        let values: Vec<f32> = (0..64).map(|i| (i as f32 - 32.0) * 0.1).collect();

        let quantized = quantizer.quantize(&values);
        let dequantized = quantizer.dequantize(&quantized);

        assert_eq!(dequantized.len(), values.len());

        for (orig, deq) in values.iter().zip(dequantized.iter()) {
            let error = (orig - deq).abs();
            assert!(error < 0.1, "Error too large: {} vs {}", orig, deq);
        }
    }

    #[test]
    fn test_7bit_lora_delta() {
        let mut quantizer = Quantizer7Bit::new(32);
        let base: Vec<i8> = vec![10, 20, 30, 40];
        let delta: Vec<i8> = vec![1, 2, 3, 4];

        let result = quantizer.apply_lora_delta(&base, &delta, 0.5);

        assert_eq!(result[0], 11); // 10 + 1*0.5 = 10.5 -> 11
        assert_eq!(result[1], 21); // 20 + 2*0.5 = 21
        assert_eq!(result[2], 32); // 30 + 3*0.5 = 31.5 -> 32
        assert_eq!(result[3], 42); // 40 + 4*0.5 = 42
    }
}
