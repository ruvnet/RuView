//! Runtime configuration for the MQTT publisher, built from CLI args.

use std::path::{Path, PathBuf};

/// File in `--data-dir` holding the persisted default MQTT client id.
pub const CLIENT_ID_FILE: &str = "mqtt_client_id";

/// All knobs the MQTT publisher needs. Built by [`MqttConfig::from_args`]
/// after [`crate::cli::Args`] parsing.
#[derive(Debug, Clone)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub client_id: String,
    pub discovery_prefix: String,
    pub tls: TlsConfig,
    pub refresh_secs: u64,
    pub rates: PublishRates,
    pub publish_pose: bool,
    pub privacy_mode: bool,
}

/// TLS settings for the MQTT publisher.
///
/// `None` means plaintext. `Some(TlsBundle::SystemTrust)` means encrypt
/// against the system trust store. `Some(TlsBundle::PinnedCa { ... })`
/// means encrypt against a specific CA (the typical Cognitum Seed mTLS
/// recipe).
#[derive(Debug, Clone)]
pub enum TlsConfig {
    Off,
    SystemTrust,
    PinnedCa { ca_file: PathBuf },
    MutualTls { ca_file: PathBuf, client_cert: PathBuf, client_key: PathBuf },
}

/// Per-entity publish rates (Hz). Zero means "publish on change only".
#[derive(Debug, Clone, Copy)]
pub struct PublishRates {
    pub vitals_hz: f64,
    pub motion_hz: f64,
    pub count_hz: f64,
    pub rssi_hz: f64,
    pub pose_hz: f64,
}

impl Default for PublishRates {
    fn default() -> Self {
        Self {
            vitals_hz: 0.2,
            motion_hz: 1.0,
            count_hz: 1.0,
            rssi_hz: 0.1,
            pose_hz: 1.0,
        }
    }
}

impl MqttConfig {
    /// Build an [`MqttConfig`] from parsed [`crate::cli::Args`].
    ///
    /// Reads `mqtt_password_env` to resolve the broker password from the
    /// environment so secrets never appear on the command line. Without
    /// `--mqtt-client-id` the client id is per-process; the server uses
    /// [`MqttConfig::from_args_with_data_dir`] for a stable one.
    pub fn from_args(args: &crate::cli::MqttArgs) -> Self {
        let password = std::env::var(&args.mqtt_password_env).ok();
        let port = args.mqtt_port.unwrap_or(if args.mqtt_tls { 8883 } else { 1883 });
        let tls = build_tls(args);
        let client_id = args
            .mqtt_client_id
            .clone()
            .unwrap_or_else(|| format!("wifi-densepose-{}", std::process::id()));

        Self {
            host: args.mqtt_host.clone(),
            port,
            username: args.mqtt_username.clone(),
            password,
            client_id,
            discovery_prefix: args.mqtt_prefix.clone(),
            tls,
            refresh_secs: args.mqtt_refresh_secs,
            rates: PublishRates {
                vitals_hz: args.mqtt_rate_vitals,
                motion_hz: args.mqtt_rate_motion,
                count_hz: args.mqtt_rate_count,
                rssi_hz: args.mqtt_rate_rssi,
                pose_hz: args.mqtt_rate_pose,
            },
            publish_pose: args.mqtt_publish_pose,
            // `--privacy-mode` is a server-wide flag (#2094); the caller sets
            // this from it after building the config.
            privacy_mode: false,
        }
    }

    /// [`MqttConfig::from_args`], with the default client id persisted in
    /// `data_dir` so it survives restarts (#2093). The client id prefixes
    /// every HA device id, so a per-process default created a new set of HA
    /// devices on each restart. An explicit `--mqtt-client-id` still wins.
    pub fn from_args_with_data_dir(args: &crate::cli::MqttArgs, data_dir: &Path) -> Self {
        let mut cfg = Self::from_args(args);
        if args.mqtt_client_id.is_none() {
            match load_or_create_client_id(data_dir) {
                Ok(id) => cfg.client_id = id,
                Err(e) => tracing::warn!(
                    "[mqtt] could not persist a client id in {}: {e}; using {} for this run, \
                     so Home Assistant device ids will change on restart. Set --mqtt-client-id \
                     to pin them.",
                    data_dir.display(),
                    cfg.client_id,
                ),
            }
        }
        cfg
    }

    /// True iff this config is safe to start. Pre-flight validation that
    /// runs before any network I/O so users get a clean error instead of
    /// a connect failure 30 s later.
    pub fn validate(&self) -> Result<(), MqttConfigError> {
        if self.host.is_empty() {
            return Err(MqttConfigError::EmptyHost);
        }
        if self.port == 0 {
            return Err(MqttConfigError::InvalidPort(self.port));
        }
        if self.refresh_secs == 0 {
            return Err(MqttConfigError::RefreshTooSmall);
        }
        for rate in [
            self.rates.vitals_hz,
            self.rates.motion_hz,
            self.rates.count_hz,
            self.rates.rssi_hz,
            self.rates.pose_hz,
        ] {
            if !rate.is_finite() || rate < 0.0 {
                return Err(MqttConfigError::InvalidRate(rate));
            }
        }
        if !self.host.eq_ignore_ascii_case("localhost")
            && !self.host.starts_with("127.")
            && !self.host.starts_with("::1")
            && matches!(self.tls, TlsConfig::Off)
        {
            // Per ADR-115 §3.9 / §9.5 — WARN now, hard-fail at v0.8.0.
            // We return a non-fatal advisory; the caller decides.
            return Err(MqttConfigError::PlaintextOnPublicHost {
                host: self.host.clone(),
            });
        }
        Ok(())
    }
}

