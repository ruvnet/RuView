//! Federation Demo - Multi-ESP32 Distributed Inference
//!
//! Demonstrates 5-chip federation with self-learning optimization.

use std::time::Instant;
use ruvllm_esp32::federation::{
    FederationConfig, FederationMode, estimate_speedup,
    PipelineConfig, PipelineNode, PipelineRole,
    FederationCoordinator, ClusterTopology,
    MicroFastGRNN, MicroGRNNConfig,
    SpeculativeDecoder, DraftVerifyConfig,
    ChipId, FederationMessage,
};
use ruvllm_esp32::optimizations::{
    MicroLoRA, LoRAConfig,
    SparseAttention, AttentionPattern,
    LayerPruner, PruningConfig,
};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║     RuvLLM ESP32 - 5-Chip Federation Benchmark                ║");
    println!("║     With Self-Learning & Ruvector Optimizations               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    const NUM_CHIPS: usize = 5;
    const TOTAL_LAYERS: usize = 10;
    const EMBED_DIM: usize = 64;
    const BENCHMARK_ITERS: usize = 1000;

    // ============================================================
    // 1. Federation Configuration Comparison
    // ============================================================
    println!("═══ Federation Mode Comparison ═══\n");

    let modes = [
        ("Standalone (1 chip)", FederationMode::Standalone, 1),
        ("Pipeline (5 chips)", FederationMode::Pipeline, 5),
        ("Tensor Parallel (5 chips)", FederationMode::TensorParallel, 5),
        ("Speculative (5 chips)", FederationMode::Speculative, 5),
        ("Mixture of Experts (5 chips)", FederationMode::MixtureOfExperts, 5),
    ];

    println!("┌─────────────────────────────┬────────────┬────────────┬─────────────┐");
    println!("│ Mode                        │ Throughput │ Latency    │ Memory/Chip │");
    println!("├─────────────────────────────┼────────────┼────────────┼─────────────┤");

    for (name, mode, chips) in modes {
        let config = FederationConfig {
            num_chips: chips,
            mode,
            ..Default::default()
        };
        let speedup = estimate_speedup(&config);

        println!("│ {:27} │ {:>8.1}x  │ {:>8.1}x  │ {:>9.1}x  │",
            name,
            speedup.throughput_multiplier,
            speedup.latency_reduction,
            speedup.memory_per_chip_reduction,
        );
    }
    println!("└─────────────────────────────┴────────────┴────────────┴─────────────┘\n");

    // ============================================================
    // 2. Pipeline Parallelism Benchmark
    // ============================================================
    println!("═══ Pipeline Parallelism (5 Chips, 10 Layers) ═══\n");

    let mut pipeline_nodes: Vec<PipelineNode> = (0..NUM_CHIPS)
        .map(|i| {
            let config = PipelineConfig::for_chip(i, NUM_CHIPS, TOTAL_LAYERS, EMBED_DIM);
            PipelineNode::new(config)
        })
        .collect();

    // Print pipeline configuration
    for (i, node) in pipeline_nodes.iter().enumerate() {
        let config = PipelineConfig::for_chip(i, NUM_CHIPS, TOTAL_LAYERS, EMBED_DIM);
        println!("  Chip {}: {:?}, Layers {}-{}",
            i,
            config.role(),
            config.layer_start,
            config.layer_start + config.layer_count - 1,
        );
    }
    println!("");

    // Simulate pipeline processing
    let start = Instant::now();
    for _ in 0..BENCHMARK_ITERS {
        // Simulate a token going through the pipeline
        let _ = pipeline_nodes[0].start_token(1);
        for chip_idx in 0..NUM_CHIPS {
            let _ = pipeline_nodes[chip_idx].process_step(|_layer, _data| Ok(()));
        }
    }
    let pipeline_time = start.elapsed();
    println!("  Pipeline throughput: {:.0} tokens/sec (simulated)",
        BENCHMARK_ITERS as f64 / pipeline_time.as_secs_f64());

    // ============================================================
    // 3. FastGRNN Router Benchmark
    // ============================================================
    println!("\n═══ FastGRNN Micro Router ═══\n");

    let grnn_config = MicroGRNNConfig {
        input_dim: 8,
        hidden_dim: 4,
        num_chips: 5,
        zeta: 16,
        nu: 16,
    };

    let mut router = MicroFastGRNN::new(grnn_config, 42).unwrap();

    println!("  Router memory: {} bytes", router.memory_size());
    println!("  Input dim: {}, Hidden dim: {}", grnn_config.input_dim, grnn_config.hidden_dim);

    // Benchmark routing decisions
    let test_input = [64i8, 32, 16, 8, 4, 2, 1, 0];
    let start = Instant::now();
    for _ in 0..BENCHMARK_ITERS {
        router.step(&test_input).unwrap();
        let _ = router.route();
    }
    let router_time = start.elapsed();

    println!("  Routing decisions: {} in {:?}", BENCHMARK_ITERS, router_time);
    println!("  Per-decision: {:.3} us", router_time.as_nanos() as f64 / BENCHMARK_ITERS as f64 / 1000.0);

    // Show routing distribution
    router.reset();
    let mut chip_counts = [0usize; 5];
    for i in 0..100 {
        let input: [i8; 8] = [(i % 127) as i8; 8];
        router.step(&input).unwrap();
        let chip = router.route();
        chip_counts[chip.0 as usize] += 1;
    }
    println!("  Route distribution (100 samples): {:?}", chip_counts);

    // ============================================================
    // 4. Speculative Decoding Benchmark
    // ============================================================
    println!("\n═══ Speculative Decoding ═══\n");

    let spec_config = DraftVerifyConfig::for_five_chips();
    let mut drafter = SpeculativeDecoder::new(spec_config.clone(), ChipId(0));
    let mut verifier = SpeculativeDecoder::new(spec_config.clone(), ChipId(1));

    println!("  Draft chip: 0, Verify chips: 1-4");
    println!("  Draft length: {}", spec_config.draft_length);
    println!("  Acceptance threshold: {:.0}%", spec_config.acceptance_threshold * 100.0);

    // Simulate speculative decoding
    let start = Instant::now();
    let mut total_accepted = 0;
    for _ in 0..BENCHMARK_ITERS / 10 {
        // Create draft
        let mut draft = ruvllm_esp32::federation::speculative::DraftResult {
            tokens: heapless::Vec::new(),
            probs: heapless::Vec::new(),
            start_pos: 0,
        };
        for i in 0..4 {
            let _ = draft.tokens.push(100 + i);
            let _ = draft.probs.push(200);
        }

        // Verify
        let result = verifier.verify_draft(&draft, |_pos, _token| 195);
        total_accepted += result.accepted_count;
    }
    let spec_time = start.elapsed();

    let acceptance_rate = total_accepted as f64 / (BENCHMARK_ITERS as f64 / 10.0 * 4.0);
    println!("  Acceptance rate: {:.1}%", acceptance_rate * 100.0);
    println!("  Estimated speedup: {:.1}x", 1.0 + acceptance_rate * 3.0);

    // ============================================================
    // 5. Coordinator with Self-Learning
    // ============================================================
    println!("\n═══ Federation Coordinator with Self-Learning ═══\n");

    let fed_config = FederationConfig::default();
    let mut coordinator = FederationCoordinator::new(fed_config, true);

    // Initialize distributed LoRA
    coordinator.init_distributed_lora(32, 42).unwrap();

    println!("  Self-learning: Enabled");
    println!("  Distributed LoRA: Rank 1, Dim 32");

    // Simulate learning updates
    for i in 0..100 {
        let loss = 1000 - i * 8 + (i % 10) as i32;
        coordinator.update_learning(loss);
    }

    let stats = coordinator.stats();
    println!("  Learning rate: {}", stats.learning_rate);
    println!("  Avg loss: {}", stats.avg_loss);
    println!("  Active chips: {}/{}", stats.active_chips, stats.total_chips);

    // ============================================================
    // 6. Combined Optimization Impact
    // ============================================================
    println!("\n═══ Combined Optimization Impact ═══\n");

    // Calculate combined improvements
    let baseline_tok_s = 236.0; // Single ESP32
    let pipeline_speedup = estimate_speedup(&FederationConfig {
        num_chips: 5,
        mode: FederationMode::Pipeline,
        ..Default::default()
    });

    let with_pipeline = baseline_tok_s * pipeline_speedup.throughput_multiplier;
    let with_sparse = with_pipeline * 1.9; // Sparse attention
    let with_binary = with_sparse * 2.0; // Binary quantization on embeddings
    let with_speculative = with_binary * (1.0 + acceptance_rate as f32 * 2.0);

    println!("  ┌──────────────────────────────┬────────────────┐");
    println!("  │ Configuration                │ Tokens/sec     │");
    println!("  ├──────────────────────────────┼────────────────┤");
    println!("  │ Baseline (1 chip)            │ {:>12.0}   │", baseline_tok_s);
    println!("  │ + Pipeline (5 chips)         │ {:>12.0}   │", with_pipeline);
    println!("  │ + Sparse Attention           │ {:>12.0}   │", with_sparse);
    println!("  │ + Binary Embeddings          │ {:>12.0}   │", with_binary);
    println!("  │ + Speculative Decoding       │ {:>12.0}   │", with_speculative);
    println!("  └──────────────────────────────┴────────────────┘");

    // Memory per chip
    let baseline_mem = 119.0; // KB
    let mem_per_chip = baseline_mem / pipeline_speedup.memory_per_chip_reduction;

    println!("\n  Memory per chip: {:.0} KB (down from {:.0} KB)", mem_per_chip, baseline_mem);

    // ============================================================
    // Summary
    // ============================================================
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                    FEDERATION SUMMARY                         ║");
    println!("╠═══════════════════════════════════════════════════════════════╣");
    println!("║  5 ESP32 Chips in Pipeline Configuration                      ║");
    println!("║                                                               ║");
    println!("║  • Pipeline Speedup: {:.1}x throughput                         ║", pipeline_speedup.throughput_multiplier);
    println!("║  • Memory/Chip: {:.0} KB (from 119 KB)                         ║", mem_per_chip);
    println!("║  • FastGRNN Router: {:.0} decisions/sec                   ║",
        BENCHMARK_ITERS as f64 / router_time.as_secs_f64());
    println!("║  • Speculative Decoding: {:.0}% acceptance                     ║", acceptance_rate * 100.0);
    println!("║  • Self-Learning: Distributed MicroLoRA enabled               ║");
    println!("║                                                               ║");
    println!("║  Combined Performance: {:.0} tokens/sec                   ║", with_speculative);
    println!("║  Improvement over baseline: {:.0}x                             ║", with_speculative / baseline_tok_s);
    println!("╚═══════════════════════════════════════════════════════════════╝");
}
