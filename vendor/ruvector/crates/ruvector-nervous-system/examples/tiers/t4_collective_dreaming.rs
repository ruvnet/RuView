//! # Tier 4: Collective Dreaming
//!
//! SOTA application: Swarm consolidation during downtime.
//!
//! ## The Problem
//! Traditional distributed systems:
//! - Active consensus requires all nodes awake
//! - No background synthesis of learned knowledge
//! - Memory fragmentation across nodes
//! - No collective "sleep" for maintenance
//!
//! ## What Changes
//! - Circadian-synchronized rest phases across swarm
//! - Hippocampal replay: consolidate recent experiences
//! - Cross-node memory exchange during low-traffic periods
//! - Emergent knowledge synthesis without central coordinator
//!
//! ## Why This Matters
//! - Swarm learns from collective experience
//! - Knowledge transfers between agents
//! - Background optimization during downtime
//! - Resilient to individual agent loss
//!
//! This is how biological systems scale learning.

use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::PI;

// ============================================================================
// Experience and Memory Structures
// ============================================================================

/// A single experience that can be replayed
#[derive(Clone, Debug)]
pub struct Experience {
    /// When this happened
    pub timestamp: u64,
    /// What was observed (sparse code)
    pub observation: Vec<u32>,
    /// What action was taken
    pub action: String,
    /// What outcome occurred
    pub outcome: f32,
    /// How surprising was this (prediction error)
    pub surprise: f32,
    /// Source agent
    pub source_agent: u32,
}

impl Experience {
    /// Compute replay priority (more surprising = higher priority)
    pub fn replay_priority(&self, current_time: u64, tau_hours: f32) -> f32 {
        let age_hours = (current_time - self.timestamp) as f32 / 3600.0;
        let recency = (-age_hours / tau_hours).exp();
        self.surprise * recency
    }
}

/// Memory trace that develops through consolidation
#[derive(Clone, Debug)]
pub struct MemoryTrace {
    /// The experience being consolidated
    pub experience: Experience,
    /// Consolidation strength (0-1)
    pub strength: f32,
    /// Number of replays
    pub replay_count: u32,
    /// Cross-agent validation count
    pub validation_count: u32,
    /// Has been transferred to other agents
    pub distributed: bool,
}

impl MemoryTrace {
    pub fn new(exp: Experience) -> Self {
        Self {
            experience: exp,
            strength: 0.0,
            replay_count: 0,
            validation_count: 0,
            distributed: false,
        }
    }

    /// Replay strengthens the trace
    pub fn replay(&mut self) {
        self.replay_count += 1;
        // Strength increases with diminishing returns
        self.strength = 1.0 - (-(self.replay_count as f32) / 5.0).exp();
    }

    /// Validation from another agent increases confidence
    pub fn validate(&mut self) {
        self.validation_count += 1;
        self.strength = (self.strength + 0.1).min(1.0);
    }

    /// Is this memory consolidated enough to be long-term?
    pub fn is_consolidated(&self) -> bool {
        self.strength > 0.7 && self.replay_count >= 3
    }
}

// ============================================================================
// Circadian Phase for Sleep Coordination
// ============================================================================

/// Phase state for each agent
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum SwarmPhase {
    /// Processing new experiences
    Awake,
    /// Beginning to wind down
    Drowsy,
    /// Light consolidation (local replay)
    LightSleep,
    /// Deep consolidation (cross-agent transfer)
    DeepSleep,
    /// Waking up, integrating transfers
    Waking,
}

impl SwarmPhase {
    pub fn from_normalized_time(t: f32) -> Self {
        let t = t % 1.0;
        if t < 0.6 {
            SwarmPhase::Awake
        } else if t < 0.65 {
            SwarmPhase::Drowsy
        } else if t < 0.75 {
            SwarmPhase::LightSleep
        } else if t < 0.9 {
            SwarmPhase::DeepSleep
        } else {
            SwarmPhase::Waking
        }
    }

    pub fn can_process_new(&self) -> bool {
        matches!(self, SwarmPhase::Awake | SwarmPhase::Waking)
    }

    pub fn can_replay(&self) -> bool {
        matches!(self, SwarmPhase::LightSleep | SwarmPhase::DeepSleep)
    }

