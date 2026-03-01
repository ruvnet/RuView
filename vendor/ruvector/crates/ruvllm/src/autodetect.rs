//! Intelligent Auto-Detection System for RuvLLM
//!
//! This module provides automatic detection of system capabilities and optimal
//! configuration selection based on the runtime environment. It handles:
//!
//! - Platform and architecture detection (macOS, Linux, Windows, WASM, iOS, Android)
//! - CPU feature detection (NEON, AVX2, AVX-512, SSE4.2)
//! - GPU capability detection (Metal, CUDA, WebGPU)
//! - Memory and core count detection
//! - Automatic configuration selection based on detected capabilities
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvllm::autodetect::{SystemCapabilities, InferenceConfig};
//!
//! // Auto-detect system capabilities
//! let caps = SystemCapabilities::detect();
//! println!("Platform: {:?}, Arch: {:?}", caps.platform, caps.arch);
//! println!("GPU: {:?}", caps.gpu);
//!
//! // Get optimal configuration
//! let config = caps.optimal_config();
//! println!("Recommended backend: {:?}", config.compute_backend);
//! println!("Recommended threads: {}", config.thread_count);
//!
//! // Or use auto-configuration directly
//! let config = InferenceConfig::auto();
//! ```
//!
//! ## Platform Support Matrix
//!
//! | Platform | Architecture | GPU Backend | Features |
//! |----------|--------------|-------------|----------|
//! | macOS | aarch64 | Metal | NEON always available |
//! | macOS | x86_64 | Metal | AVX2/AVX-512 if available |
//! | Linux | x86_64 | CUDA/CPU | AVX2/AVX-512, SSE4.2 |
//! | Linux | aarch64 | CPU | NEON always available |
//! | Windows | x86_64 | CUDA/CPU | AVX2/AVX-512, SSE4.2 |
//! | WASM | wasm32 | WebGPU | Limited feature detection |
//! | iOS | aarch64 | Metal | NEON always available |
//! | Android | aarch64 | CPU | NEON always available |

use serde::{Deserialize, Serialize};

#[cfg(feature = "coreml")]
use crate::backends::{AneCapabilities, ComputeUnits};
use crate::backends::{DType, DeviceType, Quantization};
use crate::kernels::AttentionConfig;

// =============================================================================
// Platform and Architecture Types
// =============================================================================

/// Supported operating system platforms
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    /// macOS (Intel or Apple Silicon)
    MacOS,
    /// Linux distributions
    Linux,
    /// Windows
    Windows,
    /// WebAssembly (browser or Node.js)
    Wasm,
    /// iOS (iPhone, iPad)
    IOS,
    /// Android
    Android,
    /// Unknown or unsupported platform
    Unknown,
}

impl Default for Platform {
    fn default() -> Self {
        Self::detect()
    }
}

impl Platform {
    /// Detect the current platform at compile time with runtime refinement
    pub fn detect() -> Self {
        #[cfg(target_os = "macos")]
        {
            Self::MacOS
        }

        #[cfg(target_os = "linux")]
        {
            // Check if running on Android (Linux kernel)
            #[cfg(target_os = "android")]
            {
                Self::Android
            }
            #[cfg(not(target_os = "android"))]
            {
                Self::Linux
            }
        }

        #[cfg(target_os = "windows")]
        {
            Self::Windows
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self::Wasm
        }

        #[cfg(target_os = "ios")]
        {
            Self::IOS
        }

        #[cfg(target_os = "android")]
        {
            Self::Android
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_arch = "wasm32",
            target_os = "ios",
            target_os = "android"
        )))]
        {
            Self::Unknown
        }
    }

    /// Check if this platform supports GPU acceleration
    pub fn supports_gpu(&self) -> bool {
        matches!(
            self,
            Self::MacOS | Self::Linux | Self::Windows | Self::IOS | Self::Wasm
        )
    }

    /// Get the default GPU backend for this platform
    pub fn default_gpu_backend(&self) -> Option<GpuBackend> {
        match self {
            Self::MacOS | Self::IOS => Some(GpuBackend::Metal),
            Self::Linux | Self::Windows => Some(GpuBackend::Cuda),
            Self::Wasm => Some(GpuBackend::WebGPU),
            Self::Android | Self::Unknown => None,
        }
    }
}

/// CPU architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Architecture {
    /// ARM 64-bit (Apple Silicon, ARM servers)
    Aarch64,
    /// x86 64-bit (Intel, AMD)
    X86_64,
    /// WebAssembly 32-bit
    Wasm32,
    /// Unknown architecture
    Unknown,
}

impl Default for Architecture {
    fn default() -> Self {
        Self::detect()
    }
}

impl Architecture {
    /// Detect the current architecture
    pub fn detect() -> Self {
        #[cfg(target_arch = "aarch64")]
        {
            Self::Aarch64
        }

        #[cfg(target_arch = "x86_64")]
        {
            Self::X86_64
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self::Wasm32
        }

        #[cfg(not(any(
            target_arch = "aarch64",
            target_arch = "x86_64",
            target_arch = "wasm32"
        )))]
        {
            Self::Unknown
        }
    }

    /// Check if SIMD is available for this architecture
    pub fn has_simd(&self) -> bool {
        matches!(self, Self::Aarch64 | Self::X86_64)
    }
}

// =============================================================================
// CPU Features Detection
// =============================================================================

/// CPU SIMD feature flags
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CpuFeatures {
    /// ARM NEON (128-bit SIMD, always available on aarch64)
    pub neon: bool,
    /// Intel/AMD AVX2 (256-bit SIMD)
    pub avx2: bool,
    /// Intel AVX-512 (512-bit SIMD)
    pub avx512: bool,
    /// Intel SSE 4.2
    pub sse42: bool,
    /// ARM SVE (Scalable Vector Extension)
    pub sve: bool,
    /// ARM SVE2
    pub sve2: bool,
}

impl CpuFeatures {
    /// Detect CPU features at runtime
    pub fn detect() -> Self {
        let mut features = Self::default();

        // aarch64 detection
        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on aarch64
            features.neon = true;

            // SVE/SVE2 detection would require runtime checks
            // For now, assume not available unless we can detect it
            #[cfg(target_os = "linux")]
            {
                // On Linux, we could check /proc/cpuinfo or use getauxval
                // For simplicity, assume SVE is not available
                features.sve = false;
                features.sve2 = false;
            }
        }

        // x86_64 detection
        #[cfg(target_arch = "x86_64")]
        {
            #[cfg(target_feature = "sse4.2")]
            {
                features.sse42 = true;
            }

            #[cfg(target_feature = "avx2")]
            {
                features.avx2 = true;
            }

            #[cfg(target_feature = "avx512f")]
            {
                features.avx512 = true;
            }

            // Runtime detection using std::arch (if the feature was not detected at compile time)
            #[cfg(not(target_feature = "avx2"))]
            {
                features.avx2 = Self::detect_avx2_runtime();
            }

            #[cfg(not(target_feature = "sse4.2"))]
            {
                features.sse42 = Self::detect_sse42_runtime();
            }
        }

