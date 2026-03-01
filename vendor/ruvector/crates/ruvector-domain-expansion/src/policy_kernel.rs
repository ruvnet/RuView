//! PolicyKernel: Population-Based Policy Search
//!
//! Run a small population of policy variants in parallel.
//! Each variant changes a small set of knobs:
//! - skip mode policy
//! - prepass mode
//! - speculation trigger thresholds
//! - budget allocation
//!
//! Selection: keep top performers on holdouts, mutate knobs, repeat.
//! Only merge deltas that pass replay-verify.

use crate::domain::DomainId;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration knobs that a PolicyKernel can tune.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyKnobs {
    /// Whether to skip low-value operations.
    pub skip_mode: bool,
    /// Run a cheaper prepass before full execution.
    pub prepass_enabled: bool,
    /// Threshold for triggering speculative dual-path [0.0, 1.0].
    pub speculation_threshold: f32,
    /// Budget fraction allocated to exploration vs exploitation [0.0, 1.0].
    pub exploration_budget: f32,
    /// Maximum retries on failure.
    pub max_retries: u32,
    /// Batch size for parallel evaluation.
    pub batch_size: usize,
    /// Cost decay factor for EMA.
    pub cost_decay: f32,
    /// Minimum confidence to skip uncertainty check.
    pub confidence_floor: f32,
}

impl PolicyKnobs {
    /// Sensible defaults.
    pub fn default_knobs() -> Self {
        Self {
            skip_mode: false,
            prepass_enabled: true,
            speculation_threshold: 0.15,
            exploration_budget: 0.2,
            max_retries: 2,
            batch_size: 8,
            cost_decay: 0.9,
            confidence_floor: 0.7,
        }
    }

    /// Mutate knobs with small random perturbations.
    pub fn mutate(&self, rng: &mut impl Rng, mutation_rate: f32) -> Self {
        let mut knobs = self.clone();

        if rng.gen::<f32>() < mutation_rate {
            knobs.skip_mode = !knobs.skip_mode;
        }
        if rng.gen::<f32>() < mutation_rate {
            knobs.prepass_enabled = !knobs.prepass_enabled;
        }
        if rng.gen::<f32>() < mutation_rate {
            let delta: f32 = rng.gen_range(-0.1..0.1);
            knobs.speculation_threshold = (knobs.speculation_threshold + delta).clamp(0.01, 0.5);
        }
        if rng.gen::<f32>() < mutation_rate {
            let delta: f32 = rng.gen_range(-0.1..0.1);
            knobs.exploration_budget = (knobs.exploration_budget + delta).clamp(0.01, 0.5);
        }
        if rng.gen::<f32>() < mutation_rate {
            knobs.max_retries = rng.gen_range(0..5);
        }
        if rng.gen::<f32>() < mutation_rate {
            knobs.batch_size = rng.gen_range(1..32);
        }
        if rng.gen::<f32>() < mutation_rate {
            let delta: f32 = rng.gen_range(-0.05..0.05);
            knobs.cost_decay = (knobs.cost_decay + delta).clamp(0.5, 0.99);
        }
        if rng.gen::<f32>() < mutation_rate {
            let delta: f32 = rng.gen_range(-0.1..0.1);
            knobs.confidence_floor = (knobs.confidence_floor + delta).clamp(0.3, 0.95);
        }

        knobs
    }

    /// Crossover two parent knobs to produce a child.
    pub fn crossover(&self, other: &PolicyKnobs, rng: &mut impl Rng) -> Self {
        Self {
            skip_mode: if rng.gen() {
                self.skip_mode
            } else {
                other.skip_mode
            },
            prepass_enabled: if rng.gen() {
                self.prepass_enabled
            } else {
                other.prepass_enabled
            },
            speculation_threshold: if rng.gen() {
                self.speculation_threshold
            } else {
                other.speculation_threshold
            },
            exploration_budget: if rng.gen() {
                self.exploration_budget
            } else {
                other.exploration_budget
            },
            max_retries: if rng.gen() {
                self.max_retries
            } else {
                other.max_retries
            },
            batch_size: if rng.gen() {
                self.batch_size
            } else {
                other.batch_size
            },
            cost_decay: if rng.gen() {
                self.cost_decay
            } else {
                other.cost_decay
            },
            confidence_floor: if rng.gen() {
                self.confidence_floor
            } else {
                other.confidence_floor
            },
        }
    }
}

/// A PolicyKernel is a versioned policy configuration with performance history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyKernel {
    /// Unique identifier.
    pub id: String,
    /// Configuration knobs.
    pub knobs: PolicyKnobs,
    /// Performance on holdout tasks (domain_id -> score).
    pub holdout_scores: HashMap<DomainId, f32>,
    /// Total cost incurred.
    pub total_cost: f32,
    /// Number of evaluation cycles.
    pub cycles: u64,
    /// Generation (0 = initial, increments on mutation).
    pub generation: u32,
    /// Parent kernel ID (for lineage tracking).
    pub parent_id: Option<String>,
    /// Whether this kernel has been verified via replay.
    pub replay_verified: bool,
}

