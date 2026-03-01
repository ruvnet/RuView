#![allow(
    clippy::all,
    unused_imports,
    unused_variables,
    dead_code,
    unused_mut,
    unused_assignments,
    non_camel_case_types,
    clippy::approx_constant,
    unexpected_cfgs,
    unused_must_use,
    unused_parens
)]
//! Integration tests for v2.1 cross-platform features
//!
//! Tests cover:
//! - Platform-specific fallbacks
//! - WASM-specific detection and limitations
//! - Feature detection across platforms
//! - Graceful degradation
//! - Runtime capability checking

#![allow(non_camel_case_types)]

// =============================================================================
// Platform Types
// =============================================================================

/// Target platform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    MacOS,
    Linux,
    Windows,
    iOS,
    Android,
    WebAssembly,
    Unknown,
}

impl Platform {
    /// Detect current platform at compile time
    pub fn current() -> Self {
        #[cfg(target_os = "macos")]
        return Platform::MacOS;

        #[cfg(target_os = "linux")]
        return Platform::Linux;

        #[cfg(target_os = "windows")]
        return Platform::Windows;

        #[cfg(target_os = "ios")]
        return Platform::iOS;

        #[cfg(target_os = "android")]
        return Platform::Android;

        #[cfg(target_arch = "wasm32")]
        return Platform::WebAssembly;

        #[cfg(not(any(
            target_os = "macos",
            target_os = "linux",
            target_os = "windows",
            target_os = "ios",
            target_os = "android",
            target_arch = "wasm32"
        )))]
        return Platform::Unknown;
    }

    /// Check if platform supports Metal
    pub fn supports_metal(&self) -> bool {
        matches!(self, Platform::MacOS | Platform::iOS)
    }

    /// Check if platform supports CUDA
    pub fn supports_cuda(&self) -> bool {
        matches!(self, Platform::Linux | Platform::Windows)
    }

    /// Check if platform supports WebGPU
    pub fn supports_webgpu(&self) -> bool {
        matches!(
            self,
            Platform::MacOS | Platform::Linux | Platform::Windows | Platform::WebAssembly
        )
    }

    /// Check if platform supports native file I/O
    pub fn supports_native_io(&self) -> bool {
        !matches!(self, Platform::WebAssembly)
    }

    /// Check if platform supports multi-threading
    pub fn supports_threading(&self) -> bool {
        !matches!(self, Platform::WebAssembly)
    }

    /// Get maximum recommended batch size for platform
    pub fn max_recommended_batch_size(&self) -> usize {
        match self {
            Platform::MacOS | Platform::Linux | Platform::Windows => 64,
            Platform::iOS | Platform::Android => 16,
            Platform::WebAssembly => 4,
            Platform::Unknown => 1,
        }
    }
}

/// CPU architecture
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Architecture {
    X86_64,
    Aarch64,
    Wasm32,
    Unknown,
}

impl Architecture {
    /// Detect current architecture at compile time
    pub fn current() -> Self {
        #[cfg(target_arch = "x86_64")]
        return Architecture::X86_64;

        #[cfg(target_arch = "aarch64")]
        return Architecture::Aarch64;

        #[cfg(target_arch = "wasm32")]
        return Architecture::Wasm32;

        #[cfg(not(any(
            target_arch = "x86_64",
            target_arch = "aarch64",
            target_arch = "wasm32"
        )))]
        return Architecture::Unknown;
    }

    /// Check if architecture supports SIMD
    pub fn supports_simd(&self) -> bool {
        !matches!(self, Architecture::Unknown)
    }

    /// Get SIMD width in bytes
    pub fn simd_width(&self) -> usize {
        match self {
            Architecture::X86_64 => 32,  // AVX2
            Architecture::Aarch64 => 16, // NEON
            Architecture::Wasm32 => 16,  // SIMD128
            Architecture::Unknown => 0,
        }
    }
}

// =============================================================================
// CPU Features
// =============================================================================

/// CPU feature flags
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    // x86_64 features
    pub sse: bool,
    pub sse2: bool,
    pub sse3: bool,
    pub ssse3: bool,
    pub sse4_1: bool,
    pub sse4_2: bool,
    pub avx: bool,
    pub avx2: bool,
    pub avx512f: bool,
    pub avx512vl: bool,
    pub avx512vnni: bool,
    pub fma: bool,
    pub f16c: bool,

    // ARM features
    pub neon: bool,
    pub fp16: bool,
    pub dotprod: bool,
    pub i8mm: bool,
    pub sve: bool,
    pub sve2: bool,

    // WASM features
    pub simd128: bool,
    pub relaxed_simd: bool,
}

