//! # PostgreSQL Attention Operators
//!
//! SQL-callable functions for attention mechanisms in PostgreSQL.

use super::{
    softmax, Attention, AttentionType, FlashAttention, MultiHeadAttention, ScaledDotAttention,
};
use pgrx::prelude::*;
use pgrx::JsonB;

/// Compute attention score between query and key vectors
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_attention_score(
///     ARRAY[1.0, 0.0, 0.0]::float4[],
///     ARRAY[1.0, 0.0, 0.0]::float4[],
///     'scaled_dot'
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_attention_score(
    query: Vec<f32>,
    key: Vec<f32>,
    attention_type: default!(&str, "'scaled_dot'"),
) -> f32 {
    // Parse attention type
    let attn_type = attention_type
        .parse::<AttentionType>()
        .unwrap_or(AttentionType::ScaledDot);

    // Validate dimensions
    if query.is_empty() || key.is_empty() {
        return 0.0;
    }

    if query.len() != key.len() {
        pgrx::error!(
            "Query and key dimensions must match: {} vs {}",
            query.len(),
            key.len()
        );
    }

    // Create attention mechanism
    let attention: Box<dyn Attention> = match attn_type {
        AttentionType::ScaledDot => Box::new(ScaledDotAttention::new(query.len())),
        AttentionType::FlashV2 => Box::new(FlashAttention::with_head_dim(query.len())),
        _ => Box::new(ScaledDotAttention::new(query.len())),
    };

    // Compute attention score
    let keys = vec![&key[..]];
    let scores = attention.attention_scores(&query, &keys);

    scores.first().copied().unwrap_or(0.0)
}

/// Apply softmax to an array of scores
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_softmax(ARRAY[1.0, 2.0, 3.0]::float4[]);
/// -- Returns: {0.09, 0.24, 0.67}
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_softmax(scores: Vec<f32>) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }

    softmax(&scores)
}

/// Compute multi-head attention between query and multiple keys
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_multi_head_attention(
///     ARRAY[1.0, 0.0, 0.0, 0.0]::float4[],  -- query
///     '[[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]]'::jsonb,  -- keys
///     '[[1.0, 2.0], [3.0, 4.0]]'::jsonb,  -- values
///     2  -- num_heads
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_multi_head_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    num_heads: default!(i32, 4),
) -> Vec<f32> {
    // Parse keys and values from JSON
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    // Validate inputs
    if query.is_empty() || keys.is_empty() || values.is_empty() {
        return Vec::new();
    }

    if keys.len() != values.len() {
        pgrx::error!(
            "Keys and values must have same length: {} vs {}",
            keys.len(),
            values.len()
        );
    }

    let num_heads = num_heads.max(1) as usize;
    let total_dim = query.len();

    // Check dimension compatibility
    if total_dim % num_heads != 0 {
        pgrx::error!(
            "Query dimension {} must be divisible by num_heads {}",
            total_dim,
            num_heads
        );
    }

    // Validate all keys have same dimension
    for (i, key) in keys.iter().enumerate() {
        if key.len() != total_dim {
            pgrx::error!(
                "Key {} has dimension {} but expected {}",
                i,
                key.len(),
                total_dim
            );
        }
    }

    // Create multi-head attention
    let mha = MultiHeadAttention::new(num_heads, total_dim);

    // Convert to slice references
    let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
    let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

    // Compute attention
    mha.forward(&query, &key_refs, &value_refs)
}

/// Compute Flash Attention v2 (memory-efficient)
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_flash_attention(
///     ARRAY[1.0, 0.0, 0.0, 0.0]::float4[],
///     '[[1.0, 0.0, 0.0, 0.0]]'::jsonb,
///     '[[5.0, 10.0]]'::jsonb,
///     64  -- block_size
/// );
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_flash_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    block_size: default!(i32, 64),
) -> Vec<f32> {
    // Parse keys and values from JSON
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    // Validate inputs
    if query.is_empty() || keys.is_empty() || values.is_empty() {
        return Vec::new();
    }

    if keys.len() != values.len() {
        pgrx::error!("Keys and values must have same length");
    }

    let block_size = block_size.max(1) as usize;

    // Create Flash Attention
    let flash = FlashAttention::new(query.len(), block_size);

    // Convert to slice references
    let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
    let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

    // Compute attention
    flash.forward(&query, &key_refs, &value_refs)
}

