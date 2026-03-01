//! Standard Interface Traits for ruQu
//!
//! These traits define the pluggable interfaces for ruQu, allowing:
//! - Different syndrome sources (simulators, hardware)
//! - Different gate engines (min-cut, heuristic, ML)
//! - Different action sinks (logging, hardware control)
//!
//! This keeps the core logic stable while data sources and backends change.

use crate::syndrome::DetectorBitmap;
use std::time::Duration;

/// Error type for trait implementations
#[derive(Debug, Clone, thiserror::Error)]
pub enum TraitError {
    /// Source has no more data
    #[error("Source exhausted")]
    SourceExhausted,
    /// Hardware communication error
    #[error("Hardware error: {0}")]
    HardwareError(String),
    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),
    /// Operation timed out
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

/// Result type for trait operations
pub type TraitResult<T> = Result<T, TraitError>;

// ============================================================================
// SYNDROME SOURCE TRAIT
// ============================================================================

/// A source of syndrome data (detector events)
///
/// Implementations can be:
/// - Stim-based simulator
/// - File replay
/// - Hardware interface
/// - Network stream
pub trait SyndromeSource: Send {
    /// Sample the next syndrome round
    fn sample(&mut self) -> TraitResult<DetectorBitmap>;

    /// Get the number of detectors per round
    fn num_detectors(&self) -> usize;

    /// Get the code distance (if known)
    fn code_distance(&self) -> Option<usize> {
        None
    }

    /// Check if the source is exhausted (for finite sources)
    fn is_exhausted(&self) -> bool {
        false
    }

    /// Reset the source to the beginning (if supported)
    fn reset(&mut self) -> TraitResult<()> {
        Err(TraitError::ConfigError("Reset not supported".into()))
    }

    /// Get source metadata
    fn metadata(&self) -> SourceMetadata {
        SourceMetadata::default()
    }
}

/// Metadata about a syndrome source
#[derive(Debug, Clone, Default)]
pub struct SourceMetadata {
    /// Human-readable name
    pub name: String,
    /// Code distance
    pub code_distance: Option<usize>,
    /// Error rate (if known)
    pub error_rate: Option<f64>,
    /// Number of rounds (if finite)
    pub total_rounds: Option<u64>,
    /// Source version/format
    pub version: String,
}

// ============================================================================
// TELEMETRY SOURCE TRAIT
// ============================================================================

/// A source of telemetry data (temperature, timing, etc.)
pub trait TelemetrySource: Send {
    /// Get current telemetry snapshot
    fn snapshot(&self) -> TelemetrySnapshot;

    /// Check if telemetry indicates a problem
    fn has_alert(&self) -> bool {
        false
    }
}

/// Telemetry data snapshot
#[derive(Debug, Clone, Default)]
pub struct TelemetrySnapshot {
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// Fridge temperature in Kelvin (if available)
    pub fridge_temp_k: Option<f64>,
    /// Qubit temperatures (per qubit, if available)
    pub qubit_temps: Vec<f64>,
    /// Readout fidelity estimates
    pub readout_fidelity: Vec<f64>,
    /// Gate error estimates
    pub gate_errors: Vec<f64>,
    /// Custom key-value pairs
    pub custom: Vec<(String, f64)>,
}

// ============================================================================
// GATE ENGINE TRAIT
// ============================================================================

/// A gate decision engine
///
/// Takes syndrome data and produces permit/defer/deny decisions.
pub trait GateEngine: Send {
    /// Process a syndrome round and return a decision
    fn process(&mut self, syndrome: &DetectorBitmap) -> GateDecision;

    /// Get the current risk assessment
    fn risk_assessment(&self) -> RiskAssessment;

    /// Update thresholds or parameters
    fn update_config(&mut self, config: GateConfig) -> TraitResult<()>;

    /// Get engine statistics
    fn statistics(&self) -> EngineStatistics;

    /// Reset engine state
    fn reset(&mut self);
}

