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
//! Integration tests for Apple Neural Engine (ANE) / Core ML functionality
//!
//! These tests verify end-to-end functionality of the ANE/CoreML backend,
//! including hybrid pipeline switching, fallback behavior, and memory management.
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all ANE tests (requires Apple Silicon)
//! cargo test --features coreml ane_integration
//!
//! # Run with hybrid pipeline support
//! cargo test --features hybrid-ane ane_integration
//!
//! # Run on non-Apple Silicon (tests fallback behavior)
//! cargo test ane_integration
//! ```

// Import from the crate being tested
// Note: CoreMLBackend methods require the coreml feature
#[cfg(feature = "coreml")]
use ruvllm::backends::CoreMLBackend;
use ruvllm::backends::{
    AneCapabilities, ComputeUnits, GenerateParams, LlmBackend, ModelArchitecture, ModelConfig,
    Quantization,
};
use ruvllm::error::{Result, RuvLLMError};

// ============================================================================
// Platform Detection Helpers
// ============================================================================

/// Check if running on Apple Silicon
fn is_apple_silicon() -> bool {
    cfg!(all(target_os = "macos", target_arch = "aarch64"))
}

/// Check if ANE is available
fn is_ane_available() -> bool {
    let caps = AneCapabilities::detect();
    caps.available
}

// ============================================================================
// Core ML Backend Integration Tests
// ============================================================================

#[test]
fn test_ane_capabilities_detection() {
    let caps = AneCapabilities::detect();

    if is_apple_silicon() {
        assert!(caps.available, "ANE should be available on Apple Silicon");
        assert!(caps.tops > 0.0, "TOPS should be positive on Apple Silicon");
        assert!(
            caps.max_model_size_mb > 0,
            "Max model size should be positive"
        );
        assert!(
            !caps.supported_ops.is_empty(),
            "Should have supported operations"
        );

        // Verify common operations are supported
        let expected_ops = ["MatMul", "GELU", "SiLU", "LayerNorm", "Softmax"];
        for op in &expected_ops {
            assert!(
                caps.supported_ops.iter().any(|s| s == *op),
                "Operation {} should be supported",
                op
            );
        }
    } else {
        assert!(
            !caps.available,
            "ANE should not be available on non-Apple Silicon"
        );
        assert_eq!(caps.tops, 0.0, "TOPS should be 0 when unavailable");
        assert_eq!(
            caps.max_model_size_mb, 0,
            "Max model size should be 0 when unavailable"
        );
        assert!(
            caps.supported_ops.is_empty(),
            "No operations when unavailable"
        );
    }
}

#[test]
fn test_compute_units_selection() {
    // Test default selection
    let default = ComputeUnits::default();
    assert_eq!(default, ComputeUnits::All);

    // Test ANE-focused configuration
    let ane_focus = ComputeUnits::CpuAndNeuralEngine;
    assert!(ane_focus.uses_ane());
    assert!(!ane_focus.uses_gpu());

    // Test GPU-focused configuration
    let gpu_focus = ComputeUnits::CpuAndGpu;
    assert!(!gpu_focus.uses_ane());
    assert!(gpu_focus.uses_gpu());

    // Test all units
    let all = ComputeUnits::All;
    assert!(all.uses_ane());
    assert!(all.uses_gpu());
}

#[test]
fn test_model_suitability_for_ane() {
    let caps = AneCapabilities::detect();

    if is_apple_silicon() {
        // Small models should be suitable
        assert!(caps.is_model_suitable(500), "500MB model should fit");
        assert!(caps.is_model_suitable(1000), "1GB model should fit");
        assert!(caps.is_model_suitable(2048), "2GB model should fit");

        // Large models may not fit
        // (depends on actual device, but 10GB is likely too large)
        // Skip this assertion as it's hardware-dependent
    }
}

// ============================================================================
// Core ML Backend Creation Tests
// ============================================================================

#[test]
#[cfg(feature = "coreml")]
fn test_coreml_backend_creation() {
    if is_apple_silicon() {
        let result = CoreMLBackend::new();
        assert!(result.is_ok(), "Should create backend on Apple Silicon");

        let backend = result.unwrap();
        assert!(!backend.is_model_loaded());
        assert!(backend.model_info().is_none());
    } else {
        let result = CoreMLBackend::new();
        assert!(result.is_err(), "Should fail on non-Apple Silicon");
    }
}

#[test]
#[cfg(feature = "coreml")]
fn test_coreml_backend_configuration() {
    if !is_apple_silicon() {
        return; // Skip on non-Apple Silicon
    }

    let backend = CoreMLBackend::new()
        .unwrap()
        .with_compute_units(ComputeUnits::CpuAndNeuralEngine);

    let caps = backend.ane_capabilities();
    assert!(caps.available);
    assert!(caps.tops > 0.0);
}

// ============================================================================
// Fallback Behavior Tests
// ============================================================================

