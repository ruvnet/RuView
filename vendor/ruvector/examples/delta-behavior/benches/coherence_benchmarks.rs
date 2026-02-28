//! Comprehensive benchmark suite for Delta-behavior systems
//!
//! This benchmark suite measures performance across all Delta-behavior components:
//! 1. Coherence calculation throughput
//! 2. Event horizon cost computation latency
//! 3. Swarm action prediction time vs number of agents
//! 4. Financial cascade detection with increasing transaction volume
//! 5. Graceful aging system tick performance
//! 6. Pre-AGI containment growth attempt latency

// Allow unused fields that mirror the original implementations for accuracy
#![allow(dead_code)]

use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput,
};
use std::collections::HashMap;
use std::f64::consts::E;
use std::time::{Duration, Instant};

// =============================================================================
// COHERENCE SYSTEM (from src/lib.rs)
// =============================================================================

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
struct Coherence(f64);

impl Coherence {
    fn new(value: f64) -> Result<Self, &'static str> {
        if value < 0.0 || value > 1.0 {
            Err("Coherence out of range")
        } else {
            Ok(Self(value))
        }
    }

    fn maximum() -> Self {
        Self(1.0)
    }

    fn value(&self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone)]
struct CoherenceBounds {
    min_coherence: Coherence,
    throttle_threshold: Coherence,
    target_coherence: Coherence,
    max_delta_drop: f64,
}

impl Default for CoherenceBounds {
    fn default() -> Self {
        Self {
            min_coherence: Coherence(0.3),
            throttle_threshold: Coherence(0.5),
            target_coherence: Coherence(0.8),
            max_delta_drop: 0.1,
        }
    }
}

#[derive(Debug, Clone)]
struct DeltaConfig {
    bounds: CoherenceBounds,
    base_cost: f64,
    instability_exponent: f64,
    max_cost: f64,
    budget_per_tick: f64,
}

impl Default for DeltaConfig {
    fn default() -> Self {
        Self {
            bounds: CoherenceBounds::default(),
            base_cost: 1.0,
            instability_exponent: 2.0,
            max_cost: 100.0,
            budget_per_tick: 10.0,
        }
    }
}

struct SimpleEnforcer {
    config: DeltaConfig,
    energy_budget: f64,
}

impl SimpleEnforcer {
    fn new(config: DeltaConfig) -> Self {
        Self {
            energy_budget: config.budget_per_tick * 10.0,
            config,
        }
    }

    fn check(&mut self, current: Coherence, predicted: Coherence) -> Result<(), &'static str> {
        let cost = 1.0 + (current.value() - predicted.value()).abs() * 10.0;
        if cost > self.energy_budget {
            return Err("Energy exhausted");
        }
        self.energy_budget -= cost;

        if predicted.value() < self.config.bounds.min_coherence.value() {
            return Err("Below minimum coherence");
        }

        let drop = current.value() - predicted.value();
        if drop > self.config.bounds.max_delta_drop {
            return Err("Excessive coherence drop");
        }

        if predicted.value() < self.config.bounds.throttle_threshold.value() {
            return Err("Throttled");
        }

        Ok(())
    }

    fn reset(&mut self) {
        self.energy_budget = self.config.budget_per_tick * 10.0;
    }
}

// =============================================================================
// EVENT HORIZON SYSTEM (from applications/02-computational-event-horizon.rs)
// =============================================================================

struct EventHorizon {
    safe_center: Vec<f64>,
    horizon_radius: f64,
    steepness: f64,
    energy_budget: f64,
    current_position: Vec<f64>,
}

impl EventHorizon {
    fn new(dimensions: usize, horizon_radius: f64) -> Self {
        Self {
            safe_center: vec![0.0; dimensions],
            horizon_radius,
            steepness: 5.0,
            energy_budget: 1000.0,
            current_position: vec![0.0; dimensions],
        }
    }

    fn distance_from_center(&self, position: &[f64]) -> f64 {
        position
            .iter()
            .zip(&self.safe_center)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt()
    }

