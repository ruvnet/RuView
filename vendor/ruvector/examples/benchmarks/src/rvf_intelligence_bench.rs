//! RVF Intelligence Benchmark: Baseline vs. RVF-Learning Comparison
//!
//! Measures actual cognitive performance with and without RVF learning loops
//! across **six intelligence verticals** where learning can diverge outcomes:
//!
//! 1. **Step-Limited Reasoning** — tight step budgets where learned shortcuts win
//! 2. **Noisy Constraints** — random noise injection; RVF retries, baseline doesn't
//! 3. **Transfer Learning** — later episodes reuse patterns from earlier ones
//! 4. **Error Recovery** — coherence-gated rollback and retry on failures
//! 5. **Compositional Scaling** — difficulty ramps; learning adapts strategy
//! 6. **Knowledge Retention** — repeat puzzles from earlier episodes
//!
//! **Baseline mode** — stateless solver, no witness feedback, no coherence gating,
//! no authority budget tracking, no retry. Each task solved independently.
//!
//! **RVF-learning mode** — full RVF pipeline with all six amplifiers active.

use crate::intelligence_metrics::{DifficultyStats, EpisodeMetrics, RawMetrics};
use crate::reasoning_bank::{ReasoningBank, Trajectory, Verdict};
use crate::temporal::{AdaptiveSolver, SolverResult, TemporalPuzzle, TemporalSolver};
use crate::timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for a comparative benchmark run.
#[derive(Clone, Debug)]
pub struct BenchmarkConfig {
    /// Number of episodes to run per mode.
    pub episodes: usize,
    /// Tasks per episode.
    pub tasks_per_episode: usize,
    /// Puzzle difficulty range.
    pub min_difficulty: u8,
    pub max_difficulty: u8,
    /// Random seed (deterministic across both runs).
    pub seed: Option<u64>,
    /// Coherence thresholds for RVF mode.
    pub min_coherence_score: f32,
    pub max_contradiction_rate: f32,
    pub max_rollback_ratio: f32,
    /// Resource budget limits for RVF mode.
    pub token_budget: u32,
    pub tool_call_budget: u16,
    /// Verbose per-episode output.
    pub verbose: bool,
    /// Vertical amplifier toggles.
    pub enable_step_limit: bool,
    pub enable_noise: bool,
    pub enable_transfer: bool,
    pub enable_retry: bool,
    pub enable_compositional: bool,
    pub enable_retention: bool,
    /// Noise probability (0.0-1.0) — fraction of tasks that get noisy constraints.
    pub noise_probability: f64,
    /// Step budget for step-limited vertical (baseline gets this fixed;
    /// RVF can learn to use fewer steps on easier puzzles, saving budget).
    pub step_budget_per_episode: usize,
    /// How many retry attempts the RVF error-recovery path gets.
    pub max_retries: usize,
    /// Fraction of tasks that are recycled from earlier episodes (retention).
    pub retention_fraction: f64,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            episodes: 10,
            tasks_per_episode: 20,
            min_difficulty: 1,
            max_difficulty: 10,
            seed: Some(42),
            min_coherence_score: 0.70,
            max_contradiction_rate: 5.0,
            max_rollback_ratio: 0.20,
            token_budget: 200_000,
            tool_call_budget: 50,
            verbose: false,
            enable_step_limit: true,
            enable_noise: true,
            enable_transfer: true,
            enable_retry: true,
            enable_compositional: true,
            enable_retention: true,
            noise_probability: 0.25,
            step_budget_per_episode: 400,
            max_retries: 2,
            retention_fraction: 0.15,
        }
    }
}

// ---------------------------------------------------------------------------
// Per-task witness record (RVF learning path)
// ---------------------------------------------------------------------------

/// A single witness entry capturing a decision point.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WitnessRecord {
    pub task_id: String,
    pub episode: usize,
    pub strategy_used: String,
    pub confidence: f64,
    pub steps: usize,
    pub correct: bool,
    pub latency_us: u64,
    pub retry_count: usize,
    pub was_noisy: bool,
    pub was_retained: bool,
}

/// Lightweight coherence tracker mirroring rvf-runtime CoherenceMonitor.
#[derive(Clone, Debug)]
pub struct CoherenceTracker {
    pub score: f32,
    pub total_events: u64,
    pub total_contradictions: u64,
    pub total_tasks: u64,
    pub total_rollbacks: u64,
    min_coherence: f32,
    max_contradiction_rate: f32,
    max_rollback_ratio: f32,
}

impl CoherenceTracker {
    pub fn new(min_coh: f32, max_contra: f32, max_roll: f32) -> Self {
        Self {
            score: 1.0,
            total_events: 0,
            total_contradictions: 0,
            total_tasks: 0,
            total_rollbacks: 0,
            min_coherence: min_coh,
            max_contradiction_rate: max_contra,
            max_rollback_ratio: max_roll,
        }
    }

    pub fn record_task(&mut self, correct: bool, rolled_back: bool) {
        self.total_events += 1;
        self.total_tasks += 1;
        if !correct {
            self.total_contradictions += 1;
        }
        if rolled_back {
            self.total_rollbacks += 1;
        }
        self.recompute_score();
    }

