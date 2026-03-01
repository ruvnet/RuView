//! Temporal Reasoning Benchmark Framework
//!
//! Implements temporal constraint solving and benchmarking based on:
//! - TimePuzzles benchmark methodology
//! - Tool-augmented iterative temporal reasoning
//! - Calendar math and cross-cultural date systems

use anyhow::{anyhow, Result};
use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Temporal constraint types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum TemporalConstraint {
    /// Date is exactly this value
    Exact(NaiveDate),
    /// Date is after this date
    After(NaiveDate),
    /// Date is before this date
    Before(NaiveDate),
    /// Date is between two dates (inclusive)
    Between(NaiveDate, NaiveDate),
    /// Date is on a specific day of week
    DayOfWeek(Weekday),
    /// Date is N days after reference
    DaysAfter(String, i64),
    /// Date is N days before reference
    DaysBefore(String, i64),
    /// Date is in a specific month
    InMonth(u32),
    /// Date is in a specific year
    InYear(i32),
    /// Date is a specific day of month
    DayOfMonth(u32),
    /// Relative to a named event (e.g., "Easter", "Chinese New Year")
    RelativeToEvent(String, i64),
}

/// A temporal puzzle with constraints
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalPuzzle {
    /// Unique puzzle ID
    pub id: String,
    /// Human-readable description
    pub description: String,
    /// Constraints that define the puzzle
    pub constraints: Vec<TemporalConstraint>,
    /// Named reference dates
    pub references: HashMap<String, NaiveDate>,
    /// Valid solution dates (for evaluation)
    pub solutions: Vec<NaiveDate>,
    /// Difficulty level (1-10)
    pub difficulty: u8,
    /// Tags for categorization
    pub tags: Vec<String>,
    /// Multi-dimensional difficulty vector (None = use scalar difficulty)
    pub difficulty_vector: Option<crate::timepuzzles::DifficultyVector>,
}