/// Get information about available attention types
///
/// # SQL Example
/// ```sql
/// SELECT * FROM ruvector_attention_types();
/// ```
#[pg_extern]
pub fn ruvector_attention_types() -> TableIterator<
    'static,
    (
        name!(name, String),
        name!(complexity, String),
        name!(best_for, String),
    ),
> {
    let types = vec![
        AttentionType::ScaledDot,
        AttentionType::MultiHead,
        AttentionType::FlashV2,
        AttentionType::Linear,
        AttentionType::Gat,
        AttentionType::Sparse,
        AttentionType::Moe,
        AttentionType::Cross,
        AttentionType::Sliding,
        AttentionType::Poincare,
    ];

    TableIterator::new(types.into_iter().map(|t| {
        (
            t.name().to_string(),
            t.complexity().to_string(),
            t.best_for().to_string(),
        )
    }))
}

/// Compute attention scores between a query and multiple keys
///
/// # SQL Example
/// ```sql
/// SELECT ruvector_attention_scores(
///     ARRAY[1.0, 0.0, 0.0]::float4[],
///     '[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]'::jsonb
/// );
/// -- Returns array of attention scores
/// ```
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_attention_scores(
    query: Vec<f32>,
    keys_json: JsonB,
    attention_type: default!(&str, "'scaled_dot'"),
) -> Vec<f32> {
    // Parse keys from JSON
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() {
        return Vec::new();
    }

    // Parse attention type
    let attn_type = attention_type
        .parse::<AttentionType>()
        .unwrap_or(AttentionType::ScaledDot);

    // Create attention mechanism
    let attention: Box<dyn Attention> = match attn_type {
        AttentionType::ScaledDot => Box::new(ScaledDotAttention::new(query.len())),
        AttentionType::FlashV2 => Box::new(FlashAttention::with_head_dim(query.len())),
        _ => Box::new(ScaledDotAttention::new(query.len())),
    };

    // Convert to slice references
    let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();

    // Compute attention scores
    attention.attention_scores(&query, &key_refs)
}

// ============================================================================
// Extended Attention Functions (feature-gated: attention-extended)
// ============================================================================

/// Linear attention: O(n) complexity using kernel feature maps.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_linear_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
) -> Vec<f32> {
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let val_dim = values[0].len();
    // Linear attention: phi(q)^T * (sum phi(k_i) * v_i^T) / (phi(q)^T * sum phi(k_i))
    // Using ELU+1 as kernel feature map
    let phi = |x: &[f32]| -> Vec<f32> {
        x.iter()
            .map(|&v| if v >= 0.0 { v + 1.0 } else { v.exp() })
            .collect()
    };

    let phi_q = phi(&query);

    // Compute KV = sum phi(k_i) * v_i^T and K_sum = sum phi(k_i)
    let key_dim = phi_q.len();
    let mut kv = vec![0.0f32; key_dim * val_dim];
    let mut k_sum = vec![0.0f32; key_dim];

    for (key, val) in keys.iter().zip(values.iter()) {
        let phi_k = phi(key);
        for j in 0..key_dim {
            k_sum[j] += phi_k[j];
            for d in 0..val_dim {
                kv[j * val_dim + d] += phi_k[j] * val[d];
            }
        }
    }

    // result = (phi_q^T * KV) / (phi_q^T * k_sum)
    let mut result = vec![0.0f32; val_dim];
    let mut normalizer = 0.0f32;
    for j in 0..key_dim {
        normalizer += phi_q[j] * k_sum[j];
        for d in 0..val_dim {
            result[d] += phi_q[j] * kv[j * val_dim + d];
        }
    }

    if normalizer > 1e-8 {
        for d in 0..val_dim {
            result[d] /= normalizer;
        }
    }

    result
}

