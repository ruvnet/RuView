//! Configuration management for Hyprstream service.
//!
//! This module provides configuration handling through multiple sources:
//! 1. Default configuration (embedded in binary)
//! 2. System-wide configuration file (`/etc/hyprstream/config.toml`)
//! 3. User-specified configuration file
//! 4. Environment variables (prefixed with `HYPRSTREAM_`)
//! 5. Command-line arguments
//!
//! Configuration options are loaded in order of precedence, with later sources
//! overriding earlier ones.
//!
//! # Environment Variables
//!
//! Backend-specific credentials should be provided via environment variables:
//! - `HYPRSTREAM_ENGINE_USERNAME` - Primary storage backend username
//! - `HYPRSTREAM_ENGINE_PASSWORD` - Primary storage backend password
//! - `HYPRSTREAM_CACHE_USERNAME` - Cache backend username (if needed)
//! - `HYPRSTREAM_CACHE_PASSWORD` - Cache backend password (if needed)

use clap::Parser;
use config::{Config, ConfigError};
use serde::Deserialize;
use std::env;
use std::path::PathBuf;
use std::collections::HashMap;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");
const DEFAULT_CONFIG_PATH: &str = "/etc/hyprstream/config.toml";

/// Command-line arguments parser.
///
/// This structure defines all available command-line options and their
/// corresponding environment variables. It uses clap for parsing and
/// supports both short and long option forms.
#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct CliArgs {
    /// Path to the configuration file
    #[arg(short, long, value_name = "FILE")]
    config: Option<PathBuf>,

    /// Server host address
    #[arg(long, env = "HYPRSTREAM_SERVER_HOST")]
    host: Option<String>,

    /// Server port
    #[arg(long, env = "HYPRSTREAM_SERVER_PORT")]
    port: Option<u16>,

    /// Primary storage engine type
    #[arg(long, env = "HYPRSTREAM_ENGINE")]
    engine: Option<String>,

    /// Primary storage engine connection string
    #[arg(long, env = "HYPRSTREAM_ENGINE_CONNECTION")]
    engine_connection: Option<String>,

    /// Primary storage engine options (key=value pairs)
    #[arg(long, env = "HYPRSTREAM_ENGINE_OPTIONS")]
    engine_options: Option<Vec<String>>,

    /// Enable caching
    #[arg(long, env = "HYPRSTREAM_ENABLE_CACHE")]
    enable_cache: Option<bool>,

    /// Cache engine type
    #[arg(long, env = "HYPRSTREAM_CACHE_ENGINE")]
    cache_engine: Option<String>,

    /// Cache engine connection string
    #[arg(long, env = "HYPRSTREAM_CACHE_CONNECTION")]
    cache_connection: Option<String>,

    /// Cache engine options (key=value pairs)
    #[arg(long, env = "HYPRSTREAM_CACHE_OPTIONS")]
    cache_options: Option<Vec<String>>,

    /// Cache maximum duration in seconds
    #[arg(long, env = "HYPRSTREAM_CACHE_MAX_DURATION")]
    cache_max_duration: Option<u64>,

    /// Primary storage engine username
    #[arg(long, env = "HYPRSTREAM_ENGINE_USERNAME")]
    engine_username: Option<String>,

    /// Primary storage engine password
    #[arg(long, env = "HYPRSTREAM_ENGINE_PASSWORD")]
    engine_password: Option<String>,

    /// Cache engine username
    #[arg(long, env = "HYPRSTREAM_CACHE_USERNAME")]
    cache_username: Option<String>,

    /// Cache engine password
    #[arg(long, env = "HYPRSTREAM_CACHE_PASSWORD")]
    cache_password: Option<String>,
}

/// Complete service configuration.
///
/// This structure holds all configuration options for the service,
/// including server settings, storage backend configuration, and
/// cache settings.
#[derive(Debug, Deserialize)]
pub struct Settings {
    /// Server configuration
    pub server: ServerConfig,
    /// Engine configuration
    pub engine: EngineConfig,
    /// Cache configuration
    pub cache: CacheConfig,
}

/// Server configuration options.
///
/// Defines the network interface and port for the Flight SQL service.
#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    /// Host address to bind to
    pub host: String,
    /// Port number to listen on
    pub port: u16,
}

/// Engine configuration.
///
/// Specifies the primary storage engine to use for metric data.
#[derive(Debug, Deserialize)]
pub struct EngineConfig {
    /// Engine type ("duckdb" or "adbc")
    pub engine: String,
    /// Connection string for the engine
    pub connection: String,
    /// Engine-specific options
    #[serde(default)]
    pub options: std::collections::HashMap<String, String>,
    /// Authentication credentials (not serialized)
    #[serde(skip)]
    pub credentials: Option<Credentials>,
}

/// Authentication credentials for storage backends.
#[derive(Debug, Clone, Deserialize)]
pub struct Credentials {
    /// Username for authentication
    pub username: String,
    /// Password for authentication
    pub password: String,
}

