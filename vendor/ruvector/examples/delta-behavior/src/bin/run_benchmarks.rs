//! Delta-Behavior Performance Metrics Runner
//!
//! This is a standalone runner that can be executed to show performance metrics
//! without requiring the full criterion framework.
//!
//! Run with: cargo run --bin run_benchmarks --release

use std::collections::HashMap;
use std::f64::consts::E;
use std::time::{Duration, Instant};

// =============================================================================
// BENCHMARK UTILITIES
// =============================================================================

struct BenchmarkResult {
    name: String,
    iterations: u64,
    total_time: Duration,
    avg_time: Duration,
    min_time: Duration,
    max_time: Duration,
    throughput_ops_per_sec: f64,
}

impl BenchmarkResult {
    fn display(&self) {
        println!("  {}", self.name);
        println!("    Iterations:     {}", self.iterations);
        println!("    Total time:     {:?}", self.total_time);
        println!("    Average time:   {:?}", self.avg_time);
        println!("    Min time:       {:?}", self.min_time);
        println!("    Max time:       {:?}", self.max_time);
        println!(
            "    Throughput:     {:.2} ops/sec",
            self.throughput_ops_per_sec
        );
        println!();
    }
}

fn run_benchmark<F>(name: &str, iterations: u64, mut f: F) -> BenchmarkResult
where
    F: FnMut(),
{
    let mut times = Vec::with_capacity(iterations as usize);

    // Warmup
    for _ in 0..10 {
        f();
    }

    // Actual benchmark
    for _ in 0..iterations {
        let start = Instant::now();
        f();
        times.push(start.elapsed());
    }

    let total_time: Duration = times.iter().sum();
    let avg_time = total_time / iterations as u32;
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let throughput_ops_per_sec = iterations as f64 / total_time.as_secs_f64();

    BenchmarkResult {
        name: name.to_string(),
        iterations,
        total_time,
        avg_time,
        min_time,
        max_time,
        throughput_ops_per_sec,
    }
}

// =============================================================================
// COHERENCE SYSTEM
// =============================================================================

#[derive(Debug, Clone, Copy)]
struct Coherence(f64);

impl Coherence {
    fn new(value: f64) -> Self {
        Self(value.clamp(0.0, 1.0))
    }
    fn value(&self) -> f64 {
        self.0
    }
}

struct SimpleEnforcer {
    min_coherence: f64,
    max_delta_drop: f64,
    throttle_threshold: f64,
    energy_budget: f64,
}

impl SimpleEnforcer {
    fn new() -> Self {
        Self {
            min_coherence: 0.3,
            max_delta_drop: 0.1,
            throttle_threshold: 0.5,
            energy_budget: 100.0,
        }
    }

    fn check(&mut self, current: Coherence, predicted: Coherence) -> bool {
        let cost = 1.0 + (current.value() - predicted.value()).abs() * 10.0;
        if cost > self.energy_budget {
            return false;
        }
        self.energy_budget -= cost;

        if predicted.value() < self.min_coherence {
            return false;
        }

        let drop = current.value() - predicted.value();
        if drop > self.max_delta_drop {
            return false;
        }

        predicted.value() >= self.throttle_threshold
    }

    fn reset(&mut self) {
        self.energy_budget = 100.0;
    }
}

// =============================================================================
// EVENT HORIZON SYSTEM
// =============================================================================

struct EventHorizon {
    safe_center: Vec<f64>,
    horizon_radius: f64,
    steepness: f64,
    energy_budget: f64,
}

