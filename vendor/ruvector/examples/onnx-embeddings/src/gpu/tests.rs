//! GPU Module Tests
//!
//! Comprehensive tests for GPU acceleration functionality.

use super::*;
use super::config::{GpuConfig, GpuMode, PowerPreference, GpuMemoryStats};
use super::backend::CpuBackend;
use super::shaders::ShaderModule;

// ==================== Configuration Tests ====================

#[test]
fn test_gpu_config_default() {
    let config = GpuConfig::default();

    assert_eq!(config.mode, GpuMode::Auto);
    assert_eq!(config.power_preference, PowerPreference::HighPerformance);
    assert_eq!(config.workgroup_size, 256);
    assert!(config.fallback_to_cpu);
    assert!(config.cache_shaders);
}

#[test]
fn test_gpu_config_builder() {
    let config = GpuConfig::auto()
        .with_mode(GpuMode::WebGpu)
        .with_power_preference(PowerPreference::LowPower)
        .with_workgroup_size(512)
        .with_min_batch_size(32)
        .with_min_dimension(256)
        .with_profiling(true);

    assert_eq!(config.mode, GpuMode::WebGpu);
    assert_eq!(config.power_preference, PowerPreference::LowPower);
    assert_eq!(config.workgroup_size, 512);
    assert_eq!(config.min_batch_size, 32);
    assert_eq!(config.min_dimension, 256);
    assert!(config.enable_profiling);
}

#[test]
fn test_should_use_gpu() {
    let config = GpuConfig::default()
        .with_min_batch_size(16)
        .with_min_dimension(128);

    // Below minimum batch size
    assert!(!config.should_use_gpu(8, 384));

    // Below minimum dimension
    assert!(!config.should_use_gpu(32, 64));

    // Both conditions met
    assert!(config.should_use_gpu(32, 384));

    // CPU only mode
    let cpu_config = GpuConfig::cpu_only();
    assert!(!cpu_config.should_use_gpu(1000, 1000));
}

#[test]
fn test_preset_configs() {
    let high_perf = GpuConfig::high_performance();
    assert_eq!(high_perf.workgroup_size, 512);
    assert_eq!(high_perf.min_batch_size, 8);

    let low_power = GpuConfig::low_power();
    assert_eq!(low_power.power_preference, PowerPreference::LowPower);
    assert_eq!(low_power.workgroup_size, 128);

    let cpu_only = GpuConfig::cpu_only();
    assert_eq!(cpu_only.mode, GpuMode::CpuOnly);
}

// ==================== Shader Tests ====================

#[test]
fn test_shader_registry_initialization() {
    let registry = ShaderRegistry::new();

    let expected_shaders = vec![
        "cosine_similarity",
        "batch_cosine_similarity",
        "dot_product",
        "euclidean_distance",
        "l2_normalize",
        "mean_pool",
        "max_pool",
        "cls_pool",
        "matmul",
        "vector_add",
        "vector_scale",
    ];

    for name in expected_shaders {
        assert!(registry.get(name).is_some(), "Missing shader: {}", name);
    }
}

#[test]
fn test_shader_module_content() {
    let registry = ShaderRegistry::new();

    // Check cosine similarity shader
    let cosine = registry.get("cosine_similarity").unwrap();
    assert!(cosine.source.contains("@compute"));
    assert!(cosine.source.contains("workgroup_size"));
    assert!(cosine.source.contains("cosine_similarity"));
    assert_eq!(cosine.entry_point, "cosine_similarity");
    assert_eq!(cosine.workgroup_size, [256, 1, 1]);

    // Check mean pool shader
    let mean_pool = registry.get("mean_pool").unwrap();
    assert!(mean_pool.source.contains("attention_mask"));
    assert!(mean_pool.source.contains("hidden_size"));
    assert_eq!(mean_pool.entry_point, "mean_pool");
}

#[test]
fn test_custom_shader_registration() {
    let mut registry = ShaderRegistry::new();

    let custom = ShaderModule {
        name: "custom_kernel".to_string(),
        source: "@compute @workgroup_size(64) fn custom() {}".to_string(),
        entry_point: "custom".to_string(),
        workgroup_size: [64, 1, 1],
    };

    registry.register(custom);

    assert!(registry.get("custom_kernel").is_some());
    let retrieved = registry.get("custom_kernel").unwrap();
    assert_eq!(retrieved.entry_point, "custom");
}

// ==================== Batch Operations Tests ====================

#[test]
fn test_batch_cosine_similarity() {
    let query = vec![1.0, 0.0, 0.0];
    let candidates: Vec<&[f32]> = vec![
        &[1.0, 0.0, 0.0][..],  // similarity = 1.0
        &[0.0, 1.0, 0.0][..],  // similarity = 0.0
        &[-1.0, 0.0, 0.0][..], // similarity = -1.0
    ];

    let results = batch_cosine_similarity_gpu(&query, &candidates);

    assert_eq!(results.len(), 3);
    assert!((results[0] - 1.0).abs() < 1e-6);
    assert!(results[1].abs() < 1e-6);
    assert!((results[2] - (-1.0)).abs() < 1e-6);
}