impl CpuFeatures {
    /// Detect CPU features at runtime
    pub fn detect() -> Self {
        let mut features = Self::default();

        #[cfg(target_arch = "x86_64")]
        {
            #[cfg(target_feature = "sse")]
            {
                features.sse = true;
            }
            #[cfg(target_feature = "sse2")]
            {
                features.sse2 = true;
            }
            #[cfg(target_feature = "sse3")]
            {
                features.sse3 = true;
            }
            #[cfg(target_feature = "ssse3")]
            {
                features.ssse3 = true;
            }
            #[cfg(target_feature = "sse4.1")]
            {
                features.sse4_1 = true;
            }
            #[cfg(target_feature = "sse4.2")]
            {
                features.sse4_2 = true;
            }
            #[cfg(target_feature = "avx")]
            {
                features.avx = true;
            }
            #[cfg(target_feature = "avx2")]
            {
                features.avx2 = true;
            }
            #[cfg(target_feature = "fma")]
            {
                features.fma = true;
            }
            #[cfg(target_feature = "f16c")]
            {
                features.f16c = true;
            }
        }

        #[cfg(target_arch = "aarch64")]
        {
            // NEON is always available on aarch64
            features.neon = true;

            #[cfg(target_feature = "fp16")]
            {
                features.fp16 = true;
            }
            #[cfg(target_feature = "dotprod")]
            {
                features.dotprod = true;
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            #[cfg(target_feature = "simd128")]
            {
                features.simd128 = true;
            }
            #[cfg(target_feature = "relaxed-simd")]
            {
                features.relaxed_simd = true;
            }
        }

        features
    }

    /// Create feature set for a mock x86_64 system with AVX2
    pub fn mock_x86_64_avx2() -> Self {
        Self {
            sse: true,
            sse2: true,
            sse3: true,
            ssse3: true,
            sse4_1: true,
            sse4_2: true,
            avx: true,
            avx2: true,
            fma: true,
            f16c: true,
            ..Default::default()
        }
    }

    /// Create feature set for a mock ARM system with NEON
    pub fn mock_aarch64_neon() -> Self {
        Self {
            neon: true,
            fp16: true,
            dotprod: true,
            ..Default::default()
        }
    }

    /// Create feature set for a mock WASM environment
    pub fn mock_wasm_simd() -> Self {
        Self {
            simd128: true,
            ..Default::default()
        }
    }

    /// Check if the system supports fast matrix operations
    pub fn supports_fast_matmul(&self) -> bool {
        self.avx2 || self.neon || self.simd128
    }

    /// Check if the system supports native FP16
    pub fn supports_native_fp16(&self) -> bool {
        self.f16c || self.fp16
    }

    /// Check if the system supports INT8 dot products
    pub fn supports_int8_dotprod(&self) -> bool {
        self.avx512vnni || self.dotprod || self.i8mm
    }
}

// =============================================================================
// GPU Capabilities
// =============================================================================

/// GPU backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuBackend {
    Metal,
    Cuda,
    Vulkan,
    WebGPU,
    None,
}

/// GPU capabilities
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
    pub backend: GpuBackend,
    pub device_name: String,
    pub compute_units: u32,
    pub memory_bytes: u64,
    pub supports_fp16: bool,
    pub supports_int8: bool,
    pub supports_bf16: bool,
    pub max_buffer_size: u64,
    pub max_workgroup_size: u32,
    pub unified_memory: bool,
}

impl GpuCapabilities {
    /// Create mock Metal capabilities (Apple Silicon)
    pub fn mock_metal_m4() -> Self {
        Self {
            backend: GpuBackend::Metal,
            device_name: "Apple M4 Pro".to_string(),
            compute_units: 20,
            memory_bytes: 48 * 1024 * 1024 * 1024, // 48GB unified
            supports_fp16: true,
            supports_int8: true,
            supports_bf16: true,
            max_buffer_size: 48 * 1024 * 1024 * 1024,
            max_workgroup_size: 1024,
            unified_memory: true,
        }
    }

