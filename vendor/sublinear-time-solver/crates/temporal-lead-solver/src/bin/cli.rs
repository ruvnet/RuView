//! CLI for Temporal Computational Lead Solver
//!
//! Demonstrates sublinear solving with temporal advantages

use clap::{Parser, Subcommand};
use temporal_lead::*;
use colored::*;
use std::time::Instant;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "temporal-cli")]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze matrix for sublinear solvability
    Analyze {
        /// Matrix size
        #[arg(short, long, default_value_t = 1000)]
        size: usize,

        /// Dominance factor
        #[arg(short, long, default_value_t = 2.0)]
        dominance: f64,
    },

    /// Predict functional with temporal lead
    Predict {
        /// Matrix size
        #[arg(short, long, default_value_t = 1000)]
        size: usize,

        /// Distance in kilometers
        #[arg(short, long, default_value_t = 10900.0)]
        distance: f64,

        /// Error tolerance
        #[arg(short, long, default_value_t = 0.001)]
        epsilon: f64,
    },

    /// Validate mathematical proofs
    Prove {
        /// Theorem to prove
        #[arg(short, long, default_value = "temporal-lead")]
        theorem: String,
    },

    /// Benchmark solver performance
    Benchmark {
        /// Sizes to test
        #[arg(short, long, value_delimiter = ',')]
        sizes: Vec<usize>,
    },

    /// Compare with traditional methods
    Compare {
        /// Matrix size
        #[arg(short, long, default_value_t = 1000)]
        size: usize,
    },
}

fn main() -> Result<()> {
    env_logger::init();
    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { size, dominance } => analyze_matrix(size, dominance)?,
        Commands::Predict { size, distance, epsilon } => predict_functional(size, distance, epsilon)?,
        Commands::Prove { theorem } => prove_theorem(&theorem)?,
        Commands::Benchmark { sizes } => run_benchmarks(&sizes)?,
        Commands::Compare { size } => compare_methods(size)?,
    }

    Ok(())
}

fn analyze_matrix(size: usize, dominance: f64) -> Result<()> {
    println!("{}", "Matrix Analysis".bold().blue());
    println!("{}", "─".repeat(50));

    let matrix = Matrix::diagonally_dominant(size, dominance);
    let params = DominanceParameters::from_matrix(&matrix);

    println!("Size: {}×{}", size, size);
    println!("Dominance factor δ: {:.3}", params.delta);
    println!("Max p-norm gap: {:.3}", params.max_p_norm_gap);
    println!("Condition number κ: {:.2e}", params.condition_number);
    println!("Sparsity: {:.1}%", params.sparsity * 100.0);

    if params.allows_sublinear() {
        println!("{}", "✓ Allows sublinear solving".green());

        let eps_values = vec![1e-3, 1e-6, 1e-9];
        println!("\n{}", "Query Complexity:".bold());
        for eps in eps_values {
            let queries = params.query_complexity(eps);
            let time_ns = params.time_complexity_ns(eps, size);
            println!(
                "  ε={:e}: {} queries, ~{:.3}μs",
                eps,
                queries,
                time_ns as f64 / 1000.0
            );
        }
    } else {
        println!("{}", "✗ Does not allow sublinear solving".red());
        println!("Reason: δ={:.3}, gap={:.3}", params.delta, params.max_p_norm_gap);
    }

    // Check lower bounds
    let sqrt_n = (size as f64).sqrt();
    println!("\n{}", "Lower Bounds:".bold());
    println!("  √n = {:.0} (information theoretic)", sqrt_n);
    println!("  1/δ = {:.1} (dominance limited)", 1.0 / params.delta);

    Ok(())
}

fn predict_functional(size: usize, distance_km: f64, epsilon: f64) -> Result<()> {
    println!("{}", "Temporal Prediction".bold().cyan());
    println!("{}", "─".repeat(50));

    let distance = Distance::kilometers(distance_km);
    let matrix = Matrix::diagonally_dominant(size, 3.0);
    let b = Vector::ones(size);
    let target = Vector::random(size);

    let predictor = TemporalPredictor::new(distance).with_epsilon(epsilon);

    println!("Setup:");
    println!("  Distance: {:.0} km", distance_km);
    println!("  Light travel time: {:.1} ms", distance.light_travel_time_ms());
    println!("  Matrix: {}×{}", size, size);
    println!("  Target ε: {:e}", epsilon);

    let start = Instant::now();
    let result = predictor.predict_functional(&matrix, &b, &target)?;
    let total_time = start.elapsed();

    println!("\n{}", "Results:".bold());
    println!("  Functional value: {:.6} ± {:.6}",
        result.functional_value, result.error_bound);
    println!("  Computation time: {:.3} μs",
        result.computation_time.as_secs_f64() * 1e6);
    println!("  Queries made: {}", result.queries);

    if result.has_temporal_lead() {
        println!("\n{}", "✓ TEMPORAL LEAD ACHIEVED".green().bold());
        println!("  Advantage: {:.1} ms", result.temporal_advantage_ms());
        println!("  Effective velocity: {:.0}× speed of light",
            result.temporal_advantage.effective_velocity_ratio);
    } else {
        println!("\n{}", "No temporal lead".yellow());
    }

    // Validate causality
    let (valid, msg) = predictor.validate_causality(&result);
    println!("\n{}", "Causality Check:".bold());
    println!("  {}", if valid { "✓".green() } else { "✗".red() });
    println!("  {}", msg);

    println!("\nTotal time: {:.3} ms", total_time.as_secs_f64() * 1000.0);

    Ok(())
}