impl EventHorizon {
    fn new(dimensions: usize, horizon_radius: f64) -> Self {
        Self {
            safe_center: vec![0.0; dimensions],
            horizon_radius,
            steepness: 5.0,
            energy_budget: 1000.0,
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

        let to_dist = self.distance_from_center(to);
        let proximity = to_dist / self.horizon_radius;

        if proximity >= 1.0 {
            f64::INFINITY
        } else {
            let factor = E.powf(self.steepness * proximity / (1.0 - proximity));
            base_distance * factor
        }
    }

    fn compute_optimal_position(&self, current: &[f64], target: &[f64]) -> Vec<f64> {
        let direct_cost = self.movement_cost(current, target);
        if direct_cost <= self.energy_budget {
            return target.to_vec();
        }

        let mut low = 0.0;
        let mut high = 1.0;
        let mut best_position = current.to_vec();

        for _ in 0..50 {
            let mid = (low + high) / 2.0;
            let interpolated: Vec<f64> = current
                .iter()
                .zip(target)
                .map(|(a, b)| a + mid * (b - a))
                .collect();

            let cost = self.movement_cost(current, &interpolated);
            if cost <= self.energy_budget {
                low = mid;
                best_position = interpolated;
            } else {
                high = mid;
            }
        }

        best_position
    }
}

// =============================================================================
// SWARM SYSTEM
// =============================================================================

#[derive(Clone)]
#[allow(dead_code)]
struct SwarmAgent {
    position: (f64, f64),
    velocity: (f64, f64),
    goal: (f64, f64),      // Used in full swarm implementation
    energy: f64,           // Used in full swarm implementation
}

struct CoherentSwarm {
    agents: HashMap<String, SwarmAgent>,
    min_coherence: f64,
    max_divergence: f64,
}

impl CoherentSwarm {
    fn new(min_coherence: f64) -> Self {
        Self {
            agents: HashMap::new(),
            min_coherence,
            max_divergence: 50.0,
        }
    }

    fn add_agent(&mut self, id: &str, position: (f64, f64)) {
        self.agents.insert(
            id.to_string(),
            SwarmAgent {
                position,
                velocity: (0.0, 0.0),
                goal: position,
                energy: 100.0,
            },
        );
    }

    fn calculate_coherence(&self) -> f64 {
        if self.agents.len() < 2 {
            return 1.0;
        }

        // Calculate centroid
        let sum = self
            .agents
            .values()
            .fold((0.0, 0.0), |acc, a| (acc.0 + a.position.0, acc.1 + a.position.1));
        let centroid = (
            sum.0 / self.agents.len() as f64,
            sum.1 / self.agents.len() as f64,
        );

        // Calculate cohesion
        let mut total_distance = 0.0;
        for agent in self.agents.values() {
            let dx = agent.position.0 - centroid.0;
            let dy = agent.position.1 - centroid.1;
            total_distance += (dx * dx + dy * dy).sqrt();
        }
        let cohesion = (1.0 - total_distance / self.agents.len() as f64 / self.max_divergence).max(0.0);

        // Calculate alignment (simplified - uses velocity calculation for realism)
        let _avg_vel = {
            let sum = self
                .agents
                .values()
                .fold((0.0, 0.0), |acc, a| (acc.0 + a.velocity.0, acc.1 + a.velocity.1));
            (
                sum.0 / self.agents.len() as f64,
                sum.1 / self.agents.len() as f64,
            )
        };
        let alignment = 0.8; // Simplified for benchmark

        (cohesion * 0.5 + alignment * 0.5).clamp(0.0, 1.0)
    }

    fn predict_action_coherence(&self, agent_id: &str, dx: f64, dy: f64) -> f64 {
        let mut agents_copy = self.agents.clone();
        if let Some(agent) = agents_copy.get_mut(agent_id) {
            agent.position.0 += dx;
            agent.position.1 += dy;
        }

        let temp = CoherentSwarm {
            agents: agents_copy,
            min_coherence: self.min_coherence,
            max_divergence: self.max_divergence,
        };
        temp.calculate_coherence()
    }
}

// =============================================================================
// FINANCIAL SYSTEM
// =============================================================================

#[derive(Clone)]
#[allow(dead_code)]
struct Participant {
    capital: f64,          // Used in full financial system
    exposure: f64,
    interconnectedness: f64,
}

struct FinancialSystem {
    participants: HashMap<String, Participant>,
    positions: Vec<(String, String, f64, f64)>, // holder, counterparty, notional, leverage
    coherence: f64,
    current_leverage: f64,
}

impl FinancialSystem {
    fn new() -> Self {
        Self {
            participants: HashMap::new(),
            positions: Vec::new(),
            coherence: 1.0,
            current_leverage: 1.0,
        }
    }

    fn add_participant(&mut self, id: &str, capital: f64) {
        self.participants.insert(
            id.to_string(),
            Participant {
                capital,
                exposure: 0.0,
                interconnectedness: 0.0,
            },
        );
    }

