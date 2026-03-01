//! # Living Simulation
//!
//! Not simulations that predict outcomes.
//! Simulations that maintain internal stability while being perturbed.
//!
//! Examples:
//! - Economic simulations that resist collapse and show where stress accumulates
//! - Climate models that expose fragile boundaries rather than forecasts
//! - Social simulations that surface tipping points before they happen
//!
//! You are no longer modeling reality. You are modeling fragility.

use std::collections::HashMap;

/// A node in the living simulation - responds to stress, not commands
#[derive(Clone, Debug)]
pub struct SimNode {
    pub id: usize,
    /// Current stress level (0-1)
    pub stress: f64,
    /// Resilience - ability to absorb stress without propagating
    pub resilience: f64,
    /// Threshold at which node becomes fragile
    pub fragility_threshold: f64,
    /// Whether this node is currently a fragility point
    pub is_fragile: bool,
    /// Accumulated damage from sustained stress
    pub damage: f64,
}

/// An edge representing stress transmission
#[derive(Clone, Debug)]
pub struct SimEdge {
    pub from: usize,
    pub to: usize,
    /// How much stress transmits across this edge (0-1)
    pub transmission: f64,
    /// Current load on this edge
    pub load: f64,
    /// Breaking point - edge fails above this load
    pub breaking_point: f64,
    pub broken: bool,
}

/// A living simulation that reveals fragility through perturbation
pub struct LivingSimulation {
    nodes: HashMap<usize, SimNode>,
    edges: Vec<SimEdge>,

    /// Global tension (mincut-derived)
    tension: f64,

    /// History of fragility points
    fragility_history: Vec<FragilityEvent>,

    /// Simulation time
    tick: usize,

    /// Stability threshold - below this, system is stable
    stability_threshold: f64,
}

#[derive(Clone, Debug)]
pub struct FragilityEvent {
    pub tick: usize,
    pub node_id: usize,
    pub stress_level: f64,
    pub was_cascade: bool,
}

#[derive(Debug)]
pub struct SimulationState {
    pub tick: usize,
    pub tension: f64,
    pub fragile_nodes: Vec<usize>,
    pub broken_edges: usize,
    pub avg_stress: f64,
    pub max_stress: f64,
    pub stability: f64,
}

