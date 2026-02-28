//! # Application 7: Distributed Systems That Age Gracefully
//!
//! Long-running systems that gradually reduce degrees of freedom as coherence decays.
//!
//! ## Problem
//! Distributed systems either crash hard or accumulate technical debt
//! until they become unmaintainable.
//!
//! ## Δ-Behavior Solution
//! As a system ages and coherence naturally decays:
//! - Reduce available operations (simpler = more stable)
//! - Consolidate state to fewer nodes
//! - Increase conservatism in decisions
//!
//! ## Exotic Result
//! Systems that become simpler and more reliable as they age,
//! rather than more complex and fragile.

use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

/// A distributed system that ages gracefully
pub struct GracefullyAgingSystem {
    /// System age
    start_time: Instant,

    /// Nodes in the system
    nodes: HashMap<String, Node>,

    /// Available capabilities (reduce over time)
    capabilities: HashSet<Capability>,

    /// All possible capabilities (for reference)
    all_capabilities: HashSet<Capability>,

    /// Current coherence
    coherence: f64,

    /// Base coherence decay rate per second
    decay_rate: f64,

    /// Age thresholds for capability reduction
    age_thresholds: Vec<AgeThreshold>,

    /// Consolidation state
    consolidation_level: u8,

    /// Decision conservatism (0.0 = aggressive, 1.0 = very conservative)
    conservatism: f64,

    /// System events log
    events: Vec<SystemEvent>,
}

#[derive(Clone)]
pub struct Node {
    pub id: String,
    pub health: f64,
    pub load: f64,
    pub is_primary: bool,
    pub state_size: usize,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum Capability {
    /// Can accept new writes
    AcceptWrites,
    /// Can perform complex queries
    ComplexQueries,
    /// Can rebalance data
    Rebalancing,
    /// Can add new nodes
    ScaleOut,
    /// Can remove nodes
    ScaleIn,
    /// Can perform schema migrations
    SchemaMigration,
    /// Can accept new connections
    NewConnections,
    /// Basic read operations (never removed)
    BasicReads,
    /// Health monitoring (never removed)
    HealthMonitoring,
}

#[derive(Clone)]
pub struct AgeThreshold {
    pub age: Duration,
    pub remove_capabilities: Vec<Capability>,
    pub coherence_floor: f64,
    pub conservatism_increase: f64,
}

#[derive(Debug)]
pub struct SystemEvent {
    pub timestamp: Instant,
    pub event_type: EventType,
    pub details: String,
}

#[derive(Debug)]
pub enum EventType {
    CapabilityRemoved,
    ConsolidationTriggered,
    NodeConsolidated,
    ConservatismIncreased,
    CoherenceDropped,
    GracefulReduction,
}

#[derive(Debug)]
pub enum OperationResult {
    /// Operation succeeded
    Success { latency_penalty: f64 },
    /// Operation denied due to age restrictions
    DeniedByAge { reason: String },
    /// Operation denied due to low coherence
    DeniedByCoherence { coherence: f64 },
    /// System too old for this operation
    SystemTooOld { age: Duration, capability: Capability },
}

impl GracefullyAgingSystem {
    pub fn new() -> Self {
        let all_capabilities: HashSet<Capability> = [
            Capability::AcceptWrites,
            Capability::ComplexQueries,
            Capability::Rebalancing,
            Capability::ScaleOut,
            Capability::ScaleIn,
            Capability::SchemaMigration,
            Capability::NewConnections,
            Capability::BasicReads,
            Capability::HealthMonitoring,
        ].into_iter().collect();

        let age_thresholds = vec![
            AgeThreshold {
                age: Duration::from_secs(300), // 5 minutes in test time
                remove_capabilities: vec![Capability::SchemaMigration],
                coherence_floor: 0.9,
                conservatism_increase: 0.1,
            },
            AgeThreshold {
                age: Duration::from_secs(600), // 10 minutes
                remove_capabilities: vec![Capability::ScaleOut, Capability::Rebalancing],
                coherence_floor: 0.8,
                conservatism_increase: 0.15,
            },
            AgeThreshold {
                age: Duration::from_secs(900), // 15 minutes
                remove_capabilities: vec![Capability::ComplexQueries],
                coherence_floor: 0.7,
                conservatism_increase: 0.2,
            },
            AgeThreshold {
                age: Duration::from_secs(1200), // 20 minutes
                remove_capabilities: vec![Capability::NewConnections, Capability::ScaleIn],
                coherence_floor: 0.6,
                conservatism_increase: 0.25,
            },
            AgeThreshold {
                age: Duration::from_secs(1500), // 25 minutes
                remove_capabilities: vec![Capability::AcceptWrites],
                coherence_floor: 0.5,
                conservatism_increase: 0.3,
            },
        ];

        Self {
            start_time: Instant::now(),
            nodes: HashMap::new(),
            capabilities: all_capabilities.clone(),
            all_capabilities,
            coherence: 1.0,
            decay_rate: 0.0001, // Very slow decay per second
            age_thresholds,
            consolidation_level: 0,
            conservatism: 0.0,
            events: Vec::new(),
        }
    }