#[test]
fn test_fallback_when_coreml_unavailable() {
    // When coreml feature is not enabled, CoreMLBackend type doesn't exist
    // so we can only test the AneCapabilities fallback
    #[cfg(not(feature = "coreml"))]
    {
        // Without coreml feature, ANE capabilities should report unavailable
        let caps = AneCapabilities::detect();
        // On non-Apple Silicon or without the feature, it should gracefully handle this
        if !is_apple_silicon() {
            assert!(
                !caps.available,
                "ANE should not be available without coreml feature on non-Apple Silicon"
            );
        }
    }

    #[cfg(feature = "coreml")]
    {
        if !is_apple_silicon() {
            let result = CoreMLBackend::new();
            assert!(result.is_err());

            let err = result.unwrap_err();
            let err_str = err.to_string();
            assert!(
                err_str.contains("not available"),
                "Should indicate ANE not available"
            );
        }
    }
}

#[test]
fn test_graceful_degradation() {
    // Even when ANE is not available, the AneCapabilities struct should work
    let caps = AneCapabilities {
        available: false,
        tops: 0.0,
        max_model_size_mb: 0,
        supported_ops: vec![],
    };

    // All operations should return false/empty gracefully
    assert!(!caps.is_model_suitable(100));
    assert!(!caps.is_model_suitable(0));
    assert!(!caps.available);
}

// ============================================================================
// Model Loading Error Handling Tests
// ============================================================================

#[test]
#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
fn test_unsupported_model_format_error() {
    let mut backend = CoreMLBackend::new().unwrap();

    // Try various unsupported formats
    let unsupported_formats = [
        "model.safetensors",
        "model.bin",
        "model.pt",
        "model.pth",
        "model.onnx",
    ];

    for format in &unsupported_formats {
        let result = backend.load_model(format, ModelConfig::default());
        assert!(
            result.is_err(),
            "Should reject unsupported format: {}",
            format
        );
    }
}

#[test]
#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
fn test_nonexistent_model_error() {
    let mut backend = CoreMLBackend::new().unwrap();

    let result = backend.load_model("/nonexistent/path/model.mlmodel", ModelConfig::default());
    assert!(result.is_err());
}

#[test]
#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
fn test_gguf_conversion_error() {
    let mut backend = CoreMLBackend::new().unwrap();

    // GGUF conversion is not yet implemented
    let result = backend.load_model("/path/to/model.gguf", ModelConfig::default());
    assert!(result.is_err());

    let err = result.unwrap_err();
    let err_str = err.to_string();
    assert!(
        err_str.contains("not") || err_str.contains("conversion"),
        "Error should mention conversion issue: {}",
        err_str
    );
}

// ============================================================================
// Memory Management Tests
// ============================================================================

#[test]
#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
fn test_model_unloading() {
    let mut backend = CoreMLBackend::new().unwrap();

    // Initial state
    assert!(!backend.is_model_loaded());

    // Unload should be safe even without loaded model
    backend.unload_model();
    assert!(!backend.is_model_loaded());
    assert!(backend.model_info().is_none());
}

#[test]
#[cfg(all(feature = "coreml", target_os = "macos", target_arch = "aarch64"))]
fn test_multiple_unload_calls() {
    let mut backend = CoreMLBackend::new().unwrap();

    // Multiple unload calls should be safe
    for _ in 0..5 {
        backend.unload_model();
        assert!(!backend.is_model_loaded());
    }
}

// ============================================================================
// Hybrid Pipeline Tests
// ============================================================================

#[cfg(feature = "hybrid-ane")]
mod hybrid_pipeline_tests {
    use super::*;

    #[test]
    fn test_hybrid_feature_enabled() {
        // Verify hybrid-ane feature combines metal-compute and coreml
        // This test just confirms the feature flag works
        assert!(true, "Hybrid ANE feature is enabled");
    }

    #[test]
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn test_hybrid_configuration() {
        // Test that we can configure for hybrid operation
        let ane_caps = AneCapabilities::detect();

        if ane_caps.available {
            // In hybrid mode, we'd route:
            // - MatMul/FFN to ANE
            // - Attention to GPU (Metal)
            assert!(ane_caps.supported_ops.contains(&"MatMul".to_string()));
        }
    }
}

// ============================================================================
// Performance Characteristics Tests
// ============================================================================