        features
    }

    /// Runtime AVX2 detection for x86_64
    #[cfg(target_arch = "x86_64")]
    fn detect_avx2_runtime() -> bool {
        #[cfg(all(target_arch = "x86_64", not(target_feature = "avx2")))]
        {
            // Use is_x86_feature_detected! macro if available
            #[cfg(feature = "std")]
            {
                std::arch::is_x86_feature_detected!("avx2")
            }
            #[cfg(not(feature = "std"))]
            {
                false
            }
        }
        #[cfg(target_feature = "avx2")]
        {
            true
        }
    }

    /// Runtime SSE 4.2 detection for x86_64
    #[cfg(target_arch = "x86_64")]
    fn detect_sse42_runtime() -> bool {
        #[cfg(all(target_arch = "x86_64", not(target_feature = "sse4.2")))]
        {
            #[cfg(feature = "std")]
            {
                std::arch::is_x86_feature_detected!("sse4.2")
            }
            #[cfg(not(feature = "std"))]
            {
                false
            }
        }
        #[cfg(target_feature = "sse4.2")]
        {
            true
        }
    }

    /// Get the best available SIMD width in bits
    pub fn best_simd_width(&self) -> usize {
        if self.avx512 {
            512
        } else if self.avx2 {
            256
        } else if self.neon || self.sse42 {
            128
        } else {
            0
        }
    }

    /// Get the number of floats that can be processed in parallel
    pub fn simd_float_lanes(&self) -> usize {
        self.best_simd_width() / 32 // f32 is 32 bits
    }
}

// =============================================================================
// GPU Capabilities
// =============================================================================

/// GPU compute backend types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GpuBackend {
    /// Apple Metal (macOS, iOS)
    Metal,
    /// NVIDIA CUDA
    Cuda,
    /// WebGPU (browser, cross-platform)
    WebGPU,
    /// Vulkan compute
    Vulkan,
    /// OpenCL
    OpenCL,
}

/// GPU capabilities and specifications
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GpuCapabilities {
    /// GPU compute backend
    pub backend: GpuBackend,
    /// Video RAM in megabytes (if detectable)
    pub vram_mb: Option<usize>,
    /// Number of compute units/streaming multiprocessors
    pub compute_units: Option<usize>,
    /// GPU name/model
    pub name: Option<String>,
    /// Whether the GPU supports FP16 compute
    pub supports_fp16: bool,
    /// Whether the GPU supports INT8 compute
    pub supports_int8: bool,
    /// Whether the GPU supports tensor cores / matrix engines
    pub has_tensor_cores: bool,
    /// Maximum shared memory per compute unit (bytes)
    pub max_shared_memory: Option<usize>,
}

impl GpuCapabilities {
    /// Detect GPU capabilities
    pub fn detect() -> Option<Self> {
        // Metal detection for macOS/iOS
        #[cfg(all(target_os = "macos", feature = "metal-compute"))]
        {
            return Self::detect_metal();
        }

        #[cfg(all(target_os = "macos", not(feature = "metal-compute")))]
        {
            // Metal is available on macOS but the feature isn't enabled
            // Return basic capabilities
            return Some(Self {
                backend: GpuBackend::Metal,
                vram_mb: None,
                compute_units: None,
                name: Some("Apple GPU (metal-compute feature not enabled)".to_string()),
                supports_fp16: true,
                supports_int8: true,
                has_tensor_cores: false,
                max_shared_memory: Some(32 * 1024), // 32KB typical
            });
        }

        #[cfg(target_os = "ios")]
        {
            return Some(Self {
                backend: GpuBackend::Metal,
                vram_mb: None,
                compute_units: None,
                name: Some("Apple GPU (iOS)".to_string()),
                supports_fp16: true,
                supports_int8: true,
                has_tensor_cores: false,
                max_shared_memory: Some(32 * 1024),
            });
        }

        // CUDA detection for Linux/Windows
        #[cfg(any(target_os = "linux", target_os = "windows"))]
        {
            if let Some(cuda) = Self::detect_cuda() {
                return Some(cuda);
            }
        }

        // WebGPU for WASM
        #[cfg(target_arch = "wasm32")]
        {
            return Self::detect_webgpu();
        }

        #[allow(unreachable_code)]
        None
    }

    /// Detect Metal GPU capabilities
    #[cfg(all(target_os = "macos", feature = "metal-compute"))]
    fn detect_metal() -> Option<Self> {
        use crate::metal::{get_device_info, is_metal_available};

        if !is_metal_available() {
            return None;
        }

        match get_device_info() {
            Some(info) => {
                // Check if this is Apple Silicon (M-series) for feature detection
                let is_apple_silicon = info.has_unified_memory;

                Some(Self {
                    backend: GpuBackend::Metal,
                    vram_mb: Some(info.recommended_max_working_set_size / (1024 * 1024)),
                    compute_units: Some(info.max_threads_per_threadgroup),
                    name: Some(info.name),
                    supports_fp16: is_apple_silicon, // Apple Silicon has excellent FP16
                    supports_int8: true,
                    has_tensor_cores: is_apple_silicon, // AMX on Apple Silicon
                    max_shared_memory: Some(32 * 1024), // 32KB typical threadgroup memory
                })
            }
            None => Some(Self {
                backend: GpuBackend::Metal,
                vram_mb: None,
                compute_units: None,
                name: Some("Apple GPU".to_string()),
                supports_fp16: true,
                supports_int8: true,
                has_tensor_cores: false,
                max_shared_memory: Some(32 * 1024),
            }),
        }
    }

    /// Detect CUDA GPU capabilities
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    fn detect_cuda() -> Option<Self> {
        // CUDA detection would require CUDA runtime
        // For now, return None and let the user configure manually
        // In a full implementation, this would use cuda_runtime_sys or similar
        None
    }

    /// Detect WebGPU capabilities
    #[cfg(target_arch = "wasm32")]
    fn detect_webgpu() -> Option<Self> {
        // WebGPU detection requires JavaScript interop
        // Return a placeholder that indicates WebGPU might be available
        Some(Self {
            backend: GpuBackend::WebGPU,
            vram_mb: None,
            compute_units: None,
            name: Some("WebGPU (browser)".to_string()),
            supports_fp16: true,
            supports_int8: false, // WebGPU INT8 support varies
            has_tensor_cores: false,
            max_shared_memory: Some(16 * 1024), // 16KB typical for WebGPU
        })
    }

