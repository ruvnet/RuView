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
//! Auto-Detection Integration Tests
//!
//! Tests the system capabilities detection, optimal configuration generation,
//! and intelligent hardware-aware settings for LLM inference using the
//! actual autodetect module.

use ruvllm::autodetect::{
    Architecture, ComputeBackend, CoreInfo, CpuFeatures, GpuBackend, GpuCapabilities,
    InferenceConfig, Platform, SystemCapabilities,
};
use ruvllm::backends::Quantization;
use std::collections::HashSet;

// ============================================================================
// System Detection Tests
// ============================================================================

#[test]
fn test_system_capabilities_detection() {
    let caps = SystemCapabilities::detect();

    // Platform detection
    #[cfg(target_os = "macos")]
    assert_eq!(caps.platform, Platform::MacOS);

    #[cfg(target_os = "linux")]
    assert_eq!(caps.platform, Platform::Linux);

    #[cfg(target_os = "windows")]
    assert_eq!(caps.platform, Platform::Windows);

    // Architecture detection
    #[cfg(target_arch = "aarch64")]
    assert_eq!(caps.arch, Architecture::Aarch64);

    #[cfg(target_arch = "x86_64")]
    assert_eq!(caps.arch, Architecture::X86_64);

    #[cfg(target_arch = "wasm32")]
    assert_eq!(caps.arch, Architecture::Wasm32);

    // CPU features should have baseline set
    #[cfg(target_arch = "aarch64")]
    assert!(
        caps.cpu_features.neon,
        "NEON should be available on aarch64"
    );

    // Memory should be positive
    assert!(caps.memory_mb > 0, "Memory should be detected");

    // Cores should be positive
    assert!(
        caps.cores.physical_cores > 0,
        "Physical cores should be detected"
    );
    assert!(
        caps.cores.logical_cores > 0,
        "Logical cores should be detected"
    );
    assert!(
        caps.cores.logical_cores >= caps.cores.physical_cores,
        "Logical cores should be >= physical cores"
    );
}

#[test]
fn test_optimal_config_generation() {
    let caps = SystemCapabilities::detect();
    let config = caps.optimal_config();

    // Verify reasonable defaults
    assert!(config.batch_size >= 1, "Batch size should be at least 1");
    assert!(
        config.thread_count >= 1,
        "Thread count should be at least 1"
    );
    assert!(config.block_size >= 16, "Block size should be at least 16");

    // Thread count should not exceed logical cores
    assert!(
        config.thread_count <= caps.cores.logical_cores,
        "Thread count {} should not exceed logical cores {}",
        config.thread_count,
        caps.cores.logical_cores
    );
}

#[test]
fn test_quantization_recommendation_small_model() {
    let caps = SystemCapabilities::detect();

    // Small model (3GB) - should use FP16 or Q8 on most systems
    let q_small = caps.optimal_quantization(3.0);

    if caps.memory_mb >= 16384 {
        // With 16GB+ RAM, FP16 or Q8 should be recommended
        assert!(
            matches!(q_small, Quantization::F16 | Quantization::Q8),
            "Small model with 16GB+ RAM should use F16 or Q8, got {:?}",
            q_small
        );
    }
}

#[test]
fn test_quantization_recommendation_large_model() {
    let caps = SystemCapabilities::detect();

    // Large model (70GB) - should use Q4K or Q4
    let q_large = caps.optimal_quantization(70.0);

    // Unless you have 256GB+ RAM, this should be Q4K or Q4
    if caps.memory_mb < 256 * 1024 {
        assert!(
            matches!(
                q_large,
                Quantization::Q4K | Quantization::Q4 | Quantization::Q2K
            ),
            "Large model should use aggressive quantization, got {:?}",
            q_large
        );
    }
}

