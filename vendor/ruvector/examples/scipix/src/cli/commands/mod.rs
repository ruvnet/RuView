pub mod batch;
pub mod config;
pub mod doctor;
pub mod mcp;
pub mod ocr;
pub mod serve;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Common result structure for OCR operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Source file path
    pub file: PathBuf,

    /// Extracted text content
    pub text: String,

    /// LaTeX representation (if available)
    pub latex: Option<String>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,

    /// Any errors or warnings
    pub errors: Vec<String>,
}

impl OcrResult {
    /// Create a new OCR result
    pub fn new(file: PathBuf, text: String, confidence: f64) -> Self {
        Self {
            file,
            text,
            latex: None,
            confidence,
            processing_time_ms: 0,
            errors: Vec::new(),
        }
    }

    /// Set LaTeX content
    pub fn with_latex(mut self, latex: String) -> Self {
        self.latex = Some(latex);
        self
    }

    /// Set processing time
    pub fn with_processing_time(mut self, time_ms: u64) -> Self {
        self.processing_time_ms = time_ms;
        self
    }

    /// Add an error message
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }
}

/// Configuration for OCR processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Minimum confidence threshold
    pub min_confidence: f64,

    /// Maximum image size in bytes
    pub max_image_size: usize,

    /// Supported file extensions
    pub supported_extensions: Vec<String>,

    /// API endpoint (if using remote service)
    pub api_endpoint: Option<String>,

    /// API key (if using remote service)
    pub api_key: Option<String>,
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.7,
            max_image_size: 10 * 1024 * 1024, // 10MB
            supported_extensions: vec![
                "png".to_string(),
                "jpg".to_string(),
                "jpeg".to_string(),
                "pdf".to_string(),
                "gif".to_string(),
            ],
            api_endpoint: None,
            api_key: None,
        }
    }
}
