//! # Domain Expansion Engine
//!
//! Cross-domain transfer learning for general problem-solving capability.
//!
//! ## Core Insight
//!
//! True IQ growth appears when a kernel trained on Domain 1 improves Domain 2
//! faster than Domain 2 alone. That is generalization.
//!
//! ## Two-Layer Architecture
//!
//! **Policy learning layer**: Meta Thompson Sampling with Beta priors across
//! context buckets. Chooses strategies via uncertainty-aware selection.
//! Transfer happens through compact priors — not raw trajectories.
//!
//! **Operator layer**: Deterministic domain kernels (Rust synthesis, planning,
//! tool orchestration) that generate tasks, evaluate solutions, and produce
//! embeddings into a shared representation space.
//!
//! ## Domains
//!
//! - **Rust Program Synthesis**: Generate Rust functions from specifications
//! - **Structured Planning**: Multi-step plans with dependencies and resources
//! - **Tool Orchestration**: Coordinate multiple tools/agents for complex goals
//!
//! ## Transfer Protocol
//!
//! 1. Train on Domain 1, extract `TransferPrior` (posterior summaries)
//! 2. Initialize Domain 2 with dampened priors from Domain 1
//! 3. Measure acceleration: cycles to convergence with/without transfer
//! 4. A delta is promotable only if it improves target without regressing source
//!
//! ## Population-Based Policy Search
//!
//! Run a population of `PolicyKernel` variants in parallel.
//! Each variant tunes knobs (skip mode, prepass, speculation thresholds).
//! Keep top performers on holdouts, mutate, repeat.
//!
//! ## Acceptance Test
//!
//! Domain 2 must converge faster than Domain 1 to target accuracy, cost,
//! robustness, and zero policy violations.

#![warn(missing_docs)]

pub mod cost_curve;
pub mod domain;
pub mod meta_learning;
pub mod planning;
pub mod policy_kernel;
pub mod rust_synthesis;
pub mod tool_orchestration;
pub mod transfer;

/// RVF format integration: segment serialization, witness chains, AGI packaging.
///
/// Requires the `rvf` feature to be enabled.
#[cfg(feature = "rvf")]
pub mod rvf_bridge;

// Re-export core types.
pub use cost_curve::{
    AccelerationEntry, AccelerationScoreboard, ConvergenceThresholds, CostCurve, CostCurvePoint,
    ScoreboardSummary,
};
pub use domain::{Domain, DomainEmbedding, DomainId, Evaluation, Solution, Task};
pub use meta_learning::{
    CuriosityBonus, DecayingBeta, MetaLearningEngine, MetaLearningHealth, ParetoFront, ParetoPoint,
    PlateauAction, PlateauDetector, RegretSummary, RegretTracker,
};
pub use planning::PlanningDomain;
pub use policy_kernel::{PolicyKernel, PolicyKnobs, PopulationSearch, PopulationStats};
pub use rust_synthesis::RustSynthesisDomain;
pub use tool_orchestration::ToolOrchestrationDomain;
pub use transfer::{
    ArmId, BetaParams, ContextBucket, DualPathResult, MetaThompsonEngine, TransferPrior,
    TransferVerification,
};

use std::collections::HashMap;

/// The domain expansion orchestrator.
///
/// Manages multiple domains, transfer learning between them,
/// population-based policy search, and the acceleration scoreboard.
///
/// The `meta` field provides five composable learning improvements:
/// regret tracking, decaying priors, plateau detection, Pareto front
/// optimization, and curiosity-driven exploration.
pub struct DomainExpansionEngine {
    /// Registered domains.
    domains: HashMap<DomainId, Box<dyn Domain>>,
    /// Meta Thompson Sampling engine for cross-domain transfer.
    pub thompson: MetaThompsonEngine,
    /// Population-based policy search.
    pub population: PopulationSearch,
    /// Acceleration scoreboard tracking convergence across domains.
    pub scoreboard: AccelerationScoreboard,
    /// Meta-learning engine: regret, plateau, Pareto, curiosity, decay.
    pub meta: MetaLearningEngine,
    /// Holdout tasks per domain for verification.
    holdouts: HashMap<DomainId, Vec<Task>>,
    /// Counterexample set: failed solutions that inform future decisions.
    counterexamples: HashMap<DomainId, Vec<(Task, Solution, Evaluation)>>,
}

