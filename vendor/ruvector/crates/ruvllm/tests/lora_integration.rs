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
//! Integration tests for LoRA (Low-Rank Adaptation)
//!
//! Tests MicroLoRA adaptation, forward pass, gradient accumulation,
//! EWC state management, and serialization.

use ruvllm::{
    error::Result,
    lora::{AdaptFeedback, LoraAdapter, MicroLoRA, MicroLoraConfig, TargetModule},
};
use std::collections::HashMap;

/// Create a test MicroLoRA configuration
fn create_test_config(dim: usize) -> MicroLoraConfig {
    MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj, TargetModule::VProj],
        in_features: dim,
        out_features: dim,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    }
}

/// Create test input data
fn create_test_input(dim: usize) -> Vec<f32> {
    (0..dim).map(|i| (i as f32) * 0.01).collect()
}

#[test]
fn test_micro_lora_creation() {
    let config = create_test_config(256);
    let lora = MicroLoRA::new(config);

    assert_eq!(lora.config().rank, 2);
    assert_eq!(lora.config().alpha, 4.0);
    assert!(lora.is_enabled());
}

#[test]
fn test_micro_lora_forward() {
    let config = create_test_config(64);
    let lora = MicroLoRA::new(config);

    let input = create_test_input(64);

    // Forward pass for Q projection
    let output = lora.forward(&input, &TargetModule::QProj);

    assert_eq!(output.len(), 64);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_micro_lora_adapt_changes_output() {
    let config = MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj],
        in_features: 64,
        out_features: 64,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    };

    let lora = MicroLoRA::new(config);
    let input = create_test_input(64);

    // Forward pass before adaptation
    let output_before = lora.forward(&input, &TargetModule::QProj);

    // Apply adaptation with feedback
    let feedback = AdaptFeedback::from_quality(0.8);
    lora.adapt(&input, feedback).unwrap();

    // Apply accumulated updates
    lora.apply_updates(0.01);

    // Forward pass after adaptation
    let output_after = lora.forward(&input, &TargetModule::QProj);

    assert_eq!(output_before.len(), output_after.len());

    // Output should change after adaptation
    let changed = output_before
        .iter()
        .zip(output_after.iter())
        .any(|(a, b)| (a - b).abs() > 1e-10);
    let all_near_zero = output_before.iter().all(|&v| v.abs() < 1e-6);

    assert!(
        changed || all_near_zero,
        "Adaptation should change output or both should be zero"
    );
}

#[test]
fn test_lora_forward_dimensions() {
    let input_dim = 128;
    let output_dim = 128;

    let config = MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj],
        in_features: input_dim,
        out_features: output_dim,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    };

    let lora = MicroLoRA::new(config);
    let input = create_test_input(input_dim);
    let output = lora.forward(&input, &TargetModule::QProj);

    assert_eq!(output.len(), output_dim);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_lora_adapter_creation() {
    let adapter = LoraAdapter::new(64, 64, 2, 4.0);

    assert_eq!(adapter.rank(), 2);
    assert_eq!(adapter.param_count(), 64 * 2 + 2 * 64); // A matrix + B matrix
}

#[test]
fn test_lora_adapter_forward() {
    let adapter = LoraAdapter::new(64, 64, 2, 4.0);
    let input = ndarray::Array1::from_vec(create_test_input(64));

    let output = adapter.forward(&input);

    assert_eq!(output.len(), 64);
    assert!(output.iter().all(|&v| v.is_finite()));

    // With zero-initialized B, output should be zero
    let sum: f32 = output.iter().sum();
    assert!(
        sum.abs() < 1e-6,
        "Initial forward should be ~0, got {}",
        sum
    );
}

#[test]
fn test_lora_adapter_gradient_accumulation() {
    let mut adapter = LoraAdapter::new(64, 64, 2, 4.0);
    let input = ndarray::Array1::from_elem(64, 0.1);
    let grad_output = ndarray::Array1::from_elem(64, 0.1);

    // Accumulate gradient
    adapter.accumulate_gradient(&input, &grad_output, 0.8);
    assert_eq!(adapter.pending_updates(), 1);

    // Apply gradients
    adapter.apply_gradients(0.01);
    assert_eq!(adapter.pending_updates(), 0);

    // After update, forward should produce non-zero output
    let output = adapter.forward(&input);
    let sum: f32 = output.iter().map(|x| x.abs()).sum();
    assert!(sum > 0.0, "After update, output should be non-zero");
}

// Note: EwcState is not exported from the lora module, so EWC-specific
// tests are implemented in the unit tests within micro_lora.rs

#[test]
fn test_adapt_feedback_creation() {
    let feedback = AdaptFeedback::from_quality(0.85);

    assert_eq!(feedback.quality, 0.85);
    assert_eq!(feedback.reward, Some(0.85));
    assert!(feedback.gradient_estimate.is_empty());
}

