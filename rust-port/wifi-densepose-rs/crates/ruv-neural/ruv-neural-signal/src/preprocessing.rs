//! Configurable multi-stage preprocessing pipeline for neural data.
//!
//! Provides a builder-pattern pipeline that chains filtering and artifact
//! rejection stages. The default pipeline applies:
//! 1. Notch filter at 50 Hz (power line noise removal)
//! 2. Bandpass filter 1-200 Hz
//! 3. Artifact rejection (eye blink + muscle)

use ruv_neural_core::error::{Result, RuvNeuralError};
use ruv_neural_core::signal::MultiChannelTimeSeries;

use crate::artifact::{detect_eye_blinks, detect_muscle_artifact, reject_artifacts};
use crate::filter::{BandpassFilter, NotchFilter, SignalProcessor};

/// A processing stage in the pipeline.
enum PipelineStage {
    /// Apply a notch filter to each channel.
    Notch(NotchFilter),
    /// Apply a bandpass filter to each channel.
    Bandpass(BandpassFilter),
    /// Run artifact detection and rejection.
    ArtifactRejection,
}

/// Configurable preprocessing pipeline for multi-channel neural data.
///
/// # Example
/// ```ignore
/// use ruv_neural_signal::PreprocessingPipeline;
///
/// let pipeline = PreprocessingPipeline::default_pipeline(1000.0);
/// let clean_data = pipeline.process(&raw_data).unwrap();
/// ```
pub struct PreprocessingPipeline {
    stages: Vec<PipelineStage>,
    sample_rate: f64,
}

impl PreprocessingPipeline {
    /// Create a new empty pipeline.
    pub fn new(sample_rate: f64) -> Self {
        Self {
            stages: Vec::new(),
            sample_rate,
        }
    }

    /// Create the default preprocessing pipeline:
    /// 1. Notch at 50 Hz (BW=2 Hz)
    /// 2. Bandpass 1-200 Hz (order 4)
    /// 3. Artifact rejection
    pub fn default_pipeline(sample_rate: f64) -> Self {
        let mut pipeline = Self::new(sample_rate);
        pipeline.add_notch(50.0, 2.0);
        pipeline.add_bandpass(1.0, 200.0, 4);
        pipeline.add_artifact_rejection();
        pipeline
    }

    /// Add a notch filter stage.
    ///
    /// # Arguments
    /// * `center_hz` - Center frequency to reject
    /// * `bandwidth_hz` - Rejection bandwidth
    pub fn add_notch(&mut self, center_hz: f64, bandwidth_hz: f64) {
        let filter = NotchFilter::new(center_hz, bandwidth_hz, self.sample_rate);
        self.stages.push(PipelineStage::Notch(filter));
    }

    /// Add a bandpass filter stage.
    ///
    /// # Arguments
    /// * `low_hz` - Lower cutoff frequency
    /// * `high_hz` - Upper cutoff frequency
    /// * `order` - Filter order
    pub fn add_bandpass(&mut self, low_hz: f64, high_hz: f64, order: usize) {
        let filter = BandpassFilter::new(order, low_hz, high_hz, self.sample_rate);
        self.stages.push(PipelineStage::Bandpass(filter));
    }

    /// Add an artifact rejection stage.
    ///
    /// Runs eye blink and muscle artifact detection, then interpolates
    /// across detected artifact periods.
    pub fn add_artifact_rejection(&mut self) {
        self.stages.push(PipelineStage::ArtifactRejection);
    }

