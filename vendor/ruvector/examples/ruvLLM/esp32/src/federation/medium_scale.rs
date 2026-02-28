//! Medium Scale Federation - 100 to 500 Chip Clusters
//!
//! This is the "sweet spot" for ESP32 federation:
//! - High efficiency (40-70%)
//! - Practical throughput (50K-100K tokens/sec)
//! - Manageable communication overhead
//! - Affordable cost ($400-$2,000)
//!
//! # Why 100-500 Chips?
//!
//! ```text
//! Performance vs Chip Count:
//!
//! 100K ┤              ┌─────────────────────── Communication-bound
//!      │         ____/│  Sweet Spot
//!  80K ┤       /      │  100-500 chips
//!      │     /        │
//!  60K ┤   /          │  • 40-70% efficiency
//!      │  │           │  • Low communication overhead
//!  40K ┤ │            │  • Best $/performance
//!      ││             └─────────────────────────────────
//!  20K ┤│
//!      │
//!    0 ┼──────────────────────────────────────────────────
//!        5   50  100  200  500  1K   5K   10K  100K  1M
//!             ▲           ▲
//!             │           │
//!         Good start   Best value
//! ```
//!
//! # Topology Recommendations
//!
//! | Chips | Best Topology | Clusters × Chips | Efficiency |
//! |-------|---------------|------------------|------------|
//! | 100   | 10×10 Grid    | 10 × 10          | ~70%       |
//! | 144   | 12×12 Grid    | 12 × 12          | ~65%       |
//! | 256   | 16×16 Grid    | 16 × 16          | ~55%       |
//! | 400   | 20×20 Grid    | 20 × 20          | ~45%       |
//! | 500   | 25×20 Grid    | 25 × 20          | ~40%       |

use super::massive_scale::{MassiveTopology, MassiveScaleConfig, MassiveScaleSimulator, ScaleProjection};
use heapless::Vec as HVec;

/// Medium-scale cluster sizes (sweet spot)
pub const MEDIUM_SCALE_MIN: usize = 100;
pub const MEDIUM_SCALE_MAX: usize = 500;
pub const MEDIUM_SCALE_OPTIMAL: usize = 256; // Best efficiency/throughput balance

/// Pre-optimized cluster configurations
#[derive(Debug, Clone, Copy)]
pub struct MediumClusterConfig {
    /// Total chips in cluster
    pub total_chips: usize,
    /// Number of clusters (groups)
    pub clusters: usize,
    /// Chips per cluster
    pub chips_per_cluster: usize,
    /// Expected throughput (tokens/sec)
    pub expected_throughput: f64,
    /// Expected efficiency
    pub expected_efficiency: f64,
    /// Estimated cost USD
    pub cost_usd: f64,
    /// Power consumption watts
    pub power_watts: f64,
    /// Max model parameters supportable
    pub max_params: usize,
}

impl MediumClusterConfig {
    /// Get optimal configuration for given chip count
    pub fn optimal_for(chip_count: usize) -> Self {
        let chips = chip_count.clamp(MEDIUM_SCALE_MIN, MEDIUM_SCALE_MAX);

        // Find best square-ish layout
        let sqrt = (chips as f64).sqrt();
        let clusters = sqrt.ceil() as usize;
        let per_cluster = (chips + clusters - 1) / clusters;
        let actual_chips = clusters * per_cluster;

        // Simulate to get accurate projections
        let config = MassiveScaleConfig {
            topology: MassiveTopology::HierarchicalPipeline {
                clusters,
                chips_per_cluster: per_cluster,
            },
            total_layers: 32,
            embed_dim: 64,
            hop_latency_us: 10,
            link_bandwidth: 10_000_000,
            layer_compute_us: 4000,
            speculative: true,
            spec_depth: 4,
            gradient_checkpointing: false,
            fault_tolerance: 1,
        };

        let sim = MassiveScaleSimulator::new(config);
        let proj = sim.project();

        Self {
            total_chips: actual_chips,
            clusters,
            chips_per_cluster: per_cluster,
            expected_throughput: proj.throughput_tokens_sec,
            expected_efficiency: proj.efficiency,
            cost_usd: proj.cost_usd,
            power_watts: proj.power_watts,
            max_params: proj.max_parameters,
        }
    }

    /// Get all standard configurations
    pub fn standard_configs() -> [Self; 5] {
        [
            Self::optimal_for(100),
            Self::optimal_for(144),
            Self::optimal_for(256),
            Self::optimal_for(400),
            Self::optimal_for(500),
        ]
    }
}

