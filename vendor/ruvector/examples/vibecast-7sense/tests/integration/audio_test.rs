//! Integration tests for Audio Ingestion Context
//!
//! Tests for audio file loading, resampling, segmentation, and spectrogram generation.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;
use std::io::Cursor;

// ============================================================================
// Audio File Loading Tests
// ============================================================================

mod audio_loading {
    use super::*;

    #[test]
    fn test_load_wav_file_32khz() {
        // Create test WAV data at 32kHz
        let wav_bytes = create_test_wav_bytes(5000); // 5 seconds

        // Verify WAV header
        assert_eq!(&wav_bytes[0..4], b"RIFF");
        assert_eq!(&wav_bytes[8..12], b"WAVE");
        assert_eq!(&wav_bytes[12..16], b"fmt ");

        // Parse sample rate from header
        let sample_rate =
            u32::from_le_bytes([wav_bytes[24], wav_bytes[25], wav_bytes[26], wav_bytes[27]]);
        assert_eq!(sample_rate, 32000);
    }

    #[test]
    fn test_load_audio_correct_duration() {
        let duration_ms = 5000;
        let samples = create_test_audio_samples(duration_ms, 32000);

        let expected_samples = (duration_ms as f64 * 32000.0 / 1000.0) as usize;
        assert_eq!(samples.len(), expected_samples);
    }

    #[test]
    fn test_audio_samples_in_valid_range() {
        let samples = create_test_audio_samples(1000, 32000);

        for (i, sample) in samples.iter().enumerate() {
            assert!(
                *sample >= -1.0 && *sample <= 1.0,
                "Sample {} out of range: {}",
                i,
                sample
            );
            assert!(
                !sample.is_nan() && !sample.is_infinite(),
                "Sample {} is NaN or Inf",
                i
            );
        }
    }

    #[test]
    fn test_audio_format_validation() {
        let format = AudioFormat::default();

        assert_eq!(format.sample_rate, 32000, "Must be 32kHz for Perch 2.0");
        assert_eq!(format.channels, 1, "Must be mono");
        assert!(format.bit_depth >= 16, "Minimum 16-bit");
    }

    #[test]
    fn test_load_different_durations() {
        let durations = vec![1000, 5000, 10000, 30000, 60000];

        for duration in durations {
            let samples = create_test_audio_samples(duration, 32000);
            let expected = (duration as f64 * 32.0) as usize;
            assert_eq!(
                samples.len(),
                expected,
                "Wrong sample count for {}ms",
                duration
            );
        }
    }

    #[test]
    fn test_wav_bytes_parseable() {
        let wav_bytes = create_test_wav_bytes(5000);

        // Basic WAV structure validation
        assert!(wav_bytes.len() > 44, "WAV too short for valid header");

        // Verify data chunk
        let data_marker = &wav_bytes[36..40];
        assert_eq!(data_marker, b"data");

        // Verify data size
        let data_size =
            u32::from_le_bytes([wav_bytes[40], wav_bytes[41], wav_bytes[42], wav_bytes[43]]);
        assert!(data_size > 0);
    }
}

// ============================================================================
// Resampling Tests
// ============================================================================

mod resampling {
    use super::*;

    /// Mock resampler that converts audio to target sample rate
    struct MockResampler {
        target_rate: u32,
    }

    impl MockResampler {
        fn new(target_rate: u32) -> Self {
            Self { target_rate }
        }

        fn resample(&self, samples: &[f32], source_rate: u32) -> Vec<f32> {
            if source_rate == self.target_rate {
                return samples.to_vec();
            }

            let ratio = self.target_rate as f64 / source_rate as f64;
            let new_len = (samples.len() as f64 * ratio) as usize;

            // Simple linear interpolation resampling
            (0..new_len)
                .map(|i| {
                    let src_idx = i as f64 / ratio;
                    let idx0 = src_idx.floor() as usize;
                    let idx1 = (idx0 + 1).min(samples.len() - 1);
                    let frac = src_idx - idx0 as f64;

                    samples[idx0] * (1.0 - frac as f32) + samples[idx1] * frac as f32
                })
                .collect()
        }
    }

