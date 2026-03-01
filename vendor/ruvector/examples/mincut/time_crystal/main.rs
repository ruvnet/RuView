//! Time Crystal Coordination Patterns
//!
//! This example demonstrates periodic, self-sustaining coordination patterns
//! inspired by time crystals in physics. Unlike normal crystals that have
//! repeating patterns in space, time crystals have repeating patterns in time.
//!
//! In this swarm coordination context, we create topologies that:
//! 1. Oscillate periodically between Ring → Star → Mesh → Ring
//! 2. Maintain stability without external energy input
//! 3. Self-heal when perturbations cause "melting"
//! 4. Verify structural integrity using minimum cut analysis

use ruvector_mincut::prelude::*;

/// Number of agents in the swarm
const SWARM_SIZE: u64 = 12;

/// Time crystal period (number of ticks per full cycle)
const CRYSTAL_PERIOD: usize = 9;

/// Phases of the time crystal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Ring,
    StarFormation,
    Star,
    MeshFormation,
    Mesh,
    MeshDecay,
    StarReformation,
    RingReformation,
    RingStable,
}

impl Phase {
    /// Get the expected minimum cut value for this phase
    fn expected_mincut(&self, swarm_size: u64) -> f64 {
        match self {
            Phase::Ring | Phase::RingReformation | Phase::RingStable => {
                // Ring: each node has degree 2, mincut is 2
                2.0
            }
            Phase::StarFormation | Phase::StarReformation => {
                // Transitional: between previous and next topology
                2.0 // Conservative estimate during transition
            }
            Phase::Star => {
                // Star: hub node has degree (n-1), mincut is 1 (any edge from hub)
                1.0
            }
            Phase::MeshFormation | Phase::MeshDecay => {
                // Transitional: increasing connectivity
                (swarm_size - 1) as f64 / 2.0
            }
            Phase::Mesh => {
                // Complete mesh: mincut is (n-1) - the degree of any single node
                (swarm_size - 1) as f64
            }
        }
    }

    /// Get human-readable description
    fn description(&self) -> &'static str {
        match self {
            Phase::Ring => "Ring topology - each agent connected to 2 neighbors",
            Phase::StarFormation => "Transition: Ring → Star",
            Phase::Star => "Star topology - central hub with spokes",
            Phase::MeshFormation => "Transition: Star → Mesh",
            Phase::Mesh => "Mesh topology - all agents interconnected",
            Phase::MeshDecay => "Transition: Mesh → Star",
            Phase::StarReformation => "Transition: Star → Ring",
            Phase::RingReformation => "Transition: rebuilding Ring",
            Phase::RingStable => "Ring stabilized - completing cycle",
        }
    }

    /// Get the next phase in the cycle
    fn next(&self) -> Phase {
        match self {
            Phase::Ring => Phase::StarFormation,
            Phase::StarFormation => Phase::Star,
            Phase::Star => Phase::MeshFormation,
            Phase::MeshFormation => Phase::Mesh,
            Phase::Mesh => Phase::MeshDecay,
            Phase::MeshDecay => Phase::StarReformation,
            Phase::StarReformation => Phase::RingReformation,
            Phase::RingReformation => Phase::RingStable,
            Phase::RingStable => Phase::Ring,
        }
    }
}

/// Time Crystal Swarm - periodic coordination pattern
struct TimeCrystalSwarm {
    /// The coordination graph
    graph: DynamicGraph,
    /// Current phase in the crystal cycle
    current_phase: Phase,
    /// Tick counter
    tick: usize,
    /// History of mincut values
    mincut_history: Vec<f64>,
    /// History of phases
    phase_history: Vec<Phase>,
    /// Number of agents
    swarm_size: u64,
    /// Stability score (0.0 = melted, 1.0 = perfect crystal)
    stability: f64,
}

impl TimeCrystalSwarm {
    /// Create a new time crystal swarm
    fn new(swarm_size: u64) -> Self {
        let graph = DynamicGraph::new();

        // Initialize with ring topology
        Self::build_ring(&graph, swarm_size);

        TimeCrystalSwarm {
            graph,
            current_phase: Phase::Ring,
            tick: 0,
            mincut_history: Vec::new(),
            phase_history: vec![Phase::Ring],
            swarm_size,
            stability: 1.0,
        }
    }

    /// Build a ring topology
    fn build_ring(graph: &DynamicGraph, n: u64) {
        graph.clear();

        // Connect agents in a ring: 0-1-2-...-n-1-0
        for i in 0..n {
            let next = (i + 1) % n;
            let _ = graph.insert_edge(i, next, 1.0);
        }
    }

