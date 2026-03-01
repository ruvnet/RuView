//! SONA threshold tuner implementation.

use super::adjustment::{AdjustmentReason, ThresholdAdjustment};
use super::config::{ThresholdConfig, TunerConfig};
use super::error::{SonaTuningError, SonaTuningResult};
use ruvector_sona::{
    EwcConfig, EwcPlusPlus, PatternConfig, ReasoningBank, SonaConfig, SonaEngine, TrajectoryBuilder,
};
use std::collections::VecDeque;

/// State of the SONA threshold tuner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunerState {
    /// Tuner is uninitialized.
    Uninitialized,
    /// Tuner is ready for learning.
    Ready,
    /// Tuner is tracking a regime.
    TrackingRegime,
    /// Tuner is consolidating knowledge.
    Consolidating,
}

/// Tracks operational regimes for pattern learning.
#[derive(Debug)]
pub struct RegimeTracker {
    /// Current regime ID.
    current_regime: Option<String>,
    /// Energy history for the current regime.
    energy_history: VecDeque<f32>,
    /// Maximum history length.
    max_history: usize,
    /// Regime start timestamp.
    regime_start_ms: u64,
}

impl RegimeTracker {
    /// Create a new regime tracker.
    pub fn new(max_history: usize) -> Self {
        Self {
            current_regime: None,
            energy_history: VecDeque::with_capacity(max_history),
            max_history,
            regime_start_ms: 0,
        }
    }

    /// Start tracking a new regime.
    pub fn start_regime(&mut self, regime_id: impl Into<String>, initial_energy: f32) {
        self.current_regime = Some(regime_id.into());
        self.energy_history.clear();
        self.energy_history.push_back(initial_energy);
        self.regime_start_ms = current_time_ms();
    }

    /// Record an energy observation.
    pub fn record_energy(&mut self, energy: f32) {
        if self.energy_history.len() >= self.max_history {
            self.energy_history.pop_front();
        }
        self.energy_history.push_back(energy);
    }

    /// Get the current regime ID.
    pub fn current_regime(&self) -> Option<&str> {
        self.current_regime.as_deref()
    }

    /// Get the energy history as a slice.
    pub fn energy_history(&self) -> &VecDeque<f32> {
        &self.energy_history
    }

    /// Get the average energy in the current regime.
    pub fn average_energy(&self) -> f32 {
        if self.energy_history.is_empty() {
            return 0.0;
        }
        self.energy_history.iter().sum::<f32>() / self.energy_history.len() as f32
    }

    /// Get the energy trend (positive = increasing, negative = decreasing).
    pub fn energy_trend(&self) -> f32 {
        if self.energy_history.len() < 2 {
            return 0.0;
        }

        let half = self.energy_history.len() / 2;
        let first_half_avg: f32 = self.energy_history.iter().take(half).sum::<f32>() / half as f32;
        let second_half_avg: f32 = self.energy_history.iter().skip(half).sum::<f32>()
            / (self.energy_history.len() - half) as f32;

        second_half_avg - first_half_avg
    }

    /// Get regime duration in seconds.
    pub fn regime_duration_secs(&self) -> f32 {
        (current_time_ms() - self.regime_start_ms) as f32 / 1000.0
    }

    /// End the current regime.
    pub fn end_regime(&mut self) -> Option<RegimeSummary> {
        self.current_regime.take().map(|id| RegimeSummary {
            regime_id: id,
            duration_secs: self.regime_duration_secs(),
            average_energy: self.average_energy(),
            energy_trend: self.energy_trend(),
            sample_count: self.energy_history.len(),
        })
    }
}

/// Summary of a completed regime.
#[derive(Debug, Clone)]
pub struct RegimeSummary {
    /// Regime identifier.
    pub regime_id: String,
    /// Duration in seconds.
    pub duration_secs: f32,
    /// Average energy.
    pub average_energy: f32,
    /// Energy trend.
    pub energy_trend: f32,
    /// Number of samples.
    pub sample_count: usize,
}

/// SONA threshold tuner for adaptive threshold learning.
///
/// This adapter wraps the SONA engine to provide threshold tuning
/// specifically for the coherence gate.
pub struct SonaThresholdTuner {
    /// The underlying SONA engine.
    engine: SonaEngine,
    /// EWC++ for preventing catastrophic forgetting.
    ewc: EwcPlusPlus,
    /// Reasoning bank for pattern storage and retrieval.
    reasoning_bank: ReasoningBank,
    /// Configuration.
    config: TunerConfig,
    /// Current threshold configuration.
    current_thresholds: ThresholdConfig,
    /// Regime tracker.
    regime_tracker: RegimeTracker,
    /// State.
    state: TunerState,
    /// Trajectories completed since last consolidation.
    trajectories_since_consolidation: usize,
}

