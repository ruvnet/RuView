//! Benchmark for SubpolynomialMinCut
//!
//! Demonstrates subpolynomial update performance.

use ruvector_mincut::subpolynomial::{SubpolyConfig, SubpolynomialMinCut};
use std::time::Instant;

fn main() {
    println!("=== SubpolynomialMinCut Benchmark ===\n");

    // Test different graph sizes
    for &n in &[100, 500, 1000, 5000] {
        benchmark_size(n);
    }

    println!("\n=== Complexity Verification ===\n");
    verify_subpolynomial_complexity();
}

fn benchmark_size(n: usize) {
    println!("Graph size: {} vertices", n);

    let mut mincut = SubpolynomialMinCut::for_size(n);

    // Build a random-ish graph
    let build_start = Instant::now();

    // Path + cross edges for connectivity
    for i in 0..(n as u64 - 1) {
        mincut.insert_edge(i, i + 1, 1.0).unwrap();
    }

    // Add cross edges
    for i in (0..n as u64).step_by(10) {
        let j = (i + n as u64 / 2) % n as u64;
        if i != j {
            let _ = mincut.insert_edge(i, j, 1.0);
        }
    }

    println!("  Build graph: {:?}", build_start.elapsed());

    // Build hierarchy
    let hier_start = Instant::now();
    mincut.build();
    println!("  Build hierarchy: {:?}", hier_start.elapsed());

    let stats = mincut.hierarchy_stats();
    println!(
        "  Levels: {}, Expanders: {}",
        stats.num_levels, stats.total_expanders
    );

    // Benchmark updates
    let num_updates = 100;
    let update_start = Instant::now();

    for i in 0..num_updates {
        let u = (i * 7) as u64 % n as u64;
        let v = (i * 13 + 5) as u64 % n as u64;
        if u != v {
            let _ = mincut.insert_edge(u, v, 0.5);
        }
    }

    let update_time = update_start.elapsed();
    let avg_update_us = update_time.as_micros() as f64 / num_updates as f64;

    println!(
        "  {} updates: {:?} ({:.2} μs/update)",
        num_updates, update_time, avg_update_us
    );
    println!("  Min cut: {:.1}", mincut.min_cut_value());

    let recourse = mincut.recourse_stats();
    println!(
        "  Avg recourse: {:.2}, Is subpolynomial: {}",
        recourse.amortized_recourse(),
        recourse.is_subpolynomial(n)
    );

    println!();
}

fn verify_subpolynomial_complexity() {
    // Compare update time scaling
    let sizes = [100, 200, 400, 800, 1600];
    let mut results = Vec::new();

    for &n in &sizes {
        let mut mincut = SubpolynomialMinCut::for_size(n);

        // Build graph
        for i in 0..(n as u64 - 1) {
            mincut.insert_edge(i, i + 1, 1.0).unwrap();
        }
        for i in (0..n as u64).step_by(5) {
            let j = (i + n as u64 / 3) % n as u64;
            if i != j {
                let _ = mincut.insert_edge(i, j, 1.0);
            }
        }

        mincut.build();

        // Measure updates
        let num_updates = 50;
        let start = Instant::now();

        for i in 0..num_updates {
            let u = (i * 11) as u64 % n as u64;
            let v = (i * 17 + 3) as u64 % n as u64;
            if u != v {
                let _ = mincut.insert_edge(u, v, 0.5);
            }
        }

        let avg_us = start.elapsed().as_micros() as f64 / num_updates as f64;
        results.push((n, avg_us));
    }

    println!("Size\tAvg Update (μs)\tScaling");
    println!("----\t---------------\t-------");

    for i in 0..results.len() {
        let (n, time) = results[i];
        let scaling = if i > 0 {
            let (prev_n, prev_time) = results[i - 1];
            let n_ratio = n as f64 / prev_n as f64;
            let time_ratio = time / prev_time;
            let exponent = time_ratio.log2() / n_ratio.log2();
            format!("n^{:.2}", exponent)
        } else {
            "-".to_string()
        };

        println!("{}\t{:.2}\t\t{}", n, time, scaling);
    }

    // For subpolynomial: exponent should approach 0 as n grows
    let last_ratio = results.last().unwrap().1 / results.first().unwrap().1;
    let size_ratio = sizes.last().unwrap() / sizes.first().unwrap();
    let overall_exponent = last_ratio.log2() / (size_ratio as f64).log2();

    println!("\nOverall scaling: n^{:.2}", overall_exponent);
    println!("For subpolynomial, expect exponent → 0 as n → ∞");
    println!(
        "Current exponent ({:.2}) is {} polynomial",
        overall_exponent,
        if overall_exponent < 0.5 {
            "sub"
        } else {
            "super"
        }
    );
}
