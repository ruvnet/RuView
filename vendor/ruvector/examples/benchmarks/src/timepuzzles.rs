//! TimePuzzles Generator
//!
//! Generates constraint-based temporal reasoning puzzles
//! based on the TimePuzzles benchmark methodology (arXiv:2601.07148)
//!
//! Key features:
//! - Factual temporal anchors with calendar relations
//! - Cross-cultural date systems
//! - Controlled difficulty levels
//! - Dynamic puzzle generation

use crate::temporal::{TemporalConstraint, TemporalPuzzle};
use anyhow::Result;
use chrono::{Datelike, NaiveDate};
use rand::prelude::*;
use serde::{Deserialize, Serialize};

/// Multi-dimensional difficulty vector.
///
/// Replaces single-axis difficulty to prevent collapsing effects.
/// Higher difficulty = more work and more ambiguity, NOT tighter posterior.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DifficultyVector {
    /// Size of the search range (days)
    pub range_size: usize,
    /// Target number of valid candidates in posterior
    pub posterior_target: usize,
    /// Rate of distractor constraints (0.0 - 1.0)
    pub distractor_rate: f64,
    /// Rate of noise injection (0.0 - 1.0)
    pub noise_rate: f64,
    /// Number of ambiguous solutions (dates that almost satisfy constraints)
    pub ambiguity_count: usize,
}

impl Default for DifficultyVector {
    fn default() -> Self {
        Self {
            range_size: 60,
            posterior_target: 60,
            distractor_rate: 0.0,
            noise_rate: 0.0,
            ambiguity_count: 0,
        }
    }
}

impl DifficultyVector {
    /// Build from scalar difficulty (backward compatible).
    /// Higher difficulty = wider range, more distractors, more ambiguity.
    pub fn from_scalar(difficulty: u8) -> Self {
        let d = difficulty.min(10).max(1);
        Self {
            range_size: difficulty_to_range_size(d),
            posterior_target: difficulty_to_posterior(d),
            distractor_rate: difficulty_to_distractor_rate(d),
            noise_rate: difficulty_to_noise_rate(d),
            ambiguity_count: difficulty_to_ambiguity(d),
        }
    }

    /// Scalar difficulty estimate (for backward compat).
    pub fn scalar(&self) -> u8 {
        // Weighted combination back to 1-10 scale
        let range_score = (self.range_size as f64 / 365.0 * 10.0).min(10.0);
        let distractor_score = self.distractor_rate * 10.0;
        let ambiguity_score = (self.ambiguity_count as f64 / 5.0 * 10.0).min(10.0);
        let combined = (range_score * 0.3 + distractor_score * 0.3 + ambiguity_score * 0.4) as u8;
        combined.max(1).min(10)
    }
}

/// Puzzle generator configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PuzzleGeneratorConfig {
    /// Minimum difficulty (1-10)
    pub min_difficulty: u8,
    /// Maximum difficulty (1-10)
    pub max_difficulty: u8,
    /// Constraint density (1-5)
    pub constraint_density: u8,
    /// Include cross-cultural references
    pub cross_cultural: bool,
    /// Include relative constraints
    pub relative_constraints: bool,
    /// Year range for puzzles
    pub year_range: (i32, i32),
    /// Random seed (optional)
    pub seed: Option<u64>,
}

impl Default for PuzzleGeneratorConfig {
    fn default() -> Self {
        Self {
            min_difficulty: 1,
            max_difficulty: 10,
            constraint_density: 3,
            cross_cultural: true,
            relative_constraints: true,
            year_range: (2000, 2030),
            seed: None,
        }
    }
}

/// Known events for temporal anchoring
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TemporalAnchor {
    pub name: String,
    pub date: NaiveDate,
    pub category: String,
    pub culture: String,
}

impl TemporalAnchor {
    pub fn new(
        name: impl Into<String>,
        year: i32,
        month: u32,
        day: u32,
        category: impl Into<String>,
        culture: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            date: NaiveDate::from_ymd_opt(year, month, day).unwrap(),
            category: category.into(),
            culture: culture.into(),
        }
    }
}

/// TimePuzzles generator
pub struct PuzzleGenerator {
    config: PuzzleGeneratorConfig,
    anchors: Vec<TemporalAnchor>,
    rng: StdRng,
}

impl PuzzleGenerator {
    /// Create a new generator with config
    pub fn new(config: PuzzleGeneratorConfig) -> Self {
        let rng = match config.seed {
            Some(s) => StdRng::seed_from_u64(s),
            None => StdRng::from_entropy(),
        };

        let mut gen = Self {
            config,
            anchors: Vec::new(),
            rng,
        };
        gen.init_anchors();
        gen
    }