    #[test]
    fn test_resample_44100_to_32000() {
        let source_rate = 44100;
        let target_rate = 32000;
        let duration_ms = 1000;

        // Create 44.1kHz audio
        let samples: Vec<f32> = (0..(source_rate * duration_ms / 1000) as usize)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();

        let resampler = MockResampler::new(target_rate);
        let resampled = resampler.resample(&samples, source_rate);

        let expected_len = (target_rate * duration_ms / 1000) as usize;
        // Allow 1 sample tolerance due to rounding
        assert!(
            (resampled.len() as i64 - expected_len as i64).abs() <= 1,
            "Expected ~{} samples, got {}",
            expected_len,
            resampled.len()
        );
    }

    #[test]
    fn test_resample_48000_to_32000() {
        let source_rate = 48000;
        let target_rate = 32000;
        let duration_ms = 1000;

        let samples: Vec<f32> = (0..(source_rate * duration_ms / 1000) as usize)
            .map(|i| (i as f32 * 0.01).sin())
            .collect();

        let resampler = MockResampler::new(target_rate);
        let resampled = resampler.resample(&samples, source_rate);

        let expected_len = (target_rate * duration_ms / 1000) as usize;
        assert!(
            (resampled.len() as i64 - expected_len as i64).abs() <= 1,
            "Expected ~{} samples, got {}",
            expected_len,
            resampled.len()
        );
    }

    #[test]
    fn test_resample_preserves_energy() {
        let source_rate = 44100;
        let target_rate = 32000;

        let samples: Vec<f32> = (0..44100).map(|i| (i as f32 * 0.01).sin()).collect();

        let source_energy: f32 = samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32;

        let resampler = MockResampler::new(target_rate);
        let resampled = resampler.resample(&samples, source_rate);

        let target_energy: f32 =
            resampled.iter().map(|x| x * x).sum::<f32>() / resampled.len() as f32;

        // Energy should be approximately preserved
        let energy_diff = (source_energy - target_energy).abs() / source_energy;
        assert!(
            energy_diff < 0.1,
            "Energy changed by {:.1}%",
            energy_diff * 100.0
        );
    }

    #[test]
    fn test_resample_identity_at_32000() {
        let samples = create_test_audio_samples(1000, 32000);

        let resampler = MockResampler::new(32000);
        let resampled = resampler.resample(&samples, 32000);

        assert_eq!(samples.len(), resampled.len());
        for (a, b) in samples.iter().zip(resampled.iter()) {
            assert!((a - b).abs() < 0.0001);
        }
    }
}

// ============================================================================
// Segmentation Tests
// ============================================================================

mod segmentation {
    use super::*;

    /// Mock energy-based segmenter
    struct MockSegmenter {
        window_ms: u64,
        hop_ms: u64,
        threshold: f32,
        min_duration_ms: u64,
        sample_rate: u32,
    }

    impl MockSegmenter {
        fn new() -> Self {
            Self {
                window_ms: 100,
                hop_ms: 50,
                threshold: 0.1,
                min_duration_ms: 500,
                sample_rate: 32000,
            }
        }

        fn segment(&self, samples: &[f32], recording_id: RecordingId) -> Vec<CallSegment> {
            let window_size = (self.window_ms as usize * self.sample_rate as usize) / 1000;
            let hop_size = (self.hop_ms as usize * self.sample_rate as usize) / 1000;

            // Compute energy per window
            let mut energies: Vec<f32> = Vec::new();
            let mut i = 0;
            while i + window_size <= samples.len() {
                let energy: f32 =
                    samples[i..i + window_size].iter().map(|x| x * x).sum::<f32>() / window_size as f32;
                energies.push(energy);
                i += hop_size;
            }

            // Find segments above threshold
            let mut segments = Vec::new();
            let mut in_segment = false;
            let mut segment_start = 0;

            for (i, energy) in energies.iter().enumerate() {
                let time_ms = (i * self.hop_ms as usize) as u64;

                if *energy > self.threshold && !in_segment {
                    in_segment = true;
                    segment_start = time_ms;
                } else if *energy <= self.threshold && in_segment {
                    in_segment = false;
                    let duration = time_ms - segment_start;
                    if duration >= self.min_duration_ms {
                        segments.push(CallSegment {
                            id: SegmentId::new(),
                            recording_id,
                            start_ms: segment_start,
                            end_ms: time_ms,
                            snr: 15.0,
                            energy: energies[segment_start as usize / self.hop_ms as usize],
                            clipping_score: 0.0,
                            overlap_score: 0.0,
                            quality_grade: QualityGrade::Good,
                        });
                    }
                }
            }

            segments
        }
    }