/// Gate decision output
#[derive(Debug, Clone, PartialEq)]
pub enum GateDecision {
    /// Permit the operation - low risk
    Permit {
        /// Confidence level (0.0 to 1.0)
        confidence: f64,
        /// Time-to-live in nanoseconds
        ttl_ns: u64,
        /// Optional explanation
        reason: Option<String>,
    },
    /// Defer - uncertain, need more data
    Defer {
        /// Suggested wait time in nanoseconds
        wait_ns: u64,
        /// Uncertainty level
        uncertainty: f64,
    },
    /// Deny - high risk detected
    Deny {
        /// Risk level (0.0 to 1.0)
        risk_level: f64,
        /// Recommended action
        recommended_action: String,
        /// Affected regions (bitmask or list)
        affected_regions: Vec<u32>,
    },
}

impl Default for GateDecision {
    fn default() -> Self {
        GateDecision::Defer {
            wait_ns: 1000,
            uncertainty: 1.0,
        }
    }
}

/// Risk assessment from the gate engine
#[derive(Debug, Clone, Default)]
pub struct RiskAssessment {
    /// Overall risk level (0.0 = safe, 1.0 = critical)
    pub overall_risk: f64,
    /// Structural risk (from min-cut)
    pub structural_risk: f64,
    /// Temporal risk (from recent history)
    pub temporal_risk: f64,
    /// Spatial risk (from region clustering)
    pub spatial_risk: f64,
    /// Risk per region
    pub region_risks: Vec<(u32, f64)>,
    /// Confidence in assessment
    pub confidence: f64,
}

/// Gate engine configuration
#[derive(Debug, Clone)]
pub struct GateConfig {
    /// Minimum cut threshold for permit
    pub min_cut_threshold: f64,
    /// Maximum shift for permit
    pub max_shift: f64,
    /// Permit tau threshold
    pub tau_permit: f64,
    /// Deny tau threshold
    pub tau_deny: f64,
    /// Permit time-to-live in ns
    pub permit_ttl_ns: u64,
}

impl Default for GateConfig {
    fn default() -> Self {
        Self {
            min_cut_threshold: 5.0,
            max_shift: 0.2,
            tau_permit: 0.3,
            tau_deny: 0.7,
            permit_ttl_ns: 100_000,
        }
    }
}

/// Statistics from the gate engine
#[derive(Debug, Clone, Default)]
pub struct EngineStatistics {
    /// Total rounds processed
    pub total_rounds: u64,
    /// Permits issued
    pub permits: u64,
    /// Defers issued
    pub defers: u64,
    /// Denies issued
    pub denies: u64,
    /// Average processing time in nanoseconds
    pub avg_process_ns: f64,
    /// P99 processing time in nanoseconds
    pub p99_process_ns: u64,
    /// P999 processing time in nanoseconds
    pub p999_process_ns: u64,
    /// Max processing time in nanoseconds
    pub max_process_ns: u64,
}

// ============================================================================
// ACTION SINK TRAIT
// ============================================================================

/// A sink for mitigation actions
///
/// Receives actions from the gate engine and executes them.
pub trait ActionSink: Send {
    /// Execute an action
    fn execute(&mut self, action: &MitigationAction) -> TraitResult<ActionResult>;

    /// Check if an action is supported
    fn supports(&self, action_type: ActionType) -> bool;

    /// Get sink capabilities
    fn capabilities(&self) -> ActionCapabilities;
}

/// Types of mitigation actions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionType {
    /// Quarantine a region
    QuarantineRegion,
    /// Increase syndrome measurement rounds
    IncreaseSyndromeRounds,
    /// Switch decoder mode
    SwitchDecodeMode,
    /// Trigger re-weighting
    TriggerReweight,
    /// Pause learning/writes
    PauseLearningWrites,
    /// Log event
    LogEvent,
    /// Alert operator
    AlertOperator,
    /// Inject test error
    InjectTestError,
}

/// A mitigation action to execute
#[derive(Debug, Clone)]
pub struct MitigationAction {
    /// Action type
    pub action_type: ActionType,
    /// Target region(s)
    pub target_regions: Vec<u32>,
    /// Parameters (action-specific)
    pub parameters: ActionParameters,
    /// Priority (higher = more urgent)
    pub priority: u8,
    /// Preconditions that must be true
    pub preconditions: Vec<Precondition>,
    /// Estimated cost
    pub estimated_cost: ActionCost,
    /// Expected effect
    pub expected_effect: String,
}

