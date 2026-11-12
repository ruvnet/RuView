//! Header-only readiness is keyed to the exact PHY class admitted by the learner.
use super::*;
use std::collections::VecDeque;

/// Receiver-only pipeline coordinates. Deliberately not Serialize and never an admission clock.
#[derive(Clone, Copy)]
pub(crate) struct ReceiverIngressTiming {
    dequeued_at: std::time::Instant,
    parse_us: Option<u32>,
    lock_us: Option<u32>,
}
fn diagnostic_micros(duration: Option<std::time::Duration>) -> Option<u32> {
    duration.and_then(|d| d.as_micros().try_into().ok())
}
impl ReceiverIngressTiming {
    pub(crate) fn new(dequeued_at: std::time::Instant, parse: Option<std::time::Duration>, lock_wait: Option<std::time::Duration>) -> Self {
        Self { dequeued_at, parse_us: diagnostic_micros(parse), lock_us: diagnostic_micros(lock_wait) }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub(crate) struct NativeSourceKey {
    pub(super) node_id: u8,
    pub(super) n_antennas: u8,
    pub(super) raw_bins: u16,
    pub(super) ppdu_type: u8,
    pub(super) frequency_mhz: u16,
    pub(super) bw40: bool,
    pub(super) format: native::NativeFormatId,
}
impl NativeSourceKey {
    fn from_frame(frame: &Esp32Frame, format: native::NativeFormatId) -> Self {
        Self {
            node_id: frame.node_id,
            n_antennas: frame.n_antennas,
            raw_bins: frame.n_subcarriers,
            ppdu_type: frame.ppdu_type.to_byte(),
            frequency_mhz: frame.freq_mhz,
            bw40: frame.adr018_flags.bw40,
            format,
        }
    }
    pub(super) fn grid_id(&self) -> Result<String, String> {
        native::grid_id_for_format(self.n_antennas as usize, self.raw_bins as usize,
            wifi_densepose_hardware::PpduType::from_byte(self.ppdu_type), self.format)
    }
}

/// Last rejected packet metadata only, never readiness or model authority.
/// u8 node IDs bound storage to 256 entries; no raw payload/absolute timestamps.
#[derive(Debug, Clone)]
pub(crate) struct NativePacketRejection {
    code: &'static str,
    detail: &'static str,
    raw_bins: u16,
    ppdu_type: u8,
    frequency_mhz: u16,
    observed_at: std::time::Instant,
}
const NATIVE_REJECTION_MAX_AGE: std::time::Duration = std::time::Duration::from_secs(5);
const MAX_REPORTED_NATIVE_REJECTIONS: usize = 32;
fn reject_packet(state: &mut AppStateInner, frame: &Esp32Frame, now: std::time::Instant,
                 code: &'static str, detail: &'static str) {
    state.room_learning_rejections.retain(|_, d|
        now.saturating_duration_since(d.observed_at) <= NATIVE_REJECTION_MAX_AGE);
    state.room_learning_rejections.insert(frame.node_id, NativePacketRejection {
        code, detail, raw_bins: frame.n_subcarriers, ppdu_type: frame.ppdu_type.to_byte(),
        frequency_mhz: frame.freq_mhz, observed_at: now,
    });
}
pub(super) fn source_rejection_diagnostics(state: &AppStateInner, now: std::time::Instant)
    -> serde_json::Value {
    let selected=state.room_learning.as_ref().and_then(|rt|rt.context())
        .map(|context|context.sources.into_iter().map(|source|source.source_id).collect::<Vec<_>>()).unwrap_or_default();
    let mut ids: Vec<_> = state.room_learning_rejections.iter().filter_map(|(&id,d)|
        (now.saturating_duration_since(d.observed_at)<=NATIVE_REJECTION_MAX_AGE).then_some(id)).collect();
    ids.sort_unstable_by_key(|id|(!selected.contains(id),*id));
    let total=ids.len();
    let values: Vec<_> = ids.into_iter().take(MAX_REPORTED_NATIVE_REJECTIONS).filter_map(|node| {
        let d = &state.room_learning_rejections[&node];
        let age = now.saturating_duration_since(d.observed_at);
        (age <= NATIVE_REJECTION_MAX_AGE).then(|| serde_json::json!({
            "node_id":node,"last_rejected_packet":{"code":d.code,"detail":d.detail,
            "raw_bins":d.raw_bins,"ppdu_type":d.ppdu_type,"frequency_mhz":d.frequency_mhz,
            "age_ms":age.as_millis() as u64},"admission_authority":false
        }))
    }).collect();
    serde_json::json!({"schema":"ruview.native-packet-rejections.v1","sources":values,
        "total_sources":total,"truncated":total>MAX_REPORTED_NATIVE_REJECTIONS})
}
#[derive(Debug, Clone)]
pub(crate) struct LiveSource {
    pub(super) frequency_hz: u64,
    pub(super) bandwidth_hz: Option<u64>,
    pub(super) receiver_rate_hz: f64,
    arrivals: VecDeque<std::time::Instant>,
    // Separate from arrivals/readiness/model input; one relative diagnostic predecessor.
    last_dequeued: Option<std::time::Instant>,
}
impl LiveSource {
    fn diagnostic_dequeue_interval(&mut self, at: std::time::Instant) -> Option<u32> {
        let previous = self.last_dequeued.replace(at);
        diagnostic_micros(previous.and_then(|before| at.checked_duration_since(before)))
    }
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
                && key.grid_id().is_ok()
        })
        .collect()
}
pub(crate) fn observe(
    state: &mut AppStateInner,
    frame: &Esp32Frame,
    packet: &[u8],
    now: std::time::Instant,
) {
    observe_with_receiver_timing(state, frame, packet, now, None);
}
pub(crate) fn observe_with_receiver_timing(
    state: &mut AppStateInner,
    frame: &Esp32Frame,
    packet: &[u8],
    now: std::time::Instant,
    receiver_timing: Option<ReceiverIngressTiming>,
) {
    let Some(runtime) = state.room_learning.clone() else {
        return;
    };
    // Unsupported input must never consume the bounded readiness cache. Keep
    // the selected-source rejection diagnostic without admitting any evidence.
    let format = match native::validate_adr018_native_format(packet,
        frame.n_antennas as usize, frame.n_subcarriers as usize,
        frame.ppdu_type, frame.adr018_flags) {
        Ok(format) => format,
        Err(detail) => {
            reject_packet(state, frame, now, "native_format_unverified", detail);
            return;
        }
    };
    let key = NativeSourceKey::from_frame(frame, format.id());
    let Ok(id) = key.grid_id() else {
        reject_packet(state, frame, now, "native_grid_unsupported", "The declared native grid/PPDU is unsupported.");
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
        reject_packet(state, frame, now, "native_phy_unsupported", reason);
        runtime.record_native_rejection(frame.node_id, &id, reason);
        return;
    }
    let bytes = usize::from(frame.n_antennas) * usize::from(frame.n_subcarriers) * 2;
    let Some(iq) = packet.get(20..20 + bytes) else {
        return;
    };
    if state.room_learning_rejections.get(&frame.node_id).is_some_and(|d|
        d.raw_bins == frame.n_subcarriers && d.ppdu_type == frame.ppdu_type.to_byte()
            && d.frequency_mhz == frame.freq_mhz) {
        state.room_learning_rejections.remove(&frame.node_id);
    }
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
                            && key.grid_id()
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
            last_dequeued: None,
        });
    meta.record(now);
    let rate = meta.receiver_rate_hz;
    if !runtime.selects_grid(frame.node_id, &id) {
        return;
    }
    let dequeue_interval_us = receiver_timing.and_then(|t| meta.diagnostic_dequeue_interval(t.dequeued_at));
    let native_stamp_at = receiver_timing.map(|_| std::time::Instant::now());
    let ns = runtime.receiver_now_ns();
    let projected = match native::canonicalize_esp32_with_format(NativePacketInput {
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
    }, format) {
        Ok(p) => p,
        Err(reason) => {
            runtime.record_native_rejection(frame.node_id, &id, &reason);
            return;
        }
    };
    let timing = receiver_timing.map(|t| {
        wifi_densepose_sensing_server::room_learner::lifecycle::ReceiverPipelineTiming::for_packet(
            &projected, dequeue_interval_us, t.parse_us, t.lock_us,
            diagnostic_micros(native_stamp_at.and_then(|at| at.checked_duration_since(t.dequeued_at))))
    });
    let _ = runtime.ingest_native_with_receiver_timing(&projected, ns, timing);
}