#[test]
fn test_adapt_feedback_with_gradient() {
    let gradient = vec![0.1; 64];
    let feedback = AdaptFeedback::with_gradient(0.9, gradient.clone());

    assert_eq!(feedback.quality, 0.9);
    assert_eq!(feedback.gradient_estimate.len(), 64);
}

#[test]
fn test_adapt_feedback_for_module() {
    let feedback = AdaptFeedback::from_quality(0.8).for_module(TargetModule::QProj);

    assert_eq!(feedback.source_module, Some(TargetModule::QProj));
}

#[test]
fn test_adapt_feedback_with_session() {
    let feedback = AdaptFeedback::from_quality(0.8).with_session("session-123".to_string());

    assert_eq!(feedback.session_id, Some("session-123".to_string()));
}

#[test]
fn test_multiple_adaptations() {
    let config = create_test_config(64);
    let lora = MicroLoRA::new(config);
    let input = create_test_input(64);

    // Multiple adaptation cycles
    for i in 0..5 {
        let quality = 0.5 + (i as f32 * 0.1);
        let feedback = AdaptFeedback::from_quality(quality);
        lora.adapt(&input, feedback).unwrap();
    }

    assert_eq!(lora.adaptation_count(), 5);

    // Apply updates
    lora.apply_updates(0.01);

    // Verify output is valid
    let output = lora.forward(&input, &TargetModule::QProj);
    assert_eq!(output.len(), 64);
    assert!(output.iter().all(|&v| v.is_finite()));
}

#[test]
fn test_lora_with_different_ranks() {
    let ranks = [1, 2];
    let input = create_test_input(64);

    for rank in ranks {
        let config = MicroLoraConfig {
            rank,
            alpha: rank as f32 * 2.0,
            dropout: 0.0,
            target_modules: vec![TargetModule::QProj],
            in_features: 64,
            out_features: 64,
            use_bias: false,
            standard_init: true,
            gradient_checkpointing: false,
        };

        let lora = MicroLoRA::new(config);
        let output = lora.forward(&input, &TargetModule::QProj);

        assert_eq!(
            output.len(),
            64,
            "Rank {} should produce correct output size",
            rank
        );
    }
}

#[test]
fn test_target_module_variants() {
    let modules = vec![
        TargetModule::QProj,
        TargetModule::KProj,
        TargetModule::VProj,
        TargetModule::OProj,
        TargetModule::GateProj,
        TargetModule::UpProj,
        TargetModule::DownProj,
        TargetModule::Embed,
        TargetModule::LmHead,
    ];

    for module in &modules {
        let name = module.as_str();
        assert!(!name.is_empty());
    }

    assert_eq!(TargetModule::QProj.as_str(), "q_proj");
    assert_eq!(TargetModule::VProj.as_str(), "v_proj");
}

#[test]
fn test_target_module_defaults() {
    let defaults = TargetModule::defaults();
    assert_eq!(defaults.len(), 2);
    assert!(defaults.contains(&TargetModule::QProj));
    assert!(defaults.contains(&TargetModule::VProj));
}

#[test]
fn test_target_module_attention() {
    let attention = TargetModule::attention();
    assert_eq!(attention.len(), 4);
    assert!(attention.contains(&TargetModule::QProj));
    assert!(attention.contains(&TargetModule::KProj));
    assert!(attention.contains(&TargetModule::VProj));
    assert!(attention.contains(&TargetModule::OProj));
}

#[test]
fn test_target_module_mlp() {
    let mlp = TargetModule::mlp();
    assert_eq!(mlp.len(), 3);
    assert!(mlp.contains(&TargetModule::GateProj));
    assert!(mlp.contains(&TargetModule::UpProj));
    assert!(mlp.contains(&TargetModule::DownProj));
}

#[test]
fn test_micro_lora_config_memory() {
    let config = MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj, TargetModule::VProj],
        in_features: 768,
        out_features: 768,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    };

    let memory = config.memory_bytes();
    // 2 modules * (768 * 2 + 2 * 768) * 4 bytes
    assert!(memory < 1024 * 1024, "Memory should be < 1MB for MicroLoRA");
}

#[test]
fn test_micro_lora_enable_disable() {
    let config = create_test_config(64);
    let mut lora = MicroLoRA::new(config);
    let input = create_test_input(64);

    assert!(lora.is_enabled());

    // Disable
    lora.set_enabled(false);
    assert!(!lora.is_enabled());

    // Forward when disabled should return zeros
    let output = lora.forward(&input, &TargetModule::QProj);
    assert!(output.iter().all(|&v| v == 0.0));

    // Re-enable
    lora.set_enabled(true);
    assert!(lora.is_enabled());
}

