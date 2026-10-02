//! Per-device ring buffer of full per-subcarrier MediaTek CSI, for
//! per-device channel inspection and analysis through the
//! `/api/v1/csi/mediatek/devices/:device_id/{frames,summary}` routes.
//!
//! `MediatekCsiSnapshot` (`mediatek_csi.rs`) deliberately keeps only
//! mean/peak amplitude — that's enough for the heuristic classifier
//! (`mediatek_heuristic`) but nothing for a human to actually look at the
//! channel with. This module extracts full per-chain, per-subcarrier
//! amplitude and phase directly from each frame's raw MTC1 I/Q payload at
//! ingest, and retains the last `capacity` frames per device — bounded, so
//! memory stays fixed regardless of how long a device has been streaming.
//!
//! # Memory bound
//!
//! `ring_size * chains * subcarrier_count * 2 (amplitude + phase) * 4 bytes (f32)`.
//! At the defaults this ADR-267 targets (N=256 frames, 2x2 MIMO = 4 chains,
//! 64 subcarriers): `256 * 4 * 64 * 2 * 4 = 524,288 bytes ≈ 0.5 MiB` per
//! device (plus a small, comparatively negligible per-frame Vec<i8> for
//! `rssi_dbm`, one byte per Rx chain).

use crate::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Json};
use serde::Deserialize;
use std::time::Duration;
use wifi_densepose_hardware::mediatek_csi::{CsiFrame, CsiPayload};

/// One (tx, rx) chain's per-subcarrier amplitude and phase for one frame.
#[derive(Debug, Clone)]
pub(crate) struct ChainSamples {
    pub(crate) tx: u8,
    pub(crate) rx: u8,
    /// Real units (`hypot(i, q) * frame.scale`), one per subcarrier.
    pub(crate) amplitude: Vec<f32>,
    /// Radians, one per subcarrier, unwrapped along the subcarrier axis
    /// within this frame only (see [`unwrap_phase`]) — never carried across
    /// frames or across chains.
    pub(crate) phase: Vec<f32>,
}

/// One retained frame: everything a client needs to plot or replay it.
#[derive(Debug, Clone)]
pub(crate) struct RingFrame {
    pub(crate) timestamp_us: u64,
    pub(crate) sequence: u32,
    /// One signed RSSI byte per Rx chain (ADR-267) — indexed by Rx chain
    /// only, NOT parallel to `chains` (which has one entry per Tx/Rx pair).
    pub(crate) rssi_dbm: Vec<i8>,
    pub(crate) chains: Vec<ChainSamples>,
}

/// Bounded per-device ring buffer of [`RingFrame`]s, oldest first.
#[derive(Debug, Clone)]
pub(crate) struct DeviceRing {
    capacity: usize,
    frames: std::collections::VecDeque<RingFrame>,
    subcarrier_count: u16,
}

