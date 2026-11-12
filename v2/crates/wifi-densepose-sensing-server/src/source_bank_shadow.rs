//! Opt-in, memory-only per-source calibration diagnostics. Never used by the
//! authoritative model, receipts, persistence, person count or vital publication.
use serde_json::{json, Value};
use std::collections::BTreeMap;
use wifi_densepose_signal::hardware_norm::HardwareNormalizer;
use wifi_densepose_signal::ruvsense::{
    field_model::FieldModelConfig,
    source_field_bank::{SourceBankPhase, SourceFieldBank, SourceFieldSpec, SourceFieldWindow},
};

pub(crate) struct ShadowCapture {
    bank: SourceFieldBank,
    grid: String,
    ordinals: BTreeMap<u8, u64>,
    latest: BTreeMap<u8, u64>,
    windows: BTreeMap<u8, Vec<Vec<f64>>>,
    failed: bool,
    input_width: usize,
    canonical_width: usize,
    normalizer: HardwareNormalizer,
}

impl ShadowCapture {
    pub(crate) fn new(ids: &[u8], width: usize, grid: String) -> Option<Self> {
        let mut capture = Self::with_config(ids, grid, super::field_bridge::single_link_config())?;
        capture.input_width = width;
        Some(capture)
    }

    fn with_config(ids: &[u8], grid: String, config: FieldModelConfig) -> Option<Self> {
        let specs = ids
            .iter()
            .map(|&source_id| SourceFieldSpec {
                source_id,
                grid_id: grid.clone(),
                config: config.clone(),
            })
            .collect();
        let bank = SourceFieldBank::new(specs, 5_000_000).ok()?;
        Some(Self {
            bank,
            grid,
            ordinals: ids.iter().map(|&id| (id, 0)).collect(),
            latest: BTreeMap::new(),
            windows: BTreeMap::new(),
            failed: false,
            input_width: config.n_subcarriers,
            canonical_width: config.n_subcarriers,
            normalizer: HardwareNormalizer::new(),
        })
    }

    /// Only called after the existing source/grid/sequence admission succeeds.
    /// Receiver timestamps and validated ordinals never come from an HTTP client.
    pub(crate) fn observe(&mut self, id: u8, timestamp_us: u64, frame: &[f64]) {
        if self.failed {
            return;
        }
        let Some(ordinal) = self.ordinals.get_mut(&id) else {
            self.failed = true;
            return;
        };
        if frame.len() != self.input_width
            || frame.iter().any(|v| !v.is_finite() || v.abs() > 1_000_000.)
            || timestamp_us == 0
            || self
                .latest
                .get(&id)
                .is_some_and(|at| timestamp_us <= *at || timestamp_us - *at > 5_000_000)
        {
            self.failed = true;
            return;
        }
        let canonical = if self.canonical_width == 56 {
            self.normalizer.resample_to_canonical(frame)
        } else {
            frame.to_vec()
        };
        let frame = canonical.as_slice();
        *ordinal = ordinal.saturating_add(1);
        if self.bank.phase() == SourceBankPhase::Collecting {
            if self
                .bank
                .feed(id, &self.grid, *ordinal, timestamp_us, frame)
                .is_err()
            {
                self.failed = true;
                return;
            }
        } else if self.bank.phase() != SourceBankPhase::Frozen {
            self.failed = true;
            return;
        }
        // Keep at most one runtime-sized window per configured source.
        self.latest.insert(id, timestamp_us);
        let window = self.windows.entry(id).or_default();
        window.push(frame.to_vec());
        if window.len() > 50 {
            window.remove(0);
        }
    }

    pub(crate) fn invalidate(&mut self) {
        self.failed = true;
    }

    pub(crate) fn freeze(&mut self, timestamp_us: u64) {
        if self.failed || self.bank.freeze(timestamp_us, 0).is_err() {
            self.failed = true;
        }
        // Held-out windows must contain no training frames.
        self.windows.clear();
    }