/// Comparison with smaller clusters
#[derive(Debug, Clone)]
pub struct ScaleComparison {
    /// Single chip baseline
    pub single_chip: ScaleProjection,
    /// 5-chip small cluster
    pub small_cluster: ScaleProjection,
    /// Medium cluster (specified)
    pub medium_cluster: ScaleProjection,
    /// Throughput multiplier vs single
    pub throughput_multiplier: f64,
    /// Throughput multiplier vs 5-chip
    pub vs_small_multiplier: f64,
    /// Cost per 1K tokens/sec
    pub cost_per_1k_tokens: f64,
}

impl ScaleComparison {
    /// Compare medium cluster against baselines
    pub fn analyze(chip_count: usize) -> Self {
        let base_config = MassiveScaleConfig {
            total_layers: 32,
            embed_dim: 64,
            hop_latency_us: 10,
            link_bandwidth: 10_000_000,
            layer_compute_us: 4000,
            speculative: true,
            spec_depth: 4,
            ..Default::default()
        };

        // Single chip
        let single_sim = MassiveScaleSimulator::new(MassiveScaleConfig {
            topology: MassiveTopology::FlatMesh { size: 1 },
            ..base_config.clone()
        });
        let single = single_sim.project();

        // 5-chip small cluster
        let small_sim = MassiveScaleSimulator::new(MassiveScaleConfig {
            topology: MassiveTopology::FlatMesh { size: 5 },
            ..base_config.clone()
        });
        let small = small_sim.project();

        // Medium cluster
        let medium_sim = MassiveScaleSimulator::new(MassiveScaleConfig {
            topology: MassiveTopology::recommended(chip_count),
            ..base_config.clone()
        });
        let medium = medium_sim.project();

        Self {
            throughput_multiplier: medium.throughput_tokens_sec / single.throughput_tokens_sec,
            vs_small_multiplier: medium.throughput_tokens_sec / small.throughput_tokens_sec,
            cost_per_1k_tokens: medium.cost_usd / (medium.throughput_tokens_sec / 1000.0),
            single_chip: single,
            small_cluster: small,
            medium_cluster: medium,
        }
    }
}

/// Model categories that can run at different scales
#[derive(Debug, Clone, Copy)]
pub enum ModelCategory {
    /// 50K-500K params, minimal memory
    Nano,
    /// 500K-5M params, basic tasks
    Micro,
    /// 5M-20M params, good general use
    Small,
    /// 20M-100M params, high quality
    Base,
    /// 100M-500M params, needs large clusters
    Large,
}

impl ModelCategory {
    /// Minimum chips required for this model category
    pub fn min_chips(&self) -> usize {
        match self {
            Self::Nano => 1,
            Self::Micro => 5,
            Self::Small => 50,
            Self::Base => 200,
            Self::Large => 500,
        }
    }

    /// Parameter range
    pub fn param_range(&self) -> (usize, usize) {
        match self {
            Self::Nano => (50_000, 500_000),
            Self::Micro => (500_000, 5_000_000),
            Self::Small => (5_000_000, 20_000_000),
            Self::Base => (20_000_000, 100_000_000),
            Self::Large => (100_000_000, 500_000_000),
        }
    }

    /// Example models
    pub fn examples(&self) -> &'static str {
        match self {
            Self::Nano => "TinyBERT-nano, Custom embeddings",
            Self::Micro => "DistilBERT-tiny, MiniLM",
            Self::Small => "TinyLlama, Phi-nano",
            Self::Base => "Phi-1, GPT-2-Small",
            Self::Large => "Phi-2, LLaMA-7B (quantized)",
        }
    }

    /// What's possible with given chip count
    pub fn for_chip_count(chips: usize) -> Self {
        match chips {
            0..=4 => Self::Nano,
            5..=49 => Self::Micro,
            50..=199 => Self::Small,
            200..=499 => Self::Base,
            _ => Self::Large,
        }
    }
}

