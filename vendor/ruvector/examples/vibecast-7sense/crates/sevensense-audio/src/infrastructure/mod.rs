//! Infrastructure layer for the audio bounded context.
//!
//! This module contains technical implementations for:
//! - Audio file reading (multiple formats via Symphonia)
//! - Resampling (via Rubato)
//! - Signal segmentation (energy-based algorithm)

pub mod file_reader;
pub mod resampler;
pub mod segmenter;

pub use file_reader::*;
pub use resampler::*;
pub use segmenter::*;

use async_trait::async_trait;
use sevensense_core::{AudioMetadata, RecordingId};
use std::path::Path;

use crate::domain::entities::CallSegment;
use crate::AudioError;

/// Trait for reading audio files.
#[async_trait]
pub trait AudioFileReader: Send + Sync {
    /// Reads an audio file and returns samples with metadata.
    ///
    /// # Arguments
    /// * `path` - Path to the audio file
    ///
    /// # Returns
    /// A tuple of (samples, metadata). Samples are interleaved if multi-channel.
    async fn read(&self, path: &Path) -> Result<(Vec<f32>, AudioMetadata), AudioError>;

    /// Checks if this reader supports the given file extension.
    fn supports_extension(&self, ext: &str) -> bool;
}

/// Trait for audio resampling.
pub trait AudioResampler: Send + Sync {
    /// Resamples audio to the target sample rate.
    ///
    /// # Arguments
    /// * `samples` - Input samples (mono)
    /// * `source_rate` - Source sample rate in Hz
    ///
    /// # Returns
    /// Resampled audio at the target rate.
    fn resample(&self, samples: &[f32], source_rate: u32) -> Result<Vec<f32>, AudioError>;

    /// Returns the target sample rate.
    fn target_rate(&self) -> u32;
}

/// Trait for audio segmentation.
pub trait AudioSegmenter: Send + Sync {
    /// Segments audio into regions of interest.
    ///
    /// # Arguments
    /// * `samples` - Input samples (mono)
    /// * `sample_rate` - Sample rate in Hz
    /// * `recording_id` - ID of the parent recording
    ///
    /// # Returns
    /// A vector of detected segments.
    fn segment(
        &self,
        samples: &[f32],
        sample_rate: u32,
        recording_id: RecordingId,
    ) -> Result<Vec<CallSegment>, AudioError>;
}