/// Sliding window attention with local context.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sliding_window_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    window_size: default!(i32, 256),
) -> Vec<f32> {
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let w = (window_size as usize).min(keys.len());
    // Take last `w` keys/values (sliding window)
    let start = if keys.len() > w { keys.len() - w } else { 0 };

    let window_keys = &keys[start..];
    let window_values = &values[start..];

    // Scaled dot-product attention on window
    let dim = query.len() as f32;
    let scale = dim.sqrt();

    let mut scores: Vec<f32> = window_keys
        .iter()
        .map(|k| {
            query
                .iter()
                .zip(k.iter())
                .map(|(&q, &k)| q * k)
                .sum::<f32>()
                / scale
        })
        .collect();

    // Softmax
    let max_score = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f32 = scores
        .iter_mut()
        .map(|s| {
            *s = (*s - max_score).exp();
            *s
        })
        .sum();
    if exp_sum > 0.0 {
        for s in &mut scores {
            *s /= exp_sum;
        }
    }

    // Weighted sum
    let val_dim = window_values[0].len();
    let mut result = vec![0.0f32; val_dim];
    for (score, val) in scores.iter().zip(window_values.iter()) {
        for (r, v) in result.iter_mut().zip(val.iter()) {
            *r += score * v;
        }
    }

    result
}

/// Cross-attention between query from one source and keys/values from another.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_cross_attention(
    query: Vec<f32>,
    ctx_keys_json: JsonB,
    ctx_values_json: JsonB,
) -> Vec<f32> {
    let attention = ScaledDotAttention::new(query.len());

    let keys: Vec<Vec<f32>> = match ctx_keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match ctx_values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
    let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

    attention.forward(&query, &key_refs, &value_refs)
}

/// Sparse top-k attention.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_sparse_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    top_k: default!(i32, 8),
) -> Vec<f32> {
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let dim = query.len() as f32;
    let scale = dim.sqrt();

    // Compute scores
    let mut scored: Vec<(usize, f32)> = keys
        .iter()
        .enumerate()
        .map(|(i, k)| {
            let score: f32 = query
                .iter()
                .zip(k.iter())
                .map(|(&q, &k)| q * k)
                .sum::<f32>()
                / scale;
            (i, score)
        })
        .collect();

    // Sort by score descending and take top-k
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let k = (top_k as usize).min(scored.len());
    let top = &scored[..k];

    // Softmax on top-k scores
    let max_s = top
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = top.iter().map(|(_, s)| (s - max_s).exp()).collect();
    let sum: f32 = exps.iter().sum();

    let val_dim = values[0].len();
    let mut result = vec![0.0f32; val_dim];
    for (exp_score, &(idx, _)) in exps.iter().zip(top.iter()) {
        let weight = if sum > 0.0 { exp_score / sum } else { 0.0 };
        for (r, v) in result.iter_mut().zip(values[idx].iter()) {
            *r += weight * v;
        }
    }

    result
}

/// Mixture-of-Experts attention with routing.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_moe_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    n_experts: default!(i32, 4),
    top_k: default!(i32, 2),
) -> Vec<f32> {
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let n = n_experts.max(1) as usize;
    let k = (top_k as usize).min(n);

    // Partition keys/values into n_experts groups
    let group_size = (keys.len() + n - 1) / n;

    // Router: compute gating scores for each expert based on query similarity
    let mut expert_scores: Vec<(usize, f32)> = (0..n)
        .map(|expert_idx| {
            let start = expert_idx * group_size;
            let end = (start + group_size).min(keys.len());
            if start >= keys.len() {
                return (expert_idx, f32::NEG_INFINITY);
            }
            // Average similarity with expert's keys
            let score: f32 = keys[start..end]
                .iter()
                .map(|key| {
                    query
                        .iter()
                        .zip(key.iter())
                        .map(|(&q, &k)| q * k)
                        .sum::<f32>()
                })
                .sum::<f32>()
                / (end - start) as f32;
            (expert_idx, score)
        })
        .collect();

    expert_scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Softmax on top-k expert scores
    let top_experts = &expert_scores[..k.min(expert_scores.len())];
    let max_s = top_experts
        .iter()
        .map(|(_, s)| *s)
        .fold(f32::NEG_INFINITY, f32::max);
    let exps: Vec<f32> = top_experts.iter().map(|(_, s)| (s - max_s).exp()).collect();
    let sum: f32 = exps.iter().sum();

    let val_dim = values[0].len();
    let mut result = vec![0.0f32; val_dim];

    for (weight_unnorm, &(expert_idx, _)) in exps.iter().zip(top_experts.iter()) {
        let weight = if sum > 0.0 { weight_unnorm / sum } else { 0.0 };
        let start = expert_idx * group_size;
        let end = (start + group_size).min(keys.len());

        if start >= keys.len() {
            continue;
        }

        // Run scaled dot-product attention within this expert's partition
        let expert_keys = &keys[start..end];
        let expert_values = &values[start..end];

        let attention = ScaledDotAttention::new(query.len());
        let key_refs: Vec<&[f32]> = expert_keys.iter().map(|k| &k[..]).collect();
        let value_refs: Vec<&[f32]> = expert_values.iter().map(|v| &v[..]).collect();
        let expert_result = attention.forward(&query, &key_refs, &value_refs);

        for (r, v) in result.iter_mut().zip(expert_result.iter()) {
            *r += weight * v;
        }
    }

    result
}