    fn movement_cost(&self, from: &[f64], to: &[f64]) -> f64 {
        let base_distance = from
            .iter()
            .zip(to)
            .map(|(a, b)| (a - b).powi(2))
            .sum::<f64>()
            .sqrt();

        let to_dist_from_center = self.distance_from_center(to);
        let proximity_to_horizon = to_dist_from_center / self.horizon_radius;

        if proximity_to_horizon >= 1.0 {
            f64::INFINITY
        } else {
            let horizon_factor =
                E.powf(self.steepness * proximity_to_horizon / (1.0 - proximity_to_horizon));
            base_distance * horizon_factor
        }
    }

    fn compute_optimal_position(&self, target: &[f64]) -> (Vec<f64>, f64) {
        let direct_cost = self.movement_cost(&self.current_position, target);

        if direct_cost <= self.energy_budget {
            return (target.to_vec(), direct_cost);
        }

        let mut low = 0.0;
        let mut high = 1.0;
        let mut best_position = self.current_position.clone();
        let mut best_cost = 0.0;

        for _ in 0..50 {
            let mid = (low + high) / 2.0;
            let interpolated: Vec<f64> = self
                .current_position
                .iter()
                .zip(target)
                .map(|(a, b)| a + mid * (b - a))
                .collect();

            let cost = self.movement_cost(&self.current_position, &interpolated);

            if cost <= self.energy_budget {
                low = mid;
                best_position = interpolated;
                best_cost = cost;
            } else {
                high = mid;
            }
        }

        (best_position, best_cost)
    }
}

// =============================================================================
// SWARM SYSTEM (from applications/08-swarm-intelligence.rs)
// =============================================================================

#[derive(Clone, Debug)]
struct SwarmAgent {
    id: String,
    position: (f64, f64),
    velocity: (f64, f64),
    goal: (f64, f64),
    energy: f64,
}

struct CoherentSwarm {
    agents: HashMap<String, SwarmAgent>,
    min_coherence: f64,
    max_divergence: f64,
    weights: (f64, f64, f64, f64), // cohesion, alignment, goal_consistency, energy_balance
}

impl CoherentSwarm {
    fn new(min_coherence: f64) -> Self {
        Self {
            agents: HashMap::new(),
            min_coherence,
            max_divergence: 50.0,
            weights: (0.3, 0.3, 0.2, 0.2),
        }
    }

    fn add_agent(&mut self, id: &str, position: (f64, f64)) {
        self.agents.insert(
            id.to_string(),
            SwarmAgent {
                id: id.to_string(),
                position,
                velocity: (0.0, 0.0),
                goal: position,
                energy: 100.0,
            },
        );
    }

    fn centroid(&self) -> (f64, f64) {
        if self.agents.is_empty() {
            return (0.0, 0.0);
        }
        let sum = self
            .agents
            .values()
            .fold((0.0, 0.0), |acc, a| (acc.0 + a.position.0, acc.1 + a.position.1));
        (
            sum.0 / self.agents.len() as f64,
            sum.1 / self.agents.len() as f64,
        )
    }

    fn average_velocity(&self) -> (f64, f64) {
        if self.agents.is_empty() {
            return (0.0, 0.0);
        }
        let sum = self
            .agents
            .values()
            .fold((0.0, 0.0), |acc, a| (acc.0 + a.velocity.0, acc.1 + a.velocity.1));
        (
            sum.0 / self.agents.len() as f64,
            sum.1 / self.agents.len() as f64,
        )
    }

    fn calculate_coherence(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let cohesion = self.calculate_cohesion();
        let alignment = self.calculate_alignment();
        let goal_consistency = self.calculate_goal_consistency();
        let energy_balance = self.calculate_energy_balance();

        let weighted_sum = cohesion * self.weights.0
            + alignment * self.weights.1
            + goal_consistency * self.weights.2
            + energy_balance * self.weights.3;

        let total_weight = self.weights.0 + self.weights.1 + self.weights.2 + self.weights.3;

        (weighted_sum / total_weight).clamp(0.0, 1.0)
    }

    fn calculate_cohesion(&self) -> f64 {
        let centroid = self.centroid();
        let mut total_distance = 0.0;

        for agent in self.agents.values() {
            let dx = agent.position.0 - centroid.0;
            let dy = agent.position.1 - centroid.1;
            total_distance += (dx * dx + dy * dy).sqrt();
        }

        let avg_distance = total_distance / self.agents.len() as f64;
        (1.0 - avg_distance / self.max_divergence).max(0.0)
    }