impl PolicyKernel {
    /// Create a new kernel with default knobs.
    pub fn new(id: String) -> Self {
        Self {
            id,
            knobs: PolicyKnobs::default_knobs(),
            holdout_scores: HashMap::new(),
            total_cost: 0.0,
            cycles: 0,
            generation: 0,
            parent_id: None,
            replay_verified: false,
        }
    }

    /// Create a mutated child kernel.
    pub fn mutate(&self, child_id: String, rng: &mut impl Rng) -> Self {
        Self {
            id: child_id,
            knobs: self.knobs.mutate(rng, 0.3),
            holdout_scores: HashMap::new(),
            total_cost: 0.0,
            cycles: 0,
            generation: self.generation + 1,
            parent_id: Some(self.id.clone()),
            replay_verified: false,
        }
    }

    /// Record a holdout score for a domain.
    pub fn record_score(&mut self, domain_id: DomainId, score: f32, cost: f32) {
        self.holdout_scores.insert(domain_id, score);
        self.total_cost += cost;
        self.cycles += 1;
    }

    /// Fitness: average holdout score across all evaluated domains.
    pub fn fitness(&self) -> f32 {
        if self.holdout_scores.is_empty() {
            return 0.0;
        }
        let total: f32 = self.holdout_scores.values().sum();
        total / self.holdout_scores.len() as f32
    }

    /// Cost-adjusted fitness: penalizes expensive kernels.
    pub fn cost_adjusted_fitness(&self) -> f32 {
        let raw = self.fitness();
        let cost_penalty = (self.total_cost / self.cycles.max(1) as f32).min(1.0);
        raw * (1.0 - cost_penalty * 0.3) // 30% weight on cost
    }
}

/// Population-based policy search engine.
#[derive(Clone)]
pub struct PopulationSearch {
    /// Current population of kernels.
    population: Vec<PolicyKernel>,
    /// Population size.
    pop_size: usize,
    /// Best kernel seen so far.
    best_kernel: Option<PolicyKernel>,
    /// Generation counter.
    generation: u32,
}

impl PopulationSearch {
    /// Create a new population search with initial random population.
    pub fn new(pop_size: usize) -> Self {
        let mut rng = rand::thread_rng();
        let population: Vec<PolicyKernel> = (0..pop_size)
            .map(|i| {
                let mut kernel = PolicyKernel::new(format!("kernel_g0_{}", i));
                // Random initial knobs
                kernel.knobs = PolicyKnobs::default_knobs().mutate(&mut rng, 0.8);
                kernel
            })
            .collect();

        Self {
            population,
            pop_size,
            best_kernel: None,
            generation: 0,
        }
    }

    /// Get current population for evaluation.
    pub fn population(&self) -> &[PolicyKernel] {
        &self.population
    }

    /// Get mutable reference to a kernel by index.
    pub fn kernel_mut(&mut self, index: usize) -> Option<&mut PolicyKernel> {
        self.population.get_mut(index)
    }

