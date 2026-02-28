//! OCR Engine Module
//!
//! This module provides optical character recognition capabilities for the ruvector-scipix system.
//! It supports text detection, character recognition, and mathematical expression recognition using
//! ONNX models for high-performance inference.
//!
//! # Architecture
//!
//! The OCR module is organized into several submodules:
//! - `engine`: Main OcrEngine for orchestrating OCR operations
//! - `models`: Model management, loading, and caching
//! - `inference`: ONNX inference operations for detection and recognition
//! - `decoder`: Output decoding strategies (beam search, greedy, CTC)
//! - `confidence`: Confidence scoring and calibration
//!
//! # Example
//!
//! ```no_run
//! use ruvector_scipix::ocr::{OcrEngine, OcrOptions};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize the OCR engine
//! let engine = OcrEngine::new().await?;
//!
//! // Load an image
//! let image_data = std::fs::read("math_formula.png")?;
//!
//! // Perform OCR
//! let result = engine.recognize(&image_data).await?;
//!
//! println!("Recognized text: {}", result.text);
//! println!("Confidence: {:.2}%", result.confidence * 100.0);
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Submodules
mod confidence;
mod decoder;
mod engine;
mod inference;
mod models;

// Public exports
pub use confidence::{aggregate_confidence, calculate_confidence, ConfidenceCalibrator};
pub use decoder::{BeamSearchDecoder, CTCDecoder, Decoder, GreedyDecoder, Vocabulary};
pub use engine::{OcrEngine, OcrProcessor};
pub use inference::{DetectionResult, InferenceEngine, RecognitionResult};
pub use models::{ModelHandle, ModelRegistry};

/// OCR processing options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrOptions {
    /// Detection threshold for text regions (0.0-1.0)
    pub detection_threshold: f32,

    /// Recognition confidence threshold (0.0-1.0)
    pub recognition_threshold: f32,

    /// Enable mathematical expression recognition
    pub enable_math: bool,

    /// Decoder type to use
    pub decoder_type: DecoderType,

    /// Beam width for beam search decoder
    pub beam_width: usize,

    /// Maximum batch size for inference
    pub batch_size: usize,

    /// Enable GPU acceleration if available
    pub use_gpu: bool,

    /// Language hints for recognition
    pub languages: Vec<String>,
}

impl Default for OcrOptions {
    fn default() -> Self {
        Self {
            detection_threshold: 0.5,
            recognition_threshold: 0.6,
            enable_math: true,
            decoder_type: DecoderType::BeamSearch,
            beam_width: 5,
            batch_size: 1,
            use_gpu: false,
            languages: vec!["en".to_string()],
        }
    }
}

/// Decoder type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecoderType {
    /// Beam search decoder (higher quality, slower)
    BeamSearch,
    /// Greedy decoder (faster, lower quality)
    Greedy,
    /// CTC decoder for sequence-to-sequence models
    CTC,
}

/// OCR result containing recognized text and metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResult {
    /// Recognized text
    pub text: String,

    /// Overall confidence score (0.0-1.0)
    pub confidence: f32,

    /// Detected text regions with their bounding boxes
    pub regions: Vec<TextRegion>,

    /// Whether mathematical expressions were detected
    pub has_math: bool,

    /// Processing time in milliseconds
    pub processing_time_ms: u64,
}

/// A detected text region with position and content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextRegion {
    /// Bounding box coordinates [x, y, width, height]
    pub bbox: [f32; 4],

    /// Recognized text in this region
    pub text: String,

    /// Confidence score for this region (0.0-1.0)
    pub confidence: f32,

    /// Region type (text, math, etc.)
    pub region_type: RegionType,

    /// Character-level details if available
    pub characters: Vec<Character>,
}

/// Type of text region
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RegionType {
    /// Regular text
    Text,
    /// Mathematical expression
    Math,
    /// Diagram or figure
    Diagram,
    /// Table
    Table,
    /// Unknown type
    Unknown,
}

/// Individual character with position and confidence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Character {
    /// The character
    pub char: char,

    /// Confidence score (0.0-1.0)
    pub confidence: f32,

    /// Bounding box if available
    pub bbox: Option<[f32; 4]>,
}

/// Error types for OCR operations
#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("Model loading error: {0}")]
    ModelLoading(String),

    #[error("Inference error: {0}")]
    Inference(String),

    #[error("Image processing error: {0}")]
    ImageProcessing(String),

    #[error("Decoding error: {0}")]
    Decoding(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ONNX Runtime error: {0}")]
    OnnxRuntime(String),
}

pub type Result<T> = std::result::Result<T, OcrError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ocr_options_default() {
        let options = OcrOptions::default();
        assert_eq!(options.detection_threshold, 0.5);
        assert_eq!(options.recognition_threshold, 0.6);
        assert!(options.enable_math);
        assert_eq!(options.decoder_type, DecoderType::BeamSearch);
        assert_eq!(options.beam_width, 5);
    }

    #[test]
    fn test_text_region_creation() {
        let region = TextRegion {
            bbox: [10.0, 20.0, 100.0, 30.0],
            text: "Test".to_string(),
            confidence: 0.95,
            region_type: RegionType::Text,
            characters: vec![],
        };
        assert_eq!(region.bbox[0], 10.0);
        assert_eq!(region.text, "Test");
        assert_eq!(region.region_type, RegionType::Text);
    }

    #[test]
    fn test_decoder_type_equality() {
        assert_eq!(DecoderType::BeamSearch, DecoderType::BeamSearch);
        assert_ne!(DecoderType::BeamSearch, DecoderType::Greedy);
        assert_ne!(DecoderType::Greedy, DecoderType::CTC);
    }
}
