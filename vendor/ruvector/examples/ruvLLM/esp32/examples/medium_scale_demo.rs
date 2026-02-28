//! Medium Scale Federation Demo - 100 to 500 Chip Clusters
//!
//! Shows the "sweet spot" for ESP32 federation where you get:
//! - High efficiency (40-70%)
//! - Great throughput (50K-100K tokens/sec)
//! - Practical costs ($400-$2,000)
//! - Real model capabilities (Small to Base models)

use ruvllm_esp32::federation::{
    MediumClusterConfig, ScaleComparison, MediumScaleAnalyzer,
    ModelCategory, HardwareConfig, BusType,
    MEDIUM_SCALE_MIN, MEDIUM_SCALE_MAX, MEDIUM_SCALE_OPTIMAL,
};

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║     RuvLLM ESP32 - Medium Scale Federation (100-500 Chips)            ║");
    println!("║     The Sweet Spot for Practical Distributed Inference                ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝\n");

    // ============================================================
    // 1. Why 100-500 Chips is the Sweet Spot
    // ============================================================
    println!("═══ Why 100-500 Chips? ═══\n");

    println!("  The 100-500 chip range is optimal because:");
    println!("  • High efficiency (40-70%) - minimal wasted compute");
    println!("  • Communication overhead stays low (<50%)");
    println!("  • Cost-effective ($400-$2,000 total)");
    println!("  • Can run meaningful models (5M-100M parameters)");
    println!("  • Practical hardware: fits in 1-2 rack units");
    println!();

    // ============================================================
    // 2. Standard Configurations
    // ============================================================
    println!("═══ Standard Medium-Scale Configurations ═══\n");

    println!("┌─────────┬───────────────┬────────────────┬────────────┬──────────┬──────────┐");
    println!("│  Chips  │   Topology    │   Throughput   │ Efficiency │   Cost   │  Power   │");
    println!("│         │  (clusters)   │   (tok/sec)    │            │   ($)    │   (W)    │");
    println!("├─────────┼───────────────┼────────────────┼────────────┼──────────┼──────────┤");

    for config in MediumClusterConfig::standard_configs() {
        println!("│ {:>7} │ {:>5} × {:>5} │ {:>14.0} │ {:>9.1}% │ {:>8.0} │ {:>8.1} │",
            config.total_chips,
            config.clusters,
            config.chips_per_cluster,
            config.expected_throughput,
            config.expected_efficiency * 100.0,
            config.cost_usd,
            config.power_watts,
        );
    }

    println!("└─────────┴───────────────┴────────────────┴────────────┴──────────┴──────────┘\n");

    // ============================================================
    // 3. Comparison vs Smaller Clusters
    // ============================================================
    println!("═══ Performance Comparison: Small vs Medium Clusters ═══\n");

    let key_sizes = [100, 256, 500];

    for chips in key_sizes {
        let comparison = ScaleComparison::analyze(chips);

        println!("  {} Chips vs Baselines:", chips);
        println!("  ┌───────────────┬─────────────────┬────────────────┐");
        println!("  │ Configuration │ Throughput      │ Improvement    │");
        println!("  ├───────────────┼─────────────────┼────────────────┤");
        println!("  │ 1 chip        │ {:>13.0} │ (baseline)     │",
            comparison.single_chip.throughput_tokens_sec);
        println!("  │ 5 chips       │ {:>13.0} │ {:>11.1}x    │",
            comparison.small_cluster.throughput_tokens_sec,
            comparison.small_cluster.throughput_tokens_sec / comparison.single_chip.throughput_tokens_sec);
        println!("  │ {} chips     │ {:>13.0} │ {:>11.1}x    │",
            chips,
            comparison.medium_cluster.throughput_tokens_sec,
            comparison.throughput_multiplier);
        println!("  └───────────────┴─────────────────┴────────────────┘");
        println!("    Cost per 1K tok/s: ${:.2}\n", comparison.cost_per_1k_tokens);
    }

    // ============================================================
    // 4. Model Capabilities at Each Scale
    // ============================================================
    println!("═══ What Models Can You Run? ═══\n");

    println!("┌─────────┬───────────────┬────────────────────────────────────────────────┐");
    println!("│  Chips  │  Model Size   │  Example Models                                │");
    println!("├─────────┼───────────────┼────────────────────────────────────────────────┤");

    for chips in [100, 150, 200, 256, 300, 400, 500] {
        let category = ModelCategory::for_chip_count(chips);
        let (min_params, max_params) = category.param_range();
        println!("│ {:>7} │ {:>5}-{:>5} │ {:46} │",
            chips,
            format_params(min_params),
            format_params(max_params),
            category.examples(),
        );
    }

    println!("└─────────┴───────────────┴────────────────────────────────────────────────┘\n");

    // ============================================================
    // 5. Hardware Requirements
    // ============================================================
    println!("═══ Hardware Requirements for Deployment ═══\n");

    println!("┌─────────┬────────────┬──────────┬─────────────┬───────────────────────────┐");
    println!("│  Chips  │ PCBs Req'd │ Chip/PCB │ Power (W)   │ Form Factor               │");
    println!("├─────────┼────────────┼──────────┼─────────────┼───────────────────────────┤");

    for chips in [100, 144, 256, 400, 500] {
        let hw = HardwareConfig::for_cluster(chips);
        println!("│ {:>7} │ {:>10} │ {:>8} │ {:>11.0} │ {:25} │",
            chips,
            hw.num_boards,
            hw.chips_per_board,
            hw.power_supply_watts,
            hw.form_factor,
        );
    }

    println!("└─────────┴────────────┴──────────┴─────────────┴───────────────────────────┘\n");

    println!("  Communication Bus Options:");
    println!("  ┌──────────────┬───────────────┬────────────────────────────────────────┐");
    println!("  │ Bus Type     │ Bandwidth     │ Best For                               │");
    println!("  ├──────────────┼───────────────┼────────────────────────────────────────┤");
    println!("  │ SPI          │ {:>11} │ Small clusters, simple wiring          │",
        format_bandwidth(BusType::Spi.bandwidth_bytes_sec()));
    println!("  │ I2C          │ {:>11} │ Slow but many devices                  │",
        format_bandwidth(BusType::I2c.bandwidth_bytes_sec()));
    println!("  │ UART Mesh    │ {:>11} │ Medium clusters, flexible              │",
        format_bandwidth(BusType::Uart.bandwidth_bytes_sec()));
    println!("  │ High-Speed   │ {:>11} │ Large clusters, custom hardware        │",
        format_bandwidth(BusType::HighSpeed.bandwidth_bytes_sec()));
    println!("  └──────────────┴───────────────┴────────────────────────────────────────┘\n");

    // ============================================================
    // 6. Optimization: Find Best Config for Your Needs
    // ============================================================
    println!("═══ Find Your Optimal Configuration ═══\n");

    // By throughput target
    println!("  Target Throughput → Recommended Chips:");
    println!("  ┌─────────────────────┬─────────┬────────────────┬──────────┐");
    println!("  │ Target (tok/sec)    │  Chips  │ Actual Output  │   Cost   │");
    println!("  ├─────────────────────┼─────────┼────────────────┼──────────┤");

    for target in [50_000.0, 60_000.0, 70_000.0, 80_000.0] {
        if let Some(config) = MediumScaleAnalyzer::optimize_for_throughput(target) {
            println!("  │ {:>19.0} │ {:>7} │ {:>14.0} │ ${:>7.0} │",
                target,
                config.total_chips,
                config.expected_throughput,
                config.cost_usd,
            );
        }
    }
    println!("  └─────────────────────┴─────────┴────────────────┴──────────┘\n");

    // By budget
    println!("  Budget → Maximum Configuration:");
    println!("  ┌─────────────────────┬─────────┬────────────────┬────────────┐");
    println!("  │ Budget ($)          │  Chips  │   Throughput   │ Efficiency │");
    println!("  ├─────────────────────┼─────────┼────────────────┼────────────┤");

    for budget in [500.0, 1000.0, 1500.0, 2000.0] {
        let config = MediumScaleAnalyzer::optimize_for_budget(budget);
        println!("  │ ${:>18.0} │ {:>7} │ {:>14.0} │ {:>9.1}% │",
            budget,
            config.total_chips,
            config.expected_throughput,
            config.expected_efficiency * 100.0,
        );
    }
    println!("  └─────────────────────┴─────────┴────────────────┴────────────┘\n");

    // ============================================================
    // 7. Summary: The Sweet Spot
    // ============================================================
    println!("╔═══════════════════════════════════════════════════════════════════════╗");
    println!("║                    MEDIUM SCALE SUMMARY                               ║");
    println!("╠═══════════════════════════════════════════════════════════════════════╣");
    println!("║                                                                       ║");
    println!("║  The 100-500 chip range is ideal for:                                 ║");
    println!("║                                                                       ║");
    println!("║  ✓ HOME/OFFICE: 100 chips ($400) = 53K tok/s, 70% efficient           ║");
    println!("║    - Runs Small models (5-20M params)                                 ║");
    println!("║    - Fits in single rack unit                                         ║");
    println!("║    - 50W power consumption                                            ║");
    println!("║                                                                       ║");
    println!("║  ✓ WORKSTATION: 256 chips ($1,024) = 88K tok/s, 55% efficient         ║");
    println!("║    - Runs Base models (20-100M params)                                ║");
    println!("║    - 2U rack mount                                                    ║");
    println!("║    - 130W power consumption                                           ║");
    println!("║                                                                       ║");
    println!("║  ✓ SERVER: 500 chips ($2,000) = 106K tok/s, 40% efficient             ║");
    println!("║    - Runs Large models (100M+ params)                                 ║");
    println!("║    - Full rack unit                                                   ║");
    println!("║    - 250W power consumption                                           ║");
    println!("║                                                                       ║");
    println!("║  KEY INSIGHT: Beyond 500 chips, efficiency drops significantly.       ║");
    println!("║  For larger models, use multiple 256-500 chip clusters in parallel.   ║");
    println!("║                                                                       ║");
    println!("╚═══════════════════════════════════════════════════════════════════════╝");
}

fn format_params(n: usize) -> String {
    if n >= 1_000_000_000 {
        format!("{:.0}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.0}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

fn format_bandwidth(bps: usize) -> String {
    if bps >= 1_000_000 {
        format!("{} MB/s", bps / 1_000_000)
    } else if bps >= 1_000 {
        format!("{} KB/s", bps / 1_000)
    } else {
        format!("{} B/s", bps)
    }
}