    /// Estimate VRAM needed for a model of given size
    pub fn can_fit_model(&self, model_size_gb: f32) -> bool {
        if let Some(vram_mb) = self.vram_mb {
            let vram_gb = vram_mb as f32 / 1024.0;
            // Need ~1.2x model size for activations and KV cache
            vram_gb >= model_size_gb * 1.2
        } else {
            // Unknown VRAM, assume it can fit
            true
        }
    }
}

// =============================================================================
// Core Information
// =============================================================================

/// CPU core information
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoreInfo {
    /// Number of physical CPU cores
    pub physical_cores: usize,
    /// Number of logical CPU cores (with hyperthreading)
    pub logical_cores: usize,
    /// Number of performance cores (if heterogeneous, e.g., Apple M-series)
    pub performance_cores: Option<usize>,
    /// Number of efficiency cores (if heterogeneous)
    pub efficiency_cores: Option<usize>,
}

impl Default for CoreInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl CoreInfo {
    /// Detect core information
    pub fn detect() -> Self {
        let logical_cores = Self::detect_logical_cores();
        let physical_cores = Self::detect_physical_cores(logical_cores);

        // Detect heterogeneous cores on Apple Silicon
        #[cfg(target_os = "macos")]
        {
            let (perf, eff) = Self::detect_apple_cores();
            return Self {
                physical_cores,
                logical_cores,
                performance_cores: perf,
                efficiency_cores: eff,
            };
        }

        #[cfg(not(target_os = "macos"))]
        Self {
            physical_cores,
            logical_cores,
            performance_cores: None,
            efficiency_cores: None,
        }
    }

    /// Detect logical core count
    fn detect_logical_cores() -> usize {
        // Try std::thread::available_parallelism first
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    }

    /// Detect physical core count
    fn detect_physical_cores(logical: usize) -> usize {
        // On most systems, physical = logical / 2 if hyperthreading is enabled
        // This is a heuristic; accurate detection requires platform-specific APIs

        #[cfg(target_os = "macos")]
        {
            // Use sysctl on macOS
            Self::sysctl_physical_cores().unwrap_or(logical)
        }

        #[cfg(target_os = "linux")]
        {
            // Parse /proc/cpuinfo on Linux
            Self::linux_physical_cores().unwrap_or(logical / 2).max(1)
        }

        #[cfg(target_os = "windows")]
        {
            // Windows detection would use GetLogicalProcessorInformation
            // For now, use heuristic
            (logical / 2).max(1)
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            logical
        }
    }

    /// Get physical cores via sysctl on macOS
    #[cfg(target_os = "macos")]
    fn sysctl_physical_cores() -> Option<usize> {
        use std::process::Command;

        let output = Command::new("sysctl")
            .args(["-n", "hw.physicalcpu"])
            .output()
            .ok()?;

        String::from_utf8_lossy(&output.stdout).trim().parse().ok()
    }

    /// Get physical cores from /proc/cpuinfo on Linux
    #[cfg(target_os = "linux")]
    fn linux_physical_cores() -> Option<usize> {
        use std::fs;

        let cpuinfo = fs::read_to_string("/proc/cpuinfo").ok()?;

        // Count unique physical id + core id pairs
        let mut cores = std::collections::HashSet::new();

        let mut physical_id = None;
        let mut core_id = None;

        for line in cpuinfo.lines() {
            if line.starts_with("physical id") {
                physical_id = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse::<usize>().ok());
            } else if line.starts_with("core id") {
                core_id = line
                    .split(':')
                    .nth(1)
                    .and_then(|s| s.trim().parse::<usize>().ok());
            }

            if let (Some(pid), Some(cid)) = (physical_id, core_id) {
                cores.insert((pid, cid));
                physical_id = None;
                core_id = None;
            }
        }

        if cores.is_empty() {
            // Fallback: count "processor" lines
            Some(
                cpuinfo
                    .lines()
                    .filter(|l| l.starts_with("processor"))
                    .count(),
            )
        } else {
            Some(cores.len())
        }
    }

    /// Detect Apple Silicon core configuration
    #[cfg(target_os = "macos")]
    fn detect_apple_cores() -> (Option<usize>, Option<usize>) {
        use std::process::Command;

        // Try to get performance core count
        let perf = Command::new("sysctl")
            .args(["-n", "hw.perflevel0.physicalcpu"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok());

        // Try to get efficiency core count
        let eff = Command::new("sysctl")
            .args(["-n", "hw.perflevel1.physicalcpu"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8_lossy(&o.stdout).trim().parse().ok());

        (perf, eff)
    }

    /// Get the recommended thread count for parallel workloads
    pub fn recommended_threads(&self) -> usize {
        // Prefer performance cores if available
        if let Some(perf) = self.performance_cores {
            perf
        } else {
            // Use physical cores to avoid cache contention from hyperthreading
            self.physical_cores
        }
    }
}

// =============================================================================
// System Capabilities (Main Detection Struct)
// =============================================================================

/// Apple Neural Engine (ANE) capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AneInfo {
    /// Whether ANE is available on this device
    pub available: bool,
    /// ANE compute power in TOPS (Trillion Operations Per Second)
    pub tops: f32,
    /// Maximum recommended model size in MB for ANE
    pub max_model_size_mb: usize,
    /// Supported operation types
    pub supported_ops: Vec<String>,
}

impl Default for AneInfo {
    fn default() -> Self {
        Self::detect()
    }
}

