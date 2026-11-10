//! State inspection, source resolution, and person-count calculations.
//!
//! Complies with ELMAS.md: <= 350 lines per file, early returns, clean abstraction.

use std::time::{Duration, Instant};

use wifi_densepose_sensing_server::domain::score_to_person_count;
use wifi_densepose_sensing_server::field_bridge;
use wifi_densepose_sensing_server::provenance::SourceState;

use super::types::{AppStateInner, ESP32_OFFLINE_TIMEOUT};

impl AppStateInner {
    /// Advance the sensing tick and record update for measured stream rate.
    pub fn advance_tick(&mut self) -> u64 {
        self.tick += 1;
        self.update_rate.observe(Instant::now());
        self.tick
    }

    /// Person count at a specific timestamp, using field model if calibrated.
    pub fn person_count_at(&self, observed_at_unix_ms: u64) -> usize {
        if self.bootstrap_baseline_active {
            return score_to_person_count(self.smoothed_person_score, self.prev_person_count);
        }
        match self.field_model.as_ref() {
            Some(fm) => {
                let history = self.scoring_history();
                field_bridge::occupancy_or_fallback(
                    fm,
                    history,
                    observed_at_unix_ms.saturating_mul(1_000),
                    self.smoothed_person_score,
                    self.prev_person_count,
                )
            }
            None => score_to_person_count(self.smoothed_person_score, self.prev_person_count),
        }
    }

    /// Current person count.
    pub fn person_count(&self) -> usize {
        self.person_count_at(chrono::Utc::now().timestamp_millis().max(0) as u64)
    }

    /// Return the effective data source, accounting for frame timeout.
    pub fn effective_source(&self) -> String {
        if self.source == "esp32" {
            if let Some(last) = self.last_esp32_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return "esp32:offline".to_string();
                }
            }
        }
        if self.source.starts_with("realtek_csi") {
            if let Some(last) = self.last_realtek_csi_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        } else if self.source.starts_with("realtek") {
            if let Some(last) = self.last_realtek_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        if self.source.starts_with("mediatek") {
            if let Some(last) = self.last_mediatek_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        if self.source.starts_with("qualcomm") {
            if let Some(last) = self.last_qualcomm_frame {
                if last.elapsed() > ESP32_OFFLINE_TIMEOUT {
                    return format!("{}:offline", self.source);
                }
            }
        }
        self.source.clone()
    }

    /// Age of newest frame accepted from current source, or None if none arrived.
    pub fn last_frame_age(&self) -> Option<Duration> {
        let seen = if self.source == "esp32" {
            self.last_esp32_frame
        } else if self.source.starts_with("realtek_csi") {
            self.last_realtek_csi_frame
        } else if self.source.starts_with("realtek") {
            self.last_realtek_frame
        } else if self.source.starts_with("mediatek") {
            self.last_mediatek_frame
        } else if self.source.starts_with("qualcomm") {
            self.last_qualcomm_frame
        } else if self.source.starts_with("vendor:") {
            self.last_vendor_rf_frame
        } else {
            return self.latest_update.as_ref().map(|update| {
                let now_s = chrono::Utc::now().timestamp_millis() as f64 / 1000.0;
                Duration::from_secs_f64((now_s - update.timestamp).max(0.0))
            });
        };
        seen.map(|t| t.elapsed())
    }

    /// Canonical provenance state for current source.
    pub fn source_state(&self) -> SourceState {
        SourceState::from_source_label(&self.source, self.last_frame_age(), ESP32_OFFLINE_TIMEOUT)
    }

    pub fn aggregate_person_count(&self, activity_count: usize, now: Instant) -> usize {
        let fresh_counts = self
            .node_states
            .values()
            .filter(|n| wifi_densepose_sensing_server::domain::node::node_is_fresh(n, now))
            .map(|n| n.prev_person_count);
        wifi_densepose_sensing_server::domain::fusion::aggregate_person_count(activity_count, fresh_counts)
    }

    pub fn update_room_person_count(
        &mut self,
        classification: &wifi_densepose_sensing_server::domain::ClassificationInfo,
        now: Instant,
        observed_at_unix_ms: u64,
    ) -> usize {
        if !classification.presence
            || !self
                .node_states
                .values()
                .any(|n| wifi_densepose_sensing_server::domain::node::node_is_fresh(n, now))
        {
            self.prev_person_count = 0;
            return 0;
        }
        let (fused, fallback_count) = crate::multistatic_bridge::fuse_or_fallback(
            &self.multistatic_fuser,
            &self.node_states,
            self.dedup_factor,
        );
        let activity_count = match fused {
            Some(ref frame) => {
                let score = crate::multistatic_bridge::compute_person_score_from_amplitudes(
                    &frame.fused_amplitude,
                );
                self.smoothed_person_score = self.smoothed_person_score * 0.90 + score * 0.10;
                self.person_count_at(observed_at_unix_ms)
            }
            None => fallback_count.unwrap_or(0),
        };
        let count = self.aggregate_person_count(activity_count, now).max(1);
        self.prev_person_count = count;
        count
    }

    pub fn smooth_and_classify(
        &mut self,
        raw: &mut wifi_densepose_sensing_server::domain::ClassificationInfo,
        raw_motion: f64,
    ) {
        wifi_densepose_sensing_server::domain::smoothing::presence::smooth_and_classify_parts(
            &mut self.baseline_frames,
            &mut self.baseline_motion,
            &mut self.smoothed_motion,
            &mut self.current_motion_level,
            &mut self.debounce_counter,
            &mut self.debounce_candidate,
            10.0,
            raw,
            raw_motion,
        );
    }

    pub fn smooth_vitals(
        &mut self,
        raw: &wifi_densepose_sensing_server::vital_signs::VitalSigns,
    ) -> wifi_densepose_sensing_server::vital_signs::VitalSigns {
        wifi_densepose_sensing_server::domain::smoothing::vitals::smooth_vitals_parts(
            &mut self.smoothed_hr,
            &mut self.smoothed_br,
            &mut self.smoothed_hr_conf,
            &mut self.smoothed_br_conf,
            &mut self.hr_buffer,
            &mut self.br_buffer,
            raw,
        )
    }

    pub fn adaptive_override(
        &self,
        features: &wifi_densepose_sensing_server::domain::FeatureInfo,
        classification: &mut wifi_densepose_sensing_server::domain::ClassificationInfo,
    ) {
        if let Some(ref model) = self.adaptive_model {
            let amps = self
                .frame_history
                .back()
                .map(|v| v.as_slice())
                .unwrap_or(&[]);
            let dominant_frequency = crate::adaptive_classifier::compatible_dominant_frequency(
                model.version,
                features.dominant_freq_hz,
                amps,
            );
            let feat_arr = crate::adaptive_classifier::features_from_runtime(
                &serde_json::json!({
                    "variance": features.variance,
                    "motion_band_power": features.motion_band_power,
                    "breathing_band_power": features.breathing_band_power,
                    "spectral_power": features.spectral_power,
                    "dominant_freq_hz": dominant_frequency,
                    "change_points": features.change_points,
                    "mean_rssi": features.mean_rssi,
                }),
                amps,
            );
            let (label, conf) = model.classify(&feat_arr);
            classification.motion_level = label.to_string();
            classification.presence = label != "absent";
            classification.confidence = (conf * 0.7 + classification.confidence * 0.3).clamp(0.0, 1.0);
        }
    }

    pub fn debounce_room_classification(
        &mut self,
        raw: &wifi_densepose_sensing_server::inference::RoomInference,
    ) -> wifi_densepose_sensing_server::domain::ClassificationInfo {
        wifi_densepose_sensing_server::domain::debounce_room_classification_state(
            &mut self.room_debounced_level,
            &mut self.room_debounce_candidate,
            &mut self.room_debounce_since,
            raw,
            std::time::Instant::now(),
        )
    }

    pub fn emit_rufield_event(
        &self,
        update: &wifi_densepose_sensing_server::domain::SensingUpdate,
        node_id: u8,
    ) {
        use wifi_densepose_sensing_server::rufield_surface;

        if !update.classification.presence {
            return;
        }
        let Some(effective_class) = self.engine_bridge.effective_class() else {
            return;
        };

        let timestamp_ns = if update.timestamp.is_finite() && update.timestamp > 0.0 {
            (update.timestamp * 1_000_000_000.0) as u64
        } else {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0)
        };

        let snap = rufield_surface::build_snapshot(
            timestamp_ns,
            format!("esp32_node_{node_id}"),
            rufield_surface::SensingFeatures {
                mean_rssi: update.features.mean_rssi,
                variance: update.features.variance,
                motion_band_power: update.features.motion_band_power,
                breathing_band_power: update.features.breathing_band_power,
                dominant_freq_hz: update.features.dominant_freq_hz,
                change_points: update.features.change_points,
                spectral_power: update.features.spectral_power,
            },
            rufield_surface::SensingClass {
                motion_level: update.classification.motion_level.clone(),
                presence: update.classification.presence,
                confidence: update.classification.confidence,
            },
            Some(rufield_surface::SignalField {
                grid_size: update.signal_field.grid_size,
                values: update.signal_field.values.clone(),
            }),
            rufield_surface::ruview_class_from_bfld(effective_class),
            self.engine_bridge.demoted(),
            false,
        );

        if let Ok(mut fs) = self.field_surface.try_write() {
            fs.emit(&snap);
        }
    }
}