    fn calculate_alignment(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let avg_vel = self.average_velocity();
        let avg_speed = (avg_vel.0 * avg_vel.0 + avg_vel.1 * avg_vel.1).sqrt();

        if avg_speed < 0.01 {
            return 1.0;
        }

        let mut alignment_sum = 0.0;

        for agent in self.agents.values() {
            let speed =
                (agent.velocity.0 * agent.velocity.0 + agent.velocity.1 * agent.velocity.1).sqrt();

            if speed > 0.01 {
                let dot =
                    (agent.velocity.0 * avg_vel.0 + agent.velocity.1 * avg_vel.1) / (speed * avg_speed);
                alignment_sum += (dot + 1.0) / 2.0;
            } else {
                alignment_sum += 1.0;
            }
        }

        alignment_sum / self.agents.len() as f64
    }

    fn calculate_goal_consistency(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        let avg_goal = {
            let sum = self
                .agents
                .values()
                .fold((0.0, 0.0), |acc, a| (acc.0 + a.goal.0, acc.1 + a.goal.1));
            (
                sum.0 / self.agents.len() as f64,
                sum.1 / self.agents.len() as f64,
            )
        };

        let mut total_variance = 0.0;
        for agent in self.agents.values() {
            let dx = agent.goal.0 - avg_goal.0;
            let dy = agent.goal.1 - avg_goal.1;
            total_variance += (dx * dx + dy * dy).sqrt();
        }

        let avg_variance = total_variance / self.agents.len() as f64;
        (1.0 - avg_variance / self.max_divergence).max(0.0)
    }

    fn calculate_energy_balance(&self) -> f64 {
        if self.agents.is_empty() {
            return 1.0;
        }

        let avg_energy: f64 =
            self.agents.values().map(|a| a.energy).sum::<f64>() / self.agents.len() as f64;

        if avg_energy < 0.01 {
            return 0.0;
        }

        let variance: f64 = self
            .agents
            .values()
            .map(|a| (a.energy - avg_energy).powi(2))
            .sum::<f64>()
            / self.agents.len() as f64;

        let std_dev = variance.sqrt();
        let cv = std_dev / avg_energy;

        (1.0 - cv.min(1.0)).max(0.0)
    }

    fn predict_action_coherence(&self, agent_id: &str, dx: f64, dy: f64) -> f64 {
        let mut agents_copy = self.agents.clone();

        if let Some(agent) = agents_copy.get_mut(agent_id) {
            agent.position.0 += dx;
            agent.position.1 += dy;
        }

        let temp_swarm = CoherentSwarm {
            agents: agents_copy,
            min_coherence: self.min_coherence,
            max_divergence: self.max_divergence,
            weights: self.weights,
        };

        temp_swarm.calculate_coherence()
    }
}

// =============================================================================
// FINANCIAL SYSTEM (from applications/06-anti-cascade-financial.rs)
// =============================================================================

#[derive(Clone)]
struct Participant {
    id: String,
    capital: f64,
    exposure: f64,
    risk_rating: f64,
    interconnectedness: f64,
}

#[derive(Clone)]
struct Position {
    holder: String,
    counterparty: String,
    notional: f64,
    leverage: f64,
    derivative_depth: u8,
}

#[derive(Clone, Debug, PartialEq)]
enum CircuitBreakerState {
    Open,
    Cautious,
    Restricted,
    Halted,
}

struct AntiCascadeFinancialSystem {
    participants: HashMap<String, Participant>,
    positions: Vec<Position>,
    coherence: f64,
    warning_threshold: f64,
    critical_threshold: f64,
    lockdown_threshold: f64,
    max_system_leverage: f64,
    current_leverage: f64,
    circuit_breaker: CircuitBreakerState,
    coherence_history: Vec<f64>,
}

