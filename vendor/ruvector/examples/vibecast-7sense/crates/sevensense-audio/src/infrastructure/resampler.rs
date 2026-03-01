//! Audio resampling implementation using Rubato.
//!
//! Rubato provides high-quality sample rate conversion using
//! polyphase sinc interpolation.

use rubato::{
    FftFixedInOut, Resampler, SincFixedIn,
    SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use tracing::{debug, instrument};

use super::AudioResampler;
use crate::AudioError;

/// High-quality audio resampler using Rubato.
pub struct RubatoResampler {
    /// Target sample rate in Hz.
    target_rate: u32,
    /// Resampler quality settings.
    quality: ResamplerQuality,
}

/// Quality presets for resampling.
#[derive(Debug, Clone, Copy, Default)]
pub enum ResamplerQuality {
    /// Fast resampling, lower quality.
    Fast,
    /// Balanced quality and speed.
    #[default]
    Normal,
    /// High quality, slower processing.
    High,
    /// Maximum quality for critical applications.
    Best,
}

impl ResamplerQuality {
    /// Returns the sinc length for this quality level.
    fn sinc_len(&self) -> usize {
        match self {
            Self::Fast => 64,
            Self::Normal => 128,
            Self::High => 256,
            Self::Best => 512,
        }
    }

    /// Returns the oversampling factor for this quality level.
    fn oversampling_factor(&self) -> usize {
        match self {
            Self::Fast => 64,
            Self::Normal => 128,
            Self::High => 256,
            Self::Best => 256,
        }
    }
}

impl RubatoResampler {
    /// Creates a new RubatoResampler with the target sample rate.
    ///
    /// # Arguments
    /// * `target_rate` - Target sample rate in Hz (typically 32000)
    ///
    /// # Errors
    /// Returns an error if the target rate is invalid.
    pub fn new(target_rate: u32) -> Result<Self, AudioError> {
        if target_rate == 0 {
            return Err(AudioError::Config("Target rate must be positive".into()));
        }
        Ok(Self {
            target_rate,
            quality: ResamplerQuality::default(),
        })
    }

    /// Creates a resampler with specific quality settings.
    pub fn with_quality(mut self, quality: ResamplerQuality) -> Self {
        self.quality = quality;
        self
    }

    /// Creates the Rubato resampler instance.
    fn create_resampler(
        &self,
        source_rate: u32,
        chunk_size: usize,
    ) -> Result<SincFixedIn<f32>, AudioError> {
        let params = SincInterpolationParameters {
            sinc_len: self.quality.sinc_len(),
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Cubic,
            oversampling_factor: self.quality.oversampling_factor(),
            window: WindowFunction::BlackmanHarris2,
        };

        let resample_ratio = f64::from(self.target_rate) / f64::from(source_rate);

        SincFixedIn::new(
            resample_ratio,
            2.0, // Max relative deviation from nominal ratio
            params,
            chunk_size,
            1, // Mono
        )
        .map_err(|e| AudioError::resampling(format!("Failed to create resampler: {e}")))
    }
}

impl AudioResampler for RubatoResampler {
    #[instrument(skip(self, samples), fields(source_rate = source_rate, target_rate = self.target_rate))]
    fn resample(&self, samples: &[f32], source_rate: u32) -> Result<Vec<f32>, AudioError> {
        if source_rate == self.target_rate {
            debug!("No resampling needed, rates match");
            return Ok(samples.to_vec());
        }

        if samples.is_empty() {
            return Ok(Vec::new());
        }

        let resample_ratio = f64::from(self.target_rate) / f64::from(source_rate);
        let expected_output_len = (samples.len() as f64 * resample_ratio).ceil() as usize;

        // Use chunk-based processing for memory efficiency
        let chunk_size = 1024.min(samples.len());
        let mut resampler = self.create_resampler(source_rate, chunk_size)?;

        let mut output = Vec::with_capacity(expected_output_len);
        let mut input_pos = 0;

        // Process full chunks
        while input_pos + chunk_size <= samples.len() {
            let input_chunk = vec![samples[input_pos..input_pos + chunk_size].to_vec()];
            let output_chunk = resampler
                .process(&input_chunk, None)
                .map_err(|e| AudioError::resampling(format!("Resampling failed: {e}")))?;

            output.extend(&output_chunk[0]);
            input_pos += chunk_size;
        }

        // Handle remaining samples
        if input_pos < samples.len() {
            let remaining = samples.len() - input_pos;

            // Pad the remaining samples to chunk size
            let mut padded = samples[input_pos..].to_vec();
            padded.resize(chunk_size, 0.0);

            let input_chunk = vec![padded];
            let output_chunk = resampler
                .process(&input_chunk, None)
                .map_err(|e| AudioError::resampling(format!("Final chunk failed: {e}")))?;

            // Only take the proportional amount of output
            let output_samples = ((remaining as f64) * resample_ratio).ceil() as usize;
            output.extend(&output_chunk[0][..output_samples.min(output_chunk[0].len())]);
        }

        debug!(
            input_samples = samples.len(),
            output_samples = output.len(),
            "Resampling complete"
        );

        Ok(output)
    }

    fn target_rate(&self) -> u32 {
        self.target_rate
    }
}

/// FFT-based resampler for specific ratio resampling.
///
/// More efficient when the ratio is a simple fraction.
pub struct FftResampler {
    target_rate: u32,
}

impl FftResampler {
    /// Creates a new FFT-based resampler.
    #[must_use]
    pub const fn new(target_rate: u32) -> Self {
        Self { target_rate }
    }

    /// Resamples using FFT method.
    pub fn resample(&self, samples: &[f32], source_rate: u32) -> Result<Vec<f32>, AudioError> {
        if source_rate == self.target_rate {
            return Ok(samples.to_vec());
        }

        if samples.is_empty() {
            return Ok(Vec::new());
        }

        // Calculate GCD for ratio simplification
        let gcd = Self::gcd(source_rate, self.target_rate);
        let upsample_factor = self.target_rate / gcd;
        let downsample_factor = source_rate / gcd;

        // For very complex ratios, fall back to sinc interpolation
        if upsample_factor > 16 || downsample_factor > 16 {
            let rubato = RubatoResampler::new(self.target_rate)?;
            return rubato.resample(samples, source_rate);
        }

        let chunk_size = 1024.min(samples.len());

        let mut resampler = FftFixedInOut::<f32>::new(
            source_rate as usize,
            self.target_rate as usize,
            chunk_size,
            1, // Mono
        )
        .map_err(|e| AudioError::resampling(format!("FFT resampler creation failed: {e}")))?;

        let resample_ratio = f64::from(self.target_rate) / f64::from(source_rate);
        let expected_len = (samples.len() as f64 * resample_ratio).ceil() as usize;
        let mut output = Vec::with_capacity(expected_len);

        let input_frames = resampler.input_frames_next();
        let mut pos = 0;

        while pos + input_frames <= samples.len() {
            let input = vec![samples[pos..pos + input_frames].to_vec()];
            let result = resampler
                .process(&input, None)
                .map_err(|e| AudioError::resampling(e.to_string()))?;
            output.extend(&result[0]);
            pos += input_frames;
        }

        Ok(output)
    }

    /// Calculates greatest common divisor using Euclidean algorithm.
    const fn gcd(mut a: u32, mut b: u32) -> u32 {
        while b != 0 {
            let temp = b;
            b = a % b;
            a = temp;
        }
        a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resampler_creation() {
        let resampler = RubatoResampler::new(32000);
        assert!(resampler.is_ok());
        assert_eq!(resampler.unwrap().target_rate(), 32000);
    }

    #[test]
    fn test_resampler_invalid_rate() {
        let result = RubatoResampler::new(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_resample_needed() {
        let resampler = RubatoResampler::new(44100).unwrap();
        let samples: Vec<f32> = (0..1000).map(|i| (i as f32).sin()).collect();

        let result = resampler.resample(&samples, 44100).unwrap();
        assert_eq!(result.len(), samples.len());
    }

    #[test]
    fn test_downsample() {
        let resampler = RubatoResampler::new(32000).unwrap();
        let samples: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.01).sin()).collect();

        let result = resampler.resample(&samples, 44100).unwrap();

        // Output should be approximately 32000/44100 * input length
        let expected_ratio = 32000.0 / 44100.0;
        let expected_len = (samples.len() as f64 * expected_ratio) as usize;

        // Allow 5% tolerance due to filtering artifacts
        assert!((result.len() as f64 - expected_len as f64).abs() < expected_len as f64 * 0.05);
    }

    #[test]
    fn test_upsample() {
        let resampler = RubatoResampler::new(48000).unwrap();
        let samples: Vec<f32> = (0..32000).map(|i| (i as f32 * 0.01).sin()).collect();

        let result = resampler.resample(&samples, 32000).unwrap();

        // Output should be approximately 48000/32000 * input length
        let expected_ratio = 48000.0 / 32000.0;
        let expected_len = (samples.len() as f64 * expected_ratio) as usize;

        assert!((result.len() as f64 - expected_len as f64).abs() < expected_len as f64 * 0.05);
    }

    #[test]
    fn test_empty_input() {
        let resampler = RubatoResampler::new(32000).unwrap();
        let result = resampler.resample(&[], 44100).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_quality_settings() {
        assert_eq!(ResamplerQuality::Fast.sinc_len(), 64);
        assert_eq!(ResamplerQuality::Best.sinc_len(), 512);
    }

    #[test]
    fn test_gcd() {
        assert_eq!(FftResampler::gcd(44100, 32000), 100);
        assert_eq!(FftResampler::gcd(48000, 44100), 300);
    }
}
