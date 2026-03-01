//! GPU Compute Shaders for RuVector Operations
//!
//! WGSL (WebGPU Shading Language) implementations for:
//! - Pooling operations
//! - Similarity computations
//! - Vector normalization
//! - Matrix operations

use std::collections::HashMap;

/// Shader registry for managing compute shaders
#[derive(Debug)]
pub struct ShaderRegistry {
    shaders: HashMap<String, ShaderModule>,
}

/// Shader module information
#[derive(Debug, Clone)]
pub struct ShaderModule {
    /// Shader name
    pub name: String,
    /// WGSL source code
    pub source: String,
    /// Entry point function
    pub entry_point: String,
    /// Default workgroup size
    pub workgroup_size: [u32; 3],
}

impl ShaderRegistry {
    /// Create new registry with built-in shaders
    pub fn new() -> Self {
        let mut shaders = HashMap::new();

        // Register all built-in shaders
        for shader in Self::builtin_shaders() {
            shaders.insert(shader.name.clone(), shader);
        }

        Self { shaders }
    }

    /// Get shader by name
    pub fn get(&self, name: &str) -> Option<&ShaderModule> {
        self.shaders.get(name)
    }

    /// Register custom shader
    pub fn register(&mut self, shader: ShaderModule) {
        self.shaders.insert(shader.name.clone(), shader);
    }

    /// List all available shaders
    pub fn list(&self) -> Vec<&str> {
        self.shaders.keys().map(|s| s.as_str()).collect()
    }