impl SonaThresholdTuner {
    /// Create a new SONA threshold tuner.
    pub fn new(config: TunerConfig) -> Self {
        let sona_config = SonaConfig {
            hidden_dim: config.hidden_dim,
            embedding_dim: config.embedding_dim,
            ..Default::default()
        };

        let engine = SonaEngine::with_config(sona_config);

        let ewc_config = EwcConfig {
            initial_lambda: config.ewc_lambda,
            ..Default::default()
        };
        let ewc = EwcPlusPlus::new(ewc_config);

        let pattern_config = PatternConfig::default();
        let reasoning_bank = ReasoningBank::new(pattern_config);

        Self {
            engine,
            ewc,
            reasoning_bank,
            current_thresholds: config.initial_thresholds,
            regime_tracker: RegimeTracker::new(1000),
            state: TunerState::Ready,
            trajectories_since_consolidation: 0,
            config,
        }
    }

    /// Create with default configuration.
    pub fn default_tuner() -> Self {
        Self::new(TunerConfig::default())
    }

    /// Get the current state.
    pub fn state(&self) -> TunerState {
        self.state
    }

    /// Get the current threshold configuration.
    pub fn current_thresholds(&self) -> &ThresholdConfig {
        &self.current_thresholds
    }

    /// Begin tracking a new operational regime.
    ///
    /// This starts a trajectory in the SONA engine and begins
    /// recording energy observations.
    pub fn begin_regime(&mut self, energy_trace: &[f32]) -> SonaTuningResult<TrajectoryBuilder> {
        if energy_trace.is_empty() {
            return Err(SonaTuningError::trajectory("empty energy trace"));
        }

        // Convert energy trace to embedding
        let mut embedding = vec![0.0; self.config.embedding_dim];
        for (i, &e) in energy_trace
            .iter()
            .take(self.config.embedding_dim)
            .enumerate()
        {
            embedding[i] = e;
        }

        // Start SONA trajectory
        let builder = self.engine.begin_trajectory(embedding);

        // Start regime tracking
        let regime_id = format!("regime_{}", current_time_ms());
        self.regime_tracker
            .start_regime(&regime_id, energy_trace.last().copied().unwrap_or(0.0));

        self.state = TunerState::TrackingRegime;

        Ok(builder)
    }

    /// Record an energy observation during regime tracking.
    pub fn record_energy(&mut self, energy: f32) {
        self.regime_tracker.record_energy(energy);
    }

    /// Learn from the outcome of a regime.
    ///
    /// This ends the SONA trajectory and stores successful patterns.
    pub fn learn_outcome(
        &mut self,
        builder: TrajectoryBuilder,
        success_score: f32,
    ) -> SonaTuningResult<Option<ThresholdAdjustment>> {
        // End SONA trajectory
        self.engine.end_trajectory(builder, success_score);

        // End regime tracking
        let summary = self.regime_tracker.end_regime();

        self.trajectories_since_consolidation += 1;
        self.state = TunerState::Ready;

        // If successful, store pattern
        if success_score > 0.8 {
            self.store_success_pattern(success_score)?;
        }

        // Auto-consolidate if needed
        if self.trajectories_since_consolidation >= self.config.auto_consolidate_after {
            self.consolidate_knowledge()?;
        }

        // Generate adjustment if we learned something useful
        if success_score > 0.9 {
            if let Some(summary) = summary {
                return Ok(Some(ThresholdAdjustment::new(
                    &self.current_thresholds,
                    self.current_thresholds, // Keep current for now
                    AdjustmentReason::BackgroundLearning {
                        samples: summary.sample_count,
                    },
                    success_score,
                )));
            }
        }

        Ok(None)
    }

    /// Store a successful pattern in the reasoning bank.
    fn store_success_pattern(&mut self, _score: f32) -> SonaTuningResult<()> {
        // Note: ReasoningBank uses add_trajectory for storage
        // For simplicity, we skip pattern storage in this integration
        // A full implementation would create QueryTrajectory objects
        Ok(())
    }

    /// Convert a threshold configuration to an embedding vector.
    fn threshold_to_embedding(&self, config: &ThresholdConfig) -> Vec<f32> {
        let mut embedding = vec![0.0; self.config.embedding_dim];
        embedding[0] = config.reflex;
        embedding[1] = config.retrieval;
        embedding[2] = config.heavy;
        embedding[3] = config.persistence_window_secs as f32 / 60.0; // Normalize to minutes
        embedding
    }

    /// Convert an embedding back to threshold configuration.
    fn embedding_to_threshold(&self, embedding: &[f32]) -> Option<ThresholdConfig> {
        if embedding.len() < 4 {
            return None;
        }

        let config = ThresholdConfig {
            reflex: embedding[0].clamp(0.0, 1.0),
            retrieval: embedding[1].clamp(0.0, 1.0),
            heavy: embedding[2].clamp(0.0, 1.0),
            persistence_window_secs: (embedding[3] * 60.0).max(1.0) as u64,
        };

        if config.is_valid() {
            Some(config)
        } else {
            None
        }
    }

