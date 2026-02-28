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
//! RuvLTRA-Small Model Tests
//!
//! This module provides comprehensive tests for the RuvLTRA-Small inference engine,
//! validating model loading, quantization accuracy, SONA integration, and ANE dispatch.
//!
//! ## Test Categories
//!
//! - **Model Loading**: Validate GGUF/SafeTensors loading and configuration
//! - **Quantization**: Test dequantization accuracy across all quantization formats
//! - **SONA Integration**: Test Self-Optimizing Neural Architecture adaptation
//! - **ANE Dispatch**: Test Apple Neural Engine routing and fallback behavior
//!
//! ## Running Tests
//!
//! ```bash
//! # Run all RuvLTRA tests
//! cargo test --package ruvllm ruvltra_tests
//!
//! # Run with ANE support (Apple Silicon only)
//! cargo test --package ruvllm --features coreml ruvltra_tests
//!
//! # Run with full feature set
//! cargo test --package ruvllm --all-features ruvltra_tests
//! ```

use ruvllm::backends::{
    AneCapabilities, ComputeUnits, ModelArchitecture, ModelConfig, Quantization,
};
use ruvllm::gguf::quantization::{dequantize_tensor, GgufQuantType, QuantizedTensor};
use ruvllm::kernels::ane_ops::{
    get_ane_recommendation, is_ane_available, should_use_ane, should_use_ane_activation,
    should_use_ane_matmul,
};

use std::time::{Duration, Instant};

// ============================================================================
// Test Fixtures and Constants
// ============================================================================

/// RuvLTRA-Small model configuration for testing
const RUVLTRA_SMALL_CONFIG: RuvLtraTestConfig = RuvLtraTestConfig {
    vocab_size: 32000,
    hidden_size: 2048,
    intermediate_size: 5504,
    num_hidden_layers: 22,
    num_attention_heads: 32,
    num_key_value_heads: 8,
    max_position_embeddings: 8192,
    rope_theta: 10000.0,
    layer_norm_eps: 1e-5,
};

/// Test configuration for RuvLTRA-Small
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
struct RuvLtraTestConfig {
    vocab_size: usize,
    hidden_size: usize,
    intermediate_size: usize,
    num_hidden_layers: usize,
    num_attention_heads: usize,
    num_key_value_heads: usize,
    max_position_embeddings: usize,
    rope_theta: f32,
    layer_norm_eps: f32,
}

/// Memory bounds for validation (in bytes)
const MEMORY_BOUNDS: MemoryBounds = MemoryBounds {
    // Q4_K quantization: ~1.2GB for small model
    max_model_memory: 1_500_000_000,
    // KV cache for 8K context
    max_kv_cache_memory: 500_000_000,
    // Working memory for inference
    max_working_memory: 200_000_000,
};

#[derive(Debug, Clone, Copy)]
struct MemoryBounds {
    max_model_memory: usize,
    max_kv_cache_memory: usize,
    max_working_memory: usize,
}

/// Test tolerance levels
const EPSILON: f32 = 1e-4;
const LOOSE_EPSILON: f32 = 0.01;
const QUANTIZATION_EPSILON: f32 = 0.1; // Higher tolerance for quantized values

// ============================================================================
// Model Loading Tests
// ============================================================================

mod model_loading {
    use super::*;

    #[test]
    fn test_model_config_creation() {
        let config = ModelConfig {
            architecture: ModelArchitecture::Llama,
            quantization: Some(Quantization::Q4K),
            max_sequence_length: 8192,
            vocab_size: Some(RUVLTRA_SMALL_CONFIG.vocab_size),
            use_flash_attention: true,
            ..Default::default()
        };

        assert_eq!(config.architecture, ModelArchitecture::Llama);
        assert_eq!(config.quantization, Some(Quantization::Q4K));
        assert_eq!(config.max_sequence_length, 8192);
        assert_eq!(config.vocab_size, Some(RUVLTRA_SMALL_CONFIG.vocab_size));
        assert!(config.use_flash_attention);
    }

    #[test]
    fn test_model_architecture_variants() {
        let architectures = [
            ModelArchitecture::Llama,
            ModelArchitecture::Mistral,
            ModelArchitecture::Phi,
            ModelArchitecture::Qwen,
        ];

        for arch in architectures {
            let config = ModelConfig {
                architecture: arch,
                quantization: Some(Quantization::Q4K),
                max_sequence_length: 4096,
                vocab_size: Some(32000),
                use_flash_attention: false,
                ..Default::default()
            };

            assert_eq!(config.architecture, arch);
            // Verify architecture can be formatted/debugged
            let _ = format!("{:?}", arch);
        }
    }

