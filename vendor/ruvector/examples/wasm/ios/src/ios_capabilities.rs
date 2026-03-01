//! iOS Capability Detection & Optimization Module
//!
//! Provides runtime detection of iOS-specific features and optimization hints.
//! Works with both WasmKit native and Safari WebAssembly runtimes.

// ============================================
// Capability Flags
// ============================================

/// iOS device capability flags (bit flags)
#[repr(u32)]
pub enum Capability {
    /// WASM SIMD128 support (iOS 16.4+)
    Simd128 = 1 << 0,
    /// Bulk memory operations
    BulkMemory = 1 << 1,
    /// Mutable globals
    MutableGlobals = 1 << 2,
    /// Reference types
    ReferenceTypes = 1 << 3,
    /// Multi-value returns
    MultiValue = 1 << 4,
    /// Tail call optimization
    TailCall = 1 << 5,
    /// Relaxed SIMD (iOS 17+)
    RelaxedSimd = 1 << 6,
    /// Exception handling
    ExceptionHandling = 1 << 7,
    /// Memory64 (large memory)
    Memory64 = 1 << 8,
    /// Threads (SharedArrayBuffer)
    Threads = 1 << 9,
}

/// Runtime capabilities structure
#[derive(Clone, Debug, Default)]
pub struct RuntimeCapabilities {
    /// Bitfield of supported capabilities
    pub flags: u32,
    /// Estimated CPU cores (for parallelism hints)
    pub cpu_cores: u8,
    /// Available memory in MB
    pub memory_mb: u32,
    /// Device generation hint (A11=11, A12=12, etc.)
    pub device_gen: u8,
    /// iOS version major (16, 17, etc.)
    pub ios_version: u8,
}

impl RuntimeCapabilities {
    /// Check if a capability is available
    #[inline]
    pub fn has(&self, cap: Capability) -> bool {
        (self.flags & (cap as u32)) != 0
    }

    /// Check if SIMD is available
    #[inline]
    pub fn has_simd(&self) -> bool {
        self.has(Capability::Simd128)
    }

    /// Check if relaxed SIMD is available (FMA, etc.)
    #[inline]
    pub fn has_relaxed_simd(&self) -> bool {
        self.has(Capability::RelaxedSimd)
    }

    /// Check if threading is available
    #[inline]
    pub fn has_threads(&self) -> bool {
        self.has(Capability::Threads)
    }

    /// Get recommended batch size for operations
    #[inline]
    pub fn recommended_batch_size(&self) -> usize {
        if self.has_simd() {
            if self.device_gen >= 15 { 256 }      // A15+ (iPhone 13+)
            else if self.device_gen >= 13 { 128 } // A13-A14
            else { 64 }                           // A11-A12
        } else {
            32 // Fallback
        }
    }

    /// Get recommended embedding cache size
    #[inline]
    pub fn recommended_cache_size(&self) -> usize {
        let base = if self.memory_mb >= 4096 { 1000 }  // 4GB+ devices
                   else if self.memory_mb >= 2048 { 500 }
                   else { 100 };
        base
    }
}

// ============================================
// Compile-time Detection
// ============================================

/// Detect capabilities at compile time
pub const fn compile_time_capabilities() -> u32 {
    let mut flags = 0u32;

    // SIMD128
    if cfg!(target_feature = "simd128") {
        flags |= Capability::Simd128 as u32;
    }

    // Bulk memory (always enabled in our build)
    if cfg!(target_feature = "bulk-memory") {
        flags |= Capability::BulkMemory as u32;
    }

    // Mutable globals (always enabled in our build)
    if cfg!(target_feature = "mutable-globals") {
        flags |= Capability::MutableGlobals as u32;
    }

    flags
}

/// Get compile-time capability report
#[no_mangle]
pub extern "C" fn get_compile_capabilities() -> u32 {
    compile_time_capabilities()
}

// ============================================
// Optimization Strategies
// ============================================

/// Optimization strategy for different device tiers
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum OptimizationTier {
    /// Minimal - older devices, focus on memory
    Minimal = 0,
    /// Balanced - mid-range devices
    Balanced = 1,
    /// Performance - high-end devices, maximize speed
    Performance = 2,
    /// Ultra - latest devices with all features
    Ultra = 3,
}

impl OptimizationTier {
    /// Determine tier from capabilities
    pub fn from_capabilities(caps: &RuntimeCapabilities) -> Self {
        if caps.device_gen >= 15 && caps.has_relaxed_simd() {
            OptimizationTier::Ultra
        } else if caps.device_gen >= 13 && caps.has_simd() {
            OptimizationTier::Performance
        } else if caps.has_simd() {
            OptimizationTier::Balanced
        } else {
            OptimizationTier::Minimal
        }
    }

    /// Get embedding dimension for this tier
    pub fn embedding_dim(&self) -> usize {
        match self {
            OptimizationTier::Ultra => 128,
            OptimizationTier::Performance => 64,
            OptimizationTier::Balanced => 64,
            OptimizationTier::Minimal => 32,
        }
    }

    /// Get attention heads for this tier
    pub fn attention_heads(&self) -> usize {
        match self {
            OptimizationTier::Ultra => 8,
            OptimizationTier::Performance => 4,
            OptimizationTier::Balanced => 4,
            OptimizationTier::Minimal => 2,
        }
    }

