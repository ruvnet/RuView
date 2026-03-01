//! ONNX Inference Module
//!
//! This module handles ONNX inference operations for text detection,
//! character recognition, and mathematical expression recognition.
//!
//! # Model Requirements
//!
//! This module requires ONNX models to be available in the configured model directory.
//! Without models, all inference operations will return errors.
//!
//! To use this module:
//! 1. Download compatible ONNX models (PaddleOCR, TrOCR, or similar)
//! 2. Place them in the models directory
//! 3. Enable the `ocr` feature flag

use super::{models::ModelHandle, OcrError, OcrOptions, Result};
use image::{DynamicImage, GenericImageView};
use std::sync::Arc;
use tracing::{debug, info, warn};

#[cfg(feature = "ocr")]
use ndarray::Array4;
#[cfg(feature = "ocr")]
use ort::value::Tensor;

/// Result from text detection
#[derive(Debug, Clone)]
pub struct DetectionResult {
    /// Bounding box [x, y, width, height]
    pub bbox: [f32; 4],
    /// Detection confidence
    pub confidence: f32,
    /// Cropped image region
    pub region_image: Vec<u8>,
    /// Whether this region likely contains math
    pub is_math_likely: bool,
}

/// Result from text/math recognition
#[derive(Debug, Clone)]
pub struct RecognitionResult {
    /// Logits output from the model [sequence_length, vocab_size]
    pub logits: Vec<Vec<f32>>,
    /// Character-level confidence scores
    pub character_confidences: Vec<f32>,
    /// Raw output tensor (for debugging)
    pub raw_output: Option<Vec<f32>>,
}

/// Inference engine for running ONNX models
///
/// IMPORTANT: This engine requires ONNX models to be loaded.
/// All methods will return errors if models are not properly initialized.
pub struct InferenceEngine {
    /// Detection model
    detection_model: Arc<ModelHandle>,
    /// Recognition model
    recognition_model: Arc<ModelHandle>,
    /// Math recognition model (optional)
    math_model: Option<Arc<ModelHandle>>,
    /// Whether to use GPU acceleration
    use_gpu: bool,
    /// Whether models are actually loaded (vs placeholder handles)
    models_loaded: bool,
}

impl InferenceEngine {
    /// Create a new inference engine
    pub fn new(
        detection_model: Arc<ModelHandle>,
        recognition_model: Arc<ModelHandle>,
        math_model: Option<Arc<ModelHandle>>,
        use_gpu: bool,
    ) -> Result<Self> {
        // Check if models are actually loaded with ONNX sessions
        let detection_loaded = detection_model.is_loaded();
        let recognition_loaded = recognition_model.is_loaded();
        let models_loaded = detection_loaded && recognition_loaded;

        if !models_loaded {
            warn!(
                "ONNX models not fully loaded. Detection: {}, Recognition: {}",
                detection_loaded, recognition_loaded
            );
            warn!("OCR inference will fail until models are properly configured.");
        } else {
            info!(
                "Inference engine initialized with loaded models (GPU: {})",
                if use_gpu { "enabled" } else { "disabled" }
            );
        }

        Ok(Self {
            detection_model,
            recognition_model,
            math_model,
            use_gpu,
            models_loaded,
        })
    }

    /// Check if the inference engine is ready for use
    pub fn is_ready(&self) -> bool {
        self.models_loaded
    }

    /// Run text detection on an image
    pub async fn run_detection(
        &self,
        image_data: &[u8],
        threshold: f32,
    ) -> Result<Vec<DetectionResult>> {
        if !self.models_loaded {
            return Err(OcrError::ModelLoading(
                "ONNX models not loaded. Please download and configure OCR models before use. \
                 See examples/scipix/docs/MODEL_SETUP.md for instructions."
                    .to_string(),
            ));
        }

        debug!("Running text detection (threshold: {})", threshold);
        let input_tensor = self.preprocess_image_for_detection(image_data)?;

        #[cfg(feature = "ocr")]
        {
            let detections = self
                .run_onnx_detection(&input_tensor, threshold, image_data)
                .await?;
            debug!("Detected {} regions", detections.len());
            return Ok(detections);
        }

        #[cfg(not(feature = "ocr"))]
        {
            Err(OcrError::Inference(
                "OCR feature not enabled. Rebuild with `--features ocr` to enable ONNX inference."
                    .to_string(),
            ))
        }
    }