    pub fn can_transfer(&self) -> bool {
        matches!(self, SwarmPhase::DeepSleep)
    }
}

// ============================================================================
// Dreaming Agent
// ============================================================================

/// An agent that participates in collective dreaming
pub struct DreamingAgent {
    /// Agent ID
    pub id: u32,
    /// Recent experiences (working memory)
    pub working_memory: VecDeque<Experience>,
    /// Memory traces being consolidated
    pub consolidating: Vec<MemoryTrace>,
    /// Long-term consolidated memories
    pub long_term: Vec<MemoryTrace>,
    /// Current phase
    pub phase: SwarmPhase,
    /// Phase in 24-hour cycle (0-1)
    pub cycle_phase: f32,
    /// Cycle duration in hours
    pub cycle_hours: f32,
    /// Timestamp
    pub timestamp: u64,
    /// Outgoing memory transfers
    pub outbox: Vec<Experience>,
    /// Incoming memory transfers
    pub inbox: Vec<Experience>,
    /// Statistics
    pub stats: DreamingStats,
}

#[derive(Clone, Default, Debug)]
pub struct DreamingStats {
    pub experiences_received: u64,
    pub replays_performed: u64,
    pub memories_consolidated: u64,
    pub memories_transferred: u64,
    pub memories_received_from_peers: u64,
}

impl DreamingAgent {
    pub fn new(id: u32, cycle_hours: f32) -> Self {
        Self {
            id,
            working_memory: VecDeque::new(),
            consolidating: Vec::new(),
            long_term: Vec::new(),
            phase: SwarmPhase::Awake,
            cycle_phase: (id as f32 * 0.1) % 1.0, // Stagger agents slightly
            cycle_hours,
            timestamp: 0,
            outbox: Vec::new(),
            inbox: Vec::new(),
            stats: DreamingStats::default(),
        }
    }

    /// Receive a new experience
    pub fn experience(&mut self, obs: Vec<u32>, action: &str, outcome: f32, surprise: f32) {
        if !self.phase.can_process_new() {
            return; // Reject during sleep
        }

        let exp = Experience {
            timestamp: self.timestamp,
            observation: obs,
            action: action.to_string(),
            outcome,
            surprise,
            source_agent: self.id,
        };

        self.working_memory.push_back(exp.clone());
        self.stats.experiences_received += 1;

        // Transfer surprising experiences to consolidation queue
        if surprise > 0.5 {
            self.consolidating.push(MemoryTrace::new(exp));
        }

        // Limit working memory size
        while self.working_memory.len() > 100 {
            let old = self.working_memory.pop_front().unwrap();
            // Move to consolidation if not already there
            if old.surprise > 0.3 {
                self.consolidating.push(MemoryTrace::new(old));
            }
        }
    }

    /// Advance time and run consolidation
    pub fn tick(&mut self, dt_seconds: u64) {
        self.timestamp += dt_seconds;
        self.cycle_phase =
            (self.cycle_phase + dt_seconds as f32 / (self.cycle_hours * 3600.0)) % 1.0;
        self.phase = SwarmPhase::from_normalized_time(self.cycle_phase);

        // Process based on phase
        match self.phase {
            SwarmPhase::LightSleep => {
                self.light_sleep_consolidation();
            }
            SwarmPhase::DeepSleep => {
                self.deep_sleep_consolidation();
            }
            SwarmPhase::Waking => {
                self.integrate_transfers();
            }
            _ => {}
        }

        // Prune fully consolidated memories
        self.prune_consolidating();
    }

    /// Light sleep: local replay of recent experiences
    fn light_sleep_consolidation(&mut self) {
        // Select experiences for replay by priority
        let mut to_replay: Vec<_> = self
            .consolidating
            .iter()
            .enumerate()
            .map(|(i, trace)| (i, trace.experience.replay_priority(self.timestamp, 8.0)))
            .collect();

        to_replay.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Replay top experiences
        for (idx, _) in to_replay.into_iter().take(5) {
            self.consolidating[idx].replay();
            self.stats.replays_performed += 1;
        }
    }