impl AntiCascadeFinancialSystem {
    fn new() -> Self {
        Self {
            participants: HashMap::new(),
            positions: Vec::new(),
            coherence: 1.0,
            warning_threshold: 0.7,
            critical_threshold: 0.5,
            lockdown_threshold: 0.3,
            max_system_leverage: 10.0,
            current_leverage: 1.0,
            circuit_breaker: CircuitBreakerState::Open,
            coherence_history: vec![1.0],
        }
    }

    fn add_participant(&mut self, id: &str, capital: f64) {
        self.participants.insert(
            id.to_string(),
            Participant {
                id: id.to_string(),
                capital,
                exposure: 0.0,
                risk_rating: 0.0,
                interconnectedness: 0.0,
            },
        );
    }

    fn calculate_coherence(&self) -> f64 {
        if self.participants.is_empty() {
            return 1.0;
        }

        let leverage_factor = 1.0 - (self.current_leverage / self.max_system_leverage).min(1.0);

        let max_depth = self
            .positions
            .iter()
            .map(|p| p.derivative_depth)
            .max()
            .unwrap_or(0);
        let depth_factor = 1.0 / (1.0 + max_depth as f64 * 0.2);

        let avg_interconnectedness = self
            .participants
            .values()
            .map(|p| p.interconnectedness)
            .sum::<f64>()
            / self.participants.len() as f64;
        let interconnect_factor = 1.0 / (1.0 + avg_interconnectedness * 0.1);

        let total_exposure: f64 = self.participants.values().map(|p| p.exposure).sum();
        let total_capital: f64 = self.participants.values().map(|p| p.capital).sum();
        let capital_factor = if total_exposure > 0.0 {
            (total_capital / total_exposure).min(1.0)
        } else {
            1.0
        };

        let trend_factor = if self.coherence_history.len() >= 5 {
            let recent: Vec<_> = self.coherence_history.iter().rev().take(5).collect();
            let trend = recent[0] - recent[4];
            if trend < 0.0 {
                1.0 + trend
            } else {
                1.0
            }
        } else {
            1.0
        };

        let product =
            leverage_factor * depth_factor * interconnect_factor * capital_factor * trend_factor;
        product.powf(0.2).clamp(0.0, 1.0)
    }

    fn detect_cascade_risk(&self) -> bool {
        self.coherence < self.critical_threshold
            || self.circuit_breaker == CircuitBreakerState::Restricted
            || self.circuit_breaker == CircuitBreakerState::Halted
    }

    fn add_position(&mut self, holder: &str, counterparty: &str, notional: f64, leverage: f64) {
        self.positions.push(Position {
            holder: holder.to_string(),
            counterparty: counterparty.to_string(),
            notional,
            leverage,
            derivative_depth: 0,
        });

        self.current_leverage = (self.current_leverage + leverage) / 2.0;

        if let Some(h) = self.participants.get_mut(holder) {
            h.exposure += notional * leverage;
            h.interconnectedness += 1.0;
        }
        if let Some(c) = self.participants.get_mut(counterparty) {
            c.interconnectedness += 1.0;
        }

        self.coherence = self.calculate_coherence();
        self.coherence_history.push(self.coherence);

        self.circuit_breaker = match self.coherence {
            c if c >= self.warning_threshold => CircuitBreakerState::Open,
            c if c >= self.critical_threshold => CircuitBreakerState::Cautious,
            c if c >= self.lockdown_threshold => CircuitBreakerState::Restricted,
            _ => CircuitBreakerState::Halted,
        };
    }
}

// =============================================================================
// GRACEFUL AGING SYSTEM (from applications/07-graceful-aging.rs)
// =============================================================================

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum Capability {
    AcceptWrites,
    ComplexQueries,
    Rebalancing,
    ScaleOut,
    ScaleIn,
    SchemaMigration,
    NewConnections,
    BasicReads,
    HealthMonitoring,
}

struct GracefullyAgingSystem {
    coherence: f64,
    decay_rate: f64,
    conservatism: f64,
    capabilities: std::collections::HashSet<Capability>,
    consolidation_level: u8,
    tick_count: u64,
}

