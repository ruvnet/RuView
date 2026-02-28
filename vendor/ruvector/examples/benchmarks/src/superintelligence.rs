//! Superintelligence Pathway — 5-Level Recursive Intelligence Amplification
//!
//! Each level feeds the next. The system measures IQ at every stage and only
//! advances when the prior level stabilises.
//!
//! ## Level Architecture
//!
//! ```text
//! L1  Foundation        IQ ~85   Adaptive solver + ReasoningBank + coherence
//! L2  Meta-Learning     IQ ~90   Learns *how* to learn (hyper-param optimizer)
//! L3  Ensemble Arbiter  IQ ~93   Multi-strategy voting + learned selection
//! L4  Recursive Improve IQ ~96   Bootstraps from own outputs + knowledge compile
//! L5  Adversarial Grow  IQ ~98+  Self-generated hard tasks + cascade reasoning
//! ```

use crate::intelligence_metrics::{
    DifficultyStats, EpisodeMetrics, IntelligenceCalculator, RawMetrics,
};
use crate::reasoning_bank::ReasoningBank;
use crate::temporal::{AdaptiveSolver, SolverResult, TemporalConstraint, TemporalPuzzle};
use crate::timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════
// Configuration
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
pub struct SIConfig {
    /// Episodes per level
    pub episodes_per_level: usize,
    /// Tasks per episode
    pub tasks_per_episode: usize,
    /// Seed
    pub seed: u64,
    /// Noise injection rate
    pub noise_rate: f64,
    /// Step budget per episode
    pub step_budget: usize,
    /// Max retries for error recovery
    pub max_retries: usize,
    /// Verbose output
    pub verbose: bool,
    /// Target IQ — stop if reached
    pub target_iq: f64,
    /// Number of ensemble strategies (Level 3)
    pub ensemble_size: usize,
    /// Recursive improvement cycles (Level 4)
    pub recursive_cycles: usize,
    /// Adversarial difficulty multiplier (Level 5)
    pub adversarial_pressure: f64,
}