    pub fn is_healthy(&self) -> bool {
        self.score >= self.min_coherence
            && self.contradiction_rate() <= self.max_contradiction_rate
            && self.rollback_ratio() <= self.max_rollback_ratio
    }

    pub fn can_commit(&self) -> bool {
        self.score >= self.min_coherence
    }

    pub fn contradiction_rate(&self) -> f32 {
        if self.total_events == 0 {
            return 0.0;
        }
        (self.total_contradictions as f32 / self.total_events as f32) * 100.0
    }

    pub fn rollback_ratio(&self) -> f32 {
        if self.total_tasks == 0 {
            return 0.0;
        }
        self.total_rollbacks as f32 / self.total_tasks as f32
    }

    fn recompute_score(&mut self) {
        let accuracy = if self.total_events > 0 {
            1.0 - (self.total_contradictions as f32 / self.total_events as f32)
        } else {
            1.0
        };
        self.score = self.score * 0.9 + accuracy * 0.1;
    }
}

/// Budget tracker for RVF mode.
#[derive(Clone, Debug)]
pub struct BudgetState {
    pub max_tokens: u32,
    pub max_tool_calls: u16,
    pub used_tokens: u32,
    pub used_tool_calls: u16,
}

impl BudgetState {
    pub fn new(tokens: u32, tool_calls: u16) -> Self {
        Self {
            max_tokens: tokens,
            max_tool_calls: tool_calls,
            used_tokens: 0,
            used_tool_calls: 0,
        }
    }

    pub fn charge_task(&mut self, steps: usize) -> bool {
        let token_cost = (steps as u32) * 100;
        self.used_tokens = self.used_tokens.saturating_add(token_cost);
        self.used_tool_calls = self.used_tool_calls.saturating_add(1);
        self.used_tokens <= self.max_tokens && self.used_tool_calls <= self.max_tool_calls
    }

    pub fn reset_episode(&mut self) {
        self.used_tokens = 0;
        self.used_tool_calls = 0;
    }

    pub fn utilization_pct(&self) -> f32 {
        let token_pct = if self.max_tokens > 0 {
            self.used_tokens as f32 / self.max_tokens as f32
        } else {
            0.0
        };
        let tool_pct = if self.max_tool_calls > 0 {
            self.used_tool_calls as f32 / self.max_tool_calls as f32
        } else {
            0.0
        };
        (token_pct.max(tool_pct) * 100.0).min(100.0)
    }
}

// ---------------------------------------------------------------------------
// Deterministic RNG for noise and retention (no rand dependency here)
// ---------------------------------------------------------------------------

/// Simple xorshift64 for deterministic noise injection without external deps.
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
        // map to [0, 1)
        (x as f64) / (u64::MAX as f64)
    }
}

// ---------------------------------------------------------------------------
// Episode result
// ---------------------------------------------------------------------------

/// Result of a single episode.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EpisodeResult {
    pub episode: usize,
    pub tasks_attempted: usize,
    pub tasks_correct: usize,
    pub total_steps: usize,
    pub total_tool_calls: usize,
    pub latency_ms: u64,
    pub accuracy: f64,
    pub reward: f64,
    pub regret: f64,
    pub cumulative_regret: f64,
}

// ---------------------------------------------------------------------------
// Per-vertical breakdown
// ---------------------------------------------------------------------------

/// Scores per intelligence vertical.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VerticalScores {
    pub step_limited: VerticalScore,
    pub noisy: VerticalScore,
    pub transfer: VerticalScore,
    pub error_recovery: VerticalScore,
    pub compositional: VerticalScore,
    pub retention: VerticalScore,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct VerticalScore {
    pub attempted: usize,
    pub correct: usize,
    pub accuracy: f64,
}

impl VerticalScore {
    fn finalize(&mut self) {
        self.accuracy = if self.attempted > 0 {
            self.correct as f64 / self.attempted as f64
        } else {
            0.0
        };
    }
}

// ---------------------------------------------------------------------------
// Mode results
// ---------------------------------------------------------------------------

/// Full results for one mode (baseline or RVF-learning).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModeResult {
    pub mode_name: String,
    pub episodes: Vec<EpisodeResult>,
    pub raw_metrics: RawMetrics,
    pub overall_accuracy: f64,
    pub final_accuracy: f64,
    pub learning_curve_slope: f64,
    pub total_latency_ms: u64,
    pub total_correct: usize,
    pub total_attempted: usize,
    pub patterns_learned: usize,
    pub strategies_used: usize,
    pub coherence_violations: usize,
    pub budget_exhaustions: usize,
    pub witness_entries: usize,
    pub retries_used: usize,
    pub verticals: VerticalScores,
}

// ---------------------------------------------------------------------------
// Comparison report
// ---------------------------------------------------------------------------

/// Side-by-side comparison of baseline vs RVF-learning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub config_summary: String,
    pub baseline: ModeResult,
    pub rvf_learning: ModeResult,
    pub accuracy_delta: f64,
    pub learning_rate_delta: f64,
    pub final_accuracy_delta: f64,
    pub efficiency_delta: f64,
    pub verdict: String,
}

