//! Type definitions for WASM API

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

/// OCR result returned to JavaScript
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct OcrResult {
    /// Recognized plain text
    pub text: String,

    /// LaTeX representation (if applicable)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub latex: Option<String>,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Additional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

#[wasm_bindgen]
impl OcrResult {
    /// Create a new OCR result
    #[wasm_bindgen(constructor)]
    pub fn new(text: String, confidence: f32) -> Self {
        Self {
            text,
            latex: None,
            confidence,
            metadata: None,
        }
    }

    /// Get the text
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.text.clone()
    }

    /// Get the LaTeX (if available)
    #[wasm_bindgen(getter)]
    pub fn latex(&self) -> Option<String> {
        self.latex.clone()
    }

    /// Get the confidence score
    #[wasm_bindgen(getter)]
    pub fn confidence(&self) -> f32 {
        self.confidence
    }

    /// Check if result has LaTeX
    #[wasm_bindgen(js_name = hasLatex)]
    pub fn has_latex(&self) -> bool {
        self.latex.is_some()
    }

    /// Convert to JSON
    #[wasm_bindgen(js_name = toJSON)]
    pub fn to_json(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(self)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }
}

/// Recognition output format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecognitionFormat {
    /// Plain text only
    Text,
    /// LaTeX only
    Latex,
    /// Both text and LaTeX
    Both,
}

impl RecognitionFormat {
    pub fn to_string(&self) -> String {
        match self {
            Self::Text => "text".to_string(),
            Self::Latex => "latex".to_string(),
            Self::Both => "both".to_string(),
        }
    }
}

impl Default for RecognitionFormat {
    fn default() -> Self {
        Self::Both
    }
}

/// Processing options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[wasm_bindgen]
pub struct ProcessingOptions {
    /// Output format
    pub format: String,

    /// Confidence threshold
    pub confidence_threshold: f32,

    /// Enable preprocessing
    pub preprocess: bool,

    /// Enable postprocessing
    pub postprocess: bool,
}

#[wasm_bindgen]
impl ProcessingOptions {
    /// Create default options
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set format
    #[wasm_bindgen(js_name = setFormat)]
    pub fn set_format(&mut self, format: String) {
        self.format = format;
    }

    /// Set confidence threshold
    #[wasm_bindgen(js_name = setConfidenceThreshold)]
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold;
    }
}

impl Default for ProcessingOptions {
    fn default() -> Self {
        Self {
            format: "both".to_string(),
            confidence_threshold: 0.5,
            preprocess: true,
            postprocess: true,
        }
    }
}

/// Error types for WASM API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WasmError {
    /// Image decoding error
    ImageDecode(String),

    /// Processing error
    Processing(String),

    /// Invalid input
    InvalidInput(String),

    /// Not initialized
    NotInitialized,
}

impl std::fmt::Display for WasmError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ImageDecode(msg) => write!(f, "Image decode error: {}", msg),
            Self::Processing(msg) => write!(f, "Processing error: {}", msg),
            Self::InvalidInput(msg) => write!(f, "Invalid input: {}", msg),
            Self::NotInitialized => write!(f, "WASM module not initialized"),
        }
    }
}

impl std::error::Error for WasmError {}

impl From<WasmError> for JsValue {
    fn from(error: WasmError) -> Self {
        JsValue::from_str(&error.to_string())
    }
}