impl DeviceRing {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            frames: std::collections::VecDeque::new(),
            subcarrier_count: 0,
        }
    }

    /// Extract per-chain amplitude/phase from `frame`'s raw I/Q payload and
    /// push it into the ring, evicting the oldest frame once over capacity.
    /// A `CsiPayload::Bytes` report (e.g. an ADR-267 capabilities frame,
    /// which carries no I/Q) is not retained.
    pub(crate) fn push(&mut self, frame: &CsiFrame) {
        let tx = frame.tx_count as usize;
        let rx = frame.rx_count as usize;
        let sc = frame.subcarrier_count as usize;
        let chains = match &frame.payload {
            CsiPayload::ComplexI16 { values, .. } => extract_chains(
                values.iter().map(|[i, q]| (*i as f32, *q as f32)),
                tx,
                rx,
                sc,
                frame.scale,
            ),
            CsiPayload::ComplexF32 { values, .. } => extract_chains(
                values.iter().map(|[i, q]| (*i, *q)),
                tx,
                rx,
                sc,
                frame.scale,
            ),
            CsiPayload::Bytes(_) => return,
        };
        self.subcarrier_count = frame.subcarrier_count;
        self.frames.push_back(RingFrame {
            timestamp_us: frame.timestamp_us,
            sequence: frame.sequence,
            rssi_dbm: frame.payload.rssi_dbm().to_vec(),
            chains,
        });
        while self.frames.len() > self.capacity {
            self.frames.pop_front();
        }
    }

    pub(crate) fn len(&self) -> usize {
        self.frames.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    pub(crate) fn subcarrier_count(&self) -> u16 {
        self.subcarrier_count
    }

    /// Oldest first.
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &RingFrame> {
        self.frames.iter()
    }

    pub(crate) fn last(&self) -> Option<&RingFrame> {
        self.frames.back()
    }

    /// Real elapsed time covered by the ring's retained frames (0 with
    /// fewer than 2 frames — there's no interval to measure yet).
    pub(crate) fn span_us(&self) -> u64 {
        match (self.frames.front(), self.frames.back()) {
            (Some(oldest), Some(newest)) if self.frames.len() > 1 => {
                newest.timestamp_us.saturating_sub(oldest.timestamp_us)
            }
            _ => 0,
        }
    }

    /// Frames per second implied by `span_us()` and the retained frame
    /// count. `0.0` with fewer than 2 frames.
    pub(crate) fn frame_rate_hz(&self) -> f64 {
        let span_us = self.span_us();
        if self.frames.len() < 2 || span_us == 0 {
            return 0.0;
        }
        (self.frames.len() - 1) as f64 / (span_us as f64 / 1_000_000.0)
    }

    /// "Fast" activity signal for `mediatek_activity`: the mean, over every
    /// `(chain, subcarrier)` pair observed in the last `window` of ring
    /// time (by frame timestamp, relative to the newest retained frame —
    /// not wall-clock `Instant::now()`), of that pair's coefficient of
    /// variation (`std / mean`) — a gain-normalized measure of how much the
    /// channel is fluctuating right now. Returns `(fast, frame_count)`;
    /// `frame_count` is also the module's `frames_2s`-style weight. `(0.0,
    /// n)` when fewer than 2 frames fall in the window (a std needs at
    /// least 2 samples).
    pub(crate) fn fast_activity(&self, window: Duration) -> (f64, usize) {
        let Some(newest) = self.frames.back() else {
            return (0.0, 0);
        };
        let window_us = window.as_micros() as u64;
        let recent: Vec<&RingFrame> = self
            .frames
            .iter()
            .rev()
            .take_while(|f| newest.timestamp_us.saturating_sub(f.timestamp_us) <= window_us)
            .collect();
        let n = recent.len();
        if n < 2 {
            return (0.0, n);
        }
        let sc = self.subcarrier_count as usize;
        let mut cv_sum = 0.0f64;
        let mut cv_count = 0usize;
        for (tx, rx) in chain_union(recent.iter().copied()) {
            let mut sum = vec![0.0f64; sc];
            let mut sumsq = vec![0.0f64; sc];
            let mut m = 0usize;
            for frame in &recent {
                let Some(c) = frame.chains.iter().find(|c| c.tx == tx && c.rx == rx) else {
                    continue;
                };
                m += 1;
                for (k, &a) in c.amplitude.iter().enumerate().take(sc) {
                    sum[k] += a as f64;
                    sumsq[k] += (a as f64) * (a as f64);
                }
            }
            if m < 2 {
                continue;
            }
            let m_f = m as f64;
            for k in 0..sc {
                let mean = sum[k] / m_f;
                let variance = (sumsq[k] / m_f - mean * mean).max(0.0);
                cv_sum += variance.sqrt() / mean.abs().max(1e-6);
                cv_count += 1;
            }
        }
        let fast = if cv_count > 0 {
            cv_sum / cv_count as f64
        } else {
            0.0
        };
        (fast, n)
    }

    /// Per-chain amplitude mean and standard deviation across every
    /// retained frame that carried that chain, per subcarrier — the "what
    /// does the channel usually look like" view for an inspection client.
    ///
    /// 2026-09-20: this used to take chain layout from only the newest
    /// frame and match older frames to it positionally (by index into that
    /// frame's chain list). A device that interleaves PPDU types with
    /// different dimensions (e.g. legacy 1x2 alongside HT 2x2) legitimately
    /// varies chain count frame to frame, so that silently dropped whichever
    /// chains the newest frame didn't happen to carry, and matched other
    /// frames' chains by position rather than identity. Chain identity here
    /// is now the layout: this returns one entry per **distinct (tx, rx)
    /// pair seen anywhere in the ring**, in first-seen order, each averaged
    /// only over the frames that actually carried it (`frames_with_chain`
    /// says how many, out of `self.len()`).
    pub(crate) fn amplitude_stats(&self) -> Vec<ChainAmplitudeStats> {
        let sc = self.subcarrier_count as usize;
        chain_union(self.frames.iter())
            .into_iter()
            .map(|(tx, rx)| {
                let mut sum = vec![0.0f64; sc];
                let mut sumsq = vec![0.0f64; sc];
                let mut n = 0usize;
                for frame in &self.frames {
                    let Some(c) = frame.chains.iter().find(|c| c.tx == tx && c.rx == rx) else {
                        continue;
                    };
                    n += 1;
                    for (k, &a) in c.amplitude.iter().enumerate().take(sc) {
                        sum[k] += a as f64;
                        sumsq[k] += (a as f64) * (a as f64);
                    }
                }
                let n_f = n.max(1) as f64;
                let mean: Vec<f32> = sum.iter().map(|&s| (s / n_f) as f32).collect();
                let std: Vec<f32> = sum
                    .iter()
                    .zip(sumsq.iter())
                    .map(|(&s, &sq)| {
                        let m = s / n_f;
                        (sq / n_f - m * m).max(0.0).sqrt() as f32
                    })
                    .collect();
                ChainAmplitudeStats {
                    tx,
                    rx,
                    amplitude_mean: mean,
                    amplitude_std: std,
                    frames_with_chain: n,
                }
            })
            .collect()
    }

    /// Mean RSSI per Rx chain across every retained frame.
    pub(crate) fn rssi_by_rx_chain(&self) -> Vec<(u8, f64)> {
        let n_rx = self.frames.back().map(|f| f.rssi_dbm.len()).unwrap_or(0);
        (0..n_rx)
            .map(|rx| {
                let (sum, n) = self
                    .frames
                    .iter()
                    .filter_map(|f| f.rssi_dbm.get(rx))
                    .fold((0i64, 0u32), |(s, n), &v| (s + v as i64, n + 1));
                let mean = if n > 0 { sum as f64 / n as f64 } else { 0.0 };
                (rx as u8, mean)
            })
            .collect()
    }
}

