//! # Strange Loop Self-Organizing Swarms
//!
//! This example demonstrates Hofstadter's "strange loops" - where a system's
//! self-observation creates emergent self-organization and intelligence.
//!
//! The MetaSwarm observes its own connectivity using min-cut analysis, then
//! reorganizes itself based on what it discovers. This creates a feedback loop:
//! "I am weak here" → "I will strengthen here" → "Now I am strong"
//!
//! Run: `cargo run --example strange_loop`

use std::collections::HashMap;

// ============================================================================
// SIMPLE GRAPH IMPLEMENTATION
// ============================================================================

/// A simple undirected weighted graph
#[derive(Debug, Clone)]
struct Graph {
    vertices: Vec<u64>,
    edges: HashMap<(u64, u64), f64>,
    adjacency: HashMap<u64, Vec<(u64, f64)>>,
}

impl Graph {
    fn new() -> Self {
        Self {
            vertices: Vec::new(),
            edges: HashMap::new(),
            adjacency: HashMap::new(),
        }
    }

    fn add_vertex(&mut self, v: u64) {
        if !self.vertices.contains(&v) {
            self.vertices.push(v);
            self.adjacency.insert(v, Vec::new());
        }
    }

    fn add_edge(&mut self, u: u64, v: u64, weight: f64) {
        self.add_vertex(u);
        self.add_vertex(v);
        let key = if u < v { (u, v) } else { (v, u) };
        self.edges.insert(key, weight);
        self.adjacency.get_mut(&u).unwrap().push((v, weight));
        self.adjacency.get_mut(&v).unwrap().push((u, weight));
    }

    fn degree(&self, v: u64) -> usize {
        self.adjacency.get(&v).map(|a| a.len()).unwrap_or(0)
    }

    fn weighted_degree(&self, v: u64) -> f64 {
        self.adjacency
            .get(&v)
            .map(|adj| adj.iter().map(|(_, w)| w).sum())
            .unwrap_or(0.0)
    }

    /// Approximate min-cut using minimum weighted degree
    fn approx_mincut(&self) -> f64 {
        self.vertices
            .iter()
            .map(|&v| self.weighted_degree(v))
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(0.0)
    }

    /// Find vertices with lowest connectivity (critical points)
    fn find_weak_vertices(&self) -> Vec<u64> {
        let min_degree = self
            .vertices
            .iter()
            .map(|&v| self.degree(v))
            .min()
            .unwrap_or(0);

        self.vertices
            .iter()
            .filter(|&&v| self.degree(v) == min_degree)
            .copied()
            .collect()
    }

    fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    fn edge_count(&self) -> usize {
        self.edges.len()
    }
}

// ============================================================================
// STRANGE LOOP SWARM
// ============================================================================

/// Self-model: predictions about own behavior
#[derive(Debug, Clone)]
struct SelfModel {
    /// Predicted min-cut value
    predicted_mincut: f64,
    /// Predicted weak vertices
    predicted_weak: Vec<u64>,
    /// Confidence in predictions (0.0 - 1.0)
    confidence: f64,
    /// History of prediction errors
    errors: Vec<f64>,
}

impl SelfModel {
    fn new() -> Self {
        Self {
            predicted_mincut: 0.0,
            predicted_weak: Vec::new(),
            confidence: 0.5,
            errors: Vec::new(),
        }
    }

    /// Update model based on observation
    fn update(&mut self, actual_mincut: f64, actual_weak: &[u64]) {
        // Calculate prediction error
        let error = (self.predicted_mincut - actual_mincut).abs();
        self.errors.push(error);

        // Update confidence based on error
        if error < 0.5 {
            self.confidence = (self.confidence + 0.1).min(1.0);
        } else {
            self.confidence = (self.confidence - 0.1).max(0.1);
        }

        // Simple prediction: expect similar values next time
        self.predicted_mincut = actual_mincut;
        self.predicted_weak = actual_weak.to_vec();
    }
}

/// Observation record
#[derive(Debug, Clone)]
struct Observation {
    iteration: usize,
    mincut: f64,
    weak_vertices: Vec<u64>,
    action_taken: String,
}

