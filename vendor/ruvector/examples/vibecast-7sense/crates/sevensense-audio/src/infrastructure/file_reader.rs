//! Audio file reading implementation using Symphonia.
//!
//! Symphonia provides support for multiple audio formats including
//! WAV, FLAC, MP3, OGG Vorbis, and more.

use async_trait::async_trait;
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use tracing::{debug, instrument};

use super::AudioFileReader;
use crate::AudioError;
use sevensense_core::AudioMetadata;

/// Audio file reader using Symphonia for multi-format support.
pub struct SymphoniaFileReader {
    /// Supported file extensions.
    supported_extensions: Vec<&'static str>,
}

impl SymphoniaFileReader {
    /// Creates a new SymphoniaFileReader.
    #[must_use]
    pub fn new() -> Self {
        Self {
            supported_extensions: vec![
                "wav", "wave", "flac", "mp3", "ogg", "oga", "opus", "m4a", "aac", "aiff", "aif",
            ],
        }
    }

    /// Converts an audio buffer to f32 samples.
    fn buffer_to_samples(buf: AudioBufferRef<'_>) -> Vec<f32> {
        match buf {
            AudioBufferRef::F32(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        samples.push(buf.chan(ch)[frame]);
                    }
                }
                samples
            }
            AudioBufferRef::S16(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        // Convert i16 to f32 (-1.0 to 1.0)
                        let sample = f32::from(buf.chan(ch)[frame]) / f32::from(i16::MAX);
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::S24(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);
                const MAX_24: f32 = 8_388_607.0; // 2^23 - 1

                for frame in 0..frames {
                    for ch in 0..channels {
                        let sample = buf.chan(ch)[frame].inner() as f32 / MAX_24;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::S32(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        let sample = buf.chan(ch)[frame] as f32 / i32::MAX as f32;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::F64(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        samples.push(buf.chan(ch)[frame] as f32);
                    }
                }
                samples
            }
            AudioBufferRef::U8(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        // Convert u8 (0-255) to f32 (-1.0 to 1.0)
                        let sample = (f32::from(buf.chan(ch)[frame]) - 128.0) / 128.0;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::U16(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        // Convert u16 to f32 (-1.0 to 1.0)
                        let sample = (buf.chan(ch)[frame] as f32 / u16::MAX as f32) * 2.0 - 1.0;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::U24(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);
                const MAX_24: f32 = 16_777_215.0; // 2^24 - 1

                for frame in 0..frames {
                    for ch in 0..channels {
                        let sample = (buf.chan(ch)[frame].inner() as f32 / MAX_24) * 2.0 - 1.0;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::U32(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        let sample = (buf.chan(ch)[frame] as f32 / u32::MAX as f32) * 2.0 - 1.0;
                        samples.push(sample);
                    }
                }
                samples
            }
            AudioBufferRef::S8(buf) => {
                let channels = buf.spec().channels.count();
                let frames = buf.frames();
                let mut samples = Vec::with_capacity(frames * channels);

                for frame in 0..frames {
                    for ch in 0..channels {
                        let sample = f32::from(buf.chan(ch)[frame]) / f32::from(i8::MAX);
                        samples.push(sample);
                    }
                }
                samples
            }
        }
    }
}

impl Default for SymphoniaFileReader {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AudioFileReader for SymphoniaFileReader {
    #[instrument(skip(self), fields(path = %path.display()))]
    async fn read(&self, path: &Path) -> Result<(Vec<f32>, AudioMetadata), AudioError> {
        // Get file metadata for size
        let file_metadata = std::fs::metadata(path)
            .map_err(|e| AudioError::file_read(path, e.to_string()))?;
        let file_size = file_metadata.len();

        // Open the file
        let file = File::open(path)
            .map_err(|e| AudioError::file_read(path, e.to_string()))?;

        // Create media source stream
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // Create a hint for the format
        let mut hint = Hint::new();
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            hint.with_extension(ext);
        }

        // Probe the format
        let probed = symphonia::default::get_probe()
            .format(&hint, mss, &FormatOptions::default(), &MetadataOptions::default())
            .map_err(|e| AudioError::file_read(path, format!("Failed to probe format: {e}")))?;

        let mut format = probed.format;

        // Find the first audio track
        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| AudioError::file_read(path, "No audio track found"))?;

        let track_id = track.id;

        // Get codec parameters
        let codec_params = track.codec_params.clone();

        let sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| AudioError::file_read(path, "Unknown sample rate"))?;

        let channels = codec_params
            .channels
            .map(|c| c.count() as u16)
            .unwrap_or(1);

        let bits_per_sample = codec_params
            .bits_per_sample
            .unwrap_or(16) as u16;

        debug!(
            sample_rate = sample_rate,
            channels = channels,
            bits = bits_per_sample,
            "Decoded audio parameters"
        );

        // Create decoder
        let mut decoder = symphonia::default::get_codecs()
            .make(&codec_params, &DecoderOptions::default())
            .map_err(|e| AudioError::file_read(path, format!("Failed to create decoder: {e}")))?;

        // Decode all packets
        let mut all_samples = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::IoError(ref e))
                    if e.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(e) => {
                    return Err(AudioError::file_read(path, format!("Decode error: {e}")));
                }
            };

            // Skip packets from other tracks
            if packet.track_id() != track_id {
                continue;
            }

            // Decode the packet
            let decoded = match decoder.decode(&packet) {
                Ok(decoded) => decoded,
                Err(symphonia::core::errors::Error::DecodeError(e)) => {
                    debug!("Decode error (skipping packet): {}", e);
                    continue;
                }
                Err(e) => {
                    return Err(AudioError::file_read(path, format!("Decode error: {e}")));
                }
            };

            // Convert to f32 samples
            let samples = Self::buffer_to_samples(decoded);
            all_samples.extend(samples);
        }

        // Calculate duration
        let frame_count = all_samples.len() / channels as usize;
        let duration_ms = (frame_count as u64 * 1000) / u64::from(sample_rate);

        // Get format string from extension
        let format_str = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_lowercase();

        let metadata = AudioMetadata::new(
            sample_rate,
            channels,
            bits_per_sample,
            duration_ms,
            format_str,
            file_size,
        );

        debug!(
            total_samples = all_samples.len(),
            duration_ms = duration_ms,
            "Audio decoding complete"
        );

        Ok((all_samples, metadata))
    }

    fn supports_extension(&self, ext: &str) -> bool {
        self.supported_extensions
            .contains(&ext.to_lowercase().as_str())
    }
}

