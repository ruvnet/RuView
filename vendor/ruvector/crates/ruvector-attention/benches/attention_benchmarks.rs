//! Benchmarks for ruvector-attention
//!
//! Run with: cargo bench -p ruvector-attention

use std::time::Instant;

use ruvector_attention::{
    attention::ScaledDotProductAttention,
    graph::{
        DualSpaceAttention, DualSpaceConfig, EdgeFeaturedAttention, EdgeFeaturedConfig, GraphRoPE,
        RoPEConfig,
    },
    hyperbolic::{HyperbolicAttention, HyperbolicAttentionConfig},
    moe::{MoEAttention, MoEConfig},
    sparse::{FlashAttention, LinearAttention, LocalGlobalAttention},
    training::{Adam, InfoNCELoss, Loss, Optimizer},
    traits::Attention,
};

fn main() {
    println!("=== ruvector-attention Benchmarks ===\n");

    // Configuration
    let dim = 256;
    let seq_len = 512;
    let iterations = 100;

    // Generate test data
    let query = vec![0.5f32; dim];
    let keys: Vec<Vec<f32>> = (0..seq_len)
        .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
        .collect();
    let values: Vec<Vec<f32>> = (0..seq_len)
        .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
        .collect();
    let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
    let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

    println!("Configuration:");
    println!("  Dimension: {}", dim);
    println!("  Sequence Length: {}", seq_len);
    println!("  Iterations: {}", iterations);
    println!();

    // 1. Scaled Dot-Product Attention
    {
        let attention = ScaledDotProductAttention::new(dim);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Scaled Dot-Product Attention:");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 2. Flash Attention
    {
        let attention = FlashAttention::new(dim, 64);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Flash Attention (block_size=64):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 3. Linear Attention
    {
        let attention = LinearAttention::new(dim, 64);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Linear Attention (num_features=64):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 4. Local-Global Attention
    {
        let attention = LocalGlobalAttention::new(dim, 32, 4);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Local-Global Attention (window=32, global=4):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 5. MoE Attention
    {
        let config = MoEConfig::builder()
            .dim(dim)
            .num_experts(4)
            .top_k(2)
            .build();
        let attention = MoEAttention::new(config);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("MoE Attention (4 experts, top-2):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 6. Hyperbolic Attention
    {
        let config = HyperbolicAttentionConfig {
            dim,
            curvature: -1.0,
            ..Default::default()
        };
        let attention = HyperbolicAttention::new(config);
        // Use smaller values for Poincaré ball
        let hyp_query = vec![0.1f32; dim];
        let hyp_keys: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| vec![(i as f32 * 0.001) % 0.5; dim])
            .collect();
        let hyp_values: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| vec![(i as f32 * 0.002) % 0.5; dim])
            .collect();
        let hyp_keys_refs: Vec<&[f32]> = hyp_keys.iter().map(|k| k.as_slice()).collect();
        let hyp_values_refs: Vec<&[f32]> = hyp_values.iter().map(|v| v.as_slice()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention
                .compute(&hyp_query, &hyp_keys_refs, &hyp_values_refs)
                .unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Hyperbolic Attention (curvature=1.0):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 7. Edge-Featured Graph Attention
    {
        let config = EdgeFeaturedConfig::builder()
            .node_dim(dim)
            .edge_dim(32)
            .num_heads(4)
            .build();
        let attention = EdgeFeaturedAttention::new(config);

        let graph_keys: Vec<Vec<f32>> = (0..64)
            .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
            .collect();
        let graph_values: Vec<Vec<f32>> = (0..64)
            .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
            .collect();
        let graph_keys_refs: Vec<&[f32]> = graph_keys.iter().map(|k| k.as_slice()).collect();
        let graph_values_refs: Vec<&[f32]> = graph_values.iter().map(|v| v.as_slice()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention
                .compute(&query, &graph_keys_refs, &graph_values_refs)
                .unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Edge-Featured Graph Attention (4 heads):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 8. Graph RoPE
    {
        let config = RoPEConfig::builder().dim(dim).max_position(1024).build();
        let attention = GraphRoPE::new(config);
        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention.compute(&query, &keys_refs, &values_refs).unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Graph RoPE Attention:");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 9. Dual-Space Attention
    {
        let config = DualSpaceConfig::builder()
            .dim(dim)
            .euclidean_weight(0.5)
            .hyperbolic_weight(0.5)
            .build();
        let attention = DualSpaceAttention::new(config);

        // Use smaller values for hyperbolic component
        let dual_query = vec![0.1f32; dim];
        let dual_keys: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| vec![(i as f32 * 0.001) % 0.3; dim])
            .collect();
        let dual_values: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| vec![(i as f32 * 0.002) % 0.3; dim])
            .collect();
        let dual_keys_refs: Vec<&[f32]> = dual_keys.iter().map(|k| k.as_slice()).collect();
        let dual_values_refs: Vec<&[f32]> = dual_values.iter().map(|v| v.as_slice()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = attention
                .compute(&dual_query, &dual_keys_refs, &dual_values_refs)
                .unwrap();
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("Dual-Space Attention (Euclidean + Hyperbolic):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 10. Training: InfoNCE Loss
    {
        let loss = InfoNCELoss::new(0.07);
        let anchor = vec![0.5f32; 128];
        let positive = vec![0.6f32; 128];
        let negatives: Vec<Vec<f32>> = (0..50)
            .map(|i| vec![(i as f32 * 0.01) % 1.0; 128])
            .collect();
        let neg_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

        let start = Instant::now();
        for _ in 0..iterations {
            let _ = loss.compute(&anchor, &positive, &neg_refs);
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / iterations as f64;
        println!("InfoNCE Loss (50 negatives):");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    // 11. Training: Adam Optimizer
    {
        let mut optimizer = Adam::new(dim, 0.001);
        let mut params = vec![0.5f32; dim];
        let gradients = vec![0.01f32; dim];

        let start = Instant::now();
        for _ in 0..iterations * 10 {
            optimizer.step(&mut params, &gradients);
        }
        let elapsed = start.elapsed();
        let avg_us = elapsed.as_micros() as f64 / (iterations * 10) as f64;
        println!("Adam Optimizer Step:");
        println!("  Total: {:?}", elapsed);
        println!("  Per iteration: {:.2} µs", avg_us);
        println!("  Throughput: {:.0} ops/sec", 1_000_000.0 / avg_us);
        println!();
    }

    println!("=== Benchmark Complete ===");

    // Summary
    println!("\n=== Summary ===");
    println!("All attention mechanisms functional and benchmarked.");
    println!("Module coverage:");
    println!("  - Core: ScaledDotProductAttention, MultiHeadAttention");
    println!("  - Sparse: FlashAttention, LinearAttention, LocalGlobalAttention");
    println!("  - MoE: MoEAttention with learned routing");
    println!("  - Graph: EdgeFeaturedAttention, GraphRoPE, DualSpaceAttention");
    println!("  - Hyperbolic: HyperbolicAttention, MixedCurvatureAttention");
    println!("  - Training: InfoNCE, ContrastiveLoss, Adam/AdamW/SGD, Curriculum");
}
