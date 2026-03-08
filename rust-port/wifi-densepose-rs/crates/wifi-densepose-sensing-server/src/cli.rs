//! CLI argument parsing for sensing-server
//!
//! Extracted from main.rs as part of ADR-051 Phase 1

use clap::Parser;
use std::path::PathBuf;

/// WiFi-DensePose sensing server
#[derive(Parser, Debug, Clone)]
#[command(name = "sensing-server", about = "WiFi-DensePose sensing server")]
pub struct Args {
    /// HTTP port for UI and REST API
    #[arg(long, default_value = "8080")]
    pub http_port: u16,

    /// WebSocket port for sensing stream
    #[arg(long, default_value = "8765")]
    pub ws_port: u16,

    /// UDP port for ESP32 CSI frames
    #[arg(long, default_value = "5005")]
    pub udp_port: u16,

    /// Path to UI static files
    #[arg(long, default_value = "../../ui")]
    pub ui_path: PathBuf,

    /// Tick interval in milliseconds (default 100 ms = 10 fps for smooth pose animation)
    #[arg(long, default_value = "100")]
    pub tick_ms: u64,

    /// Bind address (default 127.0.0.1; set to 0.0.0.0 for network access)
    #[arg(long, default_value = "127.0.0.1", env = "SENSING_BIND_ADDR")]
    pub bind_addr: String,

    /// Data source: auto, wifi, esp32, simulate
    #[arg(long, default_value = "auto")]
    pub source: String,

    /// Run vital sign detection benchmark (1000 frames) and exit
    #[arg(long)]
    pub benchmark: bool,

    /// Load model config from an RVF container at startup
    #[arg(long, value_name = "PATH")]
    pub load_rvf: Option<PathBuf>,

    /// Save current model state as an RVF container on shutdown
    #[arg(long, value_name = "PATH")]
    pub save_rvf: Option<PathBuf>,

    /// Load a trained .rvf model for inference
    #[arg(long, value_name = "PATH")]
    pub model: Option<PathBuf>,

    /// Training data directory
    #[arg(long, value_name = "DIR")]
    pub train_data: Option<PathBuf>,

    /// Validate model on test set and exit
    #[arg(long)]
    pub validate: bool,

    /// Logging verbosity (v=info, vv=debug, vvv=trace)
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Disable vital sign detection
    #[arg(long)]
    pub no_vitals: bool,

    /// Enable trainer API endpoints
    #[arg(long)]
    pub enable_trainer: bool,

    /// Embedding model for semantic search
    #[arg(long, value_name = "PATH")]
    pub embedding_model: Option<PathBuf>,

    /// Number of recent CSI frames to keep for breathing detection
    #[arg(long, default_value = "300")]
    pub vitals_history: usize,

    /// CSI frame rate for breathing detection (Hz)
    #[arg(long, default_value = "100.0")]
    pub vitals_fps: f64,
}

impl Args {
    /// Parse CLI arguments
    pub fn parse_args() -> Self {
        Self::parse()
    }
}