/// Simple WAV file reader using hound (for simple cases).
pub struct HoundWavReader;

impl HoundWavReader {
    /// Creates a new HoundWavReader.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Reads a WAV file synchronously.
    pub fn read_wav(&self, path: &Path) -> Result<(Vec<f32>, AudioMetadata), AudioError> {
        let reader = hound::WavReader::open(path)
            .map_err(|e| AudioError::file_read(path, e.to_string()))?;

        let spec = reader.spec();
        let sample_rate = spec.sample_rate;
        let channels = spec.channels;
        let bits_per_sample = spec.bits_per_sample;

        let file_size = std::fs::metadata(path)
            .map(|m| m.len())
            .unwrap_or(0);

        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => {
                reader.into_samples::<f32>()
                    .filter_map(Result::ok)
                    .collect()
            }
            hound::SampleFormat::Int => {
                let max_val = (1i32 << (bits_per_sample - 1)) as f32;
                reader.into_samples::<i32>()
                    .filter_map(Result::ok)
                    .map(|s| s as f32 / max_val)
                    .collect()
            }
        };

        let frame_count = samples.len() / channels as usize;
        let duration_ms = (frame_count as u64 * 1000) / u64::from(sample_rate);

        let metadata = AudioMetadata::new(
            sample_rate,
            channels,
            bits_per_sample,
            duration_ms,
            "wav".to_string(),
            file_size,
        );

        Ok((samples, metadata))
    }
}

impl Default for HoundWavReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symphonia_supported_extensions() {
        let reader = SymphoniaFileReader::new();
        assert!(reader.supports_extension("wav"));
        assert!(reader.supports_extension("WAV"));
        assert!(reader.supports_extension("flac"));
        assert!(reader.supports_extension("mp3"));
        assert!(!reader.supports_extension("txt"));
    }
}