/// Hardware configuration for physical deployment
#[derive(Debug, Clone)]
pub struct HardwareConfig {
    /// Chips per PCB (physical board)
    pub chips_per_board: usize,
    /// Number of PCBs
    pub num_boards: usize,
    /// Communication bus
    pub bus_type: BusType,
    /// Power supply requirement (watts)
    pub power_supply_watts: f64,
    /// Recommended form factor
    pub form_factor: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub enum BusType {
    /// SPI - up to 40MHz, simple
    Spi,
    /// I2C - 400kHz standard, lower bandwidth
    I2c,
    /// UART mesh - flexible, medium speed
    Uart,
    /// Custom high-speed interconnect
    HighSpeed,
}

impl BusType {
    pub fn bandwidth_bytes_sec(&self) -> usize {
        match self {
            Self::Spi => 5_000_000,      // 5 MB/s typical
            Self::I2c => 50_000,          // 50 KB/s
            Self::Uart => 1_000_000,      // 1 MB/s at 10Mbaud
            Self::HighSpeed => 50_000_000, // Custom FPGA/ASIC
        }
    }
}

impl HardwareConfig {
    /// Recommended hardware for chip count
    pub fn for_cluster(chip_count: usize) -> Self {
        match chip_count {
            0..=25 => Self {
                chips_per_board: chip_count.min(10),
                num_boards: (chip_count + 9) / 10,
                bus_type: BusType::Spi,
                power_supply_watts: chip_count as f64 * 0.5 + 10.0,
                form_factor: "Single PCB or small rack",
            },
            26..=100 => Self {
                chips_per_board: 10,
                num_boards: (chip_count + 9) / 10,
                bus_type: BusType::Spi,
                power_supply_watts: chip_count as f64 * 0.5 + 25.0,
                form_factor: "1U rack mount (10 boards)",
            },
            101..=256 => Self {
                chips_per_board: 16,
                num_boards: (chip_count + 15) / 16,
                bus_type: BusType::Uart,
                power_supply_watts: chip_count as f64 * 0.5 + 50.0,
                form_factor: "2U-4U rack mount",
            },
            257..=500 => Self {
                chips_per_board: 20,
                num_boards: (chip_count + 19) / 20,
                bus_type: BusType::Uart,
                power_supply_watts: chip_count as f64 * 0.5 + 75.0,
                form_factor: "Full rack unit",
            },
            _ => Self {
                chips_per_board: 25,
                num_boards: (chip_count + 24) / 25,
                bus_type: BusType::HighSpeed,
                power_supply_watts: chip_count as f64 * 0.5 + 100.0,
                form_factor: "Multi-rack datacenter",
            },
        }
    }
}

/// Run complete analysis for 100-500 chip clusters
pub struct MediumScaleAnalyzer;

impl MediumScaleAnalyzer {
    /// Compare all standard medium-scale configurations
    pub fn full_analysis() -> HVec<(MediumClusterConfig, ScaleComparison), 8> {
        let mut results = HVec::new();

        for chips in [100, 144, 196, 256, 324, 400, 484, 500] {
            if chips <= MEDIUM_SCALE_MAX {
                let config = MediumClusterConfig::optimal_for(chips);
                let comparison = ScaleComparison::analyze(chips);
                let _ = results.push((config, comparison));
            }
        }

        results
    }

    /// Find optimal configuration for target throughput
    pub fn optimize_for_throughput(target_tokens_sec: f64) -> Option<MediumClusterConfig> {
        // Binary search in medium scale range
        let mut low = MEDIUM_SCALE_MIN;
        let mut high = MEDIUM_SCALE_MAX;
        let mut best: Option<MediumClusterConfig> = None;

        while low <= high {
            let mid = (low + high) / 2;
            let config = MediumClusterConfig::optimal_for(mid);

            if config.expected_throughput >= target_tokens_sec {
                best = Some(config);
                high = mid.saturating_sub(1);
            } else {
                low = mid + 1;
            }
        }

        best
    }

    /// Find optimal configuration for target cost
    pub fn optimize_for_budget(budget_usd: f64) -> MediumClusterConfig {
        let max_chips = (budget_usd / 4.0) as usize; // $4 per chip
        let clamped = max_chips.clamp(MEDIUM_SCALE_MIN, MEDIUM_SCALE_MAX);
        MediumClusterConfig::optimal_for(clamped)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimal_config_100() {
        let config = MediumClusterConfig::optimal_for(100);
        assert_eq!(config.clusters, 10);
        assert_eq!(config.chips_per_cluster, 10);
        assert!(config.expected_throughput > 40000.0); // 40K+ tok/s
        assert!(config.expected_efficiency > 0.5); // 50%+ efficiency
    }

    #[test]
    fn test_optimal_config_256() {
        let config = MediumClusterConfig::optimal_for(256);
        assert_eq!(config.clusters, 16);
        assert_eq!(config.chips_per_cluster, 16);
        assert!(config.expected_throughput > 60000.0); // 60K+ tok/s
    }

    #[test]
    fn test_scale_comparison() {
        let comparison = ScaleComparison::analyze(256);
        assert!(comparison.throughput_multiplier > 50.0); // 50x+ vs single chip
        assert!(comparison.vs_small_multiplier > 10.0);   // 10x+ vs 5 chips
    }

    #[test]
    fn test_model_categories() {
        assert_eq!(ModelCategory::for_chip_count(50).min_chips(), 50);
        assert_eq!(ModelCategory::for_chip_count(256).min_chips(), 200);
    }

    #[test]
    fn test_hardware_config() {
        let hw = HardwareConfig::for_cluster(256);
        assert_eq!(hw.chips_per_board, 16);
        assert_eq!(hw.num_boards, 16);
        assert!(hw.power_supply_watts > 100.0);
    }
}
