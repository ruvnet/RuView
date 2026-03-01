//! Integration test for ruvector-attention crate from crates.io
//!
//! This tests all attention mechanisms from the published crate

use ruvector_attention::{
    attention::{ScaledDotProductAttention, MultiHeadAttention},
    sparse::{LocalGlobalAttention, LinearAttention, FlashAttention},
    hyperbolic::{HyperbolicAttention, HyperbolicAttentionConfig},
    moe::{MoEAttention, MoEConfig},
    graph::{GraphAttention, GraphAttentionConfig},
    traits::Attention,
};

fn main() {
    println!("=== ruvector-attention Crate Integration Tests ===\n");

    test_scaled_dot_product_attention();
    test_multi_head_attention();
    test_hyperbolic_attention();
    test_linear_attention();
    test_flash_attention();
    test_local_global_attention();
    test_moe_attention();
    test_graph_attention();

    println!("\n✅ All Rust crate tests passed!\n");
}

fn test_scaled_dot_product_attention() {
    let dim = 64;
    let attention = ScaledDotProductAttention::new(dim);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..3).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..3).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Scaled dot-product attention works correctly");
}

fn test_multi_head_attention() {
    let dim = 64;
    let num_heads = 8;
    let attention = MultiHeadAttention::new(dim, num_heads);

    assert_eq!(attention.dim(), dim);
    assert_eq!(attention.num_heads(), num_heads);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Multi-head attention works correctly");
}

fn test_hyperbolic_attention() {
    let dim = 64;
    let config = HyperbolicAttentionConfig {
        dim,
        curvature: 1.0,
        ..Default::default()
    };
    let attention = HyperbolicAttention::new(config);

    let query: Vec<f32> = vec![0.1; dim];
    let keys: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>() * 0.1).collect()).collect();
    let values: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Hyperbolic attention works correctly");
}

fn test_linear_attention() {
    let dim = 64;
    let num_features = 128;
    let attention = LinearAttention::new(dim, num_features);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Linear attention works correctly");
}

fn test_flash_attention() {
    let dim = 64;
    let block_size = 16;
    let attention = FlashAttention::new(dim, block_size);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Flash attention works correctly");
}

fn test_local_global_attention() {
    let dim = 64;
    let local_window = 4;
    let global_tokens = 2;
    let attention = LocalGlobalAttention::new(dim, local_window, global_tokens);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..4).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..4).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Local-global attention works correctly");
}

fn test_moe_attention() {
    let dim = 64;
    let config = MoEConfig::builder()
        .dim(dim)
        .num_experts(4)
        .top_k(2)
        .build();
    let attention = MoEAttention::new(config);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..2).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ MoE attention works correctly");
}

fn test_graph_attention() {
    let dim = 64;
    let config = GraphAttentionConfig {
        dim,
        num_heads: 4,
        ..Default::default()
    };
    let attention = GraphAttention::new(config);

    let query: Vec<f32> = vec![0.5; dim];
    let keys: Vec<Vec<f32>> = (0..3).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();
    let values: Vec<Vec<f32>> = (0..3).map(|_| (0..dim).map(|_| rand::random::<f32>()).collect()).collect();

    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    let result = attention.compute(&query, &keys_refs, &values_refs).unwrap();
    assert_eq!(result.len(), dim);
    println!("  ✓ Graph attention works correctly");
}