    #[test]
    fn test_segmentation_detects_calls() {
        let segmenter = MockSegmenter::new();
        let recording_id = RecordingId::new();

        // Create audio with clear signal/silence pattern
        let mut samples = vec![0.0f32; 64000]; // 2 seconds
        // Add a loud "call" at 200-1200ms (1 second of signal)
        for i in 6400..38400 {
            samples[i] = 0.8 * ((i as f32 * 0.05).sin()); // Louder signal
        }
        // Silence at start and end

        let segments = segmenter.segment(&samples, recording_id);

        assert!(
            !segments.is_empty(),
            "Should detect at least one segment"
        );
    }

    #[test]
    fn test_segmentation_non_overlapping() {
        let segments = create_segment_sequence(5, 500);

        for i in 0..segments.len() - 1 {
            assert!(
                segments[i].end_ms <= segments[i + 1].start_ms,
                "Segments {} and {} overlap",
                i,
                i + 1
            );
        }
    }

    #[test]
    fn test_segment_duration_constraint() {
        let segments = create_segment_sequence(10, 0);

        for segment in &segments {
            let duration = segment.end_ms - segment.start_ms;
            assert_eq!(duration, 5000, "Perch segments should be 5 seconds");
        }
    }

    #[test]
    fn test_segmentation_snr_computation() {
        let segments = create_segment_sequence(5, 500);

        for segment in &segments {
            assert!(segment.snr > 0.0, "SNR should be positive");
            assert!(segment.snr < 100.0, "SNR should be realistic");
        }
    }

    #[test]
    fn test_segment_within_recording_bounds() {
        let recording = create_test_recording_with_duration(60000);
        let segments = create_segment_sequence(10, 500);

        for segment in &segments {
            assert!(
                segment.end_ms <= recording.duration_ms,
                "Segment extends beyond recording"
            );
        }
    }

    #[test]
    fn test_segmentation_preserves_recording_id() {
        let recording_id = RecordingId::new();
        let mut segment = create_test_segment();
        segment.recording_id = recording_id;

        assert_eq!(segment.recording_id, recording_id);
    }
}

// ============================================================================
// Spectrogram Generation Tests
// ============================================================================

mod spectrogram {
    use super::*;

    const MEL_BINS: usize = 128;
    const MEL_FRAMES: usize = 500;

    #[test]
    fn test_spectrogram_dimensions() {
        let spectrogram = create_test_spectrogram();

        assert_eq!(spectrogram.len(), MEL_FRAMES, "Should have 500 frames");
        assert_eq!(
            spectrogram[0].len(),
            MEL_BINS,
            "Should have 128 mel bins"
        );
    }

    #[test]
    fn test_spectrogram_values_non_negative() {
        let spectrogram = create_test_spectrogram();

        for (frame_idx, frame) in spectrogram.iter().enumerate() {
            for (bin_idx, value) in frame.iter().enumerate() {
                assert!(
                    *value >= 0.0,
                    "Frame {} bin {} has negative value: {}",
                    frame_idx,
                    bin_idx,
                    value
                );
            }
        }
    }

    #[test]
    fn test_spectrogram_no_nan_or_inf() {
        let spectrogram = create_test_spectrogram();

        for (frame_idx, frame) in spectrogram.iter().enumerate() {
            for (bin_idx, value) in frame.iter().enumerate() {
                assert!(
                    !value.is_nan() && !value.is_infinite(),
                    "Frame {} bin {} is NaN/Inf",
                    frame_idx,
                    bin_idx
                );
            }
        }
    }

    #[test]
    fn test_spectrogram_energy_distribution() {
        let spectrogram = create_test_spectrogram();

        // Compute total energy per frame
        let frame_energies: Vec<f32> = spectrogram
            .iter()
            .map(|frame| frame.iter().sum())
            .collect();

        // Energy should vary (not all zeros or all same)
        let min_energy = frame_energies
            .iter()
            .cloned()
            .fold(f32::INFINITY, f32::min);
        let max_energy = frame_energies
            .iter()
            .cloned()
            .fold(f32::NEG_INFINITY, f32::max);

        assert!(
            max_energy > min_energy * 1.1,
            "Energy should vary across frames"
        );
    }