/// Hyperbolic (Poincare ball) attention.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_hyperbolic_attention(
    query: Vec<f32>,
    keys_json: JsonB,
    values_json: JsonB,
    curvature: default!(f32, 1.0),
) -> Vec<f32> {
    let keys: Vec<Vec<f32>> = match keys_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    let values: Vec<Vec<f32>> = match values_json.0.as_array() {
        Some(arr) => arr
            .iter()
            .filter_map(|v| {
                v.as_array().map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_f64().map(|f| f as f32))
                        .collect()
                })
            })
            .collect(),
        None => return Vec::new(),
    };

    if query.is_empty() || keys.is_empty() || values.is_empty() || keys.len() != values.len() {
        return Vec::new();
    }

    let c = curvature.max(1e-6) as f64;

    // Poincare distance: d(x, y) = (1/sqrt(c)) * acosh(1 + 2c * ||x-y||^2 / ((1-c*||x||^2)(1-c*||y||^2)))
    let poincare_dist = |a: &[f32], b: &[f32]| -> f64 {
        let norm_a_sq: f64 = a.iter().map(|&x| (x as f64).powi(2)).sum();
        let norm_b_sq: f64 = b.iter().map(|&x| (x as f64).powi(2)).sum();
        let diff_sq: f64 = a
            .iter()
            .zip(b.iter())
            .map(|(&x, &y)| ((x as f64) - (y as f64)).powi(2))
            .sum();

        let denom = (1.0 - c * norm_a_sq).max(1e-8) * (1.0 - c * norm_b_sq).max(1e-8);
        let arg = 1.0 + 2.0 * c * diff_sq / denom;
        (1.0 / c.sqrt()) * arg.max(1.0).acosh()
    };

    // Compute attention scores as negative distances
    let mut scores: Vec<f32> = keys
        .iter()
        .map(|k| -poincare_dist(&query, k) as f32)
        .collect();

    // Softmax
    let max_s = scores.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let exp_sum: f32 = scores
        .iter_mut()
        .map(|s| {
            *s = (*s - max_s).exp();
            *s
        })
        .sum();
    if exp_sum > 0.0 {
        for s in &mut scores {
            *s /= exp_sum;
        }
    }

    // Weighted sum in tangent space
    let val_dim = values[0].len();
    let mut result = vec![0.0f32; val_dim];
    for (score, val) in scores.iter().zip(values.iter()) {
        for (r, v) in result.iter_mut().zip(val.iter()) {
            *r += score * v;
        }
    }

    result
}