    /// Process multi-channel data through all pipeline stages.
    ///
    /// Each stage is applied sequentially. Filter stages process each
    /// channel independently. Artifact rejection operates on all channels.
    pub fn process(&self, data: &MultiChannelTimeSeries) -> Result<MultiChannelTimeSeries> {
        if data.num_channels == 0 || data.num_samples == 0 {
            return Err(RuvNeuralError::Signal(
                "Cannot process empty data".into(),
            ));
        }

        let mut current = data.clone();

        for stage in &self.stages {
            current = match stage {
                PipelineStage::Notch(filter) => {
                    let new_data: Vec<Vec<f64>> = current
                        .data
                        .iter()
                        .map(|ch| filter.process(ch))
                        .collect();
                    MultiChannelTimeSeries {
                        data: new_data,
                        ..current
                    }
                }
                PipelineStage::Bandpass(filter) => {
                    let new_data: Vec<Vec<f64>> = current
                        .data
                        .iter()
                        .map(|ch| filter.process(ch))
                        .collect();
                    MultiChannelTimeSeries {
                        data: new_data,
                        ..current
                    }
                }
                PipelineStage::ArtifactRejection => {
                    // Collect artifact ranges from all channels
                    let mut all_ranges = Vec::new();
                    for ch in &current.data {
                        let blinks = detect_eye_blinks(ch, current.sample_rate_hz);
                        let muscle = detect_muscle_artifact(ch, current.sample_rate_hz);
                        all_ranges.extend(blinks);
                        all_ranges.extend(muscle);
                    }

                    // Sort and merge overlapping ranges
                    all_ranges.sort_by_key(|&(s, _)| s);
                    let merged = merge_ranges(&all_ranges);

                    reject_artifacts(&current, &merged)
                }
            };
        }

        Ok(current)
    }
}

/// Merge overlapping or adjacent ranges.
fn merge_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if ranges.is_empty() {
        return Vec::new();
    }

    let mut merged = Vec::new();
    let (mut cur_start, mut cur_end) = ranges[0];

    for &(s, e) in &ranges[1..] {
        if s <= cur_end {
            cur_end = cur_end.max(e);
        } else {
            merged.push((cur_start, cur_end));
            cur_start = s;
            cur_end = e;
        }
    }
    merged.push((cur_start, cur_end));

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruv_neural_core::signal::MultiChannelTimeSeries;
    use std::f64::consts::PI;

    #[test]
    fn preprocessing_pipeline_processes_without_error() {
        let sr = 1000.0;
        let n = 2000;
        // Create multi-channel test data
        let data = MultiChannelTimeSeries {
            data: vec![
                (0..n)
                    .map(|i| {
                        let t = i as f64 / sr;
                        (2.0 * PI * 10.0 * t).sin() + 0.1 * (2.0 * PI * 50.0 * t).sin()
                    })
                    .collect(),
                (0..n)
                    .map(|i| {
                        let t = i as f64 / sr;
                        (2.0 * PI * 20.0 * t).sin() + 0.05 * (2.0 * PI * 50.0 * t).sin()
                    })
                    .collect(),
            ],
            sample_rate_hz: sr,
            num_channels: 2,
            num_samples: n,
            timestamp_start: 0.0,
        };

        let pipeline = PreprocessingPipeline::default_pipeline(sr);
        let result = pipeline.process(&data);

        assert!(result.is_ok(), "Pipeline should process without error");
        let clean = result.unwrap();
        assert_eq!(clean.num_channels, 2);
        assert_eq!(clean.num_samples, n);
    }

    #[test]
    fn empty_data_returns_error() {
        let data = MultiChannelTimeSeries {
            data: vec![],
            sample_rate_hz: 1000.0,
            num_channels: 0,
            num_samples: 0,
            timestamp_start: 0.0,
        };

        let pipeline = PreprocessingPipeline::default_pipeline(1000.0);
        let result = pipeline.process(&data);
        assert!(result.is_err());
    }

    #[test]
    fn custom_pipeline_builds_and_runs() {
        let sr = 500.0;
        let n = 1000;
        let data = MultiChannelTimeSeries {
            data: vec![(0..n)
                .map(|i| {
                    let t = i as f64 / sr;
                    (2.0 * PI * 10.0 * t).sin()
                })
                .collect()],
            sample_rate_hz: sr,
            num_channels: 1,
            num_samples: n,
            timestamp_start: 0.0,
        };

        let mut pipeline = PreprocessingPipeline::new(sr);
        pipeline.add_notch(60.0, 2.0); // 60 Hz notch for US power line
        pipeline.add_bandpass(0.5, 100.0, 2);

        let result = pipeline.process(&data);
        assert!(result.is_ok());
    }
}