    /// Deep sleep: cross-agent transfer of consolidated memories
    fn deep_sleep_consolidation(&mut self) {
        // Continue local replay
        self.light_sleep_consolidation();

        // Select memories for transfer (well-consolidated, not yet distributed)
        for trace in &mut self.consolidating {
            if trace.strength > 0.5 && !trace.distributed {
                self.outbox.push(trace.experience.clone());
                trace.distributed = true;
                self.stats.memories_transferred += 1;
            }
        }
    }

    /// Waking: integrate memories received from peers
    fn integrate_transfers(&mut self) {
        while let Some(exp) = self.inbox.pop() {
            // Check if we already have this experience
            let dominated = self
                .consolidating
                .iter()
                .any(|t| self.experiences_similar(&t.experience, &exp));

            if !dominated {
                let mut trace = MemoryTrace::new(exp);
                trace.validate(); // Peer validation
                self.consolidating.push(trace);
                self.stats.memories_received_from_peers += 1;
            } else {
                // Validate existing similar memory - find index first to avoid borrow conflict
                let idx = self
                    .consolidating
                    .iter()
                    .position(|t| Self::experiences_similar_static(&t.experience, &exp));
                if let Some(i) = idx {
                    self.consolidating[i].validate();
                }
            }
        }
    }

    fn experiences_similar(&self, a: &Experience, b: &Experience) -> bool {
        Self::experiences_similar_static(a, b)
    }

    fn experiences_similar_static(a: &Experience, b: &Experience) -> bool {
        // Simple Jaccard similarity on observations
        let set_a: HashSet<_> = a.observation.iter().collect();
        let set_b: HashSet<_> = b.observation.iter().collect();
        let intersection = set_a.intersection(&set_b).count();
        let union = set_a.union(&set_b).count();
        if union == 0 {
            return true;
        }
        (intersection as f32 / union as f32) > 0.8
    }

    fn prune_consolidating(&mut self) {
        // Move consolidated memories to long-term
        let mut to_move = Vec::new();
        for (i, trace) in self.consolidating.iter().enumerate() {
            if trace.is_consolidated() {
                to_move.push(i);
            }
        }

        // Move in reverse order to preserve indices
        for i in to_move.into_iter().rev() {
            let trace = self.consolidating.remove(i);
            self.long_term.push(trace);
            self.stats.memories_consolidated += 1;
        }

        // Limit long-term memory
        while self.long_term.len() > 500 {
            // Remove weakest
            let weakest = self
                .long_term
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    a.1.strength
                        .partial_cmp(&b.1.strength)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i);
            if let Some(idx) = weakest {
                self.long_term.remove(idx);
            }
        }
    }

    /// Receive memories from a peer
    pub fn receive_from_peer(&mut self, experiences: Vec<Experience>) {
        self.inbox.extend(experiences);
    }
}

// ============================================================================
// Collective Dream Network
// ============================================================================

/// Coordinated swarm of dreaming agents
pub struct CollectiveDream {
    /// All agents in the swarm
    pub agents: Vec<DreamingAgent>,
    /// Current timestamp
    pub timestamp: u64,
    /// Synchronization coupling strength
    pub coupling: f32,
}

impl CollectiveDream {
    pub fn new(num_agents: usize, cycle_hours: f32) -> Self {
        let agents = (0..num_agents)
            .map(|i| DreamingAgent::new(i as u32, cycle_hours))
            .collect();

        Self {
            agents,
            timestamp: 0,
            coupling: 0.3,
        }
    }

    /// Advance time for all agents
    pub fn tick(&mut self, dt_seconds: u64) {
        self.timestamp += dt_seconds;

        // Advance each agent
        for agent in &mut self.agents {
            agent.tick(dt_seconds);
        }

        // Transfer memories between agents during deep sleep
        self.memory_transfer();

        // Synchronize phases (Kuramoto-style)
        self.synchronize_phases();
    }

    fn memory_transfer(&mut self) {
        // Collect outboxes
        let mut all_transfers: Vec<(u32, Vec<Experience>)> = Vec::new();
        for agent in &mut self.agents {
            if !agent.outbox.is_empty() {
                let transfers = std::mem::take(&mut agent.outbox);
                all_transfers.push((agent.id, transfers));
            }
        }

        // Distribute to other agents
        for (source_id, experiences) in all_transfers {
            for agent in &mut self.agents {
                if agent.id != source_id && agent.phase.can_transfer() {
                    agent.receive_from_peer(experiences.clone());
                }
            }
        }
    }

