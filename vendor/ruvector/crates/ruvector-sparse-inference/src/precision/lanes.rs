//! Precision Lane definitions and configuration
//!
//! Defines the three precision lanes (3/5/7-bit) that map to intelligence roles.

use serde::{Deserialize, Serialize};

/// Precision lanes for layered quantization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PrecisionLane {
    /// 3-bit lane: Reflex signals, gating, boundaries, health metrics
    /// Uses signed int4 container restricted to 3-bit domain
    /// LUT activation for speed
    Bit3,

    /// 5-bit lane: Streaming embeddings, semantic motion, drift detection
    /// Uses signed int8 container with values in -16..15
    /// Per-channel or per-block scale
    Bit5,

    /// 7-bit lane: Reasoning, synthesis, memory writes, micro-LoRA
    /// Uses signed int8 container with values in -64..63
    /// Stable accumulators, close to int8 quality
    Bit7,

    /// Float lane: Training, calibration, aggregation boundaries only
    Float32,
}

impl PrecisionLane {
    /// Get the number of bits for this lane
    pub fn bits(&self) -> u8 {
        match self {
            Self::Bit3 => 3,
            Self::Bit5 => 5,
            Self::Bit7 => 7,
            Self::Float32 => 32,
        }
    }

    /// Get the value range for this lane
    pub fn value_range(&self) -> (i32, i32) {
        match self {
            Self::Bit3 => (-4, 3),   // 3-bit signed: -4 to 3
            Self::Bit5 => (-16, 15), // 5-bit signed: -16 to 15
            Self::Bit7 => (-64, 63), // 7-bit signed: -64 to 63
            Self::Float32 => (i32::MIN, i32::MAX),
        }
    }

    /// Get bytes per element (storage container)
    pub fn bytes_per_element(&self) -> f32 {
        match self {
            Self::Bit3 => 0.5, // Packed into int4
            Self::Bit5 => 1.0, // int8 container
            Self::Bit7 => 1.0, // int8 container
            Self::Float32 => 4.0,
        }
    }

    /// Get the default scale factor for this lane
    pub fn default_scale(&self) -> f32 {
        match self {
            Self::Bit3 => 0.25,     // Conservative for reflexes
            Self::Bit5 => 0.0625,   // 1/16 for streaming
            Self::Bit7 => 0.015625, // 1/64 for reasoning
            Self::Float32 => 1.0,
        }
    }

    /// Check if this lane supports memory writes
    pub fn allows_memory_writes(&self) -> bool {
        matches!(self, Self::Bit7 | Self::Float32)
    }

    /// Check if this lane is event-driven vs continuous
    pub fn is_event_driven(&self) -> bool {
        matches!(self, Self::Bit5 | Self::Bit7)
    }
}

impl Default for PrecisionLane {
    fn default() -> Self {
        Self::Bit7 // Default to reasoning lane
    }
}

/// Configuration for precision lane behavior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaneConfig {
    /// Default lane for new operations
    pub default_lane: PrecisionLane,

    /// Time budget per tick for 3-bit lane (microseconds)
    pub bit3_tick_budget_us: u64,

    /// Maximum consecutive 5-bit updates before forced graduation check
    pub bit5_max_updates: usize,

    /// Minimum stability steps before demotion
    pub min_stability_steps: usize,

    /// Novelty threshold for escalation (0.0 to 1.0)
    pub novelty_threshold: f32,

    /// Drift persistence threshold (steps)
    pub drift_persistence_threshold: usize,

    /// Confidence threshold for graduation (0.0 to 1.0)
    pub confidence_threshold: f32,

    /// Cost budget for escalation (arbitrary units)
    pub escalation_budget: f32,

    /// Enable automatic lane selection
    pub auto_lane_selection: bool,
}

impl Default for LaneConfig {
    fn default() -> Self {
        Self {
            default_lane: PrecisionLane::Bit5, // Start at streaming lane
            bit3_tick_budget_us: 100,          // 100μs per tick for reflexes
            bit5_max_updates: 10,              // Check graduation every 10 updates
            min_stability_steps: 5,            // 5 stable steps before demotion
            novelty_threshold: 0.3,            // 30% novelty triggers escalation
            drift_persistence_threshold: 3,    // 3 steps of drift
            confidence_threshold: 0.7,         // 70% confidence required
            escalation_budget: 1.0,            // Normalized budget
            auto_lane_selection: true,
        }
    }
}

/// Hardware target for lane optimization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HardwareTarget {
    /// ESP32: 3-bit only, tiny models
    Esp32,
    /// V0 Appliance: 5-bit streaming + 7-bit reasoning
    V0Appliance,
    /// Desktop/Server: Full lane support
    Desktop,
    /// FPGA: Deterministic 7-bit with witness logging
    Fpga,
}

impl HardwareTarget {
    /// Get supported lanes for this hardware
    pub fn supported_lanes(&self) -> Vec<PrecisionLane> {
        match self {
            Self::Esp32 => vec![PrecisionLane::Bit3],
            Self::V0Appliance => vec![
                PrecisionLane::Bit3,
                PrecisionLane::Bit5,
                PrecisionLane::Bit7,
            ],
            Self::Desktop => vec![
                PrecisionLane::Bit3,
                PrecisionLane::Bit5,
                PrecisionLane::Bit7,
                PrecisionLane::Float32,
            ],
            Self::Fpga => vec![PrecisionLane::Bit7],
        }
    }

    /// Get the default lane for this hardware
    pub fn default_lane(&self) -> PrecisionLane {
        match self {
            Self::Esp32 => PrecisionLane::Bit3,
            Self::V0Appliance => PrecisionLane::Bit5,
            Self::Desktop => PrecisionLane::Bit7,
            Self::Fpga => PrecisionLane::Bit7,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lane_bits() {
        assert_eq!(PrecisionLane::Bit3.bits(), 3);
        assert_eq!(PrecisionLane::Bit5.bits(), 5);
        assert_eq!(PrecisionLane::Bit7.bits(), 7);
        assert_eq!(PrecisionLane::Float32.bits(), 32);
    }

    #[test]
    fn test_lane_ranges() {
        assert_eq!(PrecisionLane::Bit3.value_range(), (-4, 3));
        assert_eq!(PrecisionLane::Bit5.value_range(), (-16, 15));
        assert_eq!(PrecisionLane::Bit7.value_range(), (-64, 63));
    }

    #[test]
    fn test_memory_write_permission() {
        assert!(!PrecisionLane::Bit3.allows_memory_writes());
        assert!(!PrecisionLane::Bit5.allows_memory_writes());
        assert!(PrecisionLane::Bit7.allows_memory_writes());
        assert!(PrecisionLane::Float32.allows_memory_writes());
    }

    #[test]
    fn test_hardware_targets() {
        assert_eq!(
            HardwareTarget::Esp32.supported_lanes(),
            vec![PrecisionLane::Bit3]
        );
        assert!(HardwareTarget::Desktop
            .supported_lanes()
            .contains(&PrecisionLane::Float32));
    }
}