/// Read the persisted client id from `<data_dir>/mqtt_client_id`, or create
/// one (`wifi-densepose-<8 hex>`, 23 bytes — the MQTT 3.1.1 guaranteed
/// maximum). The file is untrusted input: a missing, empty or malformed id
/// is replaced rather than sent to the broker.
pub fn load_or_create_client_id(data_dir: &Path) -> std::io::Result<String> {
    let path = data_dir.join(CLIENT_ID_FILE);
    match std::fs::read_to_string(&path) {
        Ok(s) if is_valid_client_id(s.trim()) => return Ok(s.trim().to_string()),
        Ok(_) => tracing::warn!(
            "[mqtt] {} does not hold a valid client id; replacing it",
            path.display()
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e),
    }
    let id = format!("wifi-densepose-{:08x}", rand::random::<u32>());
    std::fs::create_dir_all(data_dir)?;
    let tmp = data_dir.join(format!("{CLIENT_ID_FILE}.tmp"));
    std::fs::write(&tmp, format!("{id}\n"))?;
    std::fs::rename(&tmp, &path)?;
    Ok(id)
}

/// 1–64 bytes of `[A-Za-z0-9_-]`: safe in a client id, an HA `unique_id`
/// and an MQTT topic level.
fn is_valid_client_id(id: &str) -> bool {
    (1..=64).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn build_tls(args: &crate::cli::MqttArgs) -> TlsConfig {
    if !args.mqtt_tls {
        return TlsConfig::Off;
    }
    match (
        args.mqtt_ca_file.as_ref(),
        args.mqtt_client_cert.as_ref(),
        args.mqtt_client_key.as_ref(),
    ) {
        (Some(ca), Some(cert), Some(key)) => TlsConfig::MutualTls {
            ca_file: ca.clone(),
            client_cert: cert.clone(),
            client_key: key.clone(),
        },
        (Some(ca), None, None) => TlsConfig::PinnedCa { ca_file: ca.clone() },
        _ => TlsConfig::SystemTrust,
    }
}

/// Pre-flight validation errors.
#[derive(Debug, thiserror::Error)]
pub enum MqttConfigError {
    #[error("MQTT broker host is empty")]
    EmptyHost,
    #[error("invalid MQTT broker port: {0}")]
    InvalidPort(u16),
    #[error("--mqtt-refresh-secs must be >= 1")]
    RefreshTooSmall,
    #[error("invalid MQTT publish rate: {0} Hz")]
    InvalidRate(f64),
    #[error(
        "plaintext MQTT on non-localhost broker {host} is deprecated and will hard-fail in v0.8.0 \
         (ADR-115 §3.9). Add --mqtt-tls to encrypt."
    )]
    PlaintextOnPublicHost { host: String },
}