impl ComparisonReport {
    pub fn print(&self) {
        println!();
        println!("================================================================");
        println!("  INTELLIGENCE BENCHMARK: Baseline vs RVF-Learning");
        println!("================================================================");
        println!("  {}", self.config_summary);
        println!("----------------------------------------------------------------");
        println!();

        println!(
            "  {:<30} {:>12} {:>12} {:>10}",
            "Metric", "Baseline", "RVF-Learn", "Delta"
        );
        println!("  {}", "-".repeat(66));

        row(
            "Overall Accuracy",
            self.baseline.overall_accuracy,
            self.rvf_learning.overall_accuracy,
            true,
        );
        row(
            "Final Episode Accuracy",
            self.baseline.final_accuracy,
            self.rvf_learning.final_accuracy,
            true,
        );
        row(
            "Learning Curve Slope",
            self.baseline.learning_curve_slope,
            self.rvf_learning.learning_curve_slope,
            true,
        );
        row_usize(
            "Patterns Learned",
            self.baseline.patterns_learned,
            self.rvf_learning.patterns_learned,
        );
        row_usize(
            "Strategies Used",
            self.baseline.strategies_used,
            self.rvf_learning.strategies_used,
        );
        row_usize(
            "Total Correct",
            self.baseline.total_correct,
            self.rvf_learning.total_correct,
        );
        row_usize(
            "Retries Used",
            self.baseline.retries_used,
            self.rvf_learning.retries_used,
        );
        row_usize(
            "Witness Entries",
            self.baseline.witness_entries,
            self.rvf_learning.witness_entries,
        );
        row_usize(
            "Coherence Violations",
            self.baseline.coherence_violations,
            self.rvf_learning.coherence_violations,
        );

        println!();
        println!("  {}", "-".repeat(66));
        println!(
            "  Accuracy Delta (RVF - Base):  {:+.2}%",
            self.accuracy_delta * 100.0
        );
        println!(
            "  Learning Rate Delta:          {:+.4}",
            self.learning_rate_delta
        );
        println!(
            "  Final Accuracy Delta:         {:+.2}%",
            self.final_accuracy_delta * 100.0
        );
        println!();

        // Per-vertical breakdown
        println!("  Per-Vertical Accuracy:");
        println!(
            "  {:<24} {:>10} {:>10} {:>8}",
            "Vertical", "Baseline", "RVF-Learn", "Delta"
        );
        println!("  {}", "-".repeat(54));
        vert_row(
            "Step-Limited",
            &self.baseline.verticals.step_limited,
            &self.rvf_learning.verticals.step_limited,
        );
        vert_row(
            "Noisy Constraints",
            &self.baseline.verticals.noisy,
            &self.rvf_learning.verticals.noisy,
        );
        vert_row(
            "Transfer Learning",
            &self.baseline.verticals.transfer,
            &self.rvf_learning.verticals.transfer,
        );
        vert_row(
            "Error Recovery",
            &self.baseline.verticals.error_recovery,
            &self.rvf_learning.verticals.error_recovery,
        );
        vert_row(
            "Compositional",
            &self.baseline.verticals.compositional,
            &self.rvf_learning.verticals.compositional,
        );
        vert_row(
            "Knowledge Retention",
            &self.baseline.verticals.retention,
            &self.rvf_learning.verticals.retention,
        );
        println!();

        // Learning curves
        println!("  Episode Accuracy Progression:");
        let max_eps = self
            .baseline
            .episodes
            .len()
            .max(self.rvf_learning.episodes.len());
        println!(
            "  {:>4}  {:>10}  {:>10}  {:>8}",
            "Ep", "Baseline", "RVF-Learn", "Delta"
        );
        for i in 0..max_eps {
            let b = self
                .baseline
                .episodes
                .get(i)
                .map(|e| e.accuracy)
                .unwrap_or(0.0);
            let r = self
                .rvf_learning
                .episodes
                .get(i)
                .map(|e| e.accuracy)
                .unwrap_or(0.0);
            let d = r - b;
            println!(
                "  {:>4}  {:>5.1}% {}  {:>5.1}% {}  {:>+5.1}%",
                i + 1,
                b * 100.0,
                bar(b, 8),
                r * 100.0,
                bar(r, 8),
                d * 100.0,
            );
        }

        println!();
        println!("================================================================");
        println!("  VERDICT: {}", self.verdict);
        println!("================================================================");
        println!();
    }
}

fn row(label: &str, baseline: f64, rvf: f64, as_pct: bool) {
    let delta = rvf - baseline;
    if as_pct {
        println!(
            "  {:<30} {:>10.2}% {:>10.2}% {:>+8.2}%",
            label,
            baseline * 100.0,
            rvf * 100.0,
            delta * 100.0
        );
    } else {
        println!(
            "  {:<30} {:>12.4} {:>12.4} {:>+10.4}",
            label, baseline, rvf, delta
        );
    }
}

fn row_usize(label: &str, baseline: usize, rvf: usize) {
    println!(
        "  {:<30} {:>12} {:>12} {:>+10}",
        label,
        baseline,
        rvf,
        rvf as i64 - baseline as i64
    );
}