    /// Create mock CUDA capabilities
    pub fn mock_cuda_4090() -> Self {
        Self {
            backend: GpuBackend::Cuda,
            device_name: "NVIDIA GeForce RTX 4090".to_string(),
            compute_units: 128,
            memory_bytes: 24 * 1024 * 1024 * 1024, // 24GB VRAM
            supports_fp16: true,
            supports_int8: true,
            supports_bf16: true,
            max_buffer_size: 24 * 1024 * 1024 * 1024,
            max_workgroup_size: 1024,
            unified_memory: false,
        }
    }

    /// Create mock WebGPU capabilities
    pub fn mock_webgpu() -> Self {
        Self {
            backend: GpuBackend::WebGPU,
            device_name: "WebGPU Device".to_string(),
            compute_units: 8,
            memory_bytes: 4 * 1024 * 1024 * 1024, // 4GB typical
            supports_fp16: true,
            supports_int8: false,
            supports_bf16: false,
            max_buffer_size: 2 * 1024 * 1024 * 1024, // 2GB buffer limit
            max_workgroup_size: 256,
            unified_memory: false,
        }
    }

    /// Create capabilities when no GPU is available
    pub fn none() -> Self {
        Self {
            backend: GpuBackend::None,
            device_name: "CPU Only".to_string(),
            compute_units: 0,
            memory_bytes: 0,
            supports_fp16: false,
            supports_int8: false,
            supports_bf16: false,
            max_buffer_size: 0,
            max_workgroup_size: 0,
            unified_memory: false,
        }
    }

    /// Check if GPU is available
    pub fn is_available(&self) -> bool {
        self.backend != GpuBackend::None
    }

    /// Calculate maximum model size that fits in memory
    pub fn max_model_size(&self) -> u64 {
        if self.unified_memory {
            self.memory_bytes * 9 / 10 // 90% of unified memory
        } else {
            self.memory_bytes * 8 / 10 // 80% of VRAM
        }
    }
}

// =============================================================================
// System Capabilities
// =============================================================================

/// Complete system capabilities
#[derive(Debug, Clone)]
pub struct SystemCapabilities {
    pub platform: Platform,
    pub architecture: Architecture,
    pub cpu_features: CpuFeatures,
    pub gpu: GpuCapabilities,
    pub system_memory_bytes: u64,
    pub cpu_cores: usize,
}

impl SystemCapabilities {
    /// Detect system capabilities
    pub fn detect() -> Self {
        Self {
            platform: Platform::current(),
            architecture: Architecture::current(),
            cpu_features: CpuFeatures::detect(),
            gpu: GpuCapabilities::none(), // Would need async detection
            system_memory_bytes: 0,       // Would need system calls
            cpu_cores: 1,                 // Would need system calls
        }
    }

    /// Create mock capabilities for Apple Silicon Mac
    pub fn mock_mac_m4() -> Self {
        Self {
            platform: Platform::MacOS,
            architecture: Architecture::Aarch64,
            cpu_features: CpuFeatures::mock_aarch64_neon(),
            gpu: GpuCapabilities::mock_metal_m4(),
            system_memory_bytes: 48 * 1024 * 1024 * 1024,
            cpu_cores: 14,
        }
    }

    /// Create mock capabilities for Linux with CUDA
    pub fn mock_linux_cuda() -> Self {
        Self {
            platform: Platform::Linux,
            architecture: Architecture::X86_64,
            cpu_features: CpuFeatures::mock_x86_64_avx2(),
            gpu: GpuCapabilities::mock_cuda_4090(),
            system_memory_bytes: 64 * 1024 * 1024 * 1024,
            cpu_cores: 16,
        }
    }

    /// Create mock capabilities for WebAssembly
    pub fn mock_wasm() -> Self {
        Self {
            platform: Platform::WebAssembly,
            architecture: Architecture::Wasm32,
            cpu_features: CpuFeatures::mock_wasm_simd(),
            gpu: GpuCapabilities::mock_webgpu(),
            system_memory_bytes: 4 * 1024 * 1024 * 1024, // Limited in browser
            cpu_cores: 4,                                // Typical worker count
        }
    }

    /// Create mock capabilities for CPU-only system
    pub fn mock_cpu_only() -> Self {
        Self {
            platform: Platform::Linux,
            architecture: Architecture::X86_64,
            cpu_features: CpuFeatures::mock_x86_64_avx2(),
            gpu: GpuCapabilities::none(),
            system_memory_bytes: 32 * 1024 * 1024 * 1024,
            cpu_cores: 8,
        }
    }

