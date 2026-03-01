//! Stage IV Encoder: Emotional/AOL Data via SNN Temporal Encoding
//!
//! CRV Stage IV captures emotional impacts, tangibles, intangibles, and
//! analytical overlay (AOL) detections. The spiking neural network (SNN)
//! temporal encoding naturally models the signal-vs-noise discrimination
//! that Stage IV demands:
//!
//! - High-frequency spike bursts correlate with AOL contamination
//! - Sustained low-frequency patterns indicate clean signal line data
//! - The refractory period prevents AOL cascade (analytical runaway)
//!
//! # Architecture
//!
//! Emotional intensity timeseries → SNN input currents.
//! Network spike rate analysis detects AOL events.
//! The embedding captures both the clean signal and AOL separation.

use crate::error::CrvResult;
use crate::types::{AOLDetection, CrvConfig, StageIVData};
use ruvector_mincut::snn::{LayerConfig, NetworkConfig, NeuronConfig, SpikingNetwork};

/// Stage IV encoder using spiking neural network temporal encoding.
#[derive(Debug)]
pub struct StageIVEncoder {
    /// Embedding dimensionality.
    dim: usize,
    /// AOL detection threshold (spike rate above this = likely AOL).
    aol_threshold: f32,
    /// SNN time step.
    dt: f64,
    /// Refractory period for AOL cascade prevention.
    refractory_period: f64,
}

impl StageIVEncoder {
    /// Create a new Stage IV encoder.
    pub fn new(config: &CrvConfig) -> Self {
        Self {
            dim: config.dimensions,
            aol_threshold: config.aol_threshold,
            dt: config.snn_dt,
            refractory_period: config.refractory_period_ms,
        }
    }

    /// Create a spiking network configured for emotional signal processing.
    ///
    /// The network has 3 layers:
    /// - Input: receives emotional intensity as current
    /// - Hidden: processes temporal patterns
    /// - Output: produces the embedding dimensions
    fn create_network(&self, input_size: usize) -> SpikingNetwork {
        let hidden_size = (input_size * 2).max(16).min(128);
        let output_size = self.dim.min(64); // SNN output, will be expanded

        let neuron_config = NeuronConfig {
            tau_membrane: 20.0,
            v_rest: 0.0,
            v_reset: 0.0,
            threshold: 1.0,
            t_refrac: self.refractory_period,
            resistance: 1.0,
            threshold_adapt: 0.1,
            tau_threshold: 100.0,
            homeostatic: true,
            target_rate: 0.01,
            tau_homeostatic: 1000.0,
        };

        let config = NetworkConfig {
            layers: vec![
                LayerConfig::new(input_size).with_neuron_config(neuron_config.clone()),
                LayerConfig::new(hidden_size)
                    .with_neuron_config(neuron_config.clone())
                    .with_recurrence(),
                LayerConfig::new(output_size).with_neuron_config(neuron_config),
            ],
            stdp_config: Default::default(),
            dt: self.dt,
            winner_take_all: false,
            wta_strength: 0.0,
        };

        SpikingNetwork::new(config)
    }

    /// Encode emotional intensity values into SNN input currents.
    fn emotional_to_currents(intensities: &[(String, f32)]) -> Vec<f64> {
        intensities
            .iter()
            .map(|(_, intensity)| *intensity as f64 * 5.0) // Scale to reasonable current
            .collect()
    }

    /// Analyze spike output to detect AOL events.
    ///
    /// High spike rate in a short window indicates the analytical mind
    /// is overriding the signal line (AOL contamination).
    fn detect_aol(&self, spike_rates: &[f64], window_ms: f64) -> Vec<AOLDetection> {
        let mut detections = Vec::new();
        let threshold = self.aol_threshold as f64;

        for (i, &rate) in spike_rates.iter().enumerate() {
            if rate > threshold {
                detections.push(AOLDetection {
                    content: format!("AOL burst at timestep {}", i),
                    timestamp_ms: (i as f64 * window_ms) as u64,
                    flagged: rate > threshold * 1.5, // Auto-flag strong AOL
                    anomaly_score: (rate / threshold).min(1.0) as f32,
                });
            }
        }

        detections
    }