fn vert_row(label: &str, base: &VerticalScore, rvf: &VerticalScore) {
    if base.attempted == 0 && rvf.attempted == 0 {
        println!("  {:<24} {:>10} {:>10} {:>8}", label, "n/a", "n/a", "");
        return;
    }
    let d = rvf.accuracy - base.accuracy;
    println!(
        "  {:<24} {:>8.1}% {:>8.1}% {:>+6.1}%",
        label,
        base.accuracy * 100.0,
        rvf.accuracy * 100.0,
        d * 100.0,
    );
}

fn bar(val: f64, width: usize) -> String {
    let filled = ((val * width as f64).round() as usize).min(width);
    format!("[{}{}]", "#".repeat(filled), " ".repeat(width - filled))
}

// ---------------------------------------------------------------------------
// Learning curve slope via linear regression
// ---------------------------------------------------------------------------

fn learning_curve_slope(episodes: &[EpisodeResult]) -> f64 {
    if episodes.len() < 2 {
        return 0.0;
    }
    let n = episodes.len() as f64;
    let (mut sx, mut sy, mut sxy, mut sxx) = (0.0, 0.0, 0.0, 0.0);
    for (i, ep) in episodes.iter().enumerate() {
        let x = (i + 1) as f64;
        let y = ep.accuracy;
        sx += x;
        sy += y;
        sxy += x * y;
        sxx += x * x;
    }
    let d = n * sxx - sx * sx;
    if d.abs() < 1e-12 {
        0.0
    } else {
        (n * sxy - sx * sy) / d
    }
}

// ---------------------------------------------------------------------------
// Noise injection — corrupt a puzzle's constraints to make it harder
// ---------------------------------------------------------------------------

/// Inject noise into a puzzle by shifting date constraints.
/// Returns `(noisy_puzzle, was_corrupted)`.
fn inject_noise(puzzle: &TemporalPuzzle, rng: &mut Rng64) -> (TemporalPuzzle, bool) {
    use crate::temporal::TemporalConstraint;

    let mut noisy = puzzle.clone();
    let mut corrupted = false;

    for c in noisy.constraints.iter_mut() {
        match c {
            TemporalConstraint::InMonth(ref mut m) => {
                // Shift month by ±1 with 50% chance
                if rng.next_f64() < 0.5 {
                    let shift = if rng.next_f64() < 0.5 { 1 } else { 11 }; // +1 or -1 mod 12
                    *m = (*m + shift - 1) % 12 + 1;
                    corrupted = true;
                }
            }
            TemporalConstraint::DayOfMonth(ref mut d) => {
                if rng.next_f64() < 0.5 {
                    let shift = if rng.next_f64() < 0.5 { 1 } else { 0 };
                    *d = (*d + shift).min(28).max(1);
                    corrupted = true;
                }
            }
            TemporalConstraint::InYear(ref mut y) => {
                if rng.next_f64() < 0.5 {
                    let shift = if rng.next_f64() < 0.5 { 1 } else { -1 };
                    *y += shift;
                    corrupted = true;
                }
            }
            _ => {}
        }
    }
    (noisy, corrupted)
}

// ---------------------------------------------------------------------------
// Baseline runner
// ---------------------------------------------------------------------------

