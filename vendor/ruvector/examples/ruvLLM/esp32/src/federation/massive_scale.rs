//! Massive Scale Federation - 100s to Millions of Chips
//!
//! Hierarchical coordination for extreme-scale distributed inference.
//!
//! # Topology Options
//!
//! ```text
//! Flat (≤16 chips):     Hierarchical Tree (≤10K):     Hypercube (≤1M):
//!   ○─○─○─○─○             ┌───[Root]───┐               ○═══○
//!   │ │ │ │ │             │     │     │               ╱│   │╲
//!   └─┴─┴─┴─┘           [L1]  [L1]  [L1]             ○─┼───┼─○
//!                        │││   │││   │││             │ ○═══○ │
//!                       chips chips chips            ○═══════○
//! ```
//!
//! # Scaling Laws
//!
//! - **Pipeline**: O(n) throughput, O(1) latency per stage
//! - **Tree**: O(log n) coordination, O(n) compute
//! - **Hypercube**: O(log n) hops, O(n) total bandwidth
//! - **Torus**: O(√n) diameter, excellent locality

use heapless::Vec as HVec;
use super::protocol::ChipId;

/// Maximum depth for hierarchical topologies
pub const MAX_TREE_DEPTH: usize = 20; // 2^20 = 1M chips
/// Maximum children per node in tree
pub const MAX_CHILDREN: usize = 16;
/// Maximum nodes at any level
pub const MAX_LEVEL_NODES: usize = 64;

/// Large-scale topology types
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MassiveTopology {
    /// Flat mesh - up to ~16 chips
    FlatMesh { size: usize },
    /// Binary tree - scales to millions
    BinaryTree { depth: usize },
    /// K-ary tree with configurable fanout
    KaryTree { depth: usize, fanout: usize },
    /// Hypercube - O(log n) diameter
    Hypercube { dimensions: usize },
    /// 2D Torus - good for spatial locality
    Torus2D { width: usize, height: usize },
    /// 3D Torus - even better scaling
    Torus3D { x: usize, y: usize, z: usize },
    /// Butterfly network - FFT-like communication
    Butterfly { stages: usize },
    /// Hierarchical pipeline - practical for real deployments
    HierarchicalPipeline {
        clusters: usize,      // Number of clusters
        chips_per_cluster: usize,
    },
}

impl MassiveTopology {
    /// Total number of chips in topology
    pub fn total_chips(&self) -> usize {
        match *self {
            Self::FlatMesh { size } => size,
            Self::BinaryTree { depth } => (1 << depth) - 1,
            Self::KaryTree { depth, fanout } => {
                // (k^(d+1) - 1) / (k - 1)
                if fanout == 1 { depth + 1 }
                else { (fanout.pow(depth as u32 + 1) - 1) / (fanout - 1) }
            }
            Self::Hypercube { dimensions } => 1 << dimensions,
            Self::Torus2D { width, height } => width * height,
            Self::Torus3D { x, y, z } => x * y * z,
            Self::Butterfly { stages } => stages * (1 << stages),
            Self::HierarchicalPipeline { clusters, chips_per_cluster } => {
                clusters * chips_per_cluster
            }
        }
    }

    /// Network diameter (max hops between any two nodes)
    pub fn diameter(&self) -> usize {
        match *self {
            Self::FlatMesh { size } => size - 1,
            Self::BinaryTree { depth } => 2 * depth,
            Self::KaryTree { depth, .. } => 2 * depth,
            Self::Hypercube { dimensions } => dimensions,
            Self::Torus2D { width, height } => width / 2 + height / 2,
            Self::Torus3D { x, y, z } => x / 2 + y / 2 + z / 2,
            Self::Butterfly { stages } => stages,
            Self::HierarchicalPipeline { chips_per_cluster, .. } => {
                chips_per_cluster + 2 // Within cluster + up + down
            }
        }
    }

