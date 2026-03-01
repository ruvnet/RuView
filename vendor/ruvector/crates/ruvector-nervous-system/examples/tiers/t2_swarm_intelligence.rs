//! # Tier 2: Swarm Intelligence Without Central Control
//!
//! IoT fleets, sensor meshes, distributed robotics.
//!
//! ## What Changes
//! - Local reflexes handle local events
//! - Coherence gates synchronize only when needed
//! - No always-on coordinator
//!
//! ## Why This Matters
//! - Scale without fragility
//! - Partial failure is normal, not fatal
//! - Intelligence emerges from coordination, not command
//!
//! This is where your architecture beats cloud-centric designs.

use std::collections::{HashMap, HashSet};
use std::f32::consts::PI;

/// A node in the swarm
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct NodeId(pub u32);

/// Message between swarm nodes
#[derive(Clone, Debug)]
pub struct SwarmMessage {
    pub from: NodeId,
    pub to: Option<NodeId>, // None = broadcast
    pub timestamp: u64,
    pub content: MessageContent,
    pub priority: u8,
}

#[derive(Clone, Debug)]
pub enum MessageContent {
    /// Sensory observation
    Observation { sensor_type: String, value: f32 },
    /// Coordination request
    CoordinationRequest { task_id: u64, urgency: f32 },
    /// Phase synchronization pulse
    PhasePulse { phase: f32, frequency: f32 },
    /// Local decision announcement
    LocalDecision { action: String, confidence: f32 },
    /// Collective decision vote
    Vote { proposal_id: u64, support: bool },
}

/// Local reflex controller for each node
pub struct LocalReflex {
    pub node_id: NodeId,
    pub threshold: f32,
    pub membrane_potential: f32,
    pub refractory_until: u64,
}

impl LocalReflex {
    pub fn new(node_id: NodeId, threshold: f32) -> Self {
        Self {
            node_id,
            threshold,
            membrane_potential: 0.0,
            refractory_until: 0,
        }
    }

    /// Process local observation, return action if threshold exceeded
    pub fn process(&mut self, value: f32, timestamp: u64) -> Option<String> {
        if timestamp < self.refractory_until {
            return None;
        }

        self.membrane_potential += value;
        self.membrane_potential *= 0.9; // Leak

        if self.membrane_potential > self.threshold {
            self.refractory_until = timestamp + 100;
            self.membrane_potential = 0.0;
            Some(format!("local_action_{}", self.node_id.0))
        } else {
            None
        }
    }
}

/// Coherence gate using Kuramoto oscillator model
pub struct CoherenceGate {
    pub phase: f32,
    pub natural_frequency: f32,
    pub coupling_strength: f32,
    pub neighbor_phases: HashMap<NodeId, f32>,
}

impl CoherenceGate {
    pub fn new(natural_frequency: f32, coupling_strength: f32) -> Self {
        Self {
            phase: rand_float() * 2.0 * PI,
            natural_frequency,
            coupling_strength,
            neighbor_phases: HashMap::new(),
        }
    }

    /// Update phase based on neighbor phases
    pub fn step(&mut self, dt: f32) {
        if self.neighbor_phases.is_empty() {
            self.phase += self.natural_frequency * dt;
            self.phase %= 2.0 * PI;
            return;
        }

        // Kuramoto model: dθ/dt = ω + (K/N) Σ sin(θ_j - θ_i)
        let mut phase_coupling = 0.0;
        for (_, neighbor_phase) in &self.neighbor_phases {
            phase_coupling += (neighbor_phase - self.phase).sin();
        }

        let d_phase = self.natural_frequency
            + self.coupling_strength * phase_coupling / self.neighbor_phases.len() as f32;

        self.phase += d_phase * dt;
        self.phase %= 2.0 * PI;
    }

    /// Receive phase from neighbor
    pub fn receive_phase(&mut self, from: NodeId, phase: f32) {
        self.neighbor_phases.insert(from, phase);
    }

