//! Morphogenetic Network Growth Example
//!
//! This example demonstrates how complex network structures can emerge from
//! simple local growth rules, inspired by biological morphogenesis (embryonic development).
//!
//! Key concepts:
//! - Networks "grow" like organisms from a seed structure
//! - Local rules (gene expression analogy) create global patterns
//! - Growth signals diffuse across the network
//! - Connectivity-based rules: low mincut triggers growth, high degree triggers branching
//! - Network reaches maturity when stable

use ruvector_mincut::prelude::*;
use std::collections::HashMap;

/// Represents a network that grows organically based on local rules
struct MorphogeneticNetwork {
    /// The underlying graph structure
    graph: DynamicGraph,
    /// Growth signal strength at each node (0.0 to 1.0)
    growth_signals: HashMap<VertexId, f64>,
    /// Age of each node (cycles since creation)
    node_ages: HashMap<VertexId, usize>,
    /// Next vertex ID to assign
    next_vertex_id: VertexId,
    /// Current growth cycle
    cycle: usize,
    /// Maximum cycles before forced maturity
    max_cycles: usize,
    /// Maturity threshold (when growth stabilizes)
    maturity_threshold: f64,
}

impl MorphogeneticNetwork {
    /// Create a new morphogenetic network from a seed structure
    fn new(seed_nodes: usize, max_cycles: usize) -> Self {
        let graph = DynamicGraph::new();
        let mut growth_signals = HashMap::new();
        let mut node_ages = HashMap::new();

        // Create initial "embryo" - a small connected core
        let mut vertex_ids = Vec::new();
        for i in 0..seed_nodes {
            graph.add_vertex(i as VertexId);
            vertex_ids.push(i as VertexId);
            growth_signals.insert(i as VertexId, 1.0);
            node_ages.insert(i as VertexId, 0);
        }

        // Connect in a circular pattern for initial stability
        for i in 0..seed_nodes {
            let next = (i + 1) % seed_nodes;
            let _ = graph.insert_edge(i as VertexId, next as VertexId, 1.0);
        }

        // Add one cross-connection for interesting topology
        if seed_nodes >= 4 {
            let _ = graph.insert_edge(0, (seed_nodes / 2) as VertexId, 1.0);
        }

        MorphogeneticNetwork {
            graph,
            growth_signals,
            node_ages,
            next_vertex_id: seed_nodes as VertexId,
            cycle: 0,
            max_cycles,
            maturity_threshold: 0.1,
        }
    }

    /// Execute one growth cycle - the core of morphogenesis
    fn grow(&mut self) -> GrowthReport {
        self.cycle += 1;
        let mut report = GrowthReport::new(self.cycle);

        println!("\n🌱 Growth Cycle {} 🌱", self.cycle);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

        // Phase 1: Diffuse growth signals across edges
        self.diffuse_signals();

        // Phase 2: Age all nodes
        for age in self.node_ages.values_mut() {
            *age += 1;
        }

        // Phase 3: Apply growth rules at each node
        let nodes: Vec<VertexId> = self.graph.vertices();

        for &node in &nodes {
            let signal = *self.growth_signals.get(&node).unwrap_or(&0.0);
            let degree = self.graph.degree(node);

            // Rule 1: Low connectivity triggers new growth (cell division)
            // Check if this node is weakly connected (potential bottleneck)
            if signal > 0.5 && degree < 3 {
                if let Some(new_node) = self.spawn_node(node) {
                    report.nodes_spawned += 1;
                    println!(
                        "  🌿 Node {} spawned child {} (low connectivity: degree={})",
                        node, new_node, degree
                    );
                }
            }

            // Rule 2: High degree triggers branching (differentiation)
            if signal > 0.6 && degree > 5 {
                if let Some(new_node) = self.branch_node(node) {
                    report.branches_created += 1;
                    println!(
                        "  🌳 Node {} branched to {} (high degree: {})",
                        node, new_node, degree
                    );
                }
            }

            // Rule 3: Check mincut for growth decisions
            // Nodes in weak cuts should strengthen connectivity
            if signal > 0.4 {
                let mincut = self.compute_local_mincut(node);
                if mincut < 2.0 {
                    if let Some(new_node) = self.reinforce_connectivity(node) {
                        report.reinforcements += 1;
                        println!(
                            "  💪 Node {} reinforced (mincut={:.1}), added node {}",
                            node, mincut, new_node
                        );
                    }
                }
            }
        }

        // Phase 4: Compute network statistics
        let stats = self.graph.stats();
        report.total_nodes = stats.num_vertices;
        report.total_edges = stats.num_edges;
        report.avg_signal = self.average_signal();
        report.is_mature = self.is_mature();

        // Phase 5: Decay signals slightly (aging effect)
        for signal in self.growth_signals.values_mut() {
            *signal *= 0.9;
        }

        self.print_statistics(&report);
        report
    }

