//! OCR Engine Implementation
//!
//! This module provides the main OcrEngine for orchestrating OCR operations.
//! It handles model loading, inference coordination, and result assembly.

use super::{
    confidence::aggregate_confidence,
    decoder::{BeamSearchDecoder, CTCDecoder, Decoder, GreedyDecoder, Vocabulary},
    inference::{DetectionResult, InferenceEngine, RecognitionResult},
    models::{ModelHandle, ModelRegistry},
    Character, DecoderType, OcrError, OcrOptions, OcrResult, RegionType, Result, TextRegion,
};
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use tracing::{debug, info, warn};

/// OCR processor trait for custom implementations
pub trait OcrProcessor: Send + Sync {
    /// Process an image and return OCR results
    fn process(&self, image_data: &[u8], options: &OcrOptions) -> Result<OcrResult>;

    /// Batch process multiple images
    fn process_batch(&self, images: &[&[u8]], options: &OcrOptions) -> Result<Vec<OcrResult>>;
}

/// Main OCR Engine with thread-safe model management
pub struct OcrEngine {
    /// Model registry for loading and caching models
    registry: Arc<RwLock<ModelRegistry>>,

    /// Inference engine for running ONNX models
    inference: Arc<InferenceEngine>,

    /// Default OCR options
    default_options: OcrOptions,

    /// Vocabulary for decoding
    vocabulary: Arc<Vocabulary>,

    /// Whether the engine is warmed up
    warmed_up: Arc<RwLock<bool>>,
}

impl OcrEngine {
    /// Create a new OCR engine with default models
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use ruvector_scipix::ocr::OcrEngine;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let engine = OcrEngine::new().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new() -> Result<Self> {
        Self::with_options(OcrOptions::default()).await
    }

    /// Create a new OCR engine with custom options
    pub async fn with_options(options: OcrOptions) -> Result<Self> {
        info!("Initializing OCR engine with options: {:?}", options);

        // Initialize model registry
        let registry = Arc::new(RwLock::new(ModelRegistry::new()));

        // Load default models (in production, these would be downloaded/cached)
        debug!("Loading detection model...");
        let detection_model = registry.write().load_detection_model().await.map_err(|e| {
            OcrError::ModelLoading(format!("Failed to load detection model: {}", e))
        })?;

        debug!("Loading recognition model...");
        let recognition_model = registry
            .write()
            .load_recognition_model()
            .await
            .map_err(|e| {
                OcrError::ModelLoading(format!("Failed to load recognition model: {}", e))
            })?;

        let math_model =
            if options.enable_math {
                debug!("Loading math recognition model...");
                Some(registry.write().load_math_model().await.map_err(|e| {
                    OcrError::ModelLoading(format!("Failed to load math model: {}", e))
                })?)
            } else {
                None
            };

        // Create inference engine
        let inference = Arc::new(InferenceEngine::new(
            detection_model,
            recognition_model,
            math_model,
            options.use_gpu,
        )?);

        // Load vocabulary
        let vocabulary = Arc::new(Vocabulary::default());

        let engine = Self {
            registry,
            inference,
            default_options: options,
            vocabulary,
            warmed_up: Arc::new(RwLock::new(false)),
        };

        info!("OCR engine initialized successfully");
        Ok(engine)
    }

    /// Warm up the engine by running a dummy inference
    ///
    /// This helps reduce latency for the first real inference by initializing
    /// all ONNX runtime resources.
    pub async fn warmup(&self) -> Result<()> {
        if *self.warmed_up.read() {
            debug!("Engine already warmed up, skipping");
            return Ok(());
        }

        info!("Warming up OCR engine...");
        let start = Instant::now();

        // Create a small dummy image (100x100 black image)
        let dummy_image = vec![0u8; 100 * 100 * 3];

        // Run a dummy inference
        let _ = self.recognize(&dummy_image).await;

        *self.warmed_up.write() = true;
        info!("Engine warmup completed in {:?}", start.elapsed());
        Ok(())
    }

    /// Recognize text in an image using default options
    pub async fn recognize(&self, image_data: &[u8]) -> Result<OcrResult> {
        self.recognize_with_options(image_data, &self.default_options)
            .await
    }

