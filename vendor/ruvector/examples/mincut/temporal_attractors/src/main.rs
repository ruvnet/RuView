//! # Temporal Attractor Networks with MinCut Analysis
//!
//! This example demonstrates how networks evolve toward stable "attractor states"
//! and how minimum cut analysis helps detect convergence to these attractors.
//!
//! ## What are Temporal Attractors?
//!
//! In dynamical systems theory, an **attractor** is a state toward which a system
//! naturally evolves over time. Think of it like:
//! - A ball rolling into a valley (gravitational attractor)
//! - Water flowing to the lowest point (hydraulic attractor)
//! - A network reorganizing for optimal connectivity (topological attractor)
//!
//! ## Why This Matters for Swarms
//!
//! Multi-agent swarms naturally evolve toward stable configurations:
//! - **Optimal Attractor**: Maximum connectivity, robust communication
//! - **Fragmented Attractor**: Disconnected clusters, poor coordination
//! - **Oscillating Attractor**: Periodic patterns, unstable equilibrium
//!
//! ## How MinCut Detects Convergence
//!
//! The minimum cut value reveals the network's structural stability:
//! - **Increasing MinCut**: Network becoming more connected
//! - **Stable MinCut**: Attractor reached (equilibrium)
//! - **Decreasing MinCut**: Network fragmenting
//! - **Oscillating MinCut**: Periodic attractor (limit cycle)

use ruvector_mincut::prelude::*;
use std::time::Instant;

/// Represents different types of attractor basins a network can evolve toward
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttractorType {
    /// Network fragments into disconnected clusters (BAD for swarms)
    Fragmented,
    /// Network reaches maximum connectivity (IDEAL for swarms)
    Optimal,
    /// Network oscillates between states (UNSTABLE for swarms)
    Oscillating,
}

/// Tracks the state of the network at each time step
#[derive(Debug, Clone)]
pub struct NetworkSnapshot {
    /// Time step number
    pub step: usize,
    /// Minimum cut value at this step
    pub mincut: u64,
    /// Number of edges at this step
    pub edge_count: usize,
    /// Average connectivity (edges per node)
    pub avg_connectivity: f64,
    /// Time taken for this step (microseconds)
    pub step_duration_us: u64,
}

/// A temporal attractor network that evolves over time
///
/// The network dynamically adjusts its topology based on simple rules
/// that simulate how multi-agent systems naturally reorganize:
/// - Strengthen frequently-used connections
/// - Weaken rarely-used connections
/// - Add shortcuts for efficiency
/// - Remove redundant paths
pub struct AttractorNetwork {
    /// The underlying graph edges
    edges: Vec<(VertexId, VertexId, Weight)>,
    /// Number of nodes
    nodes: usize,
    /// Target attractor type
    attractor_type: AttractorType,
    /// History of network states
    history: Vec<NetworkSnapshot>,
    /// Current time step
    current_step: usize,
    /// Random seed for reproducibility
    seed: u64,
}

impl AttractorNetwork {
    /// Creates a new attractor network with the specified target behavior
    ///
    /// # Arguments
    /// * `nodes` - Number of nodes in the network
    /// * `attractor_type` - Target attractor basin
    /// * `seed` - Random seed for reproducibility
    pub fn new(nodes: usize, attractor_type: AttractorType, seed: u64) -> Self {
        // Initialize with a base ring topology (each node connected to neighbors)
        let mut edges = Vec::new();
        for i in 0..nodes {
            let next = (i + 1) % nodes;
            edges.push((i as VertexId, next as VertexId, 1.0));
        }

        Self {
            edges,
            nodes,
            attractor_type,
            history: Vec::new(),
            current_step: 0,
            seed,
        }
    }