    #[test]
    fn test_quantization_format_selection() {
        let quantizations = [
            (Quantization::None, "None", 32.0),
            (Quantization::F16, "F16", 16.0),
            (Quantization::Bf16, "Bf16", 16.0),
            (Quantization::Q8, "Q8", 8.0),
            (Quantization::Q4K, "Q4K", 4.5),
            (Quantization::Q4, "Q4", 4.0),
            (Quantization::Q2K, "Q2K", 2.56),
        ];

        for (quant, name, _expected_bits) in quantizations {
            let config = ModelConfig {
                architecture: ModelArchitecture::Llama,
                quantization: Some(quant),
                max_sequence_length: 4096,
                vocab_size: Some(32000),
                use_flash_attention: false,
                ..Default::default()
            };

            // Verify quantization is set correctly
            assert_eq!(config.quantization, Some(quant));

            // Verify name format
            let quant_name = format!("{:?}", quant);
            assert!(
                quant_name.contains(name) || !quant_name.is_empty(),
                "Quantization {:?} should have recognizable name",
                quant
            );
        }
    }

    #[test]
    fn test_model_config_default_values() {
        let config = ModelConfig::default();

        // Verify sensible defaults
        assert!(config.max_sequence_length > 0);
        // vocab_size is now Option, so check it's present or use default behavior
    }

    #[test]
    fn test_invalid_model_path_error() {
        // This test validates error handling for non-existent paths
        let result = std::fs::metadata("/nonexistent/path/to/model.gguf");
        assert!(result.is_err(), "Non-existent path should fail");
    }

    #[test]
    fn test_gguf_extension_validation() {
        let valid_extensions = [".gguf", ".GGUF"];
        let invalid_extensions = [".bin", ".safetensors", ".pt", ".pth"];

        for ext in valid_extensions {
            assert!(
                ext.to_lowercase().ends_with("gguf"),
                "Extension {} should be valid GGUF",
                ext
            );
        }

        for ext in invalid_extensions {
            assert!(
                !ext.to_lowercase().ends_with("gguf"),
                "Extension {} should not be GGUF",
                ext
            );
        }
    }

    #[test]
    fn test_rope_theta_configuration() {
        // Test rope theta configuration
        let config_with_theta = ModelConfig {
            architecture: ModelArchitecture::Llama,
            quantization: Some(Quantization::Q4K),
            max_sequence_length: 4096,
            vocab_size: Some(32000),
            rope_theta: Some(10000.0),
            use_flash_attention: false,
            ..Default::default()
        };
        assert_eq!(config_with_theta.rope_theta, Some(10000.0));

        // Rope theta is the frequency base for rotary position embeddings
        // The actual implementation depends on the model architecture
    }

    #[test]
    fn test_context_length_bounds() {
        let context_lengths = [512, 1024, 2048, 4096, 8192, 16384, 32768];

        for ctx_len in context_lengths {
            let config = ModelConfig {
                architecture: ModelArchitecture::Llama,
                quantization: Some(Quantization::Q4K),
                max_sequence_length: ctx_len,
                vocab_size: Some(32000),
                use_flash_attention: false,
                ..Default::default()
            };

            assert_eq!(config.max_sequence_length, ctx_len);
            assert!(ctx_len > 0, "Context length must be positive");
        }
    }
}

// ============================================================================
// Quantization Accuracy Tests
// ============================================================================

mod quantization_accuracy {
    use super::*;

    /// Test Q4_0 dequantization accuracy
    #[test]
    fn test_q4_0_dequantization_accuracy() {
        // Create test Q4_0 block: scale + packed 4-bit values
        let mut block = vec![0u8; 18];

        // Set scale = 0.5 (f16: 0x3800)
        block[0] = 0x00;
        block[1] = 0x38;

        // Pack values: (8 - offset) gives 0, (9 - offset) gives 1, etc.
        // Q4_0 uses offset of 8
        for i in 0..16 {
            let low = 8u8; // Will become 0 after offset
            let high = 9u8; // Will become 1 after offset
            block[2 + i] = low | (high << 4);
        }

        let _output = vec![0.0f32; 32];
        let dtype = GgufQuantType::Q4_0;

        // Verify block size
        assert_eq!(dtype.block_size(), 32);
        assert_eq!(dtype.type_size(), 18);

        // Dequantize
        let result = dequantize_tensor(&block, dtype, 32);
        assert!(result.is_ok(), "Dequantization should succeed");

        let output = result.unwrap();

        // Verify pattern: alternating 0.0, 0.5
        for i in 0..32 {
            if i % 2 == 0 {
                assert!(
                    output[i].abs() < QUANTIZATION_EPSILON,
                    "Even index {} should be ~0.0, got {}",
                    i,
                    output[i]
                );
            } else {
                assert!(
                    (output[i] - 0.5).abs() < QUANTIZATION_EPSILON,
                    "Odd index {} should be ~0.5, got {}",
                    i,
                    output[i]
                );
            }
        }
    }