pub(crate) struct ChainAmplitudeStats {
    pub(crate) tx: u8,
    pub(crate) rx: u8,
    pub(crate) amplitude_mean: Vec<f32>,
    pub(crate) amplitude_std: Vec<f32>,
    /// How many of the averaged-over frames actually carried this chain —
    /// out of `self.len()` for a whole-ring summary, so the UI can show
    /// when a chain's stats are based on a partial subset (e.g. a legacy
    /// 1x2 frame interleaved with HT 2x2 ones only contributes to 2 of the
    /// 4 possible chains).
    pub(crate) frames_with_chain: usize,
}

/// Every distinct `(tx, rx)` pair present in any of `frames`, in first-seen
/// order. Used both for the whole-ring summary and for a windowed
/// `/frames` response's top-level `chains` field — "all chain positions
/// seen [in this scope]", since a per-frame layout can legitimately vary
/// (interleaved PPDU types with different dimensions).
fn chain_union<'a>(frames: impl Iterator<Item = &'a RingFrame>) -> Vec<(u8, u8)> {
    let mut seen = Vec::new();
    for frame in frames {
        for c in &frame.chains {
            if !seen.contains(&(c.tx, c.rx)) {
                seen.push((c.tx, c.rx));
            }
        }
    }
    seen
}

fn extract_chains(
    iq: impl Iterator<Item = (f32, f32)>,
    tx: usize,
    rx: usize,
    sc: usize,
    scale: f32,
) -> Vec<ChainSamples> {
    let values: Vec<(f32, f32)> = iq.collect();
    let mut chains = Vec::with_capacity(tx * rx);
    for t in 0..tx {
        for r in 0..rx {
            let base = (t * rx + r) * sc;
            let mut amplitude = Vec::with_capacity(sc);
            let mut phase = Vec::with_capacity(sc);
            for k in 0..sc {
                let idx = base + k;
                let Some(&(i, q)) = values.get(idx) else {
                    break;
                };
                amplitude.push(i.hypot(q) * scale);
                phase.push(q.atan2(i));
            }
            unwrap_phase(&mut phase);
            chains.push(ChainSamples {
                tx: t as u8,
                rx: r as u8,
                amplitude,
                phase,
            });
        }
    }
    chains
}