    /// Get Q-learning state buckets for this tier
    pub fn state_buckets(&self) -> usize {
        match self {
            OptimizationTier::Ultra => 64,
            OptimizationTier::Performance => 32,
            OptimizationTier::Balanced => 16,
            OptimizationTier::Minimal => 8,
        }
    }
}

// ============================================
// Memory Optimization
// ============================================

/// Memory pool configuration for iOS
#[derive(Clone, Debug)]
pub struct MemoryConfig {
    /// Main pool size in bytes
    pub main_pool_bytes: usize,
    /// Embedding cache entries
    pub cache_entries: usize,
    /// History buffer size
    pub history_size: usize,
    /// Use memory-mapped I/O hint
    pub use_mmap: bool,
}

impl MemoryConfig {
    /// Create config for given optimization tier
    pub fn for_tier(tier: OptimizationTier) -> Self {
        match tier {
            OptimizationTier::Ultra => Self {
                main_pool_bytes: 4 * 1024 * 1024, // 4MB
                cache_entries: 1000,
                history_size: 200,
                use_mmap: true,
            },
            OptimizationTier::Performance => Self {
                main_pool_bytes: 2 * 1024 * 1024, // 2MB
                cache_entries: 500,
                history_size: 100,
                use_mmap: true,
            },
            OptimizationTier::Balanced => Self {
                main_pool_bytes: 1 * 1024 * 1024, // 1MB
                cache_entries: 200,
                history_size: 50,
                use_mmap: false,
            },
            OptimizationTier::Minimal => Self {
                main_pool_bytes: 512 * 1024, // 512KB
                cache_entries: 100,
                history_size: 25,
                use_mmap: false,
            },
        }
    }
}

// ============================================
// Swift Bridge Info
// ============================================

/// Information for Swift integration
#[repr(C)]
pub struct SwiftBridgeInfo {
    /// WASM module version
    pub version_major: u8,
    pub version_minor: u8,
    pub version_patch: u8,
    /// Feature flags
    pub feature_flags: u32,
    /// Recommended embedding dimension
    pub embedding_dim: u16,
    /// Recommended batch size
    pub batch_size: u16,
}

/// Get bridge info for Swift
#[no_mangle]
pub extern "C" fn get_bridge_info() -> SwiftBridgeInfo {
    SwiftBridgeInfo {
        version_major: 0,
        version_minor: 1,
        version_patch: 0,
        feature_flags: compile_time_capabilities(),
        embedding_dim: 64,
        batch_size: if cfg!(target_feature = "simd128") { 128 } else { 32 },
    }
}

// ============================================
// Neural Engine Offload Hints
// ============================================

/// Operations that could benefit from Neural Engine offload
#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum NeuralEngineOp {
    /// Batch embedding generation
    BatchEmbed = 0,
    /// Large matrix multiply (attention)
    MatMul = 1,
    /// Softmax over large sequences
    Softmax = 2,
    /// Similarity search over many vectors
    BatchSimilarity = 3,
}

/// Check if operation should be offloaded to Neural Engine
pub fn should_offload_to_ane(op: NeuralEngineOp, size: usize) -> bool {
    // Neural Engine is efficient for larger batch sizes
    match op {
        NeuralEngineOp::BatchEmbed => size >= 50,
        NeuralEngineOp::MatMul => size >= 100,
        NeuralEngineOp::Softmax => size >= 256,
        NeuralEngineOp::BatchSimilarity => size >= 100,
    }
}

// ============================================
// Performance Hints Export
// ============================================

/// Get recommended parameters for given device memory (MB)
#[no_mangle]
pub extern "C" fn get_recommended_config(memory_mb: u32) -> u64 {
    // Pack config into u64: [cache_size:16][batch_size:16][dim:16][heads:16]
    let (cache, batch, dim, heads) = if memory_mb >= 4096 {
        (1000u16, 256u16, 128u16, 8u16)
    } else if memory_mb >= 2048 {
        (500u16, 128u16, 64u16, 4u16)
    } else if memory_mb >= 1024 {
        (200u16, 64u16, 64u16, 4u16)
    } else {
        (100u16, 32u16, 32u16, 2u16)
    };

    ((cache as u64) << 48) | ((batch as u64) << 32) | ((dim as u64) << 16) | (heads as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compile_capabilities() {
        let caps = compile_time_capabilities();
        // Should have bulk memory and mutable globals at minimum
        assert!(caps != 0 || !cfg!(target_feature = "bulk-memory"));
    }

    #[test]
    fn test_optimization_tier() {
        let caps = RuntimeCapabilities {
            flags: Capability::Simd128 as u32,
            cpu_cores: 6,
            memory_mb: 4096,
            device_gen: 14,
            ios_version: 17,
        };
        let tier = OptimizationTier::from_capabilities(&caps);
        assert_eq!(tier, OptimizationTier::Performance);
    }

    #[test]
    fn test_memory_config() {
        let config = MemoryConfig::for_tier(OptimizationTier::Performance);
        assert_eq!(config.cache_entries, 500);
    }
}