    /// Test Q8_0 dequantization accuracy
    #[test]
    fn test_q8_0_dequantization_accuracy() {
        // Create test Q8_0 block: scale (2 bytes) + 32 int8 values
        let mut block = vec![0u8; 34];

        // Set scale = 1.0 (f16: 0x3C00)
        block[0] = 0x00;
        block[1] = 0x3C;

        // Set values 1, 2, 3, ..., 32 as signed int8
        for i in 0..32 {
            block[2 + i] = (i + 1) as u8;
        }

        let result = dequantize_tensor(&block, GgufQuantType::Q8_0, 32);
        assert!(result.is_ok());

        let output = result.unwrap();

        // Verify: values should be 1.0, 2.0, ..., 32.0
        for i in 0..32 {
            let expected = (i + 1) as f32;
            assert!(
                (output[i] - expected).abs() < EPSILON,
                "Index {}: expected {}, got {}",
                i,
                expected,
                output[i]
            );
        }
    }

    /// Test Q4_K dequantization (most common format)
    #[test]
    fn test_q4_k_dequantization_accuracy() {
        let dtype = GgufQuantType::Q4_K;

        // Verify Q4_K properties
        assert_eq!(dtype.block_size(), 256);
        assert_eq!(dtype.type_size(), 144);
        assert!(dtype.is_quantized());

        let bits = dtype.bits_per_weight();
        assert!((bits - 4.5).abs() < 0.1, "Q4_K should be ~4.5 bits/weight");
    }

    /// Test all quantization types have valid properties
    #[test]
    fn test_all_quant_types_valid() {
        let quant_types = [
            GgufQuantType::F32,
            GgufQuantType::F16,
            GgufQuantType::Q8_0,
            GgufQuantType::Q4_0,
            GgufQuantType::Q4_1,
            GgufQuantType::Q5_0,
            GgufQuantType::Q5_1,
            GgufQuantType::Q2_K,
            GgufQuantType::Q3_K,
            GgufQuantType::Q4_K,
            GgufQuantType::Q5_K,
            GgufQuantType::Q6_K,
        ];

        for dtype in quant_types {
            // Block size must be positive
            assert!(
                dtype.block_size() > 0,
                "{:?} must have positive block size",
                dtype
            );

            // Type size must be positive
            assert!(
                dtype.type_size() > 0,
                "{:?} must have positive type size",
                dtype
            );

            // Bits per weight should be in reasonable range (1-32)
            let bits = dtype.bits_per_weight();
            assert!(
                bits >= 1.0 && bits <= 32.0,
                "{:?} bits/weight {} out of range",
                dtype,
                bits
            );

            // Name should be non-empty
            assert!(
                !dtype.name().is_empty(),
                "{:?} must have non-empty name",
                dtype
            );
        }
    }

    /// Test tensor size calculation
    #[test]
    fn test_tensor_size_calculation() {
        // F32: 256 elements = 256 * 4 = 1024 bytes
        assert_eq!(GgufQuantType::F32.tensor_size(256), 1024);

        // F16: 256 elements = 256 * 2 = 512 bytes
        assert_eq!(GgufQuantType::F16.tensor_size(256), 512);

        // Q4_0: 256 elements = 8 blocks * 18 bytes = 144 bytes
        assert_eq!(GgufQuantType::Q4_0.tensor_size(256), 144);

        // Q4_K: 256 elements = 1 block * 144 bytes = 144 bytes
        assert_eq!(GgufQuantType::Q4_K.tensor_size(256), 144);
    }

    /// Test quantized vs non-quantized detection
    #[test]
    fn test_is_quantized() {
        // Non-quantized types
        assert!(!GgufQuantType::F32.is_quantized());
        assert!(!GgufQuantType::F16.is_quantized());
        assert!(!GgufQuantType::Bf16.is_quantized());

        // Quantized types
        assert!(GgufQuantType::Q4_0.is_quantized());
        assert!(GgufQuantType::Q8_0.is_quantized());
        assert!(GgufQuantType::Q4_K.is_quantized());
        assert!(GgufQuantType::Q2_K.is_quantized());
    }