impl TemporalPuzzle {
    /// Create a new puzzle
    pub fn new(id: impl Into<String>, description: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            description: description.into(),
            constraints: Vec::new(),
            references: HashMap::new(),
            solutions: Vec::new(),
            difficulty: 5,
            tags: Vec::new(),
            difficulty_vector: None,
        }
    }

    /// Add a constraint
    pub fn with_constraint(mut self, constraint: TemporalConstraint) -> Self {
        self.constraints.push(constraint);
        self
    }

    /// Add a reference date
    pub fn with_reference(mut self, name: impl Into<String>, date: NaiveDate) -> Self {
        self.references.insert(name.into(), date);
        self
    }

    /// Set solution dates
    pub fn with_solutions(mut self, solutions: Vec<NaiveDate>) -> Self {
        self.solutions = solutions;
        self
    }

    /// Set difficulty
    pub fn with_difficulty(mut self, difficulty: u8) -> Self {
        self.difficulty = difficulty.min(10).max(1);
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }

    /// Check if a date satisfies all constraints
    pub fn check_date(&self, date: NaiveDate) -> Result<bool> {
        for constraint in &self.constraints {
            if !self.check_constraint(date, constraint)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Check a single constraint
    fn check_constraint(&self, date: NaiveDate, constraint: &TemporalConstraint) -> Result<bool> {
        match constraint {
            TemporalConstraint::Exact(d) => Ok(date == *d),
            TemporalConstraint::After(d) => Ok(date > *d),
            TemporalConstraint::Before(d) => Ok(date < *d),
            TemporalConstraint::Between(start, end) => Ok(date >= *start && date <= *end),
            TemporalConstraint::DayOfWeek(dow) => Ok(date.weekday() == *dow),
            TemporalConstraint::DaysAfter(ref_name, days) => {
                let ref_date = self
                    .references
                    .get(ref_name)
                    .ok_or_else(|| anyhow!("Unknown reference: {}", ref_name))?;
                let target = *ref_date + chrono::Duration::days(*days);
                Ok(date == target)
            }
            TemporalConstraint::DaysBefore(ref_name, days) => {
                let ref_date = self
                    .references
                    .get(ref_name)
                    .ok_or_else(|| anyhow!("Unknown reference: {}", ref_name))?;
                let target = *ref_date - chrono::Duration::days(*days);
                Ok(date == target)
            }
            TemporalConstraint::InMonth(month) => Ok(date.month() == *month),
            TemporalConstraint::InYear(year) => Ok(date.year() == *year),
            TemporalConstraint::DayOfMonth(day) => Ok(date.day() == *day),
            TemporalConstraint::RelativeToEvent(event_name, days) => {
                // Look up event in references
                let event_date = self
                    .references
                    .get(event_name)
                    .ok_or_else(|| anyhow!("Unknown event: {}", event_name))?;
                let target = *event_date + chrono::Duration::days(*days);
                Ok(date == target)
            }
        }
    }

    /// Solve the puzzle by searching date space
    pub fn solve(&self, search_range: (NaiveDate, NaiveDate)) -> Result<Vec<NaiveDate>> {
        let mut solutions = Vec::new();
        let mut current = search_range.0;
        while current <= search_range.1 {
            if self.check_date(current)? {
                solutions.push(current);
            }
            current = current.succ_opt().unwrap_or(current);
        }
        Ok(solutions)
    }
}

/// Puzzle solver with tool augmentation
#[derive(Clone, Debug)]
pub struct TemporalSolver {
    /// Enable calendar math tool
    pub calendar_tool: bool,
    /// Enable web search tool
    pub web_search_tool: bool,
    /// Maximum steps allowed
    pub max_steps: usize,
    /// Current step count
    pub steps: usize,
    /// Tool call count
    pub tool_calls: usize,
    /// Stop after finding the first valid solution (early termination)
    pub stop_after_first: bool,
    /// Skip to matching weekday (advance by 7 days instead of 1)
    pub skip_weekday: Option<Weekday>,
    /// Constraint propagation pre-pass mode (controlled by PolicyKernel)
    pub prepass_mode: PrepassMode,
}

impl Default for TemporalSolver {
    fn default() -> Self {
        Self {
            calendar_tool: true,
            web_search_tool: false,
            max_steps: 100,
            steps: 0,
            tool_calls: 0,
            stop_after_first: false,
            skip_weekday: None,
            prepass_mode: PrepassMode::Off,
        }
    }
}

impl TemporalSolver {
    /// Create solver with tools
    pub fn with_tools(calendar: bool, web_search: bool) -> Self {
        Self {
            calendar_tool: calendar,
            web_search_tool: web_search,
            stop_after_first: false,
            skip_weekday: None,
            prepass_mode: PrepassMode::Off,
            ..Default::default()
        }
    }

    /// Constraint propagation pre-pass: tighten the search range
    /// using InMonth, DayOfMonth, and DayOfWeek constraints.
    ///
    /// This is the key sublinear optimization. Instead of scanning
    /// every day in the range, we compute valid date sets directly:
    ///
    /// 1. InMonth(m) + InYear(y) → range shrinks to that month (≤31 days)
    /// 2. DayOfMonth(d) + bounded range → jump directly to matching days
    /// 3. DayOfWeek(w) already handled by skip_weekday, but propagation
    ///    can further restrict: e.g., Month(2) + DayOfWeek(Mon) in a year
    ///    has only 4-5 candidates.
    ///
    /// Returns (tightened_start, tightened_end, direct_candidates).
    /// If direct_candidates is non-empty, skip the scan entirely.
    fn propagate_constraints(
        &self,
        puzzle: &TemporalPuzzle,
        range_start: NaiveDate,
        range_end: NaiveDate,
    ) -> (NaiveDate, NaiveDate, Vec<NaiveDate>) {
        let mut start = range_start;
        let mut end = range_end;

        // Extract constraint features
        let mut target_month: Option<u32> = None;
        let mut target_dom: Option<u32> = None;
        let mut target_dow: Option<Weekday> = None;
        let mut target_year: Option<i32> = None;

        for c in &puzzle.constraints {
            match c {
                TemporalConstraint::InMonth(m) => {
                    target_month = Some(*m);
                }
                TemporalConstraint::DayOfMonth(d) => {
                    target_dom = Some(*d);
                }
                TemporalConstraint::DayOfWeek(w) => {
                    target_dow = Some(*w);
                }
                TemporalConstraint::InYear(y) => {
                    target_year = Some(*y);
                }
                _ => {}
            }
        }

        // Tighten by month + year
        if let (Some(m), Some(y)) = (target_month, target_year) {
            let month_start = NaiveDate::from_ymd_opt(y, m, 1);
            let month_end = if m == 12 {
                NaiveDate::from_ymd_opt(y, 12, 31)
            } else {
                NaiveDate::from_ymd_opt(y, m + 1, 1).and_then(|d| d.pred_opt())
            };
            if let (Some(ms), Some(me)) = (month_start, month_end) {
                if ms > start {
                    start = ms;
                }
                if me < end {
                    end = me;
                }
            }
        } else if let Some(m) = target_month {
            // Month without year: tighten to first occurrence in range
            let year = start.year();
            if let Some(ms) = NaiveDate::from_ymd_opt(year, m, 1) {
                if ms >= start && ms <= end {
                    start = ms;
                    // End of that month
                    let me = if m == 12 {
                        NaiveDate::from_ymd_opt(year, 12, 31)
                    } else {
                        NaiveDate::from_ymd_opt(year, m + 1, 1).and_then(|d| d.pred_opt())
                    };
                    if let Some(me) = me {
                        if me < end {
                            end = me;
                        }
                    }
                }
            }
        }

        // Direct solve: DayOfMonth within a tight range
        if let Some(dom) = target_dom {
            if (end - start).num_days() <= 366 {
                let mut candidates = Vec::new();
                let mut y = start.year();
                let mut m = start.month();
                loop {
                    if let Some(d) = NaiveDate::from_ymd_opt(y, m, dom) {
                        if d >= start && d <= end {
                            // Verify against ALL constraints before adding
                            if puzzle.check_date(d).unwrap_or(false) {
                                candidates.push(d);
                            }
                        }
                        if d > end {
                            break;
                        }
                    }
                    // Next month
                    m += 1;
                    if m > 12 {
                        m = 1;
                        y += 1;
                    }
                    if NaiveDate::from_ymd_opt(y, m, 1)
                        .map(|d| d > end)
                        .unwrap_or(true)
                    {
                        break;
                    }
                }
                if !candidates.is_empty() {
                    return (start, end, candidates);
                }
            }
        }

        // Direct solve: DayOfWeek within a tight range (≤60 days → ≤9 candidates)
        // Only in Full mode — Light mode does InMonth/DayOfMonth only
        if self.prepass_mode == PrepassMode::Full {
            if let Some(dow) = target_dow {
                let range_days = (end - start).num_days();
                if range_days <= 60 && range_days >= 0 {
                    let mut candidates = Vec::new();
                    let mut d = start;
                    while d.weekday() != dow && d <= end {
                        d = d.succ_opt().unwrap_or(d);
                    }
                    while d <= end {
                        if puzzle.check_date(d).unwrap_or(false) {
                            candidates.push(d);
                        }
                        d = d + chrono::Duration::days(7);
                    }
                    if !candidates.is_empty() {
                        return (start, end, candidates);
                    }
                }
            }
        }

        (start, end, Vec::new())
    }

    /// Solve a puzzle with step tracking.
    ///
    /// Three-phase solve:
    /// 1. Constraint propagation: tighten range, attempt direct solve
    /// 2. If direct candidates found: verify and return (sublinear)
    /// 3. Otherwise: scan with optional weekday skip (linear/7x)
    pub fn solve(&mut self, puzzle: &TemporalPuzzle) -> Result<SolverResult> {
        self.steps = 0;
        self.tool_calls = 0;

        let start_time = std::time::Instant::now();

        // Rewrite constraints to explicit dates if calendar tool enabled
        let effective_puzzle = if self.calendar_tool {
            self.tool_calls += 1;
            self.rewrite_constraints(puzzle)?
        } else {
            puzzle.clone()
        };

        // Determine search range from effective (rewritten) constraints
        let range = self.determine_search_range(&effective_puzzle)?;

        // ─── Phase 1: Constraint propagation (if enabled) ────────────────
        let (prop_start, prop_end, direct_candidates) = match self.prepass_mode {
            PrepassMode::Off => (range.0, range.1, Vec::new()),
            PrepassMode::Light | PrepassMode::Full => {
                self.propagate_constraints(&effective_puzzle, range.0, range.1)
            }
        };

        // ─── Phase 2: Direct solve (sublinear) ──────────────────────────
        if !direct_candidates.is_empty() {
            self.steps = direct_candidates.len();
            self.tool_calls += 1; // propagation counts as a tool call
            let latency = start_time.elapsed();

            let correct = if puzzle.solutions.is_empty() {
                true
            } else {
                puzzle
                    .solutions
                    .iter()
                    .all(|s| direct_candidates.contains(s) || *s < prop_start || *s > prop_end)
            };

            return Ok(SolverResult {
                puzzle_id: puzzle.id.clone(),
                solved: !direct_candidates.is_empty(),
                correct,
                solutions: direct_candidates,
                steps: self.steps,
                tool_calls: self.tool_calls,
                latency_ms: latency.as_millis() as u64,
            });
        }

        // ─── Phase 3: Scan (linear or weekday-skip) ─────────────────────
        let mut found_solutions = Vec::new();
        let mut current = prop_start; // Use propagated (tighter) range

        // Advance to first matching weekday if skipping enabled
        if let Some(target_dow) = self.skip_weekday {
            while current.weekday() != target_dow && current <= prop_end {
                current = current.succ_opt().unwrap_or(current);
            }
        }

        while current <= prop_end && self.steps < self.max_steps {
            self.steps += 1;
            if effective_puzzle.check_date(current)? {
                found_solutions.push(current);
                if self.stop_after_first {
                    break;
                }
            }
            if self.skip_weekday.is_some() {
                current = current + chrono::Duration::days(7);
            } else {
                current = match current.succ_opt() {
                    Some(d) => d,
                    None => break,
                };
            }
        }

        let latency = start_time.elapsed();

        // Check correctness
        let correct = if puzzle.solutions.is_empty() {
            true
        } else {
            puzzle
                .solutions
                .iter()
                .all(|s| found_solutions.contains(s) || *s < prop_start || *s > prop_end)
        };

        Ok(SolverResult {
            puzzle_id: puzzle.id.clone(),
            solved: !found_solutions.is_empty(),
            correct,
            solutions: found_solutions,
            steps: self.steps,
            tool_calls: self.tool_calls,
            latency_ms: latency.as_millis() as u64,
        })
    }

    /// Determine search range from constraints
    fn determine_search_range(&self, puzzle: &TemporalPuzzle) -> Result<(NaiveDate, NaiveDate)> {
        let mut min_date = NaiveDate::from_ymd_opt(1900, 1, 1).unwrap();
        let mut max_date = NaiveDate::from_ymd_opt(2100, 12, 31).unwrap();

        for constraint in &puzzle.constraints {
            match constraint {
                TemporalConstraint::Exact(d) => {
                    min_date = *d;
                    max_date = *d;
                }
                TemporalConstraint::After(d) => {
                    if *d >= min_date {
                        min_date = d.succ_opt().unwrap_or(*d);
                    }
                }
                TemporalConstraint::Before(d) => {
                    if *d <= max_date {
                        max_date = d.pred_opt().unwrap_or(*d);
                    }
                }
                TemporalConstraint::Between(start, end) => {
                    if *start > min_date {
                        min_date = *start;
                    }
                    if *end < max_date {
                        max_date = *end;
                    }
                }
                TemporalConstraint::InYear(year) => {
                    let year_start = NaiveDate::from_ymd_opt(*year, 1, 1).unwrap_or(min_date);
                    let year_end = NaiveDate::from_ymd_opt(*year, 12, 31).unwrap_or(max_date);
                    if year_start > min_date {
                        min_date = year_start;
                    }
                    if year_end < max_date {
                        max_date = year_end;
                    }
                }
                _ => {}
            }
        }

        Ok((min_date, max_date))
    }

    /// Rewrite relative constraints to explicit dates
    fn rewrite_constraints(&self, puzzle: &TemporalPuzzle) -> Result<TemporalPuzzle> {
        let mut new_puzzle = puzzle.clone();
        let mut new_constraints = Vec::new();

        for constraint in &puzzle.constraints {
            match constraint {
                TemporalConstraint::DaysAfter(ref_name, days) => {
                    if let Some(ref_date) = puzzle.references.get(ref_name) {
                        let target = *ref_date + chrono::Duration::days(*days);
                        new_constraints.push(TemporalConstraint::Exact(target));
                    } else {
                        new_constraints.push(constraint.clone());
                    }
                }
                TemporalConstraint::DaysBefore(ref_name, days) => {
                    if let Some(ref_date) = puzzle.references.get(ref_name) {
                        let target = *ref_date - chrono::Duration::days(*days);
                        new_constraints.push(TemporalConstraint::Exact(target));
                    } else {
                        new_constraints.push(constraint.clone());
                    }
                }
                TemporalConstraint::RelativeToEvent(event_name, days) => {
                    if let Some(event_date) = puzzle.references.get(event_name) {
                        let target = *event_date + chrono::Duration::days(*days);
                        new_constraints.push(TemporalConstraint::Exact(target));
                    } else {
                        new_constraints.push(constraint.clone());
                    }
                }
                _ => new_constraints.push(constraint.clone()),
            }
        }

        new_puzzle.constraints = new_constraints;
        Ok(new_puzzle)
    }
}

/// Result from solving a puzzle
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SolverResult {
    pub puzzle_id: String,
    pub solved: bool,
    pub correct: bool,
    pub solutions: Vec<NaiveDate>,
    pub steps: usize,
    pub tool_calls: usize,
    pub latency_ms: u64,
}

/// Benchmark configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkConfig {
    /// Number of puzzles to run
    pub num_puzzles: usize,
    /// Difficulty range
    pub difficulty_range: (u8, u8),
    /// Enable calendar tool
    pub calendar_tool: bool,
    /// Enable web search tool
    pub web_search_tool: bool,
    /// Maximum steps per puzzle
    pub max_steps: usize,
    /// Constraint density (1-5)
    pub constraint_density: u8,
}

impl Default for BenchmarkConfig {
    fn default() -> Self {
        Self {
            num_puzzles: 50,
            difficulty_range: (1, 10),
            calendar_tool: true,
            web_search_tool: false,
            max_steps: 100,
            constraint_density: 3,
        }
    }
}

/// Benchmark results
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BenchmarkResults {
    pub config: BenchmarkConfig,
    pub total_puzzles: usize,
    pub solved_count: usize,
    pub correct_count: usize,
    pub accuracy: f64,
    pub avg_steps: f64,
    pub avg_tool_calls: f64,
    pub avg_latency_ms: f64,
    pub results: Vec<SolverResult>,
}

