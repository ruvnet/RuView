//! Collection parameter versioning with EWC
//!
//! Implements version management for RuVector collections using
//! Elastic Weight Consolidation (EWC) to prevent catastrophic forgetting.

use crate::plasticity::consolidate::EWC;
use crate::{NervousSystemError, Result};
use std::collections::HashMap;

/// Eligibility state for BTSP-style parameter tracking
#[derive(Debug, Clone)]
pub struct EligibilityState {
    /// Eligibility trace value
    pub trace: f32,

    /// Last update timestamp (milliseconds)
    pub last_update: u64,

    /// Time constant for decay (milliseconds)
    pub tau: f32,
}

impl EligibilityState {
    /// Create new eligibility state
    pub fn new(tau: f32) -> Self {
        Self {
            trace: 0.0,
            last_update: 0,
            tau,
        }
    }

    /// Update eligibility trace
    pub fn update(&mut self, value: f32, timestamp: u64) {
        // Decay based on time elapsed
        if self.last_update > 0 {
            let dt = (timestamp - self.last_update) as f32;
            self.trace *= (-dt / self.tau).exp();
        }

        // Add new value
        self.trace += value;
        self.last_update = timestamp;
    }

    /// Get current trace value
    pub fn trace(&self) -> f32 {
        self.trace
    }
}

/// Consolidation schedule for periodic memory replay
#[derive(Debug, Clone)]
pub struct ConsolidationSchedule {
    /// Replay interval in seconds
    pub replay_interval_secs: u64,

    /// Batch size for consolidation
    pub batch_size: usize,

    /// Learning rate for consolidation
    pub learning_rate: f32,

    /// Last consolidation timestamp
    pub last_consolidation: u64,
}

impl Default for ConsolidationSchedule {
    fn default() -> Self {
        Self {
            replay_interval_secs: 3600, // 1 hour
            batch_size: 32,
            learning_rate: 0.01,
            last_consolidation: 0,
        }
    }
}

impl ConsolidationSchedule {
    /// Create new schedule
    pub fn new(interval_secs: u64, batch_size: usize, learning_rate: f32) -> Self {
        Self {
            replay_interval_secs: interval_secs,
            batch_size,
            learning_rate,
            last_consolidation: 0,
        }
    }

    /// Check if consolidation should run
    pub fn should_consolidate(&self, current_time: u64) -> bool {
        if self.last_consolidation == 0 {
            return false; // Never consolidated yet
        }

        current_time - self.last_consolidation >= self.replay_interval_secs
    }
}

/// Parameter version for a collection
///
/// Tracks parameter versions with eligibility traces and Fisher information
/// for EWC-based continual learning.
#[derive(Debug, Clone)]
pub struct ParameterVersion {
    /// Collection ID
    pub collection_id: u64,

    /// Version number
    pub version: u32,

    /// Eligibility windows for parameters (param_id -> state)
    pub eligibility_windows: HashMap<u64, EligibilityState>,

    /// Fisher information diagonal (if computed)
    pub fisher_diagonal: Option<Vec<f32>>,

    /// Creation timestamp
    pub created_at: u64,

    /// Default tau for eligibility traces (milliseconds)
    tau: f32,
}

impl ParameterVersion {
    /// Create new parameter version
    pub fn new(collection_id: u64, version: u32, created_at: u64) -> Self {
        Self {
            collection_id,
            version,
            eligibility_windows: HashMap::new(),
            fisher_diagonal: None,
            created_at,
            tau: 2000.0, // 2 second default
        }
    }

    /// Set tau for eligibility traces
    pub fn with_tau(mut self, tau: f32) -> Self {
        self.tau = tau;
        self
    }

    /// Update eligibility for a parameter
    pub fn update_eligibility(&mut self, param_id: u64, value: f32, timestamp: u64) {
        self.eligibility_windows
            .entry(param_id)
            .or_insert_with(|| EligibilityState::new(self.tau))
            .update(value, timestamp);
    }

    /// Get eligibility trace for parameter
    pub fn get_eligibility(&self, param_id: u64) -> f32 {
        self.eligibility_windows
            .get(&param_id)
            .map(|state| state.trace())
            .unwrap_or(0.0)
    }

    /// Set Fisher information diagonal
    pub fn set_fisher(&mut self, fisher: Vec<f32>) {
        self.fisher_diagonal = Some(fisher);
    }

    /// Check if Fisher information is computed
    pub fn has_fisher(&self) -> bool {
        self.fisher_diagonal.is_some()
    }
}