    /// Evolves the network one time step toward its attractor
    ///
    /// This method implements the core dynamics that drive the network
    /// toward its target attractor state. Different attractor types use
    /// different evolution rules.
    pub fn evolve_step(&mut self) -> NetworkSnapshot {
        let step_start = Instant::now();

        match self.attractor_type {
            AttractorType::Optimal => self.evolve_toward_optimal(),
            AttractorType::Fragmented => self.evolve_toward_fragmented(),
            AttractorType::Oscillating => self.evolve_toward_oscillating(),
        }

        // Calculate current network metrics
        let mincut = self.calculate_mincut();
        let edge_count = self.edges.len();
        let node_count = self.nodes;
        let avg_connectivity = edge_count as f64 / node_count as f64;

        let snapshot = NetworkSnapshot {
            step: self.current_step,
            mincut,
            edge_count,
            avg_connectivity,
            step_duration_us: step_start.elapsed().as_micros() as u64,
        };

        self.history.push(snapshot.clone());
        self.current_step += 1;

        snapshot
    }

    /// Evolves toward maximum connectivity (optimal attractor)
    ///
    /// Strategy: Add shortcuts between distant nodes to increase connectivity
    fn evolve_toward_optimal(&mut self) {
        let n = self.nodes;

        // Add random shortcuts to increase connectivity
        // Use deterministic pseudo-random based on step and seed
        let rng_state = self.seed.wrapping_mul(self.current_step as u64 + 1);
        let u = (rng_state % n as u64) as VertexId;
        let v = ((rng_state / n as u64) % n as u64) as VertexId;

        if u != v && !self.has_edge(u, v) {
            // Add edge with increasing weight to simulate strengthening
            let weight = 1.0 + (self.current_step / 10) as f64;
            self.edges.push((u, v, weight));
        }

        // Strengthen existing edges to increase mincut
        if self.current_step % 3 == 0 {
            self.strengthen_random_edge();
        }
    }

    /// Evolves toward fragmentation (fragmented attractor)
    ///
    /// Strategy: Remove edges to create disconnected clusters
    fn evolve_toward_fragmented(&mut self) {
        // Remove random edges to fragment the network
        if self.current_step % 2 == 0 && !self.edges.is_empty() {
            let rng_state = self.seed.wrapping_mul(self.current_step as u64 + 1);
            let edge_idx = (rng_state % self.edges.len() as u64) as usize;

            // Weaken or remove the edge
            if let Some(edge) = self.edges.get_mut(edge_idx) {
                if edge.2 > 1.0 {
                    edge.2 -= 1.0;
                } else if edge_idx < self.edges.len() {
                    self.edges.remove(edge_idx);
                }
            }
        }
    }

    /// Evolves toward oscillation (oscillating attractor)
    ///
    /// Strategy: Alternate between adding and removing edges
    fn evolve_toward_oscillating(&mut self) {
        let n = self.nodes;

        // Oscillate: add edges on even steps, remove on odd steps
        if self.current_step % 2 == 0 {
            // Add phase
            let rng_state = self.seed.wrapping_mul(self.current_step as u64 + 1);
            let u = (rng_state % n as u64) as VertexId;
            let v = ((rng_state / n as u64) % n as u64) as VertexId;

            if u != v {
                self.edges.push((u, v, 2.0));
            }
        } else {
            // Remove phase
            if self.edges.len() > n {
                let rng_state = self.seed.wrapping_mul(self.current_step as u64 + 1);
                let edge_idx = (rng_state % self.edges.len() as u64) as usize;

                if edge_idx < self.edges.len() {
                    self.edges.remove(edge_idx);
                }
            }
        }
    }

    /// Calculates the minimum cut of the current network
    fn calculate_mincut(&self) -> u64 {
        if self.edges.is_empty() {
            return 0;
        }

        // Build a MinCut structure and compute
        match MinCutBuilder::new().with_edges(self.edges.clone()).build() {
            Ok(mincut) => mincut.min_cut_value() as u64,
            Err(_) => 0,
        }
    }