impl BenchmarkResults {
    /// Create from individual results
    pub fn from_results(config: BenchmarkConfig, results: Vec<SolverResult>) -> Self {
        let total = results.len();
        let solved = results.iter().filter(|r| r.solved).count();
        let correct = results.iter().filter(|r| r.correct).count();
        let avg_steps = results.iter().map(|r| r.steps as f64).sum::<f64>() / total as f64;
        let avg_tools = results.iter().map(|r| r.tool_calls as f64).sum::<f64>() / total as f64;
        let avg_latency = results.iter().map(|r| r.latency_ms as f64).sum::<f64>() / total as f64;

        Self {
            config,
            total_puzzles: total,
            solved_count: solved,
            correct_count: correct,
            accuracy: correct as f64 / total as f64,
            avg_steps,
            avg_tool_calls: avg_tools,
            avg_latency_ms: avg_latency,
            results,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_puzzle() {
        let puzzle = TemporalPuzzle::new("test-1", "Find a date in January 2024")
            .with_constraint(TemporalConstraint::InYear(2024))
            .with_constraint(TemporalConstraint::InMonth(1))
            .with_constraint(TemporalConstraint::DayOfMonth(15));

        let expected = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert!(puzzle.check_date(expected).unwrap());
        assert!(!puzzle
            .check_date(NaiveDate::from_ymd_opt(2024, 2, 15).unwrap())
            .unwrap());
    }

    #[test]
    fn test_relative_constraint() {
        let base = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
        let puzzle = TemporalPuzzle::new("test-2", "Find a date 10 days after New Year")
            .with_reference("new_year", base)
            .with_constraint(TemporalConstraint::DaysAfter("new_year".to_string(), 10));

        let expected = NaiveDate::from_ymd_opt(2024, 1, 11).unwrap();
        assert!(puzzle.check_date(expected).unwrap());
    }

    #[test]
    fn test_solver_with_rewriting() {
        let base = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
        let puzzle = TemporalPuzzle::new("test-3", "Find date relative to event")
            .with_reference("event", base)
            .with_constraint(TemporalConstraint::DaysAfter("event".to_string(), 5))
            .with_solutions(vec![NaiveDate::from_ymd_opt(2024, 6, 20).unwrap()]);

        let mut solver = TemporalSolver::with_tools(true, false);
        let result = solver.solve(&puzzle).unwrap();

        assert!(result.solved);
        assert!(result.correct);
        assert_eq!(result.solutions.len(), 1);
    }
}

// ============================================================================
// Adaptive Solver with ReasoningBank Learning
// ============================================================================

use crate::reasoning_bank::{ReasoningBank, Strategy, Trajectory, Verdict};
use crate::timepuzzles::DifficultyVector;

// ═══════════════════════════════════════════════════════════════════════════
// PolicyKernel — learned skip-mode selection
// ═══════════════════════════════════════════════════════════════════════════

/// Skip mode for the temporal solver scan loop.
/// All modes have access to all skip modes.
/// What differs is the *policy* that selects the mode.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SkipMode {
    /// Linear scan: check every date in range (1-day increments)
    None,
    /// Weekday skip: advance by 7 days when DayOfWeek constraint is present
    Weekday,
    /// Hybrid: weekday skip for initial scan, then full refinement pass
    /// around candidates to catch near-misses under noise
    Hybrid,
}

impl Default for SkipMode {
    fn default() -> Self {
        SkipMode::None
    }
}

impl std::fmt::Display for SkipMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkipMode::None => write!(f, "none"),
            SkipMode::Weekday => write!(f, "weekday"),
            SkipMode::Hybrid => write!(f, "hybrid"),
        }
    }
}

/// Context features for PolicyKernel decisions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyContext {
    /// Number of dates in the posterior (search range)
    pub posterior_range: usize,
    /// Number of distractor constraints in the puzzle
    pub distractor_count: usize,
    /// Whether a DayOfWeek constraint is present
    pub has_day_of_week: bool,
    /// Whether noise was injected
    pub noisy: bool,
    /// Difficulty vector components
    pub difficulty: DifficultyVector,
    /// Recent false-hit density (rolling window)
    pub recent_false_hit_rate: f64,
}

/// Outcome of a skip-mode decision for learning.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SkipOutcome {
    /// The skip mode that was used
    pub mode: SkipMode,
    /// Whether the solve was correct
    pub correct: bool,
    /// Steps taken
    pub steps: usize,
    /// Whether this was an early commit that turned out wrong
    pub early_commit_wrong: bool,
    /// Initial candidate count (for normalized penalty)
    pub initial_candidates: usize,
    /// Remaining candidates at commit time (for normalized penalty)
    pub remaining_at_commit: usize,
}

/// Per-context skip-mode statistics for learned policy.
///
/// Two-signal model for Thompson Sampling:
/// 1. **Safety posterior**: Beta(alpha_safety, beta_safety)
///    Updated by whether the commit was correct (not just solved).
///    Drives exploration toward safe arms.
/// 2. **Cost signal**: EMA of normalized step cost.
///    Captures efficiency without contaminating the safety posterior.
///
/// Final score = sample_safety - lambda * cost_ema
/// This separates "is it safe?" (explored by Thompson Sampling)
/// from "is it cheap?" (deterministic penalty).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SkipModeStats {
    pub attempts: usize,
    pub successes: usize,
    pub total_steps: usize,
    pub early_commit_wrongs: usize,
    /// Accumulated normalized early-commit penalty (remaining/initial fractions)
    pub early_commit_penalty_sum: f64,
    /// Safety posterior alpha: correct commits
    pub alpha_safety: f64,
    /// Safety posterior beta: incorrect commits + early wrongs
    pub beta_safety: f64,
    /// Cost EMA: exponential moving average of normalized step cost
    pub cost_ema: f64,
}

/// Lambda: weight of cost penalty in Thompson score.
/// Higher = more cost-sensitive, lower = more safety-focused.
const THOMPSON_LAMBDA: f64 = 0.3;
/// EMA decay factor for cost signal. 0.9 = slow decay, recent history matters.
const COST_EMA_ALPHA: f64 = 0.1;

impl SkipModeStats {
    /// Composite reward for backward compatibility and diagnostics.
    pub fn reward(&self) -> f64 {
        if self.attempts == 0 {
            return 0.5;
        }
        let accuracy = self.successes as f64 / self.attempts as f64;
        let cost_bonus =
            0.3 * (1.0 - (self.total_steps as f64 / self.attempts as f64) / 200.0).max(0.0);
        let avg_penalty = self.early_commit_penalty_sum / self.attempts as f64;
        let robustness_penalty = 0.2 * avg_penalty.min(1.0);
        (accuracy * 0.5 + cost_bonus - robustness_penalty).max(0.0)
    }

    /// Safety Beta posterior parameters.
    ///
    /// Prior: Beta(1, 1) = uniform.
    /// alpha = safe commits (correct, no early-commit penalty)
    /// beta  = unsafe commits (wrong, or early-commit-wrong)
    pub fn safety_beta(&self) -> (f64, f64) {
        (self.alpha_safety + 1.0, self.beta_safety + 1.0)
    }

    /// Posterior variance of the safety Beta distribution.
    /// High variance = high uncertainty = speculative dual-path trigger.
    pub fn safety_variance(&self) -> f64 {
        let (a, b) = self.safety_beta();
        (a * b) / ((a + b).powi(2) * (a + b + 1.0))
    }

    /// Update safety posterior from an outcome.
    pub fn update_safety(&mut self, correct: bool, early_commit_wrong: bool) {
        if correct && !early_commit_wrong {
            self.alpha_safety += 1.0;
        } else {
            self.beta_safety += 1.0;
            if early_commit_wrong {
                // Double penalty for early wrong commits: these are the dangerous ones
                self.beta_safety += 0.5;
            }
        }
    }

    /// Update cost EMA from an outcome.
    pub fn update_cost(&mut self, normalized_steps: f64) {
        if self.attempts <= 1 {
            self.cost_ema = normalized_steps;
        } else {
            self.cost_ema =
                COST_EMA_ALPHA * normalized_steps + (1.0 - COST_EMA_ALPHA) * self.cost_ema;
        }
    }
}

/// Constraint propagation pre-pass mode.
///
/// Controls whether the solver runs arc-consistency before scanning.
/// Selectable by PolicyKernel — kept off by default to preserve
/// learning gradient. If prepass always wins, increase generator
/// ambiguity to restore gradient.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PrepassMode {
    /// No constraint propagation (default)
    Off,
    /// Cheap local pruning: InMonth+DayOfMonth only
    Light,
    /// Full arc consistency: InMonth+DayOfMonth+DayOfWeek
    Full,
}

impl Default for PrepassMode {
    fn default() -> Self {
        PrepassMode::Off
    }
}

impl std::fmt::Display for PrepassMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrepassMode::Off => write!(f, "off"),
            PrepassMode::Light => write!(f, "light"),
            PrepassMode::Full => write!(f, "full"),
        }
    }
}

/// Metrics from constraint propagation pre-pass.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PrepassMetrics {
    /// Total pre-pass invocations
    pub invocations: usize,
    /// Total candidates pruned by pre-pass
    pub pruned_candidates: usize,
    /// Total steps the pre-pass itself took
    pub prepass_steps: usize,
    /// Estimated scan steps saved by pre-pass
    pub scan_steps_saved: usize,
    /// Number of direct solves (scan skipped entirely)
    pub direct_solves: usize,
}