/// Collection versioning with EWC
///
/// Manages collection parameter versions with continual learning support
/// via Elastic Weight Consolidation.
///
/// # Example
///
/// ```
/// use ruvector_nervous_system::integration::{CollectionVersioning, ConsolidationSchedule};
///
/// let schedule = ConsolidationSchedule::default();
/// let mut versioning = CollectionVersioning::new(1, schedule);
///
/// // Update parameters
/// let params = vec![0.5; 100];
/// versioning.update_parameters(&params);
///
/// // Bump version when needed
/// versioning.bump_version();
///
/// // Check if consolidation needed
/// let current_time = 7200; // 2 hours
/// if versioning.should_consolidate(current_time) {
///     // Trigger consolidation
///     let gradients: Vec<Vec<f32>> = vec![vec![0.1; 100]; 50];
///     versioning.consolidate(&gradients, current_time);
/// }
/// ```
pub struct CollectionVersioning {
    /// Collection ID
    collection_id: u64,

    /// Current version
    version: u32,

    /// Current parameters
    current_params: Vec<f32>,

    /// Parameter versions (version -> ParameterVersion)
    versions: HashMap<u32, ParameterVersion>,

    /// EWC instance for continual learning
    ewc: EWC,

    /// Consolidation schedule
    consolidation_policy: ConsolidationSchedule,
}

impl CollectionVersioning {
    /// Create new collection versioning
    pub fn new(collection_id: u64, consolidation_policy: ConsolidationSchedule) -> Self {
        Self {
            collection_id,
            version: 0,
            current_params: Vec::new(),
            versions: HashMap::new(),
            ewc: EWC::new(1000.0), // Default lambda
            consolidation_policy,
        }
    }

    /// Create with custom EWC lambda
    pub fn with_lambda(mut self, lambda: f32) -> Self {
        self.ewc = EWC::new(lambda);
        self
    }

    /// Bump to next version
    pub fn bump_version(&mut self) {
        self.version += 1;

        let timestamp = current_timestamp_ms();
        let param_version = ParameterVersion::new(self.collection_id, self.version, timestamp);

        self.versions.insert(self.version, param_version);
    }

    /// Update current parameters
    pub fn update_parameters(&mut self, params: &[f32]) {
        self.current_params = params.to_vec();
    }

    /// Get current parameters
    pub fn current_parameters(&self) -> &[f32] {
        &self.current_params
    }

    /// Apply EWC regularization to gradients
    ///
    /// Returns gradient with EWC penalty added.
    pub fn apply_ewc(&self, base_gradient: &[f32]) -> Vec<f32> {
        if !self.ewc.is_initialized() {
            return base_gradient.to_vec();
        }

        let ewc_grad = self.ewc.ewc_gradient(&self.current_params);

        base_gradient
            .iter()
            .zip(ewc_grad.iter())
            .map(|(base, ewc)| base + ewc)
            .collect()
    }

    /// Check if consolidation should run
    pub fn should_consolidate(&self, current_time: u64) -> bool {
        self.consolidation_policy.should_consolidate(current_time)
    }

    /// Consolidate current version
    ///
    /// Computes Fisher information and updates EWC to protect current parameters.
    pub fn consolidate(&mut self, gradients: &[Vec<f32>], current_time: u64) -> Result<()> {
        // Compute Fisher information for current parameters
        self.ewc.compute_fisher(&self.current_params, gradients)?;

        // Update consolidation timestamp
        self.consolidation_policy.last_consolidation = current_time;

        // Store Fisher in current version
        if let Some(version) = self.versions.get_mut(&self.version) {
            if !self.ewc.fisher_diag.is_empty() {
                version.set_fisher(self.ewc.fisher_diag.clone());
            }
        }

        Ok(())
    }

    /// Get current version number
    pub fn version(&self) -> u32 {
        self.version
    }

    /// Get collection ID
    pub fn collection_id(&self) -> u64 {
        self.collection_id
    }

    /// Get parameter version metadata
    pub fn get_version(&self, version: u32) -> Option<&ParameterVersion> {
        self.versions.get(&version)
    }

    /// Get EWC loss for current parameters
    pub fn ewc_loss(&self) -> f32 {
        self.ewc.ewc_loss(&self.current_params)
    }

    /// Update eligibility for parameter in current version
    pub fn update_eligibility(&mut self, param_id: u64, value: f32) {
        let timestamp = current_timestamp_ms();

        if let Some(version) = self.versions.get_mut(&self.version) {
            version.update_eligibility(param_id, value, timestamp);
        }
    }

    /// Get consolidation schedule
    pub fn consolidation_schedule(&self) -> &ConsolidationSchedule {
        &self.consolidation_policy
    }
}