    /// Diffuse growth signals to neighboring nodes (like chemical gradients)
    fn diffuse_signals(&mut self) {
        let mut new_signals = HashMap::new();

        for &node in &self.graph.vertices() {
            let current_signal = *self.growth_signals.get(&node).unwrap_or(&0.0);
            let neighbors_data = self.graph.neighbors(node);
            let neighbors: Vec<VertexId> = neighbors_data.iter().map(|(n, _)| *n).collect();

            // Signal diffuses: node keeps 60%, shares 40% with neighbors
            let retention = current_signal * 0.6;

            // Receive signal from neighbors
            let received: f64 = neighbors
                .iter()
                .map(|&n| {
                    let n_signal = self.growth_signals.get(&n).unwrap_or(&0.0);
                    let n_degree = self.graph.degree(n).max(1);
                    n_signal * 0.4 / n_degree as f64
                })
                .sum();

            new_signals.insert(node, retention + received);
        }

        self.growth_signals = new_signals;
    }

    /// Spawn a new node connected to the parent (cell division)
    fn spawn_node(&mut self, parent: VertexId) -> Option<VertexId> {
        if self.graph.num_vertices() >= 50 {
            return None; // Prevent unlimited growth
        }

        let new_node = self.next_vertex_id;
        self.next_vertex_id += 1;

        self.graph.add_vertex(new_node);
        let _ = self.graph.insert_edge(parent, new_node, 1.0);

        // Child inherits partial signal from parent
        let parent_signal = *self.growth_signals.get(&parent).unwrap_or(&0.0);
        self.growth_signals.insert(new_node, parent_signal * 0.7);
        self.node_ages.insert(new_node, 0);

        // Connect to one of parent's neighbors for stability
        let parent_neighbors = self.graph.neighbors(parent);
        if !parent_neighbors.is_empty() {
            let target = parent_neighbors[0].0;
            let _ = self.graph.insert_edge(new_node, target, 1.0);
        }

        Some(new_node)
    }

    /// Create a branch from a highly connected node (differentiation)
    fn branch_node(&mut self, node: VertexId) -> Option<VertexId> {
        if self.graph.num_vertices() >= 50 {
            return None;
        }

        let new_node = self.next_vertex_id;
        self.next_vertex_id += 1;

        self.graph.add_vertex(new_node);
        let _ = self.graph.insert_edge(node, new_node, 1.0);

        // Branch gets lower signal (specialization)
        let node_signal = *self.growth_signals.get(&node).unwrap_or(&0.0);
        self.growth_signals.insert(new_node, node_signal * 0.5);
        self.node_ages.insert(new_node, 0);

        Some(new_node)
    }

    /// Reinforce connectivity in weak areas (strengthening)
    fn reinforce_connectivity(&mut self, node: VertexId) -> Option<VertexId> {
        if self.graph.num_vertices() >= 50 {
            return None;
        }

        let new_node = self.next_vertex_id;
        self.next_vertex_id += 1;

        self.graph.add_vertex(new_node);
        let _ = self.graph.insert_edge(node, new_node, 1.0);

        // Find a distant node to connect to (create new pathway)
        let neighbors_data = self.graph.neighbors(node);
        let neighbors: Vec<VertexId> = neighbors_data.iter().map(|(n, _)| *n).collect();

        for &candidate in &self.graph.vertices() {
            if candidate != node && candidate != new_node && !neighbors.contains(&candidate) {
                let _ = self.graph.insert_edge(new_node, candidate, 1.0);
                break;
            }
        }

        let node_signal = *self.growth_signals.get(&node).unwrap_or(&0.0);
        self.growth_signals.insert(new_node, node_signal * 0.8);
        self.node_ages.insert(new_node, 0);

        Some(new_node)
    }

    /// Compute local minimum cut value around a node
    fn compute_local_mincut(&self, node: VertexId) -> f64 {
        let degree = self.graph.degree(node);
        if degree == 0 {
            return 0.0;
        }

        // Simple heuristic: ratio of edges to potential edges
        let actual_edges = degree;
        let max_possible = self.graph.num_vertices() - 1;

        (actual_edges as f64 / max_possible.max(1) as f64) * 10.0
    }

    /// Calculate average growth signal across network
    fn average_signal(&self) -> f64 {
        if self.growth_signals.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.growth_signals.values().sum();
        sum / self.growth_signals.len() as f64
    }

    /// Check if network has reached maturity (stable state)
    fn is_mature(&self) -> bool {
        self.average_signal() < self.maturity_threshold || self.cycle >= self.max_cycles
    }

    /// Print detailed network statistics
    fn print_statistics(&self, report: &GrowthReport) {
        println!("\n  📊 Network Statistics:");
        println!(
            "     Nodes: {} (+{} spawned)",
            report.total_nodes, report.nodes_spawned
        );
        println!("     Edges: {}", report.total_edges);
        println!("     Branches: {} new", report.branches_created);
        println!("     Reinforcements: {}", report.reinforcements);
        println!("     Avg Growth Signal: {:.3}", report.avg_signal);
        println!("     Density: {:.3}", self.compute_density());

        if report.is_mature {
            println!("\n  ✨ NETWORK HAS REACHED MATURITY ✨");
        }
    }

