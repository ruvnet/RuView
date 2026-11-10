//! MQTT CLI argument definitions.

use std::path::PathBuf;
use super::env::parse_env_bool;

/// MQTT publisher (HA auto-discovery) flags, flattened into the binary's
/// `main::Args` so the `--mqtt*` flags reach the actual `Args::parse()` the
/// server uses (the publisher in `mqtt::` is keyed off this group). ADR-115
/// §3.8. `--privacy-mode` is a top-level server flag (#2094), not part of
/// this group.
#[derive(clap::Args, Debug, Clone)]
pub struct MqttArgs {
    /// Enable MQTT publisher with HA auto-discovery
    #[arg(long, env = "RUVIEW_MQTT", value_parser = parse_env_bool)]
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

    /// MQTT client ID; prefixes every Home Assistant device id (default: a
    /// `wifi-densepose-<hex>` id persisted in `<data-dir>/mqtt_client_id`)
    #[arg(long, env = "RUVIEW_MQTT_CLIENT_ID")]
    pub mqtt_client_id: Option<String>,

    /// Discovery topic prefix (ADR-115 §9.2 — accepted: `homeassistant`)
    #[arg(long, env = "RUVIEW_MQTT_PREFIX", default_value = "homeassistant")]
    pub mqtt_prefix: String,

    /// Enable TLS to the broker
    #[arg(long, env = "RUVIEW_MQTT_TLS", value_parser = parse_env_bool)]
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
}

