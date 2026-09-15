//! CLI argument definitions and early-exit mode handlers.

use clap::Parser;
use std::path::PathBuf;

/// MQTT publisher (HA auto-discovery) + privacy-mode flags, shared via
/// `#[command(flatten)]` by both `cli::Args` and the binary's `main::Args`
/// so the `--mqtt*` flags reach the actual `Args::parse()` the server uses
/// (the publisher in `mqtt::` is keyed off this group). ADR-115 §3.8/§3.10.
#[derive(clap::Args, Debug, Clone)]
pub struct MqttArgs {
    /// Enable MQTT publisher with HA auto-discovery
    #[arg(long, env = "RUVIEW_MQTT")]
    pub mqtt: bool,

    /// MQTT broker host
    #[arg(long, env = "RUVIEW_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port (defaults: 1883 plain / 8883 with TLS)
    #[arg(long, env = "RUVIEW_MQTT_PORT")]
    pub mqtt_port: Option<u16>,

    /// MQTT username
    #[arg(long, env = "RUVIEW_MQTT_USERNAME")]
    pub mqtt_username: Option<String>,

    /// Environment variable holding the MQTT password
    #[arg(long, default_value = "MQTT_PASSWORD")]
    pub mqtt_password_env: String,

    /// MQTT client ID (default: wifi-densepose-<pid>)
    #[arg(long, env = "RUVIEW_MQTT_CLIENT_ID")]
    pub mqtt_client_id: Option<String>,

    /// Discovery topic prefix (ADR-115 §9.2 — accepted: `homeassistant`)
    #[arg(long, env = "RUVIEW_MQTT_PREFIX", default_value = "homeassistant")]
    pub mqtt_prefix: String,

    /// Enable TLS to the broker
    #[arg(long, env = "RUVIEW_MQTT_TLS")]
    pub mqtt_tls: bool,

    /// CA bundle for TLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_ca_file: Option<PathBuf>,

    /// Client certificate for mTLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_client_cert: Option<PathBuf>,

    /// Client key for mTLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_client_key: Option<PathBuf>,

    /// Discovery refresh interval (seconds)
    #[arg(long, default_value = "600")]
    pub mqtt_refresh_secs: u64,

    /// Vitals publish rate (Hz) — HR/BR
    #[arg(long, default_value = "0.2")]
    pub mqtt_rate_vitals: f64,

    /// Motion publish rate (Hz)
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_motion: f64,

    /// Person count publish rate (Hz)
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_count: f64,

    /// RSSI publish rate (Hz)
    #[arg(long, default_value = "0.1")]
    pub mqtt_rate_rssi: f64,

    /// Publish pose keypoints over MQTT (off by default for bandwidth)
    #[arg(long)]
    pub mqtt_publish_pose: bool,

    /// Pose publish rate (Hz) when --mqtt-publish-pose is set
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_pose: f64,