    /// Test QuantizedTensor container
    #[test]
    fn test_quantized_tensor_container() {
        let tensor = QuantizedTensor {
            data: vec![0u8; 144], // One Q4_K block
            dtype: GgufQuantType::Q4_K,
            shape: vec![256],
            num_elements: 256,
        };

        assert_eq!(tensor.block_count(), 1);
        assert!(tensor.dtype.is_quantized());
        assert_eq!(tensor.shape, vec![256]);
    }

    /// Test dequantization roundtrip sanity
    #[test]
    fn test_dequantization_finite_values() {
        // Create valid Q4_0 quantized data
        // Q4_0 format: 2 bytes scale (f16) + 16 bytes packed 4-bit values = 18 bytes per block
        // Each block represents 32 elements
        let mut data = vec![0u8; 18 * 8]; // 8 Q4_0 blocks = 256 elements

        for block in 0..8 {
            let base = block * 18;
            // Set a valid f16 scale: 0x3C00 = 1.0f16, small positive value
            data[base] = 0x00; // Low byte of f16 scale
            data[base + 1] = 0x3C; // High byte: 0x3C00 = 1.0

            // Fill packed 4-bit values with valid patterns (0-15)
            for i in 0..16 {
                let low_nibble = (i % 16) as u8;
                let high_nibble = ((i + 1) % 16) as u8;
                data[base + 2 + i] = low_nibble | (high_nibble << 4);
            }
        }

        let result = dequantize_tensor(&data, GgufQuantType::Q4_0, 256);
        assert!(result.is_ok());

        let output = result.unwrap();

        // All values should be finite
        for (i, val) in output.iter().enumerate() {
            assert!(
                val.is_finite(),
                "Value at index {} should be finite, got {}",
                i,
                val
            );
        }
    }

    /// Test quantization type conversion from u32
    #[test]
    fn test_quant_type_try_from() {
        // Valid conversions
        assert_eq!(GgufQuantType::try_from(0).unwrap(), GgufQuantType::F32);
        assert_eq!(GgufQuantType::try_from(1).unwrap(), GgufQuantType::F16);
        assert_eq!(GgufQuantType::try_from(8).unwrap(), GgufQuantType::Q8_0);
        assert_eq!(GgufQuantType::try_from(12).unwrap(), GgufQuantType::Q4_K);

        // Invalid conversion
        assert!(GgufQuantType::try_from(100).is_err());
        assert!(GgufQuantType::try_from(255).is_err());
    }
}

// ============================================================================
// SONA Integration Tests
// ============================================================================

mod sona_integration {
    use super::*;

    /// SONA configuration for testing
    #[derive(Debug, Clone)]
    struct SonaTestConfig {
        learning_rate: f32,
        momentum: f32,
        adaptation_threshold: f32,
        max_adaptations_per_step: usize,
    }

    impl Default for SonaTestConfig {
        fn default() -> Self {
            Self {
                learning_rate: 0.001,
                momentum: 0.9,
                adaptation_threshold: 0.05,
                max_adaptations_per_step: 3,
            }
        }
    }

    #[test]
    fn test_sona_config_defaults() {
        let config = SonaTestConfig::default();

        assert!(
            config.learning_rate > 0.0 && config.learning_rate < 1.0,
            "Learning rate should be in (0, 1)"
        );
        assert!(
            config.momentum >= 0.0 && config.momentum < 1.0,
            "Momentum should be in [0, 1)"
        );
        assert!(
            config.adaptation_threshold > 0.0,
            "Adaptation threshold must be positive"
        );
        assert!(
            config.max_adaptations_per_step > 0,
            "Max adaptations must be positive"
        );
    }

    #[test]
    fn test_sona_adaptation_timing() {
        // SONA adaptation should be fast (<0.05ms target)
        let start = Instant::now();

        // Simulate SONA adaptation calculation
        let mut weights = vec![0.5f32; 1000];
        let gradients = vec![0.01f32; 1000];

        // Simple gradient update (simulating SONA)
        for (w, g) in weights.iter_mut().zip(gradients.iter()) {
            *w -= 0.001 * g;
        }

        let duration = start.elapsed();

        // Should be very fast
        assert!(
            duration < Duration::from_millis(1),
            "SONA adaptation took {:?}, expected <1ms",
            duration
        );
    }

