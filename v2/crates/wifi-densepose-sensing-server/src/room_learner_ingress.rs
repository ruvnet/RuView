//! Header-only readiness is keyed to the exact PHY class admitted by the learner.
use super::*;
use std::collections::VecDeque;
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct NativeSourceKey {
    pub(super) node_id: u8,
    pub(super) n_antennas: u8,
    pub(super) raw_bins: u16,
    pub(super) ppdu_type: u8,
    pub(super) frequency_mhz: u16,
    pub(super) bw40: bool,
}
impl NativeSourceKey {
    fn from_frame(frame: &Esp32Frame) -> Self {
        Self {
            node_id: frame.node_id,
            n_antennas: frame.n_antennas,
            raw_bins: frame.n_subcarriers,
            ppdu_type: frame.ppdu_type.to_byte(),
            frequency_mhz: frame.freq_mhz,
            bw40: frame.adr018_flags.bw40,
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct LiveSource {
    pub(super) frequency_hz: u64,
    pub(super) bandwidth_hz: Option<u64>,
    pub(super) receiver_rate_hz: f64,
    arrivals: VecDeque<std::time::Instant>,
}
impl LiveSource {
    fn record(&mut self, now: std::time::Instant) {
        while self
            .arrivals
            .front()
            .is_some_and(|t| now.saturating_duration_since(*t) > CALIBRATION_GRID_EVIDENCE_WINDOW)
        {
            self.arrivals.pop_front();
        }
        if self.arrivals.len() == CALIBRATION_GRID_EVIDENCE_CAPACITY {
            self.arrivals.pop_front();
        }
        if self.arrivals.back().is_none_or(|t| now > *t) {
            self.arrivals.push_back(now);
        }
        self.receiver_rate_hz = self.evidence(now).map_or(0.0, |e| e.rate_hz);
    }
    fn evidence(&self, now: std::time::Instant) -> Option<CalibrationGridEvidence> {
        let arrivals: Vec<_> = self
            .arrivals
            .iter()
            .copied()
            .filter(|t| now.saturating_duration_since(*t) <= CALIBRATION_GRID_EVIDENCE_WINDOW)
            .collect();
        CalibrationGridEvidence::from_arrivals(&arrivals, now)
    }
    fn eligible(&self, now: std::time::Instant) -> bool {
        self.bandwidth_hz == Some(20_000_000)
            && native::is_supported_frequency_hz(self.frequency_hz)
            && self.evidence(now).is_some_and(|e| {
                e.span_s >= CALIBRATION_GRID_MIN_SPAN_S
                    && e.rate_hz >= CALIBRATION_GRID_MIN_RATE_HZ
                    && e.max_gap_s < CALIBRATION_GRID_MAX_GAP_S
                    && e.latest_age_s < 2.0
            })
            && self
                .arrivals
                .iter()
                .rev()
                .nth(wifi_densepose_sensing_server::room_learner::capture::WINDOW_PACKETS - 1)
                .is_some_and(|t| {
                    now.saturating_duration_since(*t).as_nanos()
                        <= u128::from(
                            wifi_densepose_sensing_server::room_learner::observation::MAX_AGE_NS,
                        )
                })
    }
}
pub(super) fn native_candidates(
    state: &AppStateInner,
    node: u8,
    now: std::time::Instant,
) -> Vec<(&NativeSourceKey, &LiveSource)> {
    state
        .room_learning_sources
        .iter()
        .filter(|(key, meta)| {
            key.node_id == node
                && meta.eligible(now)
                && native::grid_id(
                    key.n_antennas as usize,
                    key.raw_bins as usize,
                    wifi_densepose_hardware::PpduType::from_byte(key.ppdu_type),
                )
                .is_ok()
        })
        .collect()
}
pub(crate) fn observe(
    state: &mut AppStateInner,
    frame: &Esp32Frame,
    packet: &[u8],
    now: std::time::Instant,
) {
    let Some(runtime) = state.room_learning.clone() else {
        return;
    };
    // Unsupported input must never consume the bounded readiness cache. Keep
    // the selected-source rejection diagnostic without admitting any evidence.
    let Ok(id) = native::grid_id(
        frame.n_antennas as usize,
        frame.n_subcarriers as usize,
        frame.ppdu_type,
    ) else {
        return;
    };
    let frequency_hz = u64::from(frame.freq_mhz) * 1_000_000;
    let rejection = if frame.adr018_flags.bw40 {
        Some("native capture does not support the reported 40 MHz bandwidth")
    } else if !native::is_supported_frequency_hz(frequency_hz) {
        Some("unknown or unsupported ESP32 centre frequency")
    } else {
        None
    };
    if let Some(reason) = rejection {
        runtime.record_native_rejection(frame.node_id, &id, reason);
        return;
    }
    let bytes = usize::from(frame.n_antennas) * usize::from(frame.n_subcarriers) * 2;
    let Some(iq) = packet.get(20..20 + bytes) else {
        return;
    };
    let key = NativeSourceKey::from_frame(frame);
    if state.room_learning_sources.len() >= 1024 && !state.room_learning_sources.contains_key(&key)
    {
        // Reclaim only expired unrelated classes. Selected contexts keep their
        // identity, but eligible() still expires their readiness by receiver
        // time; retaining a key cannot manufacture a fresh source window.
        let context = runtime.context();
        state.room_learning_sources.retain(|key, meta| {
            let fresh = meta.arrivals.back().is_some_and(|last| {
                now.saturating_duration_since(*last) <= CALIBRATION_GRID_EVIDENCE_WINDOW
            });
            fresh
                || context.as_ref().is_some_and(|context| {
                    context.sources.iter().any(|source| {
                        source.source_id == key.node_id
                            && source.center_frequency_hz == meta.frequency_hz
                            && Some(source.bandwidth_hz) == meta.bandwidth_hz
                            && native::grid_id(
                                key.n_antennas as usize,
                                key.raw_bins as usize,
                                wifi_densepose_hardware::PpduType::from_byte(key.ppdu_type),
                            )
                            .ok()
                            .as_deref()
                                == Some(source.grid_id.as_str())
                    })
                })
        });
        if state.room_learning_sources.len() >= 1024 {
            return;
        }
    }
    let meta = state
        .room_learning_sources
        .entry(key)
        .or_insert_with(|| LiveSource {
            frequency_hz,
            bandwidth_hz: Some(20_000_000),
            receiver_rate_hz: 0.0,
            arrivals: VecDeque::new(),
        });
    meta.record(now);
    let rate = meta.receiver_rate_hz;
    if !runtime.selects_grid(frame.node_id, &id) {
        return;
    }
    let ns = runtime.receiver_now_ns();
    let projected = match native::canonicalize_esp32(NativePacketInput {
        node_id: frame.node_id,
        n_antennas: frame.n_antennas as usize,
        raw_bins: frame.n_subcarriers as usize,
        frequency_hz,
        ppdu_type: frame.ppdu_type,
        sequence: frame.sequence,
        received_at_ns: ns,
        receiver_sample_rate_hz: rate,
        iq_bytes: iq,
        flags: frame.adr018_flags,
        provenance: CaptureProvenance::LiveReceiverObserved,
    }) {
        Ok(p) => p,
        Err(reason) => {
            runtime.record_native_rejection(frame.node_id, &id, &reason);
            return;
        }
    };
    let _ = runtime.ingest_native(&projected, ns);
}