    /// Bisection bandwidth (edges crossing middle cut)
    pub fn bisection_bandwidth(&self) -> usize {
        match *self {
            Self::FlatMesh { .. } => 1,
            Self::BinaryTree { .. } => 1, // Root is bottleneck
            Self::KaryTree { fanout, .. } => fanout,
            Self::Hypercube { dimensions } => 1 << (dimensions - 1),
            Self::Torus2D { width, height } => 2 * width.min(height),
            Self::Torus3D { x, y, z } => 2 * x.min(y).min(z) * x.min(y).min(z),
            Self::Butterfly { stages } => 1 << (stages - 1),
            Self::HierarchicalPipeline { clusters, .. } => clusters,
        }
    }

    /// Recommended topology for given chip count
    pub fn recommended(chip_count: usize) -> Self {
        match chip_count {
            0..=16 => Self::FlatMesh { size: chip_count },
            17..=256 => Self::HierarchicalPipeline {
                clusters: (chip_count as f64).sqrt().ceil() as usize,
                chips_per_cluster: (chip_count as f64).sqrt().ceil() as usize,
            },
            257..=10_000 => {
                // Use hierarchical pipeline for medium scale
                let clusters = (chip_count as f64).sqrt().ceil() as usize;
                let per_cluster = (chip_count + clusters - 1) / clusters;
                Self::HierarchicalPipeline {
                    clusters,
                    chips_per_cluster: per_cluster,
                }
            }
            10_001..=1_000_000 => {
                // Hypercube for large scale
                let dims = (chip_count as f64).log2().ceil() as usize;
                Self::Hypercube { dimensions: dims }
            }
            _ => {
                // Millions+ : 3D Torus
                let side = (chip_count as f64).cbrt().ceil() as usize;
                Self::Torus3D { x: side, y: side, z: side }
            }
        }
    }
}

/// Scaling configuration for massive clusters
#[derive(Debug, Clone)]
pub struct MassiveScaleConfig {
    /// Topology type
    pub topology: MassiveTopology,
    /// Layers of model
    pub total_layers: usize,
    /// Embedding dimension
    pub embed_dim: usize,
    /// Communication latency per hop (microseconds)
    pub hop_latency_us: usize,
    /// Bandwidth per link (bytes/sec)
    pub link_bandwidth: usize,
    /// Computation time per layer (microseconds)
    pub layer_compute_us: usize,
    /// Enable speculative execution
    pub speculative: bool,
    /// Speculation depth (tokens to draft)
    pub spec_depth: usize,
    /// Enable gradient checkpointing for memory
    pub gradient_checkpointing: bool,
    /// Fault tolerance level (0=none, 1=retry, 2=redundancy)
    pub fault_tolerance: u8,
}

impl Default for MassiveScaleConfig {
    fn default() -> Self {
        Self {
            topology: MassiveTopology::HierarchicalPipeline {
                clusters: 10,
                chips_per_cluster: 10,
            },
            total_layers: 32,
            embed_dim: 64,
            hop_latency_us: 10,      // SPI latency
            link_bandwidth: 10_000_000, // 10 MB/s
            layer_compute_us: 4000,   // 4ms per layer on ESP32
            speculative: true,
            spec_depth: 4,
            gradient_checkpointing: false,
            fault_tolerance: 1,
        }
    }
}

/// Performance projection for massive scale
#[derive(Debug, Clone)]
pub struct ScaleProjection {
    /// Total chips
    pub total_chips: usize,
    /// Throughput in tokens/sec
    pub throughput_tokens_sec: f64,
    /// Latency per token in milliseconds
    pub latency_ms: f64,
    /// Memory per chip in KB
    pub memory_per_chip_kb: f64,
    /// Total model parameters supportable
    pub max_parameters: usize,
    /// Efficiency (vs linear scaling)
    pub efficiency: f64,
    /// Communication overhead percentage
    pub comm_overhead_pct: f64,
    /// Estimated power in watts
    pub power_watts: f64,
    /// Estimated cost in USD
    pub cost_usd: f64,
}

