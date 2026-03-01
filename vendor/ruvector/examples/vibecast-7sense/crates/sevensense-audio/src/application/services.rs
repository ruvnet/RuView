//! Application services for audio processing.
//!
//! These services coordinate domain operations with infrastructure components
//! to implement the audio ingestion use cases.

use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument, warn};

use crate::domain::entities::{CallSegment, Recording, RecordingStatus};
use crate::infrastructure::{AudioFileReader, AudioResampler, AudioSegmenter};
use crate::AudioError;
use sevensense_core::{AudioMetadata, Timestamp};

/// Service for ingesting and processing audio files.
///
/// This service orchestrates the audio ingestion pipeline:
/// 1. Read audio from various file formats
/// 2. Resample to standard rate (32kHz)
/// 3. Segment into individual calls
pub struct AudioIngestionService {
    reader: Arc<dyn AudioFileReader>,
    resampler: Arc<dyn AudioResampler>,
    segmenter: Arc<dyn AudioSegmenter>,
}

impl AudioIngestionService {
    /// Creates a new AudioIngestionService with the given components.
    #[must_use]
    pub fn new(
        reader: Arc<dyn AudioFileReader>,
        resampler: Arc<dyn AudioResampler>,
        segmenter: Arc<dyn AudioSegmenter>,
    ) -> Self {
        Self {
            reader,
            resampler,
            segmenter,
        }
    }

    /// Ingests an audio file and creates a Recording entity.
    ///
    /// This performs the following steps:
    /// 1. Read the audio file and extract metadata
    /// 2. Convert to mono if stereo
    /// 3. Resample to 32kHz if needed
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// A Recording with samples loaded and ready for segmentation.
    #[instrument(skip(self), fields(path = %path.display()))]
    pub async fn ingest_file(&self, path: &Path) -> Result<Recording, AudioError> {
        info!("Starting audio ingestion");

        // Read the audio file
        let (samples, metadata) = self.reader.read(path).await?;
        debug!(
            sample_rate = metadata.sample_rate,
            channels = metadata.channels,
            duration_ms = metadata.duration_ms,
            "Read audio file"
        );

        // Convert to mono if needed
        let mono_samples = if metadata.channels > 1 {
            debug!("Converting {} channels to mono", metadata.channels);
            Self::to_mono(&samples, metadata.channels)
        } else {
            samples
        };

        // Resample if needed
        let (resampled, final_rate) = if metadata.sample_rate != crate::TARGET_SAMPLE_RATE {
            debug!(
                "Resampling from {} Hz to {} Hz",
                metadata.sample_rate,
                crate::TARGET_SAMPLE_RATE
            );
            let resampled = self
                .resampler
                .resample(&mono_samples, metadata.sample_rate)?;
            (resampled, crate::TARGET_SAMPLE_RATE)
        } else {
            (mono_samples, metadata.sample_rate)
        };

        // Calculate new duration after resampling
        let duration_ms = (resampled.len() as u64 * 1000) / u64::from(final_rate);

        // Create updated metadata
        let final_metadata = AudioMetadata::new(
            final_rate,
            1, // Now mono
            metadata.bits_per_sample,
            duration_ms,
            metadata.format.clone(),
            metadata.file_size_bytes,
        );

        // Create the recording entity
        let mut recording = Recording::new(
            path.to_path_buf(),
            final_metadata,
            None, // Location to be set separately
            Timestamp::now(),
        );

        recording.set_samples(resampled);
        recording.set_status(RecordingStatus::Processing);

        info!(
            recording_id = %recording.id,
            duration_ms = recording.duration_ms(),
            "Audio ingestion complete"
        );

        Ok(recording)
    }