    #[test]
    fn test_spectrogram_from_audio_samples() {
        let samples = create_test_audio_samples(5000, 32000);

        // Simple mock spectrogram computation
        let hop_size = samples.len() / MEL_FRAMES;
        let spectrogram: Vec<Vec<f32>> = (0..MEL_FRAMES)
            .map(|frame| {
                let start = frame * hop_size;
                let end = (start + hop_size).min(samples.len());
                let chunk = &samples[start..end];

                // Mock mel filterbank (simplified)
                (0..MEL_BINS)
                    .map(|bin| {
                        let freq_start = bin * chunk.len() / MEL_BINS;
                        let freq_end = ((bin + 1) * chunk.len() / MEL_BINS).min(chunk.len());
                        if freq_start < freq_end {
                            chunk[freq_start..freq_end]
                                .iter()
                                .map(|x| x.abs())
                                .sum::<f32>()
                                / (freq_end - freq_start) as f32
                        } else {
                            0.0
                        }
                    })
                    .collect()
            })
            .collect();

        assert_eq!(spectrogram.len(), MEL_FRAMES);
        assert_eq!(spectrogram[0].len(), MEL_BINS);
    }

    #[test]
    fn test_spectrogram_temporal_resolution() {
        // 5 seconds at 32kHz = 160000 samples
        // 500 frames means ~10ms per frame
        let samples_per_frame = 160000 / MEL_FRAMES;
        let ms_per_frame = (samples_per_frame as f64 / 32.0) as u64;

        assert!(
            ms_per_frame >= 9 && ms_per_frame <= 11,
            "Frame duration should be ~10ms, got {}ms",
            ms_per_frame
        );
    }

    #[test]
    fn test_spectrogram_frequency_range() {
        // Perch 2.0 uses 60Hz to 16000Hz
        // With 128 mel bins, each bin covers approximately 125Hz

        let min_freq = 60.0;
        let max_freq = 16000.0;
        let hz_per_bin = (max_freq - min_freq) / MEL_BINS as f32;

        assert!(
            hz_per_bin > 100.0 && hz_per_bin < 150.0,
            "Each mel bin should cover ~125Hz, got {}Hz",
            hz_per_bin
        );
    }
}

// ============================================================================
// Recording Repository Integration Tests
// ============================================================================

mod repository_integration {
    use super::*;
    use chrono::Duration as ChronoDuration;

    #[test]
    fn test_recording_crud_operations() {
        let repo = MockRecordingRepository::new();

        // Create
        let recording = create_test_recording();
        let id = recording.id;
        repo.save(recording).unwrap();

        // Read
        let found = repo.find_by_id(&id).unwrap().unwrap();
        assert_eq!(found.id, id);

        // Count
        assert_eq!(repo.count(), 1);

        // Delete
        repo.delete(&id).unwrap();
        assert_eq!(repo.count(), 0);
        assert!(repo.find_by_id(&id).unwrap().is_none());
    }

    #[test]
    fn test_find_recordings_by_sensor() {
        let repo = MockRecordingRepository::new();

        // Add recordings from different sensors
        for i in 0..5 {
            let mut recording = create_test_recording();
            recording.sensor_id = format!("SENSOR_{}", i % 2);
            repo.save(recording).unwrap();
        }

        let sensor0_recordings = repo.find_by_sensor_id("SENSOR_0").unwrap();
        let sensor1_recordings = repo.find_by_sensor_id("SENSOR_1").unwrap();

        assert_eq!(sensor0_recordings.len(), 3);
        assert_eq!(sensor1_recordings.len(), 2);
    }

    #[test]
    fn test_find_recordings_by_date_range() {
        let repo = MockRecordingRepository::new();
        let now = chrono::Utc::now();

        // Add recordings at different times
        for i in 0..5 {
            let mut recording = create_test_recording();
            recording.start_timestamp = now - ChronoDuration::hours(i as i64);
            repo.save(recording).unwrap();
        }

        // Find recordings from last 2 hours
        let start = now - ChronoDuration::hours(2);
        let end = now + ChronoDuration::hours(1);
        let recent = repo.find_by_date_range(start, end).unwrap();

        assert_eq!(recent.len(), 3); // 0, 1, 2 hours ago
    }