#[test]
fn test_batch_dot_product() {
    let query = vec![1.0, 1.0, 1.0];
    let candidates: Vec<&[f32]> = vec![
        &[1.0, 1.0, 1.0][..],  // dot = 3.0
        &[2.0, 2.0, 2.0][..],  // dot = 6.0
        &[0.0, 0.0, 0.0][..],  // dot = 0.0
    ];

    let results = batch_dot_product_gpu(&query, &candidates);

    assert_eq!(results.len(), 3);
    assert!((results[0] - 3.0).abs() < 1e-6);
    assert!((results[1] - 6.0).abs() < 1e-6);
    assert!(results[2].abs() < 1e-6);
}

#[test]
fn test_batch_euclidean() {
    let query = vec![0.0, 0.0, 0.0];
    let candidates: Vec<&[f32]> = vec![
        &[3.0, 4.0, 0.0][..],  // dist = 5.0
        &[1.0, 0.0, 0.0][..],  // dist = 1.0
        &[0.0, 0.0, 0.0][..],  // dist = 0.0
    ];

    let results = batch_euclidean_gpu(&query, &candidates);

    assert_eq!(results.len(), 3);
    assert!((results[0] - 5.0).abs() < 1e-6);
    assert!((results[1] - 1.0).abs() < 1e-6);
    assert!(results[2].abs() < 1e-6);
}

// ==================== Pooling Tests (using public API) ====================

#[test]
fn test_mean_pool_via_api() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let pooler = GpuPooler::new(&backend, &shaders).unwrap();

    // batch=2, seq=2, hidden=3
    let tokens = vec![
        1.0, 2.0, 3.0,  // batch 0, seq 0
        4.0, 5.0, 6.0,  // batch 0, seq 1
        7.0, 8.0, 9.0,  // batch 1, seq 0
        10.0, 11.0, 12.0, // batch 1, seq 1
    ];
    let mask = vec![1i64, 1, 1, 1];

    let result = pooler.mean_pool(&tokens, &mask, 2, 2, 3).unwrap();

    assert_eq!(result.len(), 6);
    // Batch 0: mean of [1,2,3] and [4,5,6] = [2.5, 3.5, 4.5]
    assert!((result[0] - 2.5).abs() < 1e-6);
    assert!((result[1] - 3.5).abs() < 1e-6);
    assert!((result[2] - 4.5).abs() < 1e-6);
}

#[test]
fn test_cls_pool_via_api() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let pooler = GpuPooler::new(&backend, &shaders).unwrap();

    // batch=2, seq=3, hidden=4
    let tokens = vec![
        // Batch 0
        1.0, 2.0, 3.0, 4.0,    // CLS token
        5.0, 6.0, 7.0, 8.0,
        9.0, 10.0, 11.0, 12.0,
        // Batch 1
        10.0, 20.0, 30.0, 40.0, // CLS token
        50.0, 60.0, 70.0, 80.0,
        90.0, 100.0, 110.0, 120.0,
    ];

    let result = pooler.cls_pool(&tokens, 2, 4).unwrap();

    assert_eq!(result.len(), 8);

    // Batch 0: first token
    assert!((result[0] - 1.0).abs() < 1e-6);
    assert!((result[1] - 2.0).abs() < 1e-6);
    assert!((result[2] - 3.0).abs() < 1e-6);
    assert!((result[3] - 4.0).abs() < 1e-6);

    // Batch 1: first token
    assert!((result[4] - 10.0).abs() < 1e-6);
    assert!((result[5] - 20.0).abs() < 1e-6);
    assert!((result[6] - 30.0).abs() < 1e-6);
    assert!((result[7] - 40.0).abs() < 1e-6);
}

#[test]
fn test_max_pool_via_api() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let pooler = GpuPooler::new(&backend, &shaders).unwrap();

    // batch=1, seq=3, hidden=4
    let tokens = vec![
        1.0, 10.0, 3.0, 4.0,   // seq 0
        5.0, 2.0, 7.0, 8.0,    // seq 1
        9.0, 6.0, 11.0, 0.0,   // seq 2
    ];

    let mask = vec![1i64, 1, 1];

    let result = pooler.max_pool(&tokens, &mask, 1, 3, 4).unwrap();

    assert_eq!(result.len(), 4);

    // Max across all sequences for each dimension
    assert!((result[0] - 9.0).abs() < 1e-6);  // max(1, 5, 9)
    assert!((result[1] - 10.0).abs() < 1e-6); // max(10, 2, 6)
    assert!((result[2] - 11.0).abs() < 1e-6); // max(3, 7, 11)
    assert!((result[3] - 8.0).abs() < 1e-6);  // max(4, 8, 0)
}

