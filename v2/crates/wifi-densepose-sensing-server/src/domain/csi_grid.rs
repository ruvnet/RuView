//! CSI symbol grid keys, calibration bindings, and receipts.
//!
//! Subcarrier count alone is insufficient to identify a symbol grid because
//! HT-LTF and HE-LTF use different tone layouts (ADR-110 / issue #1005).

use serde::{Deserialize, Serialize};
use crate::bootstrap_baseline;
use crate::field_bridge;
use crate::ingest::Esp32Frame;

/// Exact CSI symbol grid used by one field model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CsiGridKey {
    pub n_subcarriers: u16,
    pub ppdu_type: u8,
}

impl CsiGridKey {
    #[inline]
    pub fn from_frame(frame: &Esp32Frame) -> Self {
        Self {
            n_subcarriers: frame.n_subcarriers,
            ppdu_type: frame.ppdu_type.to_byte(),
        }
    }

    pub fn as_bootstrap_grid(self) -> bootstrap_baseline::BootstrapCsiGrid {
        bootstrap_baseline::BootstrapCsiGrid {
            n_subcarriers: self.n_subcarriers,
            ppdu_type: self.ppdu_type,
        }
    }

    pub fn from_bootstrap_grid(grid: bootstrap_baseline::BootstrapCsiGrid) -> Self {
        Self {
            n_subcarriers: grid.n_subcarriers,
            ppdu_type: grid.ppdu_type,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RawGridObservation {
    pub grid: CsiGridKey,
    pub observed_at: std::time::Instant,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CalibrationGridEvidence {
    pub sample_count: usize,
    pub span_s: f64,
    pub rate_hz: f64,
    pub max_gap_s: f64,
    pub latest_age_s: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct CalibrationGridBinding {
    pub source_node_id: u8,
    pub grid: CsiGridKey,
    pub evidence: Option<CalibrationGridEvidence>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CalibrationModelReceipt {
    pub schema: &'static str,
    pub boot_epoch: String,
    pub session_id: String,
    pub model_id: String,
    pub binding_digest: String,
    pub source_node_ids: Vec<u8>,
    pub frame_count: u64,
    pub variance_explained: f64,
    pub baseline_eigenvalue_count: usize,
    pub completed_at_unix_ms: u64,
}

/// Bounded decision explanation, separate from the strict authority envelope.
#[derive(Debug, Clone, Serialize)]
pub struct DetectorDiagnostics {
    pub schema: &'static str,
    pub scored_node_id: u8,
    pub source_age_ms: u64,
    pub source_sequence: u32,
    pub required_window_frames: usize,
    pub runtime: field_bridge::OccupancyDiagnostics,
}

/// Per-frame occupancy result bound to one immutable field-model calibration receipt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CalibratedPresenceEvidence {
    pub schema: String,
    pub boot_epoch: String,
    pub session_id: String,
    pub model_id: String,
    pub binding_digest: String,
    pub source_node_ids: Vec<u8>,
    pub model_completed_at_unix_ms: u64,
    pub inference_node_id: u8,
    pub source_tick: u64,
    pub observed_at_unix_ms: u64,
    pub inference_method: String,
    pub presence: bool,
    pub person_count: usize,
    #[serde(skip)]
    pub diagnostics: Option<DetectorDiagnostics>,
}