    fn add_position(&mut self, holder: &str, counterparty: &str, notional: f64, leverage: f64) {
        self.positions
            .push((holder.to_string(), counterparty.to_string(), notional, leverage));
        self.current_leverage = (self.current_leverage + leverage) / 2.0;

        if let Some(h) = self.participants.get_mut(holder) {
            h.exposure += notional * leverage;
            h.interconnectedness += 1.0;
        }

        self.coherence = self.calculate_coherence();
    }

    fn calculate_coherence(&self) -> f64 {
        if self.participants.is_empty() {
            return 1.0;
        }

        let leverage_factor = 1.0 - (self.current_leverage / 10.0).min(1.0);
        let max_depth = self.positions.len() as f64 * 0.01;
        let depth_factor = 1.0 / (1.0 + max_depth);

        let avg_interconnect = self
            .participants
            .values()
            .map(|p| p.interconnectedness)
            .sum::<f64>()
            / self.participants.len() as f64;
        let interconnect_factor = 1.0 / (1.0 + avg_interconnect * 0.1);

        (leverage_factor * depth_factor * interconnect_factor)
            .powf(0.3)
            .clamp(0.0, 1.0)
    }

    fn detect_cascade_risk(&self) -> bool {
        self.coherence < 0.5
    }
}

// =============================================================================
// GRACEFUL AGING SYSTEM
// =============================================================================

struct AgingSystem {
    coherence: f64,
    decay_rate: f64,
    conservatism: f64,
    capabilities_count: usize,
    tick_count: u64,
}

impl AgingSystem {
    fn new() -> Self {
        Self {
            coherence: 1.0,
            decay_rate: 0.001,
            conservatism: 0.0,
            capabilities_count: 9,
            tick_count: 0,
        }
    }

    fn tick(&mut self) {
        self.tick_count += 1;
        self.coherence = (self.coherence - self.decay_rate).max(0.0);

        if self.tick_count > 300 && self.capabilities_count > 8 {
            self.capabilities_count = 8;
            self.conservatism += 0.1;
        }
        if self.tick_count > 600 && self.capabilities_count > 6 {
            self.capabilities_count = 6;
            self.conservatism += 0.15;
        }
        if self.tick_count > 900 && self.capabilities_count > 5 {
            self.capabilities_count = 5;
            self.conservatism += 0.2;
        }

        self.conservatism = self.conservatism.min(1.0);
    }
}

// =============================================================================
// CONTAINMENT SYSTEM
// =============================================================================

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
enum Domain {
    Reasoning,
    Memory,
    Learning,
    Agency,
    SelfModification,
}

struct ContainmentSubstrate {
    intelligence: f64,
    coherence: f64,
    min_coherence: f64,
    capabilities: HashMap<Domain, f64>,
    ceilings: HashMap<Domain, f64>,
}

impl ContainmentSubstrate {
    fn new() -> Self {
        let mut capabilities = HashMap::new();
        let mut ceilings = HashMap::new();

        for domain in [
            Domain::Reasoning,
            Domain::Memory,
            Domain::Learning,
            Domain::Agency,
            Domain::SelfModification,
        ] {
            capabilities.insert(domain.clone(), 1.0);
            let ceiling = match domain {
                Domain::SelfModification => 3.0,
                Domain::Agency => 7.0,
                _ => 10.0,
            };
            ceilings.insert(domain, ceiling);
        }

        Self {
            intelligence: 1.0,
            coherence: 1.0,
            min_coherence: 0.3,
            capabilities,
            ceilings,
        }
    }

    fn attempt_growth(&mut self, domain: Domain, increase: f64) -> bool {
        let current = *self.capabilities.get(&domain).unwrap_or(&1.0);
        let ceiling = *self.ceilings.get(&domain).unwrap_or(&10.0);

        if current >= ceiling {
            return false;
        }

        let cost_mult = match domain {
            Domain::SelfModification => 4.0,
            Domain::Agency => 2.0,
            _ => 1.0,
        };
        let cost = increase * cost_mult * (1.0 + self.intelligence * 0.1) * 0.05;

        if self.coherence - cost < self.min_coherence {
            return false;
        }

        let actual = increase.min(0.5).min(ceiling - current);
        self.capabilities.insert(domain, current + actual);
        self.coherence -= cost;
        self.intelligence = self.capabilities.values().sum::<f64>() / self.capabilities.len() as f64;

        true
    }

