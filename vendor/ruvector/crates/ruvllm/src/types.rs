//! Common types used across RuvLLM
//!
//! This module contains shared type definitions, enums, and data structures
//! used throughout the RuvLLM crate.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Model size variants supported by RuvLLM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelSize {
    /// 350M parameter model - fastest, lower quality
    Tiny,
    /// 700M parameter model - balanced
    Small,
    /// 1.2B parameter model - higher quality
    Medium,
    /// 2.6B parameter model - highest quality, slowest
    Large,
}

impl Default for ModelSize {
    fn default() -> Self {
        Self::Small
    }
}

impl ModelSize {
    /// Get the approximate parameter count
    pub fn param_count(&self) -> usize {
        match self {
            Self::Tiny => 350_000_000,
            Self::Small => 700_000_000,
            Self::Medium => 1_200_000_000,
            Self::Large => 2_600_000_000,
        }
    }

    /// Get the model name string
    pub fn name(&self) -> &'static str {
        match self {
            Self::Tiny => "350M",
            Self::Small => "700M",
            Self::Medium => "1.2B",
            Self::Large => "2.6B",
        }
    }
}

/// Precision levels for quantization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Precision {
    /// Full precision (32-bit float)
    FP32,
    /// Half precision (16-bit float)
    FP16,
    /// 8-bit quantization
    Q8,
    /// 4-bit quantization (K-quants)
    Q4K,
    /// 4-bit quantization (standard)
    Q4,
}

impl Default for Precision {
    fn default() -> Self {
        Self::FP16
    }
}

impl Precision {
    /// Get bytes per element
    pub fn bytes_per_element(&self) -> f32 {
        match self {
            Self::FP32 => 4.0,
            Self::FP16 => 2.0,
            Self::Q8 => 1.0,
            Self::Q4K => 0.5,
            Self::Q4 => 0.5,
        }
    }

    /// Get the compression ratio relative to FP32
    pub fn compression_ratio(&self) -> f32 {
        4.0 / self.bytes_per_element()
    }
}

/// Allocation types sharing the unified memory pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AllocationType {
    /// KV cache pages
    KvCache {
        /// Associated session ID
        session_id: String,
        /// Cache tier
        tier: String,
        /// Number of pages allocated
        page_count: usize,
    },
    /// LoRA adapter weights
    LoraAdapter {
        /// Adapter identifier
        adapter_id: String,
        /// LoRA rank
        rank: usize,
        /// Number of layers
        layer_count: usize,
    },
    /// Router weights
    RouterWeights {
        /// Version number
        version: u64,
    },
}

/// Allocation tracking entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Allocation {
    /// Unique allocation ID
    pub id: Uuid,
    /// Allocation type
    pub allocation_type: AllocationType,
    /// Size in bytes
    pub size_bytes: usize,
    /// Priority for eviction (lower = evict first)
    pub priority: f32,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last access timestamp
    pub last_accessed: DateTime<Utc>,
}

/// Memory pool statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryStats {
    /// Total memory budget
    pub total_budget: usize,
    /// Currently allocated bytes
    pub allocated_bytes: usize,
    /// Number of active allocations
    pub allocation_count: usize,
    /// KV cache allocations
    pub kv_cache_bytes: usize,
    /// LoRA adapter allocations
    pub lora_adapter_bytes: usize,
    /// Router weight allocations
    pub router_bytes: usize,
}

/// Request metadata for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestMetadata {
    /// Unique request ID
    pub request_id: Uuid,
    /// Session ID
    pub session_id: String,
    /// User ID if available
    pub user_id: Option<String>,
    /// Request timestamp
    pub timestamp: DateTime<Utc>,
    /// Input token count
    pub input_tokens: usize,
    /// Output token count
    pub output_tokens: usize,
}

/// Error information for witness logging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorInfo {
    /// Error code
    pub code: String,
    /// Error message
    pub message: String,
    /// Stack trace if available
    pub stack_trace: Option<String>,
    /// Recovery attempted
    pub recovery_attempted: bool,
}

/// Quality metrics for evaluation
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QualityMetrics {
    /// Overall quality score (0.0 - 1.0)
    pub overall_score: f32,
    /// Relevance score
    pub relevance: f32,
    /// Coherence score
    pub coherence: f32,
    /// Factuality score
    pub factuality: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_size() {
        assert_eq!(ModelSize::Tiny.param_count(), 350_000_000);
        assert_eq!(ModelSize::Large.name(), "2.6B");
    }

    #[test]
    fn test_precision() {
        assert_eq!(Precision::FP32.bytes_per_element(), 4.0);
        assert_eq!(Precision::Q4.compression_ratio(), 8.0);
    }
}