/// Action parameters
#[derive(Debug, Clone, Default)]
pub struct ActionParameters {
    /// Duration in nanoseconds (if applicable)
    pub duration_ns: Option<u64>,
    /// Intensity level (0.0 to 1.0)
    pub intensity: Option<f64>,
    /// Custom key-value pairs
    pub custom: Vec<(String, String)>,
}

/// Precondition for an action
#[derive(Debug, Clone)]
pub enum Precondition {
    /// Risk level must be above threshold
    RiskAbove(f64),
    /// Risk level must be below threshold
    RiskBelow(f64),
    /// Region must be in specified state
    RegionState(u32, String),
    /// Time since last action of this type
    TimeSinceLastAction(ActionType, Duration),
    /// Custom condition
    Custom(String),
}

/// Cost estimate for an action
#[derive(Debug, Clone, Default)]
pub struct ActionCost {
    /// Time cost in nanoseconds
    pub time_ns: u64,
    /// Qubit overhead (extra qubits needed)
    pub qubit_overhead: u32,
    /// Fidelity impact (0.0 = no impact, 1.0 = total loss)
    pub fidelity_impact: f64,
    /// Throughput impact (0.0 = no impact, 1.0 = total stop)
    pub throughput_impact: f64,
}

/// Result of executing an action
#[derive(Debug, Clone)]
pub struct ActionResult {
    /// Whether the action succeeded
    pub success: bool,
    /// Actual cost incurred
    pub actual_cost: ActionCost,
    /// Any warnings or notes
    pub notes: Vec<String>,
}

/// Capabilities of an action sink
#[derive(Debug, Clone, Default)]
pub struct ActionCapabilities {
    /// Supported action types
    pub supported_actions: Vec<ActionType>,
    /// Maximum concurrent actions
    pub max_concurrent: u32,
    /// Minimum action interval in nanoseconds
    pub min_interval_ns: u64,
}

// ============================================================================
// CONVENIENCE IMPLEMENTATIONS
// ============================================================================

/// Null syndrome source for testing
pub struct NullSyndromeSource {
    num_detectors: usize,
}

impl NullSyndromeSource {
    /// Create a new null syndrome source
    pub fn new(num_detectors: usize) -> Self {
        Self { num_detectors }
    }
}

impl SyndromeSource for NullSyndromeSource {
    fn sample(&mut self) -> TraitResult<DetectorBitmap> {
        Ok(DetectorBitmap::new(self.num_detectors))
    }

    fn num_detectors(&self) -> usize {
        self.num_detectors
    }
}

/// Logging action sink
pub struct LoggingActionSink {
    log_prefix: String,
}

impl LoggingActionSink {
    /// Create a new logging action sink with given prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            log_prefix: prefix.to_string(),
        }
    }
}

impl ActionSink for LoggingActionSink {
    fn execute(&mut self, action: &MitigationAction) -> TraitResult<ActionResult> {
        println!(
            "{}: {:?} on regions {:?}",
            self.log_prefix, action.action_type, action.target_regions
        );
        Ok(ActionResult {
            success: true,
            actual_cost: ActionCost::default(),
            notes: vec![],
        })
    }

    fn supports(&self, _action_type: ActionType) -> bool {
        true
    }

    fn capabilities(&self) -> ActionCapabilities {
        ActionCapabilities {
            supported_actions: vec![ActionType::LogEvent, ActionType::AlertOperator],
            max_concurrent: 100,
            min_interval_ns: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_syndrome_source() {
        let mut source = NullSyndromeSource::new(100);
        let syndrome = source.sample().unwrap();
        assert_eq!(syndrome.fired_count(), 0);
        assert_eq!(source.num_detectors(), 100);
    }

    #[test]
    fn test_gate_decision_default() {
        let decision = GateDecision::default();
        match decision {
            GateDecision::Defer { .. } => (),
            _ => panic!("Default should be Defer"),
        }
    }

    #[test]
    fn test_logging_action_sink() {
        let mut sink = LoggingActionSink::new("[TEST]");
        let action = MitigationAction {
            action_type: ActionType::LogEvent,
            target_regions: vec![1, 2, 3],
            parameters: ActionParameters::default(),
            priority: 5,
            preconditions: vec![],
            estimated_cost: ActionCost::default(),
            expected_effect: "Log the event".into(),
        };
        let result = sink.execute(&action).unwrap();
        assert!(result.success);
    }
}