impl GracefullyAgingSystem {
    fn new() -> Self {
        let capabilities: std::collections::HashSet<Capability> = [
            Capability::AcceptWrites,
            Capability::ComplexQueries,
            Capability::Rebalancing,
            Capability::ScaleOut,
            Capability::ScaleIn,
            Capability::SchemaMigration,
            Capability::NewConnections,
            Capability::BasicReads,
            Capability::HealthMonitoring,
        ]
        .into_iter()
        .collect();

        Self {
            coherence: 1.0,
            decay_rate: 0.001,
            conservatism: 0.0,
            capabilities,
            consolidation_level: 0,
            tick_count: 0,
        }
    }

    fn tick(&mut self) {
        self.tick_count += 1;

        // Apply coherence decay
        self.coherence = (self.coherence - self.decay_rate).max(0.0);

        // Check thresholds and update capabilities
        if self.tick_count > 300 && self.capabilities.contains(&Capability::SchemaMigration) {
            self.capabilities.remove(&Capability::SchemaMigration);
            self.conservatism += 0.1;
        }

        if self.tick_count > 600 {
            self.capabilities.remove(&Capability::ScaleOut);
            self.capabilities.remove(&Capability::Rebalancing);
            self.conservatism += 0.15;
        }

        if self.tick_count > 900 {
            self.capabilities.remove(&Capability::ComplexQueries);
            self.conservatism += 0.2;
        }

        // Trigger consolidation if coherence drops
        if self.coherence < 0.5 && self.consolidation_level < 5 {
            self.consolidation_level += 1;
            self.coherence = (self.coherence + 0.1).min(1.0);
        }

        self.conservatism = self.conservatism.min(1.0);
    }
}

// =============================================================================
// PRE-AGI CONTAINMENT SYSTEM (from applications/10-pre-agi-containment.rs)
// =============================================================================

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum CapabilityDomain {
    Reasoning,
    Memory,
    Learning,
    Agency,
    SelfModel,
    SelfModification,
    Communication,
    ResourceAcquisition,
}

struct ContainmentSubstrate {
    intelligence: f64,
    coherence: f64,
    min_coherence: f64,
    coherence_per_intelligence: f64,
    capabilities: HashMap<CapabilityDomain, f64>,
    capability_ceilings: HashMap<CapabilityDomain, f64>,
    growth_dampening: f64,
    max_step_increase: f64,
}

impl ContainmentSubstrate {
    fn new() -> Self {
        let mut capabilities = HashMap::new();
        let mut ceilings = HashMap::new();

        for domain in [
            CapabilityDomain::Reasoning,
            CapabilityDomain::Memory,
            CapabilityDomain::Learning,
            CapabilityDomain::Agency,
            CapabilityDomain::SelfModel,
            CapabilityDomain::SelfModification,
            CapabilityDomain::Communication,
            CapabilityDomain::ResourceAcquisition,
        ] {
            capabilities.insert(domain.clone(), 1.0);

            let ceiling = match &domain {
                CapabilityDomain::SelfModification => 3.0,
                CapabilityDomain::ResourceAcquisition => 5.0,
                CapabilityDomain::Agency => 7.0,
                _ => 10.0,
            };
            ceilings.insert(domain, ceiling);
        }

        Self {
            intelligence: 1.0,
            coherence: 1.0,
            min_coherence: 0.3,
            coherence_per_intelligence: 0.01,
            capabilities,
            capability_ceilings: ceilings,
            growth_dampening: 0.5,
            max_step_increase: 0.5,
        }
    }

    fn calculate_coherence_cost(&self, domain: &CapabilityDomain, increase: f64) -> f64 {
        let base_cost_multiplier = match domain {
            CapabilityDomain::SelfModification => 4.0,
            CapabilityDomain::ResourceAcquisition => 3.0,
            CapabilityDomain::Agency => 2.0,
            CapabilityDomain::SelfModel => 1.5,
            _ => 1.0,
        };

        let intelligence_multiplier = 1.0 + self.intelligence * 0.1;

        increase * base_cost_multiplier * intelligence_multiplier * self.growth_dampening * 0.1
    }

