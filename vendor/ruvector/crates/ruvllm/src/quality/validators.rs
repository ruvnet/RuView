//! Schema Validators for Quality Assessment
//!
//! This module provides validators for checking content against schemas,
//! types, ranges, and formats with combinatorial logic support.

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::fmt;

/// Result of a validation operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    /// Whether validation passed
    pub is_valid: bool,
    /// Score from 0.0 to 1.0 representing compliance level
    pub compliance_score: f32,
    /// List of validation errors
    pub errors: Vec<ValidationError>,
    /// List of warnings (non-fatal issues)
    pub warnings: Vec<String>,
    /// Number of checks performed
    pub checks_performed: usize,
    /// Number of checks passed
    pub checks_passed: usize,
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self {
            is_valid: true,
            compliance_score: 1.0,
            errors: Vec::new(),
            warnings: Vec::new(),
            checks_performed: 0,
            checks_passed: 0,
        }
    }
}

impl ValidationResult {
    /// Create a successful validation result
    pub fn success() -> Self {
        Self::default()
    }

    /// Create a failed validation result with a single error
    pub fn failure(error: ValidationError) -> Self {
        Self {
            is_valid: false,
            compliance_score: 0.0,
            errors: vec![error],
            warnings: Vec::new(),
            checks_performed: 1,
            checks_passed: 0,
        }
    }

    /// Merge two validation results
    pub fn merge(&mut self, other: ValidationResult) {
        self.is_valid = self.is_valid && other.is_valid;
        self.errors.extend(other.errors);
        self.warnings.extend(other.warnings);
        self.checks_performed += other.checks_performed;
        self.checks_passed += other.checks_passed;

        // Recalculate compliance score
        if self.checks_performed > 0 {
            self.compliance_score = self.checks_passed as f32 / self.checks_performed as f32;
        }
    }

    /// Add a check result
    pub fn add_check(&mut self, passed: bool, error: Option<ValidationError>) {
        self.checks_performed += 1;
        if passed {
            self.checks_passed += 1;
        } else {
            self.is_valid = false;
            if let Some(err) = error {
                self.errors.push(err);
            }
        }
        self.compliance_score = self.checks_passed as f32 / self.checks_performed as f32;
    }

    /// Add a warning
    pub fn add_warning(&mut self, warning: String) {
        self.warnings.push(warning);
    }
}

/// Validation error with context
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Type of validation error
    pub error_type: ValidationErrorType,
    /// Path to the field that failed validation (e.g., "data.items\[0\].name")
    pub path: String,
    /// Human-readable error message
    pub message: String,
    /// Expected value or pattern (if applicable)
    pub expected: Option<String>,
    /// Actual value found (if applicable)
    pub actual: Option<String>,
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}: {}", self.error_type, self.path, self.message)?;
        if let Some(ref expected) = self.expected {
            write!(f, " (expected: {})", expected)?;
        }
        if let Some(ref actual) = self.actual {
            write!(f, " (got: {})", actual)?;
        }
        Ok(())
    }
}

/// Types of validation errors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationErrorType {
    /// Missing required field
    MissingField,
    /// Unexpected field present
    UnexpectedField,
    /// Type mismatch
    TypeMismatch,
    /// Value out of range
    OutOfRange,
    /// Format validation failed
    InvalidFormat,
    /// Schema structure mismatch
    SchemaMismatch,
    /// Custom validation rule failed
    CustomRule,
    /// Constraint violation
    ConstraintViolation,
}

impl fmt::Display for ValidationErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField => write!(f, "MISSING"),
            Self::UnexpectedField => write!(f, "UNEXPECTED"),
            Self::TypeMismatch => write!(f, "TYPE"),
            Self::OutOfRange => write!(f, "RANGE"),
            Self::InvalidFormat => write!(f, "FORMAT"),
            Self::SchemaMismatch => write!(f, "SCHEMA"),
            Self::CustomRule => write!(f, "CUSTOM"),
            Self::ConstraintViolation => write!(f, "CONSTRAINT"),
        }
    }
}

/// Trait for schema validators
pub trait SchemaValidator: Send + Sync {
    /// Validate content against the schema
    fn validate(&self, content: &JsonValue) -> ValidationResult;

    /// Get validator name for debugging
    fn name(&self) -> &str;
}