#[cfg(test)]
mod receiver_timing_tests {
    use super::*;
    use std::time::{Duration, Instant};
    fn source() -> LiveSource { LiveSource { frequency_hz:2_437_000_000, bandwidth_hz:Some(20_000_000), receiver_rate_hz:33.0, arrivals:VecDeque::new(), last_dequeued:None } }
    #[test]
    fn diagnostic_cache_is_separate_from_readiness_and_exact_phy_keys_do_not_mix() {
        let t = Instant::now();
        let ht_key = NativeSourceKey { node_id:15, n_antennas:1, raw_bins:64, ppdu_type:0, frequency_mhz:2437, bw40:false, format:native::NativeFormatId::LegacyV2 };
        let he_key = NativeSourceKey { raw_bins:256, ppdu_type:1, ..ht_key };
        let mut cache = std::collections::HashMap::from([(ht_key, source()), (he_key, source())]);
        let ht = cache.get_mut(&ht_key).unwrap(); ht.record(t);
        let prior = (ht.arrivals.len(), ht.receiver_rate_hz, ht.eligible(t));
        assert_eq!(ht.diagnostic_dequeue_interval(t), None);
        assert_eq!(ht.diagnostic_dequeue_interval(t + Duration::from_micros(30_303)), Some(30_303));
        assert_eq!((ht.arrivals.len(), ht.receiver_rate_hz, ht.eligible(t)), prior);
        let he = cache.get_mut(&he_key).unwrap();
        assert_eq!(he.diagnostic_dequeue_interval(t + Duration::from_micros(30_304)), None);
    }
    #[test]
    fn invalid_or_oversized_durations_stay_unknown_instead_of_clipping() {
        assert_eq!(diagnostic_micros(None), None);
        assert_eq!(diagnostic_micros(Some(Duration::from_micros(u64::from(u32::MAX) + 1))), None);
        assert_eq!(diagnostic_micros(Some(Duration::from_micros(12))), Some(12));
        let t = Instant::now(); let mut s = source();
        s.diagnostic_dequeue_interval(t + Duration::from_millis(1));
        assert_eq!(s.diagnostic_dequeue_interval(t), None);
    }
    #[test]
    fn synthetic_post_dequeue_delay_does_not_become_a_radio_timestamp() {
        let t = Instant::now();
        let timing = ReceiverIngressTiming::new(t, Some(Duration::from_micros(20)), Some(Duration::from_millis(60)));
        assert_eq!(timing.parse_us, Some(20)); assert_eq!(timing.lock_us, Some(60_000));
        let before_native_stamp = t + Duration::from_micros(60_025);
        assert_eq!(diagnostic_micros(before_native_stamp.checked_duration_since(timing.dequeued_at)), Some(60_025));
    }
}
