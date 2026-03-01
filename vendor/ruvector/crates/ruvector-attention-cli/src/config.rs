use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub attention: AttentionSettings,
    pub server: ServerSettings,
    pub output: OutputSettings,
    pub benchmark: BenchmarkSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionSettings {
    pub default_dim: usize,
    pub default_heads: usize,
    pub default_type: String,
    pub dropout: f32,
    pub max_sequence_length: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSettings {
    pub host: String,
    pub port: u16,
    pub max_batch_size: usize,
    pub timeout_ms: u64,
    pub enable_cors: bool,
    pub enable_metrics: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputSettings {
    pub format: String,
    pub pretty: bool,
    pub precision: usize,
    pub color: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BenchmarkSettings {
    pub iterations: usize,
    pub warmup: usize,
    pub sample_size: usize,
    pub dimensions: Vec<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            attention: AttentionSettings {
                default_dim: 512,
                default_heads: 8,
                default_type: "scaled_dot".to_string(),
                dropout: 0.1,
                max_sequence_length: 4096,
            },
            server: ServerSettings {
                host: "0.0.0.0".to_string(),
                port: 8080,
                max_batch_size: 32,
                timeout_ms: 30000,
                enable_cors: true,
                enable_metrics: true,
            },
            output: OutputSettings {
                format: "json".to_string(),
                pretty: true,
                precision: 4,
                color: true,
            },
            benchmark: BenchmarkSettings {
                iterations: 100,
                warmup: 10,
                sample_size: 10,
                dimensions: vec![128, 256, 512, 1024],
            },
        }
    }
}

pub fn load_config(path: Option<&Path>) -> anyhow::Result<Config> {
    if let Some(p) = path {
        let content = std::fs::read_to_string(p)?;
        Ok(toml::from_str(&content)?)
    } else {
        // Try default locations
        let default_paths = [
            "ruvector-attention.toml",
            "config/ruvector-attention.toml",
            "~/.config/ruvector-attention.toml",
        ];

        for path in &default_paths {
            if let Ok(content) = std::fs::read_to_string(path) {
                return Ok(toml::from_str(&content)?);
            }
        }

        Ok(Config::default())
    }
}

impl Config {
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = toml::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }
}