    /// Compute network density
    fn compute_density(&self) -> f64 {
        let stats = self.graph.stats();
        let n = stats.num_vertices as f64;
        let m = stats.num_edges as f64;
        let max_edges = n * (n - 1.0) / 2.0;

        if max_edges > 0.0 {
            m / max_edges
        } else {
            0.0
        }
    }
}

/// Report of growth activity in a cycle
#[derive(Debug, Clone)]
struct GrowthReport {
    cycle: usize,
    nodes_spawned: usize,
    branches_created: usize,
    reinforcements: usize,
    total_nodes: usize,
    total_edges: usize,
    avg_signal: f64,
    is_mature: bool,
}

impl GrowthReport {
    fn new(cycle: usize) -> Self {
        GrowthReport {
            cycle,
            nodes_spawned: 0,
            branches_created: 0,
            reinforcements: 0,
            total_nodes: 0,
            total_edges: 0,
            avg_signal: 0.0,
            is_mature: false,
        }
    }
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════╗");
    println!("║        🧬 MORPHOGENETIC NETWORK GROWTH 🧬                ║");
    println!("║   Biological-Inspired Network Development Simulation      ║");
    println!("╚═══════════════════════════════════════════════════════════╝");

    println!("\n📖 Concept: Networks grow like biological organisms");
    println!("   - Start with a 'seed' structure (embryo)");
    println!("   - Local rules at each node (like gene expression)");
    println!("   - Growth signals diffuse (like morphogens)");
    println!("   - Simple rules create complex global patterns");

    println!("\n🧬 Growth Rules (Gene Expression Analogy):");
    println!("   1. Low Connectivity (mincut < 2) → Grow new nodes");
    println!("   2. High Degree (degree > 5) → Branch/Differentiate");
    println!("   3. Weak Cuts → Reinforce connectivity");
    println!("   4. Signals Diffuse → Coordinate growth");
    println!("   5. Aging → Signals decay over time");

    // Create seed network (the "embryo")
    let seed_size = 4;
    let max_cycles = 15;

    println!("\n🌱 Creating seed network with {} nodes...", seed_size);
    let mut network = MorphogeneticNetwork::new(seed_size, max_cycles);

    println!("   Initial structure: circular + cross-connection");
    println!("   Initial growth signals: 1.0 (maximum)");

    // Growth simulation
    let mut cycle = 0;
    let mut reports = Vec::new();

    while cycle < max_cycles {
        let report = network.grow();
        reports.push(report.clone());

        if report.is_mature {
            println!("\n🎉 Network reached maturity at cycle {}", cycle + 1);
            break;
        }

        cycle += 1;

        // Pause between cycles for readability
        std::thread::sleep(std::time::Duration::from_millis(500));
    }

    // Final summary
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                    FINAL SUMMARY                          ║");
    println!("╚═══════════════════════════════════════════════════════════╝");

    let final_report = reports.last().unwrap();

    println!("\n🌳 Network Development Complete!");
    println!("   Growth Cycles: {}", final_report.cycle);
    println!(
        "   Final Nodes: {} (started with {})",
        final_report.total_nodes, seed_size
    );
    println!("   Final Edges: {}", final_report.total_edges);
    println!(
        "   Growth Factor: {:.2}x",
        final_report.total_nodes as f64 / seed_size as f64
    );

    let total_spawned: usize = reports.iter().map(|r| r.nodes_spawned).sum();
    let total_branches: usize = reports.iter().map(|r| r.branches_created).sum();
    let total_reinforcements: usize = reports.iter().map(|r| r.reinforcements).sum();

    println!("\n📈 Growth Activity:");
    println!("   Total Nodes Spawned: {}", total_spawned);
    println!("   Total Branches: {}", total_branches);
    println!("   Total Reinforcements: {}", total_reinforcements);
    println!(
        "   Total Growth Events: {}",
        total_spawned + total_branches + total_reinforcements
    );

    println!("\n🧬 Biological Analogy:");
    println!("   - Seed → Embryo (initial structure)");
    println!("   - Signals → Morphogens (chemical gradients)");
    println!("   - Growth Rules → Gene Expression");
    println!("   - Spawning → Cell Division");
    println!("   - Branching → Cell Differentiation");
    println!("   - Maturity → Adult Organism");

    println!("\n💡 Key Insight:");
    println!("   Complex global network structure emerged from");
    println!("   simple local rules at each node. No central");
    println!("   controller - just distributed 'genetic' code!");

    println!("\n✨ This demonstrates how:");
    println!("   • Local rules → Global patterns");
    println!("   • Distributed decisions → Coherent structure");
    println!("   • Simple algorithms → Complex emergent behavior");
    println!("   • Biological principles → Network design");
}