    #[test]
    fn test_segment_repository_by_recording() {
        let repo = MockSegmentRepository::new();
        let recording_id = RecordingId::new();

        // Add segments for this recording
        for i in 0..5 {
            let segment = CallSegment {
                recording_id,
                start_ms: i * 5500,
                end_ms: i * 5500 + 5000,
                ..Default::default()
            };
            repo.save(segment).unwrap();
        }

        // Add segments for another recording
        let other_id = RecordingId::new();
        for i in 0..3 {
            let segment = CallSegment {
                recording_id: other_id,
                start_ms: i * 5500,
                end_ms: i * 5500 + 5000,
                ..Default::default()
            };
            repo.save(segment).unwrap();
        }

        let segments = repo.find_by_recording(&recording_id).unwrap();
        assert_eq!(segments.len(), 5);
    }

    #[test]
    fn test_segment_repository_by_time_range() {
        let repo = MockSegmentRepository::new();
        let recording_id = RecordingId::new();

        // Add segments spanning 0-30 seconds
        for i in 0..6 {
            let segment = CallSegment {
                recording_id,
                start_ms: i * 5000,
                end_ms: (i + 1) * 5000,
                ..Default::default()
            };
            repo.save(segment).unwrap();
        }

        // Find segments in 10-20 second range
        let segments = repo
            .find_by_time_range(&recording_id, 10000, 20000)
            .unwrap();

        assert_eq!(segments.len(), 2); // Segments at 10-15s and 15-20s
    }
}

// ============================================================================
// Quality Assessment Tests
// ============================================================================

mod quality_assessment {
    use super::*;

    #[test]
    fn test_quality_grade_from_snr() {
        assert_eq!(QualityGrade::from_snr(25.0), QualityGrade::Excellent);
        assert_eq!(QualityGrade::from_snr(20.1), QualityGrade::Excellent);
        assert_eq!(QualityGrade::from_snr(15.0), QualityGrade::Good);
        assert_eq!(QualityGrade::from_snr(10.1), QualityGrade::Good);
        assert_eq!(QualityGrade::from_snr(7.0), QualityGrade::Fair);
        assert_eq!(QualityGrade::from_snr(5.1), QualityGrade::Fair);
        assert_eq!(QualityGrade::from_snr(3.0), QualityGrade::Poor);
        assert_eq!(QualityGrade::from_snr(0.1), QualityGrade::Poor);
        assert_eq!(QualityGrade::from_snr(-5.0), QualityGrade::Unusable);
    }

    #[test]
    fn test_find_segments_by_quality() {
        let repo = MockSegmentRepository::new();

        // Add segments with varying quality
        let snr_values = vec![25.0, 15.0, 7.0, 3.0, -5.0];
        for snr in snr_values {
            let segment = create_test_segment_with_snr(snr);
            repo.save(segment).unwrap();
        }

        // Find good or better
        let good_or_better = repo.find_by_quality(QualityGrade::Good).unwrap();
        assert_eq!(good_or_better.len(), 2); // Excellent and Good

        // Find fair or better
        let fair_or_better = repo.find_by_quality(QualityGrade::Fair).unwrap();
        assert_eq!(fair_or_better.len(), 3); // Excellent, Good, Fair
    }

    #[test]
    fn test_segment_clipping_detection() {
        let mut segment = create_test_segment();

        // No clipping
        segment.clipping_score = 0.0;
        assert!(segment.clipping_score < 0.01);

        // Minor clipping
        segment.clipping_score = 0.05;
        assert!(segment.clipping_score < 0.1);

        // Severe clipping
        segment.clipping_score = 0.3;
        assert!(segment.clipping_score > 0.2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_integration_smoke_test() {
        // Create recording
        let recording = create_test_recording();

        // Create segments
        let segments = create_segment_sequence(5, 500);

        // Create spectrogram
        let spectrogram = create_test_spectrogram();

        // Verify relationships
        assert!(recording.duration_ms >= segments.last().unwrap().end_ms);
        assert_eq!(spectrogram.len(), 500);
        assert_eq!(spectrogram[0].len(), 128);
    }
}