/// PolicyKernel: decides skip_mode based on context.
///
/// Three policy levels:
/// - **Fixed** (Mode A): deterministic heuristic based on posterior_range + distractor_count
/// - **Compiled** (Mode B): compiler-suggested skip_mode from CompiledSolveConfig
/// - **Learned** (Mode C): contextual stats drive selection, adapts from outcomes
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PolicyKernel {
    /// Per-context bucket → per-skip-mode stats (for learned policy)
    pub context_stats: HashMap<String, HashMap<String, SkipModeStats>>,
    /// Early commit penalty accumulator
    pub early_commit_penalties: f64,
    /// Total early commits tracked
    pub early_commits_total: usize,
    /// Total early commits that were wrong
    pub early_commits_wrong: usize,
    /// Exploration rate (legacy, not used by Thompson Sampling)
    pub epsilon: f64,
    /// RNG state (seeded for deterministic Thompson Sampling)
    rng_state: u64,
    /// Constraint propagation pre-pass mode
    pub prepass: PrepassMode,
    /// Pre-pass metrics
    pub prepass_metrics: PrepassMetrics,
    /// Speculative dual-path attempts
    pub speculative_attempts: usize,
    /// Speculative dual-path wins (second arm was better)
    pub speculative_arm2_wins: usize,
}

impl PolicyKernel {
    pub fn new() -> Self {
        Self {
            epsilon: 0.15,
            rng_state: 42,
            ..Default::default()
        }
    }

    /// Fixed baseline policy (Mode A):
    /// Uses risk_score = R - k*D where R=posterior_range, D=distractor_count.
    ///
    /// Constants (fixed, not learned — Mode A is the control arm):
    ///   k = 30 (one distractor reduces effective range by ~30 days)
    ///   T = 140 (threshold: skip only when effective range justifies it)
    ///
    /// Rationale: distractors make the search space noisier, so a rational
    /// fixed agent should be *more cautious* (less likely to skip) when
    /// distractors are present. This is the conservative-under-distractors
    /// baseline that Mode C must learn to outperform.
    ///
    /// Decision:
    ///   If no DayOfWeek: None (nothing to skip to)
    ///   Else risk_score = R - 30*D
    ///     risk_score >= 140 → Weekday (large range, few distractors)
    ///     risk_score <  140 → None    (small range or distractor-heavy)
    const BASELINE_K: usize = 30;
    const BASELINE_T: usize = 140;

    pub fn fixed_policy(ctx: &PolicyContext) -> SkipMode {
        if !ctx.has_day_of_week {
            return SkipMode::None;
        }
        let effective_range = ctx
            .posterior_range
            .saturating_sub(Self::BASELINE_K * ctx.distractor_count);
        if effective_range >= Self::BASELINE_T {
            SkipMode::Weekday
        } else {
            SkipMode::None
        }
    }

    /// Compiled policy (Mode B):
    /// Uses compiler-suggested skip_mode from CompiledSolveConfig.
    /// Falls back to fixed policy if compiler has no suggestion.
    pub fn compiled_policy(ctx: &PolicyContext, compiled_skip: Option<SkipMode>) -> SkipMode {
        compiled_skip.unwrap_or_else(|| Self::fixed_policy(ctx))
    }