impl AneInfo {
    /// Detect ANE capabilities
    pub fn detect() -> Self {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // Apple Silicon has ANE
            // M4 Pro: 38 TOPS, M3: 18 TOPS, M2: 15.8 TOPS, M1: 11 TOPS
            Self {
                available: true,
                tops: Self::detect_ane_tops(),
                max_model_size_mb: 2048, // ~2GB models work well on ANE
                supported_ops: vec![
                    "MatMul".to_string(),
                    "Conv2D".to_string(),
                    "GELU".to_string(),
                    "SiLU".to_string(),
                    "LayerNorm".to_string(),
                    "Softmax".to_string(),
                    "Add".to_string(),
                    "Mul".to_string(),
                ],
            }
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            Self {
                available: false,
                tops: 0.0,
                max_model_size_mb: 0,
                supported_ops: vec![],
            }
        }
    }

    /// Detect ANE TOPS based on chip model
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn detect_ane_tops() -> f32 {
        use std::process::Command;

        // Try to get chip model from sysctl
        if let Ok(output) = Command::new("sysctl")
            .args(["-n", "machdep.cpu.brand_string"])
            .output()
        {
            let brand = String::from_utf8_lossy(&output.stdout).to_lowercase();

            // M4 series
            if brand.contains("m4") {
                if brand.contains("max") {
                    return 38.0; // M4 Max
                } else if brand.contains("pro") {
                    return 38.0; // M4 Pro
                } else {
                    return 38.0; // M4 base
                }
            }

            // M3 series
            if brand.contains("m3") {
                if brand.contains("max") {
                    return 18.0;
                } else if brand.contains("pro") {
                    return 18.0;
                } else {
                    return 18.0;
                }
            }

            // M2 series
            if brand.contains("m2") {
                if brand.contains("ultra") {
                    return 31.6; // 2x M2 Max
                } else if brand.contains("max") {
                    return 15.8;
                } else if brand.contains("pro") {
                    return 15.8;
                } else {
                    return 15.8;
                }
            }

            // M1 series
            if brand.contains("m1") {
                if brand.contains("ultra") {
                    return 22.0; // 2x M1 Max
                } else if brand.contains("max") {
                    return 11.0;
                } else if brand.contains("pro") {
                    return 11.0;
                } else {
                    return 11.0;
                }
            }
        }

        // Default to M1 level if detection fails
        11.0
    }

    /// Check if a model of given size is suitable for ANE
    pub fn is_model_suitable(&self, model_size_mb: usize) -> bool {
        self.available && model_size_mb <= self.max_model_size_mb
    }

    /// Get recommended compute strategy for a given model size
    pub fn recommended_strategy(&self, model_size_mb: usize) -> AneStrategy {
        if !self.available {
            return AneStrategy::GpuOnly;
        }

        if model_size_mb <= 500 {
            // Small models: ANE is great
            AneStrategy::AneOnly
        } else if model_size_mb <= self.max_model_size_mb {
            // Medium models: hybrid is best
            AneStrategy::Hybrid
        } else {
            // Large models: GPU is better
            AneStrategy::GpuOnly
        }
    }
}

/// ANE usage strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AneStrategy {
    /// Use only ANE (best for small models)
    AneOnly,
    /// Use GPU + ANE hybrid (ANE for MLP, GPU for attention)
    Hybrid,
    /// Use only GPU (best for large models)
    GpuOnly,
}

/// Complete system capabilities for inference configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemCapabilities {
    /// Operating system platform
    pub platform: Platform,
    /// CPU architecture
    pub arch: Architecture,
    /// CPU SIMD features
    pub cpu_features: CpuFeatures,
    /// GPU capabilities (if available)
    pub gpu: Option<GpuCapabilities>,
    /// Apple Neural Engine capabilities (if available)
    pub ane: AneInfo,
    /// Total system memory in megabytes
    pub memory_mb: usize,
    /// Available memory in megabytes (if detectable)
    pub available_memory_mb: Option<usize>,
    /// CPU core information
    pub cores: CoreInfo,
}

impl Default for SystemCapabilities {
    fn default() -> Self {
        Self::detect()
    }
}

impl SystemCapabilities {
    /// Detect all system capabilities
    pub fn detect() -> Self {
        Self {
            platform: Platform::detect(),
            arch: Architecture::detect(),
            cpu_features: CpuFeatures::detect(),
            gpu: GpuCapabilities::detect(),
            ane: AneInfo::detect(),
            memory_mb: Self::detect_total_memory(),
            available_memory_mb: Self::detect_available_memory(),
            cores: CoreInfo::detect(),
        }
    }