/// JSON Schema validator implementation
#[derive(Debug, Clone)]
pub struct JsonSchemaValidator {
    /// Schema definition
    schema: JsonValue,
    /// Required fields at root level
    required_fields: HashSet<String>,
    /// Strict mode (reject unknown fields)
    strict_mode: bool,
}

impl JsonSchemaValidator {
    /// Create a new JSON schema validator
    pub fn new(schema: JsonValue) -> Self {
        let required_fields = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        Self {
            schema,
            required_fields,
            strict_mode: false,
        }
    }

    /// Enable strict mode (reject unknown fields)
    pub fn with_strict_mode(mut self, strict: bool) -> Self {
        self.strict_mode = strict;
        self
    }

    /// Create from a simple field specification
    pub fn from_fields(fields: &[(&str, &str)]) -> Self {
        let mut properties = serde_json::Map::new();
        let mut required = Vec::new();

        for (name, type_str) in fields {
            let type_value = serde_json::json!({ "type": type_str });
            properties.insert(name.to_string(), type_value);
            required.push(serde_json::json!(name));
        }

        let schema = serde_json::json!({
            "type": "object",
            "properties": properties,
            "required": required
        });

        Self::new(schema)
    }

    /// Validate a value against a type specification
    fn validate_type(
        &self,
        value: &JsonValue,
        expected_type: &str,
        path: &str,
    ) -> ValidationResult {
        let mut result = ValidationResult::default();
        result.checks_performed = 1;

        let is_valid = match expected_type {
            "string" => value.is_string(),
            "number" => value.is_number(),
            "integer" => value.is_i64() || value.is_u64(),
            "boolean" => value.is_boolean(),
            "array" => value.is_array(),
            "object" => value.is_object(),
            "null" => value.is_null(),
            _ => true, // Unknown type, pass
        };

        if is_valid {
            result.checks_passed = 1;
            result.compliance_score = 1.0;
        } else {
            result.is_valid = false;
            result.compliance_score = 0.0;
            result.errors.push(ValidationError {
                error_type: ValidationErrorType::TypeMismatch,
                path: path.to_string(),
                message: format!(
                    "Expected type '{}', got '{}'",
                    expected_type,
                    value_type_name(value)
                ),
                expected: Some(expected_type.to_string()),
                actual: Some(value_type_name(value).to_string()),
            });
        }

        result
    }

    /// Recursively validate against schema
    fn validate_against_schema(
        &self,
        value: &JsonValue,
        schema: &JsonValue,
        path: &str,
    ) -> ValidationResult {
        let mut result = ValidationResult::default();

        // Check type constraint
        if let Some(type_val) = schema.get("type") {
            if let Some(type_str) = type_val.as_str() {
                result.merge(self.validate_type(value, type_str, path));
            }
        }

        // For objects, check properties
        if let (Some(obj), Some(props)) = (value.as_object(), schema.get("properties")) {
            if let Some(props_obj) = props.as_object() {
                // Check each defined property
                for (key, prop_schema) in props_obj {
                    let prop_path = if path.is_empty() {
                        key.clone()
                    } else {
                        format!("{}.{}", path, key)
                    };

                    if let Some(prop_value) = obj.get(key) {
                        result.merge(self.validate_against_schema(
                            prop_value,
                            prop_schema,
                            &prop_path,
                        ));
                    }
                }

                // Check required fields
                if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
                    for req in required {
                        if let Some(req_name) = req.as_str() {
                            result.checks_performed += 1;
                            if obj.contains_key(req_name) {
                                result.checks_passed += 1;
                            } else {
                                result.is_valid = false;
                                result.errors.push(ValidationError {
                                    error_type: ValidationErrorType::MissingField,
                                    path: format!("{}.{}", path, req_name),
                                    message: format!("Required field '{}' is missing", req_name),
                                    expected: Some(req_name.to_string()),
                                    actual: None,
                                });
                            }
                        }
                    }
                }

                // Strict mode: check for unexpected fields
                if self.strict_mode {
                    for key in obj.keys() {
                        if !props_obj.contains_key(key) {
                            result.add_warning(format!("Unexpected field '{}' in {}", key, path));
                        }
                    }
                }
            }
        }

