//! CLI for Ruvector vector database

use clap::{Parser, Subcommand};
use colored::*;
use ruvector_router_core::{DistanceMetric, SearchQuery, VectorDB, VectorEntry};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "ruvector")]
#[command(about = "High-performance vector database CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new vector database
    Create {
        /// Path to database
        #[arg(short, long, default_value = "./vectors.db")]
        path: String,

        /// Vector dimensions
        #[arg(short, long, default_value_t = 384)]
        dimensions: usize,

        /// Distance metric
        #[arg(short = 'm', long, default_value = "cosine")]
        metric: String,
    },

    /// Insert a vector
    Insert {
        /// Path to database
        #[arg(short, long, default_value = "./vectors.db")]
        path: String,

        /// Vector ID
        #[arg(short, long)]
        id: String,

        /// Vector values (comma-separated)
        #[arg(short, long)]
        vector: String,
    },

    /// Search for similar vectors
    Search {
        /// Path to database
        #[arg(short, long, default_value = "./vectors.db")]
        path: String,

        /// Query vector (comma-separated)
        #[arg(short, long)]
        vector: String,

        /// Number of results
        #[arg(short, long, default_value_t = 10)]
        k: usize,
    },

    /// Show database statistics
    Stats {
        /// Path to database
        #[arg(short, long, default_value = "./vectors.db")]
        path: String,
    },

    /// Benchmark performance
    Benchmark {
        /// Path to database
        #[arg(short, long, default_value = "./vectors.db")]
        path: String,

        /// Number of vectors to test
        #[arg(short, long, default_value_t = 1000)]
        num_vectors: usize,

        /// Vector dimensions
        #[arg(short, long, default_value_t = 384)]
        dimensions: usize,
    },
}

fn parse_vector(s: &str) -> Result<Vec<f32>, String> {
    s.split(',')
        .map(|v| {
            v.trim()
                .parse::<f32>()
                .map_err(|e| format!("Failed to parse vector: {}", e))
        })
        .collect()
}

fn parse_metric(s: &str) -> DistanceMetric {
    match s.to_lowercase().as_str() {
        "euclidean" | "l2" => DistanceMetric::Euclidean,
        "cosine" => DistanceMetric::Cosine,
        "dot" | "dotproduct" => DistanceMetric::DotProduct,
        "manhattan" | "l1" => DistanceMetric::Manhattan,
        _ => DistanceMetric::Cosine,
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Create {
            path,
            dimensions,
            metric,
        } => {
            println!(
                "{} Creating database at {}...",
                "→".green().bold(),
                path.cyan()
            );

            let _db = VectorDB::builder()
                .dimensions(dimensions)
                .storage_path(&path)
                .distance_metric(parse_metric(&metric))
                .build()?;

            println!("{} Database created successfully!", "✓".green().bold());
            println!("  Dimensions: {}", dimensions);
            println!("  Metric: {}", metric);
        }

        Commands::Insert { path, id, vector } => {
            println!("{} Opening database...", "→".green().bold());

            let vector_data = parse_vector(&vector).map_err(|e| anyhow::anyhow!(e))?;

            let dimensions = vector_data.len();

            let _db = VectorDB::builder()
                .dimensions(dimensions)
                .storage_path(&path)
                .build()?;

            let entry = VectorEntry {
                id: id.clone(),
                vector: vector_data,
                metadata: HashMap::new(),
                timestamp: chrono::Utc::now().timestamp(),
            };

            let start = Instant::now();
            _db.insert(entry)?;
            let elapsed = start.elapsed();

            println!("{} Vector inserted successfully!", "✓".green().bold());
            println!("  ID: {}", id.cyan());
            println!("  Time: {:?}", elapsed);
        }

        Commands::Search { path, vector, k } => {
            println!("{} Opening database...", "→".green().bold());

            let vector_data = parse_vector(&vector).map_err(|e| anyhow::anyhow!(e))?;

            let dimensions = vector_data.len();

            let _db = VectorDB::builder()
                .dimensions(dimensions)
                .storage_path(&path)
                .build()?;

            let query = SearchQuery {
                vector: vector_data,
                k,
                filters: None,
                threshold: None,
                ef_search: None,
            };

            let start = Instant::now();
            let results = _db.search(query)?;
            let elapsed = start.elapsed();

            println!("{} Found {} results", "✓".green().bold(), results.len());
            println!("  Query time: {:?}", elapsed);
            println!();

            for (i, result) in results.iter().enumerate() {
                println!(
                    "{}. {} (score: {:.4})",
                    i + 1,
                    result.id.cyan(),
                    result.score
                );
            }
        }

        Commands::Stats { path } => {
            println!("{} Opening database...", "→".green().bold());

            let _db = VectorDB::builder()
                .dimensions(384) // Default, actual doesn't matter for stats
                .storage_path(&path)
                .build()?;

            let stats = _db.stats();
            let count = _db.count()?;

            println!("{} Database Statistics", "✓".green().bold());
            println!();
            println!("  Total vectors: {}", count.to_string().cyan());
            println!(
                "  Average query latency: {:.2} μs",
                stats.avg_query_latency_us
            );
            println!("  QPS: {:.2}", stats.qps);
            println!(
                "  Index size: {} bytes",
                stats.index_size_bytes.to_string().cyan()
            );
        }

        Commands::Benchmark {
            path,
            num_vectors,
            dimensions,
        } => {
            println!("{} Running benchmark...", "→".green().bold());
            println!("  Vectors: {}", num_vectors);
            println!("  Dimensions: {}", dimensions);
            println!();

            let _db = VectorDB::builder()
                .dimensions(dimensions)
                .storage_path(&path)
                .build()?;

            // Generate random vectors
            use rand::Rng;
            let mut rng = rand::thread_rng();

            println!("{} Generating vectors...", "→".yellow());

            let vectors: Vec<VectorEntry> = (0..num_vectors)
                .map(|i| VectorEntry {
                    id: format!("vec_{}", i),
                    vector: (0..dimensions).map(|_| rng.gen::<f32>()).collect(),
                    metadata: HashMap::new(),
                    timestamp: chrono::Utc::now().timestamp(),
                })
                .collect();

            println!("{} Inserting vectors...", "→".yellow());

            let start = Instant::now();
            _db.insert_batch(vectors)?;
            let insert_time = start.elapsed();

            println!(
                "{} Inserted {} vectors in {:?}",
                "✓".green().bold(),
                num_vectors,
                insert_time
            );
            println!(
                "  Throughput: {:.0} inserts/sec",
                num_vectors as f64 / insert_time.as_secs_f64()
            );
            println!();

            // Benchmark search
            println!("{} Running search benchmark...", "→".yellow());

            let num_queries = 100;
            let query_vector: Vec<f32> = (0..dimensions).map(|_| rng.gen::<f32>()).collect();

            let start = Instant::now();
            for _ in 0..num_queries {
                let query = SearchQuery {
                    vector: query_vector.clone(),
                    k: 10,
                    filters: None,
                    threshold: None,
                    ef_search: None,
                };
                _db.search(query)?;
            }
            let search_time = start.elapsed();

            let avg_latency = search_time / num_queries;
            let qps = num_queries as f64 / search_time.as_secs_f64();

            println!(
                "{} Completed {} queries in {:?}",
                "✓".green().bold(),
                num_queries,
                search_time
            );
            println!("  Average latency: {:?}", avg_latency);
            println!("  QPS: {:.0}", qps);
        }
    }

    Ok(())
}