impl LivingSimulation {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            edges: Vec::new(),
            tension: 0.0,
            fragility_history: Vec::new(),
            tick: 0,
            stability_threshold: 0.3,
        }
    }

    /// Build an economic simulation
    pub fn economic(num_sectors: usize) -> Self {
        let mut sim = Self::new();

        // Create sectors as nodes
        for i in 0..num_sectors {
            sim.nodes.insert(
                i,
                SimNode {
                    id: i,
                    stress: 0.0,
                    resilience: 0.3 + (i as f64 * 0.1).min(0.5),
                    fragility_threshold: 0.6,
                    is_fragile: false,
                    damage: 0.0,
                },
            );
        }

        // Create interconnections (supply chains)
        for i in 0..num_sectors {
            for j in (i + 1)..num_sectors {
                if (i + j) % 3 == 0 {
                    // Selective connections
                    sim.edges.push(SimEdge {
                        from: i,
                        to: j,
                        transmission: 0.4,
                        load: 0.0,
                        breaking_point: 0.8,
                        broken: false,
                    });
                }
            }
        }

        sim
    }

    /// Apply external perturbation to a node
    pub fn perturb(&mut self, node_id: usize, stress_delta: f64) {
        if let Some(node) = self.nodes.get_mut(&node_id) {
            node.stress = (node.stress + stress_delta).clamp(0.0, 1.0);
        }
    }

    /// Advance simulation one tick - stress propagates, fragility emerges
    pub fn tick(&mut self) -> SimulationState {
        self.tick += 1;

        // Phase 1: Propagate stress through edges
        let mut stress_deltas: HashMap<usize, f64> = HashMap::new();

        for edge in &mut self.edges {
            if edge.broken {
                continue;
            }

            if let (Some(from_node), Some(to_node)) =
                (self.nodes.get(&edge.from), self.nodes.get(&edge.to))
            {
                let stress_diff = from_node.stress - to_node.stress;
                let transmitted = stress_diff * edge.transmission;

                edge.load = transmitted.abs();

                if edge.load > edge.breaking_point {
                    edge.broken = true;
                } else {
                    *stress_deltas.entry(edge.to).or_insert(0.0) += transmitted;
                    *stress_deltas.entry(edge.from).or_insert(0.0) -= transmitted * 0.5;
                }
            }
        }

        // Phase 2: Apply stress deltas with resilience
        for (node_id, delta) in stress_deltas {
            if let Some(node) = self.nodes.get_mut(&node_id) {
                let absorbed = delta * (1.0 - node.resilience);
                node.stress = (node.stress + absorbed).clamp(0.0, 1.0);

                // Accumulate damage from sustained stress
                if node.stress > node.fragility_threshold {
                    node.damage += 0.01;
                }
            }
        }

        // Phase 3: Update fragility status
        let mut cascade_detected = false;
        for node in self.nodes.values_mut() {
            let was_fragile = node.is_fragile;
            node.is_fragile = node.stress > node.fragility_threshold;

            if node.is_fragile && !was_fragile {
                cascade_detected = true;
            }
        }

        // Phase 4: Record fragility events
        for node in self.nodes.values() {
            if node.is_fragile {
                self.fragility_history.push(FragilityEvent {
                    tick: self.tick,
                    node_id: node.id,
                    stress_level: node.stress,
                    was_cascade: cascade_detected,
                });
            }
        }

        // Phase 5: Compute global tension
        self.tension = self.compute_tension();

        // Phase 6: Self-healing attempt
        self.attempt_healing();

        self.state()
    }

    /// Get current state
    pub fn state(&self) -> SimulationState {
        let stresses: Vec<f64> = self.nodes.values().map(|n| n.stress).collect();
        let fragile: Vec<usize> = self
            .nodes
            .values()
            .filter(|n| n.is_fragile)
            .map(|n| n.id)
            .collect();
        let broken_edges = self.edges.iter().filter(|e| e.broken).count();

        SimulationState {
            tick: self.tick,
            tension: self.tension,
            fragile_nodes: fragile,
            broken_edges,
            avg_stress: stresses.iter().sum::<f64>() / stresses.len().max(1) as f64,
            max_stress: stresses.iter().cloned().fold(0.0, f64::max),
            stability: 1.0 - self.tension,
        }
    }

    /// Identify tipping points - nodes near fragility threshold
    pub fn tipping_points(&self) -> Vec<(usize, f64)> {
        let mut points: Vec<(usize, f64)> = self
            .nodes
            .values()
            .filter(|n| !n.is_fragile)
            .map(|n| {
                let distance_to_fragility = n.fragility_threshold - n.stress;
                (n.id, distance_to_fragility)
            })
            .collect();

        points.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        points.into_iter().take(3).collect()
    }

    /// Find stress accumulation zones
    pub fn stress_accumulation_zones(&self) -> Vec<(usize, f64)> {
        let mut zones: Vec<(usize, f64)> = self
            .nodes
            .values()
            .map(|n| (n.id, n.stress + n.damage))
            .collect();

        zones.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        zones.into_iter().take(3).collect()
    }

    fn compute_tension(&self) -> f64 {
        // Tension based on fragility spread and edge stress
        let fragile_ratio = self.nodes.values().filter(|n| n.is_fragile).count() as f64
            / self.nodes.len().max(1) as f64;

        let edge_stress: f64 = self
            .edges
            .iter()
            .filter(|e| !e.broken)
            .map(|e| e.load / e.breaking_point)
            .sum::<f64>()
            / self.edges.len().max(1) as f64;

        let broken_ratio =
            self.edges.iter().filter(|e| e.broken).count() as f64 / self.edges.len().max(1) as f64;

        (fragile_ratio * 0.4 + edge_stress * 0.3 + broken_ratio * 0.3).min(1.0)
    }

    fn attempt_healing(&mut self) {
        // Only heal when tension is low enough
        if self.tension > self.stability_threshold {
            return;
        }

        // Gradually reduce stress in non-fragile nodes
        for node in self.nodes.values_mut() {
            if !node.is_fragile {
                node.stress *= 0.95;
                node.damage *= 0.99;
            }
        }
    }
}

fn main() {
    println!("=== Living Simulation ===\n");
    println!("You are no longer modeling reality. You are modeling fragility.\n");

    let mut sim = LivingSimulation::economic(8);

    println!("Economic simulation: 8 sectors, interconnected supply chains\n");

    // Run baseline
    println!("Phase 1: Baseline stability");
    for _ in 0..5 {
        sim.tick();
    }
    let baseline = sim.state();
    println!(
        "  Tension: {:.2}, Avg stress: {:.2}\n",
        baseline.tension, baseline.avg_stress
    );

    // Apply perturbation
    println!("Phase 2: Supply shock to sector 0");
    sim.perturb(0, 0.7);

    println!("Tick | Tension | Fragile | Broken | Tipping Points");
    println!("-----|---------|---------|--------|---------------");

    for _ in 0..20 {
        let state = sim.tick();
        let tipping = sim.tipping_points();
        let tipping_str: String = tipping
            .iter()
            .map(|(id, dist)| format!("{}:{:.2}", id, dist))
            .collect::<Vec<_>>()
            .join(", ");

        println!(
            "{:4} | {:.2}    | {:7} | {:6} | {}",
            state.tick,
            state.tension,
            state.fragile_nodes.len(),
            state.broken_edges,
            tipping_str
        );

        // Additional perturbation mid-crisis
        if state.tick == 12 {
            println!("     >>> Additional shock to sector 3");
            sim.perturb(3, 0.5);
        }
    }

    let final_state = sim.state();

    println!("\n=== Fragility Analysis ===");
    println!("Stress accumulation zones:");
    for (id, stress) in sim.stress_accumulation_zones() {
        println!("  Sector {}: cumulative stress {:.2}", id, stress);
    }

    println!("\nFinal tipping points (nodes nearest to fragility):");
    for (id, distance) in sim.tipping_points() {
        println!("  Sector {}: {:.2} from threshold", id, distance);
    }

    println!("\nFragility events: {}", sim.fragility_history.len());
    let cascades = sim
        .fragility_history
        .iter()
        .filter(|e| e.was_cascade)
        .count();
    println!("Cascade events: {}", cascades);

    println!("\n\"Not predicting outcomes. Exposing fragile boundaries.\"");
}