/// Massive scale simulator
pub struct MassiveScaleSimulator {
    config: MassiveScaleConfig,
}

impl MassiveScaleSimulator {
    pub fn new(config: MassiveScaleConfig) -> Self {
        Self { config }
    }

    /// Project performance for current configuration
    pub fn project(&self) -> ScaleProjection {
        let chips = self.config.topology.total_chips();
        let diameter = self.config.topology.diameter();
        let bisection = self.config.topology.bisection_bandwidth();

        // Compute distribution
        let layers_per_chip = (self.config.total_layers as f64 / chips as f64).max(0.1);
        let compute_per_chip_us = layers_per_chip * self.config.layer_compute_us as f64;

        // Communication cost
        let activation_size = self.config.embed_dim * 4; // INT8 with some overhead
        let comm_time_us = (activation_size as f64 / self.config.link_bandwidth as f64)
            * 1_000_000.0
            * diameter as f64;

        // Pipeline efficiency
        let pipeline_stages = chips.min(self.config.total_layers);
        let bubble_overhead = (pipeline_stages - 1) as f64 / pipeline_stages as f64;

        // Speculative multiplier
        let spec_multiplier = if self.config.speculative {
            1.0 + (self.config.spec_depth as f64 - 1.0) * 0.7 // 70% acceptance
        } else {
            1.0
        };

        // Throughput calculation
        let base_throughput = 1_000_000.0 / compute_per_chip_us.max(1.0);
        let comm_factor = 1.0 / (1.0 + comm_time_us / compute_per_chip_us.max(1.0));
        let efficiency = (1.0 - bubble_overhead * 0.15) * comm_factor;
        let throughput = base_throughput * pipeline_stages as f64 * efficiency * spec_multiplier;

        // Latency
        let latency_us = compute_per_chip_us * pipeline_stages as f64 + comm_time_us;
        let latency_ms = latency_us / 1000.0;

        // Memory
        let base_memory_kb = 119.0; // Single chip baseline
        let memory_per_chip = base_memory_kb / (chips as f64).sqrt().max(1.0);

        // Max parameters
        let params_per_chip = (memory_per_chip * 1024.0 * 0.7) as usize; // 70% for weights
        let max_parameters = params_per_chip * chips;

        // Communication overhead
        let comm_overhead = comm_time_us / (compute_per_chip_us + comm_time_us) * 100.0;

        // Power and cost estimates
        let power_per_chip = 0.5; // 500mW per ESP32
        let cost_per_chip = 4.0;  // $4 per ESP32

        ScaleProjection {
            total_chips: chips,
            throughput_tokens_sec: throughput,
            latency_ms,
            memory_per_chip_kb: memory_per_chip,
            max_parameters,
            efficiency,
            comm_overhead_pct: comm_overhead,
            power_watts: power_per_chip * chips as f64,
            cost_usd: cost_per_chip * chips as f64,
        }
    }

    /// Run scaling study across multiple configurations
    pub fn scaling_study(&self, chip_counts: &[usize]) -> HVec<ScaleProjection, 32> {
        let mut results = HVec::new();

        for &count in chip_counts {
            let topology = MassiveTopology::recommended(count);
            let config = MassiveScaleConfig {
                topology,
                ..self.config.clone()
            };
            let sim = MassiveScaleSimulator::new(config);
            let _ = results.push(sim.project());
        }

        results
    }