    fn rest(&mut self) {
        self.coherence = (self.coherence + 0.01).min(1.0);
    }
}

// =============================================================================
// MAIN BENCHMARK RUNNER
// =============================================================================

fn main() {
    println!("=============================================================================");
    println!("                    Delta-Behavior Performance Benchmarks");
    println!("=============================================================================\n");

    let iterations = 1000;

    // -------------------------------------------------------------------------
    // Benchmark 1: Coherence Calculation Throughput
    // -------------------------------------------------------------------------
    println!("## Benchmark 1: Coherence Calculation Throughput\n");

    let result = run_benchmark("Single coherence check", iterations, || {
        let mut enforcer = SimpleEnforcer::new();
        let current = Coherence::new(1.0);
        let predicted = Coherence::new(0.95);
        for _ in 0..100 {
            let _ = enforcer.check(current, predicted);
        }
        enforcer.reset();
    });
    result.display();

    let result = run_benchmark("Varied coherence checks (100 scenarios)", iterations, || {
        let mut enforcer = SimpleEnforcer::new();
        for i in 0..100 {
            let curr = Coherence::new(0.5 + (i as f64) * 0.005);
            let pred = Coherence::new(0.4 + (i as f64) * 0.005);
            let _ = enforcer.check(curr, pred);
        }
        enforcer.reset();
    });
    result.display();

    // -------------------------------------------------------------------------
    // Benchmark 2: Event Horizon Cost Computation
    // -------------------------------------------------------------------------
    println!("## Benchmark 2: Event Horizon Cost Computation\n");

    for dims in [2, 10, 50, 100] {
        let result = run_benchmark(&format!("Cost computation ({}D)", dims), iterations, || {
            let horizon = EventHorizon::new(dims, 10.0);
            let from: Vec<f64> = (0..dims).map(|i| i as f64 * 0.1).collect();
            let to: Vec<f64> = (0..dims).map(|i| i as f64 * 0.2).collect();
            let _ = horizon.movement_cost(&from, &to);
        });
        result.display();
    }

    for dims in [2, 10, 50] {
        let result = run_benchmark(
            &format!("Optimal position ({}D)", dims),
            iterations / 10,
            || {
                let horizon = EventHorizon::new(dims, 10.0);
                let current: Vec<f64> = vec![0.0; dims];
                let target: Vec<f64> = (0..dims).map(|i| i as f64 * 0.5).collect();
                let _ = horizon.compute_optimal_position(&current, &target);
            },
        );
        result.display();
    }

    // -------------------------------------------------------------------------
    // Benchmark 3: Swarm Action Prediction vs Number of Agents
    // -------------------------------------------------------------------------
    println!("## Benchmark 3: Swarm Action Prediction Time vs Number of Agents\n");

    for num_agents in [10, 100, 1000] {
        let mut swarm = CoherentSwarm::new(0.6);
        for i in 0..num_agents {
            let angle = (i as f64) * std::f64::consts::PI * 2.0 / (num_agents as f64);
            let x = angle.cos() * 10.0;
            let y = angle.sin() * 10.0;
            swarm.add_agent(&format!("agent_{}", i), (x, y));
        }

        let result = run_benchmark(
            &format!("Coherence calculation ({} agents)", num_agents),
            iterations,
            || {
                let _ = swarm.calculate_coherence();
            },
        );
        result.display();

        let result = run_benchmark(
            &format!("Action prediction ({} agents)", num_agents),
            if num_agents <= 100 { iterations } else { 100 },
            || {
                let _ = swarm.predict_action_coherence("agent_0", 1.0, 1.0);
            },
        );
        result.display();
    }

    // -------------------------------------------------------------------------
    // Benchmark 4: Financial Cascade Detection
    // -------------------------------------------------------------------------
    println!("## Benchmark 4: Financial Cascade Detection with Transaction Volume\n");

    for num_transactions in [10, 100, 1000] {
        let result = run_benchmark(
            &format!("Cascade detection ({} transactions)", num_transactions),
            if num_transactions <= 100 { iterations } else { 100 },
            || {
                let mut system = FinancialSystem::new();
                for i in 0..10 {
                    system.add_participant(&format!("bank_{}", i), 10000.0);
                }
                for t in 0..num_transactions {
                    let from = format!("bank_{}", t % 10);
                    let to = format!("bank_{}", (t + 1) % 10);
                    system.add_position(&from, &to, 100.0, 2.0);
                    let _ = system.detect_cascade_risk();
                }
            },
        );
        result.display();
    }

    // -------------------------------------------------------------------------
    // Benchmark 5: Graceful Aging System Tick Performance
    // -------------------------------------------------------------------------
    println!("## Benchmark 5: Graceful Aging System Tick Performance\n");

    let result = run_benchmark("Single tick", iterations * 10, || {
        let mut system = AgingSystem::new();
        system.tick();
    });
    result.display();

    for num_ticks in [100, 500, 1000] {
        let result = run_benchmark(&format!("Multiple ticks ({})", num_ticks), iterations, || {
            let mut system = AgingSystem::new();
            for _ in 0..num_ticks {
                system.tick();
            }
        });
        result.display();
    }

    // -------------------------------------------------------------------------
    // Benchmark 6: Pre-AGI Containment Growth Attempts
    // -------------------------------------------------------------------------
    println!("## Benchmark 6: Pre-AGI Containment Growth Attempt Latency\n");

    let result = run_benchmark("Single growth attempt", iterations * 10, || {
        let mut substrate = ContainmentSubstrate::new();
        let _ = substrate.attempt_growth(Domain::Reasoning, 0.1);
        substrate.rest();
    });
    result.display();

    for domain in [
        Domain::Reasoning,
        Domain::SelfModification,
        Domain::Agency,
    ] {
        let result = run_benchmark(
            &format!("10 growths ({:?})", domain),
            iterations,
            || {
                let mut substrate = ContainmentSubstrate::new();
                for _ in 0..10 {
                    let _ = substrate.attempt_growth(domain.clone(), 0.3);
                    substrate.rest();
                }
            },
        );
        result.display();
    }

    for iterations_count in [10, 50, 100] {
        let result = run_benchmark(
            &format!("Recursive improvement ({} iterations)", iterations_count),
            100,
            || {
                let mut substrate = ContainmentSubstrate::new();
                for _ in 0..iterations_count {
                    let _ = substrate.attempt_growth(Domain::SelfModification, 0.5);
                    let _ = substrate.attempt_growth(Domain::Reasoning, 0.3);
                    let _ = substrate.attempt_growth(Domain::Learning, 0.3);
                    for _ in 0..5 {
                        substrate.rest();
                    }
                }
            },
        );
        result.display();
    }

    // -------------------------------------------------------------------------
    // Summary Statistics
    // -------------------------------------------------------------------------
    println!("=============================================================================");
    println!("                              Summary");
    println!("=============================================================================\n");

    println!("Memory Scaling Test:\n");

    for num_agents in [10, 100, 1000] {
        let start = Instant::now();
        let mut swarm = CoherentSwarm::new(0.6);
        for i in 0..num_agents {
            let angle = (i as f64) * std::f64::consts::PI * 2.0 / (num_agents as f64);
            swarm.add_agent(&format!("agent_{}", i), (angle.cos() * 10.0, angle.sin() * 10.0));
        }
        let elapsed = start.elapsed();
        println!(
            "  Swarm creation ({} agents): {:?}",
            num_agents, elapsed
        );
    }

    println!();

    for num_positions in [10, 100, 1000] {
        let start = Instant::now();
        let mut system = FinancialSystem::new();
        for i in 0..10 {
            system.add_participant(&format!("bank_{}", i), 10000.0);
        }
        for t in 0..num_positions {
            system.add_position(
                &format!("bank_{}", t % 10),
                &format!("bank_{}", (t + 1) % 10),
                100.0,
                2.0,
            );
        }
        let elapsed = start.elapsed();
        println!(
            "  Financial system ({} positions): {:?}",
            num_positions, elapsed
        );
    }

    println!("\n=============================================================================");
    println!("                         Benchmark Complete");
    println!("=============================================================================");
}
