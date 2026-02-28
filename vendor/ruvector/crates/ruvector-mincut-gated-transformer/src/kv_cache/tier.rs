//! Tier definitions for the three-tier KV cache architecture.
//!
//! Defines the Hot, Warm, and Archive tiers with their characteristics.

use core::fmt;

/// Cache tier classification
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CacheTier {
    /// Tier 1: Hot buffer - FP16/BF16 full precision
    /// For recent tokens (typically last 64 tokens)
    Hot,
    /// Tier 2: Warm cache - 4-bit KIVI quantization
    /// For intermediate tokens (typically positions 64-512)
    Warm,
    /// Tier 3: Archive - 2-bit KIVI/SQuat/KVQuant
    /// For stale tokens (positions > 512)
    Archive,
}

impl CacheTier {
    /// Get the quantization bits for this tier
    #[inline]
    pub fn bits(&self) -> u8 {
        match self {
            CacheTier::Hot => 16, // FP16
            CacheTier::Warm => 4,
            CacheTier::Archive => 2,
        }
    }

    /// Get compression ratio compared to FP16
    #[inline]
    pub fn compression_ratio(&self) -> f32 {
        match self {
            CacheTier::Hot => 1.0,
            CacheTier::Warm => 4.0,    // 16/4
            CacheTier::Archive => 8.0, // 16/2
        }
    }

    /// Get expected PPL degradation
    #[inline]
    pub fn expected_ppl_delta(&self) -> f32 {
        match self {
            CacheTier::Hot => 0.0,
            CacheTier::Warm => 0.05,
            CacheTier::Archive => 0.3,
        }
    }

    /// Check if dequantization is required for attention
    #[inline]
    pub fn requires_dequantization(&self) -> bool {
        !matches!(self, CacheTier::Hot)
    }
}

impl fmt::Display for CacheTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CacheTier::Hot => write!(f, "Hot (FP16)"),
            CacheTier::Warm => write!(f, "Warm (4-bit)"),
            CacheTier::Archive => write!(f, "Archive (2-bit)"),
        }
    }
}

/// Configuration for tier boundaries
#[derive(Debug, Clone, Copy)]
pub struct TierBoundary {
    /// Tokens newer than this are in Hot tier
    pub hot_threshold: usize,
    /// Tokens older than hot but newer than this are in Warm tier
    pub warm_threshold: usize,
}

impl Default for TierBoundary {
    fn default() -> Self {
        Self {
            hot_threshold: 64,
            warm_threshold: 512,
        }
    }
}

impl TierBoundary {
    /// Create tier boundary with custom thresholds
    pub fn new(hot: usize, warm: usize) -> Self {
        assert!(hot < warm, "hot_threshold must be less than warm_threshold");
        Self {
            hot_threshold: hot,
            warm_threshold: warm,
        }
    }

    /// Determine tier for a token based on its age (distance from current position)
    #[inline]
    pub fn tier_for_age(&self, age: usize) -> CacheTier {
        if age < self.hot_threshold {
            CacheTier::Hot
        } else if age < self.warm_threshold {
            CacheTier::Warm
        } else {
            CacheTier::Archive
        }
    }

    /// Determine tier for a token position given current sequence length
    #[inline]
    pub fn tier_for_position(&self, position: usize, current_len: usize) -> CacheTier {
        if current_len <= position {
            return CacheTier::Hot; // Future or current position
        }
        let age = current_len - position - 1;
        self.tier_for_age(age)
    }

    /// Get the number of tokens in each tier
    pub fn tier_counts(&self, total_len: usize) -> TierCounts {
        if total_len == 0 {
            return TierCounts::default();
        }

        let hot_count = self.hot_threshold.min(total_len);
        let warm_count = if total_len > self.hot_threshold {
            (self.warm_threshold - self.hot_threshold).min(total_len - self.hot_threshold)
        } else {
            0
        };
        let archive_count = total_len.saturating_sub(self.warm_threshold);

        TierCounts {
            hot: hot_count,
            warm: warm_count,
            archive: archive_count,
        }
    }
}

/// Token counts per tier
#[derive(Debug, Clone, Copy, Default)]
pub struct TierCounts {
    /// Number of tokens in hot tier
    pub hot: usize,
    /// Number of tokens in warm tier
    pub warm: usize,
    /// Number of tokens in archive tier
    pub archive: usize,
}

impl TierCounts {
    /// Total number of tokens across all tiers
    #[inline]
    pub fn total(&self) -> usize {
        self.hot + self.warm + self.archive
    }

