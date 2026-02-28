//! Canvas and ImageData handling for WASM

use anyhow::{anyhow, Result};
use image::{DynamicImage, ImageBuffer, Rgba};
use wasm_bindgen::prelude::*;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, ImageData};

use crate::wasm::types::{OcrResult, RecognitionFormat};

/// Processor for canvas and image data
pub struct CanvasProcessor {
    // Could add model loading here in the future
}

impl CanvasProcessor {
    /// Create a new canvas processor
    pub fn new() -> Self {
        Self {}
    }

    /// Extract image data from HTML canvas element
    pub fn extract_canvas_image(&self, canvas: &HtmlCanvasElement) -> Result<ImageData> {
        let context = canvas
            .get_context("2d")
            .map_err(|_| anyhow!("Failed to get 2d context"))?
            .ok_or_else(|| anyhow!("Context is null"))?
            .dyn_into::<CanvasRenderingContext2d>()
            .map_err(|_| anyhow!("Failed to cast to 2d context"))?;

        let width = canvas.width();
        let height = canvas.height();

        context
            .get_image_data(0.0, 0.0, width as f64, height as f64)
            .map_err(|_| anyhow!("Failed to get image data"))
    }

    /// Convert ImageData to DynamicImage
    pub fn image_data_to_dynamic(&self, image_data: &ImageData) -> Result<DynamicImage> {
        let width = image_data.width();
        let height = image_data.height();
        let data = image_data.data();

        let img_buffer = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(width, height, data.to_vec())
            .ok_or_else(|| anyhow!("Failed to create image buffer"))?;

        Ok(DynamicImage::ImageRgba8(img_buffer))
    }

    /// Process raw image bytes
    pub async fn process_image_bytes(
        &self,
        image_bytes: &[u8],
        format: RecognitionFormat,
    ) -> Result<OcrResult> {
        // Decode image
        let img = image::load_from_memory(image_bytes)
            .map_err(|e| anyhow!("Failed to decode image: {}", e))?;

        self.process_dynamic_image(&img, format).await
    }

    /// Process ImageData from canvas
    pub async fn process_image_data(
        &self,
        image_data: &ImageData,
        format: RecognitionFormat,
    ) -> Result<OcrResult> {
        let img = self.image_data_to_dynamic(image_data)?;
        self.process_dynamic_image(&img, format).await
    }

    /// Process a DynamicImage
    async fn process_dynamic_image(
        &self,
        img: &DynamicImage,
        format: RecognitionFormat,
    ) -> Result<OcrResult> {
        // Convert to grayscale for processing
        let gray = img.to_luma8();

        // Apply preprocessing
        let preprocessed = self.preprocess_image(&gray);

        // Perform OCR (mock implementation for now)
        // In a real implementation, this would run a model
        let text = self.extract_text(&preprocessed)?;
        let latex = if matches!(format, RecognitionFormat::Latex | RecognitionFormat::Both) {
            Some(self.extract_latex(&preprocessed)?)
        } else {
            None
        };

        // Calculate confidence (simplified)
        let confidence = self.calculate_confidence(&text, &latex);

        Ok(OcrResult {
            text,
            latex,
            confidence,
            metadata: Some(serde_json::json!({
                "width": img.width(),
                "height": img.height(),
                "format": format.to_string(),
            })),
        })
    }

    /// Preprocess image for OCR
    fn preprocess_image(&self, img: &image::GrayImage) -> image::GrayImage {
        // Apply simple thresholding
        let mut output = img.clone();

        for pixel in output.pixels_mut() {
            let value = pixel.0[0];
            pixel.0[0] = if value > 128 { 255 } else { 0 };
        }

        output
    }

    /// Extract plain text (mock implementation)
    fn extract_text(&self, img: &image::GrayImage) -> Result<String> {
        // This would normally run an OCR model
        // For now, return a placeholder
        Ok("Recognized text placeholder".to_string())
    }

    /// Extract LaTeX (mock implementation)
    fn extract_latex(&self, img: &image::GrayImage) -> Result<String> {
        // This would normally run a math OCR model
        // For now, return a placeholder
        Ok(r"\sum_{i=1}^{n} x_i".to_string())
    }

    /// Calculate confidence score
    fn calculate_confidence(&self, text: &str, latex: &Option<String>) -> f32 {
        // Simple heuristic: longer text = higher confidence
        let text_score = (text.len() as f32 / 100.0).min(1.0);
        let latex_score = latex
            .as_ref()
            .map(|l| (l.len() as f32 / 50.0).min(1.0))
            .unwrap_or(0.0);

        (text_score + latex_score) / 2.0
    }
}

impl Default for CanvasProcessor {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert blob URL to image data
#[wasm_bindgen]
pub async fn blob_url_to_image_data(blob_url: &str) -> Result<ImageData, JsValue> {
    use web_sys::{window, HtmlImageElement};

    let window = window().ok_or_else(|| JsValue::from_str("No window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("No document"))?;

    // Create image element
    let img =
        HtmlImageElement::new().map_err(|_| JsValue::from_str("Failed to create image element"))?;

    img.set_src(blob_url);

    // Wait for image to load
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let img_clone = img.clone();
        let onload = Closure::wrap(Box::new(move || {
            resolve.call1(&JsValue::NULL, &img_clone).unwrap();
        }) as Box<dyn FnMut()>);

        img.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget();

        let onerror = Closure::wrap(Box::new(move || {
            reject
                .call1(&JsValue::NULL, &JsValue::from_str("Image load failed"))
                .unwrap();
        }) as Box<dyn FnMut()>);

        img.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    });

    wasm_bindgen_futures::JsFuture::from(promise).await?;

    // Create canvas and draw image
    let canvas = document
        .create_element("canvas")
        .map_err(|_| JsValue::from_str("Failed to create canvas"))?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(|_| JsValue::from_str("Failed to cast to canvas"))?;

    canvas.set_width(img.natural_width());
    canvas.set_height(img.natural_height());

    let context = canvas
        .get_context("2d")
        .map_err(|_| JsValue::from_str("Failed to get 2d context"))?
        .ok_or_else(|| JsValue::from_str("Context is null"))?
        .dyn_into::<CanvasRenderingContext2d>()
        .map_err(|_| JsValue::from_str("Failed to cast to 2d context"))?;

    context
        .draw_image_with_html_image_element(&img, 0.0, 0.0)
        .map_err(|_| JsValue::from_str("Failed to draw image"))?;

    context
        .get_image_data(0.0, 0.0, canvas.width() as f64, canvas.height() as f64)
        .map_err(|_| JsValue::from_str("Failed to get image data"))
}