    /// Check if we're synchronized enough to coordinate
    pub fn is_synchronized(&self, threshold: f32) -> bool {
        if self.neighbor_phases.is_empty() {
            return false;
        }

        // Compute order parameter (Kuramoto)
        let n = self.neighbor_phases.len() as f32;
        let sum_x: f32 = self.neighbor_phases.values().map(|p| p.cos()).sum();
        let sum_y: f32 = self.neighbor_phases.values().map(|p| p.sin()).sum();

        let r = (sum_x * sum_x + sum_y * sum_y).sqrt() / n;
        r > threshold
    }

    /// Compute communication gain to a specific neighbor
    pub fn communication_gain(&self, neighbor: &NodeId) -> f32 {
        match self.neighbor_phases.get(neighbor) {
            Some(neighbor_phase) => {
                // Higher gain when phases are aligned
                (1.0 + (neighbor_phase - self.phase).cos()) / 2.0
            }
            None => 0.0,
        }
    }
}

/// Collective decision making through emergent consensus
pub struct CollectiveDecision {
    pub proposal_id: u64,
    pub votes: HashMap<NodeId, bool>,
    pub quorum_fraction: f32,
    pub deadline: u64,
}

impl CollectiveDecision {
    pub fn new(proposal_id: u64, quorum_fraction: f32, deadline: u64) -> Self {
        Self {
            proposal_id,
            votes: HashMap::new(),
            quorum_fraction,
            deadline,
        }
    }

    pub fn record_vote(&mut self, node: NodeId, support: bool) {
        self.votes.insert(node, support);
    }

    pub fn result(&self, total_nodes: usize, current_time: u64) -> Option<bool> {
        let votes_needed = (total_nodes as f32 * self.quorum_fraction).ceil() as usize;

        if self.votes.len() >= votes_needed {
            let support_count = self.votes.values().filter(|&&v| v).count();
            Some(support_count > self.votes.len() / 2)
        } else if current_time > self.deadline {
            // Timeout - no quorum
            None
        } else {
            // Still waiting
            None
        }
    }
}

/// A single swarm node
pub struct SwarmNode {
    pub id: NodeId,
    pub reflex: LocalReflex,
    pub coherence: CoherenceGate,
    pub neighbors: HashSet<NodeId>,
    pub observations: Vec<(u64, f32)>,
    pub pending_decisions: HashMap<u64, CollectiveDecision>,
}

impl SwarmNode {
    pub fn new(id: u32) -> Self {
        Self {
            id: NodeId(id),
            reflex: LocalReflex::new(NodeId(id), 1.0),
            coherence: CoherenceGate::new(1.0, 0.5),
            neighbors: HashSet::new(),
            observations: Vec::new(),
            pending_decisions: HashMap::new(),
        }
    }

    /// Process incoming message
    pub fn receive(&mut self, msg: SwarmMessage, timestamp: u64) -> Vec<SwarmMessage> {
        let mut responses = Vec::new();

        match msg.content {
            MessageContent::Observation { value, .. } => {
                // Local reflex response
                if let Some(action) = self.reflex.process(value, timestamp) {
                    responses.push(SwarmMessage {
                        from: self.id.clone(),
                        to: None,
                        timestamp,
                        content: MessageContent::LocalDecision {
                            action,
                            confidence: 0.8,
                        },
                        priority: 1,
                    });
                }
            }
            MessageContent::PhasePulse { phase, .. } => {
                self.coherence.receive_phase(msg.from, phase);
            }
            MessageContent::CoordinationRequest { task_id, urgency } => {
                // Only respond if synchronized and urgent enough
                if self.coherence.is_synchronized(0.7) && urgency > 0.5 {
                    responses.push(SwarmMessage {
                        from: self.id.clone(),
                        to: Some(msg.from),
                        timestamp,
                        content: MessageContent::Vote {
                            proposal_id: task_id,
                            support: true,
                        },
                        priority: 2,
                    });
                }
            }
            MessageContent::Vote {
                proposal_id,
                support,
            } => {
                if let Some(decision) = self.pending_decisions.get_mut(&proposal_id) {
                    decision.record_vote(msg.from, support);
                }
            }
            _ => {}
        }

        responses
    }