    /// Find optimal configuration for target throughput
    pub fn optimize_for_throughput(&self, target_tokens_sec: f64) -> MassiveScaleConfig {
        let mut best_config = self.config.clone();
        let mut best_efficiency = 0.0;

        // Try different chip counts
        for power in 2..=20 {
            let chips = 1 << power;

            for &topology in &[
                MassiveTopology::KaryTree { depth: power, fanout: 4 },
                MassiveTopology::Hypercube { dimensions: power },
                MassiveTopology::HierarchicalPipeline {
                    clusters: 1 << (power / 2),
                    chips_per_cluster: 1 << (power - power / 2),
                },
            ] {
                if topology.total_chips() < 4 { continue; }

                let config = MassiveScaleConfig {
                    topology,
                    ..self.config.clone()
                };
                let sim = MassiveScaleSimulator::new(config.clone());
                let proj = sim.project();

                if proj.throughput_tokens_sec >= target_tokens_sec {
                    let efficiency = proj.throughput_tokens_sec / (proj.total_chips as f64);
                    if efficiency > best_efficiency {
                        best_efficiency = efficiency;
                        best_config = config;
                    }
                }
            }
        }

        best_config
    }
}

/// Distributed coordinator for massive scale
pub struct DistributedCoordinator {
    /// This node's ID
    node_id: u32,
    /// Parent node (None if root)
    parent: Option<u32>,
    /// Child nodes
    children: HVec<u32, MAX_CHILDREN>,
    /// Sibling nodes (same level)
    siblings: HVec<u32, MAX_CHILDREN>,
    /// Current level in hierarchy
    level: u8,
    /// Total levels
    total_levels: u8,
    /// Local state
    local_state: NodeState,
}

/// State of a node in the distributed system
#[derive(Debug, Clone, Default)]
pub struct NodeState {
    /// Tokens processed
    pub tokens_processed: u64,
    /// Current load (0-255)
    pub load: u8,
    /// Last heartbeat (ticks)
    pub last_heartbeat: u32,
    /// Active flag
    pub active: bool,
    /// Current sequence position being processed
    pub seq_position: u32,
    /// Error count
    pub errors: u16,
}

impl DistributedCoordinator {
    /// Create coordinator for position in tree
    pub fn new(node_id: u32, total_nodes: usize, topology: MassiveTopology) -> Self {
        let (parent, children, siblings, level, total_levels) =
            Self::compute_neighbors(node_id, total_nodes, topology);

        Self {
            node_id,
            parent,
            children,
            siblings,
            level,
            total_levels,
            local_state: NodeState { active: true, ..Default::default() },
        }
    }

    fn compute_neighbors(
        node_id: u32,
        total_nodes: usize,
        topology: MassiveTopology
    ) -> (Option<u32>, HVec<u32, MAX_CHILDREN>, HVec<u32, MAX_CHILDREN>, u8, u8) {
        let mut children = HVec::new();
        let mut siblings = HVec::new();

        match topology {
            MassiveTopology::BinaryTree { depth } |
            MassiveTopology::KaryTree { depth, fanout: 2 } => {
                let level = (node_id + 1).ilog2() as u8;
                let parent = if node_id == 0 { None } else { Some((node_id - 1) / 2) };

                let left = 2 * node_id + 1;
                let right = 2 * node_id + 2;
                if (left as usize) < total_nodes {
                    let _ = children.push(left);
                }
                if (right as usize) < total_nodes {
                    let _ = children.push(right);
                }

                // Sibling
                if node_id > 0 {
                    let sib = if node_id % 2 == 1 { node_id + 1 } else { node_id - 1 };
                    if (sib as usize) < total_nodes {
                        let _ = siblings.push(sib);
                    }
                }

                (parent, children, siblings, level, depth as u8)
            }
            MassiveTopology::Hypercube { dimensions } => {
                // In hypercube, neighbors differ by one bit
                let level = node_id.count_ones() as u8;
                for d in 0..dimensions {
                    let neighbor = node_id ^ (1 << d);
                    if (neighbor as usize) < total_nodes {
                        if neighbor < node_id {
                            // Could be parent
                        }
                        let _ = siblings.push(neighbor);
                    }
                }
                (None, children, siblings, level, dimensions as u8)
            }
            MassiveTopology::HierarchicalPipeline { clusters, chips_per_cluster } => {
                let cluster_id = node_id as usize / chips_per_cluster;
                let local_id = node_id as usize % chips_per_cluster;
                let level = local_id as u8;

                // Parent is previous in pipeline
                let parent = if local_id > 0 {
                    Some(node_id - 1)
                } else if cluster_id > 0 {
                    // Cross-cluster: last node of previous cluster
                    Some((cluster_id * chips_per_cluster - 1) as u32)
                } else {
                    None
                };

                // Child is next in pipeline
                if local_id + 1 < chips_per_cluster {
                    let _ = children.push(node_id + 1);
                } else if cluster_id + 1 < clusters {
                    // Cross-cluster
                    let _ = children.push(((cluster_id + 1) * chips_per_cluster) as u32);
                }

                (parent, children, siblings, level, chips_per_cluster as u8)
            }
            _ => {
                // Default: linear chain
                let parent = if node_id > 0 { Some(node_id - 1) } else { None };
                if ((node_id + 1) as usize) < total_nodes {
                    let _ = children.push(node_id + 1);
                }
                (parent, children, siblings, node_id as u8, total_nodes as u8)
            }
        }
    }