    /// Segments a recording into individual call segments.
    ///
    /// This analyzes the audio to find regions of interest (potential
    /// bird calls) based on energy levels and signal characteristics.
    ///
    /// # Arguments
    /// * `recording` - A Recording with samples loaded
    ///
    /// # Returns
    /// A vector of detected CallSegments, also added to the recording.
    #[instrument(skip(self, recording), fields(recording_id = %recording.id))]
    pub async fn segment_recording(
        &self,
        recording: &mut Recording,
    ) -> Result<Vec<CallSegment>, AudioError> {
        let samples = recording
            .samples
            .as_ref()
            .ok_or_else(|| AudioError::invalid_data("Recording has no samples loaded"))?;

        info!("Starting segmentation");

        let segments = self.segmenter.segment(
            samples,
            recording.metadata.sample_rate,
            recording.id,
        )?;

        let viable_count = segments.iter().filter(|s| s.is_viable()).count();
        info!(
            total_segments = segments.len(),
            viable_segments = viable_count,
            "Segmentation complete"
        );

        // Add segments to recording
        for segment in &segments {
            recording.add_segment(segment.clone());
        }

        recording.set_status(RecordingStatus::Processed);

        Ok(segments)
    }

    /// Converts multi-channel audio to mono by averaging channels.
    fn to_mono(samples: &[f32], channels: u16) -> Vec<f32> {
        let channels = channels as usize;
        let frame_count = samples.len() / channels;
        let mut mono = Vec::with_capacity(frame_count);

        for frame in 0..frame_count {
            let mut sum = 0.0f32;
            for ch in 0..channels {
                sum += samples[frame * channels + ch];
            }
            mono.push(sum / channels as f32);
        }

        mono
    }

    /// Extracts a segment's samples from the recording.
    ///
    /// # Arguments
    /// * `recording` - The source recording
    /// * `segment` - The segment to extract
    ///
    /// # Returns
    /// The audio samples for just this segment.
    pub fn extract_segment_samples(
        &self,
        recording: &Recording,
        segment: &CallSegment,
    ) -> Result<Vec<f32>, AudioError> {
        let samples = recording
            .samples
            .as_ref()
            .ok_or_else(|| AudioError::invalid_data("Recording has no samples loaded"))?;

        let sample_rate = recording.metadata.sample_rate;
        let start_sample = (segment.start_ms as usize * sample_rate as usize) / 1000;
        let end_sample = (segment.end_ms as usize * sample_rate as usize) / 1000;

        if end_sample > samples.len() {
            warn!(
                segment_end = end_sample,
                samples_len = samples.len(),
                "Segment extends beyond recording"
            );
        }

        let end_sample = end_sample.min(samples.len());
        let start_sample = start_sample.min(end_sample);

        Ok(samples[start_sample..end_sample].to_vec())
    }
}

/// Configuration for the audio ingestion service.
#[derive(Debug, Clone)]
pub struct AudioIngestionConfig {
    /// Target sample rate for all processing.
    pub target_sample_rate: u32,
    /// Minimum segment duration in milliseconds.
    pub min_segment_duration_ms: u64,
    /// Maximum segment duration in milliseconds.
    pub max_segment_duration_ms: u64,
    /// Energy threshold for segment detection.
    pub energy_threshold: f32,
}

impl Default for AudioIngestionConfig {
    fn default() -> Self {
        Self {
            target_sample_rate: crate::TARGET_SAMPLE_RATE,
            min_segment_duration_ms: 100,
            max_segment_duration_ms: 10_000,
            energy_threshold: 0.01,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_mono_stereo() {
        // Stereo samples: [L, R, L, R, ...]
        let stereo = vec![0.5, 0.3, 0.8, 0.6, 0.2, 0.4];
        let mono = AudioIngestionService::to_mono(&stereo, 2);

        assert_eq!(mono.len(), 3);
        assert!((mono[0] - 0.4).abs() < 0.001); // (0.5 + 0.3) / 2
        assert!((mono[1] - 0.7).abs() < 0.001); // (0.8 + 0.6) / 2
        assert!((mono[2] - 0.3).abs() < 0.001); // (0.2 + 0.4) / 2
    }

    #[test]
    fn test_config_defaults() {
        let config = AudioIngestionConfig::default();
        assert_eq!(config.target_sample_rate, 32_000);
        assert_eq!(config.min_segment_duration_ms, 100);
    }
}