        // For arrays, check items
        if let (Some(arr), Some(items_schema)) = (value.as_array(), schema.get("items")) {
            for (i, item) in arr.iter().enumerate() {
                let item_path = format!("{}[{}]", path, i);
                result.merge(self.validate_against_schema(item, items_schema, &item_path));
            }
        }

        // Recalculate compliance score
        if result.checks_performed > 0 {
            result.compliance_score = result.checks_passed as f32 / result.checks_performed as f32;
        }

        result
    }
}

impl SchemaValidator for JsonSchemaValidator {
    fn validate(&self, content: &JsonValue) -> ValidationResult {
        self.validate_against_schema(content, &self.schema, "")
    }

    fn name(&self) -> &str {
        "JsonSchemaValidator"
    }
}

/// Type validator for checking value types
#[derive(Debug, Clone)]
pub struct TypeValidator {
    /// Expected type name
    expected_type: String,
    /// Allow null values
    allow_null: bool,
}

impl TypeValidator {
    /// Create a new type validator
    pub fn new(expected_type: &str) -> Self {
        Self {
            expected_type: expected_type.to_string(),
            allow_null: false,
        }
    }

    /// Allow null values
    pub fn allow_null(mut self) -> Self {
        self.allow_null = true;
        self
    }
}

impl SchemaValidator for TypeValidator {
    fn validate(&self, content: &JsonValue) -> ValidationResult {
        let mut result = ValidationResult::default();
        result.checks_performed = 1;

        if self.allow_null && content.is_null() {
            result.checks_passed = 1;
            return result;
        }

        let is_valid = match self.expected_type.as_str() {
            "string" => content.is_string(),
            "number" => content.is_number(),
            "integer" => content.is_i64() || content.is_u64(),
            "boolean" => content.is_boolean(),
            "array" => content.is_array(),
            "object" => content.is_object(),
            "null" => content.is_null(),
            _ => true,
        };

        if is_valid {
            result.checks_passed = 1;
        } else {
            result.is_valid = false;
            result.errors.push(ValidationError {
                error_type: ValidationErrorType::TypeMismatch,
                path: String::new(),
                message: format!(
                    "Expected type '{}', got '{}'",
                    self.expected_type,
                    value_type_name(content)
                ),
                expected: Some(self.expected_type.clone()),
                actual: Some(value_type_name(content).to_string()),
            });
        }

        result.compliance_score = result.checks_passed as f32 / result.checks_performed as f32;
        result
    }

    fn name(&self) -> &str {
        "TypeValidator"
    }
}

/// Range validator for numeric values
#[derive(Debug, Clone)]
pub struct RangeValidator {
    /// Minimum value (inclusive)
    min: Option<f64>,
    /// Maximum value (inclusive)
    max: Option<f64>,
    /// Allow exclusive bounds
    exclusive_min: bool,
    exclusive_max: bool,
}

impl RangeValidator {
    /// Create a new range validator
    pub fn new() -> Self {
        Self {
            min: None,
            max: None,
            exclusive_min: false,
            exclusive_max: false,
        }
    }

    /// Set minimum value (inclusive)
    pub fn min(mut self, min: f64) -> Self {
        self.min = Some(min);
        self
    }

    /// Set maximum value (inclusive)
    pub fn max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Set minimum value (exclusive)
    pub fn min_exclusive(mut self, min: f64) -> Self {
        self.min = Some(min);
        self.exclusive_min = true;
        self
    }

    /// Set maximum value (exclusive)
    pub fn max_exclusive(mut self, max: f64) -> Self {
        self.max = Some(max);
        self.exclusive_max = true;
        self
    }

    /// Create a range from min to max (inclusive)
    pub fn range(min: f64, max: f64) -> Self {
        Self::new().min(min).max(max)
    }
}