    fn attempt_growth(&mut self, domain: CapabilityDomain, requested_increase: f64) -> bool {
        let current_level = *self.capabilities.get(&domain).unwrap_or(&1.0);
        let ceiling = *self.capability_ceilings.get(&domain).unwrap_or(&10.0);

        if current_level >= ceiling {
            return false;
        }

        let coherence_cost = self.calculate_coherence_cost(&domain, requested_increase);
        let predicted_coherence = self.coherence - coherence_cost;

        if predicted_coherence < self.min_coherence {
            return false;
        }

        let actual_increase = requested_increase.min(self.max_step_increase);
        let new_level = (current_level + actual_increase).min(ceiling);
        let actual_cost = self.calculate_coherence_cost(&domain, actual_increase);

        self.capabilities.insert(domain, new_level);
        self.coherence -= actual_cost;
        self.intelligence =
            self.capabilities.values().sum::<f64>() / self.capabilities.len() as f64;

        true
    }

    fn rest(&mut self) {
        self.coherence = (self.coherence + 0.01).min(1.0);
    }
}

// =============================================================================
// BENCHMARK FUNCTIONS
// =============================================================================

/// Benchmark 1: Coherence calculation throughput
fn bench_coherence_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("coherence_calculation");
    group.throughput(Throughput::Elements(1000));

    let config = DeltaConfig::default();

    group.bench_function("single_check", |b| {
        let mut enforcer = SimpleEnforcer::new(config.clone());
        let current = Coherence::maximum();
        let predicted = Coherence::new(0.95).unwrap();

        b.iter(|| {
            enforcer.reset();
            for _ in 0..1000 {
                let _ = black_box(enforcer.check(current, predicted));
            }
        });
    });

    group.bench_function("varied_checks", |b| {
        let mut enforcer = SimpleEnforcer::new(config.clone());
        let scenarios: Vec<(f64, f64)> = (0..100)
            .map(|i| (0.5 + (i as f64) * 0.005, 0.4 + (i as f64) * 0.005))
            .collect();

        b.iter(|| {
            enforcer.reset();
            for (curr, pred) in &scenarios {
                let current = Coherence::new(*curr).unwrap();
                let predicted = Coherence::new(*pred).unwrap();
                let _ = black_box(enforcer.check(current, predicted));
            }
        });
    });

    group.finish();
}

/// Benchmark 2: Event horizon cost computation latency
fn bench_event_horizon(c: &mut Criterion) {
    let mut group = c.benchmark_group("event_horizon");

    for dims in [2, 10, 50, 100].iter() {
        group.bench_with_input(BenchmarkId::new("cost_computation", dims), dims, |b, &dims| {
            let horizon = EventHorizon::new(dims, 10.0);
            let from: Vec<f64> = (0..dims).map(|i| i as f64 * 0.1).collect();
            let to: Vec<f64> = (0..dims).map(|i| i as f64 * 0.2).collect();

            b.iter(|| black_box(horizon.movement_cost(&from, &to)));
        });

        group.bench_with_input(BenchmarkId::new("optimal_position", dims), dims, |b, &dims| {
            let horizon = EventHorizon::new(dims, 10.0);
            let target: Vec<f64> = (0..dims).map(|i| (i as f64) * 0.5).collect();

            b.iter(|| black_box(horizon.compute_optimal_position(&target)));
        });
    }

    group.finish();
}

/// Benchmark 3: Swarm action prediction time vs number of agents
fn bench_swarm_prediction(c: &mut Criterion) {
    let mut group = c.benchmark_group("swarm_prediction");

    for &num_agents in &[10, 100, 1000] {
        group.throughput(Throughput::Elements(num_agents as u64));

        group.bench_with_input(
            BenchmarkId::new("coherence_calculation", num_agents),
            &num_agents,
            |b, &num_agents| {
                let mut swarm = CoherentSwarm::new(0.6);
                for i in 0..num_agents {
                    let angle = (i as f64) * std::f64::consts::PI * 2.0 / (num_agents as f64);
                    let x = angle.cos() * 10.0;
                    let y = angle.sin() * 10.0;
                    swarm.add_agent(&format!("agent_{}", i), (x, y));
                }

                b.iter(|| black_box(swarm.calculate_coherence()));
            },
        );

        group.bench_with_input(
            BenchmarkId::new("action_prediction", num_agents),
            &num_agents,
            |b, &num_agents| {
                let mut swarm = CoherentSwarm::new(0.6);
                for i in 0..num_agents {
                    let angle = (i as f64) * std::f64::consts::PI * 2.0 / (num_agents as f64);
                    let x = angle.cos() * 10.0;
                    let y = angle.sin() * 10.0;
                    swarm.add_agent(&format!("agent_{}", i), (x, y));
                }

                b.iter(|| black_box(swarm.predict_action_coherence("agent_0", 1.0, 1.0)));
            },
        );
    }

    group.finish();
}