    /// Detect total system memory in MB
    fn detect_total_memory() -> usize {
        #[cfg(target_os = "macos")]
        {
            Self::macos_total_memory().unwrap_or(8 * 1024) // Default 8GB
        }

        #[cfg(target_os = "linux")]
        {
            Self::linux_total_memory().unwrap_or(8 * 1024)
        }

        #[cfg(target_os = "windows")]
        {
            Self::windows_total_memory().unwrap_or(8 * 1024)
        }

        #[cfg(target_arch = "wasm32")]
        {
            // WASM: estimate based on navigator.deviceMemory (typically 4-8GB)
            4 * 1024
        }

        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_arch = "wasm32"
        )))]
        {
            4 * 1024 // Conservative default
        }
    }

    /// Detect available memory (not just total)
    fn detect_available_memory() -> Option<usize> {
        #[cfg(target_os = "macos")]
        {
            // macOS doesn't easily expose available memory
            // Would need vm_statistics or memory_pressure
            None
        }

        #[cfg(target_os = "linux")]
        {
            Self::linux_available_memory()
        }

        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        {
            None
        }
    }

    #[cfg(target_os = "macos")]
    fn macos_total_memory() -> Option<usize> {
        use std::process::Command;

        let output = Command::new("sysctl")
            .args(["-n", "hw.memsize"])
            .output()
            .ok()?;

        let bytes: u64 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .ok()?;

        Some((bytes / (1024 * 1024)) as usize)
    }

    #[cfg(target_os = "linux")]
    fn linux_total_memory() -> Option<usize> {
        use std::fs;

        let meminfo = fs::read_to_string("/proc/meminfo").ok()?;

        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: usize = parts[1].parse().ok()?;
                    return Some(kb / 1024); // Convert KB to MB
                }
            }
        }

        None
    }

    #[cfg(target_os = "linux")]
    fn linux_available_memory() -> Option<usize> {
        use std::fs;

        let meminfo = fs::read_to_string("/proc/meminfo").ok()?;

        for line in meminfo.lines() {
            if line.starts_with("MemAvailable:") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 {
                    let kb: usize = parts[1].parse().ok()?;
                    return Some(kb / 1024);
                }
            }
        }

        None
    }

    #[cfg(target_os = "windows")]
    fn windows_total_memory() -> Option<usize> {
        // Would use GetPhysicallyInstalledSystemMemory or GlobalMemoryStatusEx
        // For now, return None to use default
        None
    }

    /// Get the optimal inference configuration based on detected capabilities
    pub fn optimal_config(&self) -> InferenceConfig {
        let compute_backend = self.select_compute_backend();
        let quantization = self.optimal_quantization(7.0); // Default to 7B model size
        let batch_size = self.recommended_batch_size(2048); // Default 2K context
        let thread_count = self.cores.recommended_threads();
        let block_size = self.optimal_block_size();

        InferenceConfig {
            compute_backend,
            quantization,
            batch_size,
            thread_count,
            block_size,
            use_flash_attention: true,
            device_type: self.optimal_device_type(),
            dtype: self.optimal_dtype(),
        }
    }

    /// Get optimal attention configuration
    pub fn optimal_attention_config(&self) -> AttentionConfig {
        // Default Mistral-7B style configuration
        let mut config = AttentionConfig {
            num_heads: 32,
            num_kv_heads: 8, // GQA 4:1
            head_dim: 128,
            max_seq_len: self.optimal_max_seq_len(),
            causal: true,
            scale: 0.0, // Auto-compute
        };

        // Adjust for memory constraints
        let available_mb = self.available_memory_mb.unwrap_or(self.memory_mb / 2);
        if available_mb < 4096 {
            // Low memory: reduce max sequence length
            config.max_seq_len = 2048;
        } else if available_mb < 8192 {
            config.max_seq_len = 4096;
        } else {
            config.max_seq_len = 8192;
        }

        config
    }

    /// Select optimal quantization based on model size and available memory
    pub fn optimal_quantization(&self, model_size_gb: f32) -> Quantization {
        let available_mb = self.available_memory_mb.unwrap_or(self.memory_mb / 2);
        let available_gb = available_mb as f32 / 1024.0;

        // Check GPU VRAM if available
        if let Some(ref gpu) = self.gpu {
            if let Some(vram_mb) = gpu.vram_mb {
                let vram_gb = vram_mb as f32 / 1024.0;

                // Need ~1.5x model size for activations and KV cache
                if vram_gb >= model_size_gb * 1.5 {
                    // Full precision fits
                    return Quantization::F16;
                } else if vram_gb >= model_size_gb * 0.75 {
                    // INT8 fits
                    return Quantization::Q8;
                } else if vram_gb >= model_size_gb * 0.4 {
                    // Q4K fits (best quality 4-bit)
                    return Quantization::Q4K;
                }
            }
        }

        // Fall back to CPU memory estimation
        if available_gb >= model_size_gb * 4.0 {
            Quantization::F16
        } else if available_gb >= model_size_gb * 1.5 {
            Quantization::Q8
        } else if available_gb >= model_size_gb * 0.6 {
            Quantization::Q4K
        } else {
            // Very low memory: use aggressive quantization
            Quantization::Q4
        }
    }

    /// Calculate recommended batch size based on memory and sequence length
    pub fn recommended_batch_size(&self, seq_len: usize) -> usize {
        let available_mb = self.available_memory_mb.unwrap_or(self.memory_mb / 2);

        // Estimate memory per batch item (very rough):
        // KV cache: 2 * num_layers * num_kv_heads * head_dim * seq_len * 2 bytes (FP16)
        // For Mistral-7B style: 2 * 32 * 8 * 128 * seq_len * 2 = ~128KB per 1K tokens per batch
        let kv_per_token_kb = 128.0 / 1024.0; // KB per token
        let kv_per_batch_mb = (kv_per_token_kb * seq_len as f32) / 1024.0;

        // Reserve 50% of available memory for model weights
        let available_for_batch_mb = available_mb as f32 * 0.5;

        let max_batch = (available_for_batch_mb / kv_per_batch_mb).floor() as usize;

        // Clamp to reasonable range
        max_batch.clamp(1, 64)
    }

    /// Select the best compute backend
    fn select_compute_backend(&self) -> ComputeBackend {
        self.select_compute_backend_for_model(7.0 * 1024.0) // Default to 7B model (~7GB)
    }

    /// Select the best compute backend for a specific model size (in MB)
    pub fn select_compute_backend_for_model(&self, model_size_mb: f32) -> ComputeBackend {
        // Check if ANE is available and suitable for this model
        #[cfg(feature = "coreml")]
        {
            if self.ane.available {
                let strategy = self.ane.recommended_strategy(model_size_mb as usize);
                match strategy {
                    AneStrategy::AneOnly => {
                        // Small model: pure ANE is best
                        return ComputeBackend::CoreML;
                    }
                    AneStrategy::Hybrid => {
                        // Medium model: hybrid ANE+GPU if Metal is available
                        if let Some(ref gpu) = self.gpu {
                            if matches!(gpu.backend, GpuBackend::Metal) {
                                return ComputeBackend::HybridAne;
                            }
                        }
                        // Fall back to CoreML if no GPU
                        return ComputeBackend::CoreML;
                    }
                    AneStrategy::GpuOnly => {
                        // Large model: use GPU (fall through)
                    }
                }
            }
        }

        // Prefer GPU if available
        if let Some(ref gpu) = self.gpu {
            match gpu.backend {
                GpuBackend::Metal => return ComputeBackend::Metal,
                GpuBackend::Cuda => return ComputeBackend::Cuda,
                GpuBackend::WebGPU => return ComputeBackend::WebGPU,
                _ => {}
            }
        }

        // Fall back to CPU with SIMD
        if self.cpu_features.avx512 {
            ComputeBackend::CpuAvx512
        } else if self.cpu_features.avx2 {
            ComputeBackend::CpuAvx2
        } else if self.cpu_features.neon {
            ComputeBackend::CpuNeon
        } else {
            ComputeBackend::CpuScalar
        }
    }

    /// Select compute backend optimized for power efficiency (battery life)
    pub fn select_power_efficient_backend(&self) -> ComputeBackend {
        // ANE is 3-4x more power efficient than GPU
        #[cfg(feature = "coreml")]
        {
            if self.ane.available {
                return ComputeBackend::CoreML;
            }
        }

        // Fall back to standard selection
        self.select_compute_backend()
    }

    /// Get optimal device type for the backend crate
    fn optimal_device_type(&self) -> DeviceType {
        if let Some(ref gpu) = self.gpu {
            match gpu.backend {
                GpuBackend::Metal => DeviceType::Metal,
                GpuBackend::Cuda => DeviceType::Cuda(0),
                _ => DeviceType::Cpu,
            }
        } else {
            DeviceType::Cpu
        }
    }

    /// Get optimal dtype for the backend
    fn optimal_dtype(&self) -> DType {
        // Prefer FP16 if GPU supports it, otherwise F32
        if let Some(ref gpu) = self.gpu {
            if gpu.supports_fp16 {
                return DType::F16;
            }
        }

        // CPU: use F32 for best compatibility
        // (NEON and AVX2 have good F32 support)
        DType::F32
    }

    /// Get optimal block size for attention
    fn optimal_block_size(&self) -> usize {
        // Based on cache hierarchy
        if let Some(ref gpu) = self.gpu {
            if let Some(shared_mem) = gpu.max_shared_memory {
                // Target 50% shared memory utilization
                // block_size * head_dim * 4 bytes * 2 (K+V) = shared_mem / 2
                let head_dim = 128; // Typical
                let max_block = shared_mem / (head_dim * 4 * 2 * 2);
                return max_block.clamp(32, 128);
            }
        }

        // CPU: optimize for L1 cache (32KB typical, 192KB on M4 Pro)
        #[cfg(target_os = "macos")]
        {
            64 // M4 Pro has 192KB L1, can fit 64-token blocks
        }

        #[cfg(not(target_os = "macos"))]
        {
            32 // Conservative for 32KB L1
        }
    }

    /// Get optimal max sequence length
    fn optimal_max_seq_len(&self) -> usize {
        let available_mb = self.available_memory_mb.unwrap_or(self.memory_mb / 2);

        if available_mb >= 32 * 1024 {
            // 32GB+: can handle very long contexts
            32768
        } else if available_mb >= 16 * 1024 {
            16384
        } else if available_mb >= 8 * 1024 {
            8192
        } else if available_mb >= 4 * 1024 {
            4096
        } else {
            2048
        }
    }

    /// Check if the system can run a model of given size
    pub fn can_run_model(&self, model_size_gb: f32) -> bool {
        let available_mb = self.available_memory_mb.unwrap_or(self.memory_mb / 2);
        let available_gb = available_mb as f32 / 1024.0;

        // With Q4K quantization, need ~0.4x model size in memory
        // Plus overhead for activations and KV cache
        let min_required_gb = model_size_gb * 0.4 + 2.0; // 2GB overhead

        available_gb >= min_required_gb
    }

    /// Get a human-readable summary of capabilities
    pub fn summary(&self) -> String {
        let mut parts = vec![];

        parts.push(format!("{:?} ({:?})", self.platform, self.arch));
        parts.push(format!(
            "{} cores ({} physical)",
            self.cores.logical_cores, self.cores.physical_cores
        ));

        if let Some(perf) = self.cores.performance_cores {
            parts.push(format!(
                "{}P+{}E cores",
                perf,
                self.cores.efficiency_cores.unwrap_or(0)
            ));
        }

        parts.push(format!("{}GB RAM", self.memory_mb / 1024));

        if let Some(ref gpu) = self.gpu {
            let gpu_info = match gpu.vram_mb {
                Some(vram) => format!("{:?} ({}GB VRAM)", gpu.backend, vram / 1024),
                None => format!("{:?}", gpu.backend),
            };
            parts.push(gpu_info);
        } else {
            parts.push("No GPU".to_string());
        }

        // Add ANE info if available
        if self.ane.available {
            parts.push(format!("ANE ({:.0} TOPS)", self.ane.tops));
        }

        let simd = if self.cpu_features.avx512 {
            "AVX-512"
        } else if self.cpu_features.avx2 {
            "AVX2"
        } else if self.cpu_features.neon {
            "NEON"
        } else if self.cpu_features.sse42 {
            "SSE4.2"
        } else {
            "Scalar"
        };
        parts.push(simd.to_string());

        parts.join(", ")
    }

    /// Get ANE-specific summary
    pub fn ane_summary(&self) -> String {
        if !self.ane.available {
            return "ANE: Not available".to_string();
        }

        format!(
            "ANE: {:.0} TOPS, max model {}MB, {} supported ops",
            self.ane.tops,
            self.ane.max_model_size_mb,
            self.ane.supported_ops.len()
        )
    }
}