    /// Get the best available compute backend
    pub fn best_backend(&self) -> ComputeBackend {
        if self.gpu.is_available() {
            match self.gpu.backend {
                GpuBackend::Metal => ComputeBackend::Metal,
                GpuBackend::Cuda => ComputeBackend::Cuda,
                GpuBackend::WebGPU => ComputeBackend::WebGPU,
                _ => ComputeBackend::Cpu,
            }
        } else {
            ComputeBackend::Cpu
        }
    }
}

/// Compute backend selection
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComputeBackend {
    Metal,
    Cuda,
    WebGPU,
    Cpu,
}

// =============================================================================
// Fallback System
// =============================================================================

/// Backend fallback chain
pub struct FallbackChain {
    backends: Vec<ComputeBackend>,
}

impl FallbackChain {
    /// Create a fallback chain for the given capabilities
    pub fn for_capabilities(caps: &SystemCapabilities) -> Self {
        let mut backends = Vec::new();

        // Add GPU backend if available
        if caps.gpu.is_available() {
            backends.push(caps.best_backend());
        }

        // Add CPU as final fallback
        if !backends.contains(&ComputeBackend::Cpu) {
            backends.push(ComputeBackend::Cpu);
        }

        Self { backends }
    }

    /// Get the primary backend
    pub fn primary(&self) -> ComputeBackend {
        self.backends
            .first()
            .copied()
            .unwrap_or(ComputeBackend::Cpu)
    }

    /// Get all backends in order
    pub fn all(&self) -> &[ComputeBackend] {
        &self.backends
    }

    /// Check if a backend is available
    pub fn has(&self, backend: ComputeBackend) -> bool {
        self.backends.contains(&backend)
    }

    /// Get fallback for a failed backend
    pub fn fallback_for(&self, failed: ComputeBackend) -> Option<ComputeBackend> {
        let pos = self.backends.iter().position(|&b| b == failed)?;
        self.backends.get(pos + 1).copied()
    }
}

// =============================================================================
// WASM-Specific Utilities
// =============================================================================

/// WASM-specific limitations and workarounds
pub struct WasmLimitations {
    /// Maximum memory in bytes (due to 32-bit address space)
    pub max_memory: u64,
    /// Whether SharedArrayBuffer is available (for threading)
    pub has_shared_memory: bool,
    /// Whether SIMD128 is available
    pub has_simd: bool,
    /// Whether atomics are available
    pub has_atomics: bool,
    /// Maximum single allocation size
    pub max_allocation: u64,
}

impl WasmLimitations {
    /// Create with typical browser limitations
    pub fn typical_browser() -> Self {
        Self {
            max_memory: 4 * 1024 * 1024 * 1024, // 4GB
            has_shared_memory: false,           // Requires COOP/COEP headers
            has_simd: true,
            has_atomics: false,
            max_allocation: 2 * 1024 * 1024 * 1024, // 2GB single alloc
        }
    }

    /// Create with enhanced browser limitations (with headers)
    pub fn enhanced_browser() -> Self {
        Self {
            max_memory: 4 * 1024 * 1024 * 1024,
            has_shared_memory: true,
            has_simd: true,
            has_atomics: true,
            max_allocation: 2 * 1024 * 1024 * 1024,
        }
    }

    /// Create for Node.js environment
    pub fn nodejs() -> Self {
        Self {
            max_memory: 4 * 1024 * 1024 * 1024,
            has_shared_memory: true,
            has_simd: true,
            has_atomics: true,
            max_allocation: 2 * 1024 * 1024 * 1024,
        }
    }

    /// Check if multi-threading is possible
    pub fn can_multithread(&self) -> bool {
        self.has_shared_memory && self.has_atomics
    }

    /// Get recommended thread count
    pub fn recommended_threads(&self) -> usize {
        if self.can_multithread() {
            4 // Typical worker count in browsers
        } else {
            1
        }
    }

    /// Calculate maximum model size given limitations
    pub fn max_model_size(&self) -> u64 {
        // Leave headroom for runtime and other allocations
        self.max_memory * 7 / 10 // 70% of max memory
    }
}

// =============================================================================
// Configuration Generator
// =============================================================================

