//! Signal validation.

use super::Signal;

/// Result of signal validation.
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Signal is valid
    Valid,
    /// Signal is invalid with reasons
    Invalid(Vec<String>),
}

impl ValidationResult {
    /// Check if valid.
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// Get validation errors (if any).
    pub fn errors(&self) -> &[String] {
        match self {
            Self::Invalid(errors) => errors,
            Self::Valid => &[],
        }
    }
}

/// Validator for incoming signals.
pub struct SignalValidator {
    /// Maximum payload size in bytes
    max_payload_size: usize,
    /// Allowed signal types
    allowed_types: Option<Vec<String>>,
}

impl SignalValidator {
    /// Create a new validator.
    pub fn new() -> Self {
        Self {
            max_payload_size: 1024 * 1024, // 1MB default
            allowed_types: None,
        }
    }

    /// Set maximum payload size.
    pub fn with_max_payload_size(mut self, size: usize) -> Self {
        self.max_payload_size = size;
        self
    }

    /// Set allowed signal types.
    pub fn with_allowed_types(mut self, types: Vec<String>) -> Self {
        self.allowed_types = Some(types);
        self
    }

    /// Validate a signal.
    pub fn validate(&self, signal: &Signal) -> ValidationResult {
        let mut errors = Vec::new();

        // Check payload size
        let payload_str = signal.payload.to_string();
        if payload_str.len() > self.max_payload_size {
            errors.push(format!(
                "Payload exceeds maximum size of {} bytes",
                self.max_payload_size
            ));
        }

        // Check signal type if restricted
        if let Some(ref allowed) = self.allowed_types {
            if !allowed.contains(&signal.signal_type) {
                errors.push(format!(
                    "Signal type '{}' not in allowed types",
                    signal.signal_type
                ));
            }
        }

        // Check source is not empty
        if signal.source.is_empty() {
            errors.push("Signal source cannot be empty".to_string());
        }

        if errors.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid(errors)
        }
    }
}

impl Default for SignalValidator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_signal() {
        let validator = SignalValidator::new();
        let signal = Signal::new("test", serde_json::json!({"key": "value"}), "source");

        assert!(validator.validate(&signal).is_valid());
    }

    #[test]
    fn test_empty_source() {
        let validator = SignalValidator::new();
        let mut signal = Signal::new("test", serde_json::json!({}), "source");
        signal.source = String::new();

        let result = validator.validate(&signal);
        assert!(!result.is_valid());
        assert!(result.errors()[0].contains("source"));
    }

    #[test]
    fn test_disallowed_type() {
        let validator = SignalValidator::new().with_allowed_types(vec!["allowed".to_string()]);
        let signal = Signal::new("disallowed", serde_json::json!({}), "source");

        let result = validator.validate(&signal);
        assert!(!result.is_valid());
    }
}