    /// Check if this node is root
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Check if this node is leaf
    pub fn is_leaf(&self) -> bool {
        self.children.is_empty()
    }

    /// Get nodes to send to for broadcast
    pub fn broadcast_targets(&self) -> &[u32] {
        &self.children
    }

    /// Get node to send to for aggregation (reduce)
    pub fn reduce_target(&self) -> Option<u32> {
        self.parent
    }

    /// Update local state
    pub fn update_state(&mut self, tokens: u64, load: u8) {
        self.local_state.tokens_processed = tokens;
        self.local_state.load = load;
        self.local_state.last_heartbeat = self.local_state.last_heartbeat.wrapping_add(1);
    }

    /// Get aggregate statistics (for root to report)
    pub fn aggregate_stats(&self, child_stats: &[NodeState]) -> NodeState {
        let mut agg = self.local_state.clone();
        for child in child_stats {
            agg.tokens_processed += child.tokens_processed;
            agg.load = agg.load.saturating_add(child.load / (child_stats.len() as u8).max(1));
            agg.errors += child.errors;
        }
        agg
    }
}

/// Gossip protocol for state synchronization at massive scale
pub struct GossipProtocol {
    /// Known node states (sampled)
    known_states: HVec<(u32, NodeState), 64>,
    /// Fanout for gossip
    fanout: usize,
    /// Round number
    round: u32,
}

impl GossipProtocol {
    pub fn new(fanout: usize) -> Self {
        Self {
            known_states: HVec::new(),
            fanout,
            round: 0,
        }
    }

    /// Select random nodes for gossip
    pub fn select_gossip_targets(&self, my_id: u32, total_nodes: usize, seed: u32) -> HVec<u32, 8> {
        let mut targets = HVec::new();
        let mut rng = seed.wrapping_mul(1103515245).wrapping_add(my_id);

        for _ in 0..self.fanout.min(8) {
            rng = rng.wrapping_mul(1103515245).wrapping_add(12345);
            let target = (rng % total_nodes as u32) as u32;
            if target != my_id && !targets.contains(&target) {
                let _ = targets.push(target);
            }
        }

        targets
    }

    /// Merge received state
    pub fn merge_state(&mut self, node_id: u32, state: NodeState) {
        // Update or insert
        for (id, s) in self.known_states.iter_mut() {
            if *id == node_id {
                *s = state;
                return;
            }
        }
        // Insert new
        if self.known_states.len() < 64 {
            let _ = self.known_states.push((node_id, state));
        } else {
            // Replace oldest (simple LRU)
            self.known_states[0] = (node_id, state);
        }
    }

    /// Get estimated cluster health
    pub fn cluster_health(&self) -> f32 {
        if self.known_states.is_empty() {
            return 1.0;
        }
        let active = self.known_states.iter().filter(|(_, s)| s.active).count();
        active as f32 / self.known_states.len() as f32
    }
}

