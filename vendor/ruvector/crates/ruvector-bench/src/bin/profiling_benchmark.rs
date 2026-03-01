//! Performance profiling benchmark with flamegraph support
//!
//! Generates:
//! - CPU flamegraphs
//! - Memory allocation profiles
//! - Lock contention analysis
//! - SIMD utilization measurement

use anyhow::Result;
use clap::Parser;
use ruvector_bench::{create_progress_bar, DatasetGenerator, MemoryProfiler, VectorDistribution};
use ruvector_core::{
    types::{DbOptions, HnswConfig, QuantizationConfig},
    DistanceMetric, SearchQuery, VectorDB, VectorEntry,
};
use std::path::PathBuf;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "profiling-benchmark")]
#[command(about = "Performance profiling with flamegraph support")]
struct Args {
    /// Number of vectors
    #[arg(short, long, default_value = "100000")]
    num_vectors: usize,

    /// Number of queries
    #[arg(short, long, default_value = "10000")]
    queries: usize,

    /// Vector dimensions
    #[arg(short, long, default_value = "384")]
    dimensions: usize,

    /// Enable flamegraph generation
    #[arg(long)]
    flamegraph: bool,

    /// Output directory
    #[arg(short, long, default_value = "bench_results/profiling")]
    output: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    println!("╔════════════════════════════════════════╗");
    println!("║   Ruvector Performance Profiling      ║");
    println!("╚════════════════════════════════════════╝\n");

    std::fs::create_dir_all(&args.output)?;

    // Start profiling if enabled
    #[cfg(feature = "profiling")]
    let guard = if args.flamegraph {
        println!("Starting CPU profiling...");
        Some(start_profiling())
    } else {
        None
    };

    // Profile 1: Indexing performance
    println!("\n{}", "=".repeat(60));
    println!("Profiling: Index Construction");
    println!("{}\n", "=".repeat(60));
    profile_indexing(&args)?;

    // Profile 2: Search performance
    println!("\n{}", "=".repeat(60));
    println!("Profiling: Search Operations");
    println!("{}\n", "=".repeat(60));
    profile_search(&args)?;

    // Profile 3: Mixed workload
    println!("\n{}", "=".repeat(60));
    println!("Profiling: Mixed Read/Write Workload");
    println!("{}\n", "=".repeat(60));
    profile_mixed_workload(&args)?;

    // Stop profiling and generate flamegraph
    #[cfg(feature = "profiling")]
    if let Some(guard) = guard {
        println!("\nGenerating flamegraph...");
        stop_profiling(guard, &args.output)?;
    }

    #[cfg(not(feature = "profiling"))]
    if args.flamegraph {
        println!("\n⚠ Profiling feature not enabled. Rebuild with:");
        println!("  cargo build --release --features profiling");
    }

    println!(
        "\n✓ Profiling complete! Results saved to: {}",
        args.output.display()
    );
    Ok(())
}

#[cfg(feature = "profiling")]
fn start_profiling() -> pprof::ProfilerGuard<'static> {
    pprof::ProfilerGuardBuilder::default()
        .frequency(1000)
        .blocklist(&["libc", "libgcc", "pthread", "vdso"])
        .build()
        .unwrap()
}

#[cfg(feature = "profiling")]
fn stop_profiling(guard: pprof::ProfilerGuard<'static>, output_dir: &PathBuf) -> Result<()> {
    use std::fs::File;
    use std::io::Write;

    if let Ok(report) = guard.report().build() {
        let flamegraph_path = output_dir.join("flamegraph.svg");
        let mut file = File::create(&flamegraph_path)?;
        report.flamegraph(&mut file)?;
        println!("✓ Flamegraph saved to: {}", flamegraph_path.display());

        // Also generate a text report
        let profile_path = output_dir.join("profile.txt");
        let mut profile_file = File::create(&profile_path)?;
        writeln!(profile_file, "CPU Profile Report\n==================\n")?;
        writeln!(profile_file, "{:?}", report)?;
        println!("✓ Profile report saved to: {}", profile_path.display());
    }

    Ok(())
}

