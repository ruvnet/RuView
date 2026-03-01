//! Signal normalization.

use super::Signal;

/// Configuration for normalization.
#[derive(Debug, Clone)]
pub struct NormalizationConfig {
    /// Lowercase all string values
    pub lowercase_strings: bool,
    /// Trim whitespace from strings
    pub trim_whitespace: bool,
    /// Replace null values with defaults
    pub replace_nulls: bool,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            lowercase_strings: false,
            trim_whitespace: true,
            replace_nulls: false,
        }
    }
}

/// Normalizer for signals.
pub struct Normalizer {
    config: NormalizationConfig,
}

impl Normalizer {
    /// Create a new normalizer.
    pub fn new(config: NormalizationConfig) -> Self {
        Self { config }
    }

    /// Normalize a signal in place.
    pub fn normalize(&self, signal: &mut Signal) {
        if self.config.trim_whitespace {
            signal.signal_type = signal.signal_type.trim().to_string();
            signal.source = signal.source.trim().to_string();
        }

        if self.config.lowercase_strings {
            signal.signal_type = signal.signal_type.to_lowercase();
            signal.source = signal.source.to_lowercase();
        }

        // Normalize payload recursively
        signal.payload = self.normalize_value(signal.payload.clone());
    }

    fn normalize_value(&self, value: serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => {
                let mut s = s;
                if self.config.trim_whitespace {
                    s = s.trim().to_string();
                }
                if self.config.lowercase_strings {
                    s = s.to_lowercase();
                }
                serde_json::Value::String(s)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.into_iter().map(|v| self.normalize_value(v)).collect())
            }
            serde_json::Value::Object(obj) => {
                let normalized: serde_json::Map<String, serde_json::Value> = obj
                    .into_iter()
                    .map(|(k, v)| {
                        let key = if self.config.lowercase_strings {
                            k.to_lowercase()
                        } else {
                            k
                        };
                        (key, self.normalize_value(v))
                    })
                    .collect();
                serde_json::Value::Object(normalized)
            }
            serde_json::Value::Null if self.config.replace_nulls => {
                serde_json::Value::String(String::new())
            }
            other => other,
        }
    }
}

impl Default for Normalizer {
    fn default() -> Self {
        Self::new(NormalizationConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_whitespace() {
        let normalizer = Normalizer::default();
        let mut signal = Signal::new(
            "  test  ",
            serde_json::json!({"key": "  value  "}),
            "  source  ",
        );

        normalizer.normalize(&mut signal);

        assert_eq!(signal.signal_type, "test");
        assert_eq!(signal.source, "source");
        assert_eq!(signal.payload["key"], "value");
    }

    #[test]
    fn test_lowercase() {
        let config = NormalizationConfig {
            lowercase_strings: true,
            ..Default::default()
        };
        let normalizer = Normalizer::new(config);
        let mut signal = Signal::new("TEST", serde_json::json!({"KEY": "VALUE"}), "SOURCE");

        normalizer.normalize(&mut signal);

        assert_eq!(signal.signal_type, "test");
        assert_eq!(signal.source, "source");
        assert_eq!(signal.payload["key"], "value");
    }
}