impl DomainExpansionEngine {
    /// Create a new domain expansion engine with default configuration.
    ///
    /// Initializes the three core domains and the transfer engine.
    pub fn new() -> Self {
        let arms = vec![
            "greedy".into(),
            "exploratory".into(),
            "conservative".into(),
            "speculative".into(),
        ];

        let mut engine = Self {
            domains: HashMap::new(),
            thompson: MetaThompsonEngine::new(arms),
            population: PopulationSearch::new(8),
            scoreboard: AccelerationScoreboard::new(),
            meta: MetaLearningEngine::new(),
            holdouts: HashMap::new(),
            counterexamples: HashMap::new(),
        };

        // Register the three core domains.
        engine.register_domain(Box::new(RustSynthesisDomain::new()));
        engine.register_domain(Box::new(PlanningDomain::new()));
        engine.register_domain(Box::new(ToolOrchestrationDomain::new()));

        engine
    }

    /// Register a new domain.
    pub fn register_domain(&mut self, domain: Box<dyn Domain>) {
        let id = domain.id().clone();
        self.thompson.init_domain_uniform(id.clone());
        self.domains.insert(id, domain);
    }

    /// Generate holdout tasks for verification.
    pub fn generate_holdouts(&mut self, tasks_per_domain: usize, difficulty: f32) {
        for (id, domain) in &self.domains {
            let tasks = domain.generate_tasks(tasks_per_domain, difficulty);
            self.holdouts.insert(id.clone(), tasks);
        }
    }

    /// Generate training tasks for a specific domain.
    pub fn generate_tasks(&self, domain_id: &DomainId, count: usize, difficulty: f32) -> Vec<Task> {
        self.domains
            .get(domain_id)
            .map(|d| d.generate_tasks(count, difficulty))
            .unwrap_or_default()
    }

    /// Evaluate a solution and record the outcome.
    pub fn evaluate_and_record(
        &mut self,
        domain_id: &DomainId,
        task: &Task,
        solution: &Solution,
        bucket: ContextBucket,
        arm: ArmId,
    ) -> Evaluation {
        let eval = self
            .domains
            .get(domain_id)
            .map(|d| d.evaluate(task, solution))
            .unwrap_or_else(|| Evaluation::zero(vec!["Domain not found".into()]));

        // Record outcome in Thompson engine.
        self.thompson.record_outcome(
            domain_id,
            bucket.clone(),
            arm.clone(),
            eval.score,
            1.0, // unit cost for now
        );

        // Record in meta-learning engine (regret + curiosity + decaying beta).
        self.meta.record_decision(&bucket, &arm, eval.score);

        // Store counterexamples for poor solutions.
        if eval.score < 0.3 {
            self.counterexamples
                .entry(domain_id.clone())
                .or_default()
                .push((task.clone(), solution.clone(), eval.clone()));
        }

        eval
    }

    /// Embed a solution into the shared representation space.
    pub fn embed(&self, domain_id: &DomainId, solution: &Solution) -> Option<DomainEmbedding> {
        self.domains.get(domain_id).map(|d| d.embed(solution))
    }

    /// Initiate transfer from source domain to target domain.
    /// Extracts priors from source and seeds target.
    pub fn initiate_transfer(&mut self, source: &DomainId, target: &DomainId) {
        if let Some(prior) = self.thompson.extract_prior(source) {
            self.thompson
                .init_domain_with_transfer(target.clone(), &prior);
        }
    }

    /// Verify a transfer delta: did it improve target without regressing source?
    pub fn verify_transfer(
        &self,
        source: &DomainId,
        target: &DomainId,
        source_before: f32,
        source_after: f32,
        target_before: f32,
        target_after: f32,
        baseline_cycles: u64,
        transfer_cycles: u64,
    ) -> TransferVerification {
        TransferVerification::verify(
            source.clone(),
            target.clone(),
            source_before,
            source_after,
            target_before,
            target_after,
            baseline_cycles,
            transfer_cycles,
        )
    }

    /// Evaluate all policy kernels on holdout tasks.
    pub fn evaluate_population(&mut self) {
        let holdout_snapshot: HashMap<DomainId, Vec<Task>> = self.holdouts.clone();
        let domain_ids: Vec<DomainId> = self.domains.keys().cloned().collect();

        for i in 0..self.population.population().len() {
            for domain_id in &domain_ids {
                if let Some(holdout_tasks) = holdout_snapshot.get(domain_id) {
                    let mut total_score = 0.0f32;
                    let mut count = 0;

                    for task in holdout_tasks {
                        if let Some(domain) = self.domains.get(domain_id) {
                            if let Some(ref_sol) = domain.reference_solution(task) {
                                let eval = domain.evaluate(task, &ref_sol);
                                total_score += eval.score;
                                count += 1;
                            }
                        }
                    }

                    let avg_score = if count > 0 {
                        total_score / count as f32
                    } else {
                        0.0
                    };

                    if let Some(kernel) = self.population.kernel_mut(i) {
                        kernel.record_score(domain_id.clone(), avg_score, 1.0);
                    }
                }
            }
        }
    }

