//! Per-node sensing state definition and core lifecycle methods.
//!
//! Complies with ELMAS.md: <= 350 lines, guard clauses, domain cohesion.

use std::collections::VecDeque;
use std::time::Instant;

use crate::domain::csi_grid::RawGridObservation;
use crate::domain::fps::{update_csi_fps_ema, MAX_PHYSICAL_CSI_FPS};
use crate::domain::types::FeatureInfo;
use crate::ingest::Esp32VitalsPacket;
use crate::vital_signs::{VitalSignDetector, VitalSigns};

use super::constants::*;

/// Per-node sensing state for multi-node deployments (issue #249).
///
/// Each ESP32 node maintains its own independent frame history, smoothing buffers,
/// and vital sign detector so that distinct spatial channels are never mixed.
pub struct NodeState {
    pub frame_history: VecDeque<Vec<f64>>,
    pub smoothed_person_score: f64,
    pub prev_person_count: usize,
    pub smoothed_motion: f64,
    pub current_motion_level: String,
    pub latest_classification_confidence: Option<f64>,
    pub debounce_counter: u32,
    pub debounce_candidate: String,
    pub baseline_motion: f64,
    pub baseline_frames: u64,
    pub smoothed_hr: f64,
    pub smoothed_br: f64,
    pub smoothed_hr_conf: f64,
    pub smoothed_br_conf: f64,
    pub hr_buffer: VecDeque<f64>,
    pub br_buffer: VecDeque<f64>,
    pub rssi_history: VecDeque<f64>,
    pub vital_detector: VitalSignDetector,
    pub latest_vitals: VitalSigns,
    pub last_frame_time: Option<Instant>,
    pub edge_vitals: Option<Esp32VitalsPacket>,
    pub latest_sync: Option<wifi_densepose_hardware::SyncPacket>,
    pub latest_sync_at: Option<Instant>,
    pub latest_accepted_csi_at: Option<Instant>,
    pub latest_csi_sequence: Option<u32>,
    pub latest_csi_sync_valid: bool,
    pub csi_fps_ema: f64,
    pub csi_fps_samples: u32,
    pub latest_features: Option<FeatureInfo>,
    pub prev_keypoints: Option<Vec<[f64; 3]>>,
    pub motion_energy_history: VecDeque<f64>,
    pub coherence_score: f64,
    pub feature_history: Option<wifi_densepose_signal::ruvsense::longitudinal::EmbeddingHistory>,
    pub last_novelty_score: Option<f32>,
    pub active_grid: Option<(u16, wifi_densepose_hardware::PpduType)>,
    pub grid_votes: VecDeque<(u16, wifi_densepose_hardware::PpduType)>,
    pub raw_grid_observations: VecDeque<RawGridObservation>,
    pub field_model_history: VecDeque<Vec<f64>>,
    pub field_model_latest_sequence: Option<u32>,
    pub field_model_latest_seen: Option<Instant>,
}

impl Default for NodeState {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeState {
    /// Create a freshly-initialized `NodeState`.
    pub fn new() -> Self {
        Self {
            frame_history: VecDeque::new(),
            smoothed_person_score: 0.0,
            prev_person_count: 0,
            smoothed_motion: 0.0,
            current_motion_level: "absent".to_string(),
            latest_classification_confidence: None,
            debounce_counter: 0,
            debounce_candidate: "absent".to_string(),
            baseline_motion: 0.0,
            baseline_frames: 0,
            smoothed_hr: 0.0,
            smoothed_br: 0.0,
            smoothed_hr_conf: 0.0,
            smoothed_br_conf: 0.0,
            hr_buffer: VecDeque::with_capacity(8),
            br_buffer: VecDeque::with_capacity(8),
            rssi_history: VecDeque::new(),
            vital_detector: VitalSignDetector::new(20.0),
            latest_vitals: VitalSigns::default(),
            last_frame_time: None,
            edge_vitals: None,
            latest_sync: None,
            latest_sync_at: None,
            latest_accepted_csi_at: None,
            latest_csi_sequence: None,
            latest_csi_sync_valid: false,
            csi_fps_ema: 20.0,
            csi_fps_samples: 0,
            latest_features: None,
            prev_keypoints: None,
            motion_energy_history: VecDeque::with_capacity(COHERENCE_WINDOW),
            coherence_score: 1.0,
            feature_history: Some(
                wifi_densepose_signal::ruvsense::longitudinal::EmbeddingHistory::with_sketch(
                    NOVELTY_VECTOR_DIM,
                    NOVELTY_HISTORY_CAPACITY,
                    NOVELTY_SKETCH_VERSION,
                ),
            ),
            last_novelty_score: None,
            active_grid: None,
            grid_votes: VecDeque::with_capacity(GRID_VOTE_FRAMES),
            raw_grid_observations: VecDeque::with_capacity(CALIBRATION_GRID_EVIDENCE_CAPACITY),
            field_model_history: VecDeque::with_capacity(FRAME_HISTORY_CAPACITY),
            field_model_latest_sequence: None,
            field_model_latest_seen: None,
        }
    }