    /// Strip biometrics (HR/BR/pose) before any MQTT/Matter publish (ADR-115 §3.10).
    #[arg(long, env = "RUVIEW_PRIVACY_MODE")]
    pub privacy_mode: bool,
}

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

    /// Path to UI static files (from `v2/` cwd use `../ui`)
    #[arg(long, default_value = "../ui")]
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

    // ─── ADR-115 §3.8 — MQTT publisher (HA-DISCO) ──────────────────────────
    /// Enable MQTT publisher with HA auto-discovery
    #[arg(long, env = "RUVIEW_MQTT")]
    pub mqtt: bool,

    /// MQTT broker host
    #[arg(long, env = "RUVIEW_MQTT_HOST", default_value = "localhost")]
    pub mqtt_host: String,

    /// MQTT broker port (defaults: 1883 plain / 8883 with TLS)
    #[arg(long, env = "RUVIEW_MQTT_PORT")]
    pub mqtt_port: Option<u16>,

    /// MQTT username
    #[arg(long, env = "RUVIEW_MQTT_USERNAME")]
    pub mqtt_username: Option<String>,

    /// Environment variable holding the MQTT password
    #[arg(long, default_value = "MQTT_PASSWORD")]
    pub mqtt_password_env: String,

    /// MQTT client ID (default: wifi-densepose-<hostname>)
    #[arg(long, env = "RUVIEW_MQTT_CLIENT_ID")]
    pub mqtt_client_id: Option<String>,

    /// Discovery topic prefix (ADR-115 §9.2 — accepted: `homeassistant`)
    #[arg(long, env = "RUVIEW_MQTT_PREFIX", default_value = "homeassistant")]
    pub mqtt_prefix: String,

    /// Enable TLS to the broker
    #[arg(long, env = "RUVIEW_MQTT_TLS")]
    pub mqtt_tls: bool,

    /// CA bundle for TLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_ca_file: Option<PathBuf>,

    /// Client certificate for mTLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_client_cert: Option<PathBuf>,

    /// Client key for mTLS
    #[arg(long, value_name = "PATH")]
    pub mqtt_client_key: Option<PathBuf>,

    /// Discovery refresh interval (seconds)
    #[arg(long, default_value = "600")]
    pub mqtt_refresh_secs: u64,

    /// Vitals publish rate (Hz) — HR/BR
    #[arg(long, default_value = "0.2")]
    pub mqtt_rate_vitals: f64,

    /// Motion publish rate (Hz)
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_motion: f64,

    /// Person count publish rate (Hz)
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_count: f64,

    /// RSSI publish rate (Hz)
    #[arg(long, default_value = "0.1")]
    pub mqtt_rate_rssi: f64,

    /// Publish pose keypoints over MQTT (off by default for bandwidth)
    #[arg(long)]
    pub mqtt_publish_pose: bool,

    /// Pose publish rate (Hz) when --mqtt-publish-pose is set
    #[arg(long, default_value = "1.0")]
    pub mqtt_rate_pose: f64,

    // ─── ADR-115 §3.10 — Privacy mode ──────────────────────────────────────
    /// Strip biometrics (HR/BR/pose) before any MQTT or Matter publish.
    /// Discovery for those entities is suppressed entirely — the controller
    /// never sees them exist. Implements the ADR-106 primitive-isolation
    /// contract at the integration boundary.
    #[arg(long, env = "RUVIEW_PRIVACY_MODE")]
    pub privacy_mode: bool,

    // ─── ADR-115 §3.11 — Matter Bridge (HA-FABRIC) ─────────────────────────
    /// Enable Matter Bridge
    #[arg(long, env = "RUVIEW_MATTER")]
    pub matter: bool,

    /// Write Matter setup code + QR string to this file on first start
    #[arg(long, value_name = "PATH")]
    pub matter_setup_file: Option<PathBuf>,

    /// Wipe stored Matter fabric credentials before starting
    #[arg(long)]
    pub matter_reset: bool,

    /// Matter vendor ID (default: dev VID 0xFFF1 per ADR-115 §9.9)
    #[arg(long, default_value = "0xFFF1")]
    pub matter_vendor_id: String,

    /// Matter product ID (default: 0x8001)
    #[arg(long, default_value = "0x8001")]
    pub matter_product_id: String,

    // ─── ADR-115 §3.12 — Semantic Inference (HA-MIND) ─────────────────────
    /// Enable semantic inference layer (sleeping/distress/room-active/etc).
    /// Default ON — primitives are the primary product surface.
    #[arg(long, default_value_t = true)]
    pub semantic: bool,

    /// Per-primitive thresholds file
    #[arg(long, value_name = "PATH")]
    pub semantic_thresholds_file: Option<PathBuf>,

    /// Zone-tag map (e.g. {"bathroom": ["zone_3"]})
    #[arg(long, value_name = "PATH")]
    pub semantic_zones_file: Option<PathBuf>,

    /// Days of history for personalised baselines
    #[arg(long, default_value = "14")]
    pub semantic_baseline_window_days: u32,

    /// Disable a specific semantic primitive (e.g. `sleeping`); repeatable.
    /// Valid names: sleeping, distress, room_active, elderly_anomaly,
    /// meeting, bathroom, fall_risk, bed_exit, no_movement, multi_room.
    #[arg(long = "no-semantic", value_name = "PRIMITIVE")]
    pub no_semantic: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// MQTT flags default safely (disabled).
    #[test]
    fn mqtt_defaults_disabled() {
        let args = Args::parse_from(["sensing-server"]);
        assert!(!args.mqtt, "--mqtt must default to false");
        assert_eq!(args.mqtt_host, "localhost");
        assert_eq!(args.mqtt_prefix, "homeassistant");
        assert_eq!(args.mqtt_refresh_secs, 600);
        assert_eq!(args.mqtt_rate_vitals, 0.2);
        assert_eq!(args.mqtt_rate_motion, 1.0);
        assert_eq!(args.mqtt_rate_count, 1.0);
        assert_eq!(args.mqtt_rate_rssi, 0.1);
        assert!(!args.mqtt_publish_pose);
        assert_eq!(args.mqtt_rate_pose, 1.0);
        assert!(!args.mqtt_tls);
        assert!(args.mqtt_username.is_none());
        assert!(args.mqtt_port.is_none());
    }

    #[test]
    fn privacy_mode_defaults_off() {
        let args = Args::parse_from(["sensing-server"]);
        assert!(!args.privacy_mode);
    }

    #[test]
    fn matter_defaults_off_dev_vid() {
        let args = Args::parse_from(["sensing-server"]);
        assert!(!args.matter);
        assert_eq!(args.matter_vendor_id, "0xFFF1");
        assert_eq!(args.matter_product_id, "0x8001");
    }

    #[test]
    fn semantic_defaults_on() {
        let args = Args::parse_from(["sensing-server"]);
        assert!(args.semantic);
        assert!(args.no_semantic.is_empty());
        assert_eq!(args.semantic_baseline_window_days, 14);
    }

    #[test]
    fn mqtt_all_flags_compose() {
        let args = Args::parse_from([
            "sensing-server",
            "--mqtt",
            "--mqtt-host", "broker.example.com",
            "--mqtt-port", "8883",
            "--mqtt-username", "ruview",
            "--mqtt-prefix", "homeassistant",
            "--mqtt-tls",
            "--mqtt-refresh-secs", "300",
            "--mqtt-rate-vitals", "0.5",
            "--mqtt-publish-pose",
            "--mqtt-rate-pose", "2.0",
            "--privacy-mode",
        ]);
        assert!(args.mqtt);
        assert_eq!(args.mqtt_host, "broker.example.com");
        assert_eq!(args.mqtt_port, Some(8883));
        assert_eq!(args.mqtt_username.as_deref(), Some("ruview"));
        assert!(args.mqtt_tls);
        assert_eq!(args.mqtt_refresh_secs, 300);
        assert_eq!(args.mqtt_rate_vitals, 0.5);
        assert!(args.mqtt_publish_pose);
        assert_eq!(args.mqtt_rate_pose, 2.0);
        assert!(args.privacy_mode);
    }

    #[test]
    fn no_semantic_repeatable() {
        let args = Args::parse_from([
            "sensing-server",
            "--no-semantic", "sleeping",
            "--no-semantic", "meeting",
            "--no-semantic", "fall_risk",
        ]);
        assert_eq!(args.no_semantic, vec!["sleeping", "meeting", "fall_risk"]);
    }
}

/// CLI args for the Neo4j sink (flattened into `main::Args`).
#[derive(clap::Args, Debug, Clone)]
pub struct Neo4jArgs {
    /// Enable Neo4j sink for sensing event logging
    #[arg(long, env = "RUVIEW_NEO4J")]
    pub neo4j: bool,

    /// Neo4j Bolt URL
    #[arg(long, env = "RUVIEW_NEO4J_URL", default_value = "bolt://x1-370:7687")]
    pub neo4j_url: String,

    /// Neo4j username
    #[arg(long, env = "RUVIEW_NEO4J_USER", default_value = "neo4j")]
    pub neo4j_user: String,

    /// Environment variable holding the Neo4j password
    #[arg(long, default_value = "NEO4J_PASSWORD")]
    pub neo4j_password_env: String,

    /// Room identifier for the :Room node
    #[arg(long, env = "RUVIEW_NEO4J_ROOM", default_value = "main")]
    pub neo4j_room_name: String,

    /// TTL in hours: events older than this are deleted periodically (0 = no compaction)
    #[arg(long, env = "RUVIEW_NEO4J_TTL_HOURS", default_value = "168")]
    pub neo4j_ttl_hours: u64,
}