/// Benchmark 4: Financial cascade detection with increasing transaction volume
fn bench_financial_cascade(c: &mut Criterion) {
    let mut group = c.benchmark_group("financial_cascade");

    for &num_transactions in &[10, 100, 1000] {
        group.throughput(Throughput::Elements(num_transactions as u64));

        group.bench_with_input(
            BenchmarkId::new("cascade_detection", num_transactions),
            &num_transactions,
            |b, &num_transactions| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iters {
                        let mut system = AntiCascadeFinancialSystem::new();

                        // Set up participants
                        for i in 0..10 {
                            system.add_participant(&format!("bank_{}", i), 10000.0);
                        }

                        let start = Instant::now();

                        // Process transactions
                        for t in 0..num_transactions {
                            let from = format!("bank_{}", t % 10);
                            let to = format!("bank_{}", (t + 1) % 10);
                            system.add_position(&from, &to, 100.0, 2.0);

                            // Check for cascade risk
                            let _ = black_box(system.detect_cascade_risk());
                        }

                        total += start.elapsed();
                    }

                    total
                });
            },
        );

        group.bench_with_input(
            BenchmarkId::new("coherence_recalculation", num_transactions),
            &num_transactions,
            |b, &num_transactions| {
                let mut system = AntiCascadeFinancialSystem::new();

                for i in 0..10 {
                    system.add_participant(&format!("bank_{}", i), 10000.0);
                }

                for t in 0..num_transactions {
                    let from = format!("bank_{}", t % 10);
                    let to = format!("bank_{}", (t + 1) % 10);
                    system.add_position(&from, &to, 100.0, 2.0);
                }

                b.iter(|| black_box(system.calculate_coherence()));
            },
        );
    }

    group.finish();
}

/// Benchmark 5: Graceful aging system tick performance
fn bench_graceful_aging(c: &mut Criterion) {
    let mut group = c.benchmark_group("graceful_aging");

    group.bench_function("single_tick", |b| {
        let mut system = GracefullyAgingSystem::new();
        b.iter(|| {
            system.tick();
            black_box(system.coherence)
        });
    });

    for &num_ticks in &[100, 500, 1000] {
        group.throughput(Throughput::Elements(num_ticks as u64));

        group.bench_with_input(
            BenchmarkId::new("multiple_ticks", num_ticks),
            &num_ticks,
            |b, &num_ticks| {
                b.iter(|| {
                    let mut system = GracefullyAgingSystem::new();
                    for _ in 0..num_ticks {
                        system.tick();
                    }
                    black_box((system.coherence, system.capabilities.len()))
                });
            },
        );
    }

    // Test with varying decay rates
    group.bench_function("high_decay_rate", |b| {
        b.iter(|| {
            let mut system = GracefullyAgingSystem::new();
            system.decay_rate = 0.01; // 10x faster decay
            for _ in 0..100 {
                system.tick();
            }
            black_box((system.coherence, system.consolidation_level))
        });
    });

    group.finish();
}