    pub(crate) fn diagnostics(&self, timestamp_us: u64) -> Value {
        let phase = if self.failed {
            "invalid"
        } else {
            match self.bank.phase() {
                SourceBankPhase::Collecting => "collecting",
                SourceBankPhase::Frozen => "frozen",
                SourceBankPhase::Invalid => "invalid",
            }
        };
        let comparisons = if !self.failed
            && self.bank.phase() == SourceBankPhase::Frozen
            && self.windows.values().all(|frames| frames.len() == 50)
        {
            let windows: Vec<_> = self
                .windows
                .iter()
                .map(|(&id, frames)| SourceFieldWindow {
                    source_id: id,
                    grid_id: &self.grid,
                    observed_at_us: self.latest[&id],
                    frames,
                })
                .collect();
            self.bank
                .compare_empty_windows(timestamp_us, &windows)
                .ok()
                .map(|result| {
                    result
                        .into_iter()
                        .map(|(id, score)| {
                            (
                                id.to_string(),
                                json!({
                                                "matches_empty": score.matches_empty,
                                                "residual_energy": score.residual_energy,
                                                "threshold": score.residual_energy_threshold,
                                "reliable": score.reliable,
                                "maturity": score.maturity,
                                "reference_window_count": score.reference_window_count,
                                "window_size": score.window_size,
                                            }),
                            )
                        })
                        .collect::<serde_json::Map<String, Value>>()
                })
        } else {
            None
        };
        json!({ "schema": "ruview.calibration.source-bank-shadow.v1", "phase": phase,
            "frame_counts": self.bank.frame_counts(), "comparisons": comparisons,
            "authority": "diagnostic_only", "persisted": false,
            "calibrated_presence_authorized": false, "numeric_vitals_authorized": false })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn capture() -> ShadowCapture {
        ShadowCapture::with_config(
            &[15, 16, 17],
            "ht64".into(),
            FieldModelConfig {
                n_links: 1,
                n_subcarriers: 4,
                n_modes: 1,
                min_calibration_frames: 1_000,
                min_calibration_duration_s: 0.0,
                ..Default::default()
            },
        )
        .unwrap()
    }
    #[test]
    fn incomplete_capture_never_freezes_or_exposes_comparisons() {
        let mut s = capture();
        for i in 1..=1_000 {
            s.observe(15, i * 1_000, &[1., 2., 3., 4.]);
        }
        s.freeze(1_000_001);
        let d = s.diagnostics(1_000_002);
        assert_eq!(d["phase"], "invalid");
        assert!(d["comparisons"].is_null());
        assert_eq!(d["numeric_vitals_authorized"], false);
    }
    #[test]
    fn freeze_requires_new_complete_held_out_windows_and_never_exports_raw_frames() {
        let mut s = capture();
        for i in 1..=1_000 {
            for id in [15, 16, 17] {
                s.observe(id, i * 1_000, &[id as f64, 2., 3., 4.]);
            }
        }
        s.freeze(1_000_001);
        assert_eq!(s.diagnostics(1_000_002)["phase"], "frozen");
        assert!(s.diagnostics(1_000_002)["comparisons"].is_null());
        for i in 1..=50 {
            for id in [15, 16, 17] {
                s.observe(id, 1_001_000 + i * 1_000, &[id as f64, 2., 3., 4.]);
            }
        }
        let d = s.diagnostics(1_052_000);
        assert_eq!(d["comparisons"].as_object().unwrap().len(), 3);
        assert_eq!(d["frame_counts"]["15"], 1_000);
        assert_eq!(d["calibrated_presence_authorized"], false);
        assert!(d.get("windows").is_none());
        assert!(s.diagnostics(7_000_000)["comparisons"].is_null());
    }
    #[test]
    fn production_resampling_matches_the_single_source_control() {
        use wifi_densepose_signal::ruvsense::field_model::FieldModel;
        let config = FieldModelConfig {
            min_calibration_frames: 1_000,
            min_calibration_duration_s: 0.0,
            ..super::super::field_bridge::single_link_config()
        };
        let mut control = FieldModel::new(config.clone()).unwrap();
        let mut shadow = ShadowCapture::with_config(&[15, 16, 17], "ht64".into(), config).unwrap();
        shadow.input_width = 64;
        for i in 1..=1_000 {
            for id in [15, 16, 17] {
                let frame: Vec<_> = (0..64)
                    .map(|tone| id as f64 + tone as f64 + (i % 13) as f64 * 0.01)
                    .collect();
                if id == 15 {
                    assert!(super::super::field_bridge::maybe_feed_calibration(
                        &mut control,
                        &frame
                    ));
                }
                shadow.observe(id, i * 1_000, &frame);
            }
        }
        control.finalize_calibration(1_000_001, 0).unwrap();
        shadow.freeze(1_000_001);
        let models = shadow.bank.frozen_models().unwrap();
        assert_eq!(
            models[0].snapshot.modes.baseline,
            control.export_snapshot().unwrap().modes.baseline
        );
        assert_ne!(
            models[0].snapshot.modes.baseline,
            models[1].snapshot.modes.baseline
        );
    }
    #[test]
    fn unknown_or_changed_runtime_sources_invalidate_only_the_shadow() {
        let mut s = capture();
        s.observe(99, 1_000, &[1., 2., 3., 4.]);
        assert_eq!(s.diagnostics(1_001)["phase"], "invalid");
        assert_eq!(s.bank.frame_counts().len(), 3);
        let mut s = capture();
        s.observe(15, 1_000, &[1., 2., 3., 4.]);
        s.observe(15, 1_000, &[1., 2., 3., 4.]);
        assert_eq!(s.diagnostics(1_001)["phase"], "invalid");
    }
    #[tokio::test]
    async fn status_is_opt_in_privacy_redacted_and_cleared_with_binding() {
        use super::super::{calibration_status, AppStateInner};
        use axum::extract::State;
        use std::sync::Arc;
        use tokio::sync::RwLock;
        let state = Arc::new(RwLock::new(AppStateInner::minimal()));
        let axum::Json(status) = calibration_status(State(state.clone())).await;
        assert!(status.get("source_bank_shadow").is_none());
        state.write().await.source_bank_shadow = Some(capture());
        let axum::Json(status) = calibration_status(State(state.clone())).await;
        assert_eq!(status["source_bank_shadow"]["authority"], "diagnostic_only");
        state.write().await.privacy_mode = true;
        let axum::Json(status) = calibration_status(State(state.clone())).await;
        assert!(status.get("source_bank_shadow").is_none());
        state.write().await.clear_field_model_binding();
        assert!(state.read().await.source_bank_shadow.is_none());
    }
    #[test]
    fn opt_in_admission_trains_each_shadow_without_pooling_the_primary() {
        use super::super::{AppStateInner, CalibrationGridBinding, CsiGridKey};
        use wifi_densepose_signal::ruvsense::field_model::FieldModel;
        let mut state = AppStateInner::minimal();
        let grid = CsiGridKey {
            n_subcarriers: 64,
            ppdu_type: 0,
        };
        state.calibration_grid_binding = Some(CalibrationGridBinding {
            source_node_id: 15,
            grid,
            evidence: None,
        });
        state.calibration_source_node_ids.extend([15, 16, 17]);
        state.field_model =
            Some(FieldModel::new(super::super::field_bridge::single_link_config()).unwrap());
        state.source_bank_shadow = ShadowCapture::new(&[15, 16, 17], 64, "ht64".into());
        for sequence in 1..=100 {
            for id in [15, 16, 17] {
                assert!(state.maybe_feed_calibration_frame(
                    id,
                    grid,
                    sequence,
                    &vec![id as f64; 64],
                    std::time::Instant::now()
                ));
            }
        }
        assert_eq!(
            state
                .field_model
                .as_ref()
                .unwrap()
                .calibration_frame_count(),
            100
        );
        let shadow = state.source_bank_shadow.as_ref().unwrap();
        assert_eq!(
            shadow.bank.frame_counts(),
            BTreeMap::from([(15, 100), (16, 100), (17, 100)])
        );
        assert!(state.calibration_model_receipt.is_none());
        assert_eq!(
            shadow.diagnostics(u64::MAX)["numeric_vitals_authorized"],
            false
        );
        assert!(!state.maybe_feed_calibration_frame(
            15,
            grid,
            0,
            &vec![15.; 64],
            std::time::Instant::now()
        ));
        assert_eq!(
            state
                .source_bank_shadow
                .as_ref()
                .unwrap()
                .diagnostics(u64::MAX)["phase"],
            "invalid"
        );
        assert_eq!(
            state
                .field_model
                .as_ref()
                .unwrap()
                .calibration_frame_count(),
            100
        );
    }
    #[test]
    fn command_line_shadow_is_disabled_by_default() {
        use clap::Parser;
        assert!(
            !super::super::Args::try_parse_from(["sensing-server"])
                .unwrap()
                .source_bank_shadow
        );
        assert!(
            super::super::Args::try_parse_from(["sensing-server", "--source-bank-shadow"])
                .unwrap()
                .source_bank_shadow
        );
    }
}
