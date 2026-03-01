use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
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

fn bench_scaled_dot_product(c: &mut Criterion) {
    let mut group = c.benchmark_group("scaled_dot_product");

    for dim in [64, 128, 256, 512] {
        let attention = ScaledDotProductAttention::new(dim);

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, &dim| {
            let query = vec![0.5; dim];
            let keys: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
                .collect();
            let values: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
                .collect();
            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
        });
    }

    group.finish();
}

fn bench_flash_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("flash_attention");

    for seq_len in [64, 256, 512, 1024] {
        let dim = 256;
        let attention = FlashAttention::new(dim, 64);

        group.bench_with_input(
            BenchmarkId::new("seq_len", seq_len),
            &seq_len,
            |b, &seq_len| {
                let query = vec![0.5; dim];
                let keys: Vec<Vec<f32>> = (0..seq_len)
                    .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
                    .collect();
                let values: Vec<Vec<f32>> = (0..seq_len)
                    .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
                    .collect();
                let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
                let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

                b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
            },
        );
    }

    group.finish();
}

fn bench_linear_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("linear_attention");

    for seq_len in [256, 512, 1024, 2048] {
        let dim = 256;
        let attention = LinearAttention::new(dim, 64);

        group.bench_with_input(
            BenchmarkId::new("seq_len", seq_len),
            &seq_len,
            |b, &seq_len| {
                let query = vec![0.5; dim];
                let keys: Vec<Vec<f32>> = (0..seq_len)
                    .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
                    .collect();
                let values: Vec<Vec<f32>> = (0..seq_len)
                    .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
                    .collect();
                let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
                let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

                b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
            },
        );
    }

    group.finish();
}

fn bench_local_global_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("local_global_attention");

    for window_size in [16, 32, 64, 128] {
        let dim = 256;
        let attention = LocalGlobalAttention::new(dim, window_size, 4);

        group.bench_with_input(
            BenchmarkId::new("window", window_size),
            &window_size,
            |b, _| {
                let query = vec![0.5; dim];
                let keys: Vec<Vec<f32>> = (0..512)
                    .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
                    .collect();
                let values: Vec<Vec<f32>> = (0..512)
                    .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
                    .collect();
                let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
                let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

                b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
            },
        );
    }

    group.finish();
}

fn bench_moe_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("moe_attention");

    for num_experts in [2, 4, 8] {
        let config = MoEConfig::builder()
            .dim(256)
            .num_experts(num_experts)
            .top_k(2)
            .build();
        let attention = MoEAttention::new(config);

        group.bench_with_input(
            BenchmarkId::new("experts", num_experts),
            &num_experts,
            |b, _| {
                let query = vec![0.5; 256];
                let keys: Vec<Vec<f32>> = (0..100)
                    .map(|i| vec![(i as f32 * 0.01) % 1.0; 256])
                    .collect();
                let values: Vec<Vec<f32>> = (0..100)
                    .map(|i| vec![(i as f32 * 0.02) % 1.0; 256])
                    .collect();
                let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
                let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

                b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
            },
        );
    }

    group.finish();
}

fn bench_hyperbolic_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("hyperbolic_attention");

    for dim in [64, 128, 256] {
        let config = HyperbolicAttentionConfig {
            dim,
            curvature: -1.0,
            ..Default::default()
        };
        let attention = HyperbolicAttention::new(config);

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, &dim| {
            let query = vec![0.1; dim];
            let keys: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.001) % 0.5; dim])
                .collect();
            let values: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.002) % 0.5; dim])
                .collect();
            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
        });
    }

    group.finish();
}