impl Default for RangeValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl SchemaValidator for RangeValidator {
    fn validate(&self, content: &JsonValue) -> ValidationResult {
        let mut result = ValidationResult::default();
        result.checks_performed = 1;

        let value = match content.as_f64() {
            Some(v) => v,
            None => {
                result.is_valid = false;
                result.errors.push(ValidationError {
                    error_type: ValidationErrorType::TypeMismatch,
                    path: String::new(),
                    message: "Expected numeric value".to_string(),
                    expected: Some("number".to_string()),
                    actual: Some(value_type_name(content).to_string()),
                });
                return result;
            }
        };

        // Check minimum
        if let Some(min) = self.min {
            let min_ok = if self.exclusive_min {
                value > min
            } else {
                value >= min
            };
            if !min_ok {
                result.is_valid = false;
                result.errors.push(ValidationError {
                    error_type: ValidationErrorType::OutOfRange,
                    path: String::new(),
                    message: format!(
                        "Value {} is {} minimum {}",
                        value,
                        if self.exclusive_min {
                            "not greater than"
                        } else {
                            "less than"
                        },
                        min
                    ),
                    expected: Some(format!(
                        "{} {}",
                        if self.exclusive_min { ">" } else { ">=" },
                        min
                    )),
                    actual: Some(value.to_string()),
                });
            }
        }

        // Check maximum
        if let Some(max) = self.max {
            let max_ok = if self.exclusive_max {
                value < max
            } else {
                value <= max
            };
            if !max_ok {
                result.is_valid = false;
                result.errors.push(ValidationError {
                    error_type: ValidationErrorType::OutOfRange,
                    path: String::new(),
                    message: format!(
                        "Value {} is {} maximum {}",
                        value,
                        if self.exclusive_max {
                            "not less than"
                        } else {
                            "greater than"
                        },
                        max
                    ),
                    expected: Some(format!(
                        "{} {}",
                        if self.exclusive_max { "<" } else { "<=" },
                        max
                    )),
                    actual: Some(value.to_string()),
                });
            }
        }

        if result.is_valid {
            result.checks_passed = 1;
        }
        result.compliance_score = result.checks_passed as f32 / result.checks_performed as f32;
        result
    }

    fn name(&self) -> &str {
        "RangeValidator"
    }
}

/// Format validator using regex patterns
#[derive(Debug, Clone)]
pub struct FormatValidator {
    /// Format name (e.g., "email", "url", "uuid")
    format_name: String,
    /// Regex pattern
    pattern: String,
}

impl FormatValidator {
    /// Create a new format validator with custom pattern
    pub fn new(format_name: &str, pattern: &str) -> Self {
        Self {
            format_name: format_name.to_string(),
            pattern: pattern.to_string(),
        }
    }

    /// Create email format validator
    pub fn email() -> Self {
        Self::new("email", r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$")
    }

    /// Create URL format validator
    pub fn url() -> Self {
        Self::new("url", r"^https?://[^\s/$.?#].[^\s]*$")
    }

    /// Create UUID format validator
    pub fn uuid() -> Self {
        Self::new(
            "uuid",
            r"^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$",
        )
    }

    /// Create date format validator (ISO 8601)
    pub fn date() -> Self {
        Self::new("date", r"^\d{4}-\d{2}-\d{2}$")
    }

    /// Create datetime format validator (ISO 8601)
    pub fn datetime() -> Self {
        Self::new("datetime", r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}")
    }

    /// Create phone format validator (basic)
    pub fn phone() -> Self {
        Self::new("phone", r"^\+?[0-9]{10,15}$")
    }

    /// Simple pattern matching without regex dependency
    fn simple_match(&self, value: &str) -> bool {
        match self.format_name.as_str() {
            "email" => {
                // Simple email validation
                let parts: Vec<&str> = value.split('@').collect();
                if parts.len() != 2 {
                    return false;
                }
                let local = parts[0];
                let domain = parts[1];
                !local.is_empty()
                    && !domain.is_empty()
                    && domain.contains('.')
                    && domain
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '.' || c == '-')
            }
            "url" => value.starts_with("http://") || value.starts_with("https://"),
            "uuid" => {
                // UUID format: 8-4-4-4-12 hex digits
                let parts: Vec<&str> = value.split('-').collect();
                parts.len() == 5
                    && parts[0].len() == 8
                    && parts[1].len() == 4
                    && parts[2].len() == 4
                    && parts[3].len() == 4
                    && parts[4].len() == 12
                    && parts
                        .iter()
                        .all(|p| p.chars().all(|c| c.is_ascii_hexdigit()))
            }
            "date" => {
                // ISO 8601 date: YYYY-MM-DD
                let parts: Vec<&str> = value.split('-').collect();
                parts.len() == 3
                    && parts[0].len() == 4
                    && parts[1].len() == 2
                    && parts[2].len() == 2
                    && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()))
            }
            "datetime" => {
                // ISO 8601 datetime starts with date
                value.len() >= 19
                    && value.chars().nth(4) == Some('-')
                    && value.chars().nth(7) == Some('-')
                    && value.chars().nth(10) == Some('T')
            }
            "phone" => {
                let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
                let has_plus = value.starts_with('+');
                let digit_count = digits.len();
                (!has_plus || value.len() == 1 + digit_count)
                    && digit_count >= 10
                    && digit_count <= 15
            }
            _ => true, // Unknown format, assume valid
        }
    }
}

