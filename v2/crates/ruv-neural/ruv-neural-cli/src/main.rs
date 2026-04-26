//! rUv Neural CLI — Brain topology analysis, simulation, and visualization.

mod commands;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "ruv-neural")]
#[command(about = "rUv Neural — Brain Topology Analysis System")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
}

#[derive(Subcommand)]
enum Commands {
    /// Simulate neural sensor data
    Simulate {
        /// Number of channels
        #[arg(short, long, default_value = "64")]
        channels: usize,
        /// Duration in seconds
        #[arg(short, long, default_value = "10.0")]
        duration: f64,
        /// Sample rate in Hz
        #[arg(short, long, default_value = "1000.0")]
        sample_rate: f64,
        /// Output file (JSON)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Analyze a brain connectivity graph
    Analyze {
        /// Input graph file (JSON)
        #[arg(short, long)]
        input: String,
        /// Show ASCII visualization
        #[arg(long)]
        ascii: bool,
        /// Export metrics to CSV
        #[arg(long)]
        csv: Option<String>,
    },
    /// Compute minimum cut on brain graph
    Mincut {
        /// Input graph file (JSON)
        #[arg(short, long)]
        input: String,
        /// Multi-way cut with k partitions
        #[arg(short, long)]
        k: Option<usize>,
    },
    /// Run full pipeline: simulate -> process -> analyze -> decode
    Pipeline {
        /// Number of channels
        #[arg(short, long, default_value = "32")]
        channels: usize,
        /// Duration in seconds
        #[arg(short, long, default_value = "5.0")]
        duration: f64,
        /// Show real-time ASCII dashboard
        #[arg(long)]
        dashboard: bool,
    },
    /// Export brain graph to visualization format
    Export {
        /// Input graph file (JSON)
        #[arg(short, long)]
        input: String,
        /// Output format: d3, dot, gexf, csv, rvf
        #[arg(short, long, default_value = "d3")]
        format: String,
        /// Output file
        #[arg(short, long)]
        output: String,
    },
    /// Show system info and capabilities
    Info,
    /// Generate or verify Ed25519-signed capability witness bundles
    Witness {
        /// Output file path for generated witness bundle (JSON)
        #[arg(short, long)]
        output: Option<String>,
        /// Path to a witness bundle to verify
        #[arg(long)]
        verify: Option<String>,
    },
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => tracing::Level::WARN,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    tracing_subscriber::fmt()
        .with_max_level(level)
        .with_target(false)
        .init();
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    let result = match cli.command {
        Commands::Simulate {
            channels,
            duration,
            sample_rate,
            output,
        } => commands::simulate::run(channels, duration, sample_rate, output),
        Commands::Analyze { input, ascii, csv } => commands::analyze::run(&input, ascii, csv),
        Commands::Mincut { input, k } => commands::mincut::run(&input, k),
        Commands::Pipeline {
            channels,
            duration,
            dashboard,
        } => commands::pipeline::run(channels, duration, dashboard),
        Commands::Export {
            input,
            format,
            output,
        } => commands::export::run(&input, &format, &output),
        Commands::Info => {
            commands::info::run();
            Ok(())
        }
        Commands::Witness { output, verify } => {
            commands::witness::run(
                output.map(std::path::PathBuf::from),
                verify.map(std::path::PathBuf::from),
            )
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_simulate_defaults() {
        let cli = Cli::try_parse_from(["ruv-neural", "simulate"]).unwrap();
        match cli.command {
            Commands::Simulate {
                channels,
                duration,
                sample_rate,
                output,
            } => {
                assert_eq!(channels, 64);
                assert!((duration - 10.0).abs() < 1e-9);
                assert!((sample_rate - 1000.0).abs() < 1e-9);
                assert!(output.is_none());
            }
            _ => panic!("Expected Simulate command"),
        }
    }

    #[test]
    fn parse_simulate_with_args() {
        let cli = Cli::try_parse_from([
            "ruv-neural",
            "simulate",
            "-c",
            "32",
            "-d",
            "5.0",
            "-s",
            "500.0",
            "-o",
            "out.json",
        ])
        .unwrap();
        match cli.command {
            Commands::Simulate {
                channels,
                duration,
                sample_rate,
                output,
            } => {
                assert_eq!(channels, 32);
                assert!((duration - 5.0).abs() < 1e-9);
                assert!((sample_rate - 500.0).abs() < 1e-9);
                assert_eq!(output.as_deref(), Some("out.json"));
            }
            _ => panic!("Expected Simulate command"),
        }
    }

    #[test]
    fn parse_analyze() {
        let cli =
            Cli::try_parse_from(["ruv-neural", "analyze", "-i", "graph.json", "--ascii"]).unwrap();
        match cli.command {
            Commands::Analyze { input, ascii, csv } => {
                assert_eq!(input, "graph.json");
                assert!(ascii);
                assert!(csv.is_none());
            }
            _ => panic!("Expected Analyze command"),
        }
    }

    #[test]
    fn parse_mincut() {
        let cli = Cli::try_parse_from(["ruv-neural", "mincut", "-i", "graph.json", "-k", "4"])
            .unwrap();
        match cli.command {
            Commands::Mincut { input, k } => {
                assert_eq!(input, "graph.json");
                assert_eq!(k, Some(4));
            }
            _ => panic!("Expected Mincut command"),
        }
    }

    #[test]
    fn parse_pipeline() {
        let cli = Cli::try_parse_from([
            "ruv-neural",
            "pipeline",
            "-c",
            "16",
            "-d",
            "3.0",
            "--dashboard",
        ])
        .unwrap();
        match cli.command {
            Commands::Pipeline {
                channels,
                duration,
                dashboard,
            } => {
                assert_eq!(channels, 16);
                assert!((duration - 3.0).abs() < 1e-9);
                assert!(dashboard);
            }
            _ => panic!("Expected Pipeline command"),
        }
    }

    #[test]
    fn parse_export() {
        let cli = Cli::try_parse_from([
            "ruv-neural",
            "export",
            "-i",
            "graph.json",
            "-f",
            "dot",
            "-o",
            "out.dot",
        ])
        .unwrap();
        match cli.command {
            Commands::Export {
                input,
                format,
                output,
            } => {
                assert_eq!(input, "graph.json");
                assert_eq!(format, "dot");
                assert_eq!(output, "out.dot");
            }
            _ => panic!("Expected Export command"),
        }
    }

    #[test]
    fn parse_info() {
        let cli = Cli::try_parse_from(["ruv-neural", "info"]).unwrap();
        assert!(matches!(cli.command, Commands::Info));
    }

    #[test]
    fn parse_verbose() {
        let cli = Cli::try_parse_from(["ruv-neural", "-vvv", "info"]).unwrap();
        assert_eq!(cli.verbose, 3);
    }
}
