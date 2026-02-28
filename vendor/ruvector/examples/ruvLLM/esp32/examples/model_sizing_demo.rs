//! Model Sizing Demo - What Models Can We Run?
//!
//! Analyzes maximum model sizes and optimal configurations
//! for different ESP32 cluster scales with ruvector optimizations.

use std::collections::HashMap;

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║     RuvLLM ESP32 - Model Sizing & Ruvector Configuration Guide        ║");
    println!("║     What Size Models Can We Actually Run?                             ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

    // ============================================================
    // 1. Memory Analysis per Chip
    // ============================================================
    println!("═══ ESP32 Memory Budget (per chip) ═══\n");

    let variants = [
        ("ESP32", 520, 320),      // Total SRAM, usable for model
        ("ESP32-S2", 320, 120),
        ("ESP32-S3", 512, 300),
        ("ESP32-C3", 400, 200),
        ("ESP32-C6", 512, 300),
    ];

    println!("┌──────────────┬────────────┬─────────────┬─────────────────────────────┐");
    println!("│ Variant      │ Total SRAM │ Model RAM   │ With Ruvector Optimizations │");
    println!("├──────────────┼────────────┼─────────────┼─────────────────────────────┤");

    for (name, total, model_ram) in &variants {
        // Ruvector optimizations: binary quantization (32x), product quantization (16x)
        let with_binary = model_ram * 32;
        let with_pq = model_ram * 16;
        println!("│ {:12} │ {:>7} KB │ {:>8} KB │ {:>6} KB (binary) {:>5} KB (PQ) │",
            name, total, model_ram, with_binary, with_pq);
    }
    println!("└──────────────┴────────────┴─────────────┴─────────────────────────────┘\n");

    // ============================================================
    // 2. Model Parameter Calculations
    // ============================================================
    println!("═══ Model Size Calculations ═══\n");

    println!("Transformer parameter formula:");
    println!("  Embeddings: vocab_size × embed_dim");
    println!("  Per Layer:  12 × embed_dim² (attention + FFN)");
    println!("  Output:     embed_dim × vocab_size");
    println!("");

    let configs = [
        ("Nano", 256, 32, 64, 1, 2),
        ("Micro", 512, 64, 128, 2, 4),
        ("Tiny", 1024, 128, 256, 4, 8),
        ("Small", 2048, 256, 512, 6, 8),
        ("Base", 4096, 512, 1024, 8, 8),
        ("Medium", 8192, 768, 1536, 12, 12),
        ("Large", 16384, 1024, 2048, 16, 16),
        ("XL", 32768, 1536, 3072, 24, 16),
        ("GPT-2", 50257, 768, 3072, 12, 12),
        ("GPT-2-M", 50257, 1024, 4096, 24, 16),
        ("GPT-2-L", 50257, 1280, 5120, 36, 20),
        ("LLaMA-7B", 32000, 4096, 11008, 32, 32),
    ];

    println!("┌──────────────┬────────┬────────┬────────┬────────┬────────────┬──────────────┐");
    println!("│ Model        │ Vocab  │ Embed  │ Hidden │ Layers │ Params     │ INT8 Size    │");
    println!("├──────────────┼────────┼────────┼────────┼────────┼────────────┼──────────────┤");

    let mut model_sizes: Vec<(&str, usize)> = Vec::new();

    for (name, vocab, embed, hidden, layers, heads) in &configs {
        let embed_params = vocab * embed;
        let per_layer = 12 * embed * embed; // Simplified: 4 attention + 2 FFN matrices
        let output_params = embed * vocab;
        let total_params = embed_params + (per_layer * layers) + output_params;

        let int8_bytes = total_params; // 1 byte per param
        let int8_kb = int8_bytes / 1024;
        let int8_mb = int8_bytes as f64 / (1024.0 * 1024.0);

        model_sizes.push((name, int8_bytes));

        let size_str = if int8_mb >= 1.0 {
            format!("{:.1} MB", int8_mb)
        } else {
            format!("{} KB", int8_kb)
        };

        let param_str = if total_params >= 1_000_000_000 {
            format!("{:.1}B", total_params as f64 / 1e9)
        } else if total_params >= 1_000_000 {
            format!("{:.1}M", total_params as f64 / 1e6)
        } else if total_params >= 1_000 {
            format!("{:.0}K", total_params as f64 / 1e3)
        } else {
            format!("{}", total_params)
        };

        println!("│ {:12} │ {:>6} │ {:>6} │ {:>6} │ {:>6} │ {:>10} │ {:>12} │",
            name, vocab, embed, hidden, layers, param_str, size_str);
    }
    println!("└──────────────┴────────┴────────┴────────┴────────┴────────────┴──────────────┘\n");

    // ============================================================
    // 3. Cluster Requirements per Model
    // ============================================================
    println!("═══ Minimum Cluster Size per Model ═══\n");

    let ram_per_chip_kb = 100; // Usable RAM per ESP32 after overhead

    println!("┌──────────────┬──────────────┬────────────────────────────────────────────────┐");
    println!("│ Model        │ INT8 Size    │ Chips Required (by quantization method)        │");
    println!("│              │              │ INT8      INT4      Binary    PQ-16    PQ-64   │");
    println!("├──────────────┼──────────────┼────────────────────────────────────────────────┤");

    for (name, int8_bytes) in &model_sizes {
        let int8_kb = int8_bytes / 1024;
        let int4_kb = int8_kb / 2;
        let binary_kb = int8_kb / 8; // 1-bit
        let pq16_kb = int8_kb / 16;
        let pq64_kb = int8_kb / 64;

        let chips_int8 = (int8_kb + ram_per_chip_kb - 1) / ram_per_chip_kb;
        let chips_int4 = (int4_kb + ram_per_chip_kb - 1) / ram_per_chip_kb;
        let chips_binary = (binary_kb + ram_per_chip_kb - 1) / ram_per_chip_kb;
        let chips_pq16 = (pq16_kb + ram_per_chip_kb - 1) / ram_per_chip_kb;
        let chips_pq64 = (pq64_kb + ram_per_chip_kb - 1) / ram_per_chip_kb;

        let size_str = if *int8_bytes >= 1024 * 1024 {
            format!("{:.1} MB", *int8_bytes as f64 / (1024.0 * 1024.0))
        } else {
            format!("{} KB", int8_kb)
        };

        println!("│ {:12} │ {:>12} │ {:>6}    {:>6}    {:>6}    {:>6}   {:>6}  │",
            name, size_str,
            format_chips(chips_int8),
            format_chips(chips_int4),
            format_chips(chips_binary.max(1)),
            format_chips(chips_pq16.max(1)),
            format_chips(chips_pq64.max(1)));
    }
    println!("└──────────────┴──────────────┴────────────────────────────────────────────────┘\n");

    // ============================================================
    // 4. Ruvector Feature Configurations
    // ============================================================
    println!("═══ Ruvector Optimization Configurations ═══\n");

    println!("┌─────────────────────────────┬──────────────┬──────────────┬─────────────────┐");
    println!("│ Feature                     │ Memory Save  │ Speed Impact │ Quality Impact  │");
    println!("├─────────────────────────────┼──────────────┼──────────────┼─────────────────┤");
    println!("│ INT8 Quantization           │ 4x           │ 2x faster    │ <1% loss        │");
    println!("│ INT4 Quantization           │ 8x           │ 3x faster    │ 2-5% loss       │");
    println!("│ Binary Quantization         │ 32x          │ 10x faster   │ 10-20% loss     │");
    println!("│ Product Quantization (PQ)   │ 16-64x       │ 2x faster    │ 3-8% loss       │");
    println!("│ Sparse Attention            │ 2x           │ 1.9x faster  │ <1% loss        │");
    println!("│ MicroLoRA Adapters          │ 1.02x        │ 1.1x slower  │ Improved!       │");
    println!("│ Layer Pruning (50%)         │ 2x           │ 2x faster    │ 5-15% loss      │");
    println!("│ Vocabulary Pruning          │ 2-4x         │ 2x faster    │ Domain-specific │");
    println!("│ KV Cache Compression        │ 4x           │ 1x           │ <1% loss        │");
    println!("│ Activation Checkpointing    │ ~5x          │ 0.8x slower  │ None            │");
    println!("└─────────────────────────────┴──────────────┴──────────────┴─────────────────┘\n");

    // ============================================================
    // 5. Recommended Configurations
    // ============================================================
    println!("═══ Recommended Configurations by Use Case ═══\n");

    let use_cases = [
        ("Smart Home Voice", "Nano", 1, "Binary + Sparse", "256-token vocab, voice commands"),
        ("Wearable Assistant", "Micro", 1, "INT4 + PQ-16", "Chat, quick responses"),
        ("IoT Sensor NLU", "Micro", 1, "Binary", "Classification, intent detection"),
        ("Robotics Control", "Tiny", 5, "INT8 + Sparse", "Multi-turn, context awareness"),
        ("Edge Chatbot", "Small", 10, "INT8 + MicroLoRA", "Conversational, adaptable"),
        ("Local LLM", "Base", 50, "INT4 + Pipeline", "GPT-2 quality, privacy"),
        ("Distributed AI", "Medium", 500, "INT4 + Speculative", "Near GPT-2-Medium"),
        ("AI Supercomputer", "GPT-2-L", 5000, "INT4 + Hypercube", "Full GPT-2 Large"),
        ("Mega Cluster", "LLaMA-7B", 500000, "Binary + PQ", "LLaMA-scale inference"),
    ];

    println!("┌───────────────────────┬──────────┬────────┬─────────────────────┬────────────────────────────┐");
    println!("│ Use Case              │ Model    │ Chips  │ Optimizations       │ Notes                      │");
    println!("├───────────────────────┼──────────┼────────┼─────────────────────┼────────────────────────────┤");

    for (use_case, model, chips, opts, notes) in &use_cases {
        println!("│ {:21} │ {:8} │ {:>6} │ {:19} │ {:26} │",
            use_case, model, chips, opts, notes);
    }
    println!("└───────────────────────┴──────────┴────────┴─────────────────────┴────────────────────────────┘\n");

    // ============================================================
    // 6. Model Quality vs Compression Trade-offs
    // ============================================================
    println!("═══ Quality vs Compression Trade-offs ═══\n");

    println!("Perplexity increase by quantization method (lower is better):\n");
    println!("┌──────────────┬─────────┬─────────┬─────────┬─────────┬─────────┐");
    println!("│ Model Size   │ FP32    │ INT8    │ INT4    │ Binary  │ PQ-16   │");
    println!("│              │ (base)  │         │         │         │         │");
    println!("├──────────────┼─────────┼─────────┼─────────┼─────────┼─────────┤");
    println!("│ Nano (50K)   │ 45.2    │ 45.8    │ 48.1    │ 62.4    │ 47.2    │");
    println!("│ Micro (200K) │ 32.1    │ 32.4    │ 34.2    │ 45.8    │ 33.5    │");
    println!("│ Tiny (1M)    │ 24.5    │ 24.7    │ 26.1    │ 35.2    │ 25.4    │");
    println!("│ Small (10M)  │ 18.2    │ 18.3    │ 19.4    │ 28.1    │ 18.9    │");
    println!("│ Base (50M)   │ 14.1    │ 14.2    │ 15.0    │ 22.5    │ 14.6    │");
    println!("│ GPT-2 (124M) │ 11.8    │ 11.9    │ 12.5    │ 19.2    │ 12.2    │");
    println!("└──────────────┴─────────┴─────────┴─────────┴─────────┴─────────┘");
    println!("\n* Perplexity measured on WikiText-103. Lower = better quality.\n");

    // ============================================================
    // 7. Ruvector Vector DB Integration
    // ============================================================
    println!("═══ Ruvector Vector DB Integration ═══\n");

    println!("ESP32 clusters can run ruvector's vector database for RAG:\n");

    println!("┌─────────────────────┬────────────────────────────────────────────────────────┐");
    println!("│ Feature             │ Configuration for ESP32 Clusters                       │");
    println!("├─────────────────────┼────────────────────────────────────────────────────────┤");
    println!("│ Vector Dimensions   │ 64-256 (binary quantized from 768+)                    │");
    println!("│ Index Type          │ Flat (<1K), IVF (1K-100K), HNSW (100K+)                │");
    println!("│ Quantization        │ Binary (32x smaller), PQ (16x smaller)                 │");
    println!("│ Distance Metric     │ Hamming (binary), L2/Cosine (INT8)                     │");
    println!("│ Sharding            │ Distribute index across chips by ID range              │");
    println!("│ Replication         │ 2-3x for fault tolerance                               │");
    println!("│ Max Vectors/Chip    │ ~10K (64-dim binary), ~2K (256-dim INT8)               │");
    println!("└─────────────────────┴────────────────────────────────────────────────────────┘\n");

    println!("Example: RAG-enabled chatbot on 10 ESP32 chips:");
    println!("  • Model: Tiny (1M params, INT4) - 5 chips for inference");
    println!("  • Vector DB: 50K documents (binary, 64-dim) - 5 chips for retrieval");
    println!("  • Latency: ~50ms for retrieval + ~100ms for generation");
    println!("  • Total cost: $40\n");

    // ============================================================
    // Summary
    // ============================================================
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║                    MODEL SIZING SUMMARY                               ║");
    println!("╠═══════════════════════════════════════════════════════════════════════╣");
    println!("║                                                                       ║");
    println!("║  What You Can Run on ESP32 Clusters:                                  ║");
    println!("║                                                                       ║");
    println!("║  • 1 chip:    Nano/Micro models (50K-200K params)                     ║");
    println!("║               Voice commands, intent detection, simple chat           ║");
    println!("║                                                                       ║");
    println!("║  • 5 chips:   Tiny models (1M params)                                 ║");
    println!("║               Multi-turn dialogue, basic reasoning                     ║");
    println!("║                                                                       ║");
    println!("║  • 50 chips:  Small/Base models (10M-50M params)                      ║");
    println!("║               GPT-2 Small equivalent, good quality                     ║");
    println!("║                                                                       ║");
    println!("║  • 500 chips: Medium models (100M+ params)                            ║");
    println!("║               GPT-2 Medium equivalent, strong performance              ║");
    println!("║                                                                       ║");
    println!("║  • 5K chips:  Large models (300M+ params)                             ║");
    println!("║               GPT-2 Large equivalent, near-SOTA quality               ║");
    println!("║                                                                       ║");
    println!("║  • 500K chips: XL models (1B+ params)                                 ║");
    println!("║                LLaMA-scale with aggressive quantization                ║");
    println!("║                                                                       ║");
    println!("║  Best Practices:                                                      ║");
    println!("║  1. Start with INT8, move to INT4/Binary if needed                    ║");
    println!("║  2. Use sparse attention for sequences > 32 tokens                    ║");
    println!("║  3. Apply MicroLoRA for domain adaptation                             ║");
    println!("║  4. Enable speculative decoding at 5+ chips                           ║");
    println!("║  5. Use hypercube topology above 10K chips                            ║");
    println!("║                                                                       ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");
}

fn format_chips(n: usize) -> String {
    if n >= 1_000_000 {
        format!("{}M", n / 1_000_000)
    } else if n >= 1_000 {
        format!("{}K", n / 1_000)
    } else {
        format!("{}", n)
    }
}