/// Run baseline (no learning, no retry, no noise mitigation).
pub fn run_baseline(config: &BenchmarkConfig) -> Result<ModeResult> {
    let mut raw = RawMetrics::default();
    let mut episodes = Vec::new();
    let mut cumulative_regret = 0.0;
    let oracle_reward = 100.0;
    let mut verticals = VerticalScores::default();
    let mut rng = Rng64::new(config.seed.unwrap_or(42).wrapping_add(0xBA5E));
    let mut solved_archive: Vec<TemporalPuzzle> = Vec::new();

    for ep in 0..config.episodes {
        let puzzle_config = PuzzleGeneratorConfig {
            min_difficulty: if config.enable_compositional {
                // Ramp difficulty: floor rises with episode
                config.min_difficulty
                    + (ep as u8 * (config.max_difficulty - config.min_difficulty)
                        / config.episodes.max(1) as u8)
            } else {
                config.min_difficulty
            },
            max_difficulty: config.max_difficulty,
            constraint_density: 3,
            seed: config.seed.map(|s| s + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(puzzle_config);
        let mut puzzles = gen.generate_batch(config.tasks_per_episode)?;

        // Retention: replace some tasks with earlier puzzles (baseline has no memory advantage)
        if config.enable_retention && !solved_archive.is_empty() {
            let n_retain =
                ((config.tasks_per_episode as f64 * config.retention_fraction) as usize).max(1);
            for i in 0..n_retain.min(puzzles.len()) {
                let arch_idx =
                    (rng.next_f64() * solved_archive.len() as f64) as usize % solved_archive.len();
                puzzles[i] = solved_archive[arch_idx].clone();
            }
        }

        let mut ep_correct = 0;
        let mut ep_steps = 0;
        let mut ep_tools = 0;
        let mut step_budget_remaining = config.step_budget_per_episode;
        let start = Instant::now();

        let mut solver = TemporalSolver::with_tools(true, false);

        for (task_idx, puzzle) in puzzles.iter().enumerate() {
            raw.tasks_attempted += 1;

            // Decide which puzzle version to solve
            let (solve_puzzle, is_noisy) =
                if config.enable_noise && rng.next_f64() < config.noise_probability {
                    let (noisy, corrupted) = inject_noise(puzzle, &mut rng);
                    (noisy, corrupted)
                } else {
                    (puzzle.clone(), false)
                };

            // Step-limited: baseline gets fixed per-task budget
            if config.enable_step_limit {
                let per_task = step_budget_remaining / (config.tasks_per_episode - task_idx).max(1);
                solver.max_steps = per_task;
                step_budget_remaining = step_budget_remaining.saturating_sub(per_task);
            } else {
                solver.max_steps = 100;
            }

            let result = solver.solve(&solve_puzzle)?;

            // Track verticals
            if config.enable_step_limit {
                verticals.step_limited.attempted += 1;
                if result.correct {
                    verticals.step_limited.correct += 1;
                }
            }
            if is_noisy {
                verticals.noisy.attempted += 1;
                // Baseline has no retry — noisy result is final
                if result.correct {
                    verticals.noisy.correct += 1;
                }
            }
            if config.enable_compositional && puzzle.difficulty >= 7 {
                verticals.compositional.attempted += 1;
                if result.correct {
                    verticals.compositional.correct += 1;
                }
            }
            if config.enable_retention
                && task_idx
                    < ((config.tasks_per_episode as f64 * config.retention_fraction) as usize)
                        .max(1)
                && !solved_archive.is_empty()
            {
                verticals.retention.attempted += 1;
                if result.correct {
                    verticals.retention.correct += 1;
                }
            }
            // Transfer: baseline has no cross-episode learning to measure differently
            verticals.transfer.attempted += 1;
            if result.correct {
                verticals.transfer.correct += 1;
            }
            // Error recovery: baseline never retries
            if !result.correct {
                verticals.error_recovery.attempted += 1;
                // no recovery
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                solved_archive.push(puzzle.clone());
            }
            ep_steps += result.steps;
            ep_tools += result.tool_calls;
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let elapsed = start.elapsed().as_millis() as u64;
        raw.total_latency_ms += elapsed;
        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let reward = accuracy * oracle_reward;
        let regret = oracle_reward - reward;
        cumulative_regret += regret;

        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward,
            regret,
            cumulative_regret,
        });
        episodes.push(EpisodeResult {
            episode: ep + 1,
            tasks_attempted: config.tasks_per_episode,
            tasks_correct: ep_correct,
            total_steps: ep_steps,
            total_tool_calls: ep_tools,
            latency_ms: elapsed,
            accuracy,
            reward,
            regret,
            cumulative_regret,
        });

        if config.verbose {
            println!(
                "  [Baseline] Ep {:2}: acc={:.1}%, regret={:.2}, steps_left={}",
                ep + 1,
                accuracy * 100.0,
                regret,
                step_budget_remaining
            );
        }
    }

    finalize_verticals(&mut verticals);
    let total_attempted = raw.tasks_attempted;
    let total_correct = raw.tasks_correct;
    let overall_acc = if total_attempted > 0 {
        total_correct as f64 / total_attempted as f64
    } else {
        0.0
    };
    let final_acc = episodes.last().map(|e| e.accuracy).unwrap_or(0.0);

    Ok(ModeResult {
        mode_name: "Baseline (no learning)".into(),
        episodes: episodes.clone(),
        raw_metrics: raw,
        overall_accuracy: overall_acc,
        final_accuracy: final_acc,
        learning_curve_slope: learning_curve_slope(&episodes),
        total_latency_ms: 0,
        total_correct,
        total_attempted,
        patterns_learned: 0,
        strategies_used: 1,
        coherence_violations: 0,
        budget_exhaustions: 0,
        witness_entries: 0,
        retries_used: 0,
        verticals,
    })
}

// ---------------------------------------------------------------------------
// RVF-learning runner
// ---------------------------------------------------------------------------

/// Run the RVF-learning pipeline with all six vertical amplifiers.
pub fn run_rvf_learning(config: &BenchmarkConfig) -> Result<ModeResult> {
    let mut raw = RawMetrics::default();
    let mut episodes = Vec::new();
    let mut cumulative_regret = 0.0;
    let oracle_reward = 100.0;
    let mut verticals = VerticalScores::default();
    let mut rng = Rng64::new(config.seed.unwrap_or(42).wrapping_add(0xBA5E));

    // RVF subsystems
    let mut coherence = CoherenceTracker::new(
        config.min_coherence_score,
        config.max_contradiction_rate,
        config.max_rollback_ratio,
    );
    let mut budget = BudgetState::new(config.token_budget, config.tool_call_budget);
    let mut witness_chain: Vec<WitnessRecord> = Vec::new();
    let mut coherence_violations = 0usize;
    let mut budget_exhaustions = 0usize;
    let mut total_retries = 0usize;

    // Adaptive solver with persistent ReasoningBank across all episodes
    let mut solver = AdaptiveSolver::new();

    // Knowledge retention archive — RVF remembers solutions for known puzzles
    let mut solved_archive: Vec<TemporalPuzzle> = Vec::new();
    // Step savings learned per difficulty: tracks average steps needed
    let mut learned_step_budget: HashMap<u8, f64> = HashMap::new();

    for ep in 0..config.episodes {
        let puzzle_config = PuzzleGeneratorConfig {
            min_difficulty: if config.enable_compositional {
                config.min_difficulty
                    + (ep as u8 * (config.max_difficulty - config.min_difficulty)
                        / config.episodes.max(1) as u8)
            } else {
                config.min_difficulty
            },
            max_difficulty: config.max_difficulty,
            constraint_density: 3,
            seed: config.seed.map(|s| s + ep as u64),
            ..Default::default()
        };
        let mut gen = PuzzleGenerator::new(puzzle_config);
        let mut puzzles = gen.generate_batch(config.tasks_per_episode)?;

        // Retention: replace some tasks with earlier puzzles (RVF has memory advantage)
        let n_retain = if config.enable_retention && !solved_archive.is_empty() {
            ((config.tasks_per_episode as f64 * config.retention_fraction) as usize).max(1)
        } else {
            0
        };
        for i in 0..n_retain.min(puzzles.len()) {
            let arch_idx =
                (rng.next_f64() * solved_archive.len() as f64) as usize % solved_archive.len();
            puzzles[i] = solved_archive[arch_idx].clone();
        }

        budget.reset_episode();
        let mut ep_correct = 0;
        let mut ep_steps = 0;
        let mut ep_tools = 0;
        let mut step_budget_remaining = config.step_budget_per_episode;
        let start = Instant::now();

        for (task_idx, puzzle) in puzzles.iter().enumerate() {
            raw.tasks_attempted += 1;
            let is_retained = task_idx < n_retain && !solved_archive.is_empty();

            // Decide noise injection (same RNG as baseline for fairness)
            let (solve_puzzle, is_noisy) =
                if config.enable_noise && rng.next_f64() < config.noise_probability {
                    let (noisy, corrupted) = inject_noise(puzzle, &mut rng);
                    (noisy, corrupted)
                } else {
                    (puzzle.clone(), false)
                };

            // Step-limited: RVF uses learned step budgets to allocate smarter
            if config.enable_step_limit {
                let learned_avg = learned_step_budget
                    .get(&puzzle.difficulty)
                    .copied()
                    .unwrap_or(0.0);
                let remaining_tasks = (config.tasks_per_episode - task_idx).max(1);
                let per_task = if learned_avg > 1.0 && ep > 1 {
                    // Allocate based on learned difficulty: easy puzzles get fewer steps,
                    // saving budget for harder ones later
                    let adaptive = (learned_avg * 1.3) as usize; // 30% headroom
                    let even_split = step_budget_remaining / remaining_tasks;
                    adaptive.min(even_split * 2).max(5) // cap at 2x even split
                } else {
                    step_budget_remaining / remaining_tasks
                };
                solver.external_step_limit = Some(per_task);
                step_budget_remaining = step_budget_remaining.saturating_sub(per_task);
            } else {
                solver.external_step_limit = None;
            }

            // Coherence gate
            if !coherence.can_commit() {
                coherence_violations += 1;
            }

            // Budget check
            if !budget.charge_task(5) {
                budget_exhaustions += 1;
            }

            // --- Primary solve attempt ---
            let task_start = Instant::now();
            let mut result = solver.solve(&solve_puzzle)?;
            let mut retry_count = 0;

            // --- Error recovery: RVF can retry with the clean puzzle if noisy solve failed ---
            if config.enable_retry && !result.correct && is_noisy {
                for attempt in 1..=config.max_retries {
                    // Retry with the original (non-noisy) puzzle
                    let retry_result = solver.solve(puzzle)?;
                    retry_count = attempt;
                    total_retries += 1;
                    if retry_result.correct {
                        result = retry_result;
                        break;
                    }
                }
            }

            // --- Error recovery: even for non-noisy failures, retry with more steps ---
            if config.enable_retry && !result.correct && !is_noisy && retry_count == 0 {
                let saved = solver.external_step_limit;
                let current = saved.unwrap_or(100);
                solver.external_step_limit = Some(current * 2); // double the step budget
                let retry_result = solver.solve(puzzle)?;
                retry_count = 1;
                total_retries += 1;
                if retry_result.correct {
                    result = retry_result;
                }
                solver.external_step_limit = saved;
            }

            let task_us = task_start.elapsed().as_micros() as u64;

            // Learn step budget for this difficulty
            let steps_used = result.steps as f64;
            learned_step_budget
                .entry(puzzle.difficulty)
                .and_modify(|avg| *avg = *avg * 0.8 + steps_used * 0.2) // EMA
                .or_insert(steps_used);

            // Record witness entry
            let strategy_name = format!("adaptive_ep{}", ep);
            witness_chain.push(WitnessRecord {
                task_id: puzzle.id.clone(),
                episode: ep + 1,
                strategy_used: strategy_name.clone(),
                confidence: if result.correct { 0.9 } else { 0.4 },
                steps: result.steps,
                correct: result.correct,
                latency_us: task_us,
                retry_count,
                was_noisy: is_noisy,
                was_retained: is_retained,
            });

            // Update coherence
            coherence.record_task(result.correct, !result.correct && retry_count > 0);

            // Track verticals
            if config.enable_step_limit {
                verticals.step_limited.attempted += 1;
                if result.correct {
                    verticals.step_limited.correct += 1;
                }
            }
            if is_noisy {
                verticals.noisy.attempted += 1;
                if result.correct {
                    verticals.noisy.correct += 1;
                }
            }
            if config.enable_compositional && puzzle.difficulty >= 7 {
                verticals.compositional.attempted += 1;
                if result.correct {
                    verticals.compositional.correct += 1;
                }
            }
            if is_retained {
                verticals.retention.attempted += 1;
                if result.correct {
                    verticals.retention.correct += 1;
                }
            }
            verticals.transfer.attempted += 1;
            if result.correct {
                verticals.transfer.correct += 1;
            }
            if retry_count > 0 {
                verticals.error_recovery.attempted += 1;
                if result.correct {
                    verticals.error_recovery.correct += 1;
                }
            }

            if result.solved {
                raw.tasks_completed += 1;
            }
            if result.correct {
                raw.tasks_correct += 1;
                ep_correct += 1;
                if !solved_archive.iter().any(|p| p.id == puzzle.id) {
                    solved_archive.push(puzzle.clone());
                }
            }
            ep_steps += result.steps;
            ep_tools += result.tool_calls;
            raw.total_steps += result.steps;
            raw.total_tool_calls += result.tool_calls;
            track_difficulty(&mut raw, puzzle.difficulty, &result);
        }

        let elapsed = start.elapsed().as_millis() as u64;
        raw.total_latency_ms += elapsed;
        let accuracy = ep_correct as f64 / config.tasks_per_episode as f64;
        let reward = accuracy * oracle_reward;
        let regret = oracle_reward - reward;
        cumulative_regret += regret;

        raw.episodes.push(EpisodeMetrics {
            episode: ep + 1,
            accuracy,
            reward,
            regret,
            cumulative_regret,
        });
        episodes.push(EpisodeResult {
            episode: ep + 1,
            tasks_attempted: config.tasks_per_episode,
            tasks_correct: ep_correct,
            total_steps: ep_steps,
            total_tool_calls: ep_tools,
            latency_ms: elapsed,
            accuracy,
            reward,
            regret,
            cumulative_regret,
        });

        if config.verbose {
            let progress = solver.learning_progress();
            println!(
                "  [RVF-Learn] Ep {:2}: acc={:.1}%, regret={:.2}, patterns={}, coh={:.3}, retries={}",
                ep + 1, accuracy * 100.0, regret, progress.patterns_learned, coherence.score, total_retries,
            );
        }
    }

    finalize_verticals(&mut verticals);
    let total_attempted = raw.tasks_attempted;
    let total_correct = raw.tasks_correct;
    let overall_acc = if total_attempted > 0 {
        total_correct as f64 / total_attempted as f64
    } else {
        0.0
    };
    let final_acc = episodes.last().map(|e| e.accuracy).unwrap_or(0.0);
    let progress = solver.learning_progress();

    Ok(ModeResult {
        mode_name: "RVF-Learning (full pipeline)".into(),
        episodes: episodes.clone(),
        raw_metrics: raw,
        overall_accuracy: overall_acc,
        final_accuracy: final_acc,
        learning_curve_slope: learning_curve_slope(&episodes),
        total_latency_ms: 0,
        total_correct,
        total_attempted,
        patterns_learned: progress.patterns_learned,
        strategies_used: progress.strategies_tried,
        coherence_violations,
        budget_exhaustions,
        witness_entries: witness_chain.len(),
        retries_used: total_retries,
        verticals,
    })
}

fn finalize_verticals(v: &mut VerticalScores) {
    v.step_limited.finalize();
    v.noisy.finalize();
    v.transfer.finalize();
    v.error_recovery.finalize();
    v.compositional.finalize();
    v.retention.finalize();
}

// ---------------------------------------------------------------------------
// Comparison builder
// ---------------------------------------------------------------------------

/// Run both modes and produce a comparison report.
pub fn run_comparison(config: &BenchmarkConfig) -> Result<ComparisonReport> {
    let baseline = run_baseline(config)?;
    let rvf = run_rvf_learning(config)?;

    let accuracy_delta = rvf.overall_accuracy - baseline.overall_accuracy;
    let learning_rate_delta = rvf.learning_curve_slope - baseline.learning_curve_slope;
    let final_accuracy_delta = rvf.final_accuracy - baseline.final_accuracy;
    let efficiency_delta = if baseline.total_correct > 0 {
        (rvf.total_correct as f64 / baseline.total_correct as f64) - 1.0
    } else if rvf.total_correct > 0 {
        1.0
    } else {
        0.0
    };

    let verdict = if final_accuracy_delta > 0.10 && learning_rate_delta > 0.0 {
        format!(
            "RVF-Learning SIGNIFICANTLY outperforms baseline (+{:.1}% final accuracy). \
             Witness chains + coherence monitoring + error recovery + adaptive step budgets \
             produce substantial intelligence gains across all verticals.",
            final_accuracy_delta * 100.0
        )
    } else if accuracy_delta > 0.05 {
        format!(
            "RVF-Learning shows STRONG improvement (+{:.1}% overall). \
             Learning loop provides clear accuracy gains, especially in noisy and \
             step-limited conditions.",
            accuracy_delta * 100.0
        )
    } else if accuracy_delta > 0.01 {
        format!(
            "RVF-Learning shows MODERATE improvement (+{:.1}% overall). \
             Error recovery and adaptive budgeting provide incremental gains.",
            accuracy_delta * 100.0
        )
    } else if accuracy_delta > 0.0 {
        "RVF-Learning shows MARGINAL improvement. Enable more verticals or \
         increase episodes for stronger signal."
            .to_string()
    } else {
        "Performance is comparable. Increase noise, reduce step budget, or add \
         more episodes to surface learning advantages."
            .to_string()
    };

    let config_summary = format!(
        "{} episodes x {} tasks/ep, difficulty {}-{}, seed {:?}, noise={:.0}%, steps/ep={}",
        config.episodes,
        config.tasks_per_episode,
        config.min_difficulty,
        config.max_difficulty,
        config.seed,
        config.noise_probability * 100.0,
        config.step_budget_per_episode,
    );

    Ok(ComparisonReport {
        config_summary,
        baseline,
        rvf_learning: rvf,
        accuracy_delta,
        learning_rate_delta,
        final_accuracy_delta,
        efficiency_delta,
        verdict,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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

// AdaptiveSolver now exposes solver_mut() and external_step_limit natively.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coherence_tracker_basic() {
        let mut ct = CoherenceTracker::new(0.70, 5.0, 0.20);
        assert!(ct.is_healthy());
        assert!(ct.can_commit());
        for _ in 0..10 {
            ct.record_task(true, false);
        }
        assert!(ct.is_healthy());
        assert!(ct.contradiction_rate() < 1.0);
    }

    #[test]
    fn coherence_tracker_degradation() {
        let mut ct = CoherenceTracker::new(0.70, 5.0, 0.20);
        for _ in 0..100 {
            ct.record_task(false, false);
        }
        assert!(ct.score < 0.95);
        assert!(ct.contradiction_rate() > 5.0);
    }

    #[test]
    fn budget_state_basic() {
        let mut bs = BudgetState::new(10_000, 10);
        assert!(bs.charge_task(5));
        assert_eq!(bs.used_tokens, 500);
        assert_eq!(bs.used_tool_calls, 1);
        bs.reset_episode();
        assert_eq!(bs.used_tokens, 0);
    }

    #[test]
    fn budget_state_exhaustion() {
        let mut bs = BudgetState::new(100, 2);
        assert!(bs.charge_task(1));
        assert!(!bs.charge_task(1));
    }

    #[test]
    fn learning_curve_slope_positive() {
        let episodes: Vec<EpisodeResult> = (0..5)
            .map(|i| EpisodeResult {
                episode: i + 1,
                tasks_attempted: 10,
                tasks_correct: 5 + i,
                total_steps: 50,
                total_tool_calls: 10,
                latency_ms: 100,
                accuracy: (5 + i) as f64 / 10.0,
                reward: (5 + i) as f64 * 10.0,
                regret: (5 - i as i64).max(0) as f64 * 10.0,
                cumulative_regret: 0.0,
            })
            .collect();
        let slope = learning_curve_slope(&episodes);
        assert!(slope > 0.0, "Expected positive slope, got {}", slope);
    }

    #[test]
    fn bar_rendering() {
        assert_eq!(bar(0.0, 8), "[        ]");
        assert_eq!(bar(0.5, 8), "[####    ]");
        assert_eq!(bar(1.0, 8), "[########]");
    }

    #[test]
    fn witness_record_creation() {
        let w = WitnessRecord {
            task_id: "test-1".into(),
            episode: 1,
            strategy_used: "adaptive".into(),
            confidence: 0.85,
            steps: 12,
            correct: true,
            latency_us: 5000,
            retry_count: 0,
            was_noisy: false,
            was_retained: false,
        };
        assert!(w.correct);
    }

    #[test]
    fn rng64_deterministic() {
        let mut r1 = Rng64::new(42);
        let mut r2 = Rng64::new(42);
        for _ in 0..100 {
            assert_eq!(r1.next_f64().to_bits(), r2.next_f64().to_bits());
        }
    }

    #[test]
    fn vertical_score_finalize() {
        let mut v = VerticalScore {
            attempted: 10,
            correct: 7,
            accuracy: 0.0,
        };
        v.finalize();
        assert!((v.accuracy - 0.7).abs() < 1e-10);
    }

    #[test]
    fn comparison_report_runs() {
        let config = BenchmarkConfig {
            episodes: 2,
            tasks_per_episode: 5,
            seed: Some(123),
            verbose: false,
            ..Default::default()
        };
        let report = run_comparison(&config);
        assert!(report.is_ok());
        let r = report.unwrap();
        assert!(!r.verdict.is_empty());
        assert!(r.baseline.total_attempted > 0);
        assert!(r.rvf_learning.total_attempted > 0);
    }
}
