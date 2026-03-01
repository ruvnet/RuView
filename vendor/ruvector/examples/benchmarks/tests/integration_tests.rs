//! Integration tests for benchmark suite

use chrono::{NaiveDate, Weekday};
use ruvector_benchmarks::{
    logging::BenchmarkLogger,
    swarm_regret::{EpisodeResult, RegretTracker, SwarmController},
    temporal::{TemporalConstraint, TemporalPuzzle, TemporalSolver},
    timepuzzles::{PuzzleGenerator, PuzzleGeneratorConfig, SamplePuzzles},
    vector_index::{CoherenceGate, DenseVec, IvfConfig, VectorIndex},
};
use tempfile::tempdir;

// ============================================================================
// Vector Index Tests
// ============================================================================

#[test]
fn test_vector_index_insert_search() {
    let mut idx = VectorIndex::new(4);

    let id1 = idx.insert(DenseVec::new(vec![1.0, 0.0, 0.0, 0.0])).unwrap();
    let id2 = idx.insert(DenseVec::new(vec![0.9, 0.1, 0.0, 0.0])).unwrap();
    let _id3 = idx.insert(DenseVec::new(vec![0.0, 1.0, 0.0, 0.0])).unwrap();

    let q = DenseVec::new(vec![1.0, 0.0, 0.0, 0.0]);
    let results = idx.search(&q, 2, 1.0).unwrap();

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].id, id1);
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_vector_index_coherence_gate() {
    let gate = CoherenceGate::new(0.5);
    let mut idx = VectorIndex::new(4).with_gate(gate);

    idx.insert(DenseVec::new(vec![1.0, 0.0, 0.0, 0.0])).unwrap();
    idx.insert(DenseVec::new(vec![0.0, 1.0, 0.0, 0.0])).unwrap();

    let q = DenseVec::new(vec![1.0, 0.0, 0.0, 0.0]);

    // Low coherence - blocked
    let results = idx.search(&q, 10, 0.3).unwrap();
    assert!(results.is_empty());

    // High coherence - allowed
    let results = idx.search(&q, 10, 0.7).unwrap();
    assert!(!results.is_empty());
}

#[test]
fn test_vector_index_ivf() {
    let ivf = IvfConfig::new(4, 2);
    let mut idx = VectorIndex::new(8).with_ivf(ivf);

    // Insert enough vectors for clustering
    for _ in 0..100 {
        idx.insert(DenseVec::random(8)).unwrap();
    }

    idx.rebuild_ivf().unwrap();

    let stats = idx.stats();
    assert!(stats.ivf_enabled);
    assert!(stats.ivf_clusters > 0);

    // Search should work
    let q = DenseVec::random(8);
    let results = idx.search(&q, 5, 1.0).unwrap();
    assert!(results.len() <= 5);
}

#[test]
fn test_vector_index_persistence() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test_index.bin");

    let mut idx = VectorIndex::new(4);
    idx.insert(DenseVec::new(vec![1.0, 2.0, 3.0, 4.0])).unwrap();
    idx.insert(DenseVec::new(vec![5.0, 6.0, 7.0, 8.0])).unwrap();

    idx.save_to_file(&path).unwrap();

    let loaded = VectorIndex::load_from_file(&path).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded.dim(), 4);
}

// ============================================================================
// Temporal Reasoning Tests
// ============================================================================

#[test]
fn test_temporal_puzzle_exact_date() {
    let target = NaiveDate::from_ymd_opt(2024, 6, 15).unwrap();
    let puzzle = TemporalPuzzle::new("test", "Find June 15, 2024")
        .with_constraint(TemporalConstraint::Exact(target))
        .with_solutions(vec![target]);

    assert!(puzzle.check_date(target).unwrap());
    assert!(!puzzle
        .check_date(NaiveDate::from_ymd_opt(2024, 6, 14).unwrap())
        .unwrap());
}