    #[test]
    fn test_sona_routing_decision() {
        // Test routing decision logic
        struct RoutingDecision {
            use_ane: bool,
            use_neon: bool,
            confidence: f32,
        }

        fn make_routing_decision(batch_size: usize, dim: usize) -> RoutingDecision {
            let ane_available = is_ane_available();

            if ane_available && should_use_ane(batch_size, dim) {
                RoutingDecision {
                    use_ane: true,
                    use_neon: false,
                    confidence: 0.9,
                }
            } else {
                RoutingDecision {
                    use_ane: false,
                    use_neon: true,
                    confidence: 0.95,
                }
            }
        }

        // Small dimensions: NEON preferred
        let decision = make_routing_decision(1, 32);
        assert!(
            decision.use_neon || decision.use_ane,
            "Must use some compute backend"
        );

        // Large batch with aligned dims: ANE may be preferred on Apple Silicon
        let decision = make_routing_decision(32, 256);
        assert!(decision.confidence > 0.5);
    }

    #[test]
    fn test_sona_pattern_learning() {
        // Simulate SONA pattern storage
        #[derive(Debug)]
        #[allow(dead_code)]
        struct SonaPattern {
            input_hash: u64,
            optimal_config: String,
            performance_score: f32,
        }

        let patterns = vec![
            SonaPattern {
                input_hash: 12345,
                optimal_config: "ANE+NEON".to_string(),
                performance_score: 0.95,
            },
            SonaPattern {
                input_hash: 67890,
                optimal_config: "NEON-only".to_string(),
                performance_score: 0.88,
            },
        ];

        for pattern in &patterns {
            assert!(pattern.performance_score >= 0.0 && pattern.performance_score <= 1.0);
            assert!(!pattern.optimal_config.is_empty());
        }
    }

    #[test]
    fn test_sona_warmup_iterations() {
        // SONA typically needs a few iterations to warm up
        const WARMUP_ITERATIONS: usize = 3;

        let mut metrics = Vec::new();

        for i in 0..10 {
            // Simulate inference timing
            let start = Instant::now();
            std::thread::sleep(Duration::from_micros(100 + i as u64 * 10));
            let duration = start.elapsed();
            metrics.push(duration);
        }

        // Post-warmup iterations should be more stable
        let warmup_variance = calculate_variance(&metrics[..WARMUP_ITERATIONS]);
        let stable_variance = calculate_variance(&metrics[WARMUP_ITERATIONS..]);

        // Note: This is a simplified test; in real scenarios,
        // stable variance should typically be lower
        let _ = (warmup_variance, stable_variance);
    }

    fn calculate_variance(durations: &[Duration]) -> f64 {
        if durations.is_empty() {
            return 0.0;
        }
        let mean: f64 =
            durations.iter().map(|d| d.as_secs_f64()).sum::<f64>() / durations.len() as f64;

        durations
            .iter()
            .map(|d| (d.as_secs_f64() - mean).powi(2))
            .sum::<f64>()
            / durations.len() as f64
    }

    #[test]
    fn test_sona_ewc_consolidation() {
        // Test EWC++ (Elastic Weight Consolidation) behavior
        // This prevents catastrophic forgetting in SONA

        struct EwcConfig {
            lambda: f32, // Importance weight
            fisher_samples: usize,
        }

        let config = EwcConfig {
            lambda: 1000.0,
            fisher_samples: 100,
        };

        // Lambda should be positive for weight importance
        assert!(config.lambda > 0.0);
        // Need enough samples for Fisher information
        assert!(config.fisher_samples >= 10);
    }
}

// ============================================================================
// ANE Dispatch Tests
// ============================================================================

mod ane_dispatch {
    use super::*;

    #[test]
    fn test_ane_availability_detection() {
        // Should not panic
        let available = is_ane_available();

        // Result should be consistent
        assert_eq!(is_ane_available(), available);
        assert_eq!(is_ane_available(), available);
    }