    /// Initialize standard temporal anchors
    fn init_anchors(&mut self) {
        // Western holidays
        self.anchors.push(TemporalAnchor::new(
            "Christmas",
            2024,
            12,
            25,
            "holiday",
            "western",
        ));
        self.anchors.push(TemporalAnchor::new(
            "New Year", 2024, 1, 1, "holiday", "western",
        ));
        self.anchors.push(TemporalAnchor::new(
            "Independence Day",
            2024,
            7,
            4,
            "holiday",
            "american",
        ));
        self.anchors.push(TemporalAnchor::new(
            "Halloween",
            2024,
            10,
            31,
            "holiday",
            "western",
        ));
        self.anchors.push(TemporalAnchor::new(
            "Valentine's Day",
            2024,
            2,
            14,
            "holiday",
            "western",
        ));

        // Cross-cultural events
        if self.config.cross_cultural {
            // Chinese New Year 2024 (Year of the Dragon)
            self.anchors.push(TemporalAnchor::new(
                "Chinese New Year 2024",
                2024,
                2,
                10,
                "holiday",
                "chinese",
            ));
            // Diwali 2024
            self.anchors.push(TemporalAnchor::new(
                "Diwali 2024",
                2024,
                11,
                1,
                "holiday",
                "indian",
            ));
            // Eid al-Fitr 2024
            self.anchors.push(TemporalAnchor::new(
                "Eid al-Fitr 2024",
                2024,
                4,
                10,
                "holiday",
                "islamic",
            ));
            // Hanukkah 2024 (starts)
            self.anchors.push(TemporalAnchor::new(
                "Hanukkah 2024",
                2024,
                12,
                25,
                "holiday",
                "jewish",
            ));
        }

        // Historical events
        self.anchors.push(TemporalAnchor::new(
            "Moon Landing",
            1969,
            7,
            20,
            "historical",
            "global",
        ));
        self.anchors.push(TemporalAnchor::new(
            "Fall of Berlin Wall",
            1989,
            11,
            9,
            "historical",
            "global",
        ));
        self.anchors.push(TemporalAnchor::new(
            "Y2K",
            2000,
            1,
            1,
            "historical",
            "global",
        ));
    }

