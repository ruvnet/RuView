//! Canonical CLI argument structure for sensing-server.

use clap::Parser;
use std::path::PathBuf;
use super::env::parse_env_bool;
use super::mqtt::MqttArgs;
use crate::ingest::SOURCE_VALUES;

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

    /// Per-device ring buffer size (frame count) for MediaTek per-subcarrier
    /// CSI inspection (`/api/v1/csi/mediatek/devices/:id/frames`).
    #[arg(long, default_value = "256")]
    pub csi_ring: usize,

    /// Path to persist each MediaTek device's learned `activity` floor.
    #[arg(long)]
    pub activity_state: Option<PathBuf>,

    /// UDP bind address for the CSI receiver (ADR-296). Defaults to `127.0.0.1`.
    #[arg(long, default_value = "127.0.0.1", env = "RUVIEW_UDP_BIND")]
    pub udp_bind: String,

    /// Source IP/CIDR allowlist for inbound UDP CSI frames.
    #[arg(long = "udp-allow", value_name = "IP/CIDR", env = "RUVIEW_UDP_ALLOW")]
    pub udp_allow: Vec<String>,

    /// Accept a routable UDP bind with no source allowlist.
    #[arg(long, env = "RUVIEW_UDP_INSECURE_LAN", value_parser = parse_env_bool)]
    pub udp_insecure_lan: bool,

    /// Copy every admitted UDP datagram to these loopback `ip:port` targets.
    #[arg(long = "udp-tee", value_name = "IP:PORT", env = "RUVIEW_UDP_TEE")]
    pub udp_tee: Vec<String>,

    /// Path to UI static files
    #[arg(long, default_value = "../ui")]
    pub ui_path: PathBuf,

    /// Tick interval in milliseconds (default 100 ms = 10 fps)
    #[arg(long, default_value = "100")]
    pub tick_ms: u64,

    /// Bind address (default 127.0.0.1)
    #[arg(long, default_value = "127.0.0.1", env = "SENSING_BIND_ADDR")]
    pub bind_addr: String,

    /// Disable local `_ruview._tcp` discovery.
    #[arg(long, env = "RUVIEW_NO_MDNS", value_parser = parse_env_bool)]
    pub no_mdns: bool,

    /// Stable, non-secret installation routing hint published over mDNS.
    #[arg(long, env = "RUVIEW_INSTALLATION_ID")]
    pub installation_id: Option<String>,

    /// Private server state directory.
    #[arg(long, default_value = "data", env = "RUVIEW_DATA_DIR")]
    pub data_dir: PathBuf,

    /// Human-readable local service name shown by commissioning clients.
    #[arg(long, default_value = "RuView Installation", env = "RUVIEW_INSTALLATION_NAME")]
    pub installation_name: String,

    /// Additional hostname to permit in the `Host` header.
    #[arg(long = "allowed-host", value_name = "HOST")]
    pub allowed_hosts: Vec<String>,

    /// Disable Host-header validation entirely.
    #[arg(long)]
    pub disable_host_validation: bool,

    /// Strip biometrics from every output.
    #[arg(long, env = "RUVIEW_PRIVACY_MODE", value_parser = parse_env_bool)]
    pub privacy_mode: bool,

    /// MQTT publisher (HA auto-discovery) flags.
    #[command(flatten)]
    pub mqtt_opts: MqttArgs,

    /// Data source: auto, esp32, wifi, mediatek, simulated (alias: simulate)
    #[arg(long, default_value = "auto", value_parser = SOURCE_VALUES)]
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

    /// Convert a published model file to the RVF binary container.
    #[arg(long, value_name = "PATH")]
    pub convert_model: Option<PathBuf>,

    /// Output path for --convert-model (defaults to <input>.rvf).
    #[arg(long, value_name = "PATH")]
    pub convert_out: Option<PathBuf>,

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

    /// Override the upstream URL for the edge module registry.
    #[arg(
        long,
        value_name = "URL",
        env = "RUVIEW_EDGE_REGISTRY_URL",
        default_value = "https://storage.googleapis.com/cognitum-apps/app-registry.json"
    )]
    pub edge_registry_url: String,

    /// Cache TTL for the edge module registry, in seconds.
    #[arg(
        long,
        value_name = "SECS",
        env = "RUVIEW_EDGE_REGISTRY_TTL_SECS",
        default_value = "3600"
    )]
    pub edge_registry_ttl_secs: u64,

    /// Disable the edge module registry endpoint entirely.
    #[arg(long, env = "RUVIEW_NO_EDGE_REGISTRY", value_parser = parse_env_bool)]
    pub no_edge_registry: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_env_bool_flags_set_true() {
        let args = Args::parse_from([
            "sensing-server",
            "--udp-insecure-lan",
            "--no-mdns",
            "--no-edge-registry",
        ]);
        assert!(args.udp_insecure_lan);
        assert!(args.no_mdns);
        assert!(args.no_edge_registry);
    }

    #[test]
    fn unknown_source_is_rejected_at_parse_time() {
        let err = Args::try_parse_from(["sensing-server", "--source", "macos"])
            .expect_err("--source macos must be rejected");
        let msg = err.to_string();
        for valid in ["auto", "esp32", "wifi", "simulated"] {
            assert!(msg.contains(valid), "error should list '{valid}': {msg}");
        }
    }

    #[test]
    fn only_garbage_is_rejected() {
        for bogus in [
            "bogus",
            "macos",
            "linux",
            "mock",
            "MEDIATEK",
            "mediatek:simulated",
            "esp32 ",
        ] {
            assert!(
                Args::try_parse_from(["sensing-server", "--source", bogus]).is_err(),
                "--source {bogus:?} must be rejected"
            );
        }
    }

    #[test]
    fn every_documented_source_still_parses() {
        for source in [
            "auto",
            "esp32",
            "wifi",
            "simulated",
            "simulate",
            "mediatek",
            "qualcomm",
            "realtek",
            "realtek_csi",
        ] {
            let args = Args::try_parse_from(["sensing-server", "--source", source])
                .unwrap_or_else(|e| panic!("--source {source} must parse: {e}"));
            assert_eq!(args.source, source);
        }
        let args = Args::try_parse_from(["sensing-server"]).expect("default parses");
        assert_eq!(args.source, "auto");
    }
}