#[test]
fn test_temporal_puzzle_range() {
    let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    let end = NaiveDate::from_ymd_opt(2024, 1, 31).unwrap();

    let puzzle = TemporalPuzzle::new("test", "Find a date in January 2024")
        .with_constraint(TemporalConstraint::Between(start, end));

    assert!(puzzle
        .check_date(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        .unwrap());
    assert!(!puzzle
        .check_date(NaiveDate::from_ymd_opt(2024, 2, 1).unwrap())
        .unwrap());
}

#[test]
fn test_temporal_puzzle_day_of_week() {
    let puzzle = TemporalPuzzle::new("test", "Find a Monday in 2024")
        .with_constraint(TemporalConstraint::InYear(2024))
        .with_constraint(TemporalConstraint::DayOfWeek(Weekday::Mon));

    // Jan 1, 2024 is a Monday
    assert!(puzzle
        .check_date(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap())
        .unwrap());
    // Jan 2, 2024 is a Tuesday
    assert!(!puzzle
        .check_date(NaiveDate::from_ymd_opt(2024, 1, 2).unwrap())
        .unwrap());
}

#[test]
fn test_temporal_puzzle_relative() {
    let base = NaiveDate::from_ymd_opt(2024, 3, 1).unwrap();
    let puzzle = TemporalPuzzle::new("test", "Find 10 days after base")
        .with_reference("base", base)
        .with_constraint(TemporalConstraint::DaysAfter("base".to_string(), 10));

    let target = NaiveDate::from_ymd_opt(2024, 3, 11).unwrap();
    assert!(puzzle.check_date(target).unwrap());
}

#[test]
fn test_temporal_solver_basic() {
    let target = NaiveDate::from_ymd_opt(2024, 5, 20).unwrap();
    let puzzle = TemporalPuzzle::new("test", "Simple puzzle")
        .with_constraint(TemporalConstraint::Exact(target))
        .with_solutions(vec![target]);

    let mut solver = TemporalSolver::with_tools(true, false);
    let result = solver.solve(&puzzle).unwrap();

    assert!(result.solved);
    assert!(result.correct);
}

#[test]
fn test_temporal_solver_with_rewriting() {
    let base = NaiveDate::from_ymd_opt(2024, 7, 4).unwrap();
    let target = NaiveDate::from_ymd_opt(2024, 7, 14).unwrap();

    let puzzle = TemporalPuzzle::new("test", "Relative puzzle")
        .with_reference("event", base)
        .with_constraint(TemporalConstraint::DaysAfter("event".to_string(), 10))
        .with_solutions(vec![target]);

    let mut solver = TemporalSolver::with_tools(true, false);
    let result = solver.solve(&puzzle).unwrap();

    assert!(result.solved);
    assert!(result.correct);
    assert!(result.tool_calls > 0); // Rewriting used
}

// ============================================================================
// TimePuzzles Generator Tests
// ============================================================================

#[test]
fn test_puzzle_generator_basic() {
    let config = PuzzleGeneratorConfig {
        seed: Some(42),
        ..Default::default()
    };

    let mut gen = PuzzleGenerator::new(config);
    let puzzle = gen.generate_puzzle("test-1").unwrap();

    assert!(!puzzle.constraints.is_empty());
    assert!(!puzzle.solutions.is_empty());
    assert!(puzzle.difficulty >= 1 && puzzle.difficulty <= 10);
}

#[test]
fn test_puzzle_generator_batch() {
    let config = PuzzleGeneratorConfig {
        seed: Some(42),
        ..Default::default()
    };

    let mut gen = PuzzleGenerator::new(config);
    let puzzles = gen.generate_batch(20).unwrap();

    assert_eq!(puzzles.len(), 20);

    // All puzzles should be valid
    for puzzle in &puzzles {
        assert!(!puzzle.constraints.is_empty());
        assert!(!puzzle.solutions.is_empty());
    }
}

#[test]
fn test_puzzle_generator_difficulty() {
    let config = PuzzleGeneratorConfig {
        min_difficulty: 7,
        max_difficulty: 10,
        seed: Some(42),
        ..Default::default()
    };

    let mut gen = PuzzleGenerator::new(config);
    let puzzles = gen.generate_batch(10).unwrap();

    for puzzle in &puzzles {
        assert!(puzzle.difficulty >= 7);
        assert!(puzzle.difficulty <= 10);
    }
}

#[test]
fn test_sample_puzzles() {
    let easy = SamplePuzzles::easy();
    assert_eq!(easy.len(), 10);
    assert!(easy.iter().all(|p| p.difficulty <= 3));

    let medium = SamplePuzzles::medium();
    assert!(medium
        .iter()
        .all(|p| p.difficulty >= 4 && p.difficulty <= 6));

    let hard = SamplePuzzles::hard();
    assert!(hard.iter().all(|p| p.difficulty >= 7));

    let mixed = SamplePuzzles::mixed_sample();
    assert!(mixed.len() >= 40);
}

// ============================================================================
// Swarm Regret Tests
// ============================================================================

#[test]
fn test_regret_tracker_basic() {
    let mut tracker = RegretTracker::new(10);

    let result = EpisodeResult {
        episode: 1,
        num_tasks: 20,
        solved: 18,
        correct: 17,
        total_steps: 100,
        tool_calls: 20,
        latency_ms: 1000,
        reward: 80.0,
        oracle_reward: 99.0,
    };

    tracker.record_episode(result);

    assert_eq!(tracker.episodes.len(), 1);
    assert!((tracker.current_cumulative_regret() - 19.0).abs() < 0.01);
}

#[test]
fn test_regret_tracker_sublinear() {
    let mut tracker = RegretTracker::new(10);

    // Simulate improving performance (decreasing regret)
    for i in 0..10 {
        let accuracy = 0.5 + 0.05 * i as f64;
        let result = EpisodeResult {
            episode: i + 1,
            num_tasks: 20,
            solved: (20.0 * accuracy) as usize,
            correct: (20.0 * accuracy) as usize,
            total_steps: 100 - i * 5,
            tool_calls: 20,
            latency_ms: 1000,
            reward: accuracy * 100.0 - (100 - i * 5) as f64 * 0.1,
            oracle_reward: 99.0,
        };
        tracker.record_episode(result);
    }

    // Average regret should be decreasing
    assert!(tracker.is_sublinear());
    assert!(tracker.regret_trend() < 0.0);
}

#[test]
fn test_swarm_controller() {
    let mut controller = SwarmController::new(20);

    // Run a few episodes
    for _ in 0..5 {
        controller.start_episode();
        controller.complete_episode(18, 17, 80, 20, 500);
    }

    let status = controller.status();
    assert_eq!(status.episode, 5);
    assert!(status.accuracy > 0.8);
}

// ============================================================================
// Logging Tests
// ============================================================================

#[test]
fn test_benchmark_logger() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("test.log");

    let mut logger = BenchmarkLogger::new(path.to_str().unwrap()).unwrap();

    logger
        .log_temporal(
            "bench-1", "puzzle-1", 5, true, true, 10, 2, 100, 3, true, false,
        )
        .unwrap();

    logger
        .log_vector("search", 128, 10000, 1, 10, true, 0.9, 500, 10)
        .unwrap();

    logger
        .log_swarm(1, 20, 18, 17, 85.0, 99.0, 14.0, 14.0, true)
        .unwrap();

    logger.flush().unwrap();

    // Read back
    let reader = ruvector_benchmarks::logging::LogReader::new(path.to_str().unwrap());
    let entries = reader.read_all().unwrap();
    assert_eq!(entries.len(), 3);
}