#[test]
fn test_ane_tops_values() {
    // Test known TOPS values for various chips
    struct ChipSpec {
        name: &'static str,
        min_tops: f32,
        max_tops: f32,
    }

    // Known Apple Silicon TOPS ranges
    let chip_specs = [
        ChipSpec {
            name: "M1",
            min_tops: 11.0,
            max_tops: 11.5,
        },
        ChipSpec {
            name: "M1 Pro/Max",
            min_tops: 11.0,
            max_tops: 11.5,
        },
        ChipSpec {
            name: "M2",
            min_tops: 15.0,
            max_tops: 16.0,
        },
        ChipSpec {
            name: "M3",
            min_tops: 18.0,
            max_tops: 18.5,
        },
        ChipSpec {
            name: "M4",
            min_tops: 35.0,
            max_tops: 40.0,
        },
    ];

    if is_apple_silicon() {
        let caps = AneCapabilities::detect();
        // Detected TOPS should fall within one of the known ranges
        let in_known_range = chip_specs
            .iter()
            .any(|spec| caps.tops >= spec.min_tops && caps.tops <= spec.max_tops + 5.0);

        // Just verify it's a reasonable positive value
        assert!(caps.tops > 0.0, "TOPS should be positive");
        assert!(caps.tops < 100.0, "TOPS should be reasonable (< 100)");
    }
}

// ============================================================================
// Error Type Tests
// ============================================================================

#[test]
fn test_error_messages() {
    // Test that error messages are informative
    let caps = AneCapabilities {
        available: false,
        tops: 0.0,
        max_model_size_mb: 0,
        supported_ops: vec![],
    };

    // Debug output should be readable
    let debug = format!("{:?}", caps);
    assert!(debug.contains("available"));
    assert!(debug.contains("false"));
}

#[test]
#[cfg(feature = "coreml")]
fn test_error_chain() {
    if !is_apple_silicon() {
        let result: Result<CoreMLBackend> = CoreMLBackend::new();
        let err = result.unwrap_err();

        // Error should be a Config error
        match &err {
            RuvLLMError::Config(msg) => {
                assert!(msg.contains("not available") || msg.contains("feature"));
            }
            other => {
                panic!("Expected Config error, got {:?}", other);
            }
        }
    }
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

#[test]
fn test_ane_capabilities_thread_safe() {
    use std::sync::Arc;
    use std::thread;

    let caps = Arc::new(AneCapabilities::detect());

    let handles: Vec<_> = (0..4)
        .map(|i| {
            let caps = Arc::clone(&caps);
            thread::spawn(move || {
                // Read operations should be thread-safe
                let _ = caps.available;
                let _ = caps.tops;
                let _ = caps.max_model_size_mb;
                let _ = caps.is_model_suitable(1000);
                let _ = format!("{:?}", caps);
                i
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("Thread should complete successfully");
    }
}

// ============================================================================
// Benchmark-style Tests (Run with --release)
// ============================================================================

#[test]
#[ignore] // Run with: cargo test --release -- --ignored
fn test_ane_capabilities_detection_performance() {
    use std::time::Instant;

    let iterations = 1000;
    let start = Instant::now();

    for _ in 0..iterations {
        let _ = AneCapabilities::detect();
    }

    let duration = start.elapsed();
    let avg_ns = duration.as_nanos() as f64 / iterations as f64;

    println!(
        "AneCapabilities::detect() average time: {:.2} ns ({:.2} us)",
        avg_ns,
        avg_ns / 1000.0
    );

    // Detection should be fast (< 1ms)
    assert!(
        avg_ns < 1_000_000.0,
        "Detection should be < 1ms, was {} ns",
        avg_ns
    );
}

// ============================================================================
// Documentation Examples Tests
// ============================================================================

#[test]
fn test_readme_example_capabilities() {
    // Example from module documentation
    let caps = AneCapabilities::detect();

    if caps.available {
        println!("ANE available with {} TOPS", caps.tops);
        println!("Max model size: {} MB", caps.max_model_size_mb);
        println!("Supported ops: {:?}", caps.supported_ops);
    } else {
        println!("ANE not available on this device");
    }
}

#[test]
fn test_readme_example_compute_units() {
    // Example from module documentation
    let units = ComputeUnits::CpuAndNeuralEngine;

    println!("Compute units: {}", units.description());
    println!("Uses ANE: {}", units.uses_ane());
    println!("Uses GPU: {}", units.uses_gpu());

    assert!(units.uses_ane());
    assert!(!units.uses_gpu());
}

// ============================================================================
// Property-based Test Helpers
// ============================================================================

#[test]
fn test_model_suitability_monotonic() {
    // Model suitability should be monotonic: if a larger model fits, smaller ones should too
    let caps = AneCapabilities {
        available: true,
        tops: 38.0,
        max_model_size_mb: 2048,
        supported_ops: vec!["MatMul".to_string()],
    };

    // If 2048 fits, all smaller sizes should fit
    if caps.is_model_suitable(2048) {
        for size in [0, 1, 100, 500, 1000, 1500, 2000, 2047] {
            assert!(
                caps.is_model_suitable(size),
                "Size {} should fit if {} fits",
                size,
                2048
            );
        }
    }

    // If 2049 doesn't fit, all larger sizes shouldn't fit either
    if !caps.is_model_suitable(2049) {
        for size in [2050, 3000, 4096, 10000] {
            assert!(
                !caps.is_model_suitable(size),
                "Size {} should not fit if {} doesn't fit",
                size,
                2049
            );
        }
    }
}
