// Error handling tests for ruvector-scipix
//
// Tests error types, conversions, display messages, and retry logic.
// Target: 95%+ coverage of error handling code

#[cfg(test)]
mod error_tests {
    use std::fmt;
    use std::io;

    // Mock error types for testing
    #[derive(Debug, Clone, PartialEq)]
    enum ScipixError {
        // Image errors
        InvalidImageFormat(String),
        ImageTooLarge { size: u64, max: u64 },
        ImagePreprocessingFailed(String),
        ImageLoadError(String),

        // Model errors
        ModelNotFound(String),
        ModelLoadError(String),
        InferenceError(String),

        // OCR errors
        TextDetectionFailed(String),
        TextRecognitionFailed(String),
        LowConfidence { score: f32, threshold: f32 },

        // Math parsing errors
        ParseError(String),
        InvalidExpression(String),

        // I/O errors
        IoError(String),

        // API errors
        ApiError { status: u16, message: String },
        RateLimitExceeded,

        // System errors
        Timeout(std::time::Duration),
        OutOfMemory,
        Internal(String),
    }

    impl fmt::Display for ScipixError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::InvalidImageFormat(format) => {
                    write!(f, "Invalid image format: {}", format)
                }
                Self::ImageTooLarge { size, max } => {
                    write!(f, "Image too large: {} bytes (max: {} bytes)", size, max)
                }
                Self::ImagePreprocessingFailed(reason) => {
                    write!(f, "Image preprocessing failed: {}", reason)
                }
                Self::ImageLoadError(msg) => write!(f, "Failed to load image: {}", msg),
                Self::ModelNotFound(model) => write!(f, "Model not found: {}", model),
                Self::ModelLoadError(msg) => write!(f, "Failed to load model: {}", msg),
                Self::InferenceError(msg) => write!(f, "Model inference failed: {}", msg),
                Self::TextDetectionFailed(msg) => write!(f, "Text detection failed: {}", msg),
                Self::TextRecognitionFailed(msg) => {
                    write!(f, "Text recognition failed: {}", msg)
                }
                Self::LowConfidence { score, threshold } => write!(
                    f,
                    "Low confidence score: {:.2} (threshold: {:.2})",
                    score, threshold
                ),
                Self::ParseError(msg) => write!(f, "Parse error: {}", msg),
                Self::InvalidExpression(expr) => write!(f, "Invalid expression: {}", expr),
                Self::IoError(msg) => write!(f, "I/O error: {}", msg),
                Self::ApiError { status, message } => {
                    write!(f, "API error {}: {}", status, message)
                }
                Self::RateLimitExceeded => write!(f, "Rate limit exceeded"),
                Self::Timeout(duration) => write!(f, "Operation timed out after {:?}", duration),
                Self::OutOfMemory => write!(f, "Out of memory"),
                Self::Internal(msg) => write!(f, "Internal error: {}", msg),
            }
        }
    }

    impl std::error::Error for ScipixError {}

    impl From<io::Error> for ScipixError {
        fn from(err: io::Error) -> Self {
            Self::IoError(err.to_string())
        }
    }

    impl ScipixError {
        fn is_retryable(&self) -> bool {
            matches!(
                self,
                Self::Timeout(_)
                    | Self::ApiError { status: 503, .. }
                    | Self::ApiError { status: 429, .. }
                    | Self::InferenceError(_)
            )
        }

        fn status_code(&self) -> Option<u16> {
            match self {
                Self::InvalidImageFormat(_) => Some(400),
                Self::ImageTooLarge { .. } => Some(413),
                Self::ModelNotFound(_) => Some(404),
                Self::RateLimitExceeded => Some(429),
                Self::ApiError { status, .. } => Some(*status),
                Self::Timeout(_) => Some(408),
                Self::OutOfMemory => Some(507),
                _ => Some(500),
            }
        }
    }

    #[test]
    fn test_error_creation() {
        let err = ScipixError::InvalidImageFormat("svg".to_string());
        assert_eq!(
            err,
            ScipixError::InvalidImageFormat("svg".to_string())
        );
    }

    #[test]
    fn test_error_display_invalid_format() {
        let err = ScipixError::InvalidImageFormat("svg".to_string());
        assert_eq!(err.to_string(), "Invalid image format: svg");
    }

    #[test]
    fn test_error_display_image_too_large() {
        let err = ScipixError::ImageTooLarge {
            size: 15_000_000,
            max: 10_000_000,
        };
        assert_eq!(
            err.to_string(),
            "Image too large: 15000000 bytes (max: 10000000 bytes)"
        );
    }

    #[test]
    fn test_error_display_low_confidence() {
        let err = ScipixError::LowConfidence {
            score: 0.65,
            threshold: 0.8,
        };
        assert_eq!(
            err.to_string(),
            "Low confidence score: 0.65 (threshold: 0.80)"
        );
    }

    #[test]
    fn test_error_display_api_error() {
        let err = ScipixError::ApiError {
            status: 404,
            message: "Not found".to_string(),
        };
        assert_eq!(err.to_string(), "API error 404: Not found");
    }

    #[test]
    fn test_error_display_timeout() {
        let err = ScipixError::Timeout(std::time::Duration::from_secs(30));
        assert_eq!(err.to_string(), "Operation timed out after 30s");
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let scipix_err: ScipixError = io_err.into();

        match scipix_err {
            ScipixError::IoError(msg) => assert!(msg.contains("file not found")),
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_is_retryable_timeout() {
        let err = ScipixError::Timeout(std::time::Duration::from_secs(10));
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_503() {
        let err = ScipixError::ApiError {
            status: 503,
            message: "Service Unavailable".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_retryable_429() {
        let err = ScipixError::ApiError {
            status: 429,
            message: "Too Many Requests".to_string(),
        };
        assert!(err.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_404() {
        let err = ScipixError::ApiError {
            status: 404,
            message: "Not Found".to_string(),
        };
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_is_not_retryable_invalid_format() {
        let err = ScipixError::InvalidImageFormat("svg".to_string());
        assert!(!err.is_retryable());
    }

    #[test]
    fn test_status_code_invalid_format() {
        let err = ScipixError::InvalidImageFormat("svg".to_string());
        assert_eq!(err.status_code(), Some(400));
    }

    #[test]
    fn test_status_code_image_too_large() {
        let err = ScipixError::ImageTooLarge {
            size: 15_000_000,
            max: 10_000_000,
        };
        assert_eq!(err.status_code(), Some(413));
    }

    #[test]
    fn test_status_code_not_found() {
        let err = ScipixError::ModelNotFound("model.onnx".to_string());
        assert_eq!(err.status_code(), Some(404));
    }

    #[test]
    fn test_status_code_rate_limit() {
        let err = ScipixError::RateLimitExceeded;
        assert_eq!(err.status_code(), Some(429));
    }

    #[test]
    fn test_status_code_timeout() {
        let err = ScipixError::Timeout(std::time::Duration::from_secs(30));
        assert_eq!(err.status_code(), Some(408));
    }

    #[test]
    fn test_status_code_out_of_memory() {
        let err = ScipixError::OutOfMemory;
        assert_eq!(err.status_code(), Some(507));
    }

    #[test]
    fn test_status_code_internal() {
        let err = ScipixError::Internal("something went wrong".to_string());
        assert_eq!(err.status_code(), Some(500));
    }

    #[test]
    fn test_error_cloning() {
        let err1 = ScipixError::InvalidImageFormat("svg".to_string());
        let err2 = err1.clone();
        assert_eq!(err1, err2);
    }

    #[test]
    fn test_multiple_error_types() {
        let errors = vec![
            ScipixError::InvalidImageFormat("svg".to_string()),
            ScipixError::ImageTooLarge {
                size: 15_000_000,
                max: 10_000_000,
            },
            ScipixError::ModelNotFound("model.onnx".to_string()),
            ScipixError::RateLimitExceeded,
            ScipixError::Timeout(std::time::Duration::from_secs(30)),
        ];

        assert_eq!(errors.len(), 5);
        for err in &errors {
            assert!(!err.to_string().is_empty());
        }
    }

    #[test]
    fn test_error_categorization() {
        let image_errors = vec![
            ScipixError::InvalidImageFormat("svg".to_string()),
            ScipixError::ImageTooLarge {
                size: 15_000_000,
                max: 10_000_000,
            },
            ScipixError::ImagePreprocessingFailed("deskew failed".to_string()),
        ];

        for err in &image_errors {
            match err {
                ScipixError::InvalidImageFormat(_)
                | ScipixError::ImageTooLarge { .. }
                | ScipixError::ImagePreprocessingFailed(_) => {
                    // Image-related errors
                    assert!(err.status_code().is_some());
                }
                _ => panic!("Expected image error"),
            }
        }
    }

    #[test]
    fn test_retryable_errors_collection() {
        let errors = vec![
            ScipixError::Timeout(std::time::Duration::from_secs(30)),
            ScipixError::ApiError {
                status: 503,
                message: "Service Unavailable".to_string(),
            },
            ScipixError::InferenceError("temporary failure".to_string()),
        ];

        let retryable_count = errors.iter().filter(|e| e.is_retryable()).count();
        assert_eq!(retryable_count, 3);
    }

    #[test]
    fn test_non_retryable_errors_collection() {
        let errors = vec![
            ScipixError::InvalidImageFormat("svg".to_string()),
            ScipixError::ModelNotFound("model.onnx".to_string()),
            ScipixError::ParseError("invalid latex".to_string()),
        ];

        let retryable_count = errors.iter().filter(|e| e.is_retryable()).count();
        assert_eq!(retryable_count, 0);
    }
}