/// Benchmark attention mechanisms.
#[cfg(feature = "attention-extended")]
#[pg_extern(immutable, parallel_safe)]
pub fn ruvector_attention_benchmark(
    dim: default!(i32, 64),
    seq_len: default!(i32, 128),
    attention_type: default!(&str, "'scaled_dot'"),
) -> JsonB {
    use std::time::Instant;

    let d = dim.max(1) as usize;
    let n = seq_len.max(1) as usize;

    // Generate random data
    let query: Vec<f32> = (0..d).map(|i| ((i as f32 * 0.1).sin())).collect();
    let keys: Vec<Vec<f32>> = (0..n)
        .map(|j| (0..d).map(|i| ((i + j) as f32 * 0.1).cos()).collect())
        .collect();
    let values: Vec<Vec<f32>> = (0..n)
        .map(|j| (0..d).map(|i| ((i + j) as f32 * 0.05).sin()).collect())
        .collect();

    let key_refs: Vec<&[f32]> = keys.iter().map(|k| &k[..]).collect();
    let value_refs: Vec<&[f32]> = values.iter().map(|v| &v[..]).collect();

    let iterations = 100;
    let start = Instant::now();

    let attn_type = attention_type
        .parse::<AttentionType>()
        .unwrap_or(AttentionType::ScaledDot);

    let attention: Box<dyn Attention> = match attn_type {
        AttentionType::FlashV2 => Box::new(FlashAttention::new(d, 64)),
        AttentionType::MultiHead => Box::new(MultiHeadAttention::new(4.max(1), d)),
        _ => Box::new(ScaledDotAttention::new(d)),
    };

    for _ in 0..iterations {
        let _ = attention.forward(&query, &key_refs, &value_refs);
    }

    let elapsed = start.elapsed();
    let avg_us = elapsed.as_micros() as f64 / iterations as f64;

    JsonB(serde_json::json!({
        "attention_type": attention_type,
        "dim": d,
        "seq_len": n,
        "iterations": iterations,
        "avg_latency_us": avg_us,
        "throughput_ops_per_sec": 1_000_000.0 / avg_us,
        "total_time_ms": elapsed.as_millis(),
    }))
}

#[cfg(feature = "pg_test")]
#[pgrx::pg_schema]
mod tests {
    use super::*;

    // Helper to convert Vec<Vec<f32>> to JsonB for tests
    fn to_json(data: Vec<Vec<f32>>) -> JsonB {
        JsonB(serde_json::json!(data))
    }

    #[pg_test]
    fn test_ruvector_attention_score() {
        let query = vec![1.0, 0.0, 0.0];
        let key = vec![1.0, 0.0, 0.0];

        let score = ruvector_attention_score(query, key, "scaled_dot");

        // Perfect match should give high score (after softmax, it would be 1.0)
        assert!(score > 0.99);
    }

    #[pg_test]
    fn test_ruvector_softmax() {
        let scores = vec![1.0, 2.0, 3.0];
        let result = ruvector_softmax(scores);

        assert_eq!(result.len(), 3);

        // Should sum to 1
        let sum: f32 = result.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        // Higher input should have higher output
        assert!(result[2] > result[1]);
        assert!(result[1] > result[0]);
    }

    #[pg_test]
    fn test_ruvector_multi_head_attention() {
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let keys = to_json(vec![vec![1.0, 0.0, 0.0, 0.0], vec![0.0, 1.0, 0.0, 0.0]]);
        let values = to_json(vec![vec![1.0, 2.0], vec![3.0, 4.0]]);

        let result = ruvector_multi_head_attention(query, keys, values, 2);

        assert_eq!(result.len(), 2);
        // Should be closer to first value
        assert!(result[0] < 2.0);
    }

    #[pg_test]
    fn test_ruvector_flash_attention() {
        let query = vec![1.0, 0.0, 0.0, 0.0];
        let keys = to_json(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let values = to_json(vec![vec![5.0, 10.0]]);

        let result = ruvector_flash_attention(query, keys, values, 64);

        assert_eq!(result.len(), 2);
        assert!((result[0] - 5.0).abs() < 0.01);
        assert!((result[1] - 10.0).abs() < 0.01);
    }

    #[pg_test]
    fn test_ruvector_attention_scores() {
        let query = vec![1.0, 0.0, 0.0];
        let keys = to_json(vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ]);

        let scores = ruvector_attention_scores(query, keys, "scaled_dot");

        assert_eq!(scores.len(), 3);

        // Should sum to 1 (softmax)
        let sum: f32 = scores.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        // First key matches best
        assert!(scores[0] > scores[1]);
        assert!(scores[0] > scores[2]);
    }

    #[pg_test]
    fn test_ruvector_attention_types_query() {
        // This would be run as SQL: SELECT * FROM ruvector_attention_types();
        // Testing that the function doesn't panic
        let types = ruvector_attention_types();
        let results: Vec<_> = types.collect();

        // Should have multiple attention types
        assert!(results.len() >= 5);
    }
}
