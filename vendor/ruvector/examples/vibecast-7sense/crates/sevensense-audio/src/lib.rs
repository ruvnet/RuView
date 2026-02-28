//! # sevensense-audio
//!
//! Audio processing and segmentation for the 7sense bioacoustics platform.
//!
//! This crate provides:
//! - Audio file decoding (WAV, FLAC, MP3, Ogg)
//! - Sample rate conversion and normalization
//! - Spectrogram generation
//! - Segment detection and extraction
//! - Audio quality analysis
//!
//! ## Architecture
//!
//! The crate follows Domain-Driven Design with clean architecture:
//! - **Domain Layer**: Core entities (Recording, CallSegment) and repository traits
//! - **Application Layer**: Use cases and services (AudioIngestionService)
//! - **Infrastructure Layer**: Technical implementations (file readers, resamplers)
//!
//! ## Example Usage
//!
//! ```rust,no_run
//! use sevensense_audio::application::AudioIngestionService;
//! use sevensense_audio::infrastructure::{SymphoniaFileReader, RubatoResampler, EnergySegmenter};
//! use std::path::Path;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create infrastructure components
//! let reader = Arc::new(SymphoniaFileReader::new());
//! let resampler = Arc::new(RubatoResampler::new(32000)?);
//! let segmenter = Arc::new(EnergySegmenter::default());
//!
//! // Create the service
//! let service = AudioIngestionService::new(reader, resampler, segmenter);
//!
//! // Ingest an audio file
//! let mut recording = service.ingest_file(Path::new("recording.wav")).await?;
//!
//! // Segment the recording to find calls
//! let segments = service.segment_recording(&mut recording).await?;
//! println!("Found {} call segments", segments.len());
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]

pub mod domain;
pub mod application;
pub mod infrastructure;
pub mod spectrogram;

// Re-export main types
pub use domain::entities::{Recording, CallSegment, SignalQuality};
pub use domain::repository::RecordingRepository;
pub use application::services::AudioIngestionService;
pub use application::error::{AudioError, AudioResult};
pub use spectrogram::{MelSpectrogram, SpectrogramConfig};

/// Standard target sample rate for all processing (32 kHz).
pub const TARGET_SAMPLE_RATE: u32 = 32_000;

/// Standard segment duration for analysis (5 seconds).
pub const STANDARD_SEGMENT_DURATION_MS: u64 = 5_000;

/// Crate version information.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