/// Action the swarm can take on itself
#[derive(Debug, Clone)]
enum Action {
    Strengthen(Vec<u64>), // Add edges to these vertices
    Redistribute,         // Balance connectivity
    Stabilize,            // Do nothing - optimal state
}

/// A swarm that observes and reorganizes itself through strange loops
struct MetaSwarm {
    graph: Graph,
    self_model: SelfModel,
    observations: Vec<Observation>,
    iteration: usize,
    stability_threshold: f64,
}

impl MetaSwarm {
    fn new(num_agents: usize) -> Self {
        let mut graph = Graph::new();

        // Create initial ring topology
        for i in 0..num_agents as u64 {
            graph.add_edge(i, (i + 1) % num_agents as u64, 1.0);
        }

        Self {
            graph,
            self_model: SelfModel::new(),
            observations: Vec::new(),
            iteration: 0,
            stability_threshold: 0.1,
        }
    }

    /// The main strange loop: observe → model → decide → act
    fn think(&mut self) -> bool {
        self.iteration += 1;

        println!("\n╔══════════════════════════════════════════════════════════╗");
        println!(
            "║  ITERATION {} - STRANGE LOOP CYCLE                       ",
            self.iteration
        );
        println!("╚══════════════════════════════════════════════════════════╝");

        // STEP 1: OBSERVE SELF
        println!("\n📡 Step 1: Self-Observation");
        let current_mincut = self.graph.approx_mincut();
        let weak_vertices = self.graph.find_weak_vertices();

        println!("   Min-cut value: {:.2}", current_mincut);
        println!("   Weak vertices: {:?}", weak_vertices);
        println!(
            "   Graph: {} vertices, {} edges",
            self.graph.vertex_count(),
            self.graph.edge_count()
        );

        // STEP 2: UPDATE SELF-MODEL
        println!("\n🧠 Step 2: Update Self-Model");
        let predicted = self.self_model.predicted_mincut;
        let error = (predicted - current_mincut).abs();

        self.self_model.update(current_mincut, &weak_vertices);

        println!("   Predicted min-cut: {:.2}", predicted);
        println!("   Actual min-cut: {:.2}", current_mincut);
        println!("   Prediction error: {:.2}", error);
        println!(
            "   Model confidence: {:.1}%",
            self.self_model.confidence * 100.0
        );

        // STEP 3: DECIDE REORGANIZATION
        println!("\n🤔 Step 3: Decide Reorganization");
        let action = self.decide();
        let action_str = match &action {
            Action::Strengthen(v) => format!("Strengthen {:?}", v),
            Action::Redistribute => "Redistribute".to_string(),
            Action::Stabilize => "Stabilize (optimal)".to_string(),
        };
        println!("   Decision: {}", action_str);

        // STEP 4: APPLY REORGANIZATION
        println!("\n⚡ Step 4: Apply Reorganization");
        let changed = self.apply_action(&action);

        if changed {
            let new_mincut = self.graph.approx_mincut();
            println!(
                "   New min-cut: {:.2} (Δ = {:.2})",
                new_mincut,
                new_mincut - current_mincut
            );
        } else {
            println!("   No changes applied (stable state)");
        }

        // Record observation
        self.observations.push(Observation {
            iteration: self.iteration,
            mincut: current_mincut,
            weak_vertices: weak_vertices.clone(),
            action_taken: action_str,
        });

        // Check for convergence
        let converged = self.check_convergence();
        if converged {
            println!("\n✨ STRANGE LOOP CONVERGED!");
            println!("   The swarm has reached self-organized stability.");
        }

        converged
    }

    /// Decide what action to take based on self-observation
    fn decide(&self) -> Action {
        let mincut = self.graph.approx_mincut();
        let weak = self.graph.find_weak_vertices();

        // Decision logic based on self-knowledge
        if mincut < 2.0 {
            // Very weak - strengthen urgently
            Action::Strengthen(weak)
        } else if mincut < 4.0 && !weak.is_empty() {
            // Somewhat weak - strengthen weak points
            Action::Strengthen(weak)
        } else if self.self_model.confidence > 0.8 && mincut > 3.0 {
            // High confidence, good connectivity - stable
            Action::Stabilize
        } else {
            // Redistribute for better balance
            Action::Redistribute
        }
    }