    /// Generate phase synchronization pulse
    pub fn emit_phase_pulse(&self, timestamp: u64) -> SwarmMessage {
        SwarmMessage {
            from: self.id.clone(),
            to: None,
            timestamp,
            content: MessageContent::PhasePulse {
                phase: self.coherence.phase,
                frequency: self.coherence.natural_frequency,
            },
            priority: 0,
        }
    }

    /// Step simulation
    pub fn step(&mut self, dt: f32) {
        self.coherence.step(dt);
    }
}

/// The swarm network (only for simulation, not central control)
pub struct SwarmNetwork {
    pub nodes: HashMap<NodeId, SwarmNode>,
    pub message_queue: Vec<SwarmMessage>,
    pub timestamp: u64,
}

impl SwarmNetwork {
    pub fn new(num_nodes: usize, connectivity: f32) -> Self {
        let mut nodes = HashMap::new();

        for i in 0..num_nodes {
            let mut node = SwarmNode::new(i as u32);

            // Random neighbors based on connectivity
            for j in 0..num_nodes {
                if i != j && rand_float() < connectivity {
                    node.neighbors.insert(NodeId(j as u32));
                }
            }

            nodes.insert(NodeId(i as u32), node);
        }

        Self {
            nodes,
            message_queue: Vec::new(),
            timestamp: 0,
        }
    }

    /// Simulate one step
    pub fn step(&mut self, dt: f32) {
        self.timestamp += (dt * 1000.0) as u64;

        // Process message queue
        let messages = std::mem::take(&mut self.message_queue);
        for msg in messages {
            let targets: Vec<NodeId> = match &msg.to {
                Some(target) => vec![target.clone()],
                None => self.nodes.keys().cloned().collect(),
            };

            for target in targets {
                if target != msg.from {
                    if let Some(node) = self.nodes.get_mut(&target) {
                        let responses = node.receive(msg.clone(), self.timestamp);
                        self.message_queue.extend(responses);
                    }
                }
            }
        }

        // Step all nodes and emit phase pulses periodically
        let mut new_messages = Vec::new();
        for (_, node) in &mut self.nodes {
            node.step(dt);

            // Emit phase pulse every 100ms
            if self.timestamp % 100 == 0 {
                new_messages.push(node.emit_phase_pulse(self.timestamp));
            }
        }
        self.message_queue.extend(new_messages);
    }

    /// Inject observation at a node
    pub fn inject_observation(&mut self, node_id: &NodeId, value: f32) {
        self.message_queue.push(SwarmMessage {
            from: node_id.clone(),
            to: Some(node_id.clone()),
            timestamp: self.timestamp,
            content: MessageContent::Observation {
                sensor_type: "generic".to_string(),
                value,
            },
            priority: 1,
        });
    }

    /// Check synchronization level
    pub fn synchronization_order_parameter(&self) -> f32 {
        let n = self.nodes.len() as f32;
        let sum_x: f32 = self.nodes.values().map(|n| n.coherence.phase.cos()).sum();
        let sum_y: f32 = self.nodes.values().map(|n| n.coherence.phase.sin()).sum();

        (sum_x * sum_x + sum_y * sum_y).sqrt() / n
    }

    /// Count nodes that would respond to coordination
    pub fn responsive_nodes(&self, threshold: f32) -> usize {
        self.nodes
            .values()
            .filter(|n| n.coherence.is_synchronized(threshold))
            .count()
    }
}

fn rand_float() -> f32 {
    // Simple PRNG for example (not cryptographic)
    static mut SEED: u32 = 12345;
    unsafe {
        SEED = SEED.wrapping_mul(1103515245).wrapping_add(12345);
        (SEED as f32) / (u32::MAX as f32)
    }
}