#[test]
fn test_auto_config_matches_manual() {
    let auto = InferenceConfig::auto();
    let caps = SystemCapabilities::detect();
    let manual = caps.optimal_config();

    // Auto should produce same result as manual
    assert_eq!(
        auto.batch_size, manual.batch_size,
        "Auto batch size should match manual"
    );
    assert_eq!(
        auto.thread_count, manual.thread_count,
        "Auto thread count should match manual"
    );
    assert_eq!(
        auto.block_size, manual.block_size,
        "Auto block size should match manual"
    );
    assert_eq!(
        auto.compute_backend, manual.compute_backend,
        "Auto compute backend should match manual"
    );
}

#[test]
fn test_platform_specific_gpu_detection() {
    let caps = SystemCapabilities::detect();

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        // Apple Silicon should detect Metal
        assert!(caps.gpu.is_some(), "Apple Silicon should have GPU");
        let gpu = caps.gpu.as_ref().unwrap();
        assert_eq!(gpu.backend, GpuBackend::Metal);
    }

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    {
        // Intel Mac should detect Metal
        assert!(caps.gpu.is_some(), "Intel Mac should have GPU");
        let gpu = caps.gpu.as_ref().unwrap();
        assert_eq!(gpu.backend, GpuBackend::Metal);
    }
}

#[test]
fn test_cpu_feature_detection_aarch64() {
    #[cfg(target_arch = "aarch64")]
    {
        let features = CpuFeatures::detect();

        // NEON is mandatory on aarch64
        assert!(features.neon, "NEON must be available on aarch64");
    }
}

#[test]
fn test_cpu_feature_detection_x86_64() {
    #[cfg(target_arch = "x86_64")]
    {
        let features = CpuFeatures::detect();

        // SSE4.2 should be common on modern x86_64
        // Note: This depends on compile-time detection or runtime check
        println!(
            "SSE4.2: {}, AVX2: {}, AVX-512: {}",
            features.sse42, features.avx2, features.avx512
        );
    }
}

#[test]
fn test_memory_detection() {
    let caps = SystemCapabilities::detect();

    // Memory should be in reasonable range (256MB to 1TB)
    assert!(caps.memory_mb >= 256, "Memory should be at least 256MB");
    assert!(
        caps.memory_mb <= 1024 * 1024,
        "Memory should be at most 1TB"
    );

    println!(
        "Detected memory: {} MB ({:.1} GB)",
        caps.memory_mb,
        caps.memory_mb as f64 / 1024.0
    );
}

#[test]
fn test_core_count_detection() {
    let cores = CoreInfo::detect();

    // Physical cores should be reasonable
    assert!(
        cores.physical_cores >= 1,
        "Should have at least 1 physical core"
    );
    assert!(
        cores.physical_cores <= 256,
        "Should have at most 256 physical cores"
    );

    // Logical cores should be >= physical
    assert!(
        cores.logical_cores >= cores.physical_cores,
        "Logical cores {} should >= physical cores {}",
        cores.logical_cores,
        cores.physical_cores
    );

    println!(
        "Detected cores: {} physical, {} logical",
        cores.physical_cores, cores.logical_cores
    );

    // Check heterogeneous cores on Apple Silicon
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        if let (Some(perf), Some(eff)) = (cores.performance_cores, cores.efficiency_cores) {
            println!("  Performance cores: {}, Efficiency cores: {}", perf, eff);
        }
    }
}

#[test]
fn test_recommended_batch_size_scaling() {
    let caps = SystemCapabilities::detect();

    // Test that batch size decreases with longer sequences
    let batch_512 = caps.recommended_batch_size(512);
    let batch_4096 = caps.recommended_batch_size(4096);
    let batch_16384 = caps.recommended_batch_size(16384);

    assert!(
        batch_512 >= batch_4096,
        "Shorter sequences should allow larger batches"
    );
    assert!(
        batch_4096 >= batch_16384,
        "Medium sequences should allow larger batches than long ones"
    );
}

