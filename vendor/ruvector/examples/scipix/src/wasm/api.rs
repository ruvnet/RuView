//! JavaScript API for Scipix OCR

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, ImageData};

use crate::wasm::canvas::CanvasProcessor;
use crate::wasm::memory::WasmBuffer;
use crate::wasm::types::{OcrResult, RecognitionFormat};

static PROCESSOR: OnceCell<Arc<CanvasProcessor>> = OnceCell::new();

/// Main WASM API for Scipix OCR
#[wasm_bindgen]
pub struct ScipixWasm {
    processor: Arc<CanvasProcessor>,
    format: RecognitionFormat,
    confidence_threshold: f32,
}

#[wasm_bindgen]
impl ScipixWasm {
    /// Create a new ScipixWasm instance
    #[wasm_bindgen(constructor)]
    pub async fn new() -> Result<ScipixWasm, JsValue> {
        let processor = PROCESSOR
            .get_or_init(|| Arc::new(CanvasProcessor::new()))
            .clone();

        Ok(ScipixWasm {
            processor,
            format: RecognitionFormat::Both,
            confidence_threshold: 0.5,
        })
    }

    /// Recognize text from raw image data
    #[wasm_bindgen]
    pub async fn recognize(&self, image_data: &[u8]) -> Result<JsValue, JsValue> {
        let buffer = WasmBuffer::from_slice(image_data);

        let result = self
            .processor
            .process_image_bytes(buffer.as_slice(), self.format)
            .await
            .map_err(|e| JsValue::from_str(&format!("Recognition failed: {}", e)))?;

        // Filter by confidence threshold
        let filtered = self.filter_by_confidence(result);

        serde_wasm_bindgen::to_value(&filtered)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Recognize text from HTML Canvas element
    #[wasm_bindgen(js_name = recognizeFromCanvas)]
    pub async fn recognize_from_canvas(
        &self,
        canvas: &HtmlCanvasElement,
    ) -> Result<JsValue, JsValue> {
        let image_data = self
            .processor
            .extract_canvas_image(canvas)
            .map_err(|e| JsValue::from_str(&format!("Canvas extraction failed: {}", e)))?;

        let result = self
            .processor
            .process_image_data(&image_data, self.format)
            .await
            .map_err(|e| JsValue::from_str(&format!("Recognition failed: {}", e)))?;

        let filtered = self.filter_by_confidence(result);

        serde_wasm_bindgen::to_value(&filtered)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Recognize text from base64-encoded image
    #[wasm_bindgen(js_name = recognizeBase64)]
    pub async fn recognize_base64(&self, base64: &str) -> Result<JsValue, JsValue> {
        // Remove data URL prefix if present
        let base64_data = if base64.contains(',') {
            base64.split(',').nth(1).unwrap_or(base64)
        } else {
            base64
        };

        let image_bytes = base64::decode(base64_data)
            .map_err(|e| JsValue::from_str(&format!("Base64 decode failed: {}", e)))?;

        self.recognize(&image_bytes).await
    }

    /// Recognize text from ImageData object
    #[wasm_bindgen(js_name = recognizeImageData)]
    pub async fn recognize_image_data(&self, image_data: &ImageData) -> Result<JsValue, JsValue> {
        let result = self
            .processor
            .process_image_data(image_data, self.format)
            .await
            .map_err(|e| JsValue::from_str(&format!("Recognition failed: {}", e)))?;

        let filtered = self.filter_by_confidence(result);

        serde_wasm_bindgen::to_value(&filtered)
            .map_err(|e| JsValue::from_str(&format!("Serialization failed: {}", e)))
    }

    /// Set the output format (text, latex, or both)
    #[wasm_bindgen(js_name = setFormat)]
    pub fn set_format(&mut self, format: &str) {
        self.format = match format.to_lowercase().as_str() {
            "text" => RecognitionFormat::Text,
            "latex" => RecognitionFormat::Latex,
            "both" => RecognitionFormat::Both,
            _ => RecognitionFormat::Both,
        };
    }

    /// Set the confidence threshold (0.0 - 1.0)
    #[wasm_bindgen(js_name = setConfidenceThreshold)]
    pub fn set_confidence_threshold(&mut self, threshold: f32) {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
    }

    /// Get the current confidence threshold
    #[wasm_bindgen(js_name = getConfidenceThreshold)]
    pub fn get_confidence_threshold(&self) -> f32 {
        self.confidence_threshold
    }

    /// Get the version of the library
    #[wasm_bindgen(js_name = getVersion)]
    pub fn get_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    /// Get supported output formats
    #[wasm_bindgen(js_name = getSupportedFormats)]
    pub fn get_supported_formats(&self) -> Vec<JsValue> {
        vec![
            JsValue::from_str("text"),
            JsValue::from_str("latex"),
            JsValue::from_str("both"),
        ]
    }

    /// Batch process multiple images
    #[wasm_bindgen(js_name = recognizeBatch)]
    pub async fn recognize_batch(&self, images: Vec<JsValue>) -> Result<JsValue, JsValue> {
        let mut results = Vec::new();

        for img in images {
            // Try to process as Uint8Array
            if let Ok(bytes) = js_sys::Uint8Array::new(&img).to_vec() {
                match self.recognize(&bytes).await {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        web_sys::console::warn_1(&JsValue::from_str(&format!(
                            "Failed to process image: {:?}",
                            e
                        )));
                        results.push(JsValue::NULL);
                    }
                }
            }
        }

        Ok(js_sys::Array::from_iter(results).into())
    }

    // Private helper methods

    fn filter_by_confidence(&self, mut result: OcrResult) -> OcrResult {
        if result.confidence < self.confidence_threshold {
            result.text = String::new();
            result.latex = None;
        }
        result
    }
}

/// Create a new ScipixWasm instance (factory function)
#[wasm_bindgen(js_name = createScipix)]
pub async fn create_scipix() -> Result<ScipixWasm, JsValue> {
    ScipixWasm::new().await
}
