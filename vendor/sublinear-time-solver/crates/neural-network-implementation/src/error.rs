//! Error types for the temporal neural network system

use thiserror::Error;

/// Result type alias for temporal neural network operations
pub type Result<T> = std::result::Result<T, TemporalNeuralError>;

/// Comprehensive error types for the temporal neural network system
#[derive(Error, Debug, Clone)]
pub enum TemporalNeuralError {
    /// Library initialization failed
    #[error("Initialization error: {reason}")]
    InitializationError {
        /// Reason for the failure
        reason: String,
    },

    /// Invalid configuration provided
    #[error("Configuration error: {message}")]
    ConfigurationError {
        /// Error message
        message: String,
        /// Configuration field that caused the error
        field: Option<String>,
    },

    /// Data processing or validation error
    #[error("Data error: {message}")]
    DataError {
        /// Error message
        message: String,
        /// Data context (e.g., file path, sample index)
        context: Option<String>,
    },

    /// Model architecture or parameter error
    #[error("Model error in {component}: {message}")]
    ModelError {
        /// Model component that failed
        component: String,
        /// Error message
        message: String,
        /// Additional context
        context: Vec<(String, String)>,
    },

    /// Training process error
    #[error("Training error at epoch {epoch}: {message}")]
    TrainingError {
        /// Training epoch when error occurred
        epoch: usize,
        /// Error message
        message: String,
        /// Training metrics at time of failure
        metrics: Option<TrainingMetrics>,
    },

    /// Inference or prediction error
    #[error("Inference error: {message}")]
    InferenceError {
        /// Error message
        message: String,
        /// Input that caused the error
        input_shape: Option<Vec<usize>>,
        /// Latency budget exceeded flag
        latency_exceeded: bool,
    },

    /// Sublinear solver integration error
    #[error("Solver error: {message}")]
    SolverError {
        /// Error message
        message: String,
        /// Solver algorithm that failed
        algorithm: Option<String>,
        /// Certificate error if available
        certificate_error: Option<f64>,
    },

    /// Kalman filter error
    #[error("Kalman filter error: {message}")]
    KalmanError {
        /// Error message
        message: String,
        /// Filter state when error occurred
        state_dimension: Option<usize>,
    },

    /// Quantization or optimization error
    #[error("Quantization error: {message}")]
    QuantizationError {
        /// Error message
        message: String,
        /// Quantization scheme that failed
        scheme: Option<String>,
        /// Accuracy loss if measured
        accuracy_loss: Option<f64>,
    },

    /// I/O operation error
    #[error("IO error: {message}")]
    IoError {
        /// Error message
        message: String,
        /// File path if applicable
        path: Option<String>,
        /// Underlying IO error
        source: Option<std::io::Error>,
    },

    /// Serialization/deserialization error
    #[error("Serialization error: {message}")]
    SerializationError {
        /// Error message
        message: String,
        /// Format being serialized/deserialized
        format: Option<String>,
    },

    /// Numerical computation error
    #[error("Numerical error: {message}")]
    NumericalError {
        /// Error message
        message: String,
        /// Value that caused the error
        problematic_value: Option<f64>,
        /// Operation that failed
        operation: Option<String>,
    },

    /// Memory allocation or management error
    #[error("Memory error: {message}")]
    MemoryError {
        /// Error message
        message: String,
        /// Requested memory size in bytes
        requested_bytes: Option<usize>,
        /// Available memory in bytes
        available_bytes: Option<usize>,
    },

    /// Performance or latency constraint violation
    #[error("Performance error: {message}")]
    PerformanceError {
        /// Error message
        message: String,
        /// Actual latency measured
        actual_latency_ms: Option<f64>,
        /// Target latency constraint
        target_latency_ms: Option<f64>,
        /// Performance metric that was violated
        metric: Option<String>,
    },

    /// Validation or verification error
    #[error("Validation error: {message}")]
    ValidationError {
        /// Error message
        message: String,
        /// Expected value or range
        expected: Option<String>,
        /// Actual value received
        actual: Option<String>,
        /// Validation rule that failed
        rule: Option<String>,
    },

    /// External dependency error
    #[error("External error: {message}")]
    ExternalError {
        /// Error message
        message: String,
        /// External library or service
        external_source: String,
        /// Original error if available
        original_error: Option<String>,
    },

    /// Dimension mismatch error
    #[error("Dimension mismatch: {message}")]
    DimensionMismatch {
        /// Error message
        message: String,
        /// Expected dimensions
        expected: Option<String>,
        /// Actual dimensions
        actual: Option<String>,
        /// Context of the operation
        context: Option<String>,
    },
}

/// Training metrics for error reporting
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrainingMetrics {
    /// Training loss
    pub train_loss: f64,
    /// Validation loss
    pub val_loss: Option<f64>,
    /// Learning rate
    pub learning_rate: f64,
    /// Number of samples processed
    pub samples_processed: usize,
    /// Wall clock time elapsed
    pub elapsed_ms: f64,
}

impl TemporalNeuralError {
    /// Create a configuration error with field context
    pub fn config_field_error(field: &str, message: &str) -> Self {
        Self::ConfigurationError {
            message: message.to_string(),
            field: Some(field.to_string()),
        }
    }

    /// Create a data error with context
    pub fn data_context_error(message: &str, context: &str) -> Self {
        Self::DataError {
            message: message.to_string(),
            context: Some(context.to_string()),
        }
    }

    /// Create a model error with component and context
    pub fn model_component_error(component: &str, message: &str) -> Self {
        Self::ModelError {
            component: component.to_string(),
            message: message.to_string(),
            context: vec![],
        }
    }