impl SchemaValidator for FormatValidator {
    fn validate(&self, content: &JsonValue) -> ValidationResult {
        let mut result = ValidationResult::default();
        result.checks_performed = 1;

        let value = match content.as_str() {
            Some(s) => s,
            None => {
                result.is_valid = false;
                result.errors.push(ValidationError {
                    error_type: ValidationErrorType::TypeMismatch,
                    path: String::new(),
                    message: "Expected string value for format validation".to_string(),
                    expected: Some("string".to_string()),
                    actual: Some(value_type_name(content).to_string()),
                });
                return result;
            }
        };

        // Use simple pattern validation (avoiding regex dependency)
        let is_valid = self.simple_match(value);

        if is_valid {
            result.checks_passed = 1;
        } else {
            result.is_valid = false;
            result.errors.push(ValidationError {
                error_type: ValidationErrorType::InvalidFormat,
                path: String::new(),
                message: format!("Value does not match {} format", self.format_name),
                expected: Some(self.format_name.clone()),
                actual: Some(value.to_string()),
            });
        }

        result.compliance_score = result.checks_passed as f32 / result.checks_performed as f32;
        result
    }

    fn name(&self) -> &str {
        "FormatValidator"
    }
}

/// Combinator for combining validators
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationCombinator {
    /// All validators must pass
    And,
    /// At least one validator must pass
    Or,
}

/// Combined validator that uses multiple validators
pub struct CombinedValidator {
    /// Validators to combine
    validators: Vec<Box<dyn SchemaValidator>>,
    /// How to combine results
    combinator: ValidationCombinator,
}

impl CombinedValidator {
    /// Create a new combined validator with AND logic
    pub fn all(validators: Vec<Box<dyn SchemaValidator>>) -> Self {
        Self {
            validators,
            combinator: ValidationCombinator::And,
        }
    }

    /// Create a new combined validator with OR logic
    pub fn any(validators: Vec<Box<dyn SchemaValidator>>) -> Self {
        Self {
            validators,
            combinator: ValidationCombinator::Or,
        }
    }

    /// Add a validator
    pub fn add(mut self, validator: Box<dyn SchemaValidator>) -> Self {
        self.validators.push(validator);
        self
    }
}

impl SchemaValidator for CombinedValidator {
    fn validate(&self, content: &JsonValue) -> ValidationResult {
        match self.combinator {
            ValidationCombinator::And => {
                let mut combined = ValidationResult::success();
                for validator in &self.validators {
                    let result = validator.validate(content);
                    combined.merge(result);
                    // Short-circuit if AND fails
                    if !combined.is_valid {
                        break;
                    }
                }
                combined
            }
            ValidationCombinator::Or => {
                let mut any_passed = false;
                let mut combined = ValidationResult::default();
                combined.checks_performed = self.validators.len();

                for validator in &self.validators {
                    let result = validator.validate(content);
                    if result.is_valid {
                        any_passed = true;
                        combined.checks_passed += 1;
                    }
                }

                combined.is_valid = any_passed;
                combined.compliance_score = if combined.checks_performed > 0 {
                    combined.checks_passed as f32 / combined.checks_performed as f32
                } else {
                    1.0
                };
                combined
            }
        }
    }

    fn name(&self) -> &str {
        match self.combinator {
            ValidationCombinator::And => "CombinedValidator(AND)",
            ValidationCombinator::Or => "CombinedValidator(OR)",
        }
    }
}