    /// Learned policy (Mode C):
    /// Two-signal Thompson Sampling.
    ///
    /// Signal 1 (safety): sample from Beta(alpha_safety, beta_safety)
    ///   - Naturally explores uncertain arms
    ///   - Converges as evidence accumulates
    ///   - O(√T) regret bound
    ///
    /// Signal 2 (cost): deterministic EMA penalty
    ///   - No exploration needed (fully observed)
    ///   - Penalizes expensive arms
    ///
    /// Score = safety_sample - lambda * cost_ema
    ///
    /// When the top two arms are within delta AND uncertainty is high,
    /// returns both arms for speculative dual-path execution.
    pub fn learned_policy(&mut self, ctx: &PolicyContext) -> SkipMode {
        if !ctx.has_day_of_week {
            return SkipMode::None;
        }

        let bucket = Self::context_bucket(ctx);
        let modes = ["none", "weekday", "hybrid"];

        // Collect sampling params before borrowing self for sampling
        let params: Vec<(SkipMode, f64, f64, f64)> = {
            let stats_map = self.context_stats.entry(bucket).or_default();
            modes
                .iter()
                .map(|mode_name| {
                    let stats = stats_map.get(*mode_name).cloned().unwrap_or_default();
                    let (alpha, beta) = stats.safety_beta();
                    let mode = match *mode_name {
                        "weekday" => SkipMode::Weekday,
                        "hybrid" => SkipMode::Hybrid,
                        _ => SkipMode::None,
                    };
                    (mode, alpha, beta, stats.cost_ema)
                })
                .collect()
        };

        // Sample and score (now safe to borrow self mutably for RNG)
        let mut scored: Vec<(SkipMode, f64)> = params
            .into_iter()
            .map(|(mode, alpha, beta, cost_ema)| {
                let safety_sample = self.sample_beta(alpha, beta);
                let score = safety_sample - THOMPSON_LAMBDA * cost_ema;
                (mode, score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored
            .first()
            .map(|(m, _)| m.clone())
            .unwrap_or(SkipMode::None)
    }

    /// Check if speculation is warranted for Mode C.
    ///
    /// Returns Some((arm1, arm2)) if:
    /// 1. Top two arms are within `delta` of each other, AND
    /// 2. Safety variance of the top arm is above threshold
    ///
    /// Otherwise returns None (single-path is sufficient).
    pub fn should_speculate(&mut self, ctx: &PolicyContext) -> Option<(SkipMode, SkipMode)> {
        if !ctx.has_day_of_week {
            return None;
        }

        // Only speculate in medium/large range with distractors or noise
        if ctx.posterior_range < 61 || (ctx.distractor_count == 0 && !ctx.noisy) {
            return None;
        }

        let bucket = Self::context_bucket(ctx);
        let modes = ["none", "weekday", "hybrid"];

        // Collect params first to avoid double mutable borrow
        let params: Vec<(SkipMode, f64, f64, f64, f64)> = {
            let stats_map = self.context_stats.entry(bucket).or_default();
            modes
                .iter()
                .map(|mode_name| {
                    let stats = stats_map.get(*mode_name).cloned().unwrap_or_default();
                    let (alpha, beta) = stats.safety_beta();
                    let variance = stats.safety_variance();
                    let mode = match *mode_name {
                        "weekday" => SkipMode::Weekday,
                        "hybrid" => SkipMode::Hybrid,
                        _ => SkipMode::None,
                    };
                    (mode, alpha, beta, stats.cost_ema, variance)
                })
                .collect()
        };

        // Now sample with self.sample_beta() — no conflicting borrow
        let mut scored: Vec<(SkipMode, f64, f64)> = params
            .into_iter()
            .map(|(mode, alpha, beta, cost_ema, variance)| {
                let safety_sample = self.sample_beta(alpha, beta);
                let score = safety_sample - THOMPSON_LAMBDA * cost_ema;
                (mode, score, variance)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        if scored.len() >= 2 {
            let (ref arm1, score1, var1) = scored[0];
            let (ref arm2, score2, _) = scored[1];
            let delta = 0.15;
            let var_threshold = 0.02; // Beta(1,1) has var≈0.083, so 0.02 = moderate certainty

            if (score1 - score2).abs() < delta && var1 > var_threshold {
                return Some((arm1.clone(), arm2.clone()));
            }
        }

        None
    }

    /// Sample from Beta(alpha, beta) using rejection sampling.
    ///
    /// Uses Joehnk's algorithm for alpha,beta < 1 and
    /// Cheng's BA algorithm for larger params.
    /// Deterministic given internal rng_state.
    fn sample_beta(&mut self, alpha: f64, beta: f64) -> f64 {
        // For our use case, alpha and beta are typically 1..50
        // Use the gamma ratio method: Beta(a,b) = X/(X+Y) where X~Gamma(a), Y~Gamma(b)
        let x = self.sample_gamma(alpha);
        let y = self.sample_gamma(beta);
        if x + y == 0.0 {
            return 0.5;
        }
        x / (x + y)
    }

    /// Sample from Gamma(shape, 1) using Marsaglia & Tsang's method.
    fn sample_gamma(&mut self, shape: f64) -> f64 {
        if shape < 1.0 {
            // Boost: Gamma(shape) = Gamma(shape+1) * U^(1/shape)
            let u = self.next_f64().max(1e-10);
            return self.sample_gamma(shape + 1.0) * u.powf(1.0 / shape);
        }

        let d = shape - 1.0 / 3.0;
        let c = 1.0 / (9.0 * d).sqrt();

        loop {
            let x = self.next_standard_normal();
            let v = (1.0 + c * x).powi(3);
            if v <= 0.0 {
                continue;
            }

            let u = self.next_f64().max(1e-10);

            // Squeeze test
            if u < 1.0 - 0.0331 * x * x * x * x {
                return d * v;
            }
            if u.ln() < 0.5 * x * x + d * (1.0 - v + v.ln()) {
                return d * v;
            }
        }
    }

    /// Box-Muller standard normal sample.
    fn next_standard_normal(&mut self) -> f64 {
        let u1 = self.next_f64().max(1e-10);
        let u2 = self.next_f64();
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f64::consts::PI * u2).cos()
    }

    /// Record the outcome of a skip-mode decision.
    ///
    /// EarlyCommitPenalty is normalized:
    ///   penalty = (remaining_at_commit / initial_candidates) * PENALTY_SCALE
    ///
    /// Committing at 5% of scan = cheap (penalty ≈ 0.05).
    /// Committing at 90% of scan = expensive (penalty ≈ 0.90).
    /// Only charged when the commit is *wrong*.
    const PENALTY_SCALE: f64 = 1.0;

    pub fn record_outcome(&mut self, ctx: &PolicyContext, outcome: &SkipOutcome) {
        let bucket = Self::context_bucket(ctx);
        let mode_name = outcome.mode.to_string();

        let stats_map = self.context_stats.entry(bucket).or_default();
        let stats = stats_map.entry(mode_name).or_default();
        stats.attempts += 1;
        stats.total_steps += outcome.steps;
        if outcome.correct {
            stats.successes += 1;
        }

        // Update two-signal model
        // Signal 1: safety posterior
        stats.update_safety(outcome.correct, outcome.early_commit_wrong);
        // Signal 2: cost EMA (normalize steps to 0..1 range)
        let normalized_cost = (outcome.steps as f64 / 200.0).min(1.0);
        stats.update_cost(normalized_cost);

        if outcome.early_commit_wrong {
            stats.early_commit_wrongs += 1;
            self.early_commits_wrong += 1;
            // Normalized penalty: remaining/initial fraction
            let penalty = if outcome.initial_candidates > 0 {
                (outcome.remaining_at_commit as f64 / outcome.initial_candidates as f64)
                    * Self::PENALTY_SCALE
            } else {
                1.0 - (outcome.steps as f64 / 200.0).min(1.0)
            };
            self.early_commit_penalties += penalty;
            stats.early_commit_penalty_sum += penalty;
        }
        self.early_commits_total += 1;
    }

    /// Early commit penalty rate.
    pub fn early_commit_rate(&self) -> f64 {
        if self.early_commits_total == 0 {
            return 0.0;
        }
        self.early_commits_wrong as f64 / self.early_commits_total as f64
    }

    /// Build a context bucket key for stats grouping (public for witnesses).
    pub fn context_bucket_static(ctx: &PolicyContext) -> String {
        Self::context_bucket(ctx)
    }

    /// Build a context bucket key for stats grouping.
    ///
    /// 3 range × 3 distractor × 2 noise = 18 buckets.
    /// Fine enough for the bandit to learn per-context preferences,
    /// coarse enough to accumulate enough samples per bucket.
    fn context_bucket(ctx: &PolicyContext) -> String {
        let range_bucket = match ctx.posterior_range {
            0..=60 => "small",
            61..=180 => "medium",
            _ => "large",
        };
        let distractor_bucket = match ctx.distractor_count {
            0 => "clean",
            1 => "some",
            _ => "heavy",
        };
        let noise_bucket = if ctx.noisy { "noisy" } else { "clean" };
        format!("{}:{}:{}", range_bucket, distractor_bucket, noise_bucket)
    }

    fn next_f64(&mut self) -> f64 {
        let mut x = self.rng_state.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x as f64) / (u64::MAX as f64)
    }

    /// Print diagnostic summary.
    pub fn print_diagnostics(&self) {
        println!();
        println!("  PolicyKernel Diagnostics (Thompson Sampling, two-signal)");
        println!(
            "  Early commits: {}/{} wrong ({:.1}%)",
            self.early_commits_wrong,
            self.early_commits_total,
            self.early_commit_rate() * 100.0
        );
        println!("  Accumulated penalty: {:.2}", self.early_commit_penalties);
        println!("  Prepass mode: {}", self.prepass);
        if self.prepass_metrics.invocations > 0 {
            println!("  Prepass: {} invocations, {} direct solves, {} candidates pruned, {} scan steps saved",
                self.prepass_metrics.invocations, self.prepass_metrics.direct_solves,
                self.prepass_metrics.pruned_candidates, self.prepass_metrics.scan_steps_saved);
        }
        if self.speculative_attempts > 0 {
            println!(
                "  Speculation: {} attempts, {} arm2 wins ({:.0}%)",
                self.speculative_attempts,
                self.speculative_arm2_wins,
                self.speculative_arm2_wins as f64 / self.speculative_attempts as f64 * 100.0
            );
        }
        println!("  Context buckets: {}", self.context_stats.len());

        for (bucket, modes) in &self.context_stats {
            println!("    {}", bucket);
            for (mode, stats) in modes {
                let (a, b) = stats.safety_beta();
                println!(
                    "      {:<8} n={:<4} safe=Beta({:.1},{:.1}) cost_ema={:.3} reward={:.3}",
                    mode,
                    stats.attempts,
                    a,
                    b,
                    stats.cost_ema,
                    stats.reward()
                );
            }
        }
    }
}

/// Adaptive temporal solver with learning capabilities
///
/// Uses ReasoningBank to:
/// - Track solution trajectories
/// - Learn from successes and failures
/// - Adapt strategy based on puzzle characteristics
/// - Achieve sublinear regret through experience

// ═══════════════════════════════════════════════════════════════════════════
// KnowledgeCompiler — constraint signature → compiled solve config
// ═══════════════════════════════════════════════════════════════════════════

/// Compiled solver configuration for a known constraint signature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompiledSolveConfig {
    /// Whether to use calendar rewriting
    pub use_rewriting: bool,
    /// Minimum steps that succeeded for this signature
    pub max_steps: usize,
    /// Average steps across all successes (for bounded trial budget)
    pub avg_steps: f64,
    /// Number of successful observations compiled
    pub observations: usize,
    /// Expected correctness
    pub expected_correct: bool,
    /// Stop after first solution (early termination for known single-solution puzzles)
    pub stop_after_first: bool,
    /// Hit count (how often this config was used and succeeded)
    pub hit_count: usize,
    /// Counterexample count (failures on this signature)
    pub counterexample_count: usize,
    /// Compiled skip mode suggestion (for Mode B policy)
    pub compiled_skip_mode: SkipMode,
}

impl CompiledSolveConfig {
    /// Confidence: Laplace-smoothed success rate.
    pub fn confidence(&self) -> f64 {
        let total = self.hit_count + self.counterexample_count;
        if total == 0 {
            return 0.5;
        }
        (self.hit_count as f64 + 1.0) / (total as f64 + 2.0)
    }

    /// Trial budget: bounded step limit for Strategy Zero.
    /// Uses avg_steps * 2.0 as budget (enough headroom for variance),
    /// with a floor of max_steps and a ceiling of 25% of external limit.
    pub fn trial_budget(&self, external_limit: usize) -> usize {
        let budget = if self.observations > 2 && self.avg_steps > 1.0 {
            // Enough data: use 2x average steps for headroom
            (self.avg_steps * 2.0) as usize
        } else {
            // Not enough data or trivially small: use max observed steps
            self.max_steps.max(10)
        };
        budget.max(10).min(external_limit / 4)
    }
}

/// KnowledgeCompiler: learns constraint-signature → optimal solve config.
/// Consulted as "Strategy Zero" before any other strategy runs.
///
/// Signature version: v1 (difficulty:sorted_constraints)
/// Change this when canonicalization rules change.
const COMPILER_SIG_VERSION: &str = "v1";

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KnowledgeCompiler {
    /// Compiled constraint signature → config
    pub signature_cache: HashMap<String, CompiledSolveConfig>,
    /// Cache hits
    pub hits: usize,
    /// Cache misses
    pub misses: usize,
    /// False hits (compiled config tried but solve was wrong)
    pub false_hits: usize,
    /// Steps saved by successful Strategy Zero (vs estimated fallback cost)
    pub steps_saved: i64,
    /// Confidence threshold for attempting Strategy Zero
    pub confidence_threshold: f64,
}

impl KnowledgeCompiler {
    pub fn new() -> Self {
        Self {
            confidence_threshold: 0.7,
            ..Default::default()
        }
    }

    /// Build constraint signature from puzzle features.
    /// Includes version prefix for cache safety across refactors.
    pub fn signature(puzzle: &TemporalPuzzle) -> String {
        let mut sig_parts: Vec<String> = puzzle
            .constraints
            .iter()
            .map(|c| constraint_type_name(c))
            .collect();
        sig_parts.sort();
        format!(
            "{}:{}:{}",
            COMPILER_SIG_VERSION,
            puzzle.difficulty,
            sig_parts.join(",")
        )
    }

    /// Compile knowledge from a ReasoningBank's trajectories.
    pub fn compile_from_bank(&mut self, bank: &ReasoningBank) {
        for traj in &bank.trajectories {
            let correct = traj
                .verdict
                .as_ref()
                .map(|v| v.is_success())
                .unwrap_or(false);
            if !correct {
                continue;
            }

            // Build signature from constraint types (versioned)
            let mut sig_parts = traj.constraint_types.clone();
            sig_parts.sort();
            let sig = format!(
                "{}:{}:{}",
                COMPILER_SIG_VERSION,
                traj.difficulty,
                sig_parts.join(",")
            );

            if let Some(attempt) = traj.attempts.first() {
                // Determine compiled skip mode from constraint types
                let has_dow = traj.constraint_types.iter().any(|c| c == "DayOfWeek");
                let compiled_skip = if has_dow {
                    SkipMode::Weekday
                } else {
                    SkipMode::None
                };

                let entry = self
                    .signature_cache
                    .entry(sig)
                    .or_insert(CompiledSolveConfig {
                        use_rewriting: true,
                        max_steps: attempt.steps,
                        avg_steps: 0.0,
                        observations: 0,
                        expected_correct: true,
                        stop_after_first: true,
                        hit_count: 0,
                        counterexample_count: 0,
                        compiled_skip_mode: compiled_skip,
                    });
                // Keep minimum steps that succeeded
                entry.max_steps = entry.max_steps.min(attempt.steps);
                // Running average of steps
                let n = entry.observations as f64;
                entry.avg_steps = (entry.avg_steps * n + attempt.steps as f64) / (n + 1.0);
                entry.observations += 1;
                // Compiled from successful trajectories → seed confidence
                entry.hit_count = entry.observations;
            }
        }
    }

    /// Look up a compiled config for a puzzle. Returns None on cache miss.
    pub fn lookup(&mut self, puzzle: &TemporalPuzzle) -> Option<&CompiledSolveConfig> {
        let sig = Self::signature(puzzle);
        if self.signature_cache.contains_key(&sig) {
            self.hits += 1;
            // Safe: we just checked containment
            self.signature_cache.get(&sig)
        } else {
            self.misses += 1;
            None
        }
    }

    /// Record a counterexample: Strategy Zero failed on this signature.
    /// Quarantine escalation: 2 false hits → disable the entry.
    pub fn record_failure(&mut self, puzzle: &TemporalPuzzle) {
        self.false_hits += 1;
        let sig = Self::signature(puzzle);
        if let Some(config) = self.signature_cache.get_mut(&sig) {
            config.counterexample_count += 1;
            // 2-failure quarantine: disable after 2 false hits
            if config.counterexample_count >= 2 {
                config.expected_correct = false;
            }
        }
    }

    /// Record a successful Strategy Zero hit.
    /// Tracks steps saved vs estimated fallback cost.
    pub fn record_success(&mut self, puzzle: &TemporalPuzzle, actual_steps: usize) {
        let sig = Self::signature(puzzle);
        if let Some(config) = self.signature_cache.get_mut(&sig) {
            config.hit_count += 1;
            // Estimate fallback cost as avg_steps * 2 (full scan is typically ~2x early-term)
            let estimated_fallback = if config.avg_steps > 0.0 {
                (config.avg_steps * 2.0) as i64
            } else {
                config.max_steps as i64
            };
            self.steps_saved += estimated_fallback - actual_steps as i64;
        }
    }

    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }

    pub fn cache_size(&self) -> usize {
        self.signature_cache.len()
    }

    /// Print diagnostic summary: per-signature stats, false hit distribution.
    pub fn print_diagnostics(&self) {
        println!();
        println!("  Compiler Diagnostics (cache_size={})", self.cache_size());
        println!(
            "  {:<40} {:>5} {:>5} {:>6} {:>8} {:>6}",
            "Signature", "Obs", "Hits", "Fails", "AvgStep", "Conf"
        );
        println!("  {}", "-".repeat(72));

        let mut entries: Vec<_> = self.signature_cache.iter().collect();
        entries.sort_by(|a, b| b.1.counterexample_count.cmp(&a.1.counterexample_count));

        for (sig, config) in entries.iter().take(15) {
            let short_sig = if sig.len() > 38 { &sig[..38] } else { sig };
            println!(
                "  {:<40} {:>5} {:>5} {:>6} {:>7.1} {:>.3}",
                short_sig,
                config.observations,
                config.hit_count,
                config.counterexample_count,
                config.avg_steps,
                config.confidence()
            );
        }

        // Summary
        let total_configs = self.signature_cache.len();
        let disabled = self
            .signature_cache
            .values()
            .filter(|c| !c.expected_correct)
            .count();
        let total_false_hits: usize = self
            .signature_cache
            .values()
            .map(|c| c.counterexample_count)
            .sum();
        let false_hit_sigs = self
            .signature_cache
            .values()
            .filter(|c| c.counterexample_count > 0)
            .count();

        println!();
        println!(
            "  Total signatures: {}, disabled: {}",
            total_configs, disabled
        );
        println!(
            "  False hits: {} across {} signatures ({:.1}% of sigs)",
            total_false_hits,
            false_hit_sigs,
            if total_configs > 0 {
                false_hit_sigs as f64 / total_configs as f64 * 100.0
            } else {
                0.0
            }
        );
        println!("  Steps saved by compiler: {}", self.steps_saved);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// StrategyRouter — contextual bandit for strategy selection
// ═══════════════════════════════════════════════════════════════════════════

/// Context bucket key for the bandit.
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct RoutingContext {
    /// Constraint family (sorted constraint types)
    pub constraint_family: String,
    /// Difficulty bucket (1-3=easy, 4-7=mid, 8-10=hard)
    pub difficulty_bucket: u8,
    /// Whether input is noisy
    pub noisy: bool,
}

/// Per-arm stats in the contextual bandit.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ArmStats {
    pub pulls: usize,
    pub successes: usize,
    pub total_steps: usize,
    pub noise_successes: usize,
    pub noise_pulls: usize,
}

impl ArmStats {
    pub fn reward(&self) -> f64 {
        if self.pulls == 0 {
            return 0.5;
        } // Optimistic prior
        let success_rate = self.successes as f64 / self.pulls as f64;
        let cost_bonus = if self.total_steps > 0 {
            // Lower steps = higher reward. Normalize to ~0..0.3
            0.3 * (1.0 - (self.total_steps as f64 / self.pulls as f64) / 100.0).max(0.0)
        } else {
            0.0
        };
        let robustness_bonus = if self.noise_pulls > 0 {
            0.2 * (self.noise_successes as f64 / self.noise_pulls as f64)
        } else {
            0.0
        };
        success_rate * 0.5 + cost_bonus + robustness_bonus
    }
}

/// Adaptive strategy router using contextual bandit.
/// Learns per-context ordering and budget allocation for strategies.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StrategyRouter {
    /// Per-context, per-strategy arm stats
    pub arms: HashMap<RoutingContext, HashMap<String, ArmStats>>,
    /// Exploration rate (epsilon-greedy)
    pub epsilon: f64,
    /// Minimum exploration observations before dropping a strategy
    pub min_observations: usize,
    /// RNG state for exploration
    rng_state: u64,
}

impl StrategyRouter {
    pub fn new() -> Self {
        Self {
            arms: HashMap::new(),
            epsilon: 0.15,
            min_observations: 10,
            rng_state: 42,
        }
    }