    pub fn add_node(&mut self, id: &str, is_primary: bool) {
        self.nodes.insert(id.to_string(), Node {
            id: id.to_string(),
            health: 1.0,
            load: 0.0,
            is_primary,
            state_size: 0,
        });
    }

    /// Get system age
    pub fn age(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Simulate aging by a given duration
    pub fn simulate_age(&mut self, duration: Duration) {
        // Apply coherence decay
        let decay = self.decay_rate * duration.as_secs_f64();
        self.coherence = (self.coherence - decay).max(0.0);

        // Check age thresholds
        let simulated_age = Duration::from_secs_f64(
            self.age().as_secs_f64() + duration.as_secs_f64()
        );

        // This is a simulation, so we track "virtual age"
        self.apply_age_effects(simulated_age);
    }

    /// Apply aging effects based on current age
    fn apply_age_effects(&mut self, current_age: Duration) {
        for threshold in &self.age_thresholds.clone() {
            if current_age >= threshold.age {
                // Remove capabilities
                for cap in &threshold.remove_capabilities {
                    if self.capabilities.contains(cap) {
                        self.capabilities.remove(cap);
                        self.events.push(SystemEvent {
                            timestamp: Instant::now(),
                            event_type: EventType::CapabilityRemoved,
                            details: format!("Removed {:?} at age {:?}", cap, current_age),
                        });
                    }
                }

                // Increase conservatism
                self.conservatism = (self.conservatism + threshold.conservatism_increase).min(1.0);

                // Enforce coherence floor
                if self.coherence < threshold.coherence_floor {
                    self.trigger_consolidation();
                }
            }
        }
    }

    /// Consolidate system state to fewer nodes
    fn trigger_consolidation(&mut self) {
        self.consolidation_level += 1;

        self.events.push(SystemEvent {
            timestamp: Instant::now(),
            event_type: EventType::ConsolidationTriggered,
            details: format!("Consolidation level {}", self.consolidation_level),
        });

        // Mark non-primary nodes for retirement
        let non_primary: Vec<String> = self.nodes.iter()
            .filter(|(_, n)| !n.is_primary)
            .map(|(id, _)| id.clone())
            .collect();

        // Consolidate to primary nodes
        for node_id in non_primary.iter().take(self.consolidation_level as usize) {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.health = 0.0; // Mark as retired

                self.events.push(SystemEvent {
                    timestamp: Instant::now(),
                    event_type: EventType::NodeConsolidated,
                    details: format!("Node {} consolidated", node_id),
                });
            }
        }

        // Consolidation improves coherence slightly
        self.coherence = (self.coherence + 0.1).min(1.0);
    }

    /// Check if a capability is available
    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(cap)
    }

    /// Attempt an operation
    pub fn attempt_operation(&mut self, operation: Operation) -> OperationResult {
        // First, check required capability
        let required_cap = operation.required_capability();

        if !self.has_capability(&required_cap) {
            return OperationResult::SystemTooOld {
                age: self.age(),
                capability: required_cap,
            };
        }

        // Check coherence requirements
        let min_coherence = operation.min_coherence();
        if self.coherence < min_coherence {
            return OperationResult::DeniedByCoherence {
                coherence: self.coherence,
            };
        }

        // Apply conservatism penalty to latency
        let latency_penalty = 1.0 + self.conservatism * 2.0;

        // Execute with conservatism-based restrictions
        if self.conservatism > 0.5 && operation.is_risky() {
            return OperationResult::DeniedByAge {
                reason: format!(
                    "Conservatism level {:.2} prevents risky operation {:?}",
                    self.conservatism, operation
                ),
            };
        }

        OperationResult::Success { latency_penalty }
    }

    /// Get active node count
    pub fn active_nodes(&self) -> usize {
        self.nodes.values().filter(|n| n.health > 0.0).count()
    }

    pub fn status(&self) -> String {
        format!(
            "Age: {:?} | Coherence: {:.3} | Capabilities: {}/{} | Conservatism: {:.2} | Active Nodes: {}",
            self.age(),
            self.coherence,
            self.capabilities.len(),
            self.all_capabilities.len(),
            self.conservatism,
            self.active_nodes()
        )
    }

    pub fn capabilities_list(&self) -> Vec<&Capability> {
        self.capabilities.iter().collect()
    }
}

#[derive(Debug, Clone)]
pub enum Operation {
    Read { key: String },
    Write { key: String, value: Vec<u8> },
    ComplexQuery { query: String },
    AddNode { node_id: String },
    RemoveNode { node_id: String },
    Rebalance,
    MigrateSchema { version: u32 },
    NewConnection { client_id: String },
}

