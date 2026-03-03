use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct HyprSettings {
    pub engine: EngineSettings,
    pub cache: CacheSettings,
}

#[derive(Debug, Deserialize)]
pub struct EngineSettings {
    pub engine: String,
    pub connection: String,
    pub options: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize)]
pub struct CacheSettings {
    pub enabled: bool,
    pub engine: String,
    pub connection: String,
    pub max_duration_secs: u64,
}

impl HyprSettings {
    pub fn new() -> Result<Self, ConfigError> {
        let config_dir = Path::new("config");
        
        let builder = Config::builder()
            // Start with default settings
            .add_source(File::from(config_dir.join("default.toml")).required(false))
            // Add local overrides
            .add_source(File::from(config_dir.join("local.toml")).required(false))
            // Add environment variables with prefix MIDSTREAM_
            .add_source(Environment::with_prefix("MIDSTREAM").separator("_"));

        builder.build()?.try_deserialize()
    }
}

impl Default for HyprSettings {
    fn default() -> Self {
        Self {
            engine: EngineSettings {
                engine: "duckdb".to_string(),
                connection: ":memory:".to_string(),
                options: std::collections::HashMap::new(),
            },
            cache: CacheSettings {
                enabled: true,
                engine: "duckdb".to_string(),
                connection: ":memory:".to_string(),
                max_duration_secs: 3600,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn setup() {
        INIT.call_once(|| {
            std::env::set_var("MIDSTREAM_ENGINE_ENGINE", "test_engine");
        });
    }

    #[test]
    fn test_default_settings() {
        let settings = HyprSettings::default();
        assert_eq!(settings.engine.engine, "duckdb");
        assert!(settings.cache.enabled);
    }

    #[test]
    fn test_environment_override() {
        setup();
        let settings = HyprSettings::new().unwrap();
        assert_eq!(settings.engine.engine, "test_engine");
    }
}
