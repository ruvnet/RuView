//! RuVector Cloud Run GPU Benchmark Suite with Self-Learning Models
//!
//! High-performance benchmarks for vector operations on Cloud Run with GPU support.
//! Includes self-learning models for various industries using RuVector's GNN, Attention, and Graph crates.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod benchmark;
mod cuda;
mod report;
mod self_learning;
mod server;
mod simd;

#[derive(Parser)]
#[command(name = "ruvector-gpu-benchmark")]
#[command(about = "RuVector Cloud Run GPU Benchmark Suite")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run quick benchmark (single configuration)
    Quick {
        /// Vector dimensions
        #[arg(short, long, default_value = "128")]
        dims: usize,

        /// Number of vectors
        #[arg(short, long, default_value = "10000")]
        num_vectors: usize,

        /// Number of queries
        #[arg(short, long, default_value = "1000")]
        num_queries: usize,

        /// Output file path
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Enable GPU acceleration
        #[arg(long, default_value = "true")]
        gpu: bool,
    },

    /// Run full benchmark suite
    Full {
        /// Output directory
        #[arg(short, long, default_value = "./benchmark_results")]
        output_dir: PathBuf,

        /// Benchmark sizes: small, medium, large, xlarge
        #[arg(short, long, default_value = "small,medium,large")]
        sizes: String,

        /// Vector dimensions to test
        #[arg(long, default_value = "128,256,512,768,1024,1536")]
        dims: String,

        /// Enable GPU acceleration
        #[arg(long, default_value = "true")]
        gpu: bool,
    },

    /// Run distance computation benchmarks
    Distance {
        /// Vector dimensions
        #[arg(short, long, default_value = "128")]
        dims: usize,

        /// Batch size
        #[arg(short, long, default_value = "64")]
        batch_size: usize,

        /// Number of vectors in database
        #[arg(short, long, default_value = "100000")]
        num_vectors: usize,

        /// Number of iterations
        #[arg(short, long, default_value = "100")]
        iterations: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run GNN benchmarks
    Gnn {
        /// Number of graph nodes
        #[arg(long, default_value = "10000")]
        num_nodes: usize,

        /// Number of graph edges
        #[arg(long, default_value = "50000")]
        num_edges: usize,

        /// Feature dimensions
        #[arg(short, long, default_value = "256")]
        dims: usize,

        /// Number of GNN layers
        #[arg(short, long, default_value = "3")]
        layers: usize,

        /// Number of iterations
        #[arg(short, long, default_value = "50")]
        iterations: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run HNSW index benchmarks
    Hnsw {
        /// Vector dimensions
        #[arg(short, long, default_value = "128")]
        dims: usize,

        /// Number of vectors
        #[arg(short, long, default_value = "100000")]
        num_vectors: usize,

        /// ef_construction parameter
        #[arg(long, default_value = "200")]
        ef_construction: usize,

        /// ef_search parameter
        #[arg(long, default_value = "100")]
        ef_search: usize,

        /// k nearest neighbors
        #[arg(short, long, default_value = "10")]
        k: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run quantization benchmarks
    Quantization {
        /// Vector dimensions
        #[arg(short, long, default_value = "128")]
        dims: usize,

        /// Number of vectors
        #[arg(short, long, default_value = "100000")]
        num_vectors: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run CUDA kernel benchmarks (GPU only)
    Cuda {
        /// Number of iterations
        #[arg(short, long, default_value = "100")]
        iterations: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Run TPU benchmarks (Google Cloud TPU)
    Tpu {
        /// Number of iterations
        #[arg(short, long, default_value = "50")]
        iterations: usize,

        /// Output file
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Train self-learning industry models
    Train {
        /// Number of training epochs
        #[arg(short, long, default_value = "50")]
        epochs: usize,

        /// Output directory for trained models
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },

    /// Run exotic research experiments
    Exotic {
        /// Number of iterations
        #[arg(short, long, default_value = "500")]
        iterations: usize,

        /// Output directory
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },

    /// Generate report from benchmark results
    Report {
        /// Input directory with benchmark results
        #[arg(short, long)]
        input_dir: PathBuf,

        /// Output file
        #[arg(short, long)]
        output: PathBuf,

        /// Output format: json, csv, html, markdown
        #[arg(short, long, default_value = "html")]
        format: String,
    },

    /// Start HTTP server for Cloud Run
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "8080")]
        port: u16,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("ruvector=info".parse()?)
                .add_directive("gpu_benchmark=info".parse()?),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Quick {
            dims,
            num_vectors,
            num_queries,
            output,
            gpu,
        } => {
            benchmark::run_quick(dims, num_vectors, num_queries, output, gpu).await?;
        }

        Commands::Full {
            output_dir,
            sizes,
            dims,
            gpu,
        } => {
            let sizes: Vec<&str> = sizes.split(',').collect();
            let dims: Vec<usize> = dims.split(',').map(|s| s.trim().parse().unwrap()).collect();
            benchmark::run_full(&output_dir, &sizes, &dims, gpu).await?;
        }

        Commands::Distance {
            dims,
            batch_size,
            num_vectors,
            iterations,
            output,
        } => {
            benchmark::run_distance(dims, batch_size, num_vectors, iterations, output).await?;
        }

        Commands::Gnn {
            num_nodes,
            num_edges,
            dims,
            layers,
            iterations,
            output,
        } => {
            benchmark::run_gnn(num_nodes, num_edges, dims, layers, iterations, output).await?;
        }

        Commands::Hnsw {
            dims,
            num_vectors,
            ef_construction,
            ef_search,
            k,
            output,
        } => {
            benchmark::run_hnsw(dims, num_vectors, ef_construction, ef_search, k, output).await?;
        }

        Commands::Quantization {
            dims,
            num_vectors,
            output,
        } => {
            benchmark::run_quantization(dims, num_vectors, output).await?;
        }

        Commands::Cuda { iterations, output } => {
            cuda::run_cuda_benchmarks(iterations, output).await?;
        }

        Commands::Tpu { iterations, output } => {
            cuda::run_tpu_benchmarks(iterations, output).await?;
        }

        Commands::Train { epochs, output_dir } => {
            self_learning::run_industry_training(epochs, output_dir).await?;
        }

        Commands::Exotic {
            iterations,
            output_dir,
        } => {
            self_learning::run_exotic_experiments(iterations, output_dir).await?;
        }

        Commands::Report {
            input_dir,
            output,
            format,
        } => {
            report::generate_report(&input_dir, &output, &format)?;
        }

        Commands::Serve { port } => {
            server::run_server(port).await?;
        }
    }

    Ok(())
}