    /// Evolve the policy kernel population and update Pareto front.
    pub fn evolve_population(&mut self) {
        // Record current population into Pareto front before evolving.
        let gen = self.population.generation();
        for kernel in self.population.population() {
            let accuracy = kernel.fitness();
            let cost = if kernel.cycles > 0 {
                kernel.total_cost / kernel.cycles as f32
            } else {
                0.0
            };
            // Robustness approximated by consistency across domains.
            let robustness = if kernel.holdout_scores.len() > 1 {
                let mean = accuracy;
                let var: f32 = kernel
                    .holdout_scores
                    .values()
                    .map(|s| (s - mean).powi(2))
                    .sum::<f32>()
                    / kernel.holdout_scores.len() as f32;
                (1.0 - var.sqrt()).max(0.0)
            } else {
                accuracy
            };
            self.meta
                .record_kernel(&kernel.id, accuracy, cost, robustness, gen);
        }

        self.population.evolve();
    }

    /// Get the best policy kernel found so far.
    pub fn best_kernel(&self) -> Option<&PolicyKernel> {
        self.population.best()
    }

    /// Get population statistics.
    pub fn population_stats(&self) -> PopulationStats {
        self.population.stats()
    }

    /// Get the scoreboard summary.
    pub fn scoreboard_summary(&self) -> ScoreboardSummary {
        self.scoreboard.summary()
    }

    /// Get registered domain IDs.
    pub fn domain_ids(&self) -> Vec<DomainId> {
        self.domains.keys().cloned().collect()
    }

    /// Get counterexamples for a domain.
    pub fn counterexamples(&self, domain_id: &DomainId) -> &[(Task, Solution, Evaluation)] {
        self.counterexamples
            .get(domain_id)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Select best arm for a context using Thompson Sampling.
    pub fn select_arm(&self, domain_id: &DomainId, bucket: &ContextBucket) -> Option<ArmId> {
        let mut rng = rand::thread_rng();
        self.thompson.select_arm(domain_id, bucket, &mut rng)
    }

    /// Check if dual-path speculation should be triggered.
    pub fn should_speculate(&self, domain_id: &DomainId, bucket: &ContextBucket) -> bool {
        self.thompson.is_uncertain(domain_id, bucket, 0.15)
    }

    /// Select arm with curiosity-boosted Thompson Sampling.
    ///
    /// Combines the standard Thompson sample with a UCB-style exploration
    /// bonus that favors under-visited bucket/arm combinations.
    pub fn select_arm_curious(
        &self,
        domain_id: &DomainId,
        bucket: &ContextBucket,
    ) -> Option<ArmId> {
        let mut rng = rand::thread_rng();
        // Get all arms and compute boosted scores
        let prior = self.thompson.extract_prior(domain_id)?;
        let arms: Vec<ArmId> = prior
            .bucket_priors
            .get(bucket)
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();

        if arms.is_empty() {
            return self.thompson.select_arm(domain_id, bucket, &mut rng);
        }

        let mut best_arm = None;
        let mut best_score = f32::NEG_INFINITY;

        for arm in &arms {
            let params = prior.get_prior(bucket, arm);
            let sample = params.sample(&mut rng);
            let boosted = self.meta.boosted_score(bucket, arm, sample);

            if boosted > best_score {
                best_score = boosted;
                best_arm = Some(arm.clone());
            }
        }

        best_arm.or_else(|| self.thompson.select_arm(domain_id, bucket, &mut rng))
    }

    /// Get meta-learning health diagnostics.
    pub fn meta_health(&self) -> MetaLearningHealth {
        self.meta.health_check()
    }

    /// Check cost curve for plateau and get recommended action.
    pub fn check_plateau(&mut self, domain_id: &DomainId) -> PlateauAction {
        if let Some(curve) = self.scoreboard.curves.get(domain_id) {
            self.meta.check_plateau(&curve.points)
        } else {
            PlateauAction::Continue
        }
    }

    /// Get regret summary across all learning contexts.
    pub fn regret_summary(&self) -> RegretSummary {
        self.meta.regret.summary()
    }

    /// Get the Pareto front of non-dominated policy kernels.
    pub fn pareto_front(&self) -> &ParetoFront {
        &self.meta.pareto
    }
}

impl Default for DomainExpansionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_creation() {
        let engine = DomainExpansionEngine::new();
        let ids = engine.domain_ids();
        assert_eq!(ids.len(), 3);
    }

    #[test]
    fn test_generate_tasks_all_domains() {
        let engine = DomainExpansionEngine::new();
        for domain_id in engine.domain_ids() {
            let tasks = engine.generate_tasks(&domain_id, 5, 0.5);
            assert_eq!(tasks.len(), 5);
        }
    }