    /// Build routing context from puzzle features.
    pub fn context(puzzle: &TemporalPuzzle, noisy: bool) -> RoutingContext {
        let mut families: Vec<String> = puzzle
            .constraints
            .iter()
            .map(|c| constraint_type_name(c))
            .collect();
        families.sort();
        families.dedup();

        let difficulty_bucket = match puzzle.difficulty {
            1..=3 => 1,
            4..=7 => 2,
            _ => 3,
        };

        RoutingContext {
            constraint_family: families.join(","),
            difficulty_bucket,
            noisy,
        }
    }

    /// Select the best strategy for a context.
    /// Returns ordered list of (strategy_name, budget_fraction).
    pub fn select(&mut self, ctx: &RoutingContext, available: &[String]) -> Vec<(String, f64)> {
        // Epsilon-greedy: explore with probability epsilon
        let r = self.next_f64();
        if r < self.epsilon {
            // Explore: random permutation
            let mut shuffled = available.to_vec();
            for i in (1..shuffled.len()).rev() {
                let j = (self.next_f64() * (i + 1) as f64) as usize;
                shuffled.swap(i, j.min(i));
            }
            return shuffled
                .into_iter()
                .map(|s| (s, 1.0 / available.len() as f64))
                .collect();
        }

        // Exploit: rank by reward, filter out strategies with zero success after min_observations
        let arm_map = self.arms.entry(ctx.clone()).or_default();
        let mut ranked: Vec<(String, f64)> = available
            .iter()
            .map(|s| {
                let stats = arm_map.get(s).cloned().unwrap_or_default();
                let should_drop = stats.pulls >= self.min_observations && stats.successes == 0;
                let reward = if should_drop { -1.0 } else { stats.reward() };
                (s.clone(), reward)
            })
            .collect();

        ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

        // Filter out dropped strategies (reward < 0), keep at least one
        let mut result: Vec<(String, f64)> =
            ranked.into_iter().filter(|(_, r)| *r >= 0.0).collect();
        if result.is_empty() {
            result = vec![(available[0].clone(), 1.0)];
        }

        // Allocate budget: best gets 60%, rest split remainder
        let n = result.len();
        result.iter_mut().enumerate().for_each(|(i, (_, budget))| {
            *budget = if i == 0 {
                0.6
            } else {
                0.4 / (n - 1).max(1) as f64
            };
        });

        result
    }

    /// Update arm stats after a solve attempt.
    pub fn update(
        &mut self,
        ctx: &RoutingContext,
        strategy: &str,
        correct: bool,
        steps: usize,
        noisy: bool,
    ) {
        let arm_map = self.arms.entry(ctx.clone()).or_default();
        let stats = arm_map.entry(strategy.to_string()).or_default();
        stats.pulls += 1;
        stats.total_steps += steps;
        if correct {
            stats.successes += 1;
        }
        if noisy {
            stats.noise_pulls += 1;
            if correct {
                stats.noise_successes += 1;
            }
        }
    }