impl MqttConfigError {
    /// True for errors that block startup. False for advisories the user
    /// can override (used for the v0.7.0 → v0.8.0 deprecation curve on
    /// plaintext).
    pub fn is_fatal(&self) -> bool {
        !matches!(self, MqttConfigError::PlaintextOnPublicHost { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> crate::cli::MqttArgs {
        use clap::Parser;
        #[derive(Parser)]
        struct W {
            #[command(flatten)]
            m: crate::cli::MqttArgs,
        }
        W::parse_from(std::iter::once("sensing-server").chain(args.iter().copied())).m
    }

    #[test]
    fn from_args_defaults_localhost_1883() {
        let cfg = MqttConfig::from_args(&parse(&[]));
        assert_eq!(cfg.host, "localhost");
        assert_eq!(cfg.port, 1883);
        assert_eq!(cfg.discovery_prefix, "homeassistant");
        assert!(matches!(cfg.tls, TlsConfig::Off));
        assert_eq!(cfg.refresh_secs, 600);
        assert_eq!(cfg.rates.vitals_hz, 0.2);
        assert!(!cfg.publish_pose);
        assert!(!cfg.privacy_mode);
    }

    #[test]
    fn tls_flag_bumps_port_to_8883() {
        let cfg = MqttConfig::from_args(&parse(&["--mqtt-tls"]));
        assert_eq!(cfg.port, 8883);
        assert!(matches!(cfg.tls, TlsConfig::SystemTrust));
    }

    #[test]
    fn explicit_port_overrides_default() {
        let cfg = MqttConfig::from_args(&parse(&["--mqtt-port", "8884"]));
        assert_eq!(cfg.port, 8884);
    }

    #[test]
    fn mtls_when_full_triplet_supplied() {
        let cfg = MqttConfig::from_args(&parse(&[
            "--mqtt-tls",
            "--mqtt-ca-file", "/etc/ca.pem",
            "--mqtt-client-cert", "/etc/client.pem",
            "--mqtt-client-key", "/etc/client.key",
        ]));
        assert!(matches!(cfg.tls, TlsConfig::MutualTls { .. }));
    }

    #[test]
    fn validate_rejects_empty_host() {
        let mut cfg = MqttConfig::from_args(&parse(&[]));
        cfg.host = String::new();
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, MqttConfigError::EmptyHost));
        assert!(err.is_fatal());
    }

    #[test]
    fn validate_rejects_zero_port() {
        let mut cfg = MqttConfig::from_args(&parse(&[]));
        cfg.port = 0;
        assert!(matches!(cfg.validate(), Err(MqttConfigError::InvalidPort(0))));
    }

    #[test]
    fn validate_localhost_plaintext_ok() {
        let cfg = MqttConfig::from_args(&parse(&[]));
        // localhost + plaintext is fine — no advisory.
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_plaintext_public_advises_but_not_fatal() {
        let cfg = MqttConfig::from_args(&parse(&["--mqtt-host", "broker.example.com"]));
        let err = cfg.validate().unwrap_err();
        assert!(matches!(err, MqttConfigError::PlaintextOnPublicHost { .. }));
        assert!(!err.is_fatal(), "v0.7.0 should warn, not block (ADR-115 §3.9)");
    }

    #[test]
    fn validate_public_tls_ok() {
        let cfg = MqttConfig::from_args(&parse(&[
            "--mqtt-host", "broker.example.com",
            "--mqtt-tls",
        ]));
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn validate_rejects_negative_rate() {
        let mut cfg = MqttConfig::from_args(&parse(&[]));
        cfg.rates.vitals_hz = -1.0;
        assert!(matches!(cfg.validate(), Err(MqttConfigError::InvalidRate(_))));
    }

    #[test]
    fn validate_rejects_nan_rate() {
        let mut cfg = MqttConfig::from_args(&parse(&[]));
        cfg.rates.motion_hz = f64::NAN;
        assert!(matches!(cfg.validate(), Err(MqttConfigError::InvalidRate(_))));
    }

    #[test]
    fn default_client_id_is_stable_across_restarts() {
        let dir = tempfile::tempdir().unwrap();
        // Two "restarts": each builds its config from scratch.
        let first = MqttConfig::from_args_with_data_dir(&parse(&[]), dir.path());
        let second = MqttConfig::from_args_with_data_dir(&parse(&[]), dir.path());
        assert_eq!(first.client_id, second.client_id);
        assert!(first.client_id.starts_with("wifi-densepose-"));
        assert_eq!(first.client_id.len(), 23);
        let stored = std::fs::read_to_string(dir.path().join(CLIENT_ID_FILE)).unwrap();
        assert_eq!(stored.trim(), first.client_id);
        // A different install gets a different id.
        let other = tempfile::tempdir().unwrap();
        let third = MqttConfig::from_args_with_data_dir(&parse(&[]), other.path());
        assert_ne!(first.client_id, third.client_id);
    }

    #[test]
    fn explicit_client_id_wins_and_is_not_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = MqttConfig::from_args_with_data_dir(
            &parse(&["--mqtt-client-id", "kitchen"]),
            dir.path(),
        );
        assert_eq!(cfg.client_id, "kitchen");
        assert!(!dir.path().join(CLIENT_ID_FILE).exists());
    }

    #[test]
    fn malformed_persisted_client_id_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(CLIENT_ID_FILE);
        for bad in ["", "has space", "a/b", "+", &"x".repeat(65)] {
            std::fs::write(&path, bad).unwrap();
            let id = load_or_create_client_id(dir.path()).unwrap();
            assert!(is_valid_client_id(&id), "{bad:?} -> {id:?}");
            assert_eq!(std::fs::read_to_string(&path).unwrap().trim(), id);
        }
        std::fs::write(&path, "my-node_1\n").unwrap();
        assert_eq!(load_or_create_client_id(dir.path()).unwrap(), "my-node_1");
    }

    #[test]
    fn unwritable_data_dir_falls_back_to_a_per_process_id() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("not-a-dir");
        std::fs::write(&file, "x").unwrap();
        let cfg = MqttConfig::from_args_with_data_dir(&parse(&[]), &file);
        assert_eq!(
            cfg.client_id,
            format!("wifi-densepose-{}", std::process::id())
        );
    }

    #[test]
    fn password_env_resolution() {
        std::env::set_var("RUVIEW_TEST_MQTT_PW", "s3cret");
        let cfg = MqttConfig::from_args(&parse(&[
            "--mqtt-password-env", "RUVIEW_TEST_MQTT_PW",
        ]));
        assert_eq!(cfg.password.as_deref(), Some("s3cret"));
        std::env::remove_var("RUVIEW_TEST_MQTT_PW");
    }
}