    /// Build a star topology
    fn build_star(graph: &DynamicGraph, n: u64) {
        graph.clear();

        // Agent 0 is the hub, connected to all others
        for i in 1..n {
            let _ = graph.insert_edge(0, i, 1.0);
        }
    }

    /// Build a mesh topology (complete graph)
    fn build_mesh(graph: &DynamicGraph, n: u64) {
        graph.clear();

        // Connect every agent to every other agent
        for i in 0..n {
            for j in (i + 1)..n {
                let _ = graph.insert_edge(i, j, 1.0);
            }
        }
    }

    /// Transition from one topology to another
    fn transition_topology(&mut self) {
        match self.current_phase {
            Phase::Ring => {
                // Stay in ring, prepare for transition
            }
            Phase::StarFormation => {
                // Build star topology
                Self::build_star(&self.graph, self.swarm_size);
            }
            Phase::Star => {
                // Stay in star
            }
            Phase::MeshFormation => {
                // Build mesh topology
                Self::build_mesh(&self.graph, self.swarm_size);
            }
            Phase::Mesh => {
                // Stay in mesh
            }
            Phase::MeshDecay => {
                // Transition back to star
                Self::build_star(&self.graph, self.swarm_size);
            }
            Phase::StarReformation => {
                // Stay in star before transitioning to ring
            }
            Phase::RingReformation => {
                // Build ring topology
                Self::build_ring(&self.graph, self.swarm_size);
            }
            Phase::RingStable => {
                // Stay in ring
            }
        }
    }

    /// Advance one time step
    fn tick(&mut self) -> Result<()> {
        self.tick += 1;

        // Compute current minimum cut
        let mincut_value = self.compute_mincut()?;
        self.mincut_history.push(mincut_value);

        // Check stability
        let expected = self.current_phase.expected_mincut(self.swarm_size);
        let deviation = (mincut_value - expected).abs() / expected.max(1.0);

        // Update stability score (exponential moving average)
        let stability_contribution = if deviation < 0.1 { 1.0 } else { 0.0 };
        self.stability = 0.9 * self.stability + 0.1 * stability_contribution;

        // Detect melting (loss of periodicity)
        if self.stability < 0.5 {
            println!("⚠️  WARNING: Time crystal melting detected!");
            println!("   Stability: {:.2}%", self.stability * 100.0);
            self.restabilize()?;
        }

        // Move to next phase
        self.current_phase = self.current_phase.next();
        self.phase_history.push(self.current_phase);

        // Transition topology
        self.transition_topology();

        Ok(())
    }

    /// Compute minimum cut of current topology
    fn compute_mincut(&self) -> Result<f64> {
        // Build a mincut analyzer
        let edges: Vec<(VertexId, VertexId, Weight)> = self
            .graph
            .edges()
            .iter()
            .map(|e| (e.source, e.target, e.weight))
            .collect();

        if edges.is_empty() {
            return Ok(f64::INFINITY);
        }

        let mincut = MinCutBuilder::new().exact().with_edges(edges).build()?;

        let value = mincut.min_cut_value();
        Ok(value)
    }

    /// Restabilize the crystal after melting
    fn restabilize(&mut self) -> Result<()> {
        println!("🔧 Restabilizing time crystal...");

        // Reset to known good state (ring)
        Self::build_ring(&self.graph, self.swarm_size);
        self.current_phase = Phase::Ring;
        self.stability = 1.0;

        println!("✓  Crystal restabilized");
        Ok(())
    }

    /// Verify crystal periodicity
    fn verify_periodicity(&self) -> bool {
        if self.mincut_history.len() < CRYSTAL_PERIOD * 2 {
            return true; // Not enough data yet
        }

        // Check if pattern repeats
        let n = self.mincut_history.len();
        let mut matches = 0;
        let mut total = 0;

        for i in 0..CRYSTAL_PERIOD.min(n / 2) {
            let current = self.mincut_history[n - 1 - i];
            let previous_cycle = self.mincut_history[n - 1 - i - CRYSTAL_PERIOD];

            let deviation = (current - previous_cycle).abs();
            if deviation < 0.5 {
                matches += 1;
            }
            total += 1;
        }

        matches as f64 / total as f64 > 0.7
    }