// =============================================================================
// Compute Backend Selection
// =============================================================================

/// Compute backend for inference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ComputeBackend {
    /// Apple Metal GPU
    Metal,
    /// Apple Neural Engine via Core ML (38 TOPS on M4 Pro)
    /// Optimal for small models (<1B params) and batch inference
    CoreML,
    /// Hybrid Metal GPU + ANE (best of both worlds)
    /// Uses ANE for MLP/FFN layers, GPU for attention
    HybridAne,
    /// NVIDIA CUDA GPU
    Cuda,
    /// WebGPU (browser/cross-platform)
    WebGPU,
    /// CPU with AVX-512 SIMD
    CpuAvx512,
    /// CPU with AVX2 SIMD
    CpuAvx2,
    /// CPU with ARM NEON SIMD
    CpuNeon,
    /// CPU scalar (no SIMD)
    CpuScalar,
}

impl ComputeBackend {
    /// Check if this is a GPU/accelerator backend
    pub fn is_gpu(&self) -> bool {
        matches!(
            self,
            Self::Metal | Self::CoreML | Self::HybridAne | Self::Cuda | Self::WebGPU
        )
    }

    /// Check if this backend uses the Neural Engine
    pub fn uses_ane(&self) -> bool {
        matches!(self, Self::CoreML | Self::HybridAne)
    }

    /// Get expected relative performance (higher = better)
    /// Note: ANE performance depends heavily on model size and batch configuration
    pub fn relative_performance(&self) -> f32 {
        match self {
            Self::HybridAne => 12.0, // Best for models that benefit from ANE+GPU
            Self::Metal => 10.0,     // Apple Silicon GPU is very efficient
            Self::CoreML => 8.0,     // ANE alone (great for small models, limited for large)
            Self::Cuda => 15.0,      // NVIDIA is fastest for large models
            Self::WebGPU => 5.0,     // WebGPU has overhead
            Self::CpuAvx512 => 4.0,  // AVX-512 is fast
            Self::CpuAvx2 => 2.5,    // AVX2 is good
            Self::CpuNeon => 2.0,    // NEON is comparable to AVX2
            Self::CpuScalar => 1.0,  // Baseline
        }
    }

    /// Get power efficiency rating (higher = more efficient)
    /// ANE is significantly more power efficient than GPU
    pub fn power_efficiency(&self) -> f32 {
        match self {
            Self::CoreML => 4.0,    // ANE is 3-4x more power efficient than GPU
            Self::HybridAne => 3.0, // Hybrid gets some efficiency benefits
            Self::Metal => 2.0,     // Apple Silicon GPU is efficient
            Self::Cuda => 1.0,      // NVIDIA uses more power
            Self::WebGPU => 1.5,    // Varies
            Self::CpuAvx512 => 1.2,
            Self::CpuAvx2 => 1.3,
            Self::CpuNeon => 1.5, // ARM is power efficient
            Self::CpuScalar => 1.0,
        }
    }
}

// =============================================================================
// Inference Configuration
// =============================================================================

/// Configuration generated by auto-detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceConfig {
    /// Selected compute backend
    pub compute_backend: ComputeBackend,
    /// Recommended quantization
    pub quantization: Quantization,
    /// Recommended batch size
    pub batch_size: usize,
    /// Recommended thread count for CPU inference
    pub thread_count: usize,
    /// Optimal block size for attention
    pub block_size: usize,
    /// Whether to use flash attention
    pub use_flash_attention: bool,
    /// Device type for the backend crate
    pub device_type: DeviceType,
    /// Data type for tensors
    pub dtype: DType,
}

impl Default for InferenceConfig {
    fn default() -> Self {
        Self::auto()
    }
}

impl InferenceConfig {
    /// Create an auto-configured inference config
    pub fn auto() -> Self {
        SystemCapabilities::detect().optimal_config()
    }

    /// Create a config optimized for low memory usage
    pub fn low_memory() -> Self {
        let mut config = Self::auto();
        config.quantization = Quantization::Q4K;
        config.batch_size = 1;
        config.block_size = 32;
        config
    }

    /// Create a config optimized for high throughput
    pub fn high_throughput() -> Self {
        let caps = SystemCapabilities::detect();
        let mut config = caps.optimal_config();

        // Increase batch size for throughput
        config.batch_size = (config.batch_size * 2).min(32);

        // Use larger blocks
        config.block_size = 128;

        config
    }