/// Cache configuration for the storage backend.
#[derive(Debug, Deserialize)]
pub struct CacheConfig {
    /// Whether caching is enabled
    pub enabled: bool,
    /// Cache storage engine type (e.g., "duckdb", "adbc")
    pub engine: String,
    /// Cache connection string
    pub connection: String,
    /// Cache engine options
    pub options: HashMap<String, String>,
    /// Cache credentials (optional)
    #[serde(default)]
    pub credentials: Option<Credentials>,
    /// Maximum duration to keep entries in cache (in seconds)
    #[serde(default = "default_ttl")]
    pub ttl: Option<u64>,
}

fn default_ttl() -> Option<u64> {
    Some(3600) // Default 1 hour TTL
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            engine: "duckdb".to_string(),
            connection: ":memory:".to_string(),
            options: HashMap::new(),
            credentials: None,
            ttl: default_ttl(),
        }
    }
}

impl Settings {
    /// Loads configuration from all available sources.
    pub fn new(cli: CliArgs) -> Result<Self, ConfigError> {
        let mut builder = Config::builder();

        // Load default configuration
        builder = builder.add_source(config::File::from_str(
            DEFAULT_CONFIG,
            config::FileFormat::Toml,
        ));

        // Load system configuration if it exists
        if let Ok(metadata) = std::fs::metadata(DEFAULT_CONFIG_PATH) {
            if metadata.is_file() {
                builder = builder.add_source(config::File::from(PathBuf::from(DEFAULT_CONFIG_PATH)));
            }
        }

        // Load user configuration if specified
        if let Some(ref config_path) = cli.config {
            builder = builder.add_source(config::File::from(config_path.clone()));
        }

        // Add environment variables (prefixed with HYPRSTREAM_)
        builder = builder.add_source(config::Environment::with_prefix("HYPRSTREAM"));

        // Override with command line arguments
        if let Some(ref host) = cli.host {
            builder = builder.set_override("server.host", host.as_str())?;
        }
        if let Some(port) = cli.port {
            builder = builder.set_override("server.port", port)?;
        }

        // Engine settings
        if let Some(ref engine) = cli.engine {
            builder = builder.set_override("engine.engine", engine.as_str())?;
        }
        if let Some(ref connection) = cli.engine_connection {
            builder = builder.set_override("engine.connection", connection.as_str())?;
        }
        if let Some(ref options) = cli.engine_options {
            let options: std::collections::HashMap<String, String> = options
                .iter()
                .filter_map(|opt| {
                    let parts: Vec<&str> = opt.split('=').collect();
                    if parts.len() == 2 {
                        Some((parts[0].to_string(), parts[1].to_string()))
                    } else {
                        None
                    }
                })
                .collect();
            builder = builder.set_override("engine.options", options)?;
        }

        // Cache settings
        if let Some(enabled) = cli.enable_cache {
            builder = builder.set_override("cache.enabled", enabled)?;
        }
        if let Some(ref engine) = cli.cache_engine {
            builder = builder.set_override("cache.engine", engine.as_str())?;
        }
        if let Some(ref connection) = cli.cache_connection {
            builder = builder.set_override("cache.connection", connection.as_str())?;
        }
        if let Some(ref options) = cli.cache_options {
            let options: std::collections::HashMap<String, String> = options
                .iter()
                .filter_map(|opt| {
                    let parts: Vec<&str> = opt.split('=').collect();
                    if parts.len() == 2 {
                        Some((parts[0].to_string(), parts[1].to_string()))
                    } else {
                        None
                    }
                })
                .collect();
            builder = builder.set_override("cache.options", options)?;
        }
        if let Some(duration) = cli.cache_max_duration {
            builder = builder.set_override("cache.max_duration_secs", duration)?;
        }

        // Build initial settings
        let mut settings: Settings = builder.build()?.try_deserialize()?;

        // Load credentials from environment variables and CLI args
        settings.engine.credentials = Self::load_engine_credentials(&cli);
        settings.cache.credentials = Self::load_cache_credentials(&cli);

        Ok(settings)
    }

    /// Load engine credentials from all available sources.
    /// Priority order (highest to lowest):
    /// 1. Environment variables
    /// 2. Command line arguments
    fn load_engine_credentials(cli: &CliArgs) -> Option<Credentials> {
        // Try environment variables first
        if let (Some(username), Some(password)) = (
            env::var("HYPRSTREAM_ENGINE_USERNAME").ok(),
            env::var("HYPRSTREAM_ENGINE_PASSWORD").ok(),
        ) {
            return Some(Credentials { username, password });
        }

        // Try command line arguments
        if let (Some(username), Some(password)) = (&cli.engine_username, &cli.engine_password) {
            return Some(Credentials {
                username: username.clone(),
                password: password.clone(),
            });
        }

        None
    }

    /// Load cache credentials from all available sources.
    /// Priority order (highest to lowest):
    /// 1. Environment variables
    /// 2. Command line arguments
    fn load_cache_credentials(cli: &CliArgs) -> Option<Credentials> {
        // Try environment variables first
        if let (Some(username), Some(password)) = (
            env::var("HYPRSTREAM_CACHE_USERNAME").ok(),
            env::var("HYPRSTREAM_CACHE_PASSWORD").ok(),
        ) {
            return Some(Credentials { username, password });
        }

        // Try command line arguments
        if let (Some(username), Some(password)) = (&cli.cache_username, &cli.cache_password) {
            return Some(Credentials {
                username: username.clone(),
                password: password.clone(),
            });
        }

        None
    }
}
