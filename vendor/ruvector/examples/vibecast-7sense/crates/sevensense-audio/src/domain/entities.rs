//! Domain entities for audio processing.
//!
//! These are the core aggregates of the audio bounded context.

use serde::{Deserialize, Serialize};
use sevensense_core::{
    AudioMetadata, GeoLocation, RecordingId, SegmentId, Timestamp,
};
use std::path::PathBuf;

/// Represents an audio recording from the field.
///
/// A Recording is the aggregate root for the audio context. It contains
/// metadata about the source file and a collection of identified call
/// segments extracted during analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recording {
    /// Unique identifier for this recording.
    pub id: RecordingId,

    /// Path to the original source file.
    pub source_path: PathBuf,

    /// Audio metadata (sample rate, channels, duration, etc.).
    pub metadata: AudioMetadata,

    /// Geographic location where the recording was made.
    pub location: Option<GeoLocation>,

    /// Timestamp when the recording was captured.
    pub recorded_at: Timestamp,

    /// Call segments identified in this recording.
    pub segments: Vec<CallSegment>,

    /// Raw audio samples (mono, resampled to target rate).
    #[serde(skip)]
    pub samples: Option<Vec<f32>>,

    /// Processing status.
    pub status: RecordingStatus,

    /// When this recording was ingested into the system.
    pub ingested_at: Timestamp,
}

impl Recording {
    /// Creates a new Recording with the given parameters.
    #[must_use]
    pub fn new(
        source_path: PathBuf,
        metadata: AudioMetadata,
        location: Option<GeoLocation>,
        recorded_at: Timestamp,
    ) -> Self {
        Self {
            id: RecordingId::new(),
            source_path,
            metadata,
            location,
            recorded_at,
            segments: Vec::new(),
            samples: None,
            status: RecordingStatus::Pending,
            ingested_at: Timestamp::now(),
        }
    }

    /// Adds a call segment to this recording.
    pub fn add_segment(&mut self, segment: CallSegment) {
        self.segments.push(segment);
    }

    /// Returns the number of segments.
    #[must_use]
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Returns the total duration in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.metadata.duration_ms
    }

    /// Updates the recording status.
    pub fn set_status(&mut self, status: RecordingStatus) {
        self.status = status;
    }

    /// Checks if the recording has been processed.
    #[must_use]
    pub fn is_processed(&self) -> bool {
        matches!(self.status, RecordingStatus::Processed)
    }

    /// Gets high-quality segments only.
    #[must_use]
    pub fn high_quality_segments(&self) -> Vec<&CallSegment> {
        self.segments
            .iter()
            .filter(|s| matches!(s.signal_quality, SignalQuality::High))
            .collect()
    }

    /// Sets the raw audio samples.
    pub fn set_samples(&mut self, samples: Vec<f32>) {
        self.samples = Some(samples);
    }

    /// Takes ownership of the samples, leaving None in their place.
    pub fn take_samples(&mut self) -> Option<Vec<f32>> {
        self.samples.take()
    }
}

/// Status of recording processing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecordingStatus {
    /// Recording is pending processing.
    Pending,
    /// Recording is currently being processed.
    Processing,
    /// Recording has been fully processed.
    Processed,
    /// Processing failed.
    Failed,
}

impl Default for RecordingStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Represents a segment of audio containing a potential vocalization.
///
/// Call segments are extracted from recordings using energy-based
/// segmentation and represent isolated vocalizations suitable for
/// species classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallSegment {
    /// Unique identifier for this segment.
    pub id: SegmentId,

    /// The recording this segment belongs to.
    pub recording_id: RecordingId,

    /// Start time of the segment in milliseconds from recording start.
    pub start_ms: u64,

    /// End time of the segment in milliseconds from recording start.
    pub end_ms: u64,

    /// Peak amplitude in the segment (0.0 to 1.0).
    pub peak_amplitude: f32,

    /// Root mean square energy of the segment.
    pub rms_energy: f32,

    /// Assessed quality of the signal in this segment.
    pub signal_quality: SignalQuality,

    /// Zero-crossing rate (useful for distinguishing noise from calls).
    pub zero_crossing_rate: f32,

    /// Spectral centroid in Hz (indicates "brightness" of sound).
    pub spectral_centroid: Option<f32>,

    /// Dominant frequency in Hz.
    pub dominant_frequency: Option<f32>,
}

impl CallSegment {
    /// Creates a new CallSegment with the given parameters.
    #[must_use]
    pub fn new(
        recording_id: RecordingId,
        start_ms: u64,
        end_ms: u64,
        peak_amplitude: f32,
        rms_energy: f32,
        signal_quality: SignalQuality,
    ) -> Self {
        Self {
            id: SegmentId::new(),
            recording_id,
            start_ms,
            end_ms,
            peak_amplitude,
            rms_energy,
            signal_quality,
            zero_crossing_rate: 0.0,
            spectral_centroid: None,
            dominant_frequency: None,
        }
    }