// ==================== Vector Operations Tests ====================

#[test]
fn test_normalize_batch() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let ops = GpuVectorOps::new(&backend, &shaders).unwrap();

    let mut vectors = vec![
        3.0, 4.0, 0.0,  // norm = 5, normalized = [0.6, 0.8, 0]
        0.0, 0.0, 5.0,  // norm = 5, normalized = [0, 0, 1]
    ];

    ops.normalize_batch(&mut vectors, 3).unwrap();

    // Check first vector
    assert!((vectors[0] - 0.6).abs() < 1e-6);
    assert!((vectors[1] - 0.8).abs() < 1e-6);
    assert!(vectors[2].abs() < 1e-6);

    // Check second vector
    assert!(vectors[3].abs() < 1e-6);
    assert!(vectors[4].abs() < 1e-6);
    assert!((vectors[5] - 1.0).abs() < 1e-6);
}

#[test]
fn test_matmul() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let ops = GpuVectorOps::new(&backend, &shaders).unwrap();

    // 2x3 matrix
    let matrix = vec![
        1.0, 2.0, 3.0,
        4.0, 5.0, 6.0,
    ];

    // 3x1 vector
    let vector = vec![1.0, 1.0, 1.0];

    let result = ops.matmul(&matrix, &vector, 2, 3).unwrap();

    assert_eq!(result.len(), 2);
    assert!((result[0] - 6.0).abs() < 1e-6);  // 1+2+3
    assert!((result[1] - 15.0).abs() < 1e-6); // 4+5+6
}

#[test]
fn test_batch_add() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let ops = GpuVectorOps::new(&backend, &shaders).unwrap();

    let a = vec![1.0, 2.0, 3.0, 4.0];
    let b = vec![5.0, 6.0, 7.0, 8.0];

    let result = ops.batch_add(&a, &b).unwrap();

    assert_eq!(result, vec![6.0, 8.0, 10.0, 12.0]);
}

#[test]
fn test_batch_scale() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let ops = GpuVectorOps::new(&backend, &shaders).unwrap();

    let mut vectors = vec![1.0, 2.0, 3.0, 4.0];

    ops.batch_scale(&mut vectors, 2.0).unwrap();

    assert_eq!(vectors, vec![2.0, 4.0, 6.0, 8.0]);
}

// ==================== Integration Tests ====================

#[test]
fn test_gpu_similarity_with_backend() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let similarity = GpuSimilarity::new(&backend, &shaders).unwrap();

    let query = vec![1.0, 0.0, 0.0];
    let candidates: Vec<&[f32]> = vec![
        &[1.0, 0.0, 0.0][..],
        &[0.0, 1.0, 0.0][..],
    ];

    let results = similarity.batch_cosine(&query, &candidates).unwrap();

    assert_eq!(results.len(), 2);
    assert!((results[0] - 1.0).abs() < 1e-6);
}

#[test]
fn test_top_k_similar() {
    let backend = CpuBackend;
    let shaders = ShaderRegistry::new();
    let similarity = GpuSimilarity::new(&backend, &shaders).unwrap();

    let query = vec![1.0, 0.0, 0.0];
    let candidates: Vec<&[f32]> = vec![
        &[0.0, 1.0, 0.0][..],   // sim = 0
        &[1.0, 0.0, 0.0][..],   // sim = 1 (best)
        &[0.5, 0.5, 0.0][..],   // sim ≈ 0.707
        &[-1.0, 0.0, 0.0][..],  // sim = -1 (worst)
    ];

    let top2 = similarity.top_k(&query, &candidates, 2).unwrap();

    assert_eq!(top2.len(), 2);
    assert_eq!(top2[0].0, 1); // Index of [1,0,0]
    assert_eq!(top2[1].0, 2); // Index of [0.5,0.5,0]
}

// ==================== Memory Stats Tests ====================

#[test]
fn test_memory_stats() {
    let stats = GpuMemoryStats {
        total: 1024 * 1024 * 1024, // 1GB
        used: 512 * 1024 * 1024,   // 512MB
        free: 512 * 1024 * 1024,
        peak: 768 * 1024 * 1024,
    };

    assert!((stats.usage_percent() - 50.0).abs() < 0.1);
}

#[test]
fn test_empty_memory_stats() {
    let stats = GpuMemoryStats::default();
    assert_eq!(stats.usage_percent(), 0.0);
}

// ==================== Backend Tests ====================

#[test]
fn test_cpu_backend_info() {
    let backend = CpuBackend;

    assert!(backend.is_available());

    let info = backend.device_info();
    assert_eq!(info.backend, "CPU");
    assert!(!info.supports_compute);
}