    /// Recognize text in an image with custom options
    pub async fn recognize_with_options(
        &self,
        image_data: &[u8],
        options: &OcrOptions,
    ) -> Result<OcrResult> {
        let start = Instant::now();
        debug!("Starting OCR recognition");

        // Step 1: Run text detection
        debug!("Running text detection...");
        let detection_results = self
            .inference
            .run_detection(image_data, options.detection_threshold)
            .await?;

        debug!("Detected {} regions", detection_results.len());

        if detection_results.is_empty() {
            warn!("No text regions detected");
            return Ok(OcrResult {
                text: String::new(),
                confidence: 0.0,
                regions: vec![],
                has_math: false,
                processing_time_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Step 2: Run recognition on each detected region
        debug!("Running text recognition...");
        let mut text_regions = Vec::new();
        let mut has_math = false;

        for detection in detection_results {
            // Determine region type
            let region_type = if options.enable_math && detection.is_math_likely {
                has_math = true;
                RegionType::Math
            } else {
                RegionType::Text
            };

            // Run appropriate recognition
            let recognition = if region_type == RegionType::Math {
                self.inference
                    .run_math_recognition(&detection.region_image, options)
                    .await?
            } else {
                self.inference
                    .run_recognition(&detection.region_image, options)
                    .await?
            };

            // Decode the recognition output
            let decoded_text = self.decode_output(&recognition, options)?;

            // Calculate confidence
            let confidence = aggregate_confidence(&recognition.character_confidences);

            // Filter by confidence threshold
            if confidence < options.recognition_threshold {
                debug!(
                    "Skipping region with low confidence: {:.2} < {:.2}",
                    confidence, options.recognition_threshold
                );
                continue;
            }

            // Build character list
            let characters = decoded_text
                .chars()
                .zip(recognition.character_confidences.iter())
                .map(|(ch, &conf)| Character {
                    char: ch,
                    confidence: conf,
                    bbox: None, // Could be populated if available from model
                })
                .collect();

            text_regions.push(TextRegion {
                bbox: detection.bbox,
                text: decoded_text,
                confidence,
                region_type,
                characters,
            });
        }

        // Step 3: Combine results
        let combined_text = text_regions
            .iter()
            .map(|r| r.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let overall_confidence = if text_regions.is_empty() {
            0.0
        } else {
            text_regions.iter().map(|r| r.confidence).sum::<f32>() / text_regions.len() as f32
        };

        let processing_time_ms = start.elapsed().as_millis() as u64;

        debug!(
            "OCR completed in {}ms, recognized {} regions",
            processing_time_ms,
            text_regions.len()
        );

        Ok(OcrResult {
            text: combined_text,
            confidence: overall_confidence,
            regions: text_regions,
            has_math,
            processing_time_ms,
        })
    }

    /// Batch process multiple images
    pub async fn recognize_batch(
        &self,
        images: &[&[u8]],
        options: &OcrOptions,
    ) -> Result<Vec<OcrResult>> {
        info!("Processing batch of {} images", images.len());
        let start = Instant::now();

        // Process images in parallel using rayon
        let results: Result<Vec<OcrResult>> = images
            .iter()
            .map(|image_data| {
                // Note: In a real async implementation, we'd use tokio::spawn
                // For now, we'll use blocking since we're in a sync context
                futures::executor::block_on(self.recognize_with_options(image_data, options))
            })
            .collect();

        info!("Batch processing completed in {:?}", start.elapsed());

        results
    }

    /// Decode recognition output using the selected decoder
    fn decode_output(
        &self,
        recognition: &RecognitionResult,
        options: &OcrOptions,
    ) -> Result<String> {
        debug!("Decoding output with {:?} decoder", options.decoder_type);

        let decoded = match options.decoder_type {
            DecoderType::BeamSearch => {
                let decoder = BeamSearchDecoder::new(self.vocabulary.clone(), options.beam_width);
                decoder.decode(&recognition.logits)?
            }
            DecoderType::Greedy => {
                let decoder = GreedyDecoder::new(self.vocabulary.clone());
                decoder.decode(&recognition.logits)?
            }
            DecoderType::CTC => {
                let decoder = CTCDecoder::new(self.vocabulary.clone());
                decoder.decode(&recognition.logits)?
            }
        };

        Ok(decoded)
    }

    /// Get the current model registry
    pub fn registry(&self) -> Arc<RwLock<ModelRegistry>> {
        Arc::clone(&self.registry)
    }

    /// Get the default options
    pub fn default_options(&self) -> &OcrOptions {
        &self.default_options
    }

    /// Check if engine is warmed up
    pub fn is_warmed_up(&self) -> bool {
        *self.warmed_up.read()
    }
}

impl OcrProcessor for OcrEngine {
    fn process(&self, image_data: &[u8], options: &OcrOptions) -> Result<OcrResult> {
        // Blocking wrapper for async method
        futures::executor::block_on(self.recognize_with_options(image_data, options))
    }

    fn process_batch(&self, images: &[&[u8]], options: &OcrOptions) -> Result<Vec<OcrResult>> {
        // Blocking wrapper for async method
        futures::executor::block_on(self.recognize_batch(images, options))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decoder_selection() {
        let options = OcrOptions {
            decoder_type: DecoderType::BeamSearch,
            ..Default::default()
        };
        assert_eq!(options.decoder_type, DecoderType::BeamSearch);
    }

    #[test]
    fn test_warmup_flag() {
        let flag = Arc::new(RwLock::new(false));
        assert!(!*flag.read());
        *flag.write() = true;
        assert!(*flag.read());
    }
}