fn prove_theorem(theorem_name: &str) -> Result<()> {
    println!("{}", "Mathematical Proof".bold().magenta());
    println!("{}", "─".repeat(50));

    let proof = match theorem_name {
        "temporal-lead" => TheoremProver::prove_temporal_lead_theorem(),
        "lower-bounds" => TheoremProver::prove_lower_bounds(),
        _ => {
            println!("Unknown theorem: {}", theorem_name);
            return Ok(());
        }
    };

    println!("{}", proof.theorem.bold());
    println!("\n{}", "Assumptions:".underline());
    for assumption in &proof.assumptions {
        println!("  • {}", assumption);
    }

    println!("\n{}", "Proof Steps:".underline());
    for (i, step) in proof.steps.iter().enumerate() {
        println!("\n{}. {}", i + 1, step.description.bold());
        println!("   {}", step.justification.italic());
        if let Some(eq) = &step.equation {
            println!("   {}", eq.cyan());
        }
    }

    println!("\n{}", "Conclusion:".underline());
    println!("{}", proof.conclusion);

    println!("\n{}", "References:".underline());
    for reference in &proof.references {
        println!("  • {}", reference);
    }

    Ok(())
}

fn run_benchmarks(sizes: &[usize]) -> Result<()> {
    println!("{}", "Performance Benchmarks".bold().yellow());
    println!("{}", "─".repeat(50));

    let test_sizes = if sizes.is_empty() {
        vec![10, 100, 1000, 10000]
    } else {
        sizes.to_vec()
    };

    println!("{:>10} {:>15} {:>15} {:>15} {:>10}",
        "Size", "Sublinear (μs)", "Traditional (μs)", "Speedup", "Complexity");

    for &n in &test_sizes {
        let matrix = Matrix::diagonally_dominant(n, 2.0);
        let b = Vector::ones(n);

        // Sublinear solve
        let solver = SublinearSolver::new();
        let start = Instant::now();
        let result = solver.solve(&matrix, &b)?;
        let sublinear_time = start.elapsed();

        // Traditional estimate (O(n³))
        let traditional_time_us = (n * n * n) as f64 / 1000.0;
        let speedup = traditional_time_us / sublinear_time.as_secs_f64() / 1e6;

        println!("{:>10} {:>15.3} {:>15.0} {:>15.1}x {:>10}",
            n,
            sublinear_time.as_secs_f64() * 1e6,
            traditional_time_us,
            speedup,
            format!("{:?}", result.complexity)
        );
    }

    Ok(())
}

fn compare_methods(size: usize) -> Result<()> {
    println!("{}", "Method Comparison".bold().green());
    println!("{}", "─".repeat(50));

    let matrix = Matrix::diagonally_dominant(size, 2.5);
    let b = Vector::random(size);

    let methods = vec![
        SolverMethod::Neumann,
        SolverMethod::ForwardPush,
        SolverMethod::Bidirectional,
        SolverMethod::Adaptive,
    ];

    println!("{:>15} {:>12} {:>12} {:>12} {:>10}",
        "Method", "Time (μs)", "Iterations", "Residual", "Converged");

    for method in methods {
        let solver = SublinearSolver::with_method(method);
        let start = Instant::now();
        let result = solver.solve(&matrix, &b)?;
        let time = start.elapsed();

        let converged = if result.converged(1e-6) { "✓" } else { "✗" };

        println!("{:>15} {:>12.3} {:>12} {:>12.6} {:>10}",
            format!("{:?}", method),
            time.as_secs_f64() * 1e6,
            result.iterations,
            result.residual,
            converged
        );
    }

    // Show complexity table
    println!("\n{}", "Complexity Classes:".bold());
    let table = TheoremProver::complexity_table();
    for (name, entry) in table.iter() {
        println!("  {}: {} time, {} space",
            name.bold(),
            entry.time.cyan(),
            entry.space.yellow()
        );
    }

    Ok(())
}