    /// Create a config optimized for low latency
    pub fn low_latency() -> Self {
        let mut config = Self::auto();

        // Use single batch for lowest latency
        config.batch_size = 1;

        // Smaller blocks reduce per-block overhead
        config.block_size = 32;

        // Use all threads for parallel decode
        let caps = SystemCapabilities::detect();
        config.thread_count = caps.cores.logical_cores;

        config
    }

    /// Get estimated tokens per second for this configuration
    pub fn estimated_tokens_per_second(&self) -> f32 {
        let base = match self.compute_backend {
            ComputeBackend::HybridAne => 90.0, // Hybrid can exceed pure Metal for suitable models
            ComputeBackend::Metal => 80.0,
            ComputeBackend::CoreML => 60.0, // ANE alone (great for small models)
            ComputeBackend::Cuda => 100.0,
            ComputeBackend::WebGPU => 40.0,
            ComputeBackend::CpuAvx512 => 30.0,
            ComputeBackend::CpuAvx2 => 20.0,
            ComputeBackend::CpuNeon => 20.0,
            ComputeBackend::CpuScalar => 5.0,
        };

        // Adjust for quantization
        let quant_factor = match self.quantization {
            Quantization::Q4 | Quantization::Q4K => 2.0, // 4-bit is fastest
            Quantization::Q8 => 1.5,
            Quantization::F16 | Quantization::Bf16 => 1.0,
            Quantization::None => 0.5,
            Quantization::Q2K => 2.5, // Most aggressive quantization
        };

        // Adjust for batch size (throughput scales sublinearly)
        let batch_factor = (self.batch_size as f32).sqrt();

        base * quant_factor * batch_factor
    }