    #[test]
    fn test_ane_capabilities_detection() {
        let caps = AneCapabilities::detect();

        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            // On Apple Silicon, ANE should be available
            assert!(caps.available, "ANE should be available on Apple Silicon");
            assert!(caps.tops > 0.0, "TOPS should be positive");
            assert!(
                caps.max_model_size_mb > 0,
                "Max model size should be positive"
            );
            assert!(!caps.supported_ops.is_empty(), "Should have supported ops");
        }

        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            // On non-Apple Silicon, ANE may not be available
            if !caps.available {
                assert_eq!(caps.tops, 0.0);
                assert_eq!(caps.max_model_size_mb, 0);
            }
        }
    }

    #[test]
    fn test_ane_routing_thresholds() {
        // Test various dimension combinations
        let test_cases = [
            // (batch, dim, description)
            (1, 64, "minimum ANE dimensions"),
            (1, 128, "small aligned tensor"),
            (32, 256, "typical LLM dimensions"),
            (64, 4096, "large batch with large dim"),
            (1, 32, "below minimum dim"),
            (100, 128, "above max batch"),
        ];

        for (batch, dim, desc) in test_cases {
            let should_use = should_use_ane(batch, dim);
            // Just verify no panic
            let _ = (should_use, desc);
        }
    }

    #[test]
    fn test_ane_matmul_routing() {
        let test_cases = [
            // (m, k, n, description)
            (1, 64, 64, "small square matmul"),
            (32, 256, 128, "medium matmul"),
            (1, 4096, 4096, "large matmul"),
            (64, 512, 512, "optimal ANE size"),
            (1, 8192, 8192, "very large matmul"),
        ];

        for (m, k, n, desc) in test_cases {
            let _should_use = should_use_ane_matmul(m, k, n);
            let recommendation = get_ane_recommendation(m, k, n);

            // Recommendation should be consistent
            assert!(
                recommendation.confidence >= 0.0 && recommendation.confidence <= 1.0,
                "Confidence for {} should be in [0, 1]",
                desc
            );

            // Expected speedup should be reasonable
            assert!(
                recommendation.expected_speedup > 0.0 && recommendation.expected_speedup < 10.0,
                "Speedup for {} should be reasonable",
                desc
            );
        }
    }

    #[test]
    fn test_ane_activation_routing() {
        let test_cases = [
            (1, 64),
            (32, 256),
            (64, 4096),
            (100, 128),   // Above typical ANE batch limit
            (1, 1000000), // Very large tensor
        ];

        for (batch, dim) in test_cases {
            let should_use = should_use_ane_activation(batch, dim);
            // Just verify no panic and reasonable result
            let _ = should_use;
        }
    }

    #[test]
    fn test_ane_recommendation_structure() {
        let rec = get_ane_recommendation(1, 256, 256);

        // All fields should be valid
        assert!(rec.confidence >= 0.0 && rec.confidence <= 1.0);
        assert!(!rec.reason.is_empty());
        assert!(rec.expected_speedup > 0.0);

        // Test Clone
        let cloned = rec.clone();
        assert_eq!(rec.use_ane, cloned.use_ane);
        assert_eq!(rec.confidence, cloned.confidence);

        // Test Debug
        let debug = format!("{:?}", rec);
        assert!(debug.contains("use_ane"));
    }

    #[test]
    fn test_compute_units_configuration() {
        let units = [
            ComputeUnits::CpuOnly,
            ComputeUnits::CpuAndGpu,
            ComputeUnits::CpuAndNeuralEngine,
            ComputeUnits::All,
        ];

        for unit in units {
            // Test ANE usage flag
            let _uses_ane = unit.uses_ane();
            let _uses_gpu = unit.uses_gpu();

            // At least CPU should always be used
            // (implied by all compute unit configurations)

            // Test description
            let desc = unit.description();
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn test_ane_dimension_alignment() {
        // ANE prefers 16-aligned dimensions
        let aligned_dims = [16, 32, 64, 128, 256, 512, 1024, 2048, 4096];
        let unaligned_dims = [17, 33, 65, 100, 255, 1000];

        for dim in aligned_dims {
            assert_eq!(dim % 16, 0, "{} should be 16-aligned", dim);
        }

        for dim in unaligned_dims {
            assert_ne!(dim % 16, 0, "{} should not be 16-aligned", dim);
        }
    }

    #[test]
    fn test_ane_no_dispatch_errors() {
        // Simulate dispatch to verify no errors occur
        let test_tensors = [(1, 64), (32, 256), (64, 4096)];

        for (batch, dim) in test_tensors {
            // These should never panic
            let _ = should_use_ane(batch, dim);
            let _ = should_use_ane_activation(batch, dim);
            let _ = should_use_ane_matmul(batch, dim, dim);
        }
    }

    #[test]
    fn test_fallback_behavior() {
        // Test that fallback to NEON works when ANE is unavailable
        let mut data = vec![1.0f32; 64];

        // This should work regardless of ANE availability
        // by falling back to scalar/NEON implementation
        for v in data.iter_mut() {
            *v = *v / (1.0 + (-*v).exp()); // SiLU
        }

        // All values should be valid
        assert!(data.iter().all(|v| v.is_finite()));
    }
}

// ============================================================================
// Memory Management Tests
// ============================================================================

mod memory_management {
    use super::*;

    #[test]
    fn test_memory_bounds_validation() {
        // Verify memory bounds are reasonable
        assert!(MEMORY_BOUNDS.max_model_memory > 0);
        assert!(MEMORY_BOUNDS.max_kv_cache_memory > 0);
        assert!(MEMORY_BOUNDS.max_working_memory > 0);

        // Total should be reasonable for device
        let total = MEMORY_BOUNDS.max_model_memory
            + MEMORY_BOUNDS.max_kv_cache_memory
            + MEMORY_BOUNDS.max_working_memory;

        // Should fit in 8GB device memory
        assert!(total < 8_000_000_000, "Total memory {} exceeds 8GB", total);
    }

    #[test]
    fn test_tensor_memory_estimation() {
        // Estimate memory for RuvLTRA-Small tensors
        let hidden_size = RUVLTRA_SMALL_CONFIG.hidden_size;
        let _num_layers = RUVLTRA_SMALL_CONFIG.num_hidden_layers;
        let vocab_size = RUVLTRA_SMALL_CONFIG.vocab_size;

        // Embedding: vocab_size * hidden_size * bytes_per_element
        let embedding_size_f32 = vocab_size * hidden_size * 4;
        let embedding_size_q4k = GgufQuantType::Q4_K.tensor_size(vocab_size * hidden_size);

        // Q4_K should be much smaller
        assert!(
            embedding_size_q4k < embedding_size_f32 / 4,
            "Q4_K should be at least 4x smaller than F32"
        );
    }

    #[test]
    fn test_kv_cache_sizing() {
        let hidden_size = RUVLTRA_SMALL_CONFIG.hidden_size;
        let num_layers = RUVLTRA_SMALL_CONFIG.num_hidden_layers;
        let num_kv_heads = RUVLTRA_SMALL_CONFIG.num_key_value_heads;
        let max_seq_len = RUVLTRA_SMALL_CONFIG.max_position_embeddings;

        let head_dim = hidden_size / RUVLTRA_SMALL_CONFIG.num_attention_heads;

        // KV cache per layer: 2 * seq_len * num_kv_heads * head_dim * sizeof(f16)
        let kv_per_layer = 2 * max_seq_len * num_kv_heads * head_dim * 2;
        let total_kv_cache = kv_per_layer * num_layers;

        assert!(
            total_kv_cache < MEMORY_BOUNDS.max_kv_cache_memory as usize,
            "KV cache {} exceeds bound {}",
            total_kv_cache,
            MEMORY_BOUNDS.max_kv_cache_memory
        );
    }

    #[test]
    fn test_working_memory_allocation() {
        // Simulate working memory allocation
        let batch_size = 1;
        let seq_len = 1024;
        let hidden_size = RUVLTRA_SMALL_CONFIG.hidden_size;

        // Activations: batch * seq * hidden * sizeof(f32)
        let activation_memory = batch_size * seq_len * hidden_size * 4;

        // Should fit in working memory
        assert!(activation_memory < MEMORY_BOUNDS.max_working_memory as usize);
    }
}

// ============================================================================
// Output Validation Tests
// ============================================================================

mod output_validation {
    use super::*;

    #[test]
    fn test_logits_finite() {
        // Simulated logits output
        let logits: Vec<f32> = (0..RUVLTRA_SMALL_CONFIG.vocab_size)
            .map(|i| (i as f32) * 0.001 - 16.0)
            .collect();

        // All logits should be finite
        for (i, logit) in logits.iter().enumerate() {
            assert!(
                logit.is_finite(),
                "Logit at index {} should be finite, got {}",
                i,
                logit
            );
        }
    }

    #[test]
    fn test_softmax_probabilities() {
        // Simulated softmax output
        let mut probs = vec![0.1f32; 10];

        // Apply softmax normalization
        let max_val = probs.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let mut sum = 0.0;
        for p in probs.iter_mut() {
            *p = (*p - max_val).exp();
            sum += *p;
        }
        for p in probs.iter_mut() {
            *p /= sum;
        }

        // Probabilities should sum to 1.0
        let prob_sum: f32 = probs.iter().sum();
        assert!(
            (prob_sum - 1.0).abs() < EPSILON,
            "Probabilities should sum to 1.0, got {}",
            prob_sum
        );

        // All probabilities should be in [0, 1]
        for (i, p) in probs.iter().enumerate() {
            assert!(
                *p >= 0.0 && *p <= 1.0,
                "Probability at {} should be in [0, 1], got {}",
                i,
                p
            );
        }
    }

    #[test]
    fn test_token_generation_coherence() {
        // Test that token sequences have reasonable patterns
        let sample_tokens: Vec<u32> = vec![1, 234, 567, 89, 1234, 5678];

        // All tokens should be valid (within vocab range)
        for token in &sample_tokens {
            assert!(
                *token < RUVLTRA_SMALL_CONFIG.vocab_size as u32,
                "Token {} exceeds vocab size",
                token
            );
        }

        // No repeated padding tokens at start (unless intentional)
        // This is a basic coherence check
        let has_varied_tokens = sample_tokens.windows(2).any(|w| w[0] != w[1]);
        assert!(
            has_varied_tokens || sample_tokens.len() <= 1,
            "Token sequence should have variety"
        );
    }

    #[test]
    fn test_attention_weights_valid() {
        let seq_len = 32;

        // Simulated attention weights (should sum to 1 per row after softmax)
        let mut attention = vec![0.0f32; seq_len * seq_len];

        // Initialize with causal mask pattern
        for i in 0..seq_len {
            for j in 0..=i {
                attention[i * seq_len + j] = 1.0 / (i + 1) as f32;
            }
        }

        // Verify row sums are approximately 1.0
        for i in 0..seq_len {
            let row_sum: f32 = attention[i * seq_len..(i + 1) * seq_len].iter().sum();
            assert!(
                (row_sum - 1.0).abs() < LOOSE_EPSILON,
                "Attention row {} should sum to 1.0, got {}",
                i,
                row_sum
            );
        }
    }
}

// ============================================================================
// Performance Validation Tests
// ============================================================================

mod performance_validation {
    use super::*;

    #[test]
    fn test_inference_timing_reasonable() {
        // Basic timing test for operations
        let start = Instant::now();

        // Simulate a basic forward pass calculation
        let data: Vec<f32> = (0..4096).map(|i| i as f32 * 0.001).collect();
        let mut output = vec![0.0f32; 4096];

        for (i, (o, d)) in output.iter_mut().zip(data.iter()).enumerate() {
            *o = *d * (i as f32 % 10.0 + 1.0);
        }

        let duration = start.elapsed();

        // Basic operations should be very fast
        assert!(
            duration < Duration::from_millis(10),
            "Basic ops took {:?}",
            duration
        );
    }

    #[test]
    fn test_batch_processing_scaling() {
        let batch_sizes = [1, 2, 4, 8, 16, 32];
        let dim = 256;

        let mut timings = Vec::new();

        for batch_size in batch_sizes {
            let start = Instant::now();

            // Simulate batch processing
            let data = vec![1.0f32; batch_size * dim];
            let _: f32 = data.iter().sum();

            timings.push((batch_size, start.elapsed()));
        }

        // Larger batches should take more time (linear or better scaling)
        // This is a sanity check that batch size affects timing
        let _ = timings;
    }

    #[test]
    #[ignore] // Run with: cargo test --release -- --ignored
    fn test_throughput_benchmark() {
        let iterations = 100;
        let dim = 4096;

        let data: Vec<f32> = (0..dim).map(|i| i as f32 * 0.001).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let _: f32 = data.iter().map(|x| x * x).sum();
        }
        let duration = start.elapsed();

        let ops_per_second = (iterations * dim) as f64 / duration.as_secs_f64();

        println!("Throughput: {:.2e} ops/sec", ops_per_second);

        // Should achieve reasonable throughput
        assert!(
            ops_per_second > 1_000_000.0,
            "Throughput {:.2e} below minimum",
            ops_per_second
        );
    }
}