    /// Apply the chosen action to reorganize
    fn apply_action(&mut self, action: &Action) -> bool {
        match action {
            Action::Strengthen(vertices) => {
                let n = self.graph.vertex_count() as u64;
                for &v in vertices {
                    // Connect to a vertex far away
                    let target = (v + n / 2) % n;
                    if self.graph.degree(v) < 4 {
                        self.graph.add_edge(v, target, 1.0);
                        println!("   Added edge: {} -- {}", v, target);
                    }
                }
                !vertices.is_empty()
            }
            Action::Redistribute => {
                // Find most connected and least connected
                let max_v = self
                    .graph
                    .vertices
                    .iter()
                    .max_by_key(|&&v| self.graph.degree(v))
                    .copied();
                let min_v = self
                    .graph
                    .vertices
                    .iter()
                    .min_by_key(|&&v| self.graph.degree(v))
                    .copied();

                if let (Some(max), Some(min)) = (max_v, min_v) {
                    if self.graph.degree(max) > self.graph.degree(min) + 1 {
                        self.graph.add_edge(min, max, 0.5);
                        println!("   Redistributed: {} -- {}", min, max);
                        return true;
                    }
                }
                false
            }
            Action::Stabilize => false,
        }
    }

    /// Check if the strange loop has converged
    fn check_convergence(&self) -> bool {
        if self.observations.len() < 3 {
            return false;
        }

        // Check if min-cut has stabilized
        let recent: Vec<f64> = self
            .observations
            .iter()
            .rev()
            .take(3)
            .map(|o| o.mincut)
            .collect();

        let variance: f64 = {
            let mean = recent.iter().sum::<f64>() / recent.len() as f64;
            recent.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / recent.len() as f64
        };

        variance < self.stability_threshold && self.self_model.confidence > 0.7
    }

    /// Print the journey summary
    fn print_summary(&self) {
        println!("\n{:═^60}", " STRANGE LOOP JOURNEY ");
        println!("\nIteration | Min-Cut | Action");
        println!("{}", "-".repeat(60));

        for obs in &self.observations {
            println!(
                "{:^9} | {:^7.2} | {}",
                obs.iteration, obs.mincut, obs.action_taken
            );
        }

        if let (Some(first), Some(last)) = (self.observations.first(), self.observations.last()) {
            println!("\n📊 Summary:");
            println!("   Starting min-cut: {:.2}", first.mincut);
            println!("   Final min-cut: {:.2}", last.mincut);
            println!("   Improvement: {:.2}", last.mincut - first.mincut);
            println!("   Iterations: {}", self.iteration);
            println!(
                "   Final confidence: {:.1}%",
                self.self_model.confidence * 100.0
            );
        }
    }
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║       STRANGE LOOP SELF-ORGANIZING SWARMS                  ║");
    println!("║       Hofstadter's Self-Reference in Action                ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    println!("\n📖 Concept: A swarm that observes itself and reorganizes");
    println!("   based on what it discovers about its own structure.\n");
    println!("   This creates emergent intelligence through self-reference.");

    // Create a swarm of 10 agents
    let mut swarm = MetaSwarm::new(10);

    // Run the strange loop until convergence or max iterations
    let max_iterations = 15;
    let mut converged = false;

    for _ in 0..max_iterations {
        if swarm.think() {
            converged = true;
            break;
        }
    }

    // Print summary
    swarm.print_summary();

    if converged {
        println!("\n✅ The swarm achieved self-organized stability!");
        println!("   Through self-observation and self-modification,");
        println!("   it evolved into a robust configuration.");
    } else {
        println!("\n⚠️  Max iterations reached.");
        println!("   The swarm is still evolving.");
    }

    println!("\n🔮 Key Insight: The strange loop creates intelligence");
    println!("   not from complex rules, but from simple self-reference.");
    println!("   'I observe myself' → 'I change' → 'I observe the change'");
}