#[test]
fn test_inference_config_presets() {
    let auto = InferenceConfig::auto();
    let low_mem = InferenceConfig::low_memory();
    let high_throughput = InferenceConfig::high_throughput();
    let low_latency = InferenceConfig::low_latency();

    // Low memory should use aggressive quantization
    assert!(
        matches!(
            low_mem.quantization,
            Quantization::Q4 | Quantization::Q4K | Quantization::Q2K
        ),
        "Low memory config should use aggressive quantization"
    );
    assert_eq!(low_mem.batch_size, 1, "Low memory should use batch size 1");

    // Low latency should use batch size 1
    assert_eq!(
        low_latency.batch_size, 1,
        "Low latency should use batch size 1"
    );

    // All configs should have flash attention enabled
    assert!(auto.use_flash_attention);
    assert!(low_mem.use_flash_attention);
    assert!(high_throughput.use_flash_attention);
    assert!(low_latency.use_flash_attention);
}

#[test]
fn test_compute_backend_selection() {
    let caps = SystemCapabilities::detect();
    let config = caps.optimal_config();

    // On macOS with GPU, should select Metal
    #[cfg(target_os = "macos")]
    {
        if caps.gpu.is_some() {
            assert_eq!(
                config.compute_backend,
                ComputeBackend::Metal,
                "Should select Metal on macOS with GPU"
            );
        }
    }

    // On aarch64 without GPU, should select NEON
    #[cfg(target_arch = "aarch64")]
    {
        if caps.gpu.is_none() {
            assert_eq!(
                config.compute_backend,
                ComputeBackend::CpuNeon,
                "Should select NEON on aarch64 without GPU"
            );
        }
    }

    // Verify GPU backends are detected as GPU
    assert!(ComputeBackend::Metal.is_gpu());
    assert!(ComputeBackend::Cuda.is_gpu());
    assert!(ComputeBackend::WebGPU.is_gpu());
    assert!(!ComputeBackend::CpuNeon.is_gpu());
    assert!(!ComputeBackend::CpuAvx2.is_gpu());
    assert!(!ComputeBackend::CpuScalar.is_gpu());
}