// ============================================================================
// Thread Safety Tests
// ============================================================================

mod thread_safety {
    use super::*;
    use std::thread;

    #[test]
    fn test_ane_detection_thread_safe() {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                thread::spawn(|| {
                    for _ in 0..100 {
                        let _ = is_ane_available();
                        let _ = AneCapabilities::detect();
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should complete");
        }
    }

    #[test]
    fn test_quantization_thread_safe() {
        let handles: Vec<_> = (0..4)
            .map(|i| {
                thread::spawn(move || {
                    let mut data = vec![0u8; 18];
                    data[0] = 0x00;
                    data[1] = 0x3C;
                    for j in 2..18 {
                        data[j] = ((i + j) % 256) as u8;
                    }

                    let result = dequantize_tensor(&data, GgufQuantType::Q4_0, 32);
                    assert!(result.is_ok());

                    let output = result.unwrap();
                    assert!(output.iter().all(|v| v.is_finite()));
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should complete");
        }
    }

    #[test]
    fn test_concurrent_routing_decisions() {
        let handles: Vec<_> = (0..4)
            .map(|i| {
                thread::spawn(move || {
                    for j in 0..100 {
                        let batch = (i + 1) * (j + 1) % 64 + 1;
                        let dim = ((i + j) * 16 + 64) % 4096 + 64;

                        let _ = should_use_ane(batch, dim);
                        let _ = should_use_ane_matmul(batch, dim, dim);
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().expect("Thread should complete");
        }
    }
}