    /// Returns the duration of the segment in milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }

    /// Sets the zero-crossing rate.
    pub fn with_zero_crossing_rate(mut self, rate: f32) -> Self {
        self.zero_crossing_rate = rate;
        self
    }

    /// Sets the spectral centroid.
    pub fn with_spectral_centroid(mut self, centroid: f32) -> Self {
        self.spectral_centroid = Some(centroid);
        self
    }

    /// Sets the dominant frequency.
    pub fn with_dominant_frequency(mut self, freq: f32) -> Self {
        self.dominant_frequency = Some(freq);
        self
    }

    /// Checks if this segment meets minimum quality standards.
    #[must_use]
    pub fn is_viable(&self) -> bool {
        !matches!(self.signal_quality, SignalQuality::Noise)
            && self.duration_ms() >= 100
            && self.rms_energy > 0.001
    }
}

/// Quality assessment of the signal in a segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignalQuality {
    /// High quality signal with clear vocalization.
    High,
    /// Medium quality signal, usable but may have some noise.
    Medium,
    /// Low quality signal, may be difficult to classify.
    Low,
    /// Primarily noise, likely not a vocalization.
    Noise,
}

impl SignalQuality {
    /// Assesses signal quality based on SNR and energy metrics.
    #[must_use]
    pub fn from_metrics(snr_db: f32, rms_energy: f32, zero_crossing_rate: f32) -> Self {
        // High SNR and moderate energy suggests good signal
        if snr_db > 20.0 && rms_energy > 0.05 && zero_crossing_rate < 0.3 {
            return Self::High;
        }

        // Moderate SNR
        if snr_db > 10.0 && rms_energy > 0.02 {
            return Self::Medium;
        }

        // Low SNR but some signal present
        if snr_db > 3.0 && rms_energy > 0.01 {
            return Self::Low;
        }

        // Too noisy or no clear signal
        Self::Noise
    }

    /// Returns a numeric score (0.0 to 1.0) for the quality level.
    #[must_use]
    pub fn score(&self) -> f32 {
        match self {
            Self::High => 1.0,
            Self::Medium => 0.7,
            Self::Low => 0.4,
            Self::Noise => 0.1,
        }
    }
}

impl Default for SignalQuality {
    fn default() -> Self {
        Self::Medium
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_metadata() -> AudioMetadata {
        AudioMetadata::new(32000, 1, 16, 5000, "wav".to_string(), 320000)
    }

    #[test]
    fn test_recording_creation() {
        let recording = Recording::new(
            PathBuf::from("/test/recording.wav"),
            create_test_metadata(),
            None,
            Timestamp::now(),
        );

        assert_eq!(recording.segment_count(), 0);
        assert_eq!(recording.duration_ms(), 5000);
        assert!(!recording.is_processed());
    }

    #[test]
    fn test_recording_add_segment() {
        let mut recording = Recording::new(
            PathBuf::from("/test/recording.wav"),
            create_test_metadata(),
            None,
            Timestamp::now(),
        );

        let segment = CallSegment::new(
            recording.id,
            1000,
            2000,
            0.8,
            0.3,
            SignalQuality::High,
        );

        recording.add_segment(segment);
        assert_eq!(recording.segment_count(), 1);
    }

    #[test]
    fn test_segment_duration() {
        let segment = CallSegment::new(
            RecordingId::new(),
            1000,
            2500,
            0.8,
            0.3,
            SignalQuality::High,
        );

        assert_eq!(segment.duration_ms(), 1500);
    }

    #[test]
    fn test_segment_viability() {
        let viable = CallSegment::new(
            RecordingId::new(),
            0,
            500,
            0.5,
            0.1,
            SignalQuality::Medium,
        );
        assert!(viable.is_viable());

        let noise = CallSegment::new(
            RecordingId::new(),
            0,
            500,
            0.1,
            0.001,
            SignalQuality::Noise,
        );
        assert!(!noise.is_viable());
    }

    #[test]
    fn test_signal_quality_from_metrics() {
        assert_eq!(
            SignalQuality::from_metrics(25.0, 0.1, 0.2),
            SignalQuality::High
        );
        assert_eq!(
            SignalQuality::from_metrics(15.0, 0.05, 0.3),
            SignalQuality::Medium
        );
        assert_eq!(
            SignalQuality::from_metrics(5.0, 0.02, 0.4),
            SignalQuality::Low
        );
        assert_eq!(
            SignalQuality::from_metrics(1.0, 0.005, 0.5),
            SignalQuality::Noise
        );
    }
}