#[test]
fn test_system_summary() {
    let caps = SystemCapabilities::detect();
    let summary = caps.summary();

    println!("System Summary: {}", summary);

    // Summary should contain useful information
    assert!(!summary.is_empty(), "Summary should not be empty");
    assert!(
        summary.contains("cores") || summary.contains("RAM"),
        "Summary should contain cores or RAM info"
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

    // Test boundary conditions
    // Note: can_run_model uses available_memory_mb which defaults to memory_mb / 2
    let available_gb = caps.available_memory_mb.unwrap_or(caps.memory_mb / 2) as f32 / 1024.0;
    let max_model = (available_gb - 2.0) / 0.4; // Reverse the formula from can_run_model

    if max_model > 0.0 {
        // Should be able to run a model slightly smaller than max
        assert!(
            caps.can_run_model(max_model * 0.8),
            "Should be able to run model at 80% of max"
        );
    }
}

#[test]
fn test_estimated_tokens_per_second() {
    let auto = InferenceConfig::auto();
    let tps = auto.estimated_tokens_per_second();

    assert!(tps > 0.0, "Estimated tokens per second should be positive");

    // Metal and CUDA should have higher estimates than CPU
    let metal_tps = {
        let mut config = auto.clone();
        config.compute_backend = ComputeBackend::Metal;
        config.estimated_tokens_per_second()
    };

    let cpu_tps = {
        let mut config = auto.clone();
        config.compute_backend = ComputeBackend::CpuScalar;
        config.estimated_tokens_per_second()
    };

    assert!(
        metal_tps > cpu_tps,
        "Metal should have higher estimated TPS than CPU scalar"
    );
}

// ============================================================================
// Hardware Fingerprinting Tests
// ============================================================================

#[test]
fn test_hardware_fingerprint_stability() {
    // Run detection multiple times and verify consistency
    let cap1 = SystemCapabilities::detect();
    let cap2 = SystemCapabilities::detect();

    assert_eq!(cap1.platform, cap2.platform);
    assert_eq!(cap1.arch, cap2.arch);
    assert_eq!(cap1.cores.logical_cores, cap2.cores.logical_cores);
    assert_eq!(cap1.cpu_features.neon, cap2.cpu_features.neon);

    // Memory may vary slightly due to system activity, but should be close
    let mem_diff = (cap1.memory_mb as i64 - cap2.memory_mb as i64).abs();
    assert!(mem_diff < 100, "Memory detection should be stable");
}

#[test]
fn test_all_supported_platforms() {
    // Verify all platform variants are distinct
    let platforms = vec![
        Platform::MacOS,
        Platform::Linux,
        Platform::Windows,
        Platform::Wasm,
        Platform::IOS,
        Platform::Android,
        Platform::Unknown,
    ];

    let unique: HashSet<_> = platforms.iter().collect();
    assert_eq!(unique.len(), 7, "All platform variants should be distinct");
}

#[test]
fn test_all_architecture_variants() {
    let archs = vec![
        Architecture::Aarch64,
        Architecture::X86_64,
        Architecture::Wasm32,
        Architecture::Unknown,
    ];

    let unique: HashSet<_> = archs.iter().collect();
    assert_eq!(
        unique.len(),
        4,
        "All architecture variants should be distinct"
    );
}

#[test]
fn test_all_gpu_backend_variants() {
    let backends = vec![
        GpuBackend::Metal,
        GpuBackend::Cuda,
        GpuBackend::WebGPU,
        GpuBackend::Vulkan,
        GpuBackend::OpenCL,
    ];

    let unique: HashSet<_> = backends.iter().collect();
    assert_eq!(
        unique.len(),
        5,
        "All GPU backend variants should be distinct"
    );
}

#[test]
fn test_all_compute_backend_variants() {
    let backends = vec![
        ComputeBackend::Metal,
        ComputeBackend::Cuda,
        ComputeBackend::WebGPU,
        ComputeBackend::CpuAvx512,
        ComputeBackend::CpuAvx2,
        ComputeBackend::CpuNeon,
        ComputeBackend::CpuScalar,
    ];

    let unique: HashSet<_> = backends.iter().collect();
    assert_eq!(
        unique.len(),
        7,
        "All compute backend variants should be distinct"
    );

    // Verify relative performance ordering
    assert!(
        ComputeBackend::Cuda.relative_performance() > ComputeBackend::Metal.relative_performance()
    );
    assert!(
        ComputeBackend::Metal.relative_performance()
            > ComputeBackend::CpuAvx512.relative_performance()
    );
    assert!(
        ComputeBackend::CpuAvx512.relative_performance()
            > ComputeBackend::CpuAvx2.relative_performance()
    );
    assert!(
        ComputeBackend::CpuAvx2.relative_performance()
            >= ComputeBackend::CpuNeon.relative_performance()
    );
    assert!(
        ComputeBackend::CpuNeon.relative_performance()
            > ComputeBackend::CpuScalar.relative_performance()
    );
}

#[test]
fn test_gpu_can_fit_model() {
    // Test with a synthetic GPU
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
    assert!(gpu.can_fit_model(7.0), "16GB VRAM should fit 7B model");

    // 16GB should not fit 70B model (needs ~100GB)
    assert!(
        !gpu.can_fit_model(70.0),
        "16GB VRAM should not fit 70B model"
    );

    // Edge case: unknown VRAM
    let gpu_unknown = GpuCapabilities {
        backend: GpuBackend::Metal,
        vram_mb: None,
        compute_units: None,
        name: Some("Unknown GPU".to_string()),
        supports_fp16: true,
        supports_int8: true,
        has_tensor_cores: false,
        max_shared_memory: None,
    };

    // Unknown VRAM should assume it can fit (optimistic)
    assert!(
        gpu_unknown.can_fit_model(7.0),
        "Unknown VRAM should optimistically assume model fits"
    );
}

// ============================================================================
// System Capabilities Display Test
// ============================================================================

#[test]
fn test_system_capabilities_display() {
    let caps = SystemCapabilities::detect();

    println!("\n=== System Capabilities ===");
    println!("Platform: {:?}", caps.platform);
    println!("Architecture: {:?}", caps.arch);
    println!(
        "Memory: {} MB ({:.1} GB)",
        caps.memory_mb,
        caps.memory_mb as f64 / 1024.0
    );
    println!(
        "Cores: {} physical, {} logical",
        caps.cores.physical_cores, caps.cores.logical_cores
    );

    if let Some(ref gpu) = caps.gpu {
        println!("GPU: {:?} - {:?}", gpu.backend, gpu.name);
        if let Some(vram) = gpu.vram_mb {
            println!("     VRAM: {} MB", vram);
        }
        println!(
            "     FP16: {}, INT8: {}, Tensor Cores: {}",
            gpu.supports_fp16, gpu.supports_int8, gpu.has_tensor_cores
        );
    } else {
        println!("GPU: None");
    }

    println!("\nCPU Features:");
    #[cfg(target_arch = "aarch64")]
    println!("  NEON: {}", caps.cpu_features.neon);

    #[cfg(target_arch = "x86_64")]
    {
        println!("  SSE4.2: {}", caps.cpu_features.sse42);
        println!("  AVX2: {}", caps.cpu_features.avx2);
        println!("  AVX-512: {}", caps.cpu_features.avx512);
    }

    println!(
        "  Best SIMD width: {} bits",
        caps.cpu_features.best_simd_width()
    );
    println!(
        "  SIMD float lanes: {}",
        caps.cpu_features.simd_float_lanes()
    );

    let config = caps.optimal_config();
    println!("\n=== Optimal Configuration ===");
    println!("Compute Backend: {:?}", config.compute_backend);
    println!("Quantization: {:?}", config.quantization);
    println!("Batch Size: {}", config.batch_size);
    println!("Thread Count: {}", config.thread_count);
    println!("Block Size: {}", config.block_size);
    println!("Flash Attention: {}", config.use_flash_attention);
    println!("Device Type: {:?}", config.device_type);
    println!("DType: {:?}", config.dtype);
    println!("Estimated TPS: {:.1}", config.estimated_tokens_per_second());

    println!("\n=== Summary ===");
    println!("{}", caps.summary());

    // Test passes if we get here without panicking
    assert!(true);
}

// ============================================================================
// Attention Config Integration
// ============================================================================

#[test]
fn test_optimal_attention_config() {
    let caps = SystemCapabilities::detect();
    let attn_config = caps.optimal_attention_config();

    // Verify reasonable attention configuration
    assert!(attn_config.num_heads > 0, "Should have at least 1 head");
    assert!(
        attn_config.num_kv_heads > 0,
        "Should have at least 1 KV head"
    );
    assert!(attn_config.head_dim > 0, "Should have positive head dim");
    assert!(
        attn_config.max_seq_len >= 1024,
        "Should support at least 1K context"
    );

    // GQA ratio should be valid
    let gqa_ratio = attn_config.gqa_ratio();
    assert!(gqa_ratio >= 1, "GQA ratio should be at least 1");
    assert!(
        attn_config.num_heads % attn_config.num_kv_heads == 0,
        "num_heads should be divisible by num_kv_heads"
    );

    // Scale should be reasonable
    let scale = attn_config.effective_scale();
    assert!(
        scale > 0.0 && scale < 1.0,
        "Scale should be between 0 and 1"
    );

    println!(
        "Attention Config: {} heads, {} KV heads, {} head_dim, {} max_seq_len, GQA {}:1",
        attn_config.num_heads,
        attn_config.num_kv_heads,
        attn_config.head_dim,
        attn_config.max_seq_len,
        gqa_ratio
    );
}