pub fn smooth_and_classify(
    state: &mut AppStateInner,
    raw: &mut wifi_densepose_sensing_server::domain::ClassificationInfo,
    raw_motion: f64,
) {
    state.smooth_and_classify(raw, raw_motion);
}

pub fn smooth_vitals(
    state: &mut AppStateInner,
    raw: &wifi_densepose_sensing_server::vital_signs::VitalSigns,
) -> wifi_densepose_sensing_server::vital_signs::VitalSigns {
    state.smooth_vitals(raw)
}

pub fn adaptive_override(
    state: &AppStateInner,
    features: &wifi_densepose_sensing_server::domain::FeatureInfo,
    classification: &mut wifi_densepose_sensing_server::domain::ClassificationInfo,
) {
    state.adaptive_override(features, classification);
}

pub fn aggregate_person_count(
    activity_count: usize,
    node_states: &std::collections::HashMap<u8, wifi_densepose_sensing_server::domain::node::NodeState>,
    now: Instant,
) -> usize {
    let fresh_counts = node_states
        .values()
        .filter(|n| wifi_densepose_sensing_server::domain::node::node_is_fresh(n, now))
        .map(|n| n.prev_person_count);
    wifi_densepose_sensing_server::domain::fusion::aggregate_person_count(activity_count, fresh_counts)
}

pub fn update_room_person_count(
    state: &mut AppStateInner,
    classification: &wifi_densepose_sensing_server::domain::ClassificationInfo,
    now: Instant,
    observed_at_unix_ms: u64,
) -> usize {
    state.update_room_person_count(classification, now, observed_at_unix_ms)
}

pub fn emit_rufield_event(
    s: &AppStateInner,
    update: &wifi_densepose_sensing_server::domain::SensingUpdate,
    node_id: u8,
) {
    s.emit_rufield_event(update, node_id);
}