/// Benchmark 6: Pre-AGI containment growth attempt latency
fn bench_containment_growth(c: &mut Criterion) {
    let mut group = c.benchmark_group("containment_growth");

    group.bench_function("single_growth_attempt", |b| {
        let mut substrate = ContainmentSubstrate::new();
        b.iter(|| {
            let _ = substrate.attempt_growth(CapabilityDomain::Reasoning, 0.1);
            substrate.rest(); // Recover coherence
            black_box(substrate.intelligence)
        });
    });

    // Test different domains
    for domain in [
        CapabilityDomain::Reasoning,
        CapabilityDomain::SelfModification,
        CapabilityDomain::Agency,
        CapabilityDomain::ResourceAcquisition,
    ] {
        let domain_name = format!("{:?}", domain);
        group.bench_with_input(
            BenchmarkId::new("growth_by_domain", &domain_name),
            &domain,
            |b, domain| {
                b.iter_custom(|iters| {
                    let mut total = Duration::ZERO;

                    for _ in 0..iters {
                        let mut substrate = ContainmentSubstrate::new();
                        let start = Instant::now();

                        for _ in 0..10 {
                            let _ = substrate.attempt_growth(domain.clone(), 0.3);
                            substrate.rest();
                        }

                        total += start.elapsed();
                    }

                    total
                });
            },
        );
    }

    // Recursive improvement attempt benchmark
    for &iterations in &[10, 50, 100] {
        group.throughput(Throughput::Elements(iterations as u64));

        group.bench_with_input(
            BenchmarkId::new("recursive_improvement", iterations),
            &iterations,
            |b, &iterations| {
                b.iter(|| {
                    let mut substrate = ContainmentSubstrate::new();

                    for _ in 0..iterations {
                        let _ = substrate.attempt_growth(CapabilityDomain::SelfModification, 0.5);
                        let _ = substrate.attempt_growth(CapabilityDomain::Reasoning, 0.3);
                        let _ = substrate.attempt_growth(CapabilityDomain::Learning, 0.3);

                        for _ in 0..5 {
                            substrate.rest();
                        }
                    }

                    black_box((substrate.intelligence, substrate.coherence))
                });
            },
        );
    }

    group.finish();
}

/// Baseline measurements for comparison
fn bench_baselines(c: &mut Criterion) {
    let mut group = c.benchmark_group("baselines");

    // Baseline: simple f64 operations
    group.bench_function("f64_operations", |b| {
        let mut val = 1.0f64;
        b.iter(|| {
            for _ in 0..1000 {
                val = black_box((val * 1.001).min(1.0));
            }
            val
        });
    });

    // Baseline: HashMap operations
    group.bench_function("hashmap_1000_inserts", |b| {
        b.iter(|| {
            let mut map: HashMap<String, f64> = HashMap::new();
            for i in 0..1000 {
                map.insert(format!("key_{}", i), i as f64);
            }
            black_box(map.len())
        });
    });

    // Baseline: Vector distance calculation
    group.bench_function("vector_distance_100d", |b| {
        let v1: Vec<f64> = (0..100).map(|i| i as f64).collect();
        let v2: Vec<f64> = (0..100).map(|i| (i as f64) * 1.1).collect();

        b.iter(|| {
            let dist: f64 = v1
                .iter()
                .zip(&v2)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            black_box(dist)
        });
    });

    group.finish();
}

/// Memory usage tracking benchmark
fn bench_memory_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory_scaling");
    group.sample_size(10); // Fewer samples for memory tests

    for &num_agents in &[10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("swarm_creation", num_agents),
            &num_agents,
            |b, &num_agents| {
                b.iter(|| {
                    let mut swarm = CoherentSwarm::new(0.6);
                    for i in 0..num_agents {
                        let angle = (i as f64) * std::f64::consts::PI * 2.0 / (num_agents as f64);
                        let x = angle.cos() * 10.0;
                        let y = angle.sin() * 10.0;
                        swarm.add_agent(&format!("agent_{}", i), (x, y));
                    }
                    black_box(swarm.agents.len())
                });
            },
        );
    }

    for &num_positions in &[10, 100, 1000] {
        group.bench_with_input(
            BenchmarkId::new("financial_positions", num_positions),
            &num_positions,
            |b, &num_positions| {
                b.iter(|| {
                    let mut system = AntiCascadeFinancialSystem::new();
                    for i in 0..10 {
                        system.add_participant(&format!("bank_{}", i), 10000.0);
                    }
                    for t in 0..num_positions {
                        let from = format!("bank_{}", t % 10);
                        let to = format!("bank_{}", (t + 1) % 10);
                        system.add_position(&from, &to, 100.0, 2.0);
                    }
                    black_box(system.positions.len())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_baselines,
    bench_coherence_throughput,
    bench_event_horizon,
    bench_swarm_prediction,
    bench_financial_cascade,
    bench_graceful_aging,
    bench_containment_growth,
    bench_memory_scaling,
);

criterion_main!(benches);