    /// Checks if an edge exists between two nodes
    fn has_edge(&self, u: VertexId, v: VertexId) -> bool {
        self.edges
            .iter()
            .any(|e| (e.0 == u && e.1 == v) || (e.0 == v && e.1 == u))
    }

    /// Strengthens a random edge
    fn strengthen_random_edge(&mut self) {
        if self.edges.is_empty() {
            return;
        }

        let rng_state = self.seed.wrapping_mul(self.current_step as u64 + 1);
        let edge_idx = (rng_state % self.edges.len() as u64) as usize;

        if let Some(edge) = self.edges.get_mut(edge_idx) {
            edge.2 += 1.0;
        }
    }

    /// Checks if the network has converged to its attractor
    ///
    /// Convergence is detected by analyzing the stability of mincut values
    /// over the last few steps.
    pub fn has_converged(&self, window: usize) -> bool {
        if self.history.len() < window {
            return false;
        }

        let recent = &self.history[self.history.len() - window..];
        let mincuts: Vec<u64> = recent.iter().map(|s| s.mincut).collect();

        // For optimal: mincut should be high and stable
        // For fragmented: mincut should be 0 or very low and stable
        // For oscillating: mincut should show periodic pattern

        match self.attractor_type {
            AttractorType::Optimal => {
                // Converged if mincut is high and not changing
                let avg = mincuts.iter().sum::<u64>() / mincuts.len() as u64;
                mincuts
                    .iter()
                    .all(|&mc| (mc as i64 - avg as i64).abs() <= 1)
            }
            AttractorType::Fragmented => {
                // Converged if mincut is 0 or very low
                mincuts.iter().all(|&mc| mc <= 1)
            }
            AttractorType::Oscillating => {
                // Converged if showing periodic pattern
                if mincuts.len() < 4 {
                    return false;
                }
                // Check for simple 2-period oscillation
                mincuts.chunks(2).all(|pair| {
                    if pair.len() == 2 {
                        (pair[0] as i64 - pair[1] as i64).abs() > 0
                    } else {
                        true
                    }
                })
            }
        }
    }

    /// Returns the network's history
    pub fn history(&self) -> &[NetworkSnapshot] {
        &self.history
    }

    /// Prints a summary of the network's evolution
    pub fn print_summary(&self) {
        println!("\n{}", "=".repeat(70));
        println!("TEMPORAL ATTRACTOR NETWORK SUMMARY");
        println!("{}", "=".repeat(70));
        println!("Attractor Type: {:?}", self.attractor_type);
        println!("Total Steps: {}", self.current_step);
        println!("Nodes: {}", self.nodes);
        println!("Current Edges: {}", self.edges.len());

        if let Some(first) = self.history.first() {
            if let Some(last) = self.history.last() {
                println!("\nMinCut Evolution:");
                println!("  Initial: {}", first.mincut);
                println!("  Final:   {}", last.mincut);
                println!("  Change:  {:+}", last.mincut as i64 - first.mincut as i64);

                println!("\nConnectivity Evolution:");
                println!("  Initial Avg: {:.2}", first.avg_connectivity);
                println!("  Final Avg:   {:.2}", last.avg_connectivity);

                let total_time: u64 = self.history.iter().map(|s| s.step_duration_us).sum();
                println!("\nPerformance:");
                println!("  Total Time: {:.2}ms", total_time as f64 / 1000.0);
                println!(
                    "  Avg Step:   {:.2}μs",
                    total_time as f64 / self.history.len() as f64
                );
            }
        }
        println!("{}", "=".repeat(70));
    }
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║    TEMPORAL ATTRACTOR NETWORKS WITH MINCUT ANALYSIS              ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("This example demonstrates how networks evolve toward stable states");
    println!("and how minimum cut analysis reveals structural convergence.\n");

    let nodes = 10;
    let max_steps = 50;
    let convergence_window = 10;