fn main() {
    println!("=== Tier 2: Swarm Intelligence Without Central Control ===\n");

    // Create swarm with 100 nodes, 20% connectivity
    let mut swarm = SwarmNetwork::new(100, 0.2);

    println!("Swarm initialized: {} nodes", swarm.nodes.len());
    println!(
        "Initial synchronization: {:.2}",
        swarm.synchronization_order_parameter()
    );

    // Let the swarm synchronize
    println!("\nPhase synchronization emerging...");
    for step in 0..50 {
        swarm.step(0.1);

        if step % 10 == 0 {
            println!(
                "  Step {}: sync = {:.3}, responsive = {}",
                step,
                swarm.synchronization_order_parameter(),
                swarm.responsive_nodes(0.7)
            );
        }
    }

    println!(
        "\nFinal synchronization: {:.2}",
        swarm.synchronization_order_parameter()
    );
    println!(
        "Nodes ready for coordination: {}",
        swarm.responsive_nodes(0.7)
    );

    // Inject local event - triggers local reflex
    println!("\nInjecting local event at node 5...");
    swarm.inject_observation(&NodeId(5), 2.0);
    swarm.step(0.1);

    // Check for local decisions
    let decisions: usize = swarm
        .message_queue
        .iter()
        .filter(|m| matches!(m.content, MessageContent::LocalDecision { .. }))
        .count();
    println!("  Local decisions triggered: {}", decisions);

    // Simulate partial failure
    println!("\nSimulating partial failure (removing 30% of nodes)...");
    let nodes_to_remove: Vec<NodeId> = swarm.nodes.keys().take(30).cloned().collect();

    for node_id in nodes_to_remove {
        swarm.nodes.remove(&node_id);
    }

    println!("  Remaining nodes: {}", swarm.nodes.len());

    // Let swarm recover
    println!("\nRecovery phase...");
    for step in 0..30 {
        swarm.step(0.1);

        if step % 10 == 0 {
            println!(
                "  Step {}: sync = {:.3}, responsive = {}",
                step,
                swarm.synchronization_order_parameter(),
                swarm.responsive_nodes(0.7)
            );
        }
    }

    println!(
        "\nPost-failure synchronization: {:.2}",
        swarm.synchronization_order_parameter()
    );
    println!("System continues operating with reduced capacity");

    println!("\n=== Key Benefits ===");
    println!("- No central coordinator - emergent synchronization");
    println!("- Local reflexes handle local events");
    println!("- Coherence gates synchronize only when needed");
    println!("- Partial failure is normal, not catastrophic");
    println!("- Intelligence emerges from coordination, not command");
    println!("\nThis beats cloud-centric designs for scale and resilience.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_reflex() {
        let mut reflex = LocalReflex::new(NodeId(0), 1.0);

        // Below threshold
        assert!(reflex.process(0.3, 0).is_none());
        assert!(reflex.process(0.3, 1).is_none());

        // Accumulates and fires
        let result = reflex.process(1.0, 2);
        assert!(result.is_some());

        // Refractory
        assert!(reflex.process(2.0, 3).is_none());
    }

    #[test]
    fn test_coherence_synchronization() {
        let mut gate = CoherenceGate::new(1.0, 2.0);

        // Not synchronized without neighbors
        assert!(!gate.is_synchronized(0.5));

        // Add synchronized neighbors
        gate.receive_phase(NodeId(1), gate.phase);
        gate.receive_phase(NodeId(2), gate.phase + 0.1);

        assert!(gate.is_synchronized(0.9));
    }

    #[test]
    fn test_collective_decision() {
        let mut decision = CollectiveDecision::new(1, 0.5, 1000);

        // Not enough votes
        decision.record_vote(NodeId(0), true);
        assert!(decision.result(4, 0).is_none());

        // Quorum reached
        decision.record_vote(NodeId(1), true);
        assert_eq!(decision.result(4, 0), Some(true));
    }

    #[test]
    fn test_swarm_network_creation() {
        let swarm = SwarmNetwork::new(10, 0.3);
        assert_eq!(swarm.nodes.len(), 10);
    }
}