    fn next_f64(&mut self) -> f64 {
        let mut x = self.rng_state.max(1);
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng_state = x;
        (x as f64) / (u64::MAX as f64)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// AdaptiveSolver
// ═══════════════════════════════════════════════════════════════════════════

pub struct AdaptiveSolver {
    /// Internal solver
    solver: TemporalSolver,
    /// ReasoningBank for learning
    pub reasoning_bank: ReasoningBank,
    /// Current strategy
    current_strategy: Strategy,
    /// Total episodes completed
    pub episodes: usize,
    /// When set, solve() uses this step limit instead of the strategy's
    pub external_step_limit: Option<usize>,
    /// KnowledgeCompiler for Strategy Zero (compiled solve configs)
    pub compiler: KnowledgeCompiler,
    /// Whether to use the compiler as Strategy Zero
    pub compiler_enabled: bool,
    /// Adaptive strategy router (contextual bandit)
    pub router: StrategyRouter,
    /// Whether to use the adaptive router instead of fixed strategy selection
    pub router_enabled: bool,
    /// PolicyKernel for skip-mode decisions (all modes use this)
    pub policy_kernel: PolicyKernel,
    /// Whether the current puzzle is noisy (set by caller before solve)
    pub noisy_hint: bool,
}

impl Default for AdaptiveSolver {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveSolver {
    /// Create a new adaptive solver
    pub fn new() -> Self {
        Self {
            solver: TemporalSolver::default(),
            reasoning_bank: ReasoningBank::new(),
            current_strategy: Strategy::default(),
            episodes: 0,
            external_step_limit: None,
            compiler: KnowledgeCompiler::new(),
            compiler_enabled: false,
            router: StrategyRouter::new(),
            router_enabled: false,
            policy_kernel: PolicyKernel::new(),
            noisy_hint: false,
        }
    }

    /// Create with pre-trained ReasoningBank
    pub fn with_reasoning_bank(reasoning_bank: ReasoningBank) -> Self {
        Self {
            solver: TemporalSolver::default(),
            reasoning_bank,
            current_strategy: Strategy::default(),
            episodes: 0,
            external_step_limit: None,
            compiler: KnowledgeCompiler::new(),
            compiler_enabled: false,
            router: StrategyRouter::new(),
            router_enabled: false,
            policy_kernel: PolicyKernel::new(),
            noisy_hint: false,
        }
    }

    /// Recompile knowledge from the current ReasoningBank.
    pub fn recompile(&mut self) {
        self.compiler.compile_from_bank(&self.reasoning_bank);
    }

    /// Get mutable reference to the internal solver for configuration.
    pub fn solver_mut(&mut self) -> &mut TemporalSolver {
        &mut self.solver
    }

    /// Build a PolicyContext from puzzle features.
    fn build_policy_context(&self, puzzle: &TemporalPuzzle) -> PolicyContext {
        let has_dow = puzzle
            .constraints
            .iter()
            .any(|c| matches!(c, TemporalConstraint::DayOfWeek(_)));

        // Estimate posterior range from Between constraint
        let posterior_range = puzzle
            .constraints
            .iter()
            .find_map(|c| match c {
                TemporalConstraint::Between(start, end) => {
                    Some((*end - *start).num_days().max(0) as usize)
                }
                _ => None,
            })
            .unwrap_or(365);

        // Count distractors: redundant constraints that don't narrow the search
        // (wider Between, redundant InYear, After well before range)
        let distractor_count = count_distractors(puzzle);

        let dv = puzzle
            .difficulty_vector
            .clone()
            .unwrap_or_else(|| DifficultyVector::from_scalar(puzzle.difficulty));

        PolicyContext {
            posterior_range,
            distractor_count,
            has_day_of_week: has_dow,
            noisy: self.noisy_hint,
            difficulty: dv,
            recent_false_hit_rate: self.policy_kernel.early_commit_rate(),
        }
    }

    /// Solve a puzzle with adaptive learning.
    ///
    /// All modes have access to the same solver capabilities (including skip_weekday).
    /// What differs is the **policy** that decides how to use them:
    /// - Mode A (baseline): fixed heuristic policy
    /// - Mode B (compiler): compiler-suggested policy
    /// - Mode C (full): learned PolicyKernel policy
    pub fn solve(&mut self, puzzle: &TemporalPuzzle) -> Result<SolverResult> {
        // Reset solver state
        self.solver.skip_weekday = None;

        // Get constraint types for pattern matching
        let constraint_types: Vec<String> = puzzle
            .constraints
            .iter()
            .map(|c| constraint_type_name(c))
            .collect();

        // Build policy context (same for all modes)
        let policy_ctx = self.build_policy_context(puzzle);

        // ─── PolicyKernel: decide skip_mode (all modes participate) ──────
        let skip_mode = if self.router_enabled {
            // Mode C: learned policy
            self.policy_kernel.learned_policy(&policy_ctx)
        } else if self.compiler_enabled {
            // Mode B: compiler-suggested policy
            let compiled_skip = self
                .compiler
                .lookup(puzzle)
                .map(|config| config.compiled_skip_mode.clone());
            PolicyKernel::compiled_policy(&policy_ctx, compiled_skip)
        } else {
            // Mode A: fixed baseline policy
            PolicyKernel::fixed_policy(&policy_ctx)
        };

        // Apply skip_mode to solver
        match &skip_mode {
            SkipMode::None => {
                self.solver.skip_weekday = None;
            }
            SkipMode::Weekday => {
                self.solver.skip_weekday = puzzle.constraints.iter().find_map(|c| match c {
                    TemporalConstraint::DayOfWeek(w) => Some(*w),
                    _ => None,
                });
            }
            SkipMode::Hybrid => {
                // Hybrid: use weekday skip for initial scan (set here),
                // then do a refinement pass below if needed.
                // Force minimum evidence: never stop_after_first in Hybrid mode.
                self.solver.skip_weekday = puzzle.constraints.iter().find_map(|c| match c {
                    TemporalConstraint::DayOfWeek(w) => Some(*w),
                    _ => None,
                });
                // Hybrid safety: disable early termination so solver checks
                // all matching weekdays before committing
                self.solver.stop_after_first = false;
            }
        }

        // Accumulated steps across all attempts (Strategy Zero + fallback)
        let mut extra_steps: usize = 0;
        let mut extra_tool_calls: usize = 0;

        // ─── Strategy Zero: KnowledgeCompiler (bounded trial) ────────────
        if self.compiler_enabled {
            let conf_threshold = self.compiler.confidence_threshold;
            let compiled = self.compiler.lookup(puzzle).map(|config| {
                (
                    config.expected_correct,
                    config.confidence(),
                    config.trial_budget(self.external_step_limit.unwrap_or(400)),
                    config.use_rewriting,
                    config.stop_after_first,
                )
            });

            if let Some((expected_correct, confidence, trial_budget, use_rewriting, stop_first)) =
                compiled
            {
                if expected_correct && confidence >= conf_threshold {
                    self.solver.calendar_tool = use_rewriting;
                    self.solver.stop_after_first = stop_first;
                    self.solver.max_steps = trial_budget;

                    let start = std::time::Instant::now();
                    let result = self.solver.solve(puzzle)?;
                    let latency = start.elapsed().as_millis() as u64;

                    self.solver.stop_after_first = false;

                    if result.correct {
                        self.compiler.record_success(puzzle, result.steps);
                        let mut trajectory = Trajectory::new(&puzzle.id, puzzle.difficulty);
                        trajectory.constraint_types = constraint_types;
                        trajectory.latency_ms = latency;
                        let sol_str = result
                            .solutions
                            .first()
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "none".to_string());
                        let bucket_key = PolicyKernel::context_bucket_static(&policy_ctx);
                        trajectory.record_attempt_witnessed(
                            sol_str,
                            0.95,
                            result.steps,
                            result.tool_calls,
                            "compiler",
                            &skip_mode.to_string(),
                            &bucket_key,
                        );
                        trajectory.set_verdict(
                            Verdict::Success,
                            puzzle.solutions.first().map(|d| d.to_string()),
                        );
                        self.reasoning_bank.record_trajectory(trajectory);
                        self.episodes += 1;

                        // Record successful skip outcome
                        let outcome = SkipOutcome {
                            mode: skip_mode,
                            correct: true,
                            steps: result.steps,
                            early_commit_wrong: false,
                            initial_candidates: policy_ctx.posterior_range,
                            remaining_at_commit: 0,
                        };
                        self.policy_kernel.record_outcome(&policy_ctx, &outcome);

                        if self.router_enabled {
                            let ctx = StrategyRouter::context(puzzle, false);
                            self.router
                                .update(&ctx, "compiler", true, result.steps, false);
                        }

                        return Ok(result);
                    } else {
                        extra_steps += result.steps;
                        extra_tool_calls += result.tool_calls;
                        self.compiler.record_failure(puzzle);

                        // Record early commit wrong if solver claimed solved but was wrong
                        if result.solved && !result.correct {
                            // Estimate remaining: initial minus steps scanned
                            let remaining = policy_ctx.posterior_range.saturating_sub(result.steps);
                            let outcome = SkipOutcome {
                                mode: skip_mode.clone(),
                                correct: false,
                                steps: result.steps,
                                early_commit_wrong: true,
                                initial_candidates: policy_ctx.posterior_range,
                                remaining_at_commit: remaining,
                            };
                            self.policy_kernel.record_outcome(&policy_ctx, &outcome);
                        }
                    }
                }
            }
        }

        // ─── Strategy Selection (fixed or router) ───────────────────────
        if self.router_enabled {
            let ctx = StrategyRouter::context(puzzle, false);
            let available = vec![
                "default".to_string(),
                "aggressive".to_string(),
                "conservative".to_string(),
                "adaptive".to_string(),
            ];
            let ranked = self.router.select(&ctx, &available);
            if let Some((top_strategy, _)) = ranked.first() {
                self.current_strategy = self
                    .reasoning_bank
                    .strategy_from_name(top_strategy, puzzle.difficulty);
            }
        } else {
            self.current_strategy = self
                .reasoning_bank
                .get_strategy(puzzle.difficulty, &constraint_types);
        }

        // Configure solver based on strategy (external limit overrides strategy)
        self.solver.calendar_tool = self.current_strategy.use_rewriting;
        self.solver.max_steps = self
            .external_step_limit
            .unwrap_or(self.current_strategy.max_steps);
        self.solver.stop_after_first = false;
        // Wire prepass mode from PolicyKernel
        self.solver.prepass_mode = self.policy_kernel.prepass.clone();

        // Create trajectory for this puzzle
        let mut trajectory = Trajectory::new(&puzzle.id, puzzle.difficulty);
        trajectory.constraint_types = constraint_types;

        // Solve the puzzle
        let start = std::time::Instant::now();
        let mut result = self.solver.solve(puzzle)?;
        trajectory.latency_ms = start.elapsed().as_millis() as u64;

        // Track prepass metrics if enabled
        if self.policy_kernel.prepass != PrepassMode::Off {
            self.policy_kernel.prepass_metrics.invocations += 1;
            // Direct solve: steps < 15 and correct means propagation worked
            if result.steps <= 15 && result.correct && result.solved {
                self.policy_kernel.prepass_metrics.direct_solves += 1;
                // Estimate scan steps saved
                let would_have_scanned = policy_ctx.posterior_range;
                self.policy_kernel.prepass_metrics.scan_steps_saved += would_have_scanned;
            }
            // Estimate pruned candidates
            let actual_range = (result.steps as f64 * 7.0) as usize; // rough
            let saved = policy_ctx.posterior_range.saturating_sub(actual_range);
            self.policy_kernel.prepass_metrics.pruned_candidates += saved;
        }

        // ─── Hybrid refinement pass ──────────────────────────────────────
        // If Hybrid mode was used and we found solutions via weekday skip,
        // do a narrow linear scan around each candidate to catch near-misses.
        if skip_mode == SkipMode::Hybrid && !result.solutions.is_empty() {
            let mut refined_solutions = result.solutions.clone();
            self.solver.skip_weekday = None; // Linear for refinement
            let saved_max = self.solver.max_steps;
            self.solver.max_steps = 14; // Check ±7 days around each candidate

            for candidate in &result.solutions {
                let refine_start = *candidate - chrono::Duration::days(7);
                let refine_end = *candidate + chrono::Duration::days(7);
                let refine_puzzle = TemporalPuzzle {
                    id: puzzle.id.clone(),
                    description: puzzle.description.clone(),
                    constraints: puzzle.constraints.clone(),
                    references: puzzle.references.clone(),
                    solutions: puzzle.solutions.clone(),
                    difficulty: puzzle.difficulty,
                    tags: puzzle.tags.clone(),
                    difficulty_vector: puzzle.difficulty_vector.clone(),
                };
                // Manually search the refinement window
                let mut cur = refine_start;
                while cur <= refine_end {
                    if let Ok(true) = refine_puzzle.check_date(cur) {
                        if !refined_solutions.contains(&cur) {
                            refined_solutions.push(cur);
                        }
                    }
                    cur = match cur.succ_opt() {
                        Some(d) => d,
                        None => break,
                    };
                    result.steps += 1;
                }
            }
            self.solver.max_steps = saved_max;
            result.solutions = refined_solutions;
            // Re-check correctness after refinement
            result.correct = if puzzle.solutions.is_empty() {
                true
            } else {
                puzzle
                    .solutions
                    .iter()
                    .all(|s| result.solutions.contains(s))
            };
        }

        // Accumulate overhead from failed Strategy Zero attempt
        result.steps += extra_steps;
        result.tool_calls += extra_tool_calls;

        // Record attempt
        let solution_str = result
            .solutions
            .first()
            .map(|d| d.to_string())
            .unwrap_or_else(|| "none".to_string());

        let confidence = self.calculate_confidence(&result, puzzle);

        let bucket_key = PolicyKernel::context_bucket_static(&policy_ctx);
        trajectory.record_attempt_witnessed(
            solution_str,
            confidence,
            result.steps,
            result.tool_calls,
            &self.current_strategy.name,
            &skip_mode.to_string(),
            &bucket_key,
        );

        // Determine verdict
        let verdict = if result.correct {
            if confidence >= 0.9 {
                Verdict::Success
            } else {
                Verdict::Acceptable
            }
        } else if result.solved {
            Verdict::Suboptimal {
                reason: "Solution found but incorrect".to_string(),
                delta: 1.0 - confidence,
            }
        } else if confidence < self.current_strategy.confidence_threshold {
            Verdict::LowConfidence
        } else {
            Verdict::Failed
        };

        trajectory.set_verdict(verdict, puzzle.solutions.first().map(|d| d.to_string()));

        // ─── Record PolicyKernel outcome ─────────────────────────────────
        let early_commit_wrong = result.solved && !result.correct;
        let remaining = policy_ctx.posterior_range.saturating_sub(result.steps);
        let outcome = SkipOutcome {
            mode: skip_mode,
            correct: result.correct,
            steps: result.steps,
            early_commit_wrong,
            initial_candidates: policy_ctx.posterior_range,
            remaining_at_commit: remaining,
        };
        self.policy_kernel.record_outcome(&policy_ctx, &outcome);

        // Update router stats
        if self.router_enabled {
            let ctx = StrategyRouter::context(puzzle, false);
            self.router.update(
                &ctx,
                &self.current_strategy.name,
                result.correct,
                result.steps,
                false,
            );
        }

        // Record trajectory for learning
        self.reasoning_bank.record_trajectory(trajectory);
        self.episodes += 1;

        Ok(result)
    }

    /// Calculate confidence in a result
    fn calculate_confidence(&self, result: &SolverResult, puzzle: &TemporalPuzzle) -> f64 {
        let mut confidence = 0.5;

        // Higher confidence if solved quickly
        if result.solved {
            confidence += 0.2;
            if result.steps < self.solver.max_steps / 2 {
                confidence += 0.1;
            }
        }

        // Higher confidence with tool use on complex puzzles
        if result.tool_calls > 0 && puzzle.difficulty > 5 {
            confidence += 0.1;
        }

        // Lower confidence if took many steps
        if result.steps > self.solver.max_steps * 3 / 4 {
            confidence -= 0.1;
        }

        // Adjust based on learned calibration
        let calibrated_threshold = self
            .reasoning_bank
            .calibration
            .get_threshold(puzzle.difficulty);
        if confidence >= calibrated_threshold {
            confidence += 0.05;
        }

        confidence.min(1.0).max(0.0)
    }

    /// Get learning progress
    pub fn learning_progress(&self) -> crate::reasoning_bank::LearningProgress {
        self.reasoning_bank.learning_progress()
    }

    /// Get hints for a puzzle
    pub fn get_hints(&self, constraint_types: &[String]) -> Vec<String> {
        self.reasoning_bank.get_hints(constraint_types)
    }
}

/// Count distractor constraints in a puzzle.
/// A distractor is a constraint that is likely redundant (doesn't narrow the search much).
/// Public so the generator can tag puzzles with their distractor count.
pub fn count_distractors(puzzle: &TemporalPuzzle) -> usize {
    let mut count = 0;
    let mut seen_between = false;
    let mut seen_inyear = false;
    let mut seen_dow = false;

    for c in &puzzle.constraints {
        match c {
            TemporalConstraint::Between(_, _) => {
                if seen_between {
                    count += 1; // Redundant Between (wider or duplicate)
                }
                seen_between = true;
            }
            TemporalConstraint::InYear(_) => {
                if seen_inyear {
                    count += 1; // Redundant InYear
                }
                seen_inyear = true;
            }
            TemporalConstraint::DayOfWeek(_) => {
                if seen_dow {
                    count += 1; // Redundant DayOfWeek
                }
                seen_dow = true;
            }
            TemporalConstraint::After(d) => {
                // After a date well before the Between range → distractor
                if seen_between {
                    if let Some(between_start) = puzzle.constraints.iter().find_map(|c2| match c2 {
                        TemporalConstraint::Between(s, _) => Some(*s),
                        _ => None,
                    }) {
                        if *d < between_start - chrono::Duration::days(14) {
                            count += 1;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    count
}

/// Get the type name of a constraint for pattern matching
fn constraint_type_name(constraint: &TemporalConstraint) -> String {
    match constraint {
        TemporalConstraint::Exact(_) => "Exact".to_string(),
        TemporalConstraint::After(_) => "After".to_string(),
        TemporalConstraint::Before(_) => "Before".to_string(),
        TemporalConstraint::Between(_, _) => "Between".to_string(),
        TemporalConstraint::DayOfWeek(_) => "DayOfWeek".to_string(),
        TemporalConstraint::DaysAfter(_, _) => "DaysAfter".to_string(),
        TemporalConstraint::DaysBefore(_, _) => "DaysBefore".to_string(),
        TemporalConstraint::InMonth(_) => "InMonth".to_string(),
        TemporalConstraint::InYear(_) => "InYear".to_string(),
        TemporalConstraint::DayOfMonth(_) => "DayOfMonth".to_string(),
        TemporalConstraint::RelativeToEvent(_, _) => "RelativeToEvent".to_string(),
    }
}
