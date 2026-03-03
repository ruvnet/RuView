//! Training binary for temporal neural networks

use clap::Parser;
use log::info;
use std::path::PathBuf;
use temporal_neural_net::{
    config::Config,
    data::TimeSeriesData,
    models::{SystemA, SystemB},
    training::Trainer,
    error::Result,
};

#[derive(Parser)]
#[command(name = "train")]
#[command(about = "Train temporal neural network models")]
struct Args {
    /// Configuration file path
    #[arg(short, long)]
    config: PathBuf,

    /// Training data file
    #[arg(short, long)]
    data: PathBuf,

    /// Output directory for trained model
    #[arg(short, long, default_value = "output")]
    output: PathBuf,

    /// Verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    if args.verbose {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug")).init();
    } else {
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    }

    temporal_neural_net::init()?;

    info!("Loading configuration from {:?}", args.config);
    let config = Config::from_file(&args.config)?;

    info!("Loading training data from {:?}", args.data);
    let data = TimeSeriesData::from_csv(&args.data)?;

    info!("Creating temporal splits (70/15/15)");
    let splits = data.temporal_split(0.7, 0.15, 0.15)?;
    splits.validate()?;

    let (train_size, val_size, test_size) = splits.get_sizes();
    info!("Data split sizes - Train: {}, Val: {}, Test: {}", train_size, val_size, test_size);

    // Create output directory
    std::fs::create_dir_all(&args.output)?;

    let mut trainer = Trainer::new(config.training.clone())?;

    match config.system {
        temporal_neural_net::config::SystemConfig::Traditional(_) => {
            info!("Training System A (Traditional)");
            let mut model = SystemA::new(&config.model)?;
            let result = trainer.train_system_a(&mut model, &splits)?;

            info!("Training completed:");
            info!("  - Final loss: {:.6}", result.final_loss);
            info!("  - Best val loss: {:.6}", result.best_val_loss);
            info!("  - Converged: {}", result.converged);
            info!("  - Training time: {:.2}s", result.total_time_seconds);

            // Save model (simplified)
            let model_path = args.output.join("system_a_model.json");
            info!("Saving model to {:?}", model_path);
        }

        temporal_neural_net::config::SystemConfig::TemporalSolver(ref solver_config) => {
            info!("Training System B (Temporal Solver)");
            let mut model = SystemB::new(&config.model, solver_config)?;
            let result = trainer.train_system_b(&mut model, &splits)?;

            info!("Training completed:");
            info!("  - Final loss: {:.6}", result.final_loss);
            info!("  - Best val loss: {:.6}", result.best_val_loss);
            info!("  - Converged: {}", result.converged);
            info!("  - Training time: {:.2}s", result.total_time_seconds);

            // Print System B specific metrics
            if let Some(ref last_metrics) = result.history.metrics.last() {
                if let Some(ref b_metrics) = last_metrics.system_b_metrics {
                    info!("System B metrics:");
                    info!("  - Gate pass rate: {:.3}", b_metrics.gate_pass_rate);
                    info!("  - Avg certificate error: {:.6}", b_metrics.avg_certificate_error);
                    info!("  - Kalman prediction error: {:.6}", b_metrics.kalman_prediction_error);
                }
            }

            // Save model (simplified)
            let model_path = args.output.join("system_b_model.json");
            info!("Saving model to {:?}", model_path);
        }
    }

    // Save training history
    let history_path = args.output.join("training_history.json");
    let history_json = serde_json::to_string_pretty(trainer.get_history())?;
    std::fs::write(&history_path, history_json)?;
    info!("Training history saved to {:?}", history_path);

    info!("Training completed successfully!");
    Ok(())
}