/// `numpy.unwrap`-style phase unwrapping along one frame's subcarrier
/// axis: whenever a consecutive step exceeds +/- pi, fold it back by the
/// nearest multiple of 2*pi so the sequence reads as a continuous phase
/// ramp across frequency instead of wrapping at +/- pi. Frame-local only —
/// never carries state across frames, and computed independently per chain.
fn unwrap_phase(phase: &mut [f32]) {
    const TWO_PI: f32 = std::f32::consts::TAU;
    for i in 1..phase.len() {
        let mut delta = phase[i] - phase[i - 1];
        while delta > std::f32::consts::PI {
            phase[i] -= TWO_PI;
            delta -= TWO_PI;
        }
        while delta < -std::f32::consts::PI {
            phase[i] += TWO_PI;
            delta += TWO_PI;
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct CsiFramesQuery {
    /// Frames to return, newest `n`. Capped at the ring's retained frame
    /// count (never an error to ask for more than exists).
    n: Option<usize>,
    /// Comma-separated subset of `"amp"`, `"phase"`. Both when omitted.
    fields: Option<String>,
}

fn wants(fields: &Option<String>, name: &str) -> bool {
    fields
        .as_deref()
        .is_none_or(|f| f.split(',').any(|x| x.trim() == name))
}

fn device_not_found(device_id: &str) -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": "unknown device_id", "device_id": device_id })),
    )
}

/// `GET /api/v1/csi/mediatek/devices/{device_id}/frames?n=64&fields=amp,phase`
///
/// `{device_id, subcarrier_count, chains: [{tx, rx}], frames: [{ts_us, seq,
/// rssi, chains: [{tx, rx}], amp: [[..per subcarrier..] per chain], phase:
/// [..]}]}`, oldest first within `frames`.
///
/// 2026-09-20: a 2x2 device legitimately interleaves PPDU types with
/// different dimensions (e.g. legacy 1x2 alongside HT 2x2), so a single
/// window can contain frames whose `amp`/`phase` row count differs. The
/// top-level `chains` is every chain position seen anywhere **in this
/// returned window** (not just the newest frame) — a discovery list, not an
/// index. Each frame additionally carries its **own** `chains` array,
/// naming exactly what its `amp[i]`/`phase[i]` rows are — a consumer must
/// index by a frame's own `chains`, never by the top-level one, since a
/// short frame's rows don't align with the top-level list's positions.
///
/// `404` when `device_id` has no ring at all (never reported, or
/// `mediatek_csi_by_device` only ever saw a Bytes/capabilities report).
/// `amp`/`phase` per frame are omitted (not `null`) when excluded by
/// `fields`.
pub(crate) async fn mediatek_csi_frames(
    State(state): State<SharedState>,
    Path(device_id): Path<String>,
    Query(query): Query<CsiFramesQuery>,
) -> impl IntoResponse {
    let s = state.read().await;
    let Some(ring) = s.mediatek_csi_ring_by_device.get(&device_id) else {
        return device_not_found(&device_id).into_response();
    };
    if ring.is_empty() {
        return device_not_found(&device_id).into_response();
    }
    let include_amp = wants(&query.fields, "amp");
    let include_phase = wants(&query.fields, "phase");
    let n = query.n.unwrap_or(64).min(ring.len());
    let skip = ring.len() - n;
    let window: Vec<&RingFrame> = ring.iter().skip(skip).collect();

    let frames: Vec<serde_json::Value> = window
        .iter()
        .map(|f| {
            let frame_chains: Vec<serde_json::Value> = f
                .chains
                .iter()
                .map(|c| serde_json::json!({ "tx": c.tx, "rx": c.rx }))
                .collect();
            let mut obj = serde_json::json!({
                "ts_us": f.timestamp_us,
                "seq": f.sequence,
                "rssi": f.rssi_dbm,
                "chains": frame_chains,
            });
            if let Some(map) = obj.as_object_mut() {
                if include_amp {
                    map.insert(
                        "amp".to_string(),
                        serde_json::json!(f
                            .chains
                            .iter()
                            .map(|c| &c.amplitude)
                            .collect::<Vec<_>>()),
                    );
                }
                if include_phase {
                    map.insert(
                        "phase".to_string(),
                        serde_json::json!(f.chains.iter().map(|c| &c.phase).collect::<Vec<_>>()),
                    );
                }
            }
            obj
        })
        .collect();

    let chains: Vec<serde_json::Value> = chain_union(window.into_iter())
        .into_iter()
        .map(|(tx, rx)| serde_json::json!({ "tx": tx, "rx": rx }))
        .collect();

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "device_id": device_id,
            "subcarrier_count": ring.subcarrier_count(),
            "chains": chains,
            "frames": frames,
        })),
    )
        .into_response()
}