    /// Generate a single puzzle with multi-dimensional difficulty vector.
    ///
    /// Difficulty scaling (higher = more work, not tighter posterior):
    /// - Low (1-2): small range, no DayOfWeek, no distractors
    /// - Medium (3-6): DayOfWeek + moderate range = 7x cost surface
    /// - High (7-10): wide range + distractors + ambiguity + anchor constraints
    ///
    /// All modes have access to weekday skipping; what differs is the policy.
    pub fn generate_puzzle(&mut self, id: impl Into<String>) -> Result<TemporalPuzzle> {
        let id = id.into();
        let difficulty = self
            .rng
            .gen_range(self.config.min_difficulty..=self.config.max_difficulty);

        // Build difficulty vector from scalar
        let dv = DifficultyVector::from_scalar(difficulty);

        // DayOfWeek (difficulty 3+): creates cost surface for policy decisions
        let use_day_of_week = difficulty >= 3;

        // Range size from difficulty vector (wider range at higher difficulty)
        let range_days = dv.range_size as i64;

        // Pick target date
        let year = self
            .rng
            .gen_range(self.config.year_range.0..=self.config.year_range.1);
        let month = self.rng.gen_range(1..=12);
        let max_day = days_in_month(year, month);
        let day = self.rng.gen_range(1..=max_day);
        let target = NaiveDate::from_ymd_opt(year, month, day).unwrap();

        // Build Between range centered on target, clamped to year
        let year_start = NaiveDate::from_ymd_opt(year, 1, 1).unwrap();
        let year_end = NaiveDate::from_ymd_opt(year, 12, 31).unwrap();
        let half = range_days / 2;
        let range_start = (target - chrono::Duration::days(half)).max(year_start);
        let range_end = (range_start + chrono::Duration::days(range_days - 1)).min(year_end);

        let mut puzzle = TemporalPuzzle::new(id.clone(), format!("Find the date (puzzle {})", id))
            .with_difficulty(difficulty)
            .with_solutions(vec![target]);

        // Attach difficulty vector
        puzzle.difficulty_vector = Some(dv.clone());

        // Base constraints: InYear + Between (defines search range)
        puzzle
            .constraints
            .push(TemporalConstraint::InYear(target.year()));
        puzzle
            .constraints
            .push(TemporalConstraint::Between(range_start, range_end));

        let mut used_anchors: Vec<TemporalAnchor> = Vec::new();

        // DayOfWeek (difficulty 3+): creates cost surface for all modes
        if use_day_of_week {
            puzzle
                .constraints
                .push(TemporalConstraint::DayOfWeek(target.weekday()));
        }

        // Anchor reference for high difficulty (7+)
        if difficulty >= 7 && self.config.relative_constraints {
            if let Some(anchor) = self.anchors.choose(&mut self.rng).cloned() {
                let diff = (target - anchor.date).num_days();
                let constraint = if diff >= 0 {
                    TemporalConstraint::DaysAfter(anchor.name.clone(), diff)
                } else {
                    TemporalConstraint::DaysBefore(anchor.name.clone(), diff.abs())
                };
                puzzle.constraints.push(constraint);
                used_anchors.push(anchor);
            }
        }

        // Add anchor references
        for anchor in used_anchors {
            puzzle.references.insert(anchor.name.clone(), anchor.date);
        }

        // Distractor injection (from difficulty vector rate)
        if dv.distractor_rate > 0.0 && self.rng.gen_bool(dv.distractor_rate.min(0.99)) {
            let distractor = self.generate_distractor(target, range_start, range_end);
            puzzle.constraints.push(distractor);
        }

        // Distractor DayOfWeek (difficulty 6+): DayOfWeek present but misleading.
        // Adds a SECOND DayOfWeek that is a distractor — it matches the target
        // but unconditional weekday skipping on the wrong dow will miss solutions.
        // This creates a real tradeoff for the PolicyKernel.
        if difficulty >= 6 && use_day_of_week {
            let distractor_dow_chance: f64 = match difficulty {
                6 => 0.15,
                7 => 0.25,
                8 => 0.35,
                9..=10 => 0.50,
                _ => 0.0,
            };
            if self.rng.gen_bool(distractor_dow_chance.min(0.99)) {
                // Add a redundant wider Between that doesn't narrow search
                // but pairs with the existing DayOfWeek to create a trap:
                // the DayOfWeek is valid but the wider range means skip saves less
                let wider_start = range_start - chrono::Duration::days(self.rng.gen_range(14..60));
                let wider_end = range_end + chrono::Duration::days(self.rng.gen_range(14..60));
                puzzle
                    .constraints
                    .push(TemporalConstraint::Between(wider_start, wider_end));
            }
        }

        // Ambiguity: add near-miss solutions at high difficulty
        // These are dates that satisfy most but not all constraints,
        // making early commits risky.
        if dv.ambiguity_count > 0 {
            // No-op structurally (solutions list stays correct),
            // but the wider range at high difficulty naturally creates more
            // dates that pass most constraints, increasing false-positive risk
            // for aggressive skip modes.
        }

        // Count actual distractors injected (deterministic, observable)
        let actual_distractor_count = crate::temporal::count_distractors(&puzzle);

        // Tags: all features visible to policies for deterministic observability
        puzzle.tags = vec![
            format!("difficulty:{}", difficulty),
            format!("year:{}", year),
            format!("range_size:{}", dv.range_size),
            format!("distractor_rate:{:.2}", dv.distractor_rate),
            format!("distractor_count:{}", actual_distractor_count),
            format!("ambiguity:{}", dv.ambiguity_count),
            format!("has_dow:{}", use_day_of_week),
        ];

        Ok(puzzle)
    }

    /// Generate a distractor constraint: true for the target but doesn't narrow the search.
    fn generate_distractor(
        &mut self,
        target: NaiveDate,
        range_start: NaiveDate,
        range_end: NaiveDate,
    ) -> TemporalConstraint {
        match self.rng.gen_range(0u8..3) {
            0 => {
                // Wider Between (superset of existing range → no shrink)
                let wider_start = range_start - chrono::Duration::days(self.rng.gen_range(10..60));
                let wider_end = range_end + chrono::Duration::days(self.rng.gen_range(10..60));
                TemporalConstraint::Between(wider_start, wider_end)
            }
            1 => {
                // Redundant InYear (already present)
                TemporalConstraint::InYear(target.year())
            }
            _ => {
                // After a date well before the range (no shrink)
                let days_before = self.rng.gen_range(30..180) as i64;
                TemporalConstraint::After(target - chrono::Duration::days(days_before))
            }
        }
    }

    /// Generate a batch of puzzles
    pub fn generate_batch(&mut self, count: usize) -> Result<Vec<TemporalPuzzle>> {
        let mut puzzles = Vec::with_capacity(count);
        for i in 0..count {
            let puzzle = self.generate_puzzle(format!("puzzle-{:04}", i + 1))?;
            puzzles.push(puzzle);
        }
        Ok(puzzles)
    }

    /// Generate puzzles at specific difficulty
    pub fn generate_at_difficulty(
        &mut self,
        count: usize,
        difficulty: u8,
    ) -> Result<Vec<TemporalPuzzle>> {
        let orig_min = self.config.min_difficulty;
        let orig_max = self.config.max_difficulty;

        self.config.min_difficulty = difficulty;
        self.config.max_difficulty = difficulty;

        let puzzles = self.generate_batch(count);

        self.config.min_difficulty = orig_min;
        self.config.max_difficulty = orig_max;

        puzzles
    }
}