    /// Get built-in shader definitions
    fn builtin_shaders() -> Vec<ShaderModule> {
        vec![
            // Cosine Similarity
            ShaderModule {
                name: "cosine_similarity".to_string(),
                source: SHADER_COSINE_SIMILARITY.to_string(),
                entry_point: "cosine_similarity".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // Batch Cosine Similarity
            ShaderModule {
                name: "batch_cosine_similarity".to_string(),
                source: SHADER_BATCH_COSINE_SIMILARITY.to_string(),
                entry_point: "batch_cosine_similarity".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // Dot Product
            ShaderModule {
                name: "dot_product".to_string(),
                source: SHADER_DOT_PRODUCT.to_string(),
                entry_point: "dot_product".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // Euclidean Distance
            ShaderModule {
                name: "euclidean_distance".to_string(),
                source: SHADER_EUCLIDEAN_DISTANCE.to_string(),
                entry_point: "euclidean_distance".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // L2 Normalize
            ShaderModule {
                name: "l2_normalize".to_string(),
                source: SHADER_L2_NORMALIZE.to_string(),
                entry_point: "l2_normalize".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // Mean Pooling
            ShaderModule {
                name: "mean_pool".to_string(),
                source: SHADER_MEAN_POOL.to_string(),
                entry_point: "mean_pool".to_string(),
                workgroup_size: [64, 1, 1],
            },
            // Max Pooling
            ShaderModule {
                name: "max_pool".to_string(),
                source: SHADER_MAX_POOL.to_string(),
                entry_point: "max_pool".to_string(),
                workgroup_size: [64, 1, 1],
            },
            // CLS Pooling
            ShaderModule {
                name: "cls_pool".to_string(),
                source: SHADER_CLS_POOL.to_string(),
                entry_point: "cls_pool".to_string(),
                workgroup_size: [64, 1, 1],
            },
            // Matrix-Vector Multiplication
            ShaderModule {
                name: "matmul".to_string(),
                source: SHADER_MATMUL.to_string(),
                entry_point: "matmul".to_string(),
                workgroup_size: [16, 16, 1],
            },
            // Vector Addition
            ShaderModule {
                name: "vector_add".to_string(),
                source: SHADER_VECTOR_ADD.to_string(),
                entry_point: "vector_add".to_string(),
                workgroup_size: [256, 1, 1],
            },
            // Vector Scale
            ShaderModule {
                name: "vector_scale".to_string(),
                source: SHADER_VECTOR_SCALE.to_string(),
                entry_point: "vector_scale".to_string(),
                workgroup_size: [256, 1, 1],
            },
        ]
    }
}

impl Default for ShaderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Shader Source Code ====================

// Public aliases for operations.rs
pub const MEAN_POOL_SHADER: &str = SHADER_MEAN_POOL;
pub const MAX_POOL_SHADER: &str = SHADER_MAX_POOL;
pub const BATCH_COSINE_SIMILARITY_SHADER: &str = SHADER_BATCH_COSINE_SIMILARITY;
pub const DOT_PRODUCT_SHADER: &str = SHADER_DOT_PRODUCT;
pub const EUCLIDEAN_DISTANCE_SHADER: &str = SHADER_EUCLIDEAN_DISTANCE;
pub const L2_NORMALIZE_SHADER: &str = SHADER_L2_NORMALIZE;
pub const MATMUL_SHADER: &str = SHADER_MATMUL;
pub const VECTOR_ADD_SHADER: &str = SHADER_VECTOR_ADD;

/// Cosine similarity between two vectors
pub const SHADER_COSINE_SIMILARITY: &str = r#"
struct Params {
    dimension: u32,
    count: u32,
}

@group(0) @binding(0) var<storage, read> query: array<f32>;
@group(0) @binding(1) var<storage, read> candidate: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

var<workgroup> shared_dot: array<f32, 256>;
var<workgroup> shared_norm_a: array<f32, 256>;
var<workgroup> shared_norm_b: array<f32, 256>;

@compute @workgroup_size(256)
fn cosine_similarity(@builtin(global_invocation_id) gid: vec3<u32>,
                     @builtin(local_invocation_id) lid: vec3<u32>) {
    let idx = gid.x;
    let local_idx = lid.x;

    var dot: f32 = 0.0;
    var norm_a: f32 = 0.0;
    var norm_b: f32 = 0.0;

    // Compute partial sums
    var i = local_idx;
    while (i < params.dimension) {
        let a = query[i];
        let b = candidate[i];
        dot += a * b;
        norm_a += a * a;
        norm_b += b * b;
        i += 256u;
    }

    // Store in shared memory
    shared_dot[local_idx] = dot;
    shared_norm_a[local_idx] = norm_a;
    shared_norm_b[local_idx] = norm_b;
    workgroupBarrier();

    // Reduction
    for (var stride = 128u; stride > 0u; stride >>= 1u) {
        if (local_idx < stride) {
            shared_dot[local_idx] += shared_dot[local_idx + stride];
            shared_norm_a[local_idx] += shared_norm_a[local_idx + stride];
            shared_norm_b[local_idx] += shared_norm_b[local_idx + stride];
        }
        workgroupBarrier();
    }

    // Write result
    if (local_idx == 0u) {
        let norm_product = sqrt(shared_norm_a[0] * shared_norm_b[0]);
        if (norm_product > 1e-12) {
            result[0] = shared_dot[0] / norm_product;
        } else {
            result[0] = 0.0;
        }
    }
}
"#;

/// Batch cosine similarity - one query vs many candidates
pub const SHADER_BATCH_COSINE_SIMILARITY: &str = r#"
struct Params {
    dimension: u32,
    num_candidates: u32,
}

@group(0) @binding(0) var<storage, read> query: array<f32>;
@group(0) @binding(1) var<storage, read> candidates: array<f32>;
@group(0) @binding(2) var<storage, read_write> results: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn batch_cosine_similarity(@builtin(global_invocation_id) gid: vec3<u32>) {
    let candidate_idx = gid.x;

    if (candidate_idx >= params.num_candidates) {
        return;
    }

    let base = candidate_idx * params.dimension;

    var dot: f32 = 0.0;
    var norm_a: f32 = 0.0;
    var norm_b: f32 = 0.0;

    for (var i = 0u; i < params.dimension; i++) {
        let a = query[i];
        let b = candidates[base + i];
        dot += a * b;
        norm_a += a * a;
        norm_b += b * b;
    }

    let norm_product = sqrt(norm_a * norm_b);
    if (norm_product > 1e-12) {
        results[candidate_idx] = dot / norm_product;
    } else {
        results[candidate_idx] = 0.0;
    }
}
"#;

/// Dot product computation
pub const SHADER_DOT_PRODUCT: &str = r#"
struct Params {
    dimension: u32,
    num_candidates: u32,
}

@group(0) @binding(0) var<storage, read> query: array<f32>;
@group(0) @binding(1) var<storage, read> candidates: array<f32>;
@group(0) @binding(2) var<storage, read_write> results: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn dot_product(@builtin(global_invocation_id) gid: vec3<u32>) {
    let candidate_idx = gid.x;

    if (candidate_idx >= params.num_candidates) {
        return;
    }

    let base = candidate_idx * params.dimension;

    var dot: f32 = 0.0;
    for (var i = 0u; i < params.dimension; i++) {
        dot += query[i] * candidates[base + i];
    }

    results[candidate_idx] = dot;
}
"#;

/// Euclidean distance computation
pub const SHADER_EUCLIDEAN_DISTANCE: &str = r#"
struct Params {
    dimension: u32,
    num_candidates: u32,
}

@group(0) @binding(0) var<storage, read> query: array<f32>;
@group(0) @binding(1) var<storage, read> candidates: array<f32>;
@group(0) @binding(2) var<storage, read_write> results: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn euclidean_distance(@builtin(global_invocation_id) gid: vec3<u32>) {
    let candidate_idx = gid.x;

    if (candidate_idx >= params.num_candidates) {
        return;
    }

    let base = candidate_idx * params.dimension;

    var sum_sq: f32 = 0.0;
    for (var i = 0u; i < params.dimension; i++) {
        let diff = query[i] - candidates[base + i];
        sum_sq += diff * diff;
    }

    results[candidate_idx] = sqrt(sum_sq);
}
"#;

/// L2 normalization
pub const SHADER_L2_NORMALIZE: &str = r#"
struct Params {
    dimension: u32,
    num_vectors: u32,
}

@group(0) @binding(0) var<storage, read> input_vectors: array<f32>;
@group(0) @binding(1) var<storage, read> _dummy: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_vectors: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn l2_normalize(@builtin(global_invocation_id) gid: vec3<u32>) {
    let vec_idx = gid.x;

    if (vec_idx >= params.num_vectors) {
        return;
    }

    let base = vec_idx * params.dimension;

    // Compute norm
    var norm_sq: f32 = 0.0;
    for (var i = 0u; i < params.dimension; i++) {
        let val = input_vectors[base + i];
        norm_sq += val * val;
    }

    let norm = sqrt(norm_sq);

    // Normalize and write to output
    if (norm > 1e-12) {
        for (var i = 0u; i < params.dimension; i++) {
            output_vectors[base + i] = input_vectors[base + i] / norm;
        }
    } else {
        for (var i = 0u; i < params.dimension; i++) {
            output_vectors[base + i] = input_vectors[base + i];
        }
    }
}
"#;

/// Mean pooling over sequence
pub const SHADER_MEAN_POOL: &str = r#"
struct Params {
    batch_size: u32,
    seq_length: u32,
    hidden_size: u32,
}

@group(0) @binding(0) var<storage, read> tokens: array<f32>;
@group(0) @binding(1) var<storage, read> attention_mask: array<i32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(64)
fn mean_pool(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch_idx = gid.x / params.hidden_size;
    let hidden_idx = gid.x % params.hidden_size;

    if (batch_idx >= params.batch_size) {
        return;
    }

    let tokens_base = batch_idx * params.seq_length * params.hidden_size;
    let mask_base = batch_idx * params.seq_length;

    var sum: f32 = 0.0;
    var count: f32 = 0.0;

    for (var i = 0u; i < params.seq_length; i++) {
        if (attention_mask[mask_base + i] == 1) {
            sum += tokens[tokens_base + i * params.hidden_size + hidden_idx];
            count += 1.0;
        }
    }

    let out_idx = batch_idx * params.hidden_size + hidden_idx;
    if (count > 0.0) {
        output[out_idx] = sum / count;
    } else {
        output[out_idx] = 0.0;
    }
}
"#;

/// Max pooling over sequence
pub const SHADER_MAX_POOL: &str = r#"
struct Params {
    batch_size: u32,
    seq_length: u32,
    hidden_size: u32,
}

@group(0) @binding(0) var<storage, read> tokens: array<f32>;
@group(0) @binding(1) var<storage, read> attention_mask: array<i32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(64)
fn max_pool(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch_idx = gid.x / params.hidden_size;
    let hidden_idx = gid.x % params.hidden_size;

    if (batch_idx >= params.batch_size) {
        return;
    }

    let tokens_base = batch_idx * params.seq_length * params.hidden_size;
    let mask_base = batch_idx * params.seq_length;

    var max_val: f32 = -3.402823e+38; // -FLT_MAX
    var found: bool = false;

    for (var i = 0u; i < params.seq_length; i++) {
        if (attention_mask[mask_base + i] == 1) {
            let val = tokens[tokens_base + i * params.hidden_size + hidden_idx];
            if (!found || val > max_val) {
                max_val = val;
                found = true;
            }
        }
    }

    let out_idx = batch_idx * params.hidden_size + hidden_idx;
    output[out_idx] = select(0.0, max_val, found);
}
"#;

/// CLS token pooling (first token)
pub const SHADER_CLS_POOL: &str = r#"
struct Params {
    batch_size: u32,
    seq_length: u32,
    hidden_size: u32,
}

@group(0) @binding(0) var<storage, read> tokens: array<f32>;
@group(0) @binding(1) var<storage, read> _dummy: array<f32>;
@group(0) @binding(2) var<storage, read_write> output: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(64)
fn cls_pool(@builtin(global_invocation_id) gid: vec3<u32>) {
    let batch_idx = gid.x / params.hidden_size;
    let hidden_idx = gid.x % params.hidden_size;

    if (batch_idx >= params.batch_size) {
        return;
    }

    // CLS is first token
    let tokens_base = batch_idx * params.seq_length * params.hidden_size;
    let out_idx = batch_idx * params.hidden_size + hidden_idx;

    output[out_idx] = tokens[tokens_base + hidden_idx];
}
"#;

/// Matrix-vector multiplication
pub const SHADER_MATMUL: &str = r#"
struct Params {
    rows: u32,
    cols: u32,
}

@group(0) @binding(0) var<storage, read> matrix: array<f32>;
@group(0) @binding(1) var<storage, read> vector: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(16, 16)
fn matmul(@builtin(global_invocation_id) gid: vec3<u32>) {
    let row = gid.x;

    if (row >= params.rows) {
        return;
    }

    var sum: f32 = 0.0;
    for (var col = 0u; col < params.cols; col++) {
        sum += matrix[row * params.cols + col] * vector[col];
    }

    result[row] = sum;
}
"#;

/// Vector addition
pub const SHADER_VECTOR_ADD: &str = r#"
struct Params {
    length: u32,
}

@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> result: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn vector_add(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;

    if (idx >= params.length) {
        return;
    }

    result[idx] = a[idx] + b[idx];
}
"#;

/// Vector scaling
pub const SHADER_VECTOR_SCALE: &str = r#"
struct Params {
    length: u32,
    scale: f32,
}

@group(0) @binding(0) var<storage, read> input_vector: array<f32>;
@group(0) @binding(1) var<storage, read> _dummy: array<f32>;
@group(0) @binding(2) var<storage, read_write> output_vector: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn vector_scale(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;

    if (idx >= params.length) {
        return;
    }

    output_vector[idx] = input_vector[idx] * params.scale;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_registry() {
        let registry = ShaderRegistry::new();

        // Check all built-in shaders are registered
        assert!(registry.get("cosine_similarity").is_some());
        assert!(registry.get("batch_cosine_similarity").is_some());
        assert!(registry.get("dot_product").is_some());
        assert!(registry.get("euclidean_distance").is_some());
        assert!(registry.get("l2_normalize").is_some());
        assert!(registry.get("mean_pool").is_some());
        assert!(registry.get("max_pool").is_some());
        assert!(registry.get("cls_pool").is_some());
        assert!(registry.get("matmul").is_some());
        assert!(registry.get("vector_add").is_some());
        assert!(registry.get("vector_scale").is_some());
    }

    #[test]
    fn test_shader_content() {
        let registry = ShaderRegistry::new();

        let cosine = registry.get("cosine_similarity").unwrap();
        assert!(cosine.source.contains("@compute"));
        assert!(cosine.source.contains("workgroup_size"));
        assert_eq!(cosine.entry_point, "cosine_similarity");
    }

    #[test]
    fn test_custom_shader() {
        let mut registry = ShaderRegistry::new();

        registry.register(ShaderModule {
            name: "custom_op".to_string(),
            source: "// custom shader".to_string(),
            entry_point: "custom".to_string(),
            workgroup_size: [128, 1, 1],
        });

        assert!(registry.get("custom_op").is_some());
    }
}