    /// Create a training error with metrics
    pub fn training_with_metrics(epoch: usize, message: &str, metrics: TrainingMetrics) -> Self {
        Self::TrainingError {
            epoch,
            message: message.to_string(),
            metrics: Some(metrics),
        }
    }

    /// Create an inference error with latency flag
    pub fn inference_latency_error(message: &str, latency_exceeded: bool) -> Self {
        Self::InferenceError {
            message: message.to_string(),
            input_shape: None,
            latency_exceeded,
        }
    }

    /// Create a solver error with certificate context
    pub fn solver_certificate_error(message: &str, certificate_error: f64) -> Self {
        Self::SolverError {
            message: message.to_string(),
            algorithm: None,
            certificate_error: Some(certificate_error),
        }
    }

    /// Create a performance error with latency metrics
    pub fn performance_latency_error(
        message: &str,
        actual_ms: f64,
        target_ms: f64
    ) -> Self {
        Self::PerformanceError {
            message: message.to_string(),
            actual_latency_ms: Some(actual_ms),
            target_latency_ms: Some(target_ms),
            metric: Some("latency".to_string()),
        }
    }

    /// Check if error is related to latency constraints
    pub fn is_latency_error(&self) -> bool {
        match self {
            Self::InferenceError { latency_exceeded, .. } => *latency_exceeded,
            Self::PerformanceError { metric, .. } => {
                metric.as_ref().map_or(false, |m| m.contains("latency"))
            }
            _ => false,
        }
    }

    /// Check if error is recoverable (can retry)
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::InitializationError { .. } => false,
            Self::ConfigurationError { .. } => false,
            Self::DataError { .. } => false,
            Self::ModelError { .. } => false,
            Self::TrainingError { .. } => true,  // Can retry with different params
            Self::InferenceError { .. } => true, // Can retry inference
            Self::SolverError { .. } => true,    // Can fallback or retry
            Self::KalmanError { .. } => true,    // Can reset filter
            Self::QuantizationError { .. } => false,
            Self::IoError { .. } => true,        // Can retry I/O
            Self::SerializationError { .. } => false,
            Self::NumericalError { .. } => true, // Can adjust parameters
            Self::MemoryError { .. } => true,    // Can reduce batch size
            Self::PerformanceError { .. } => true, // Can optimize
            Self::ValidationError { .. } => false,
            Self::ExternalError { .. } => true,  // Can retry external calls
            Self::DimensionMismatch { .. } => false, // Usually indicates programming error
        }
    }

    /// Get error category for logging and monitoring
    pub fn category(&self) -> &'static str {
        match self {
            Self::InitializationError { .. } => "initialization",
            Self::ConfigurationError { .. } => "configuration",
            Self::DataError { .. } => "data",
            Self::ModelError { .. } => "model",
            Self::TrainingError { .. } => "training",
            Self::InferenceError { .. } => "inference",
            Self::SolverError { .. } => "solver",
            Self::KalmanError { .. } => "kalman",
            Self::QuantizationError { .. } => "quantization",
            Self::IoError { .. } => "io",
            Self::SerializationError { .. } => "serialization",
            Self::NumericalError { .. } => "numerical",
            Self::MemoryError { .. } => "memory",
            Self::PerformanceError { .. } => "performance",
            Self::ValidationError { .. } => "validation",
            Self::ExternalError { .. } => "external",
            Self::DimensionMismatch { .. } => "dimension_mismatch",
        }
    }
}

// Implement conversions from common error types
impl From<std::io::Error> for TemporalNeuralError {
    fn from(err: std::io::Error) -> Self {
        Self::IoError {
            message: err.to_string(),
            path: None,
            source: Some(err),
        }
    }
}

impl From<serde_json::Error> for TemporalNeuralError {
    fn from(err: serde_json::Error) -> Self {
        Self::SerializationError {
            message: err.to_string(),
            format: Some("json".to_string()),
        }
    }
}

impl From<csv::Error> for TemporalNeuralError {
    fn from(err: csv::Error) -> Self {
        Self::SerializationError {
            message: err.to_string(),
            format: Some("csv".to_string()),
        }
    }
}

// Temporarily comment out until sublinear integration is fixed
// impl From<sublinear::SolverError> for TemporalNeuralError {
//     fn from(err: sublinear::SolverError) -> Self {
//         Self::SolverError {
//             message: err.to_string(),
//             algorithm: None,
//             certificate_error: None,
//         }
//     }
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_categorization() {
        let config_err = TemporalNeuralError::ConfigurationError {
            message: "Invalid parameter".to_string(),
            field: Some("learning_rate".to_string()),
        };
        assert_eq!(config_err.category(), "configuration");
        assert!(!config_err.is_recoverable());
        assert!(!config_err.is_latency_error());

        let latency_err = TemporalNeuralError::inference_latency_error(
            "Exceeded budget", true
        );
        assert_eq!(latency_err.category(), "inference");
        assert!(latency_err.is_recoverable());
        assert!(latency_err.is_latency_error());
    }

    #[test]
    fn test_error_creation_helpers() {
        let err = TemporalNeuralError::config_field_error(
            "batch_size", "Must be positive"
        );
        match err {
            TemporalNeuralError::ConfigurationError { field, .. } => {
                assert_eq!(field, Some("batch_size".to_string()));
            }
            _ => panic!("Wrong error type"),
        }
    }

    #[test]
    fn test_training_metrics_serialization() {
        let metrics = TrainingMetrics {
            train_loss: 0.1,
            val_loss: Some(0.12),
            learning_rate: 1e-3,
            samples_processed: 1000,
            elapsed_ms: 5000.0,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        let deserialized: TrainingMetrics = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.train_loss, metrics.train_loss);
        assert_eq!(deserialized.val_loss, metrics.val_loss);
    }
}