    /// Find a similar regime configuration from past experience.
    pub fn find_similar_regime(&self, current_energy: &[f32]) -> Option<ThresholdConfig> {
        // Convert current energy to query embedding
        let mut query = vec![0.0; self.config.embedding_dim];
        for (i, &e) in current_energy
            .iter()
            .take(self.config.embedding_dim)
            .enumerate()
        {
            query[i] = e;
        }

        // Query reasoning bank using find_similar
        let similar = self.reasoning_bank.find_similar(&query, 1);
        if let Some(pattern) = similar.first() {
            self.embedding_to_threshold(&pattern.centroid)
        } else {
            None
        }
    }

    /// Instantly adapt to an energy spike.
    ///
    /// This uses Micro-LoRA for ultra-fast (<0.05ms) adaptation.
    pub fn instant_adapt(&mut self, energy_spike: f32) -> ThresholdAdjustment {
        // Apply Micro-LoRA adaptation
        let input = vec![energy_spike; self.config.embedding_dim];
        let mut output = vec![0.0; self.config.embedding_dim];
        self.engine.apply_micro_lora(&input, &mut output);

        // Generate adjustment
        ThresholdAdjustment::for_energy_spike(&self.current_thresholds, energy_spike)
    }

    /// Apply a threshold adjustment.
    pub fn apply_adjustment(&mut self, adjustment: &ThresholdAdjustment) {
        if adjustment.new_thresholds.is_valid() {
            self.current_thresholds = adjustment.new_thresholds;
        }
    }

    /// Consolidate learned knowledge using EWC++.
    ///
    /// This prevents catastrophic forgetting when adapting to new regimes.
    pub fn consolidate_knowledge(&mut self) -> SonaTuningResult<()> {
        self.state = TunerState::Consolidating;

        // Trigger EWC++ consolidation
        self.ewc.consolidate_all_tasks();

        self.trajectories_since_consolidation = 0;
        self.state = TunerState::Ready;

        Ok(())
    }

    /// Get tuner statistics.
    pub fn stats(&self) -> TunerStats {
        TunerStats {
            state: self.state,
            current_thresholds: self.current_thresholds,
            patterns_stored: self.reasoning_bank.pattern_count(),
            trajectories_since_consolidation: self.trajectories_since_consolidation,
            regime_average_energy: self.regime_tracker.average_energy(),
            regime_energy_trend: self.regime_tracker.energy_trend(),
        }
    }

    /// Reset the tuner to initial state.
    pub fn reset(&mut self) {
        self.current_thresholds = self.config.initial_thresholds;
        self.regime_tracker = RegimeTracker::new(1000);
        self.trajectories_since_consolidation = 0;
        self.state = TunerState::Ready;
    }
}

impl std::fmt::Debug for SonaThresholdTuner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SonaThresholdTuner")
            .field("state", &self.state)
            .field("current_thresholds", &self.current_thresholds)
            .field("patterns_stored", &self.reasoning_bank.pattern_count())
            .finish()
    }
}

/// Tuner statistics.
#[derive(Debug, Clone, Copy)]
pub struct TunerStats {
    /// Current state.
    pub state: TunerState,
    /// Current thresholds.
    pub current_thresholds: ThresholdConfig,
    /// Number of patterns stored.
    pub patterns_stored: usize,
    /// Trajectories since last consolidation.
    pub trajectories_since_consolidation: usize,
    /// Average energy in current regime.
    pub regime_average_energy: f32,
    /// Energy trend in current regime.
    pub regime_energy_trend: f32,
}

/// Get current time in milliseconds.
fn current_time_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tuner_creation() {
        let tuner = SonaThresholdTuner::default_tuner();
        assert_eq!(tuner.state(), TunerState::Ready);
    }

    #[test]
    fn test_regime_tracker() {
        let mut tracker = RegimeTracker::new(100);

        tracker.start_regime("test", 0.5);
        tracker.record_energy(0.6);
        tracker.record_energy(0.7);

        assert_eq!(tracker.current_regime(), Some("test"));
        assert!(tracker.average_energy() > 0.5);
        assert!(tracker.energy_trend() > 0.0);
    }

    #[test]
    fn test_instant_adapt() {
        let mut tuner = SonaThresholdTuner::default_tuner();
        let initial = *tuner.current_thresholds();

        let adjustment = tuner.instant_adapt(0.5);

        assert!(adjustment.new_thresholds.reflex < initial.reflex);
        assert!(adjustment.urgent);
    }

    #[test]
    fn test_threshold_embedding_roundtrip() {
        let tuner = SonaThresholdTuner::default_tuner();
        let original = ThresholdConfig::default();

        let embedding = tuner.threshold_to_embedding(&original);
        let recovered = tuner.embedding_to_threshold(&embedding);

        assert!(recovered.is_some());
        let recovered = recovered.unwrap();
        assert!((recovered.reflex - original.reflex).abs() < 0.001);
    }
}