    // Run three different attractor scenarios
    let scenarios = vec![
        (
            AttractorType::Optimal,
            "Networks that want to maximize connectivity",
        ),
        (
            AttractorType::Fragmented,
            "Networks that fragment into clusters",
        ),
        (
            AttractorType::Oscillating,
            "Networks that oscillate between states",
        ),
    ];

    for (idx, (attractor_type, description)) in scenarios.into_iter().enumerate() {
        println!("\n┌─────────────────────────────────────────────────────────────────┐");
        println!("│ SCENARIO {}: {:?} Attractor", idx + 1, attractor_type);
        println!("│ {}", description);
        println!("└─────────────────────────────────────────────────────────────────┘\n");

        let mut network = AttractorNetwork::new(nodes, attractor_type, 12345 + idx as u64);

        println!("Step  | MinCut | Edges | Avg Conn | Time(μs) | Status");
        println!("------|--------|-------|----------|----------|------------------");

        for step in 0..max_steps {
            let snapshot = network.evolve_step();

            let status = if network.has_converged(convergence_window) {
                "✓ CONVERGED"
            } else {
                "  evolving..."
            };

            // Print every 5th step for readability
            if step % 5 == 0 || network.has_converged(convergence_window) {
                println!(
                    "{:5} | {:6} | {:5} | {:8.2} | {:8} | {}",
                    snapshot.step,
                    snapshot.mincut,
                    snapshot.edge_count,
                    snapshot.avg_connectivity,
                    snapshot.step_duration_us,
                    status
                );
            }

            if network.has_converged(convergence_window) && step > convergence_window {
                println!("\n✓ Attractor reached at step {}", step);
                break;
            }
        }

        network.print_summary();

        // Analyze the convergence pattern
        println!("\nConvergence Analysis:");
        let history = network.history();
        if history.len() >= 10 {
            let last_10: Vec<u64> = history.iter().rev().take(10).map(|s| s.mincut).collect();
            print!("Last 10 MinCuts: ");
            for (i, mc) in last_10.iter().rev().enumerate() {
                print!("{}", mc);
                if i < last_10.len() - 1 {
                    print!(" → ");
                }
            }
            println!();

            // Detect pattern
            let variance: f64 = {
                let mean = last_10.iter().sum::<u64>() as f64 / last_10.len() as f64;
                last_10
                    .iter()
                    .map(|&x| {
                        let diff = x as f64 - mean;
                        diff * diff
                    })
                    .sum::<f64>()
                    / last_10.len() as f64
            };

            println!("Variance: {:.2}", variance);
            if variance < 0.1 {
                println!("Pattern: STABLE (reached equilibrium)");
            } else if variance > 10.0 {
                println!("Pattern: OSCILLATING (limit cycle detected)");
            } else {
                println!("Pattern: TRANSITIONING (approaching attractor)");
            }
        }
    }

    println!("\n╔═══════════════════════════════════════════════════════════════════╗");
    println!("║                        KEY INSIGHTS                               ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();
    println!("1. OPTIMAL ATTRACTORS: MinCut increases → better connectivity");
    println!("   • Ideal for swarm communication");
    println!("   • Fault-tolerant topology");
    println!("   • Maximum information flow");
    println!();
    println!("2. FRAGMENTED ATTRACTORS: MinCut decreases → network splits");
    println!("   • Poor for swarm coordination");
    println!("   • Isolated clusters form");
    println!("   • Communication breakdown");
    println!();
    println!("3. OSCILLATING ATTRACTORS: MinCut fluctuates → periodic pattern");
    println!("   • Unstable equilibrium");
    println!("   • May indicate resonance");
    println!("   • Requires damping strategies");
    println!();
    println!("MinCut as a Convergence Indicator:");
    println!("• Stable MinCut → Attractor reached");
    println!("• Increasing MinCut → Strengthening network");
    println!("• Decreasing MinCut → Warning sign");
    println!("• Oscillating MinCut → Limit cycle detected");
    println!();
}