// ============================================================================
// End-to-End Tests
// ============================================================================

#[test]
fn test_full_benchmark_workflow() {
    // Generate puzzles
    let config = PuzzleGeneratorConfig {
        min_difficulty: 2,
        max_difficulty: 5,
        seed: Some(12345),
        ..Default::default()
    };

    let mut gen = PuzzleGenerator::new(config);
    let puzzles = gen.generate_batch(10).unwrap();

    // Create solver (budget must cover wider posterior-based ranges)
    let mut solver = TemporalSolver::with_tools(true, false);
    solver.max_steps = 400;

    // Run all puzzles
    let mut results = Vec::new();
    for puzzle in &puzzles {
        let result = solver.solve(puzzle).unwrap();
        results.push(result);
    }

    // Check results
    let solved = results.iter().filter(|r| r.solved).count();
    let correct = results.iter().filter(|r| r.correct).count();

    // Should solve most easy-medium puzzles
    assert!(solved >= 5);
    assert!(correct >= 5);
}

#[test]
fn test_vector_temporal_integration() {
    // This tests using vector index to store temporal embeddings
    let mut idx = VectorIndex::new(64);

    // Create "embeddings" for dates (simplified)
    for day in 1..=31 {
        let mut values = vec![0.0f32; 64];
        values[0] = day as f32 / 31.0; // Day component
        values[1] = 1.0 / 12.0; // Month component (January)
        values[2] = 2024.0 / 3000.0; // Year component
        idx.insert(DenseVec::new(values)).unwrap();
    }

    // Search for similar dates
    let mut query = vec![0.0f32; 64];
    query[0] = 15.0 / 31.0; // Looking for mid-month
    query[1] = 1.0 / 12.0;
    query[2] = 2024.0 / 3000.0;

    let results = idx.search(&DenseVec::new(query), 5, 1.0).unwrap();

    // Should find dates near the 15th
    assert!(!results.is_empty());
}