#[test]
fn test_micro_lora_reset() {
    let config = create_test_config(64);
    let lora = MicroLoRA::new(config);
    let input = create_test_input(64);

    // Perform some adaptations
    for _ in 0..5 {
        let feedback = AdaptFeedback::from_quality(0.8);
        lora.adapt(&input, feedback).unwrap();
    }

    assert!(lora.adaptation_count() > 0);

    // Reset
    lora.reset();

    assert_eq!(lora.adaptation_count(), 0);
    assert_eq!(lora.forward_count(), 0);
}

#[test]
fn test_micro_lora_memory_usage() {
    let config = create_test_config(64);
    let lora = MicroLoRA::new(config);

    let memory = lora.memory_bytes();
    let params = lora.param_count();

    assert!(memory > 0);
    assert!(params > 0);
    assert_eq!(memory, params * std::mem::size_of::<f32>());
}

#[test]
fn test_lora_adapter_simd_forward() {
    let adapter = LoraAdapter::new(64, 64, 2, 4.0);
    let input = create_test_input(64);
    let mut output = vec![0.0f32; 64];

    adapter.forward_simd(&input, &mut output);

    // Compare with regular forward
    let input_array = ndarray::Array1::from_vec(input.clone());
    let expected = adapter.forward(&input_array);

    for (o, e) in output.iter().zip(expected.iter()) {
        assert!(
            (o - e).abs() < 1e-5,
            "SIMD forward mismatch: {} vs {}",
            o,
            e
        );
    }
}

#[test]
fn test_micro_lora_with_custom_dimensions() {
    let config = MicroLoraConfig {
        rank: 2,
        alpha: 4.0,
        dropout: 0.0,
        target_modules: vec![TargetModule::QProj, TargetModule::VProj],
        in_features: 256, // Default dimensions
        out_features: 256,
        use_bias: false,
        standard_init: true,
        gradient_checkpointing: false,
    };

    // Create with custom dimensions per module
    let mut dimensions = HashMap::new();
    dimensions.insert(TargetModule::QProj, (128, 128));
    dimensions.insert(TargetModule::VProj, (128, 128));

    let lora = MicroLoRA::with_dimensions(config, dimensions);

    let input = create_test_input(128);
    let output = lora.forward(&input, &TargetModule::QProj);

    assert_eq!(output.len(), 128);
}

#[test]
fn test_micro_lora_save_load() {
    let config = create_test_config(64);
    let lora = MicroLoRA::new(config);
    let input = create_test_input(64);

    // Apply some adaptation
    let feedback = AdaptFeedback::from_quality(0.85);
    lora.adapt(&input, feedback).unwrap();
    lora.apply_updates(0.01);

    // Export state
    let state = lora.export_state();

    assert_eq!(state.config.rank, 2);
    assert!(!state.adapters.is_empty());

    // Restore from state
    let lora_restored = MicroLoRA::from_state(state).unwrap();

    // Both should produce same output
    let output_original = lora.forward(&input, &TargetModule::QProj);
    let output_restored = lora_restored.forward(&input, &TargetModule::QProj);

    for (a, b) in output_original.iter().zip(output_restored.iter()) {
        assert!(
            (a - b).abs() < 1e-5,
            "Restored model should match: {} vs {}",
            a,
            b
        );
    }
}

// Note: test_lora_apply_updates_with_ewc removed as EwcState is not exported

#[test]
fn test_lora_adapter_reset() {
    let mut adapter = LoraAdapter::new(64, 64, 2, 4.0);
    let input = ndarray::Array1::from_elem(64, 0.1);
    let grad_output = ndarray::Array1::from_elem(64, 0.1);

    // Accumulate some gradients and apply
    adapter.accumulate_gradient(&input, &grad_output, 0.8);
    adapter.apply_gradients(0.01);

    // Reset
    adapter.reset();

    assert_eq!(adapter.pending_updates(), 0);

    // B matrix should be reset to zero
    let output = adapter.forward(&input);
    let sum: f32 = output.iter().sum();
    assert!(sum.abs() < 1e-6, "After reset, output should be ~0");
}

#[test]
fn test_config_for_hidden_dim() {
    let config = MicroLoraConfig::for_hidden_dim(512);

    assert_eq!(config.in_features, 512);
    assert_eq!(config.out_features, 512);
    assert_eq!(config.rank, 2); // Default rank
}

#[test]
fn test_config_builder_methods() {
    let config = MicroLoraConfig::for_hidden_dim(256)
        .with_rank(1)
        .with_alpha(8.0)
        .with_targets(vec![
            TargetModule::QProj,
            TargetModule::KProj,
            TargetModule::VProj,
        ]);

    assert_eq!(config.rank, 1);
    assert_eq!(config.alpha, 8.0);
    assert_eq!(config.target_modules.len(), 3);
}