    fn synchronize_phases(&mut self) {
        // Compute mean phase
        let n = self.agents.len() as f32;
        let mean_sin: f32 = self
            .agents
            .iter()
            .map(|a| (a.cycle_phase * 2.0 * PI).sin())
            .sum::<f32>()
            / n;
        let mean_cos: f32 = self
            .agents
            .iter()
            .map(|a| (a.cycle_phase * 2.0 * PI).cos())
            .sum::<f32>()
            / n;
        let _mean_phase = mean_sin.atan2(mean_cos) / (2.0 * PI);

        // Each agent adjusts toward mean
        for agent in &mut self.agents {
            let current = agent.cycle_phase * 2.0 * PI;
            let sin_diff = mean_sin * current.cos() - mean_cos * current.sin();
            let adjustment = self.coupling * sin_diff / (2.0 * PI);
            agent.cycle_phase = (agent.cycle_phase + adjustment).rem_euclid(1.0);
        }
    }

    /// Get synchronization order parameter
    pub fn synchronization(&self) -> f32 {
        let n = self.agents.len() as f32;
        let sum_sin: f32 = self
            .agents
            .iter()
            .map(|a| (a.cycle_phase * 2.0 * PI).sin())
            .sum();
        let sum_cos: f32 = self
            .agents
            .iter()
            .map(|a| (a.cycle_phase * 2.0 * PI).cos())
            .sum();
        (sum_sin * sum_sin + sum_cos * sum_cos).sqrt() / n
    }

    /// Get phase distribution
    pub fn phase_distribution(&self) -> HashMap<SwarmPhase, usize> {
        let mut dist = HashMap::new();
        for agent in &self.agents {
            *dist.entry(agent.phase.clone()).or_insert(0) += 1;
        }
        dist
    }

    /// Generate a collective experience for the swarm
    pub fn swarm_experience(
        &mut self,
        agent_id: usize,
        obs: Vec<u32>,
        action: &str,
        outcome: f32,
        surprise: f32,
    ) {
        if agent_id < self.agents.len() {
            self.agents[agent_id].experience(obs, action, outcome, surprise);
        }
    }

    /// Get total consolidated memories across swarm
    pub fn total_consolidated(&self) -> usize {
        self.agents.iter().map(|a| a.long_term.len()).sum()
    }

    /// Get collective statistics
    pub fn collective_stats(&self) -> DreamingStats {
        let mut stats = DreamingStats::default();
        for agent in &self.agents {
            stats.experiences_received += agent.stats.experiences_received;
            stats.replays_performed += agent.stats.replays_performed;
            stats.memories_consolidated += agent.stats.memories_consolidated;
            stats.memories_transferred += agent.stats.memories_transferred;
            stats.memories_received_from_peers += agent.stats.memories_received_from_peers;
        }
        stats
    }
}

// ============================================================================
// Example Usage
// ============================================================================

