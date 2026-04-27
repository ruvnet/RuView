//! CLI argument definitions and early-exit mode handlers.

use std::path::PathBuf;
use clap::Parser;

/// CLI arguments for the sensing server.
#[derive(Parser, Debug)]
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

    /// Enable progressive loading (Layer A instant start)
    #[arg(long)]
    pub progressive: bool,

    /// Export an RVF container package and exit (no server)
    #[arg(long, value_name = "PATH")]
    pub export_rvf: Option<PathBuf>,

    /// Run training mode (train a model and exit)
    #[arg(long)]
    pub train: bool,

    /// Path to dataset directory (MM-Fi or Wi-Pose)
    #[arg(long, value_name = "PATH")]
    pub dataset: Option<PathBuf>,

    /// Dataset type: "mmfi" or "wipose"
    #[arg(long, value_name = "TYPE", default_value = "mmfi")]
    pub dataset_type: String,

    /// Number of training epochs
    #[arg(long, default_value = "100")]
    pub epochs: usize,

    /// Directory for training checkpoints
    #[arg(long, value_name = "DIR")]
    pub checkpoint_dir: Option<PathBuf>,

    /// Run self-supervised contrastive pretraining (ADR-024)
    #[arg(long)]
    pub pretrain: bool,

    /// Number of pretraining epochs (default 50)
    #[arg(long, default_value = "50")]
    pub pretrain_epochs: usize,

    /// Extract embeddings mode: load model and extract CSI embeddings
    #[arg(long)]
    pub embed: bool,

    /// Build fingerprint index from embeddings (env|activity|temporal|person)
    #[arg(long, value_name = "TYPE")]
    pub build_index: Option<String>,

    /// Node positions for multistatic fusion (format: "x,y,z;x,y,z;...")
    #[arg(long, env = "SENSING_NODE_POSITIONS")]
    pub node_positions: Option<String>,

    /// Start field model calibration on boot (empty room required)
    #[arg(long)]
    pub calibrate: bool,
}