    /// Run text recognition on a region image
    pub async fn run_recognition(
        &self,
        region_image: &[u8],
        options: &OcrOptions,
    ) -> Result<RecognitionResult> {
        if !self.models_loaded {
            return Err(OcrError::ModelLoading(
                "ONNX models not loaded. Please download and configure OCR models before use."
                    .to_string(),
            ));
        }

        debug!("Running text recognition");
        let input_tensor = self.preprocess_image_for_recognition(region_image)?;

        #[cfg(feature = "ocr")]
        {
            let result = self.run_onnx_recognition(&input_tensor, options).await?;
            return Ok(result);
        }

        #[cfg(not(feature = "ocr"))]
        {
            Err(OcrError::Inference(
                "OCR feature not enabled. Rebuild with `--features ocr` to enable ONNX inference."
                    .to_string(),
            ))
        }
    }

    /// Run math recognition on a region image
    pub async fn run_math_recognition(
        &self,
        region_image: &[u8],
        options: &OcrOptions,
    ) -> Result<RecognitionResult> {
        if !self.models_loaded {
            return Err(OcrError::ModelLoading(
                "ONNX models not loaded. Please download and configure OCR models before use."
                    .to_string(),
            ));
        }

        debug!("Running math recognition");

        if self.math_model.is_none() || !self.math_model.as_ref().unwrap().is_loaded() {
            warn!("Math model not loaded, falling back to text recognition");
            return self.run_recognition(region_image, options).await;
        }

        let input_tensor = self.preprocess_image_for_math(region_image)?;

        #[cfg(feature = "ocr")]
        {
            let result = self
                .run_onnx_math_recognition(&input_tensor, options)
                .await?;
            return Ok(result);
        }

        #[cfg(not(feature = "ocr"))]
        {
            Err(OcrError::Inference(
                "OCR feature not enabled. Rebuild with `--features ocr` to enable ONNX inference."
                    .to_string(),
            ))
        }
    }