    /// Measured sample rate only after sufficient inter-frame deltas warmed the EMA.
    pub fn measured_sample_rate_hz(&self) -> Option<f64> {
        (self.csi_fps_samples >= 5 && self.csi_fps_ema.is_finite())
            .then(|| self.csi_fps_ema.clamp(1.0, MAX_PHYSICAL_CSI_FPS))
    }

    /// Effective sample rate falling back to 20.0 Hz prior to warm-up.
    pub fn effective_sample_rate_hz(&self) -> f64 {
        self.measured_sample_rate_hz().unwrap_or(20.0)
    }

    /// Record a CSI data-frame arrival and return whether it was the node's first sensing frame.
    pub fn observe_csi_frame_arrival(&mut self, now: Instant) -> bool {
        let first_sensing_frame = self.last_frame_time.is_none();
        if let Some(prev) = self.last_frame_time {
            let dt = now.duration_since(prev).as_secs_f64();
            if let Some(new_ema) = update_csi_fps_ema(self.csi_fps_ema, dt) {
                self.csi_fps_ema = new_ema;
                self.csi_fps_samples = self.csi_fps_samples.saturating_add(1);
            }
        }
        self.last_frame_time = Some(now);
        first_sensing_frame
    }

    /// Record an accepted CSI sample and preserve wire sequence metadata.
    pub fn observe_accepted_csi_frame(
        &mut self,
        sequence: u32,
        sync_valid: bool,
        now: Instant,
    ) -> bool {
        self.latest_accepted_csi_at = Some(now);
        self.latest_csi_sequence = Some(sequence);
        self.latest_csi_sync_valid = sync_valid;
        self.observe_csi_frame_arrival(now)
    }

    /// Update the coherence score from the latest motion_energy value.
    pub fn update_coherence(&mut self, motion_energy: f64) {
        if self.motion_energy_history.len() >= COHERENCE_WINDOW {
            self.motion_energy_history.pop_front();
        }
        self.motion_energy_history.push_back(motion_energy);

        let n = self.motion_energy_history.len();
        if n < 2 {
            self.coherence_score = 1.0;
            return;
        }

        let mean: f64 = self.motion_energy_history.iter().sum::<f64>() / n as f64;
        let variance: f64 = self
            .motion_energy_history
            .iter()
            .map(|v| (v - mean) * (v - mean))
            .sum::<f64>()
            / (n - 1) as f64;

        self.coherence_score = (1.0 / (1.0 + variance)).clamp(0.0, 1.0);
    }

    /// Select the EMA alpha based on the current coherence score.
    pub fn ema_alpha(&self) -> f64 {
        if self.coherence_score < COHERENCE_LOW_THRESHOLD {
            TEMPORAL_EMA_ALPHA_LOW_COHERENCE
        } else {
            TEMPORAL_EMA_ALPHA_DEFAULT
        }
    }

    /// ADR-084 novelty step against the per-node longitudinal sketch bank.
    pub fn update_novelty(&mut self, amplitudes: &[f64]) {
        let history = match &mut self.feature_history {
            Some(h) => h,
            None => return,
        };
        let mut feature: Vec<f32> = amplitudes
            .iter()
            .take(NOVELTY_VECTOR_DIM)
            .map(|&v| v as f32)
            .collect();
        feature.resize(NOVELTY_VECTOR_DIM, 0.0);

        self.last_novelty_score = history.novelty(&feature);

        let _ = history.push(
            wifi_densepose_signal::ruvsense::longitudinal::EmbeddingEntry {
                person_id: 0,
                day_us: 0,
                embedding: feature,
            },
        );
    }
}