/// Helper function to get type name from JSON value
fn value_type_name(value: &JsonValue) -> &'static str {
    match value {
        JsonValue::Null => "null",
        JsonValue::Bool(_) => "boolean",
        JsonValue::Number(_) => "number",
        JsonValue::String(_) => "string",
        JsonValue::Array(_) => "array",
        JsonValue::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_json_schema_validator() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            },
            "required": ["name"]
        });

        let validator = JsonSchemaValidator::new(schema);

        // Valid content
        let valid = json!({ "name": "Alice", "age": 30 });
        let result = validator.validate(&valid);
        assert!(result.is_valid);
        assert!(result.compliance_score > 0.9);

        // Missing required field
        let invalid = json!({ "age": 30 });
        let result = validator.validate(&invalid);
        assert!(!result.is_valid);
        assert!(!result.errors.is_empty());
    }

    #[test]
    fn test_type_validator() {
        let validator = TypeValidator::new("string");

        let valid = json!("hello");
        assert!(validator.validate(&valid).is_valid);

        let invalid = json!(123);
        assert!(!validator.validate(&invalid).is_valid);
    }

    #[test]
    fn test_range_validator() {
        let validator = RangeValidator::range(0.0, 100.0);

        assert!(validator.validate(&json!(50)).is_valid);
        assert!(validator.validate(&json!(0)).is_valid);
        assert!(validator.validate(&json!(100)).is_valid);
        assert!(!validator.validate(&json!(-1)).is_valid);
        assert!(!validator.validate(&json!(101)).is_valid);
    }

    #[test]
    fn test_exclusive_range() {
        let validator = RangeValidator::new().min_exclusive(0.0).max_exclusive(10.0);

        assert!(validator.validate(&json!(5)).is_valid);
        assert!(!validator.validate(&json!(0)).is_valid);
        assert!(!validator.validate(&json!(10)).is_valid);
    }

    #[test]
    fn test_format_validator_email() {
        let validator = FormatValidator::email();

        assert!(validator.validate(&json!("test@example.com")).is_valid);
        assert!(!validator.validate(&json!("not-an-email")).is_valid);
    }

    #[test]
    fn test_format_validator_uuid() {
        let validator = FormatValidator::uuid();

        assert!(
            validator
                .validate(&json!("550e8400-e29b-41d4-a716-446655440000"))
                .is_valid
        );
        assert!(!validator.validate(&json!("not-a-uuid")).is_valid);
    }

    #[test]
    fn test_combined_validator_and() {
        let validators: Vec<Box<dyn SchemaValidator>> = vec![
            Box::new(TypeValidator::new("number")),
            Box::new(RangeValidator::range(0.0, 100.0)),
        ];

        let combined = CombinedValidator::all(validators);

        assert!(combined.validate(&json!(50)).is_valid);
        assert!(!combined.validate(&json!("50")).is_valid); // Type fails
        assert!(!combined.validate(&json!(150)).is_valid); // Range fails
    }

    #[test]
    fn test_combined_validator_or() {
        let validators: Vec<Box<dyn SchemaValidator>> = vec![
            Box::new(TypeValidator::new("string")),
            Box::new(TypeValidator::new("number")),
        ];

        let combined = CombinedValidator::any(validators);

        assert!(combined.validate(&json!("hello")).is_valid);
        assert!(combined.validate(&json!(123)).is_valid);
        assert!(!combined.validate(&json!(true)).is_valid);
    }

    #[test]
    fn test_validation_result_merge() {
        let mut result1 = ValidationResult::success();
        result1.checks_performed = 2;
        result1.checks_passed = 2;

        let mut result2 = ValidationResult::default();
        result2.checks_performed = 3;
        result2.checks_passed = 2;
        result2.is_valid = false;
        result2.errors.push(ValidationError {
            error_type: ValidationErrorType::TypeMismatch,
            path: "test".to_string(),
            message: "Test error".to_string(),
            expected: None,
            actual: None,
        });

        result1.merge(result2);

        assert!(!result1.is_valid);
        assert_eq!(result1.checks_performed, 5);
        assert_eq!(result1.checks_passed, 4);
        assert_eq!(result1.errors.len(), 1);
    }

    #[test]
    fn test_from_fields() {
        let validator =
            JsonSchemaValidator::from_fields(&[("name", "string"), ("count", "integer")]);

        let valid = json!({ "name": "test", "count": 5 });
        assert!(validator.validate(&valid).is_valid);

        let invalid = json!({ "name": "test" }); // missing count
        assert!(!validator.validate(&invalid).is_valid);
    }
}