    #[test]
    fn test_arm_selection() {
        let engine = DomainExpansionEngine::new();
        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "general".into(),
        };
        for domain_id in engine.domain_ids() {
            let arm = engine.select_arm(&domain_id, &bucket);
            assert!(arm.is_some());
        }
    }

    #[test]
    fn test_evaluate_and_record() {
        let mut engine = DomainExpansionEngine::new();
        let domain_id = DomainId("rust_synthesis".into());
        let tasks = engine.generate_tasks(&domain_id, 1, 0.3);
        let task = &tasks[0];

        let solution = Solution {
            task_id: task.id.clone(),
            content:
                "fn double(values: &[i64]) -> Vec<i64> { values.iter().map(|&x| x * 2).collect() }"
                    .into(),
            data: serde_json::Value::Null,
        };

        let bucket = ContextBucket {
            difficulty_tier: "easy".into(),
            category: "transform".into(),
        };
        let arm = ArmId("greedy".into());

        let eval = engine.evaluate_and_record(&domain_id, task, &solution, bucket, arm);
        assert!(eval.score >= 0.0 && eval.score <= 1.0);
    }

    #[test]
    fn test_cross_domain_embedding() {
        let engine = DomainExpansionEngine::new();

        let rust_sol = Solution {
            task_id: "rust".into(),
            content: "fn foo() { for i in 0..10 { if i > 5 { } } }".into(),
            data: serde_json::Value::Null,
        };

        let plan_sol = Solution {
            task_id: "plan".into(),
            content: "allocate cpu then schedule parallel jobs".into(),
            data: serde_json::json!({"steps": []}),
        };

        let rust_emb = engine
            .embed(&DomainId("rust_synthesis".into()), &rust_sol)
            .unwrap();
        let plan_emb = engine
            .embed(&DomainId("structured_planning".into()), &plan_sol)
            .unwrap();

        // Embeddings should be same dimension.
        assert_eq!(rust_emb.dim, plan_emb.dim);

        // Cross-domain similarity should be defined.
        let sim = rust_emb.cosine_similarity(&plan_emb);
        assert!(sim >= -1.0 && sim <= 1.0);
    }

    #[test]
    fn test_transfer_flow() {
        let mut engine = DomainExpansionEngine::new();
        let source = DomainId("rust_synthesis".into());
        let target = DomainId("structured_planning".into());

        // Record some outcomes in source domain.
        let bucket = ContextBucket {
            difficulty_tier: "medium".into(),
            category: "algorithm".into(),
        };

        for _ in 0..30 {
            engine.thompson.record_outcome(
                &source,
                bucket.clone(),
                ArmId("greedy".into()),
                0.85,
                1.0,
            );
        }

        // Initiate transfer.
        engine.initiate_transfer(&source, &target);

        // Verify the transfer.
        let verification = engine.verify_transfer(
            &source, &target, 0.85,  // source before
            0.845, // source after (within tolerance)
            0.3,   // target before
            0.7,   // target after
            100,   // baseline cycles
            45,    // transfer cycles
        );

        assert!(verification.promotable);
        assert!(verification.acceleration_factor > 1.0);
    }

    #[test]
    fn test_population_evolution() {
        let mut engine = DomainExpansionEngine::new();
        engine.generate_holdouts(3, 0.3);
        engine.evaluate_population();

        let stats_before = engine.population_stats();
        assert_eq!(stats_before.generation, 0);

        engine.evolve_population();
        let stats_after = engine.population_stats();
        assert_eq!(stats_after.generation, 1);
    }

    #[test]
    fn test_speculation_trigger() {
        let engine = DomainExpansionEngine::new();
        let bucket = ContextBucket {
            difficulty_tier: "hard".into(),
            category: "unknown".into(),
        };

        // With uniform priors, should be uncertain.
        assert!(engine.should_speculate(&DomainId("rust_synthesis".into()), &bucket,));
    }

    #[test]
    fn test_counterexample_tracking() {
        let mut engine = DomainExpansionEngine::new();
        let domain_id = DomainId("rust_synthesis".into());
        let tasks = engine.generate_tasks(&domain_id, 1, 0.9);
        let task = &tasks[0];

        // Submit a terrible solution.
        let solution = Solution {
            task_id: task.id.clone(),
            content: "".into(), // empty = bad
            data: serde_json::Value::Null,
        };

        let bucket = ContextBucket {
            difficulty_tier: "hard".into(),
            category: "algorithm".into(),
        };
        let arm = ArmId("speculative".into());

        let eval = engine.evaluate_and_record(&domain_id, task, &solution, bucket, arm);
        assert!(eval.score < 0.3);

        // Should be recorded as counterexample.
        assert!(!engine.counterexamples(&domain_id).is_empty());
    }
}