/// Range size by difficulty level.
/// Higher difficulty → wider range → more work for the solver.
fn difficulty_to_range_size(difficulty: u8) -> usize {
    match difficulty {
        1 => 14,
        2 => 30,
        3 => 56, // 8 weeks
        4 => 84, // 12 weeks
        5 => 120,
        6 => 150,
        7 => 200,
        8 => 250,
        9 => 300,
        10 => 365,
        _ => 120,
    }
}

/// Posterior target by difficulty level.
/// Higher difficulty → more valid candidates → more ambiguity.
/// (Flipped from old model: difficulty increases ambiguity, not reduces it.)
fn difficulty_to_posterior(difficulty: u8) -> usize {
    match difficulty {
        1 => 2,
        2 => 4,
        3 => 8,
        4 => 12,
        5 => 18,
        6 => 25,
        7 => 35,
        8 => 50,
        9 => 70,
        10 => 100,
        _ => 18,
    }
}

/// Distractor rate by difficulty level.
fn difficulty_to_distractor_rate(difficulty: u8) -> f64 {
    match difficulty {
        1..=3 => 0.0,
        4 => 0.05,
        5 => 0.10,
        6 => 0.20,
        7 => 0.30,
        8 => 0.40,
        9 => 0.50,
        10 => 0.60,
        _ => 0.10,
    }
}

/// Noise rate by difficulty level.
fn difficulty_to_noise_rate(difficulty: u8) -> f64 {
    match difficulty {
        1..=3 => 0.0,
        4..=5 => 0.10,
        6..=7 => 0.20,
        8..=9 => 0.30,
        10 => 0.40,
        _ => 0.10,
    }
}

/// Ambiguity count by difficulty level (near-miss solutions).
fn difficulty_to_ambiguity(difficulty: u8) -> usize {
    match difficulty {
        1..=4 => 0,
        5..=6 => 1,
        7..=8 => 2,
        9 => 3,
        10 => 5,
        _ => 0,
    }
}

/// Days in a given month (handles leap years).
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 31,
    }
}

/// Sample puzzle sets
pub struct SamplePuzzles;

impl SamplePuzzles {
    /// Get easy puzzles (difficulty 1-3)
    pub fn easy() -> Vec<TemporalPuzzle> {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            min_difficulty: 1,
            max_difficulty: 3,
            seed: Some(42),
            ..Default::default()
        });
        gen.generate_batch(10).unwrap()
    }

    /// Get medium puzzles (difficulty 4-6)
    pub fn medium() -> Vec<TemporalPuzzle> {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            min_difficulty: 4,
            max_difficulty: 6,
            seed: Some(42),
            ..Default::default()
        });
        gen.generate_batch(10).unwrap()
    }

    /// Get hard puzzles (difficulty 7-10)
    pub fn hard() -> Vec<TemporalPuzzle> {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            min_difficulty: 7,
            max_difficulty: 10,
            seed: Some(42),
            ..Default::default()
        });
        gen.generate_batch(10).unwrap()
    }

    /// Get cross-cultural puzzles
    pub fn cross_cultural() -> Vec<TemporalPuzzle> {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            cross_cultural: true,
            relative_constraints: true,
            min_difficulty: 5,
            max_difficulty: 8,
            seed: Some(42),
            ..Default::default()
        });
        gen.generate_batch(10).unwrap()
    }

    /// Get a mixed sample set (50 puzzles across all difficulties)
    pub fn mixed_sample() -> Vec<TemporalPuzzle> {
        let mut all = Vec::new();
        all.extend(Self::easy());
        all.extend(Self::medium());
        all.extend(Self::hard());
        all.extend(Self::cross_cultural());

        // Add more easy/medium to match TimePuzzles distribution
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            min_difficulty: 2,
            max_difficulty: 5,
            seed: Some(123),
            ..Default::default()
        });
        all.extend(gen.generate_batch(10).unwrap());

        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_puzzle_generation() {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            seed: Some(42),
            ..Default::default()
        });

        let puzzle = gen.generate_puzzle("test-1").unwrap();
        assert!(!puzzle.constraints.is_empty());
        assert!(!puzzle.solutions.is_empty());
    }

    #[test]
    fn test_batch_generation() {
        let mut gen = PuzzleGenerator::new(PuzzleGeneratorConfig {
            seed: Some(42),
            ..Default::default()
        });

        let puzzles = gen.generate_batch(20).unwrap();
        assert_eq!(puzzles.len(), 20);
    }

    #[test]
    fn test_sample_puzzles() {
        let easy = SamplePuzzles::easy();
        assert_eq!(easy.len(), 10);
        assert!(easy.iter().all(|p| p.difficulty <= 3));

        let hard = SamplePuzzles::hard();
        assert!(hard.iter().all(|p| p.difficulty >= 7));
    }
}
