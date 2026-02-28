use clap::{Parser, Subcommand};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod commands;
mod config;
mod output;
mod server;

#[derive(Parser)]
#[command(name = "ruvector-attention")]
#[command(author = "rUv <ruv@ruv.io>")]
#[command(version)]
#[command(about = "High-performance attention mechanisms CLI", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<std::path::PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, global = true, default_value = "info")]
    log_level: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Compute attention over input data
    Compute(commands::compute::ComputeArgs),

    /// Run performance benchmarks
    Benchmark(commands::benchmark::BenchmarkArgs),

    /// Convert between data formats
    Convert(commands::convert::ConvertArgs),

    /// Start HTTP server
    Serve(commands::serve::ServeArgs),

    /// Interactive REPL
    Repl(commands::repl::ReplArgs),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false)
                .with_file(true)
                .with_line_number(true)
        )
        .with(tracing_subscriber::EnvFilter::new(&cli.log_level))
        .init();

    // Load configuration
    let config = config::load_config(cli.config.as_deref())?;

    // Execute command
    match cli.command {
        Commands::Compute(args) => commands::compute::run(args, &config).await,
        Commands::Benchmark(args) => commands::benchmark::run(args, &config).await,
        Commands::Convert(args) => commands::convert::run(args, &config),
        Commands::Serve(args) => commands::serve::run(args, &config).await,
        Commands::Repl(args) => commands::repl::run(args, &config).await,
    }
}