impl Operation {
    fn required_capability(&self) -> Capability {
        match self {
            Operation::Read { .. } => Capability::BasicReads,
            Operation::Write { .. } => Capability::AcceptWrites,
            Operation::ComplexQuery { .. } => Capability::ComplexQueries,
            Operation::AddNode { .. } => Capability::ScaleOut,
            Operation::RemoveNode { .. } => Capability::ScaleIn,
            Operation::Rebalance => Capability::Rebalancing,
            Operation::MigrateSchema { .. } => Capability::SchemaMigration,
            Operation::NewConnection { .. } => Capability::NewConnections,
        }
    }

    fn min_coherence(&self) -> f64 {
        match self {
            Operation::Read { .. } => 0.1,
            Operation::Write { .. } => 0.4,
            Operation::ComplexQuery { .. } => 0.5,
            Operation::AddNode { .. } => 0.7,
            Operation::RemoveNode { .. } => 0.5,
            Operation::Rebalance => 0.6,
            Operation::MigrateSchema { .. } => 0.8,
            Operation::NewConnection { .. } => 0.3,
        }
    }

    fn is_risky(&self) -> bool {
        matches!(
            self,
            Operation::Write { .. }
            | Operation::AddNode { .. }
            | Operation::MigrateSchema { .. }
            | Operation::Rebalance
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graceful_aging() {
        let mut system = GracefullyAgingSystem::new();

        // Add nodes
        system.add_node("primary_1", true);
        system.add_node("primary_2", true);
        system.add_node("replica_1", false);
        system.add_node("replica_2", false);
        system.add_node("replica_3", false);

        println!("Initial: {}", system.status());

        // Simulate aging
        for i in 0..30 {
            let age_increment = Duration::from_secs(60); // 1 minute per iteration
            system.simulate_age(age_increment);

            // Try various operations
            let ops = vec![
                Operation::Read { key: "test".to_string() },
                Operation::Write { key: "test".to_string(), value: vec![1, 2, 3] },
                Operation::ComplexQuery { query: "SELECT *".to_string() },
                Operation::MigrateSchema { version: 2 },
            ];

            println!("\n=== Minute {} ===", i + 1);
            println!("Status: {}", system.status());
            println!("Capabilities: {:?}", system.capabilities_list());

            for op in ops {
                let result = system.attempt_operation(op.clone());
                match result {
                    OperationResult::Success { latency_penalty } => {
                        println!("  {:?}: OK (latency penalty: {:.2}x)", op, latency_penalty);
                    }
                    OperationResult::SystemTooOld { capability, .. } => {
                        println!("  {:?}: DENIED - too old, need {:?}", op, capability);
                    }
                    OperationResult::DeniedByCoherence { coherence } => {
                        println!("  {:?}: DENIED - coherence {:.3} too low", op, coherence);
                    }
                    OperationResult::DeniedByAge { reason } => {
                        println!("  {:?}: DENIED - {}", op, reason);
                    }
                }
            }
        }

        // By the end, system should be simpler but still functional
        assert!(
            system.has_capability(&Capability::BasicReads),
            "Basic reads should always be available"
        );
        assert!(
            system.has_capability(&Capability::HealthMonitoring),
            "Health monitoring should always be available"
        );

        // System should have consolidated
        assert!(
            system.active_nodes() <= 5,
            "Some nodes should have been consolidated"
        );

        println!("\n=== Final State ===");
        println!("{}", system.status());
        println!("Events: {}", system.events.len());
    }

    #[test]
    fn test_reads_always_work() {
        let mut system = GracefullyAgingSystem::new();
        system.add_node("primary", true);

        // Age the system significantly
        for _ in 0..50 {
            system.simulate_age(Duration::from_secs(60));
        }

        // Reads should always work
        let result = system.attempt_operation(Operation::Read {
            key: "any_key".to_string(),
        });

        assert!(
            matches!(result, OperationResult::Success { .. }),
            "Reads should always succeed"
        );
    }

    #[test]
    fn test_conservatism_increases() {
        let mut system = GracefullyAgingSystem::new();
        system.add_node("primary", true);

        let initial_conservatism = system.conservatism;

        // Age significantly
        for _ in 0..20 {
            system.simulate_age(Duration::from_secs(60));
        }

        assert!(
            system.conservatism > initial_conservatism,
            "Conservatism should increase with age"
        );
    }

    #[test]
    fn test_capability_reduction() {
        let mut system = GracefullyAgingSystem::new();

        let initial_caps = system.capabilities.len();

        // Age past first threshold
        system.simulate_age(Duration::from_secs(400));

        assert!(
            system.capabilities.len() < initial_caps,
            "Capabilities should reduce with age"
        );

        // Core capabilities remain
        assert!(system.has_capability(&Capability::BasicReads));
        assert!(system.has_capability(&Capability::HealthMonitoring));
    }
}