    /// Create a config optimized for power efficiency (uses ANE when available)
    pub fn power_efficient() -> Self {
        let caps = SystemCapabilities::detect();
        let mut config = caps.optimal_config();

        // Override with power-efficient backend selection
        config.compute_backend = caps.select_power_efficient_backend();

        // Use smaller batches for better power efficiency
        config.batch_size = 1;

        config
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_detection() {
        let platform = Platform::detect();

        #[cfg(target_os = "macos")]
        assert_eq!(platform, Platform::MacOS);

        #[cfg(target_os = "linux")]
        assert_eq!(platform, Platform::Linux);

        #[cfg(target_os = "windows")]
        assert_eq!(platform, Platform::Windows);
    }

    #[test]
    fn test_architecture_detection() {
        let arch = Architecture::detect();

        #[cfg(target_arch = "aarch64")]
        assert_eq!(arch, Architecture::Aarch64);

        #[cfg(target_arch = "x86_64")]
        assert_eq!(arch, Architecture::X86_64);
    }

    #[test]
    fn test_cpu_features_detection() {
        let features = CpuFeatures::detect();

        #[cfg(target_arch = "aarch64")]
        assert!(features.neon, "NEON should always be available on aarch64");

        // SIMD width should be non-zero on supported architectures
        #[cfg(any(target_arch = "aarch64", target_arch = "x86_64"))]
        assert!(
            features.best_simd_width() >= 128,
            "Should have at least 128-bit SIMD"
        );
    }

    #[test]
    fn test_system_capabilities_detect() {
        let caps = SystemCapabilities::detect();

        // Should always have at least 1 core
        assert!(caps.cores.physical_cores >= 1);
        assert!(caps.cores.logical_cores >= 1);

        // Should have some memory detected
        assert!(caps.memory_mb > 0, "Memory should be detected");

        // Platform and arch should match
        #[cfg(target_os = "macos")]
        assert_eq!(caps.platform, Platform::MacOS);

        #[cfg(target_arch = "aarch64")]
        assert_eq!(caps.arch, Architecture::Aarch64);
    }

    #[test]
    fn test_optimal_config() {
        let caps = SystemCapabilities::detect();
        let config = caps.optimal_config();

        // Config should have reasonable values
        assert!(config.batch_size >= 1);
        assert!(config.thread_count >= 1);
        assert!(config.block_size >= 16);

        // Backend should match platform capabilities
        #[cfg(all(target_os = "macos", feature = "metal-compute"))]
        {
            if caps.gpu.is_some() {
                assert_eq!(config.compute_backend, ComputeBackend::Metal);
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            if !config.compute_backend.is_gpu() {
                assert_eq!(config.compute_backend, ComputeBackend::CpuNeon);
            }
        }
    }

    #[test]
    fn test_inference_config_auto() {
        let config = InferenceConfig::auto();

        assert!(config.batch_size >= 1);
        assert!(config.thread_count >= 1);
        assert!(config.use_flash_attention);
    }

    #[test]
    fn test_inference_config_presets() {
        let low_mem = InferenceConfig::low_memory();
        let high_throughput = InferenceConfig::high_throughput();
        let low_latency = InferenceConfig::low_latency();

        // Low memory should use aggressive quantization
        assert!(matches!(
            low_mem.quantization,
            Quantization::Q4 | Quantization::Q4K | Quantization::Q2K
        ));
        assert_eq!(low_mem.batch_size, 1);

        // Low latency should use batch size 1
        assert_eq!(low_latency.batch_size, 1);

        // High throughput should have larger batch
        assert!(high_throughput.batch_size >= 2);
    }

    #[test]
    fn test_optimal_quantization() {
        let caps = SystemCapabilities::detect();

        // Small model should use higher precision
        let quant_small = caps.optimal_quantization(1.0);

        // Large model should use more aggressive quantization
        let quant_large = caps.optimal_quantization(70.0);

        // Large model quantization should save more memory
        assert!(
            quant_large.bytes_per_weight() <= quant_small.bytes_per_weight(),
            "Larger models should use more aggressive quantization"
        );
    }

    #[test]
    fn test_recommended_batch_size() {
        let caps = SystemCapabilities::detect();

        // Shorter sequences should allow larger batches
        let batch_short = caps.recommended_batch_size(512);
        let batch_long = caps.recommended_batch_size(8192);

        assert!(
            batch_short >= batch_long,
            "Shorter sequences should allow larger batches"
        );
    }

    #[test]
    fn test_can_run_model() {
        let caps = SystemCapabilities::detect();

        // Should be able to run a tiny model
        assert!(caps.can_run_model(0.1), "Should be able to run 100MB model");

        // Likely can't run a 1TB model
        assert!(
            !caps.can_run_model(1000.0),
            "Should not be able to run 1TB model"
        );
    }

    #[test]
    fn test_system_summary() {
        let caps = SystemCapabilities::detect();
        let summary = caps.summary();

        // Summary should contain platform info
        assert!(!summary.is_empty());
        assert!(summary.contains("cores") || summary.contains("RAM"));
    }

    #[test]
    fn test_compute_backend_properties() {
        assert!(ComputeBackend::Metal.is_gpu());
        assert!(ComputeBackend::Cuda.is_gpu());
        assert!(!ComputeBackend::CpuNeon.is_gpu());
        assert!(!ComputeBackend::CpuScalar.is_gpu());

        // GPU should have higher relative performance
        assert!(
            ComputeBackend::Metal.relative_performance()
                > ComputeBackend::CpuNeon.relative_performance()
        );
    }

    #[test]
    fn test_gpu_can_fit_model() {
        let gpu = GpuCapabilities {
            backend: GpuBackend::Metal,
            vram_mb: Some(16 * 1024), // 16GB
            compute_units: Some(128),
            name: Some("Test GPU".to_string()),
            supports_fp16: true,
            supports_int8: true,
            has_tensor_cores: true,
            max_shared_memory: Some(32 * 1024),
        };

        // 16GB should fit 7B model (needs ~10GB with overhead)
        assert!(gpu.can_fit_model(7.0));

        // 16GB should not fit 70B model (needs ~100GB)
        assert!(!gpu.can_fit_model(70.0));
    }

    #[test]
    fn test_core_info() {
        let cores = CoreInfo::detect();

        // Should have at least 1 core
        assert!(cores.physical_cores >= 1);
        assert!(cores.logical_cores >= 1);

        // Logical should be >= physical
        assert!(cores.logical_cores >= cores.physical_cores);

        // Recommended threads should be reasonable
        let recommended = cores.recommended_threads();
        assert!(recommended >= 1);
        assert!(recommended <= cores.logical_cores);
    }

    #[test]
    fn test_estimated_tokens_per_second() {
        let config = InferenceConfig::auto();
        let tps = config.estimated_tokens_per_second();

        // Should be positive
        assert!(tps > 0.0);

        // Low latency config should have lower throughput but same latency
        let low_latency = InferenceConfig::low_latency();
        let tps_low_latency = low_latency.estimated_tokens_per_second();
        assert!(tps_low_latency > 0.0);
    }

    // =========================================================================
    // ANE (Apple Neural Engine) Tests
    // =========================================================================

    #[test]
    fn test_ane_info_detect() {
        let ane = AneInfo::detect();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            assert!(ane.available, "ANE should be available on Apple Silicon");
            assert!(ane.tops > 0.0, "ANE TOPS should be positive");
            assert!(
                ane.max_model_size_mb > 0,
                "ANE max model size should be positive"
            );
            assert!(
                !ane.supported_ops.is_empty(),
                "ANE should have supported ops"
            );
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            assert!(
                !ane.available,
                "ANE should not be available on non-Apple Silicon"
            );
        }
    }

    #[test]
    fn test_ane_model_suitability() {
        let ane = AneInfo {
            available: true,
            tops: 38.0,
            max_model_size_mb: 2048,
            supported_ops: vec!["MatMul".to_string()],
        };

        // Small model should be suitable
        assert!(ane.is_model_suitable(500));
        assert!(ane.is_model_suitable(2048));

        // Large model should not be suitable
        assert!(!ane.is_model_suitable(4096));
        assert!(!ane.is_model_suitable(8192));
    }

    #[test]
    fn test_ane_strategy_recommendation() {
        let ane = AneInfo {
            available: true,
            tops: 38.0,
            max_model_size_mb: 2048,
            supported_ops: vec!["MatMul".to_string()],
        };

        // Small model: ANE only
        assert_eq!(ane.recommended_strategy(300), AneStrategy::AneOnly);

        // Medium model: Hybrid
        assert_eq!(ane.recommended_strategy(1000), AneStrategy::Hybrid);

        // Large model: GPU only
        assert_eq!(ane.recommended_strategy(4000), AneStrategy::GpuOnly);
    }

    #[test]
    fn test_ane_strategy_unavailable() {
        let ane = AneInfo {
            available: false,
            tops: 0.0,
            max_model_size_mb: 0,
            supported_ops: vec![],
        };

        // All sizes should recommend GPU when ANE unavailable
        assert_eq!(ane.recommended_strategy(100), AneStrategy::GpuOnly);
        assert_eq!(ane.recommended_strategy(1000), AneStrategy::GpuOnly);
        assert_eq!(ane.recommended_strategy(10000), AneStrategy::GpuOnly);
    }

    #[test]
    fn test_compute_backend_ane_properties() {
        // CoreML and HybridAne should use ANE
        assert!(ComputeBackend::CoreML.uses_ane());
        assert!(ComputeBackend::HybridAne.uses_ane());

        // Other backends should not use ANE
        assert!(!ComputeBackend::Metal.uses_ane());
        assert!(!ComputeBackend::Cuda.uses_ane());
        assert!(!ComputeBackend::CpuNeon.uses_ane());

        // ANE backends should be considered GPU/accelerator
        assert!(ComputeBackend::CoreML.is_gpu());
        assert!(ComputeBackend::HybridAne.is_gpu());
    }

    #[test]
    fn test_compute_backend_power_efficiency() {
        // ANE should have highest power efficiency
        assert!(
            ComputeBackend::CoreML.power_efficiency() > ComputeBackend::Metal.power_efficiency(),
            "CoreML should be more power efficient than Metal"
        );
        assert!(
            ComputeBackend::HybridAne.power_efficiency() > ComputeBackend::Metal.power_efficiency(),
            "HybridAne should be more power efficient than Metal"
        );
    }

    #[test]
    fn test_system_capabilities_includes_ane() {
        let caps = SystemCapabilities::detect();

        // ANE info should be populated
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            assert!(caps.ane.available);
            // Summary should mention ANE
            let summary = caps.summary();
            assert!(summary.contains("ANE"), "Summary should include ANE info");
        }
    }

    #[test]
    fn test_ane_summary() {
        let caps = SystemCapabilities::detect();
        let ane_summary = caps.ane_summary();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            assert!(ane_summary.contains("TOPS"));
            assert!(ane_summary.contains("supported ops"));
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            assert!(ane_summary.contains("Not available"));
        }
    }

    #[test]
    fn test_power_efficient_config() {
        let config = InferenceConfig::power_efficient();

        // Power efficient config should use batch size 1
        assert_eq!(config.batch_size, 1);

        // On Apple Silicon with coreml feature, should prefer ANE
        #[cfg(all(target_os = "macos", target_arch = "aarch64", feature = "coreml"))]
        {
            assert!(
                config.compute_backend.uses_ane(),
                "Power efficient config should use ANE on Apple Silicon"
            );
        }
    }

    #[test]
    fn test_select_compute_backend_for_model_size() {
        let caps = SystemCapabilities::detect();

        // Different model sizes should potentially get different backends
        let _small_backend = caps.select_compute_backend_for_model(500.0);
        let _medium_backend = caps.select_compute_backend_for_model(2000.0);
        let _large_backend = caps.select_compute_backend_for_model(10000.0);

        // All backends should be valid
        // (Actual values depend on platform and feature flags)
    }
}