    /// Evolve to next generation: select top performers, mutate, fill population.
    pub fn evolve(&mut self) {
        let mut rng = rand::thread_rng();
        self.generation += 1;

        // Sort by cost-adjusted fitness (descending)
        self.population.sort_by(|a, b| {
            b.cost_adjusted_fitness()
                .partial_cmp(&a.cost_adjusted_fitness())
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Track best
        if let Some(best) = self.population.first() {
            if self
                .best_kernel
                .as_ref()
                .map_or(true, |b| best.fitness() > b.fitness())
            {
                self.best_kernel = Some(best.clone());
            }
        }

        // Elite selection: keep top 25%
        let elite_count = (self.pop_size / 4).max(1);
        let elites: Vec<PolicyKernel> = self.population[..elite_count].to_vec();

        // Build next generation
        let mut next_gen = Vec::with_capacity(self.pop_size);

        // Keep elites
        for elite in &elites {
            let mut kept = elite.clone();
            kept.id = format!("kernel_g{}_{}", self.generation, next_gen.len());
            kept.holdout_scores.clear();
            kept.total_cost = 0.0;
            kept.cycles = 0;
            next_gen.push(kept);
        }

        // Fill rest with mutations and crossovers
        while next_gen.len() < self.pop_size {
            let parent_idx = rng.gen_range(0..elites.len());
            let child_id = format!("kernel_g{}_{}", self.generation, next_gen.len());

            let child = if rng.gen::<f32>() < 0.3 && elites.len() > 1 {
                // Crossover
                let other_idx =
                    (parent_idx + 1 + rng.gen_range(0..elites.len() - 1)) % elites.len();
                let mut child = PolicyKernel::new(child_id);
                child.knobs = elites[parent_idx]
                    .knobs
                    .crossover(&elites[other_idx].knobs, &mut rng);
                child.generation = self.generation;
                child.parent_id = Some(elites[parent_idx].id.clone());
                child
            } else {
                // Mutation
                elites[parent_idx].mutate(child_id, &mut rng)
            };

            next_gen.push(child);
        }

        self.population = next_gen;
    }

    /// Get the best kernel found so far.
    pub fn best(&self) -> Option<&PolicyKernel> {
        self.best_kernel.as_ref()
    }

    /// Current generation number.
    pub fn generation(&self) -> u32 {
        self.generation
    }

    /// Get fitness statistics for the current population.
    pub fn stats(&self) -> PopulationStats {
        let fitnesses: Vec<f32> = self.population.iter().map(|k| k.fitness()).collect();
        let mean = fitnesses.iter().sum::<f32>() / fitnesses.len().max(1) as f32;
        let max = fitnesses.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let min = fitnesses.iter().cloned().fold(f32::INFINITY, f32::min);
        let variance = fitnesses.iter().map(|f| (f - mean).powi(2)).sum::<f32>()
            / fitnesses.len().max(1) as f32;

        PopulationStats {
            generation: self.generation,
            pop_size: self.population.len(),
            mean_fitness: mean,
            max_fitness: max,
            min_fitness: min,
            fitness_variance: variance,
            best_ever_fitness: self
                .best_kernel
                .as_ref()
                .map(|k| k.fitness())
                .unwrap_or(0.0),
        }
    }
}

/// Statistics about the current population.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationStats {
    pub generation: u32,
    pub pop_size: usize,
    pub mean_fitness: f32,
    pub max_fitness: f32,
    pub min_fitness: f32,
    pub fitness_variance: f32,
    pub best_ever_fitness: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_knobs_default() {
        let knobs = PolicyKnobs::default_knobs();
        assert!(!knobs.skip_mode);
        assert!(knobs.prepass_enabled);
        assert!(knobs.speculation_threshold > 0.0);
    }

    #[test]
    fn test_policy_knobs_mutate() {
        let knobs = PolicyKnobs::default_knobs();
        let mut rng = rand::thread_rng();
        let mutated = knobs.mutate(&mut rng, 1.0); // high mutation rate
                                                   // At least something should differ (probabilistically)
                                                   // Can't guarantee due to randomness, but bounds should hold
        assert!(mutated.speculation_threshold >= 0.01 && mutated.speculation_threshold <= 0.5);
        assert!(mutated.exploration_budget >= 0.01 && mutated.exploration_budget <= 0.5);
    }

    #[test]
    fn test_policy_kernel_fitness() {
        let mut kernel = PolicyKernel::new("test".into());
        assert_eq!(kernel.fitness(), 0.0);

        kernel.record_score(DomainId("d1".into()), 0.8, 1.0);
        kernel.record_score(DomainId("d2".into()), 0.6, 1.0);
        assert!((kernel.fitness() - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_population_search_evolve() {
        let mut search = PopulationSearch::new(8);
        assert_eq!(search.population().len(), 8);

        // Simulate evaluation
        for i in 0..8 {
            if let Some(kernel) = search.kernel_mut(i) {
                let score = 0.3 + (i as f32) * 0.08;
                kernel.record_score(DomainId("test".into()), score, 1.0);
            }
        }

        search.evolve();
        assert_eq!(search.population().len(), 8);
        assert_eq!(search.generation(), 1);
        assert!(search.best().is_some());
    }

    #[test]
    fn test_population_stats() {
        let mut search = PopulationSearch::new(4);

        for i in 0..4 {
            if let Some(kernel) = search.kernel_mut(i) {
                kernel.record_score(DomainId("test".into()), (i as f32) * 0.25, 1.0);
            }
        }

        let stats = search.stats();
        assert_eq!(stats.pop_size, 4);
        assert!(stats.max_fitness >= stats.min_fitness);
        assert!(stats.mean_fitness >= stats.min_fitness);
        assert!(stats.mean_fitness <= stats.max_fitness);
    }

    #[test]
    fn test_crossover() {
        let a = PolicyKnobs {
            skip_mode: true,
            prepass_enabled: false,
            speculation_threshold: 0.1,
            exploration_budget: 0.1,
            max_retries: 1,
            batch_size: 4,
            cost_decay: 0.8,
            confidence_floor: 0.5,
        };
        let b = PolicyKnobs {
            skip_mode: false,
            prepass_enabled: true,
            speculation_threshold: 0.4,
            exploration_budget: 0.4,
            max_retries: 4,
            batch_size: 16,
            cost_decay: 0.95,
            confidence_floor: 0.9,
        };

        let mut rng = rand::thread_rng();
        let child = a.crossover(&b, &mut rng);

        // Child values should come from one parent or the other
        assert!(child.max_retries == 1 || child.max_retries == 4);
        assert!(child.batch_size == 4 || child.batch_size == 16);
    }
}
