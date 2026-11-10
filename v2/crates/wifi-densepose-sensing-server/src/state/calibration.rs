//! Calibration sequence ordering, grid freshness, and field model feeding.
//!
//! Complies with ELMAS.md: <= 350 lines per file, guard clauses, no nesting.

use std::collections::VecDeque;
use std::time::Instant;
use tracing::{debug, warn};

use wifi_densepose_sensing_server::domain::{
    CalibratedPresenceEvidence, CalibrationGridBinding, CsiGridKey, DetectorDiagnostics,
};
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_sensing_server::vital_signs::VitalSigns;
use wifi_densepose_signal::ruvsense::field_model::CalibrationStatus;

use crate::api::calibration::calibration_source_accepts;
use super::types::{AppStateInner, ESP32_OFFLINE_TIMEOUT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CalibrationSequenceOrder {
    First,
    Forward,
    Duplicate,
    Reordered(u32),
    Discontinuity,
}

/// Bounded same-source UDP reordering window.
pub const CALIBRATION_SEQUENCE_REORDER_WINDOW: u32 = 16;

pub fn calibration_sequence_order(
    previous: Option<u32>,
    sequence: u32,
) -> CalibrationSequenceOrder {
    let Some(previous) = previous else {
        return CalibrationSequenceOrder::First;
    };
    let delta = sequence.wrapping_sub(previous);
    if delta == 0 {
        return CalibrationSequenceOrder::Duplicate;
    }
    if delta < (1_u32 << 31) {
        return CalibrationSequenceOrder::Forward;
    }
    let reorder_depth = previous.wrapping_sub(sequence);
    if reorder_depth <= CALIBRATION_SEQUENCE_REORDER_WINDOW {
        CalibrationSequenceOrder::Reordered(reorder_depth)
    } else {
        CalibrationSequenceOrder::Discontinuity
    }
}

impl AppStateInner {
    /// Admit at most one raw CSI observation per forward node sequence into
    /// the current calibration session.
    pub fn maybe_feed_calibration_frame(
        &mut self,
        node_id: u8,
        grid: CsiGridKey,
        sequence: u32,
        amplitudes: &[f64],
        observed_at: Instant,
    ) -> bool {
        let binding_matches = self
            .calibration_grid_binding
            .is_some_and(|binding| binding.grid == grid);
        if !binding_matches
            || !calibration_source_accepts(&self.calibration_source_node_ids, node_id)
            || self.calibration_sequence_fault_node_ids.contains(&node_id)
        {
            return false;
        }

        let previous_sequence = self.calibration_last_sequences.get(&node_id).copied();
        match calibration_sequence_order(previous_sequence, sequence) {
            CalibrationSequenceOrder::Duplicate => return false,
            CalibrationSequenceOrder::Reordered(depth) => {
                *self
                    .calibration_reordered_packets
                    .entry(node_id)
                    .or_default() += 1;
                self.calibration_max_reorder_depth
                    .entry(node_id)
                    .and_modify(|observed| *observed = (*observed).max(depth))
                    .or_insert(depth);
                debug!(
                    node_id,
                    previous_sequence,
                    sequence,
                    depth,
                    "Ignored bounded late CSI packet during calibration"
                );
                return false;
            }
            CalibrationSequenceOrder::Discontinuity => {
                self.calibration_sequence_fault_node_ids.insert(node_id);
                warn!(
                    node_id,
                    previous_sequence,
                    sequence,
                    "Calibration source sequence discontinuity exceeded reorder window; restart empty-room capture"
                );
                return false;
            }
            CalibrationSequenceOrder::First | CalibrationSequenceOrder::Forward => {}
        }

        let writes_baseline = self
            .calibration_grid_binding
            .is_some_and(|binding| binding.source_node_id == node_id);
        let accepted = self.field_model.as_mut().is_some_and(|field| {
            if matches!(
                field.status(),
                CalibrationStatus::Uncalibrated | CalibrationStatus::Collecting
            ) {
                if writes_baseline {
                    field_bridge::maybe_feed_calibration(field, amplitudes)
                } else {
                    true
                }
            } else {
                field.check_freshness(
                    (chrono::Utc::now().timestamp_millis().max(0) as u64)
                        .saturating_mul(1_000),
                ) == CalibrationStatus::Fresh
            }
        });

        if accepted {
            self.calibration_observed_source_node_ids.insert(node_id);
            self.calibration_last_sequences.insert(node_id, sequence);
            if let Some(node) = self.node_states.get_mut(&node_id) {
                node.push_field_model_frame(sequence, amplitudes, observed_at);
            }
        }
        accepted
    }

    pub fn clear_calibration_sequence_state(&mut self) {
        self.calibration_last_sequences.clear();
        self.calibration_reordered_packets.clear();
        self.calibration_max_reorder_depth.clear();
        self.calibration_sequence_fault_node_ids.clear();
    }

    pub fn clear_field_model_binding(&mut self) {
        self.calibration_grid_binding = None;
        for node in self.node_states.values_mut() {
            node.clear_field_model_history();
        }
    }

    pub fn calibration_grid_is_fresh(
        &self,
        binding: CalibrationGridBinding,
        now: Instant,
    ) -> bool {
        !self
            .calibration_sequence_fault_node_ids
            .contains(&binding.source_node_id)
            && self
                .node_states
                .get(&binding.source_node_id)
                .and_then(|node| node.field_model_latest_seen)
                .is_some_and(|seen| {
                    now.saturating_duration_since(seen) < ESP32_OFFLINE_TIMEOUT
                })
    }

    pub fn field_model_holdout_ready(
        &self,
        binding: CalibrationGridBinding,
        required_window_size: usize,
        now: Instant,
    ) -> bool {
        self.calibration_grid_binding.is_some_and(|current| {
            current.source_node_id == binding.source_node_id && current.grid == binding.grid
        }) && self.calibration_grid_is_fresh(binding, now)
            && self
                .node_states
                .get(&binding.source_node_id)
                .is_some_and(|node| node.field_model_history.len() >= required_window_size)
    }

    pub fn begin_field_model_holdout(&mut self, binding: CalibrationGridBinding) {
        if let Some(node) = self.node_states.get_mut(&binding.source_node_id) {
            node.clear_field_model_history();
        }
    }

    pub fn field_model_status_at(&self, observed_at_unix_ms: u64) -> Option<CalibrationStatus> {
        self.field_model.as_ref().map(|model| {
            field_bridge::calibration_status_at(model, observed_at_unix_ms.saturating_mul(1_000))
        })
    }

    pub fn field_model_active_at(&self, observed_at_unix_ms: u64) -> bool {
        self.field_model_status_at(observed_at_unix_ms)
            .is_some_and(|status| status != CalibrationStatus::Expired)
    }

    pub fn bootstrap_baseline_active_at(&self, observed_at_unix_ms: u64) -> bool {
        self.bootstrap_baseline_active && self.field_model_active_at(observed_at_unix_ms)
    }

    pub fn explicit_calibration_fresh_at(&self, observed_at_unix_ms: u64) -> bool {
        !self.bootstrap_baseline_active
            && self.field_model_status_at(observed_at_unix_ms) == Some(CalibrationStatus::Fresh)
    }

    pub fn bound_source_node_id(&self) -> Option<u8> {
        self.calibration_grid_binding
            .map(|binding| binding.source_node_id)
            .or_else(|| {
                self.bootstrap_baseline.as_ref().and_then(|metadata| {
                    (metadata.source_node_ids.len() == 1)
                        .then_some(metadata.source_node_ids[0])
                })
            })
    }

    pub fn scoring_history(&self) -> &VecDeque<Vec<f64>> {
        if let Some(node_id) = self.bound_source_node_id() {
            self.node_states
                .get(&node_id)
                .map(|state| &state.field_model_history)
                .unwrap_or(&self.frame_history)
        } else if !self.frame_history.is_empty() {
            &self.frame_history
        } else {
            self.node_states
                .values()
                .filter(|ns| !ns.frame_history.is_empty())
                .max_by_key(|ns| ns.last_frame_time)
                .map(|ns| &ns.frame_history)
                .unwrap_or(&self.frame_history)
        }
    }

    pub fn calibrated_presence_evidence(
        &self,
        inference_node_id: u8,
        source_tick: u64,
        observed_at_unix_ms: u64,
    ) -> Option<CalibratedPresenceEvidence> {
        if !self.explicit_calibration_fresh_at(observed_at_unix_ms) {
            return None;
        }
        let receipt = self.calibration_model_receipt.as_ref()?;
        let occupancy = field_bridge::calibrated_occupancy(
            self.field_model.as_ref()?,
            self.scoring_history(),
            observed_at_unix_ms.saturating_mul(1_000),
        )?;
        Some(CalibratedPresenceEvidence {
            schema: field_bridge::CALIBRATED_PRESENCE_EVIDENCE_SCHEMA.to_string(),
            boot_epoch: receipt.boot_epoch.clone(),
            session_id: receipt.session_id.clone(),
            model_id: receipt.model_id.clone(),
            binding_digest: receipt.binding_digest.clone(),
            source_node_ids: receipt.source_node_ids.clone(),
            model_completed_at_unix_ms: receipt.completed_at_unix_ms,
            inference_node_id,
            source_tick,
            observed_at_unix_ms,
            inference_method: occupancy.method.wire_name().to_string(),
            presence: occupancy.person_count > 0,
            person_count: occupancy.person_count,
            diagnostics: self.calibration_grid_binding.and_then(|binding| {
                let node = self.node_states.get(&binding.source_node_id)?;
                Some(DetectorDiagnostics {
                    schema: field_bridge::DETECTOR_DIAGNOSTICS_SCHEMA,
                    scored_node_id: binding.source_node_id,
                    source_age_ms: node.field_model_latest_seen?.elapsed().as_millis().min(u64::MAX as u128) as u64,
                    source_sequence: node.field_model_latest_sequence?,
                    required_window_frames: self.field_model.as_ref()?.modes()?.baseline_runtime_window_size.unwrap_or(50),
                    runtime: occupancy.diagnostics,
                })
            }),
        })
    }

    pub fn bootstrap_empty_prior_applies(&self, observed_at_unix_ms: u64) -> bool {
        self.bootstrap_background_match(observed_at_unix_ms)
            .is_some_and(|result| result.matches_empty)
    }

    pub fn bootstrap_background_match(
        &self,
        observed_at_unix_ms: u64,
    ) -> Option<field_bridge::BootstrapBackgroundMatch> {
        if !self.bootstrap_baseline_active {
            return None;
        }
        let field_model = self.field_model.as_ref()?;
        let metadata = self.bootstrap_baseline.as_ref()?;
        let [source_node_id] = metadata.source_node_ids.as_slice() else {
            return None;
        };
        let binding = self.calibration_grid_binding?;
        let grid_ok = binding.grid == CsiGridKey::from_bootstrap_grid(metadata.source_grid);
        if binding.source_node_id != *source_node_id || !grid_ok || self.calibration_sequence_fault_node_ids.contains(source_node_id) {
            return None;
        }
        let node = self.node_states.get(source_node_id)?;
        let fresh = node.field_model_latest_seen.is_some_and(|s| Instant::now().saturating_duration_since(s) < ESP32_OFFLINE_TIMEOUT);
        if !fresh {
            return None;
        }
        field_bridge::bootstrap_background_match(
            field_model,
            &node.field_model_history,
            observed_at_unix_ms.saturating_mul(1_000),
        )
    }

    pub fn calibrated_vitals_for_publication(
        &self,
        candidates: &VitalSigns,
        room_person_count: usize,
        observed_at_unix_ms: u64,
    ) -> Option<VitalSigns> {
        if room_person_count != 1 || !self.calibration_sequence_fault_node_ids.is_empty() {
            return None;
        }
        let binding = self.calibration_grid_binding?;
        let window = self.field_model.as_ref()?.modes()?.baseline_runtime_window_size?;
        if !self.field_model_holdout_ready(binding, window, Instant::now()) {
            return None;
        }
        let evidence = self.calibrated_presence_evidence(binding.source_node_id, self.tick, observed_at_unix_ms)?;
        crate::domain::vitals::vitals_for_publication(candidates, true, evidence.person_count)
    }
}

pub fn calibrated_vitals_for_publication(
    state: &AppStateInner,
    candidates: &VitalSigns,
    room_person_count: usize,
    observed_at_unix_ms: u64,
) -> Option<VitalSigns> {
    state.calibrated_vitals_for_publication(candidates, room_person_count, observed_at_unix_ms)
}