/// `GET /api/v1/csi/mediatek/devices/{device_id}/summary` — one call for the
/// analysis view: per-subcarrier amplitude mean/std per chain
/// over the ring, mean RSSI per Rx chain, the ring's real frame rate, and
/// the heuristic classifier's current window stats for the same device.
/// `404` when `device_id` has no ring at all.
pub(crate) async fn mediatek_csi_summary(
    State(state): State<SharedState>,
    Path(device_id): Path<String>,
) -> impl IntoResponse {
    let mut s = state.write().await;
    let Some(ring) = s.mediatek_csi_ring_by_device.get(&device_id) else {
        return device_not_found(&device_id).into_response();
    };
    if ring.is_empty() {
        return device_not_found(&device_id).into_response();
    }

    let chains: Vec<serde_json::Value> = ring
        .amplitude_stats()
        .into_iter()
        .map(|c| {
            serde_json::json!({
                "tx": c.tx,
                "rx": c.rx,
                "amplitude_mean": c.amplitude_mean,
                "amplitude_std": c.amplitude_std,
                "frames_with_chain": c.frames_with_chain,
            })
        })
        .collect();
    let rssi_by_rx_chain: Vec<serde_json::Value> = ring
        .rssi_by_rx_chain()
        .into_iter()
        .map(|(rx, mean)| serde_json::json!({ "rx": rx, "mean_rssi_dbm": mean }))
        .collect();
    let ring_frame_count = ring.len();
    let ring_span_ms = ring.span_us() / 1000;
    let frame_rate_hz = ring.frame_rate_hz();
    let subcarrier_count = ring.subcarrier_count();

    // Separate borrow: the heuristic's history lives in a different map on
    // the same AppStateInner, and `verdict()` needs `&mut self` to prune by
    // age — done after the (now-dropped) immutable `ring` borrow above ends.
    let now = std::time::Instant::now();
    let heuristic = s
        .mediatek_heuristic_by_device
        .get_mut(&device_id)
        .map(|hist| hist.verdict(now))
        .map(|v| {
            serde_json::json!({
                "presence_state": v.presence_state.as_str(),
                "coefficient_of_variation": v.window.coefficient_of_variation,
                "baseline_deviation": v.window.baseline_deviation,
                "sample_count": v.window.sample_count,
                "span_ms": v.window.span_ms,
            })
        });

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "device_id": device_id,
            "subcarrier_count": subcarrier_count,
            "ring_frame_count": ring_frame_count,
            "ring_span_ms": ring_span_ms,
            "frame_rate_hz": frame_rate_hz,
            "rssi_by_rx_chain": rssi_by_rx_chain,
            "chains": chains,
            "heuristic": heuristic,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_hardware::mediatek_csi::simulator::{MediatekCsiSimulator, SimulatorConfig};

    #[test]
    fn ring_retains_frames_in_order_up_to_capacity() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut ring = DeviceRing::new(3);
        let seqs: Vec<u32> = (0..5)
            .map(|_| {
                let frame = sim.next_frame();
                ring.push(&frame);
                frame.sequence
            })
            .collect();
        assert_eq!(ring.len(), 3, "must evict down to capacity");
        let retained: Vec<u32> = ring.iter().map(|f| f.sequence).collect();
        assert_eq!(
            retained,
            seqs[2..],
            "oldest-first, only the last `capacity` frames survive"
        );
    }

    #[test]
    fn capabilities_frame_is_not_retained() {
        let sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut ring = DeviceRing::new(8);
        ring.push(&sim.capabilities_frame());
        assert!(
            ring.is_empty(),
            "a Bytes-payload capabilities frame carries no I/Q to retain"
        );
    }

    #[test]
    fn chain_layout_matches_tx_rx_dimensions() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig {
            tx_count: 2,
            rx_count: 3,
            subcarriers: 8,
            ..Default::default()
        })
        .unwrap();
        let mut ring = DeviceRing::new(4);
        ring.push(&sim.next_frame());
        let frame = ring.last().unwrap();
        assert_eq!(frame.chains.len(), 6, "2 tx x 3 rx = 6 chains");
        for chain in &frame.chains {
            assert_eq!(chain.amplitude.len(), 8);
            assert_eq!(chain.phase.len(), 8);
        }
        assert_eq!(
            frame.rssi_dbm.len(),
            3,
            "one RSSI byte per Rx chain, not per tx/rx pair"
        );
    }

    #[test]
    fn unwrap_removes_discontinuities_bigger_than_pi() {
        // A synthetic phase ramp that would wrap at +/- pi without unwrapping.
        let mut phase = vec![3.0, -3.0, 3.0, -3.0];
        unwrap_phase(&mut phase);
        for w in phase.windows(2) {
            assert!(
                (w[1] - w[0]).abs() < std::f32::consts::PI + 1e-3,
                "no residual jump > pi: {:?}",
                phase
            );
        }
    }

    #[test]
    fn amplitude_stats_and_frame_rate_are_sane_over_a_short_run() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig::default()).unwrap();
        let mut ring = DeviceRing::new(16);
        for _ in 0..10 {
            ring.push(&sim.next_frame());
        }
        let stats = ring.amplitude_stats();
        assert_eq!(
            stats.len(),
            2 * 3,
            "default simulator config is 2 tx x 3 rx"
        );
        for s in &stats {
            assert_eq!(s.amplitude_mean.len(), ring.subcarrier_count() as usize);
            assert_eq!(s.amplitude_std.len(), ring.subcarrier_count() as usize);
        }
        // Default simulator frame_period_us = 20_000 -> 10 frames span 9 periods.
        assert!(ring.frame_rate_hz() > 0.0);
        let rssi = ring.rssi_by_rx_chain();
        assert_eq!(rssi.len(), 3);
    }

    /// 2026-09-20 field bug: a 2x2 device interleaves legacy 1x2 and HT 2x2
    /// PPDUs, so per-frame chain count legitimately varies within one ring.
    /// `amplitude_stats` must union chain positions across every frame
    /// (not just the newest) and report how many frames actually backed
    /// each one.
    #[test]
    fn interleaved_1x2_and_2x2_frames_are_unioned_correctly() {
        let mut sim_1x2 = MediatekCsiSimulator::new(SimulatorConfig {
            tx_count: 1,
            rx_count: 2,
            subcarriers: 4,
            ..Default::default()
        })
        .unwrap();
        let mut sim_2x2 = MediatekCsiSimulator::new(SimulatorConfig {
            tx_count: 2,
            rx_count: 2,
            subcarriers: 4,
            ..Default::default()
        })
        .unwrap();
        let mut ring = DeviceRing::new(8);
        // Interleaved: 1x2, 2x2, 1x2, 2x2.
        ring.push(&sim_1x2.next_frame());
        ring.push(&sim_2x2.next_frame());
        ring.push(&sim_1x2.next_frame());
        ring.push(&sim_2x2.next_frame());
        assert_eq!(ring.len(), 4);

        let counts: Vec<usize> = ring.iter().map(|f| f.chains.len()).collect();
        assert_eq!(
            counts,
            vec![2, 4, 2, 4],
            "per-frame chain count must legitimately vary"
        );

        let stats = ring.amplitude_stats();
        assert_eq!(
            stats.len(),
            4,
            "union of the 1x2 frames' 2 chains and the 2x2 frames' 4 chains"
        );
        let by_pos: std::collections::HashMap<(u8, u8), usize> = stats
            .iter()
            .map(|s| ((s.tx, s.rx), s.frames_with_chain))
            .collect();
        assert_eq!(
            by_pos[&(0, 0)],
            4,
            "(0,0) exists in every frame, 1x2 and 2x2 alike"
        );
        assert_eq!(by_pos[&(0, 1)], 4);
        assert_eq!(
            by_pos[&(1, 0)],
            2,
            "(1,0) only exists in the two 2x2 frames"
        );
        assert_eq!(by_pos[&(1, 1)], 2);
    }

    #[test]
    fn fast_activity_is_zero_for_a_flat_signal_and_positive_for_a_varying_one() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig {
            tx_count: 1,
            rx_count: 1,
            subcarriers: 4,
            ..Default::default()
        })
        .unwrap();
        let mut flat_ring = DeviceRing::new(16);
        for _ in 0..8 {
            flat_ring.push(&sim.next_frame());
        }
        // The ADR-266 simulator's amplitude has a slow motion phase, not
        // frame-to-frame noise, so back-to-back frames read as effectively
        // flat over a short window: fast_activity should be near zero.
        let (fast_flat, n) = flat_ring.fast_activity(Duration::from_secs(2));
        assert_eq!(n, 8);
        assert!(
            fast_flat < 0.05,
            "expected a near-flat signal, got fast={fast_flat}"
        );
    }

    #[test]
    fn fast_activity_ignores_frames_outside_the_window() {
        let mut sim = MediatekCsiSimulator::new(SimulatorConfig {
            tx_count: 1,
            rx_count: 1,
            subcarriers: 4,
            frame_period_us: 500_000, // 0.5s apart
            ..Default::default()
        })
        .unwrap();
        let mut ring = DeviceRing::new(16);
        for _ in 0..8 {
            ring.push(&sim.next_frame()); // spans 3.5s total
        }
        let (_, n_full) = ring.fast_activity(Duration::from_secs(10));
        assert_eq!(n_full, 8, "a wide window covers every retained frame");
        let (_, n_narrow) = ring.fast_activity(Duration::from_secs(1));
        assert!(
            n_narrow < 8,
            "a 1s window must exclude older frames from a 3.5s-spanning ring, got {n_narrow}"
        );
    }

    mod endpoint_tests {
        //! Exercises `mediatek_csi_frames` / `mediatek_csi_summary` directly
        //! against `AppStateInner`, constructing the axum extractors by hand
        //! rather than routing a full `Request` through a `Router` — these
        //! handlers take no headers/body, so `Path`/`Query`/`State` cover
        //! everything a real request would provide.
        use super::*;
        use crate::AppStateInner;
        use axum::response::IntoResponse;
        use std::sync::Arc;
        use tokio::sync::RwLock;
        use wifi_densepose_hardware::mediatek_csi::simulator::{
            MediatekCsiSimulator, SimulatorConfig,
        };

        async fn body_json(
            response: axum::response::Response,
        ) -> (axum::http::StatusCode, serde_json::Value) {
            let status = response.status();
            let bytes = axum::body::to_bytes(response.into_body(), 1_000_000)
                .await
                .unwrap();
            (status, serde_json::from_slice(&bytes).unwrap())
        }

        fn seed_two_frames(s: &mut AppStateInner, device_id: &str) {
            let mut sim = MediatekCsiSimulator::new(SimulatorConfig {
                tx_count: 1,
                rx_count: 1,
                subcarriers: 4,
                ..Default::default()
            })
            .unwrap();
            let mut ring = DeviceRing::new(8);
            ring.push(&sim.next_frame());
            ring.push(&sim.next_frame());
            s.mediatek_csi_ring_by_device
                .insert(device_id.to_string(), ring);
        }

        #[tokio::test]
        async fn two_ingested_frames_return_in_order_with_correct_shapes() {
            let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
            seed_two_frames(&mut *state.write().await, "dev-a");

            let response = mediatek_csi_frames(
                State(state),
                Path("dev-a".to_string()),
                Query(CsiFramesQuery {
                    n: None,
                    fields: None,
                }),
            )
            .await
            .into_response();
            let (status, body) = body_json(response).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert_eq!(body["device_id"], "dev-a");
            assert_eq!(body["subcarrier_count"], 4);

            let frames = body["frames"].as_array().expect("frames array");
            assert_eq!(frames.len(), 2, "both ingested frames must be returned");
            // Oldest first: sequence 0 then 1 (MediatekCsiSimulator starts at 0).
            assert_eq!(frames[0]["seq"], 0);
            assert_eq!(frames[1]["seq"], 1);

            let chains = body["chains"].as_array().expect("chains array");
            assert_eq!(chains.len(), 1, "1 tx x 1 rx = 1 chain");

            let amp = frames[0]["amp"].as_array().expect("amp array");
            assert_eq!(amp.len(), 1, "one amplitude vector per chain");
            assert_eq!(
                amp[0].as_array().unwrap().len(),
                4,
                "one value per subcarrier"
            );
            let phase = frames[0]["phase"].as_array().expect("phase array");
            assert_eq!(phase[0].as_array().unwrap().len(), 4);
        }

        #[tokio::test]
        async fn fields_query_param_limits_which_arrays_are_returned() {
            let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
            seed_two_frames(&mut *state.write().await, "dev-a");

            let response = mediatek_csi_frames(
                State(state),
                Path("dev-a".to_string()),
                Query(CsiFramesQuery {
                    n: Some(1),
                    fields: Some("amp".to_string()),
                }),
            )
            .await
            .into_response();
            let (_, body) = body_json(response).await;
            let frames = body["frames"].as_array().unwrap();
            assert_eq!(frames.len(), 1, "n=1 must cap the returned frame count");
            assert!(frames[0].get("amp").is_some());
            assert!(
                frames[0].get("phase").is_none(),
                "phase must be omitted, not null, when excluded"
            );
        }

        #[tokio::test]
        async fn summary_matches_the_seeded_ring() {
            let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
            seed_two_frames(&mut *state.write().await, "dev-a");

            let response = mediatek_csi_summary(State(state), Path("dev-a".to_string()))
                .await
                .into_response();
            let (status, body) = body_json(response).await;
            assert_eq!(status, axum::http::StatusCode::OK);
            assert_eq!(body["device_id"], "dev-a");
            assert_eq!(body["subcarrier_count"], 4);
            assert_eq!(body["ring_frame_count"], 2);

            let chains = body["chains"].as_array().expect("chains array");
            assert_eq!(chains.len(), 1);
            assert_eq!(chains[0]["amplitude_mean"].as_array().unwrap().len(), 4);
            assert_eq!(chains[0]["amplitude_std"].as_array().unwrap().len(), 4);

            let rssi = body["rssi_by_rx_chain"]
                .as_array()
                .expect("rssi_by_rx_chain array");
            assert_eq!(rssi.len(), 1, "1 rx chain");

            // heuristic is present but null when this device never went
            // through mediatek_heuristic_by_device (this test only seeds the
            // ring, mirroring a device whose heuristic hasn't observed yet).
            assert!(body["heuristic"].is_null());
        }

        #[tokio::test]
        async fn unknown_device_returns_404_on_both_endpoints() {
            let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));

            let frames_resp = mediatek_csi_frames(
                State(state.clone()),
                Path("no-such-device".to_string()),
                Query(CsiFramesQuery {
                    n: None,
                    fields: None,
                }),
            )
            .await
            .into_response();
            assert_eq!(frames_resp.status(), axum::http::StatusCode::NOT_FOUND);

            let summary_resp =
                mediatek_csi_summary(State(state), Path("no-such-device".to_string()))
                    .await
                    .into_response();
            assert_eq!(summary_resp.status(), axum::http::StatusCode::NOT_FOUND);
        }

        /// 2026-09-20 field bug: a consumer indexing a short frame's
        /// `amp`/`phase` rows by the top-level `chains` list read undefined
        /// on legacy 1x2 frames interleaved with HT 2x2 ones. Each frame
        /// must carry its own `chains`, and the top-level list must be the
        /// union across the returned window, not just the newest frame.
        #[tokio::test]
        async fn interleaved_1x2_and_2x2_frames_get_correct_per_frame_chains() {
            let state: SharedState = Arc::new(RwLock::new(AppStateInner::minimal()));
            {
                let mut s = state.write().await;
                let mut sim_1x2 = MediatekCsiSimulator::new(SimulatorConfig {
                    tx_count: 1,
                    rx_count: 2,
                    subcarriers: 4,
                    ..Default::default()
                })
                .unwrap();
                let mut sim_2x2 = MediatekCsiSimulator::new(SimulatorConfig {
                    tx_count: 2,
                    rx_count: 2,
                    subcarriers: 4,
                    ..Default::default()
                })
                .unwrap();
                let mut ring = DeviceRing::new(8);
                ring.push(&sim_1x2.next_frame());
                ring.push(&sim_2x2.next_frame());
                s.mediatek_csi_ring_by_device
                    .insert("dev-mixed".to_string(), ring);
            }

            let response = mediatek_csi_frames(
                State(state),
                Path("dev-mixed".to_string()),
                Query(CsiFramesQuery {
                    n: None,
                    fields: None,
                }),
            )
            .await
            .into_response();
            let (status, body) = body_json(response).await;
            assert_eq!(status, axum::http::StatusCode::OK);

            let top_chains = body["chains"].as_array().unwrap();
            assert_eq!(
                top_chains.len(),
                4,
                "union across the window: the 1x2 frame's 2 plus the 2x2 frame's 2 new ones"
            );

            let frames = body["frames"].as_array().unwrap();
            assert_eq!(frames.len(), 2);

            let f0_chains = frames[0]["chains"].as_array().unwrap();
            let f0_amp = frames[0]["amp"].as_array().unwrap();
            assert_eq!(f0_chains.len(), 2, "first frame is 1x2 -> 2 chains");
            assert_eq!(
                f0_amp.len(),
                2,
                "amp rows must match THIS frame's own chains, not the top-level union"
            );

            let f1_chains = frames[1]["chains"].as_array().unwrap();
            let f1_amp = frames[1]["amp"].as_array().unwrap();
            assert_eq!(f1_chains.len(), 4, "second frame is 2x2 -> 4 chains");
            assert_eq!(f1_amp.len(), 4);
        }
    }
}