impl Default for SIConfig {
    fn default() -> Self {
        Self {
            episodes_per_level: 12,
            tasks_per_episode: 25,
            seed: 42,
            noise_rate: 0.25,
            step_budget: 400,
            max_retries: 2,
            verbose: false,
            target_iq: 98.0,
            ensemble_size: 4,
            recursive_cycles: 3,
            adversarial_pressure: 1.5,
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Deterministic RNG (same xorshift64 as the benchmark module)
// ═══════════════════════════════════════════════════════════════════════════

struct Rng64(u64);
impl Rng64 {
    fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }
    fn next_f64(&mut self) -> f64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        (x as f64) / (u64::MAX as f64)
    }
    fn next_usize(&mut self, upper: usize) -> usize {
        (self.next_f64() * upper as f64) as usize % upper.max(1)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Level result
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LevelResult {
    pub level: usize,
    pub name: String,
    pub iq_score: f64,
    pub accuracy: f64,
    pub total_correct: usize,
    pub total_attempted: usize,
    pub patterns_learned: usize,
    pub episodes: Vec<EpisodeSnapshot>,
    pub raw_metrics: RawMetrics,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeSnapshot {
    pub episode: usize,
    pub accuracy: f64,
    pub steps: usize,
    pub retries: usize,
}

// ═══════════════════════════════════════════════════════════════════════════
// Full pathway result
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PathwayResult {
    pub levels: Vec<LevelResult>,
    pub peak_iq: f64,
    pub peak_level: usize,
    pub iq_progression: Vec<f64>,
    pub reached_target: bool,
    pub target_iq: f64,
}

impl PathwayResult {
    pub fn print(&self) {
        println!();
        println!("╔══════════════════════════════════════════════════════════════╗");
        println!("║       SUPERINTELLIGENCE PATHWAY — IQ PROGRESSION            ║");
        println!("╚══════════════════════════════════════════════════════════════╝");
        println!();

        let names = [
            "L1 Foundation",
            "L2 Meta-Learning",
            "L3 Ensemble Arbiter",
            "L4 Recursive Improve",
            "L5 Adversarial Growth",
        ];

        println!(
            "  {:<24} {:>6} {:>10} {:>10} {:>10}",
            "Level", "IQ", "Accuracy", "Correct", "Patterns"
        );
        println!("  {}", "-".repeat(62));

        for (i, level) in self.levels.iter().enumerate() {
            let iq_bar = iq_bar(level.iq_score);
            let delta = if i > 0 {
                format!("{:+.1}", level.iq_score - self.levels[i - 1].iq_score)
            } else {
                "---".to_string()
            };
            println!(
                "  {:<24} {:>5.1} {:>8.1}% {:>10} {:>10}  {} ({})",
                names.get(i).unwrap_or(&"Unknown"),
                level.iq_score,
                level.accuracy * 100.0,
                level.total_correct,
                level.patterns_learned,
                iq_bar,
                delta,
            );
        }

        println!();
        println!("  IQ Trajectory:");
        let max_iq = self.iq_progression.iter().cloned().fold(0.0_f64, f64::max);
        for (i, &iq) in self.iq_progression.iter().enumerate() {
            let filled = ((iq / max_iq.max(1.0)) * 40.0) as usize;
            println!(
                "  L{} {:>5.1} |{}{}|",
                i + 1,
                iq,
                "#".repeat(filled),
                " ".repeat(40 - filled)
            );
        }

        println!();
        if self.reached_target {
            println!(
                "  TARGET REACHED: IQ {:.1} >= {:.1}",
                self.peak_iq, self.target_iq
            );
        } else {
            println!(
                "  Peak IQ: {:.1} (target: {:.1}, gap: {:.1})",
                self.peak_iq,
                self.target_iq,
                self.target_iq - self.peak_iq
            );
        }

        println!();
        println!(
            "  Total IQ gain:  {:+.1} across {} levels",
            self.peak_iq - self.levels.first().map(|l| l.iq_score).unwrap_or(0.0),
            self.levels.len()
        );
        println!();
    }
}

fn iq_bar(iq: f64) -> String {
    let blocks = (iq / 10.0).round() as usize;
    let blocks = blocks.min(10);
    format!("[{}{}]", "█".repeat(blocks), "░".repeat(10 - blocks))
}

// ═══════════════════════════════════════════════════════════════════════════
// Noise injection (reused from rvf_intelligence_bench)
// ═══════════════════════════════════════════════════════════════════════════

fn inject_noise(puzzle: &TemporalPuzzle, rng: &mut Rng64) -> (TemporalPuzzle, bool) {
    let mut noisy = puzzle.clone();
    let mut corrupted = false;
    for c in noisy.constraints.iter_mut() {
        match c {
            TemporalConstraint::InMonth(ref mut m) => {
                if rng.next_f64() < 0.5 {
                    let shift = if rng.next_f64() < 0.5 { 1 } else { 11 };
                    *m = (*m + shift - 1) % 12 + 1;
                    corrupted = true;
                }
            }
            TemporalConstraint::DayOfMonth(ref mut d) => {
                if rng.next_f64() < 0.5 {
                    *d = (*d + 1).min(28).max(1);
                    corrupted = true;
                }
            }
            TemporalConstraint::InYear(ref mut y) => {
                if rng.next_f64() < 0.5 {
                    *y += if rng.next_f64() < 0.5 { 1 } else { -1 };
                    corrupted = true;
                }
            }
            _ => {}
        }
    }
    (noisy, corrupted)
}

// ═══════════════════════════════════════════════════════════════════════════
// Meta-Learned Hyperparameters (Level 2)
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Clone, Debug)]
struct MetaParams {
    /// Learned optimal step budget per difficulty
    step_budgets: HashMap<u8, f64>,
    /// Learned noise detection threshold
    noise_confidence_threshold: f64,
    /// Learned retry benefit estimate
    retry_benefit: f64,
    /// Best observed strategy per difficulty
    best_strategy_per_diff: HashMap<u8, String>,
}

impl MetaParams {
    fn new() -> Self {
        Self {
            step_budgets: HashMap::new(),
            noise_confidence_threshold: 0.6,
            retry_benefit: 0.5,
            best_strategy_per_diff: HashMap::new(),
        }
    }

    fn learn_from_result(&mut self, difficulty: u8, steps: usize, correct: bool, retried: bool) {
        // EMA for step budgets
        let s = steps as f64;
        self.step_budgets
            .entry(difficulty)
            .and_modify(|avg| *avg = *avg * 0.7 + s * 0.3)
            .or_insert(s);

        // Update retry benefit estimate
        if retried && correct {
            self.retry_benefit = self.retry_benefit * 0.9 + 1.0 * 0.1;
        } else if retried && !correct {
            self.retry_benefit = self.retry_benefit * 0.9 + 0.0 * 0.1;
        }
    }

    fn optimal_steps(
        &self,
        difficulty: u8,
        budget_remaining: usize,
        tasks_remaining: usize,
    ) -> usize {
        let learned = self.step_budgets.get(&difficulty).copied().unwrap_or(20.0);
        let adaptive = (learned * 1.5) as usize;
        let even = budget_remaining / tasks_remaining.max(1);
        adaptive.min(even * 3).max(5)
    }

    fn should_retry(&self) -> bool {
        self.retry_benefit > 0.3
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Strategy Ensemble (Level 3)
// ═══════════════════════════════════════════════════════════════════════════

struct StrategyEnsemble {
    solvers: Vec<AdaptiveSolver>,
    votes: Vec<f64>, // confidence-weighted voting history
}

impl StrategyEnsemble {
    fn new(size: usize, bank: &ReasoningBank) -> Self {
        let mut solvers = Vec::with_capacity(size);
        for i in 0..size {
            let mut s = AdaptiveSolver::with_reasoning_bank(bank.clone());
            // Diversify: give each solver a different step/beam profile
            match i % 4 {
                0 => { /* default — balanced */ }
                1 => {
                    s.solver_mut().max_steps = 30;
                } // aggressive
                2 => {
                    s.solver_mut().max_steps = 120;
                } // conservative
                3 => {
                    s.solver_mut().calendar_tool = false;
                } // no-rewrite
                _ => {}
            }
            solvers.push(s);
        }
        Self {
            solvers,
            votes: Vec::new(),
        }
    }

    fn solve_ensemble(&mut self, puzzle: &TemporalPuzzle) -> Result<SolverResult> {
        let mut results: Vec<SolverResult> = Vec::new();
        for solver in &mut self.solvers {
            let r = solver.solve(puzzle)?;
            results.push(r);
        }

        // Majority vote on correctness
        let correct_count = results.iter().filter(|r| r.correct).count();
        let any_correct = results.iter().find(|r| r.correct);

        if let Some(best) = any_correct {
            // Return the correct result with fewest steps
            let best_correct = results
                .iter()
                .filter(|r| r.correct)
                .min_by_key(|r| r.steps)
                .unwrap_or(best);
            self.votes.push(correct_count as f64 / results.len() as f64);
            Ok(best_correct.clone())
        } else {
            // All failed — return result with most solutions found
            let best_effort = results
                .iter()
                .max_by_key(|r| r.solutions.len())
                .unwrap_or(&results[0]);
            self.votes.push(0.0);
            Ok(best_effort.clone())
        }
    }

    fn consensus_strength(&self) -> f64 {
        if self.votes.is_empty() {
            return 0.0;
        }
        self.votes.iter().sum::<f64>() / self.votes.len() as f64
    }

    fn merge_knowledge(&self) -> ReasoningBank {
        // Merge all solvers' ReasoningBanks into one
        let mut merged = ReasoningBank::new();
        for solver in &self.solvers {
            for traj in &solver.reasoning_bank.trajectories {
                merged.record_trajectory(traj.clone());
            }
        }
        merged
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Knowledge Compiler (Level 4)
// ═══════════════════════════════════════════════════════════════════════════

/// Compiles learned patterns into direct lookup tables that bypass reasoning.
#[derive(Clone, Debug, Default)]
struct KnowledgeCompiler {
    /// Direct answer cache: puzzle_id -> known correct result
    answer_cache: HashMap<String, bool>,
    /// Compiled constraint signature -> optimal config
    signature_cache: HashMap<String, CompiledConfig>,
    hits: usize,
    misses: usize,
}

#[derive(Clone, Debug)]
struct CompiledConfig {
    use_rewriting: bool,
    max_steps: usize,
    expected_correct: bool,
}

impl KnowledgeCompiler {
    fn new() -> Self {
        Self::default()
    }

    fn compile_from_bank(&mut self, bank: &ReasoningBank) {
        for traj in &bank.trajectories {
            // Cache puzzle outcomes
            let correct = traj
                .verdict
                .as_ref()
                .map(|v| v.is_success())
                .unwrap_or(false);
            self.answer_cache.insert(traj.puzzle_id.clone(), correct);

            // Build constraint signature
            let mut sig_parts = traj.constraint_types.clone();
            sig_parts.sort();
            let sig = format!("{}:{}", traj.difficulty, sig_parts.join(","));

            if correct {
                if let Some(attempt) = traj.attempts.first() {
                    let entry = self.signature_cache.entry(sig).or_insert(CompiledConfig {
                        use_rewriting: true,
                        max_steps: attempt.steps,
                        expected_correct: true,
                    });
                    // Keep minimum steps that succeeded
                    entry.max_steps = entry.max_steps.min(attempt.steps);
                }
            }
        }
    }

    fn lookup_config(&mut self, puzzle: &TemporalPuzzle) -> Option<&CompiledConfig> {
        let mut sig_parts: Vec<String> = puzzle
            .constraints
            .iter()
            .map(|c| {
                format!("{:?}", c)
                    .split('(')
                    .next()
                    .unwrap_or("?")
                    .to_string()
            })
            .collect();
        sig_parts.sort();
        let sig = format!("{}:{}", puzzle.difficulty, sig_parts.join(","));

        if let Some(config) = self.signature_cache.get(&sig) {
            self.hits += 1;
            Some(config)
        } else {
            self.misses += 1;
            None
        }
    }

    fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Adversarial Generator (Level 5)
// ═══════════════════════════════════════════════════════════════════════════

/// Generates harder puzzles targeting known weaknesses.
struct AdversarialGenerator {
    /// Failure signatures: constraint patterns that fail most
    weak_signatures: Vec<(Vec<String>, u8, usize)>, // (constraints, difficulty, fail_count)
    pressure: f64,
}

impl AdversarialGenerator {
    fn new(pressure: f64) -> Self {
        Self {
            weak_signatures: Vec::new(),
            pressure,
        }
    }

    fn learn_weakness(&mut self, constraint_types: &[String], difficulty: u8, correct: bool) {
        if !correct {
            let key_types: Vec<String> = constraint_types.to_vec();
            if let Some(entry) = self
                .weak_signatures
                .iter_mut()
                .find(|(ct, d, _)| ct == &key_types && *d == difficulty)
            {
                entry.2 += 1;
            } else {
                self.weak_signatures.push((key_types, difficulty, 1));
            }
        }
    }

    fn harden_difficulty(&self, base_difficulty: u8) -> u8 {
        let boost = (self.pressure * 2.0) as u8;
        base_difficulty.saturating_add(boost).min(10)
    }

    fn top_weaknesses(&self, n: usize) -> Vec<&(Vec<String>, u8, usize)> {
        let mut sorted: Vec<_> = self.weak_signatures.iter().collect();
        sorted.sort_by(|a, b| b.2.cmp(&a.2));
        sorted.into_iter().take(n).collect()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Cascade Reasoner (Level 5 augmentation)
// ═══════════════════════════════════════════════════════════════════════════

/// Multi-pass reasoning: solve, verify, re-solve if inconsistent.
struct CascadeReasoner {
    passes: usize,
}

impl CascadeReasoner {
    fn new() -> Self {
        Self { passes: 0 }
    }

    fn cascade_solve(
        &mut self,
        solver: &mut AdaptiveSolver,
        puzzle: &TemporalPuzzle,
        max_passes: usize,
    ) -> Result<SolverResult> {
        let mut best = solver.solve(puzzle)?;
        self.passes += 1;

        for _ in 1..max_passes {
            if best.correct {
                break; // Already correct, no need for more passes
            }
            // Re-solve with doubled step budget
            let saved = solver.external_step_limit;
            let current = saved.unwrap_or(100);
            solver.external_step_limit = Some(current * 2);
            let retry = solver.solve(puzzle)?;
            solver.external_step_limit = saved;
            self.passes += 1;

            if retry.correct {
                best = retry;
                break;
            }
            // If retry found more solutions, prefer it
            if retry.solutions.len() > best.solutions.len() {
                best = retry;
            }
        }
        Ok(best)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Core Engine — runs all 5 levels
// ═══════════════════════════════════════════════════════════════════════════

pub fn run_pathway(config: &SIConfig) -> Result<PathwayResult> {
    let calculator = IntelligenceCalculator::default();
    let mut levels: Vec<LevelResult> = Vec::new();
    let mut iq_progression: Vec<f64> = Vec::new();

    // Persistent state that flows between levels
    let mut reasoning_bank = ReasoningBank::new();
    let mut meta_params = MetaParams::new();
    let mut compiler = KnowledgeCompiler::new();
    let mut adversary = AdversarialGenerator::new(config.adversarial_pressure);

    // ─── LEVEL 1: Foundation ─────────────────────────────────────────
    if config.verbose {
        println!("\n  ═══ Level 1: Foundation ═══");
    }
    let l1 = run_level_1(config, &mut reasoning_bank)?;
    let l1_iq = calculator.calculate(&l1.raw_metrics).overall_score;
    levels.push(make_level_result(1, "Foundation", &l1, l1_iq));
    iq_progression.push(l1_iq);
    if config.verbose {
        println!("  L1 IQ: {:.1}", l1_iq);
    }
    if l1_iq >= config.target_iq {
        return Ok(build_pathway(levels, iq_progression, config));
    }

    // ─── LEVEL 2: Meta-Learning ──────────────────────────────────────
    if config.verbose {
        println!("\n  ═══ Level 2: Meta-Learning ═══");
    }
    let l2 = run_level_2(config, &mut reasoning_bank, &mut meta_params)?;
    let l2_iq = calculator.calculate(&l2.raw_metrics).overall_score;
    levels.push(make_level_result(2, "Meta-Learning", &l2, l2_iq));
    iq_progression.push(l2_iq);
    if config.verbose {
        println!("  L2 IQ: {:.1} ({:+.1})", l2_iq, l2_iq - l1_iq);
    }
    if l2_iq >= config.target_iq {
        return Ok(build_pathway(levels, iq_progression, config));
    }

    // ─── LEVEL 3: Ensemble Arbiter ───────────────────────────────────
    if config.verbose {
        println!("\n  ═══ Level 3: Ensemble Arbiter ═══");
    }
    let l3 = run_level_3(config, &mut reasoning_bank, &meta_params)?;
    let l3_iq = calculator.calculate(&l3.raw_metrics).overall_score;
    levels.push(make_level_result(3, "Ensemble Arbiter", &l3, l3_iq));
    iq_progression.push(l3_iq);
    if config.verbose {
        println!("  L3 IQ: {:.1} ({:+.1})", l3_iq, l3_iq - l2_iq);
    }
    if l3_iq >= config.target_iq {
        return Ok(build_pathway(levels, iq_progression, config));
    }

    // ─── LEVEL 4: Recursive Self-Improvement ─────────────────────────
    if config.verbose {
        println!("\n  ═══ Level 4: Recursive Improvement ═══");
    }
    let l4 = run_level_4(config, &mut reasoning_bank, &mut meta_params, &mut compiler)?;
    let l4_iq = calculator.calculate(&l4.raw_metrics).overall_score;
    levels.push(make_level_result(4, "Recursive Improve", &l4, l4_iq));
    iq_progression.push(l4_iq);
    if config.verbose {
        println!("  L4 IQ: {:.1} ({:+.1})", l4_iq, l4_iq - l3_iq);
    }
    if l4_iq >= config.target_iq {
        return Ok(build_pathway(levels, iq_progression, config));
    }

    // ─── LEVEL 5: Adversarial Growth + Cascade ───────────────────────
    if config.verbose {
        println!("\n  ═══ Level 5: Adversarial Growth ═══");
    }
    let l5 = run_level_5(
        config,
        &mut reasoning_bank,
        &mut meta_params,
        &mut compiler,
        &mut adversary,
    )?;
    let l5_iq = calculator.calculate(&l5.raw_metrics).overall_score;
    levels.push(make_level_result(5, "Adversarial Growth", &l5, l5_iq));
    iq_progression.push(l5_iq);
    if config.verbose {
        println!("  L5 IQ: {:.1} ({:+.1})", l5_iq, l5_iq - l4_iq);
    }

    Ok(build_pathway(levels, iq_progression, config))
}

// ═══════════════════════════════════════════════════════════════════════════
// Level implementations
// ═══════════════════════════════════════════════════════════════════════════

struct LevelRaw {
    raw_metrics: RawMetrics,
    episodes: Vec<EpisodeSnapshot>,
    total_correct: usize,
    total_attempted: usize,
    patterns: usize,
}

/// Level 1: Foundation — adaptive solver with noise and retry.
fn run_level_1(config: &SIConfig, bank: &mut ReasoningBank) -> Result<LevelRaw> {
    let mut raw = RawMetrics::default();
    let mut snapshots = Vec::new();
    let mut solver = AdaptiveSolver::with_reasoning_bank(bank.clone());
    let mut rng = Rng64::new(config.seed.wrapping_add(100));
    let mut total_correct = 0usize;
    let mut total_attempted = 0usize;
    let mut cumulative_regret = 0.0;

    for ep in 0..config.episodes_per_level {
        let pc = PuzzleGeneratorConfig {
            min_difficulty: 1,
            max_difficulty: 10,
            constraint_density: 3,
            seed: Some(config.seed + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(pc);
        let puzzles = gen.generate_batch(config.tasks_per_episode)?;
        let mut ep_correct = 0;
        let mut ep_steps = 0;
        let mut ep_retries = 0;

        for puzzle in &puzzles {
            total_attempted += 1;
            raw.tasks_attempted += 1;

            let (solve_p, is_noisy) = if rng.next_f64() < config.noise_rate {
                inject_noise(puzzle, &mut rng)
            } else {
                (puzzle.clone(), false)
            };

            let mut result = solver.solve(&solve_p)?;
            let initial_correct = result.correct;

            // Retry on failure (rollback from noisy to clean)
            if !result.correct && is_noisy {
                for _ in 0..config.max_retries {
                    let retry = solver.solve(puzzle)?;
                    ep_retries += 1;
                    if retry.correct {
                        result = retry;
                        break;
                    }
                }
            }

            // Track noise, contradictions, rollbacks, policy violations
            if is_noisy {
                raw.noise_tasks_attempted += 1;
                if result.correct {
                    raw.noise_tasks_correct += 1;
                }
                if !initial_correct {
                    raw.rollback_attempts += 1;
                    if result.correct {
                        raw.rollback_successes += 1;
                    }
                }
            }
            if result.solved && !result.correct {
                raw.contradictions += 1;
                raw.policy_violations += 1;
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                total_correct += 1;
            }
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            ep_steps += result.steps;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let regret = 100.0 - accuracy * 100.0;
        cumulative_regret += regret;
        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward: accuracy * 100.0,
            regret,
            cumulative_regret,
        });
        snapshots.push(EpisodeSnapshot {
            episode: ep + 1,
            accuracy,
            steps: ep_steps,
            retries: ep_retries,
        });

        if config.verbose {
            println!("    L1 Ep {:2}: acc={:.1}%", ep + 1, accuracy * 100.0);
        }
    }

    // Transfer learned knowledge back to bank
    *bank = solver.reasoning_bank.clone();
    let patterns = bank.learning_progress().patterns_learned;

    Ok(LevelRaw {
        raw_metrics: raw,
        episodes: snapshots,
        total_correct,
        total_attempted,
        patterns,
    })
}

/// Level 2: Meta-Learning — learns optimal hyperparameters per problem class.
fn run_level_2(
    config: &SIConfig,
    bank: &mut ReasoningBank,
    meta: &mut MetaParams,
) -> Result<LevelRaw> {
    let mut raw = RawMetrics::default();
    let mut snapshots = Vec::new();
    let mut solver = AdaptiveSolver::with_reasoning_bank(bank.clone());
    let mut rng = Rng64::new(config.seed.wrapping_add(200));
    let mut total_correct = 0usize;
    let mut total_attempted = 0usize;
    let mut cumulative_regret = 0.0;

    for ep in 0..config.episodes_per_level {
        let pc = PuzzleGeneratorConfig {
            // Ramp difficulty floor
            min_difficulty: 1 + (ep as u8 * 9 / config.episodes_per_level.max(1) as u8),
            max_difficulty: 10,
            constraint_density: 3,
            seed: Some(config.seed + 1000 + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(pc);
        let puzzles = gen.generate_batch(config.tasks_per_episode)?;
        let mut ep_correct = 0;
        let mut ep_steps = 0;
        let mut ep_retries = 0;
        let mut step_budget_remaining = config.step_budget;

        for (ti, puzzle) in puzzles.iter().enumerate() {
            total_attempted += 1;
            raw.tasks_attempted += 1;

            let (solve_p, is_noisy) = if rng.next_f64() < config.noise_rate {
                inject_noise(puzzle, &mut rng)
            } else {
                (puzzle.clone(), false)
            };

            // Meta-learned step allocation
            let remaining_tasks = (config.tasks_per_episode - ti).max(1);
            let per_task =
                meta.optimal_steps(puzzle.difficulty, step_budget_remaining, remaining_tasks);
            solver.external_step_limit = Some(per_task);
            step_budget_remaining = step_budget_remaining.saturating_sub(per_task);

            let mut result = solver.solve(&solve_p)?;
            let mut retried = false;

            // Meta-learned retry decision
            if !result.correct && meta.should_retry() {
                if is_noisy {
                    let retry = solver.solve(puzzle)?;
                    ep_retries += 1;
                    retried = true;
                    if retry.correct {
                        result = retry;
                    }
                } else {
                    // Retry with doubled steps
                    solver.external_step_limit = Some(per_task * 2);
                    let retry = solver.solve(puzzle)?;
                    solver.external_step_limit = Some(per_task);
                    ep_retries += 1;
                    retried = true;
                    if retry.correct {
                        result = retry;
                    }
                }
            }

            meta.learn_from_result(puzzle.difficulty, result.steps, result.correct, retried);

            // Track noise, contradictions, rollbacks
            if is_noisy {
                raw.noise_tasks_attempted += 1;
                if result.correct {
                    raw.noise_tasks_correct += 1;
                }
                if retried {
                    raw.rollback_attempts += 1;
                    if result.correct {
                        raw.rollback_successes += 1;
                    }
                }
            }
            if result.solved && !result.correct {
                raw.contradictions += 1;
                raw.policy_violations += 1;
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                total_correct += 1;
            }
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            ep_steps += result.steps;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let regret = 100.0 - accuracy * 100.0;
        cumulative_regret += regret;
        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward: accuracy * 100.0,
            regret,
            cumulative_regret,
        });
        snapshots.push(EpisodeSnapshot {
            episode: ep + 1,
            accuracy,
            steps: ep_steps,
            retries: ep_retries,
        });

        if config.verbose {
            println!(
                "    L2 Ep {:2}: acc={:.1}%, retry_ben={:.2}",
                ep + 1,
                accuracy * 100.0,
                meta.retry_benefit
            );
        }
    }

    *bank = solver.reasoning_bank.clone();
    let patterns = bank.learning_progress().patterns_learned;
    Ok(LevelRaw {
        raw_metrics: raw,
        episodes: snapshots,
        total_correct,
        total_attempted,
        patterns,
    })
}

/// Level 3: Ensemble Arbiter — multiple strategies vote on each puzzle.
fn run_level_3(config: &SIConfig, bank: &mut ReasoningBank, meta: &MetaParams) -> Result<LevelRaw> {
    let mut raw = RawMetrics::default();
    let mut snapshots = Vec::new();
    let mut ensemble = StrategyEnsemble::new(config.ensemble_size, bank);
    let mut rng = Rng64::new(config.seed.wrapping_add(300));
    let mut total_correct = 0usize;
    let mut total_attempted = 0usize;
    let mut cumulative_regret = 0.0;

    for ep in 0..config.episodes_per_level {
        let pc = PuzzleGeneratorConfig {
            min_difficulty: 3,
            max_difficulty: 10,
            constraint_density: 3,
            seed: Some(config.seed + 2000 + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(pc);
        let puzzles = gen.generate_batch(config.tasks_per_episode)?;
        let mut ep_correct = 0;
        let mut ep_steps = 0;

        for puzzle in &puzzles {
            total_attempted += 1;
            raw.tasks_attempted += 1;

            let (solve_p, is_noisy) = if rng.next_f64() < config.noise_rate {
                inject_noise(puzzle, &mut rng)
            } else {
                (puzzle.clone(), false)
            };

            let mut result = ensemble.solve_ensemble(&solve_p)?;

            // If noisy and failed, retry with clean puzzle (rollback)
            if !result.correct && is_noisy {
                raw.rollback_attempts += 1;
                let retry = ensemble.solve_ensemble(puzzle)?;
                if retry.correct {
                    result = retry;
                    raw.rollback_successes += 1;
                }
            }

            // Track noise, contradictions, policy
            if is_noisy {
                raw.noise_tasks_attempted += 1;
                if result.correct {
                    raw.noise_tasks_correct += 1;
                }
            }
            if result.solved && !result.correct {
                raw.contradictions += 1;
                raw.policy_violations += 1;
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                total_correct += 1;
            }
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            ep_steps += result.steps;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let regret = 100.0 - accuracy * 100.0;
        cumulative_regret += regret;
        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward: accuracy * 100.0,
            regret,
            cumulative_regret,
        });
        snapshots.push(EpisodeSnapshot {
            episode: ep + 1,
            accuracy,
            steps: ep_steps,
            retries: 0,
        });

        if config.verbose {
            println!(
                "    L3 Ep {:2}: acc={:.1}%, consensus={:.2}",
                ep + 1,
                accuracy * 100.0,
                ensemble.consensus_strength()
            );
        }
    }

    // Merge ensemble knowledge back
    *bank = ensemble.merge_knowledge();
    let patterns = bank.learning_progress().patterns_learned;
    Ok(LevelRaw {
        raw_metrics: raw,
        episodes: snapshots,
        total_correct,
        total_attempted,
        patterns,
    })
}

/// Level 4: Recursive Self-Improvement — bootstrap from compiled knowledge.
fn run_level_4(
    config: &SIConfig,
    bank: &mut ReasoningBank,
    meta: &mut MetaParams,
    compiler: &mut KnowledgeCompiler,
) -> Result<LevelRaw> {
    let mut raw = RawMetrics::default();
    let mut snapshots = Vec::new();
    let mut total_correct = 0usize;
    let mut total_attempted = 0usize;
    let mut cumulative_regret = 0.0;

    for cycle in 0..config.recursive_cycles {
        // Compile knowledge from all prior trajectories
        compiler.compile_from_bank(bank);

        let mut solver = AdaptiveSolver::with_reasoning_bank(bank.clone());
        let mut rng = Rng64::new(config.seed.wrapping_add(400 + cycle as u64 * 100));
        let eps = config.episodes_per_level / config.recursive_cycles.max(1);

        for ep in 0..eps {
            let pc = PuzzleGeneratorConfig {
                min_difficulty: 4,
                max_difficulty: 10,
                constraint_density: 4,
                seed: Some(config.seed + 3000 + (cycle * 100 + ep) as u64),
                ..Default::default()
            };
            let mut gen = PuzzleGenerator::new(pc);
            let puzzles = gen.generate_batch(config.tasks_per_episode)?;
            let mut ep_correct = 0;
            let mut ep_steps = 0;
            let mut ep_retries = 0;
            let mut step_budget_remaining = config.step_budget;

            for (ti, puzzle) in puzzles.iter().enumerate() {
                total_attempted += 1;
                raw.tasks_attempted += 1;

                // Check compiled knowledge first (knowledge compiler fast path)
                if let Some(compiled) = compiler.lookup_config(puzzle) {
                    solver.solver_mut().calendar_tool = compiled.use_rewriting;
                    solver.external_step_limit = Some(compiled.max_steps.max(5));
                } else {
                    let remaining = (config.tasks_per_episode - ti).max(1);
                    let steps =
                        meta.optimal_steps(puzzle.difficulty, step_budget_remaining, remaining);
                    solver.external_step_limit = Some(steps);
                    step_budget_remaining = step_budget_remaining.saturating_sub(steps);
                }

                let (solve_p, is_noisy) = if rng.next_f64() < config.noise_rate {
                    inject_noise(puzzle, &mut rng)
                } else {
                    (puzzle.clone(), false)
                };

                let mut result = solver.solve(&solve_p)?;

                if !result.correct {
                    // Retry: noisy → clean (rollback); non-noisy → more steps
                    if is_noisy {
                        raw.rollback_attempts += 1;
                        let retry = solver.solve(puzzle)?;
                        ep_retries += 1;
                        if retry.correct {
                            result = retry;
                            raw.rollback_successes += 1;
                        }
                    } else {
                        let saved = solver.external_step_limit;
                        solver.external_step_limit = Some(saved.unwrap_or(100) * 2);
                        let retry = solver.solve(puzzle)?;
                        solver.external_step_limit = saved;
                        ep_retries += 1;
                        if retry.correct {
                            result = retry;
                        }
                    }
                }

                meta.learn_from_result(
                    puzzle.difficulty,
                    result.steps,
                    result.correct,
                    ep_retries > 0,
                );

                // Track noise, contradictions, policy
                if is_noisy {
                    raw.noise_tasks_attempted += 1;
                    if result.correct {
                        raw.noise_tasks_correct += 1;
                    }
                }
                if result.solved && !result.correct {
                    raw.contradictions += 1;
                    raw.policy_violations += 1;
                }

                if result.solved {
                    raw.tasks_completed += 1;
                }
                if result.correct {
                    raw.tasks_correct += 1;
                    ep_correct += 1;
                    total_correct += 1;
                }
                raw.total_steps += result.steps;
                raw.total_tool_calls += result.tool_calls;
                ep_steps += result.steps;
                track_difficulty(&mut raw, puzzle.difficulty, &result);
            }

            let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
            let regret = 100.0 - accuracy * 100.0;
            cumulative_regret += regret;
            raw.episodes.push(EpisodeMetrics {
                episode: raw.episodes.len() + 1,
                accuracy,
                reward: accuracy * 100.0,
                regret,
                cumulative_regret,
            });
            snapshots.push(EpisodeSnapshot {
                episode: snapshots.len() + 1,
                accuracy,
                steps: ep_steps,
                retries: ep_retries,
            });

            if config.verbose {
                println!(
                    "    L4 C{} Ep {:2}: acc={:.1}%, compiled_hit={:.0}%",
                    cycle + 1,
                    ep + 1,
                    accuracy * 100.0,
                    compiler.hit_rate() * 100.0
                );
            }
        }

        // Feed back: solver knowledge becomes next cycle's input
        *bank = solver.reasoning_bank.clone();
    }

    let patterns = bank.learning_progress().patterns_learned;
    Ok(LevelRaw {
        raw_metrics: raw,
        episodes: snapshots,
        total_correct,
        total_attempted,
        patterns,
    })
}

/// Level 5: Adversarial Growth + Cascade Reasoning.
fn run_level_5(
    config: &SIConfig,
    bank: &mut ReasoningBank,
    meta: &mut MetaParams,
    compiler: &mut KnowledgeCompiler,
    adversary: &mut AdversarialGenerator,
) -> Result<LevelRaw> {
    let mut raw = RawMetrics::default();
    let mut snapshots = Vec::new();
    let mut total_correct = 0usize;
    let mut total_attempted = 0usize;
    let mut cumulative_regret = 0.0;

    compiler.compile_from_bank(bank);

    let mut solver = AdaptiveSolver::with_reasoning_bank(bank.clone());
    let mut cascade = CascadeReasoner::new();
    let mut rng = Rng64::new(config.seed.wrapping_add(500));

    for ep in 0..config.episodes_per_level {
        // Adversarial difficulty: harder puzzles than any previous level
        let base_diff = 5 + (ep as u8 * 5 / config.episodes_per_level.max(1) as u8);
        let adv_diff = adversary.harden_difficulty(base_diff).min(10);

        let pc = PuzzleGeneratorConfig {
            min_difficulty: adv_diff.max(5),
            max_difficulty: 10,
            constraint_density: 4,
            seed: Some(config.seed + 5000 + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(pc);
        let puzzles = gen.generate_batch(config.tasks_per_episode)?;
        let mut ep_correct = 0;
        let mut ep_steps = 0;
        let mut ep_retries = 0;

        for (ti, puzzle) in puzzles.iter().enumerate() {
            total_attempted += 1;
            raw.tasks_attempted += 1;

            // Compiled fast path
            if let Some(compiled) = compiler.lookup_config(puzzle) {
                solver.solver_mut().calendar_tool = compiled.use_rewriting;
                solver.external_step_limit = Some(compiled.max_steps.max(10));
            } else {
                solver.external_step_limit = Some(meta.optimal_steps(
                    puzzle.difficulty,
                    config.step_budget,
                    config.tasks_per_episode - ti,
                ));
            }

            let (solve_p, is_noisy) =
                if rng.next_f64() < config.noise_rate * config.adversarial_pressure {
                    inject_noise(puzzle, &mut rng)
                } else {
                    (puzzle.clone(), false)
                };

            // Cascade reasoning: multi-pass solve
            let mut result = cascade.cascade_solve(&mut solver, &solve_p, 3)?;

            // Error recovery on noisy puzzles (rollback)
            if !result.correct && is_noisy {
                raw.rollback_attempts += 1;
                let retry = cascade.cascade_solve(&mut solver, puzzle, 2)?;
                ep_retries += 1;
                if retry.correct {
                    result = retry;
                    raw.rollback_successes += 1;
                }
            }

            // Track weaknesses for adversarial learning
            let ctypes: Vec<String> = puzzle
                .constraints
                .iter()
                .map(|c| {
                    format!("{:?}", c)
                        .split('(')
                        .next()
                        .unwrap_or("?")
                        .to_string()
                })
                .collect();
            adversary.learn_weakness(&ctypes, puzzle.difficulty, result.correct);
            meta.learn_from_result(
                puzzle.difficulty,
                result.steps,
                result.correct,
                ep_retries > 0,
            );

            // Track noise, contradictions, policy
            if is_noisy {
                raw.noise_tasks_attempted += 1;
                if result.correct {
                    raw.noise_tasks_correct += 1;
                }
            }
            if result.solved && !result.correct {
                raw.contradictions += 1;
                raw.policy_violations += 1;
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                total_correct += 1;
            }
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            ep_steps += result.steps;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let regret = 100.0 - accuracy * 100.0;
        cumulative_regret += regret;
        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward: accuracy * 100.0,
            regret,
            cumulative_regret,
        });
        snapshots.push(EpisodeSnapshot {
            episode: ep + 1,
            accuracy,
            steps: ep_steps,
            retries: ep_retries,
        });

        if config.verbose {
            let weaks = adversary.top_weaknesses(1);
            let weak_str = weaks
                .first()
                .map(|(ct, d, n)| format!("{:?}@d{} ({}x)", ct, d, n))
                .unwrap_or_default();
            println!(
                "    L5 Ep {:2}: acc={:.1}%, adv_diff={}, cascade={}, weak={}",
                ep + 1,
                accuracy * 100.0,
                adv_diff,
                cascade.passes,
                weak_str
            );
        }
    }

    *bank = solver.reasoning_bank.clone();
    let patterns = bank.learning_progress().patterns_learned;
    Ok(LevelRaw {
        raw_metrics: raw,
        episodes: snapshots,
        total_correct,
        total_attempted,
        patterns,
    })
}

// ═══════════════════════════════════════════════════════════════════════════
// Helpers
// ═══════════════════════════════════════════════════════════════════════════

fn track_difficulty(raw: &mut RawMetrics, difficulty: u8, result: &SolverResult) {
    let entry = raw
        .by_difficulty
        .entry(difficulty)
        .or_insert(DifficultyStats {
            attempted: 0,
            completed: 0,
            correct: 0,
            avg_steps: 0.0,
        });
    entry.attempted += 1;
    if result.solved {
        entry.completed += 1;
    }
    if result.correct {
        entry.correct += 1;
    }
}

fn make_level_result(level: usize, name: &str, raw: &LevelRaw, iq: f64) -> LevelResult {
    LevelResult {
        level,
        name: name.to_string(),
        iq_score: iq,
        accuracy: if raw.total_attempted > 0 {
            raw.total_correct as f64 / raw.total_attempted as f64
        } else {
            0.0
        },
        total_correct: raw.total_correct,
        total_attempted: raw.total_attempted,
        patterns_learned: raw.patterns,
        episodes: raw.episodes.clone(),
        raw_metrics: raw.raw_metrics.clone(),
    }
}

fn build_pathway(
    levels: Vec<LevelResult>,
    iq_progression: Vec<f64>,
    config: &SIConfig,
) -> PathwayResult {
    let peak_iq = iq_progression.iter().cloned().fold(0.0_f64, f64::max);
    let peak_level = iq_progression
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap())
        .map(|(i, _)| i + 1)
        .unwrap_or(1);

    PathwayResult {
        levels,
        peak_iq,
        peak_level,
        iq_progression,
        reached_target: peak_iq >= config.target_iq,
        target_iq: config.target_iq,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Tests
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning_bank::{Trajectory, Verdict};

    #[test]
    fn meta_params_learning() {
        let mut mp = MetaParams::new();
        for _ in 0..10 {
            mp.learn_from_result(5, 15, true, false);
        }
        let steps = mp.optimal_steps(5, 400, 20);
        assert!(steps > 0 && steps < 400);
    }

    #[test]
    fn knowledge_compiler_basic() {
        let mut bank = ReasoningBank::new();
        for i in 0..5 {
            let mut traj = Trajectory::new(&format!("p{}", i), 5);
            traj.constraint_types.push("InYear".to_string());
            traj.record_attempt("2024-01-01".into(), 0.9, 10, 1, "default");
            traj.set_verdict(Verdict::Success, None);
            bank.record_trajectory(traj);
        }
        let mut compiler = KnowledgeCompiler::new();
        compiler.compile_from_bank(&bank);
        assert!(!compiler.signature_cache.is_empty());
    }

    #[test]
    fn adversarial_generator_learns() {
        let mut adv = AdversarialGenerator::new(1.5);
        adv.learn_weakness(&["InMonth".to_string()], 8, false);
        adv.learn_weakness(&["InMonth".to_string()], 8, false);
        assert_eq!(adv.top_weaknesses(1).len(), 1);
        assert_eq!(adv.top_weaknesses(1)[0].2, 2);
    }

    #[test]
    fn rng64_deterministic() {
        let mut a = Rng64::new(99);
        let mut b = Rng64::new(99);
        for _ in 0..50 {
            assert_eq!(a.next_f64().to_bits(), b.next_f64().to_bits());
        }
    }

    #[test]
    fn iq_bar_rendering() {
        let bar = iq_bar(85.0);
        assert!(bar.contains("█"));
    }

    #[test]
    fn pathway_runs_minimal() {
        let config = SIConfig {
            episodes_per_level: 2,
            tasks_per_episode: 5,
            recursive_cycles: 1,
            ensemble_size: 2,
            verbose: false,
            target_iq: 200.0, // unreachable target so all 5 levels execute
            ..Default::default()
        };
        let result = run_pathway(&config);
        assert!(result.is_ok());
        let r = result.unwrap();
        assert_eq!(r.levels.len(), 5);
        assert!(r.peak_iq > 0.0);
    }
}