/// Fault tolerance manager
pub struct FaultTolerance {
    /// Redundancy level (1 = no redundancy, 2 = pairs, 3 = triples)
    redundancy: u8,
    /// Failed node IDs
    failed_nodes: HVec<u32, 64>,
    /// Backup assignments (primary -> backup)
    backups: HVec<(u32, u32), 32>,
}

impl FaultTolerance {
    pub fn new(redundancy: u8) -> Self {
        Self {
            redundancy: redundancy.max(1),
            failed_nodes: HVec::new(),
            backups: HVec::new(),
        }
    }

    /// Mark node as failed
    pub fn mark_failed(&mut self, node_id: u32) {
        if !self.failed_nodes.contains(&node_id) {
            let _ = self.failed_nodes.push(node_id);
        }
    }

    /// Get backup for failed node
    pub fn get_backup(&self, failed_id: u32) -> Option<u32> {
        self.backups.iter()
            .find(|(primary, _)| *primary == failed_id)
            .map(|(_, backup)| *backup)
    }

    /// Assign backups for nodes
    pub fn assign_backups(&mut self, total_nodes: usize) {
        if self.redundancy < 2 { return; }

        for i in 0..total_nodes {
            let backup = (i + total_nodes / 2) % total_nodes;
            if self.backups.len() < 32 {
                let _ = self.backups.push((i as u32, backup as u32));
            }
        }
    }

    /// Check if node is available (not failed)
    pub fn is_available(&self, node_id: u32) -> bool {
        !self.failed_nodes.contains(&node_id)
    }

    /// Get failure rate
    pub fn failure_rate(&self, total_nodes: usize) -> f32 {
        self.failed_nodes.len() as f32 / total_nodes as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topology_sizing() {
        assert_eq!(MassiveTopology::BinaryTree { depth: 10 }.total_chips(), 1023);
        assert_eq!(MassiveTopology::Hypercube { dimensions: 10 }.total_chips(), 1024);
        assert_eq!(MassiveTopology::Torus2D { width: 100, height: 100 }.total_chips(), 10_000);
    }

    #[test]
    fn test_scaling_projection() {
        let config = MassiveScaleConfig {
            topology: MassiveTopology::HierarchicalPipeline {
                clusters: 10,
                chips_per_cluster: 10,
            },
            ..Default::default()
        };

        let sim = MassiveScaleSimulator::new(config);
        let proj = sim.project();

        assert_eq!(proj.total_chips, 100);
        assert!(proj.throughput_tokens_sec > 1000.0);
        assert!(proj.efficiency > 0.5);

        println!("100 chips: {:.0} tok/s, {:.1}% efficiency",
            proj.throughput_tokens_sec, proj.efficiency * 100.0);
    }

    #[test]
    fn test_massive_scale() {
        let chip_counts = [5, 100, 1000, 10_000, 100_000, 1_000_000];

        for &count in &chip_counts {
            let topology = MassiveTopology::recommended(count);
            let config = MassiveScaleConfig {
                topology,
                ..Default::default()
            };
            let sim = MassiveScaleSimulator::new(config);
            let proj = sim.project();

            println!("{:>10} chips: {:>12.0} tok/s, {:>6.1}% eff, ${:.0}",
                count, proj.throughput_tokens_sec, proj.efficiency * 100.0, proj.cost_usd);
        }
    }

    #[test]
    fn test_distributed_coordinator() {
        let coord = DistributedCoordinator::new(
            5,
            100,
            MassiveTopology::BinaryTree { depth: 7 }
        );

        assert!(!coord.is_root());
        println!("Node 5: parent={:?}, children={:?}", coord.parent, coord.children);
    }

    #[test]
    fn test_gossip_protocol() {
        let mut gossip = GossipProtocol::new(3);

        let targets = gossip.select_gossip_targets(5, 1000, 42);
        assert!(!targets.is_empty());
        assert!(!targets.contains(&5)); // Shouldn't include self

        gossip.merge_state(10, NodeState { active: true, ..Default::default() });
        assert_eq!(gossip.cluster_health(), 1.0);
    }
}