    /// Preprocess image for detection model
    fn preprocess_image_for_detection(&self, image_data: &[u8]) -> Result<Vec<f32>> {
        let img = image::load_from_memory(image_data)
            .map_err(|e| OcrError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

        let input_shape = self.detection_model.input_shape();
        let (_, _, height, width) = (
            input_shape[0],
            input_shape[1],
            input_shape[2],
            input_shape[3],
        );

        let resized = img.resize_exact(
            width as u32,
            height as u32,
            image::imageops::FilterType::Lanczos3,
        );

        let rgb = resized.to_rgb8();
        let mut tensor = Vec::with_capacity(3 * height * width);

        // Convert to NCHW format with normalization
        for c in 0..3 {
            for y in 0..height {
                for x in 0..width {
                    let pixel = rgb.get_pixel(x as u32, y as u32);
                    tensor.push(pixel[c] as f32 / 255.0);
                }
            }
        }

        Ok(tensor)
    }

    /// Preprocess image for recognition model
    fn preprocess_image_for_recognition(&self, image_data: &[u8]) -> Result<Vec<f32>> {
        let img = image::load_from_memory(image_data)
            .map_err(|e| OcrError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

        let input_shape = self.recognition_model.input_shape();
        let (_, channels, height, width) = (
            input_shape[0],
            input_shape[1],
            input_shape[2],
            input_shape[3],
        );

        let resized = img.resize_exact(
            width as u32,
            height as u32,
            image::imageops::FilterType::Lanczos3,
        );

        let mut tensor = Vec::with_capacity(channels * height * width);

        if channels == 1 {
            let gray = resized.to_luma8();
            for y in 0..height {
                for x in 0..width {
                    let pixel = gray.get_pixel(x as u32, y as u32);
                    tensor.push((pixel[0] as f32 / 127.5) - 1.0);
                }
            }
        } else {
            let rgb = resized.to_rgb8();
            for c in 0..3 {
                for y in 0..height {
                    for x in 0..width {
                        let pixel = rgb.get_pixel(x as u32, y as u32);
                        tensor.push((pixel[c] as f32 / 127.5) - 1.0);
                    }
                }
            }
        }

        Ok(tensor)
    }

    /// Preprocess image for math recognition model
    fn preprocess_image_for_math(&self, image_data: &[u8]) -> Result<Vec<f32>> {
        let math_model = self
            .math_model
            .as_ref()
            .ok_or_else(|| OcrError::Inference("Math model not loaded".to_string()))?;

        let img = image::load_from_memory(image_data)
            .map_err(|e| OcrError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

        let input_shape = math_model.input_shape();
        let (_, channels, height, width) = (
            input_shape[0],
            input_shape[1],
            input_shape[2],
            input_shape[3],
        );

        let resized = img.resize_exact(
            width as u32,
            height as u32,
            image::imageops::FilterType::Lanczos3,
        );

        let mut tensor = Vec::with_capacity(channels * height * width);

        if channels == 1 {
            let gray = resized.to_luma8();
            for y in 0..height {
                for x in 0..width {
                    let pixel = gray.get_pixel(x as u32, y as u32);
                    tensor.push((pixel[0] as f32 / 127.5) - 1.0);
                }
            }
        } else {
            let rgb = resized.to_rgb8();
            for c in 0..channels {
                for y in 0..height {
                    for x in 0..width {
                        let pixel = rgb.get_pixel(x as u32, y as u32);
                        tensor.push((pixel[c] as f32 / 127.5) - 1.0);
                    }
                }
            }
        }

        Ok(tensor)
    }

    /// ONNX detection inference (requires `ocr` feature)
    #[cfg(feature = "ocr")]
    async fn run_onnx_detection(
        &self,
        input_tensor: &[f32],
        threshold: f32,
        original_image: &[u8],
    ) -> Result<Vec<DetectionResult>> {
        let session_arc = self.detection_model.session().ok_or_else(|| {
            OcrError::OnnxRuntime("Detection model session not loaded".to_string())
        })?;
        let mut session = session_arc.lock();

        let input_shape = self.detection_model.input_shape();
        let shape: Vec<usize> = input_shape.to_vec();

        // Create tensor from input data
        let input_array = Array4::from_shape_vec(
            (shape[0], shape[1], shape[2], shape[3]),
            input_tensor.to_vec(),
        )
        .map_err(|e| OcrError::Inference(format!("Failed to create input tensor: {}", e)))?;

        // Convert to dynamic-dimension view and create ORT tensor
        let input_dyn = input_array.into_dyn();
        let input_tensor = Tensor::from_array(input_dyn)
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to create ORT tensor: {}", e)))?;

        // Run inference
        let outputs = session
            .run(ort::inputs![input_tensor])
            .map_err(|e| OcrError::OnnxRuntime(format!("Inference failed: {}", e)))?;

        let output_tensor = outputs
            .iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| OcrError::OnnxRuntime("No output tensor found".to_string()))?;

        let (_, raw_data) = output_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to extract output: {}", e)))?;
        let output_data: Vec<f32> = raw_data.to_vec();

        let original_img = image::load_from_memory(original_image)
            .map_err(|e| OcrError::ImageProcessing(format!("Failed to decode image: {}", e)))?;

        let detections = self.parse_detection_output(&output_data, threshold, &original_img)?;
        Ok(detections)
    }

    /// Parse detection model output
    #[cfg(feature = "ocr")]
    fn parse_detection_output(
        &self,
        output: &[f32],
        threshold: f32,
        original_img: &DynamicImage,
    ) -> Result<Vec<DetectionResult>> {
        let mut results = Vec::new();
        let output_shape = self.detection_model.output_shape();

        if output_shape.len() >= 2 {
            let num_detections = output_shape[1];
            let detection_size = if output_shape.len() >= 3 {
                output_shape[2]
            } else {
                85
            };

            for i in 0..num_detections {
                let base_idx = i * detection_size;
                if base_idx + 5 > output.len() {
                    break;
                }

                let confidence = output[base_idx + 4];
                if confidence < threshold {
                    continue;
                }

                let cx = output[base_idx];
                let cy = output[base_idx + 1];
                let w = output[base_idx + 2];
                let h = output[base_idx + 3];

                let img_width = original_img.width() as f32;
                let img_height = original_img.height() as f32;

                let x = ((cx - w / 2.0) * img_width).max(0.0);
                let y = ((cy - h / 2.0) * img_height).max(0.0);
                let width = (w * img_width).min(img_width - x);
                let height = (h * img_height).min(img_height - y);

                if width <= 0.0 || height <= 0.0 {
                    continue;
                }

                let cropped =
                    original_img.crop_imm(x as u32, y as u32, width as u32, height as u32);

                let mut region_bytes = Vec::new();
                cropped
                    .write_to(
                        &mut std::io::Cursor::new(&mut region_bytes),
                        image::ImageFormat::Png,
                    )
                    .map_err(|e| {
                        OcrError::ImageProcessing(format!("Failed to encode region: {}", e))
                    })?;

                let aspect_ratio = width / height;
                let is_math_likely = aspect_ratio > 2.0 || aspect_ratio < 0.5;

                results.push(DetectionResult {
                    bbox: [x, y, width, height],
                    confidence,
                    region_image: region_bytes,
                    is_math_likely,
                });
            }
        }

        Ok(results)
    }

    /// ONNX recognition inference (requires `ocr` feature)
    #[cfg(feature = "ocr")]
    async fn run_onnx_recognition(
        &self,
        input_tensor: &[f32],
        _options: &OcrOptions,
    ) -> Result<RecognitionResult> {
        let session_arc = self.recognition_model.session().ok_or_else(|| {
            OcrError::OnnxRuntime("Recognition model session not loaded".to_string())
        })?;
        let mut session = session_arc.lock();

        let input_shape = self.recognition_model.input_shape();
        let shape: Vec<usize> = input_shape.to_vec();

        let input_array = Array4::from_shape_vec(
            (shape[0], shape[1], shape[2], shape[3]),
            input_tensor.to_vec(),
        )
        .map_err(|e| OcrError::Inference(format!("Failed to create input tensor: {}", e)))?;

        let input_dyn = input_array.into_dyn();
        let input_ort = Tensor::from_array(input_dyn)
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to create ORT tensor: {}", e)))?;

        let outputs = session
            .run(ort::inputs![input_ort])
            .map_err(|e| OcrError::OnnxRuntime(format!("Recognition inference failed: {}", e)))?;

        let output_tensor = outputs
            .iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| OcrError::OnnxRuntime("No output tensor found".to_string()))?;

        let (_, raw_data) = output_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to extract output: {}", e)))?;
        let output_data: Vec<f32> = raw_data.to_vec();

        let output_shape = self.recognition_model.output_shape();
        let seq_len = output_shape.get(1).copied().unwrap_or(26);
        let vocab_size = output_shape.get(2).copied().unwrap_or(37);

        let mut logits = Vec::new();
        let mut character_confidences = Vec::new();

        for i in 0..seq_len {
            let start_idx = i * vocab_size;
            let end_idx = start_idx + vocab_size;

            if end_idx <= output_data.len() {
                let step_logits: Vec<f32> = output_data[start_idx..end_idx].to_vec();

                let max_logit = step_logits
                    .iter()
                    .cloned()
                    .fold(f32::NEG_INFINITY, f32::max);
                let exp_sum: f32 = step_logits.iter().map(|&x| (x - max_logit).exp()).sum();

                let softmax: Vec<f32> = step_logits
                    .iter()
                    .map(|&x| (x - max_logit).exp() / exp_sum)
                    .collect();

                let max_confidence = softmax.iter().cloned().fold(0.0f32, f32::max);
                character_confidences.push(max_confidence);
                logits.push(step_logits);
            }
        }

        Ok(RecognitionResult {
            logits,
            character_confidences,
            raw_output: Some(output_data),
        })
    }

    /// ONNX math recognition inference (requires `ocr` feature)
    #[cfg(feature = "ocr")]
    async fn run_onnx_math_recognition(
        &self,
        input_tensor: &[f32],
        _options: &OcrOptions,
    ) -> Result<RecognitionResult> {
        let math_model = self
            .math_model
            .as_ref()
            .ok_or_else(|| OcrError::Inference("Math model not loaded".to_string()))?;

        let session_arc = math_model
            .session()
            .ok_or_else(|| OcrError::OnnxRuntime("Math model session not loaded".to_string()))?;
        let mut session = session_arc.lock();

        let input_shape = math_model.input_shape();
        let shape: Vec<usize> = input_shape.to_vec();

        let input_array = Array4::from_shape_vec(
            (shape[0], shape[1], shape[2], shape[3]),
            input_tensor.to_vec(),
        )
        .map_err(|e| OcrError::Inference(format!("Failed to create input tensor: {}", e)))?;

        let input_dyn = input_array.into_dyn();
        let input_ort = Tensor::from_array(input_dyn)
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to create ORT tensor: {}", e)))?;

        let outputs = session.run(ort::inputs![input_ort]).map_err(|e| {
            OcrError::OnnxRuntime(format!("Math recognition inference failed: {}", e))
        })?;

        let output_tensor = outputs
            .iter()
            .next()
            .map(|(_, v)| v)
            .ok_or_else(|| OcrError::OnnxRuntime("No output tensor found".to_string()))?;

        let (_, raw_data) = output_tensor
            .try_extract_tensor::<f32>()
            .map_err(|e| OcrError::OnnxRuntime(format!("Failed to extract output: {}", e)))?;
        let output_data: Vec<f32> = raw_data.to_vec();

        let output_shape = math_model.output_shape();
        let seq_len = output_shape.get(1).copied().unwrap_or(50);
        let vocab_size = output_shape.get(2).copied().unwrap_or(512);

        let mut logits = Vec::new();
        let mut character_confidences = Vec::new();

        for i in 0..seq_len {
            let start_idx = i * vocab_size;
            let end_idx = start_idx + vocab_size;

            if end_idx <= output_data.len() {
                let step_logits: Vec<f32> = output_data[start_idx..end_idx].to_vec();

                let max_logit = step_logits
                    .iter()
                    .cloned()
                    .fold(f32::NEG_INFINITY, f32::max);
                let exp_sum: f32 = step_logits.iter().map(|&x| (x - max_logit).exp()).sum();

                let softmax: Vec<f32> = step_logits
                    .iter()
                    .map(|&x| (x - max_logit).exp() / exp_sum)
                    .collect();

                let max_confidence = softmax.iter().cloned().fold(0.0f32, f32::max);
                character_confidences.push(max_confidence);
                logits.push(step_logits);
            }
        }

        Ok(RecognitionResult {
            logits,
            character_confidences,
            raw_output: Some(output_data),
        })
    }

    /// Get detection model
    pub fn detection_model(&self) -> &ModelHandle {
        &self.detection_model
    }

    /// Get recognition model
    pub fn recognition_model(&self) -> &ModelHandle {
        &self.recognition_model
    }

    /// Get math model if available
    pub fn math_model(&self) -> Option<&ModelHandle> {
        self.math_model.as_ref().map(|m| m.as_ref())
    }

    /// Check if GPU acceleration is enabled
    pub fn is_gpu_enabled(&self) -> bool {
        self.use_gpu
    }
}

/// Batch inference optimization
impl InferenceEngine {
    /// Run batch detection on multiple images
    pub async fn run_batch_detection(
        &self,
        images: &[&[u8]],
        threshold: f32,
    ) -> Result<Vec<Vec<DetectionResult>>> {
        if !self.models_loaded {
            return Err(OcrError::ModelLoading(
                "ONNX models not loaded. Cannot run batch detection.".to_string(),
            ));
        }

        debug!("Running batch detection on {} images", images.len());

        let mut results = Vec::new();
        for image in images {
            let detections = self.run_detection(image, threshold).await?;
            results.push(detections);
        }

        Ok(results)
    }