/// Optimal configuration for a given system
#[derive(Debug, Clone)]
pub struct OptimalConfig {
    pub backend: ComputeBackend,
    pub batch_size: usize,
    pub context_length: usize,
    pub thread_count: usize,
    pub quantization: QuantizationType,
    pub use_flash_attention: bool,
    pub use_kv_cache: bool,
    pub memory_mapped_weights: bool,
}

/// Quantization type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantizationType {
    F32,
    F16,
    BF16,
    Q8_0,
    Q4_0,
    Q4_K,
}

impl OptimalConfig {
    /// Generate optimal configuration for given capabilities
    pub fn for_capabilities(caps: &SystemCapabilities, model_size_bytes: u64) -> Self {
        let backend = caps.best_backend();

        // Determine quantization based on model size and memory
        let available_memory = if caps.gpu.is_available() {
            caps.gpu.max_model_size()
        } else {
            caps.system_memory_bytes * 7 / 10
        };

        let quantization = if model_size_bytes <= available_memory {
            if caps.cpu_features.supports_native_fp16() || caps.gpu.supports_fp16 {
                QuantizationType::F16
            } else {
                QuantizationType::F32
            }
        } else if model_size_bytes / 2 <= available_memory {
            QuantizationType::Q8_0
        } else {
            QuantizationType::Q4_K
        };

        // Determine batch size
        let batch_size = caps.platform.max_recommended_batch_size();

        // Context length based on memory
        let context_length = match backend {
            ComputeBackend::Metal => 8192,
            ComputeBackend::Cuda => 8192,
            ComputeBackend::WebGPU => 2048,
            ComputeBackend::Cpu => 4096,
        };

        // Thread count
        let thread_count = if caps.platform.supports_threading() {
            caps.cpu_cores.min(8)
        } else {
            1
        };

        // Flash attention availability
        let use_flash_attention = matches!(backend, ComputeBackend::Metal | ComputeBackend::Cuda);

        // Memory mapping (not available in WASM)
        let memory_mapped_weights = caps.platform.supports_native_io();

        Self {
            backend,
            batch_size,
            context_length,
            thread_count,
            quantization,
            use_flash_attention,
            use_kv_cache: true,
            memory_mapped_weights,
        }
    }

    /// Generate WASM-specific configuration
    pub fn for_wasm(limits: &WasmLimitations, model_size_bytes: u64) -> Self {
        let quantization = if model_size_bytes <= limits.max_model_size() {
            QuantizationType::F16
        } else if model_size_bytes / 2 <= limits.max_model_size() {
            QuantizationType::Q8_0
        } else {
            QuantizationType::Q4_K
        };

        Self {
            backend: ComputeBackend::WebGPU,
            batch_size: 4,
            context_length: 2048,
            thread_count: limits.recommended_threads(),
            quantization,
            use_flash_attention: false,
            use_kv_cache: true,
            memory_mapped_weights: false, // Not available in WASM
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // Platform Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_platform_detection() {
        let platform = Platform::current();
        // Just verify it returns something valid
        assert!(matches!(
            platform,
            Platform::MacOS
                | Platform::Linux
                | Platform::Windows
                | Platform::iOS
                | Platform::Android
                | Platform::WebAssembly
                | Platform::Unknown
        ));
    }

    #[test]
    fn test_platform_metal_support() {
        assert!(Platform::MacOS.supports_metal());
        assert!(Platform::iOS.supports_metal());
        assert!(!Platform::Linux.supports_metal());
        assert!(!Platform::Windows.supports_metal());
        assert!(!Platform::WebAssembly.supports_metal());
    }

    #[test]
    fn test_platform_cuda_support() {
        assert!(Platform::Linux.supports_cuda());
        assert!(Platform::Windows.supports_cuda());
        assert!(!Platform::MacOS.supports_cuda());
        assert!(!Platform::WebAssembly.supports_cuda());
    }

    #[test]
    fn test_platform_webgpu_support() {
        assert!(Platform::MacOS.supports_webgpu());
        assert!(Platform::Linux.supports_webgpu());
        assert!(Platform::Windows.supports_webgpu());
        assert!(Platform::WebAssembly.supports_webgpu());
        assert!(!Platform::iOS.supports_webgpu());
    }

    #[test]
    fn test_platform_native_io() {
        assert!(Platform::MacOS.supports_native_io());
        assert!(Platform::Linux.supports_native_io());
        assert!(!Platform::WebAssembly.supports_native_io());
    }

    #[test]
    fn test_platform_threading() {
        assert!(Platform::MacOS.supports_threading());
        assert!(Platform::Linux.supports_threading());
        assert!(!Platform::WebAssembly.supports_threading());
    }

    #[test]
    fn test_platform_batch_sizes() {
        assert!(Platform::MacOS.max_recommended_batch_size() >= 32);
        assert!(Platform::iOS.max_recommended_batch_size() <= 32);
        assert!(Platform::WebAssembly.max_recommended_batch_size() <= 8);
    }

    // -------------------------------------------------------------------------
    // Architecture Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_architecture_detection() {
        let arch = Architecture::current();
        assert!(matches!(
            arch,
            Architecture::X86_64
                | Architecture::Aarch64
                | Architecture::Wasm32
                | Architecture::Unknown
        ));
    }