/// Get current timestamp in milliseconds
fn current_timestamp_ms() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eligibility_state() {
        let mut state = EligibilityState::new(1000.0);

        state.update(1.0, 100); // Start at time 100
        assert_eq!(state.trace(), 1.0);

        // After 1 time constant, should decay to ~0.37
        state.update(0.0, 1100); // 1000ms later
        assert!(
            state.trace() > 0.3 && state.trace() < 0.4,
            "trace: {}",
            state.trace()
        );
    }

    #[test]
    fn test_consolidation_schedule() {
        let mut schedule = ConsolidationSchedule::new(3600, 32, 0.01);

        // Never consolidated yet (last_consolidation == 0)
        assert!(!schedule.should_consolidate(0));

        // Set initial consolidation time
        schedule.last_consolidation = 1; // Mark as having consolidated once
                                         // After 2+ hours, should consolidate
        assert!(schedule.should_consolidate(7201));

        schedule.last_consolidation = 7200;
        // Immediately after, should not consolidate
        assert!(!schedule.should_consolidate(7200));
    }

    #[test]
    fn test_parameter_version() {
        let mut version = ParameterVersion::new(1, 0, 0);

        version.update_eligibility(0, 1.0, 100);
        version.update_eligibility(1, 0.5, 100);

        assert_eq!(version.get_eligibility(0), 1.0);
        assert_eq!(version.get_eligibility(1), 0.5);
        assert_eq!(version.get_eligibility(999), 0.0); // Non-existent

        assert!(!version.has_fisher());
        version.set_fisher(vec![0.1; 10]);
        assert!(version.has_fisher());
    }

    #[test]
    fn test_collection_versioning() {
        let schedule = ConsolidationSchedule::default();
        let mut versioning = CollectionVersioning::new(1, schedule);

        assert_eq!(versioning.version(), 0);

        versioning.bump_version();
        assert_eq!(versioning.version(), 1);

        versioning.bump_version();
        assert_eq!(versioning.version(), 2);
    }

    #[test]
    fn test_update_parameters() {
        let schedule = ConsolidationSchedule::default();
        let mut versioning = CollectionVersioning::new(1, schedule);

        let params = vec![0.5; 100];
        versioning.update_parameters(&params);

        assert_eq!(versioning.current_parameters(), &params);
    }

    #[test]
    fn test_consolidation() {
        let schedule = ConsolidationSchedule::new(10, 32, 0.01);
        let mut versioning = CollectionVersioning::new(1, schedule);

        versioning.bump_version();
        let params = vec![0.5; 50];
        versioning.update_parameters(&params);

        let gradients: Vec<Vec<f32>> = vec![vec![0.1; 50]; 10];
        // Consolidate with timestamp 5
        let result = versioning.consolidate(&gradients, 5);

        assert!(result.is_ok());

        // Should not consolidate immediately after
        assert!(!versioning.should_consolidate(5));

        // Should consolidate after interval (5 + 10 = 15 or later)
        assert!(versioning.should_consolidate(20));
    }

    #[test]
    fn test_ewc_integration() {
        let schedule = ConsolidationSchedule::default();
        let mut versioning =
            CollectionVersioning::with_lambda(CollectionVersioning::new(1, schedule), 1000.0);

        versioning.bump_version();
        let params = vec![0.5; 20];
        versioning.update_parameters(&params);

        // Consolidate to compute Fisher
        let gradients: Vec<Vec<f32>> = vec![vec![0.1; 20]; 5];
        versioning.consolidate(&gradients, 0).unwrap();

        // Now EWC should be active
        let new_params = vec![0.6; 20];
        versioning.update_parameters(&new_params);

        let loss = versioning.ewc_loss();
        assert!(loss > 0.0, "EWC loss should be positive");

        // Apply EWC to gradients
        let base_grad = vec![0.1; 20];
        let modified_grad = versioning.apply_ewc(&base_grad);

        assert_eq!(modified_grad.len(), 20);
        // Should have added EWC penalty
        assert!(modified_grad.iter().any(|&g| g != 0.1));
    }

    #[test]
    fn test_eligibility_tracking() {
        let schedule = ConsolidationSchedule::default();
        let mut versioning = CollectionVersioning::new(1, schedule);

        versioning.bump_version();

        versioning.update_eligibility(0, 1.0);
        versioning.update_eligibility(1, 0.5);

        let version = versioning.get_version(1).unwrap();
        assert!(version.get_eligibility(0) > 0.9);
        assert!(version.get_eligibility(1) > 0.4);
    }

    #[test]
    fn test_multiple_versions() {
        let schedule = ConsolidationSchedule::default();
        let mut versioning = CollectionVersioning::new(1, schedule);

        for v in 1..=5 {
            versioning.bump_version();
            assert_eq!(versioning.version(), v);

            let version = versioning.get_version(v);
            assert!(version.is_some());
            assert_eq!(version.unwrap().version, v);
        }
    }
}