    /// Run batch recognition on multiple regions
    pub async fn run_batch_recognition(
        &self,
        regions: &[&[u8]],
        options: &OcrOptions,
    ) -> Result<Vec<RecognitionResult>> {
        if !self.models_loaded {
            return Err(OcrError::ModelLoading(
                "ONNX models not loaded. Cannot run batch recognition.".to_string(),
            ));
        }

        debug!("Running batch recognition on {} regions", regions.len());

        let mut results = Vec::new();
        for region in regions {
            let result = self.run_recognition(region, options).await?;
            results.push(result);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ocr::models::{ModelMetadata, ModelType};
    use std::path::PathBuf;

    fn create_test_model(model_type: ModelType, path: PathBuf) -> Arc<ModelHandle> {
        let metadata = ModelMetadata {
            name: format!("{:?} Model", model_type),
            version: "1.0.0".to_string(),
            input_shape: vec![1, 3, 640, 640],
            output_shape: vec![1, 100, 85],
            input_dtype: "float32".to_string(),
            file_size: 1000,
            checksum: None,
        };

        Arc::new(ModelHandle::new(model_type, path, metadata).unwrap())
    }

    #[test]
    fn test_inference_engine_creation_without_models() {
        let detection = create_test_model(
            ModelType::Detection,
            PathBuf::from("/nonexistent/model.onnx"),
        );
        let recognition = create_test_model(
            ModelType::Recognition,
            PathBuf::from("/nonexistent/model.onnx"),
        );

        let engine = InferenceEngine::new(detection, recognition, None, false).unwrap();
        assert!(!engine.is_ready());
    }

    #[tokio::test]
    async fn test_detection_fails_without_models() {
        let detection = create_test_model(
            ModelType::Detection,
            PathBuf::from("/nonexistent/model.onnx"),
        );
        let recognition = create_test_model(
            ModelType::Recognition,
            PathBuf::from("/nonexistent/model.onnx"),
        );
        let engine = InferenceEngine::new(detection, recognition, None, false).unwrap();

        let png_data = create_test_png();
        let result = engine.run_detection(&png_data, 0.5).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OcrError::ModelLoading(_)));
    }

    #[tokio::test]
    async fn test_recognition_fails_without_models() {
        let detection = create_test_model(
            ModelType::Detection,
            PathBuf::from("/nonexistent/model.onnx"),
        );
        let recognition = create_test_model(
            ModelType::Recognition,
            PathBuf::from("/nonexistent/model.onnx"),
        );
        let engine = InferenceEngine::new(detection, recognition, None, false).unwrap();

        let png_data = create_test_png();
        let options = OcrOptions::default();
        let result = engine.run_recognition(&png_data, &options).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), OcrError::ModelLoading(_)));
    }

    #[test]
    fn test_is_ready_reflects_model_state() {
        let detection = create_test_model(ModelType::Detection, PathBuf::from("/fake/path"));
        let recognition = create_test_model(ModelType::Recognition, PathBuf::from("/fake/path"));
        let engine = InferenceEngine::new(detection, recognition, None, false).unwrap();
        assert!(!engine.is_ready());
    }

    fn create_test_png() -> Vec<u8> {
        use image::{ImageBuffer, RgbImage};
        let img: RgbImage = ImageBuffer::from_fn(10, 10, |_, _| image::Rgb([255, 255, 255]));
        let mut bytes: Vec<u8> = Vec::new();
        img.write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
        bytes
    }
}