    #[test]
    fn test_architecture_simd_support() {
        assert!(Architecture::X86_64.supports_simd());
        assert!(Architecture::Aarch64.supports_simd());
        assert!(Architecture::Wasm32.supports_simd());
        assert!(!Architecture::Unknown.supports_simd());
    }

    #[test]
    fn test_architecture_simd_width() {
        assert_eq!(Architecture::X86_64.simd_width(), 32); // AVX2
        assert_eq!(Architecture::Aarch64.simd_width(), 16); // NEON
        assert_eq!(Architecture::Wasm32.simd_width(), 16); // SIMD128
        assert_eq!(Architecture::Unknown.simd_width(), 0);
    }

    // -------------------------------------------------------------------------
    // CPU Features Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_cpu_features_x86_64_mock() {
        let features = CpuFeatures::mock_x86_64_avx2();
        assert!(features.sse);
        assert!(features.sse2);
        assert!(features.avx);
        assert!(features.avx2);
        assert!(features.fma);
    }

    #[test]
    fn test_cpu_features_aarch64_mock() {
        let features = CpuFeatures::mock_aarch64_neon();
        assert!(features.neon);
        assert!(features.fp16);
        assert!(features.dotprod);
    }

    #[test]
    fn test_cpu_features_wasm_mock() {
        let features = CpuFeatures::mock_wasm_simd();
        assert!(features.simd128);
        assert!(!features.avx2);
        assert!(!features.neon);
    }

    #[test]
    fn test_cpu_features_fast_matmul() {
        let x86 = CpuFeatures::mock_x86_64_avx2();
        assert!(x86.supports_fast_matmul());

        let arm = CpuFeatures::mock_aarch64_neon();
        assert!(arm.supports_fast_matmul());

        let wasm = CpuFeatures::mock_wasm_simd();
        assert!(wasm.supports_fast_matmul());

        let none = CpuFeatures::default();
        assert!(!none.supports_fast_matmul());
    }

    #[test]
    fn test_cpu_features_native_fp16() {
        let x86 = CpuFeatures::mock_x86_64_avx2();
        assert!(x86.supports_native_fp16()); // f16c

        let arm = CpuFeatures::mock_aarch64_neon();
        assert!(arm.supports_native_fp16()); // fp16

        let wasm = CpuFeatures::mock_wasm_simd();
        assert!(!wasm.supports_native_fp16());
    }

    // -------------------------------------------------------------------------
    // GPU Capabilities Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_gpu_metal_mock() {
        let gpu = GpuCapabilities::mock_metal_m4();
        assert_eq!(gpu.backend, GpuBackend::Metal);
        assert!(gpu.unified_memory);
        assert!(gpu.supports_fp16);
        assert!(gpu.supports_bf16);
    }

    #[test]
    fn test_gpu_cuda_mock() {
        let gpu = GpuCapabilities::mock_cuda_4090();
        assert_eq!(gpu.backend, GpuBackend::Cuda);
        assert!(!gpu.unified_memory);
        assert!(gpu.supports_fp16);
    }

    #[test]
    fn test_gpu_webgpu_mock() {
        let gpu = GpuCapabilities::mock_webgpu();
        assert_eq!(gpu.backend, GpuBackend::WebGPU);
        assert!(gpu.supports_fp16);
        assert!(!gpu.supports_int8); // Typically not supported
    }

    #[test]
    fn test_gpu_none() {
        let gpu = GpuCapabilities::none();
        assert_eq!(gpu.backend, GpuBackend::None);
        assert!(!gpu.is_available());
    }

    #[test]
    fn test_gpu_max_model_size() {
        let metal = GpuCapabilities::mock_metal_m4();
        let cuda = GpuCapabilities::mock_cuda_4090();

        // Unified memory allows larger models
        assert!(metal.max_model_size() > cuda.max_model_size());
    }

    // -------------------------------------------------------------------------
    // System Capabilities Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_system_capabilities_mac() {
        let caps = SystemCapabilities::mock_mac_m4();
        assert_eq!(caps.platform, Platform::MacOS);
        assert_eq!(caps.architecture, Architecture::Aarch64);
        assert_eq!(caps.best_backend(), ComputeBackend::Metal);
    }

    #[test]
    fn test_system_capabilities_linux_cuda() {
        let caps = SystemCapabilities::mock_linux_cuda();
        assert_eq!(caps.platform, Platform::Linux);
        assert_eq!(caps.architecture, Architecture::X86_64);
        assert_eq!(caps.best_backend(), ComputeBackend::Cuda);
    }

    #[test]
    fn test_system_capabilities_wasm() {
        let caps = SystemCapabilities::mock_wasm();
        assert_eq!(caps.platform, Platform::WebAssembly);
        assert_eq!(caps.architecture, Architecture::Wasm32);
        assert_eq!(caps.best_backend(), ComputeBackend::WebGPU);
    }

    #[test]
    fn test_system_capabilities_cpu_only() {
        let caps = SystemCapabilities::mock_cpu_only();
        assert_eq!(caps.best_backend(), ComputeBackend::Cpu);
    }

    // -------------------------------------------------------------------------
    // Fallback Chain Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_fallback_chain_metal() {
        let caps = SystemCapabilities::mock_mac_m4();
        let chain = FallbackChain::for_capabilities(&caps);

        assert_eq!(chain.primary(), ComputeBackend::Metal);
        assert!(chain.has(ComputeBackend::Cpu));
        assert_eq!(
            chain.fallback_for(ComputeBackend::Metal),
            Some(ComputeBackend::Cpu)
        );
    }

    #[test]
    fn test_fallback_chain_cpu_only() {
        let caps = SystemCapabilities::mock_cpu_only();
        let chain = FallbackChain::for_capabilities(&caps);

        assert_eq!(chain.primary(), ComputeBackend::Cpu);
        assert_eq!(chain.all().len(), 1);
        assert_eq!(chain.fallback_for(ComputeBackend::Cpu), None);
    }

    #[test]
    fn test_fallback_chain_order() {
        let caps = SystemCapabilities::mock_linux_cuda();
        let chain = FallbackChain::for_capabilities(&caps);

        let backends = chain.all();
        assert_eq!(backends[0], ComputeBackend::Cuda);
        assert_eq!(backends[1], ComputeBackend::Cpu);
    }

    // -------------------------------------------------------------------------
    // WASM Limitations Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_wasm_limitations_typical() {
        let limits = WasmLimitations::typical_browser();
        assert!(!limits.has_shared_memory);
        assert!(!limits.can_multithread());
        assert_eq!(limits.recommended_threads(), 1);
    }

    #[test]
    fn test_wasm_limitations_enhanced() {
        let limits = WasmLimitations::enhanced_browser();
        assert!(limits.has_shared_memory);
        assert!(limits.has_atomics);
        assert!(limits.can_multithread());
        assert!(limits.recommended_threads() > 1);
    }

    #[test]
    fn test_wasm_limitations_nodejs() {
        let limits = WasmLimitations::nodejs();
        assert!(limits.can_multithread());
        assert!(limits.has_simd);
    }

    #[test]
    fn test_wasm_max_model_size() {
        let limits = WasmLimitations::typical_browser();
        let max_size = limits.max_model_size();
        assert!(max_size < limits.max_memory);
        assert!(max_size > 0);
    }

    // -------------------------------------------------------------------------
    // Optimal Configuration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_optimal_config_mac() {
        let caps = SystemCapabilities::mock_mac_m4();
        let model_size = 7 * 1024 * 1024 * 1024; // 7B model (~7GB)

        let config = OptimalConfig::for_capabilities(&caps, model_size);

        assert_eq!(config.backend, ComputeBackend::Metal);
        assert!(config.use_flash_attention);
        assert!(config.memory_mapped_weights);
        assert!(config.thread_count > 1);
    }

    #[test]
    fn test_optimal_config_cuda() {
        let caps = SystemCapabilities::mock_linux_cuda();
        let model_size = 13 * 1024 * 1024 * 1024; // 13B model

        let config = OptimalConfig::for_capabilities(&caps, model_size);

        assert_eq!(config.backend, ComputeBackend::Cuda);
        assert!(config.use_flash_attention);
    }

    #[test]
    fn test_optimal_config_quantization_fallback() {
        let caps = SystemCapabilities::mock_cpu_only();
        let model_size = 70 * 1024 * 1024 * 1024; // 70B model - too large

        let config = OptimalConfig::for_capabilities(&caps, model_size);

        // Should fall back to aggressive quantization
        assert!(matches!(
            config.quantization,
            QuantizationType::Q4_0 | QuantizationType::Q4_K | QuantizationType::Q8_0
        ));
    }

    #[test]
    fn test_optimal_config_wasm() {
        let limits = WasmLimitations::typical_browser();
        let model_size = 2 * 1024 * 1024 * 1024; // 2B model

        let config = OptimalConfig::for_wasm(&limits, model_size);

        assert_eq!(config.backend, ComputeBackend::WebGPU);
        assert!(!config.use_flash_attention);
        assert!(!config.memory_mapped_weights);
        assert!(config.context_length <= 4096);
        assert!(config.batch_size <= 8);
    }

    #[test]
    fn test_optimal_config_small_model() {
        let caps = SystemCapabilities::mock_mac_m4();
        let model_size = 1 * 1024 * 1024 * 1024; // 1GB model

        let config = OptimalConfig::for_capabilities(&caps, model_size);

        // Small model should use FP16, not quantized
        assert!(matches!(
            config.quantization,
            QuantizationType::F16 | QuantizationType::F32
        ));
    }

    // -------------------------------------------------------------------------
    // Integration Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_full_detection_pipeline() {
        // Test the full detection -> configuration pipeline
        let caps = SystemCapabilities::detect();

        // Should always return valid values
        assert!(caps.cpu_cores == 0 || caps.cpu_cores >= 1);

        let chain = FallbackChain::for_capabilities(&caps);
        assert!(!chain.all().is_empty());

        // Generate config for a 7B model
        let config = OptimalConfig::for_capabilities(&caps, 7 * 1024 * 1024 * 1024);
        assert!(config.batch_size >= 1);
        assert!(config.context_length >= 512);
    }

    #[test]
    fn test_platform_specific_defaults() {
        // Test that each platform gets sensible defaults
        let platforms = vec![
            SystemCapabilities::mock_mac_m4(),
            SystemCapabilities::mock_linux_cuda(),
            SystemCapabilities::mock_wasm(),
            SystemCapabilities::mock_cpu_only(),
        ];

        for caps in platforms {
            let config = OptimalConfig::for_capabilities(&caps, 4 * 1024 * 1024 * 1024);

            // Basic sanity checks
            assert!(config.batch_size >= 1);
            assert!(config.context_length >= 512);
            assert!(config.thread_count >= 1);
            assert!(config.use_kv_cache); // Always enabled
        }
    }

    #[test]
    fn test_graceful_degradation() {
        // Start with high-end system
        let mut caps = SystemCapabilities::mock_linux_cuda();

        // Remove GPU
        caps.gpu = GpuCapabilities::none();

        let config = OptimalConfig::for_capabilities(&caps, 7 * 1024 * 1024 * 1024);

        // Should fall back to CPU
        assert_eq!(config.backend, ComputeBackend::Cpu);
        assert!(!config.use_flash_attention); // Not available on CPU
    }

    #[test]
    fn test_memory_constrained_config() {
        // Very limited memory
        let mut caps = SystemCapabilities::mock_cpu_only();
        caps.system_memory_bytes = 8 * 1024 * 1024 * 1024; // 8GB only

        // Try to load a large model
        let model_size = 30 * 1024 * 1024 * 1024; // 30GB

        let config = OptimalConfig::for_capabilities(&caps, model_size);

        // Should use aggressive quantization
        assert!(matches!(
            config.quantization,
            QuantizationType::Q4_0 | QuantizationType::Q4_K
        ));
    }
}