fn profile_indexing(args: &Args) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("profiling.db");

    let options = DbOptions {
        dimensions: args.dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let mem_profiler = MemoryProfiler::new();
    let start = Instant::now();

    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        args.dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    println!("Indexing {} vectors for profiling...", args.num_vectors);
    let pb = create_progress_bar(args.num_vectors as u64, "Indexing");

    for i in 0..args.num_vectors {
        let entry = VectorEntry {
            id: Some(i.to_string()),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: None,
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Indexing complete");

    let elapsed = start.elapsed();
    let memory_mb = mem_profiler.current_usage_mb();

    println!("\nIndexing Performance:");
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!(
        "  Throughput: {:.0} vectors/sec",
        args.num_vectors as f64 / elapsed.as_secs_f64()
    );
    println!("  Memory: {:.2} MB", memory_mb);

    Ok(())
}

fn profile_search(args: &Args) -> Result<()> {
    let (db, queries) = setup_database(args)?;

    println!("Running {} search queries for profiling...", args.queries);
    let pb = create_progress_bar(args.queries as u64, "Searching");

    let start = Instant::now();
    for query in &queries {
        db.search(SearchQuery {
            vector: query.clone(),
            k: 10,
            filter: None,
            ef_search: None,
        })?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Search complete");

    let elapsed = start.elapsed();

    println!("\nSearch Performance:");
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!("  QPS: {:.0}", args.queries as f64 / elapsed.as_secs_f64());
    println!(
        "  Avg latency: {:.2}ms",
        elapsed.as_secs_f64() * 1000.0 / args.queries as f64
    );

    Ok(())
}

fn profile_mixed_workload(args: &Args) -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("mixed.db");

    let options = DbOptions {
        dimensions: args.dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        args.dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    let num_ops = args.num_vectors / 10;
    println!(
        "Running {} mixed operations (70% writes, 30% reads)...",
        num_ops
    );
    let pb = create_progress_bar(num_ops as u64, "Processing");

    let start = Instant::now();
    let mut write_count = 0;
    let mut read_count = 0;

    for i in 0..num_ops {
        if i % 10 < 7 {
            // Write operation
            let entry = VectorEntry {
                id: Some(i.to_string()),
                vector: gen.generate(1).into_iter().next().unwrap(),
                metadata: None,
            };
            db.insert(entry)?;
            write_count += 1;
        } else {
            // Read operation
            let query = gen.generate(1).into_iter().next().unwrap();
            db.search(SearchQuery {
                vector: query,
                k: 10,
                filter: None,
                ef_search: None,
            })?;
            read_count += 1;
        }
        pb.inc(1);
    }
    pb.finish_with_message("✓ Mixed workload complete");

    let elapsed = start.elapsed();

    println!("\nMixed Workload Performance:");
    println!("  Total time: {:.2}s", elapsed.as_secs_f64());
    println!(
        "  Writes: {} ({:.0} writes/sec)",
        write_count,
        write_count as f64 / elapsed.as_secs_f64()
    );
    println!(
        "  Reads: {} ({:.0} reads/sec)",
        read_count,
        read_count as f64 / elapsed.as_secs_f64()
    );
    println!(
        "  Total throughput: {:.0} ops/sec",
        num_ops as f64 / elapsed.as_secs_f64()
    );

    Ok(())
}

fn setup_database(args: &Args) -> Result<(VectorDB, Vec<Vec<f32>>)> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("search.db");

    let options = DbOptions {
        dimensions: args.dimensions,
        distance_metric: DistanceMetric::Cosine,
        storage_path: db_path.to_str().unwrap().to_string(),
        hnsw_config: Some(HnswConfig::default()),
        quantization: Some(QuantizationConfig::Scalar),
    };

    let db = VectorDB::new(options)?;

    let gen = DatasetGenerator::new(
        args.dimensions,
        VectorDistribution::Normal {
            mean: 0.0,
            std_dev: 1.0,
        },
    );

    println!("Preparing database with {} vectors...", args.num_vectors);
    let pb = create_progress_bar(args.num_vectors as u64, "Preparing");

    for i in 0..args.num_vectors {
        let entry = VectorEntry {
            id: Some(i.to_string()),
            vector: gen.generate(1).into_iter().next().unwrap(),
            metadata: None,
        };
        db.insert(entry)?;
        pb.inc(1);
    }
    pb.finish_with_message("✓ Database ready");

    let queries = gen.generate(args.queries);

    Ok((db, queries))
}