    /// Crystallize - establish the periodic pattern
    fn crystallize(&mut self, cycles: usize) -> Result<()> {
        println!("❄️  Crystallizing time pattern over {} cycles...\n", cycles);

        for cycle in 0..cycles {
            println!("═══ Cycle {} ═══", cycle + 1);

            for _step in 0..CRYSTAL_PERIOD {
                self.tick()?;

                let mincut = self.mincut_history.last().copied().unwrap_or(0.0);
                let expected = self.current_phase.expected_mincut(self.swarm_size);
                let status = if (mincut - expected).abs() < 0.5 {
                    "✓"
                } else {
                    "✗"
                };

                println!(
                    "  Tick {:2} | Phase: {:18} | MinCut: {:5.1} (expected {:5.1}) {}",
                    self.tick,
                    format!("{:?}", self.current_phase),
                    mincut,
                    expected,
                    status
                );
            }

            // Check periodicity after each cycle
            if cycle > 0 {
                let periodic = self.verify_periodicity();
                println!(
                    "\n  Periodicity: {} | Stability: {:.1}%\n",
                    if periodic {
                        "✓ VERIFIED"
                    } else {
                        "✗ BROKEN"
                    },
                    self.stability * 100.0
                );
            }
        }

        Ok(())
    }

    /// Get current statistics
    fn stats(&self) -> CrystalStats {
        CrystalStats {
            tick: self.tick,
            current_phase: self.current_phase,
            stability: self.stability,
            periodicity_verified: self.verify_periodicity(),
            avg_mincut: self.mincut_history.iter().sum::<f64>() / self.mincut_history.len() as f64,
        }
    }
}

/// Statistics about the time crystal
#[derive(Debug)]
struct CrystalStats {
    tick: usize,
    current_phase: Phase,
    stability: f64,
    periodicity_verified: bool,
    avg_mincut: f64,
}

fn main() -> Result<()> {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     TIME CRYSTAL COORDINATION PATTERNS                     ║");
    println!("║                                                            ║");
    println!("║  Periodic, self-sustaining swarm topologies that          ║");
    println!("║  oscillate without external energy, verified by            ║");
    println!("║  minimum cut analysis at each phase                        ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();

    println!("Swarm Configuration:");
    println!("  • Agents: {}", SWARM_SIZE);
    println!("  • Crystal Period: {} ticks", CRYSTAL_PERIOD);
    println!("  • Phase Sequence: Ring → Star → Mesh → Ring");
    println!();

    // Create time crystal swarm
    let mut swarm = TimeCrystalSwarm::new(SWARM_SIZE);

    // Demonstrate phase descriptions
    println!("Phase Descriptions:");
    for (i, phase) in [
        Phase::Ring,
        Phase::StarFormation,
        Phase::Star,
        Phase::MeshFormation,
        Phase::Mesh,
        Phase::MeshDecay,
        Phase::StarReformation,
        Phase::RingReformation,
        Phase::RingStable,
    ]
    .iter()
    .enumerate()
    {
        println!(
            "  {}. {:18} - {} (mincut: {})",
            i + 1,
            format!("{:?}", phase),
            phase.description(),
            phase.expected_mincut(SWARM_SIZE)
        );
    }
    println!();

    // Crystallize the pattern over 3 full cycles
    swarm.crystallize(3)?;

    // Display final statistics
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║ FINAL STATISTICS                                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let stats = swarm.stats();
    println!("\n  Total Ticks: {}", stats.tick);
    println!("  Current Phase: {:?}", stats.current_phase);
    println!("  Stability: {:.1}%", stats.stability * 100.0);
    println!(
        "  Periodicity: {}",
        if stats.periodicity_verified {
            "✓ VERIFIED"
        } else {
            "✗ BROKEN"
        }
    );
    println!("  Average MinCut: {:.2}", stats.avg_mincut);
    println!();

    // Demonstrate phase transition visualization
    println!("Phase History (last {} ticks):", CRYSTAL_PERIOD);
    let history_len = swarm.phase_history.len();
    let start = history_len.saturating_sub(CRYSTAL_PERIOD);

    for (i, (phase, mincut)) in swarm.phase_history[start..]
        .iter()
        .zip(swarm.mincut_history[start..].iter())
        .enumerate()
    {
        let expected = phase.expected_mincut(SWARM_SIZE);
        let bar_length = (*mincut / SWARM_SIZE as f64 * 40.0) as usize;
        let bar = "█".repeat(bar_length);

        println!(
            "  {:2}. {:18} {:5.1} {} {}",
            start + i + 1,
            format!("{:?}", phase),
            mincut,
            bar,
            if (*mincut - expected).abs() < 0.5 {
                "✓"
            } else {
                "✗"
            }
        );
    }

    println!("\n✓ Time crystal coordination complete!");
    println!("\nKey Insights:");
    println!("  • The swarm maintains periodic oscillations autonomously");
    println!("  • Each phase has a characteristic minimum cut signature");
    println!("  • Stability monitoring prevents degradation");
    println!("  • Pattern repeats without external energy input");
    println!();

    Ok(())
}