    /// Encode Stage IV data into a temporal embedding.
    ///
    /// Runs the SNN on emotional intensity data, analyzes spike patterns
    /// for AOL contamination, and produces a combined embedding that
    /// captures both clean signal and AOL separation.
    pub fn encode(&self, data: &StageIVData) -> CrvResult<Vec<f32>> {
        // Build input from emotional impact data
        let input_size = data.emotional_impact.len().max(1);
        let currents = Self::emotional_to_currents(&data.emotional_impact);

        if currents.is_empty() {
            // Fall back to text-based encoding if no emotional intensity data
            return self.encode_from_text(data);
        }

        // Run SNN simulation
        let mut network = self.create_network(input_size);
        let num_steps = 100; // 100ms simulation
        let mut spike_counts = vec![0usize; network.layer_size(network.num_layers() - 1)];
        let mut step_rates = Vec::new();

        for step in 0..num_steps {
            // Inject currents (modulated by step for temporal variation)
            let modulated: Vec<f64> = currents
                .iter()
                .map(|&c| c * (1.0 + 0.3 * ((step as f64 * 0.1).sin())))
                .collect();
            network.inject_current(&modulated);

            let spikes = network.step();
            for spike in &spikes {
                if spike.neuron_id < spike_counts.len() {
                    spike_counts[spike.neuron_id] += 1;
                }
            }

            // Track rate per window
            if step % 10 == 9 {
                let rate = spikes.len() as f64 / 10.0;
                step_rates.push(rate);
            }
        }

        // Build embedding from spike counts and output activities
        let output = network.get_output();
        let mut embedding = vec![0.0f32; self.dim];

        // First portion: spike count features
        let spike_dims = spike_counts.len().min(self.dim / 3);
        let max_count = *spike_counts.iter().max().unwrap_or(&1) as f32;
        for (i, &count) in spike_counts.iter().take(spike_dims).enumerate() {
            embedding[i] = count as f32 / max_count.max(1.0);
        }

        // Second portion: membrane potential output
        let pot_offset = self.dim / 3;
        let pot_dims = output.len().min(self.dim / 3);
        for (i, &v) in output.iter().take(pot_dims).enumerate() {
            if pot_offset + i < self.dim {
                embedding[pot_offset + i] = v as f32;
            }
        }

        // Third portion: text-derived features from tangibles/intangibles
        let text_offset = 2 * self.dim / 3;
        self.encode_text_features(data, &mut embedding[text_offset..]);

        // Encode AOL information
        let aol_detections = self.detect_aol(&step_rates, 10.0);
        let aol_count = (aol_detections.len() + data.aol_detections.len()) as f32;
        if self.dim > 2 {
            // Store AOL contamination level in last dimension
            embedding[self.dim - 1] = (aol_count / num_steps as f32).min(1.0);
        }

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for f in &mut embedding {
                *f /= norm;
            }
        }

        Ok(embedding)
    }

    /// Text-based encoding fallback when no intensity timeseries is available.
    fn encode_from_text(&self, data: &StageIVData) -> CrvResult<Vec<f32>> {
        let mut embedding = vec![0.0f32; self.dim];
        self.encode_text_features(data, &mut embedding);

        // L2 normalize
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-6 {
            for f in &mut embedding {
                *f /= norm;
            }
        }

        Ok(embedding)
    }

    /// Encode text descriptors (tangibles, intangibles) into feature slots.
    fn encode_text_features(&self, data: &StageIVData, features: &mut [f32]) {
        if features.is_empty() {
            return;
        }

        // Hash tangibles
        for (i, tangible) in data.tangibles.iter().enumerate() {
            for (j, byte) in tangible.bytes().enumerate() {
                let idx = (i * 7 + j) % features.len();
                features[idx] += (byte as f32 / 255.0) * 0.3;
            }
        }

        // Hash intangibles
        for (i, intangible) in data.intangibles.iter().enumerate() {
            for (j, byte) in intangible.bytes().enumerate() {
                let idx = (i * 11 + j + features.len() / 2) % features.len();
                features[idx] += (byte as f32 / 255.0) * 0.3;
            }
        }
    }

    /// Get the AOL anomaly score for a given Stage IV embedding.
    ///
    /// Higher values indicate more AOL contamination.
    pub fn aol_score(&self, embedding: &[f32]) -> f32 {
        if embedding.len() >= self.dim && self.dim > 2 {
            embedding[self.dim - 1].abs()
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> CrvConfig {
        CrvConfig {
            dimensions: 32,
            aol_threshold: 0.7,
            refractory_period_ms: 50.0,
            snn_dt: 1.0,
            ..CrvConfig::default()
        }
    }

    #[test]
    fn test_encoder_creation() {
        let config = test_config();
        let encoder = StageIVEncoder::new(&config);
        assert_eq!(encoder.dim, 32);
        assert_eq!(encoder.aol_threshold, 0.7);
    }

    #[test]
    fn test_text_only_encode() {
        let config = test_config();
        let encoder = StageIVEncoder::new(&config);

        let data = StageIVData {
            emotional_impact: vec![],
            tangibles: vec!["metal".to_string(), "concrete".to_string()],
            intangibles: vec!["historical significance".to_string()],
            aol_detections: vec![],
        };

        let embedding = encoder.encode(&data).unwrap();
        assert_eq!(embedding.len(), 32);
    }

    #[test]
    fn test_full_encode_with_snn() {
        let config = test_config();
        let encoder = StageIVEncoder::new(&config);

        let data = StageIVData {
            emotional_impact: vec![
                ("awe".to_string(), 0.8),
                ("unease".to_string(), 0.3),
                ("curiosity".to_string(), 0.6),
            ],
            tangibles: vec!["stone wall".to_string()],
            intangibles: vec!["ancient purpose".to_string()],
            aol_detections: vec![AOLDetection {
                content: "looks like a castle".to_string(),
                timestamp_ms: 500,
                flagged: true,
                anomaly_score: 0.8,
            }],
        };

        let embedding = encoder.encode(&data).unwrap();
        assert_eq!(embedding.len(), 32);

        // Should be normalized
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.1 || norm < 0.01); // normalized or near-zero
    }

    #[test]
    fn test_aol_detection() {
        let config = test_config();
        let encoder = StageIVEncoder::new(&config);

        let rates = vec![0.1, 0.2, 0.9, 0.95, 0.3, 0.1];
        let detections = encoder.detect_aol(&rates, 10.0);

        // Should detect the high-rate windows as AOL
        assert!(detections.len() >= 2);
        for d in &detections {
            assert!(d.anomaly_score > 0.0);
        }
    }
}