    /// Calculate memory usage in bytes given head dimension
    pub fn memory_bytes(&self, head_dim: usize, num_heads: usize, num_layers: usize) -> usize {
        let bytes_per_element = 2; // FP16
        let kv_factor = 2; // Keys and Values

        // Hot: FP16 (2 bytes)
        let hot_bytes = self.hot * head_dim * bytes_per_element;

        // Warm: 4-bit (0.5 bytes) + scale overhead
        let warm_bytes = (self.warm * head_dim) / 2 + self.warm * 4; // 4 bytes scale per token

        // Archive: 2-bit (0.25 bytes) + scale overhead
        let archive_bytes = (self.archive * head_dim) / 4 + self.archive * 4;

        (hot_bytes + warm_bytes + archive_bytes) * num_heads * num_layers * kv_factor
    }
}

/// Configuration for tier behavior
#[derive(Debug, Clone)]
pub struct TierConfig {
    /// Tier boundary thresholds
    pub boundary: TierBoundary,
    /// Whether to use adaptive boundaries based on quality metrics
    pub adaptive: bool,
    /// Minimum hot buffer size (never reduce below this)
    pub min_hot_size: usize,
    /// Maximum hot buffer size (never increase above this)
    pub max_hot_size: usize,
    /// Quality threshold for boundary adaptation (0.0 - 1.0)
    pub quality_threshold: f32,
}

impl Default for TierConfig {
    fn default() -> Self {
        Self {
            boundary: TierBoundary::default(),
            adaptive: true,
            min_hot_size: 32,
            max_hot_size: 256,
            quality_threshold: 0.95,
        }
    }
}

impl TierConfig {
    /// Create a configuration for long contexts (> 8K tokens)
    pub fn long_context() -> Self {
        Self {
            boundary: TierBoundary::new(64, 1024),
            adaptive: true,
            min_hot_size: 64,
            max_hot_size: 512,
            quality_threshold: 0.95,
        }
    }

    /// Create a configuration for extreme contexts (> 32K tokens)
    pub fn extreme_context() -> Self {
        Self {
            boundary: TierBoundary::new(128, 2048),
            adaptive: true,
            min_hot_size: 64,
            max_hot_size: 256,
            quality_threshold: 0.97,
        }
    }

    /// Create a memory-optimized configuration
    pub fn memory_optimized() -> Self {
        Self {
            boundary: TierBoundary::new(32, 256),
            adaptive: false,
            min_hot_size: 32,
            max_hot_size: 64,
            quality_threshold: 0.90,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tier_bits() {
        assert_eq!(CacheTier::Hot.bits(), 16);
        assert_eq!(CacheTier::Warm.bits(), 4);
        assert_eq!(CacheTier::Archive.bits(), 2);
    }

    #[test]
    fn test_tier_compression() {
        assert_eq!(CacheTier::Hot.compression_ratio(), 1.0);
        assert_eq!(CacheTier::Warm.compression_ratio(), 4.0);
        assert_eq!(CacheTier::Archive.compression_ratio(), 8.0);
    }

    #[test]
    fn test_tier_boundary_default() {
        let boundary = TierBoundary::default();
        assert_eq!(boundary.hot_threshold, 64);
        assert_eq!(boundary.warm_threshold, 512);
    }

    #[test]
    fn test_tier_for_age() {
        let boundary = TierBoundary::new(64, 512);

        assert_eq!(boundary.tier_for_age(0), CacheTier::Hot);
        assert_eq!(boundary.tier_for_age(63), CacheTier::Hot);
        assert_eq!(boundary.tier_for_age(64), CacheTier::Warm);
        assert_eq!(boundary.tier_for_age(511), CacheTier::Warm);
        assert_eq!(boundary.tier_for_age(512), CacheTier::Archive);
        assert_eq!(boundary.tier_for_age(10000), CacheTier::Archive);
    }

    #[test]
    fn test_tier_counts() {
        let boundary = TierBoundary::new(64, 512);

        // Small sequence
        let counts = boundary.tier_counts(50);
        assert_eq!(counts.hot, 50);
        assert_eq!(counts.warm, 0);
        assert_eq!(counts.archive, 0);

        // Medium sequence
        let counts = boundary.tier_counts(256);
        assert_eq!(counts.hot, 64);
        assert_eq!(counts.warm, 192);
        assert_eq!(counts.archive, 0);

        // Large sequence
        let counts = boundary.tier_counts(1024);
        assert_eq!(counts.hot, 64);
        assert_eq!(counts.warm, 448);
        assert_eq!(counts.archive, 512);
    }

    #[test]
    #[should_panic(expected = "hot_threshold must be less than warm_threshold")]
    fn test_invalid_boundary() {
        let _boundary = TierBoundary::new(512, 64);
    }
}