fn bench_edge_featured_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("edge_featured_attention");

    for num_heads in [1, 2, 4, 8] {
        let config = EdgeFeaturedConfig::builder()
            .node_dim(256)
            .edge_dim(32)
            .num_heads(num_heads)
            .build();
        let attention = EdgeFeaturedAttention::new(config);

        group.bench_with_input(BenchmarkId::new("heads", num_heads), &num_heads, |b, _| {
            let query = vec![0.5; 256];
            let keys: Vec<Vec<f32>> = (0..64)
                .map(|i| vec![(i as f32 * 0.01) % 1.0; 256])
                .collect();
            let values: Vec<Vec<f32>> = (0..64)
                .map(|i| vec![(i as f32 * 0.02) % 1.0; 256])
                .collect();
            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
        });
    }

    group.finish();
}

fn bench_graph_rope(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph_rope");

    for dim in [64, 128, 256] {
        let config = RoPEConfig::builder().dim(dim).max_position(1024).build();
        let attention = GraphRoPE::new(config);

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, &dim| {
            let query = vec![0.5; dim];
            let keys: Vec<Vec<f32>> = (0..256)
                .map(|i| vec![(i as f32 * 0.01) % 1.0; dim])
                .collect();
            let values: Vec<Vec<f32>> = (0..256)
                .map(|i| vec![(i as f32 * 0.02) % 1.0; dim])
                .collect();
            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
        });
    }

    group.finish();
}

fn bench_dual_space_attention(c: &mut Criterion) {
    let mut group = c.benchmark_group("dual_space_attention");

    for dim in [64, 128, 256] {
        let config = DualSpaceConfig::builder()
            .dim(dim)
            .euclidean_weight(0.5)
            .hyperbolic_weight(0.5)
            .build();
        let attention = DualSpaceAttention::new(config);

        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, &dim| {
            let query = vec![0.1; dim];
            let keys: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.001) % 0.3; dim])
                .collect();
            let values: Vec<Vec<f32>> = (0..100)
                .map(|i| vec![(i as f32 * 0.002) % 0.3; dim])
                .collect();
            let keys_refs: Vec<&[f32]> = keys.iter().map(|k| k.as_slice()).collect();
            let values_refs: Vec<&[f32]> = values.iter().map(|v| v.as_slice()).collect();

            b.iter(|| black_box(attention.compute(&query, &keys_refs, &values_refs).unwrap()));
        });
    }

    group.finish();
}

fn bench_infonce_loss(c: &mut Criterion) {
    let mut group = c.benchmark_group("infonce_loss");

    for num_negatives in [10, 50, 100, 200] {
        let loss = InfoNCELoss::new(0.07);

        group.bench_with_input(
            BenchmarkId::new("negatives", num_negatives),
            &num_negatives,
            |b, &num_neg| {
                let anchor = vec![0.5; 128];
                let positive = vec![0.6; 128];
                let negatives: Vec<Vec<f32>> = (0..num_neg)
                    .map(|i| vec![(i as f32 * 0.01) % 1.0; 128])
                    .collect();
                let neg_refs: Vec<&[f32]> = negatives.iter().map(|n| n.as_slice()).collect();

                b.iter(|| black_box(loss.compute(&anchor, &positive, &neg_refs)));
            },
        );
    }

    group.finish();
}

fn bench_adam_optimizer(c: &mut Criterion) {
    let mut group = c.benchmark_group("adam_optimizer");

    for dim in [128, 256, 512, 1024] {
        group.bench_with_input(BenchmarkId::new("dim", dim), &dim, |b, &dim| {
            let mut optimizer = Adam::new(dim, 0.001);
            let mut params = vec![0.5; dim];
            let gradients = vec![0.01; dim];

            b.iter(|| {
                optimizer.step(&mut params, &gradients);
                black_box(&params)
            });
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_scaled_dot_product,
    bench_flash_attention,
    bench_linear_attention,
    bench_local_global_attention,
    bench_moe_attention,
    bench_hyperbolic_attention,
    bench_edge_featured_attention,
    bench_graph_rope,
    bench_dual_space_attention,
    bench_infonce_loss,
    bench_adam_optimizer,
);
criterion_main!(benches);