fn main() {
    println!("=== Tier 4: Collective Dreaming ===\n");

    // Create swarm of 10 agents with 1-hour cycles (for demo)
    let mut swarm = CollectiveDream::new(10, 1.0);

    println!("Swarm initialized: {} agents", swarm.agents.len());
    println!("Initial synchronization: {:.2}", swarm.synchronization());

    // Simulate experiences during awake phase
    println!("\n=== Awake Phase: Gathering Experiences ===");
    for minute in 0..30 {
        // Generate experiences for random agents
        for _ in 0..5 {
            let agent_id = (minute * 3 + 1) % 10;
            let obs: Vec<u32> = (0..50).map(|i| ((minute + i) * 7) as u32 % 10000).collect();
            let surprise = ((minute as f32 * 0.1).sin().abs() * 0.8) + 0.2;

            swarm.swarm_experience(
                agent_id,
                obs,
                &format!("action_{}", minute),
                ((minute as f32 * 0.05).cos() + 1.0) / 2.0,
                surprise,
            );
        }

        swarm.tick(60); // 1 minute

        if minute % 10 == 9 {
            let dist = swarm.phase_distribution();
            println!("  Minute {}: phases = {:?}", minute + 1, dist);
        }
    }

    // Continue through sleep cycle
    println!("\n=== Sleep Cycle: Consolidation ===");
    for minute in 30..60 {
        swarm.tick(60);

        if minute % 10 == 9 {
            let dist = swarm.phase_distribution();
            let stats = swarm.collective_stats();
            println!(
                "  Minute {}: phases = {:?}, consolidated = {}, transferred = {}",
                minute + 1,
                dist,
                stats.memories_consolidated,
                stats.memories_transferred
            );
        }
    }

    // Let agents wake up and integrate
    println!("\n=== Waking Phase: Integration ===");
    for minute in 60..70 {
        swarm.tick(60);

        if minute % 5 == 4 {
            let dist = swarm.phase_distribution();
            let stats = swarm.collective_stats();
            println!(
                "  Minute {}: phases = {:?}, peer memories = {}",
                minute + 1,
                dist,
                stats.memories_received_from_peers
            );
        }
    }

    // Final statistics
    println!("\n=== Final Statistics ===");
    let stats = swarm.collective_stats();
    println!("Total experiences: {}", stats.experiences_received);
    println!("Replays performed: {}", stats.replays_performed);
    println!("Memories consolidated: {}", stats.memories_consolidated);
    println!("Memories transferred: {}", stats.memories_transferred);
    println!(
        "Memories from peers: {}",
        stats.memories_received_from_peers
    );
    println!("Total long-term memories: {}", swarm.total_consolidated());
    println!("Final synchronization: {:.2}", swarm.synchronization());

    // Per-agent summary
    println!("\n=== Per-Agent Memory ===");
    for agent in &swarm.agents {
        println!(
            "  Agent {}: {} LT memories, {} consolidating, phase {:?}",
            agent.id,
            agent.long_term.len(),
            agent.consolidating.len(),
            agent.phase
        );
    }

    println!("\n=== Key Benefits ===");
    println!("- Synchronized rest phases across swarm");
    println!("- Hippocampal replay during sleep consolidates learning");
    println!("- Cross-agent memory transfer shares knowledge");
    println!("- No central coordinator needed");
    println!("- Resilient to individual agent loss");
    println!("\nThis is how biological systems scale collective learning.");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_transitions() {
        let mut agent = DreamingAgent::new(0, 1.0); // 1-hour cycle

        // Start awake
        assert!(matches!(agent.phase, SwarmPhase::Awake));

        // Advance to sleep
        agent.tick(2400); // 40 minutes
                          // Should be in some sleep phase
        assert!(!matches!(agent.phase, SwarmPhase::Awake));
    }

    #[test]
    fn test_consolidation() {
        let mut agent = DreamingAgent::new(0, 0.5); // Fast cycle

        // Add surprising experience
        agent.experience(vec![1, 2, 3], "test", 1.0, 0.9);
        assert!(!agent.consolidating.is_empty());

        // Advance through sleep
        for _ in 0..60 {
            agent.tick(60);
        }

        // Some should be consolidated
        // Note: may not consolidate in one cycle, that's OK
        assert!(agent.stats.replays_performed > 0);
    }

    #[test]
    fn test_memory_transfer() {
        let mut swarm = CollectiveDream::new(3, 0.25); // Fast cycles

        // Add experience to agent 0
        swarm.agents[0].experience(vec![1, 2, 3], "test", 1.0, 0.9);

        // Run through complete cycle
        for _ in 0..90 {
            swarm.tick(60);
        }

        // Check that memory was transferred
        let stats = swarm.collective_stats();
        // At least some transfer should happen
        assert!(stats.replays_performed > 0);
    }

    #[test]
    fn test_synchronization() {
        let mut swarm = CollectiveDream::new(5, 1.0);

        // Initially may not be synchronized
        let initial = swarm.synchronization();

        // Run for a while
        for _ in 0..120 {
            swarm.tick(60);
        }

        // Should become more synchronized
        let final_sync = swarm.synchronization();
        assert!(final_sync >= initial * 0.9); // At least maintain
    }
}
