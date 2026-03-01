//! GPU Configuration for RuVector ONNX Embeddings
//!
//! Provides configuration options for GPU acceleration including
//! device selection, memory limits, and performance tuning.

use serde::{Deserialize, Serialize};

/// GPU execution mode
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum GpuMode {
    /// Automatically select best available backend
    #[default]
    Auto,
    /// Force WebGPU backend
    WebGpu,
    /// Force CUDA-WASM transpiled backend
    CudaWasm,
    /// CPU-only (disable GPU)
    CpuOnly,
}

/// Power preference for GPU device selection
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerPreference {
    /// Prefer low power consumption (integrated GPU)
    LowPower,
    /// Prefer high performance (discrete GPU)
    #[default]
    HighPerformance,
    /// No preference
    None,
}

/// GPU acceleration configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuConfig {
    /// GPU execution mode
    pub mode: GpuMode,

    /// Power preference for device selection
    pub power_preference: PowerPreference,

    /// Maximum GPU memory usage (bytes, 0 = unlimited)
    pub max_memory: u64,

    /// Workgroup size for compute shaders (0 = auto)
    pub workgroup_size: u32,

    /// Enable async GPU operations
    pub async_compute: bool,

    /// Minimum batch size to use GPU (smaller batches use CPU)
    pub min_batch_size: usize,

    /// Minimum vector dimension to use GPU
    pub min_dimension: usize,

    /// Enable shader caching
    pub cache_shaders: bool,

    /// Enable profiling and timing
    pub enable_profiling: bool,

    /// Fallback to CPU on GPU error
    pub fallback_to_cpu: bool,

    /// Device index (for multi-GPU systems)
    pub device_index: u32,
}

impl Default for GpuConfig {
    fn default() -> Self {
        Self {
            mode: GpuMode::Auto,
            power_preference: PowerPreference::HighPerformance,
            max_memory: 0, // unlimited
            workgroup_size: 256,
            async_compute: true,
            min_batch_size: 16,
            min_dimension: 128,
            cache_shaders: true,
            enable_profiling: false,
            fallback_to_cpu: true,
            device_index: 0,
        }
    }
}

impl GpuConfig {
    /// Create configuration with automatic settings
    pub fn auto() -> Self {
        Self::default()
    }

    /// Create configuration for high performance
    pub fn high_performance() -> Self {
        Self {
            mode: GpuMode::Auto,
            power_preference: PowerPreference::HighPerformance,
            workgroup_size: 512,
            async_compute: true,
            min_batch_size: 8,
            min_dimension: 64,
            ..Default::default()
        }
    }

    /// Create configuration for low power usage
    pub fn low_power() -> Self {
        Self {
            mode: GpuMode::Auto,
            power_preference: PowerPreference::LowPower,
            workgroup_size: 128,
            async_compute: false,
            min_batch_size: 32,
            min_dimension: 256,
            ..Default::default()
        }
    }

    /// Create CPU-only configuration
    pub fn cpu_only() -> Self {
        Self {
            mode: GpuMode::CpuOnly,
            ..Default::default()
        }
    }

    /// Create WebGPU-specific configuration
    pub fn webgpu() -> Self {
        Self {
            mode: GpuMode::WebGpu,
            ..Default::default()
        }
    }

    /// Create CUDA-WASM specific configuration
    #[cfg(feature = "cuda-wasm")]
    pub fn cuda_wasm() -> Self {
        Self {
            mode: GpuMode::CudaWasm,
            workgroup_size: 256,
            ..Default::default()
        }
    }

    // Builder methods

    /// Set GPU mode
    pub fn with_mode(mut self, mode: GpuMode) -> Self {
        self.mode = mode;
        self
    }

    /// Set power preference
    pub fn with_power_preference(mut self, pref: PowerPreference) -> Self {
        self.power_preference = pref;
        self
    }

    /// Set maximum memory
    pub fn with_max_memory(mut self, bytes: u64) -> Self {
        self.max_memory = bytes;
        self
    }

    /// Set workgroup size
    pub fn with_workgroup_size(mut self, size: u32) -> Self {
        self.workgroup_size = size;
        self
    }

    /// Set minimum batch size for GPU usage
    pub fn with_min_batch_size(mut self, size: usize) -> Self {
        self.min_batch_size = size;
        self
    }

    /// Set minimum dimension for GPU usage
    pub fn with_min_dimension(mut self, dim: usize) -> Self {
        self.min_dimension = dim;
        self
    }

    /// Enable or disable profiling
    pub fn with_profiling(mut self, enable: bool) -> Self {
        self.enable_profiling = enable;
        self
    }

    /// Enable or disable CPU fallback
    pub fn with_fallback(mut self, enable: bool) -> Self {
        self.fallback_to_cpu = enable;
        self
    }

    /// Set device index
    pub fn with_device(mut self, index: u32) -> Self {
        self.device_index = index;
        self
    }

    /// Check if GPU should be used for given workload
    pub fn should_use_gpu(&self, batch_size: usize, dimension: usize) -> bool {
        self.mode != GpuMode::CpuOnly
            && batch_size >= self.min_batch_size
            && dimension >= self.min_dimension
    }
}

/// GPU memory statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuMemoryStats {
    /// Total GPU memory (bytes)
    pub total: u64,
    /// Used GPU memory (bytes)
    pub used: u64,
    /// Free GPU memory (bytes)
    pub free: u64,
    /// Peak usage (bytes)
    pub peak: u64,
}

impl GpuMemoryStats {
    /// Get usage percentage
    pub fn usage_percent(&self) -> f32 {
        if self.total > 0 {
            (self.used as f32 / self.total as f32) * 100.0
        } else {
            0.0
        }
    }
}

/// GPU profiling data
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GpuProfilingData {
    /// Total operations executed
    pub operations: u64,
    /// Total GPU time (microseconds)
    pub gpu_time_us: u64,
    /// Total CPU time (microseconds)
    pub cpu_time_us: u64,
    /// GPU speedup over CPU
    pub speedup: f32,
    /// Memory transfers (bytes)
    pub memory_transferred: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GpuConfig::default();
        assert_eq!(config.mode, GpuMode::Auto);
        assert_eq!(config.power_preference, PowerPreference::HighPerformance);
        assert!(config.fallback_to_cpu);
    }

    #[test]
    fn test_should_use_gpu() {
        let config = GpuConfig::default()
            .with_min_batch_size(16)
            .with_min_dimension(128);

        assert!(!config.should_use_gpu(8, 384));   // batch too small
        assert!(!config.should_use_gpu(32, 64));   // dimension too small
        assert!(config.should_use_gpu(32, 384));   // both ok
    }

    #[test]
    fn test_cpu_only() {
        let config = GpuConfig::cpu_only();
        assert!(!config.should_use_gpu(1000, 1000));
    }

    #[test]
    fn test_builder() {
        let config = GpuConfig::auto()
            .with_mode(GpuMode::WebGpu)
            .with_max_memory(1024 * 1024 * 1024)
            .with_workgroup_size(512)
            .with_profiling(true);

        assert_eq!(config.mode, GpuMode::WebGpu);
        assert_eq!(config.max_memory, 1024 * 1024 * 1024);
        assert_eq!(config.workgroup_size, 512);
        assert!(config.enable_profiling);
    }
}
