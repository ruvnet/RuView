//! Guided occupancy-signal readiness for the live ESP32 calibration service.
//!
//! This is an operator-labelled signal-contrast check, not a signed capability
//! certificate or a measured occupancy accuracy claim. Raw CSI stays in memory.

use std::collections::VecDeque;

use serde::Serialize;
use wifi_densepose_core::types::CsiFrame;
use wifi_densepose_signal::BaselineCalibration;

const MIN_FRAMES: usize = 32;
const MIN_VERIFIED_FRAMES: usize = 600;
const MAX_FRAMES: usize = 4096;
const LIVE_WINDOW: usize = 64;
const STALE_MS: u64 = 2_000;
const MIN_CAPTURE_TIMEOUT_MS: u64 = 120_000;
// Site-dependent heuristics, not real-room accuracy thresholds. The relative
// contrast default is informed by the measured mean-amplitude signal in #1909.
const MAX_EMPTY_BASELINE_SHIFT: f32 = 0.10;
const MIN_RELATIVE_CONTRAST: f32 = 0.0075;
const NOISE_MULTIPLIER: f32 = 3.0;
const MIN_ABSOLUTE_CONTRAST: f32 = 0.01;

/// Operator-declared UDP source. `Live` means only that the service was started
/// in live mode; the unauthenticated ESP32 wire format cannot prove hardware
/// origin or rule out replay on the socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvidenceKind {
    Live,
    Replay,
    Synthetic,
}

impl EvidenceKind {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "live" => Some(Self::Live),
            "replay" => Some(Self::Replay),
            "synthetic" => Some(Self::Synthetic),
            _ => None,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Live => "live_udp_unverified",
            Self::Replay => "replay_udp_declared",
            Self::Synthetic => "synthetic_udp_declared",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Empty,
    Occupied,
}

impl Phase {
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "empty" => Some(Self::Empty),
            "occupied" => Some(Self::Occupied),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Readiness {
    pub target: &'static str,
    pub state: &'static str,
    pub evidence_source: &'static str,
    pub room_id: Option<String>,
    pub baseline_id: Option<String>,
    pub device_id: Option<String>,
    pub channel: Option<u8>,
    pub phase: Option<&'static str>,
    pub frames: usize,
    pub target_frames: usize,
    pub observed_empty_amp: Option<f32>,
    pub observed_occupied_amp: Option<f32>,
    pub signal_contrast_pct: Option<f32>,
    pub empty_frames: usize,
    pub occupied_frames: usize,
    pub last_frame_age_ms: Option<u64>,
    pub occupancy: Option<&'static str>,
    pub reason: &'static str,
    pub next_action: &'static str,
    pub note: &'static str,
}

struct Capture {
    phase: Phase,
    target: usize,
    scores: Vec<f32>,
    started_ms: u64,
}

#[derive(Debug, Clone, Copy)]
struct WindowStats {
    median_amp: f32,
    mad_amp: f32,
    frames: usize,
}

/// One UDP stream and one room at a time, matching `calibrate-serve`'s socket.
pub struct OccupancyReadiness {
    evidence: EvidenceKind,
    baseline: Option<BaselineCalibration>,
    room_id: Option<String>,
    baseline_id: Option<String>,
    device_id: Option<String>,
    channel: Option<u8>,
    empty: Option<WindowStats>,
    occupied: Option<WindowStats>,
    capture: Option<Capture>,
    live: VecDeque<f32>,
    last_frame_ms: u64,
    mismatch: bool,
    drifted: bool,
    last_capture_failed: bool,
}

impl Default for OccupancyReadiness {
    fn default() -> Self {
        Self {
            evidence: EvidenceKind::Live,
            baseline: None,
            room_id: None,
            baseline_id: None,
            device_id: None,
            channel: None,
            empty: None,
            occupied: None,
            capture: None,
            live: VecDeque::with_capacity(LIVE_WINDOW),
            last_frame_ms: 0,
            mismatch: false,
            drifted: false,
            last_capture_failed: false,
        }
    }
}

impl OccupancyReadiness {
    pub fn new(evidence: EvidenceKind) -> Self {
        Self {
            evidence,
            ..Self::default()
        }
    }

    pub fn is_capturing(&self) -> bool {
        self.capture.is_some()
    }

    pub fn start(
        &mut self,
        room_id: String,
        baseline: BaselineCalibration,
        phase: Phase,
        target_frames: usize,
        now_ms: u64,
    ) -> Result<(), &'static str> {
        if self.capture.is_some() {
            return Err("a verification capture is already running");
        }
        if !(MIN_FRAMES..=MAX_FRAMES).contains(&target_frames) {
            return Err("target_frames must be between 32 and 4096");
        }
        let id = baseline.calibration_uuid().to_string();
        if phase == Phase::Occupied
            && (self.empty.is_none() || self.baseline_id.as_deref() != Some(&id)
                || self.room_id.as_deref() != Some(&room_id)
                // calibration_uuid currently hashes metadata, not every
                // subcarrier statistic; require exact bounded baseline bytes.
                || self.baseline.as_ref().map(BaselineCalibration::to_bytes) != Some(baseline.to_bytes())
                || !self.empty_compatible())
        {
            return Err("verify this room's empty baseline first");
        }
        if phase == Phase::Empty {
            // A recheck invalidates the old occupied contrast until both phases
            // are observed again. Changing baseline/room can never inherit it.
            self.empty = None;
            self.occupied = None;
            self.room_id = Some(room_id);
            self.baseline_id = Some(id);
            self.baseline = Some(baseline);
            self.device_id = None;
            self.channel = None;
            self.mismatch = false;
            self.drifted = false;
        }
        self.live.clear();
        self.last_frame_ms = 0;
        self.last_capture_failed = false;
        self.capture = Some(Capture {
            phase,
            target: target_frames,
            scores: Vec::with_capacity(target_frames),
            started_ms: now_ms,
        });
        Ok(())
    }

    pub fn record(&mut self, frame: &CsiFrame, now_ms: u64) {
        let Some(baseline) = &self.baseline else {
            return;
        };
        match (&self.device_id, self.channel) {
            (None, None) => {
                self.device_id = Some(frame.metadata.device_id.as_str().to_owned());
                self.channel = Some(frame.metadata.channel);
            }
            (Some(id), Some(channel))
                if id == frame.metadata.device_id.as_str() && channel == frame.metadata.channel => {
            }
            _ => {
                self.mismatch = true;
                self.capture = None;
                self.empty = None;
                self.occupied = None;
                return;
            }
        }
        if self.last_frame_ms != 0 && now_ms.saturating_sub(self.last_frame_ms) > STALE_MS {
            // A resumed stream cannot reuse pre-outage samples or validation.
            self.live.clear();
            self.empty = None;
            self.occupied = None;
            self.capture = None;
        }
        match baseline.deviation(frame) {
            Ok(score) if score.amplitude_z_median.is_finite() => {
                // Align exactly with the baseline's first-stream, sequential
                // active bins; averaging all FFT bins would include guard/DC
                // tones and make empty verification incomparable.
                let n = baseline.subcarriers.len();
                if n == 0 || frame.num_spatial_streams() == 0 {
                    self.mismatch = true;
                    return;
                }
                let amp =
                    ((0..n).map(|k| frame.data[[0, k]].norm()).sum::<f64>() / n as f64) as f32;
                if !amp.is_finite() {
                    self.mismatch = true;
                    return;
                }
                self.last_frame_ms = now_ms;
                self.live.push_back(amp);
                if self.live.len() > LIVE_WINDOW {
                    self.live.pop_front();
                }
                if let Some(capture) = &mut self.capture {
                    capture.scores.push(amp);
                    if capture.scores.len() >= capture.target {
                        self.finish();
                    }
                } else if let (Some(empty), Some(occupied)) = (self.empty, self.occupied) {
                    if self.live.len() >= MIN_FRAMES && self.contrast_ok() {
                        let mut window: Vec<f32> = self.live.iter().copied().collect();
                        let contrast = (occupied.median_amp - empty.median_amp).abs();
                        let margin = contrast
                            .max(centre_uncertainty(empty, occupied))
                            .max(empty.median_amp.abs() * 0.05)
                            .max(MIN_ABSOLUTE_CONTRAST);
                        let current = median(&mut window);
                        if current < empty.median_amp.min(occupied.median_amp) - margin
                            || current > empty.median_amp.max(occupied.median_amp) + margin
                        {
                            self.drifted = true;
                        }
                    }
                }
            }
            _ => {
                // Wrong CSI grid or nonfinite score invalidates the live claim.
                self.mismatch = true;
                self.capture = None;
                self.empty = None;
                self.occupied = None;
            }
        }
    }

    pub fn tick(&mut self, now_ms: u64) {
        if self
            .capture
            .as_ref()
            .is_some_and(|c| now_ms.saturating_sub(c.started_ms) > capture_timeout_ms(c.target))
        {
            self.capture = None;
            self.last_capture_failed = true;
        }
    }

    fn finish(&mut self) {
        let Some(mut capture) = self.capture.take() else {
            return;
        };
        let centre = median(&mut capture.scores);
        let mut deviations: Vec<f32> = capture.scores.iter().map(|x| (x - centre).abs()).collect();
        let stats = WindowStats {
            median_amp: centre,
            mad_amp: median(&mut deviations),
            frames: capture.scores.len(),
        };
        match capture.phase {
            Phase::Empty => self.empty = Some(stats),
            Phase::Occupied => self.occupied = Some(stats),
        }
    }

    fn empty_compatible(&self) -> bool {
        let (Some(empty), Some(baseline)) = (self.empty, self.baseline.as_ref()) else {
            return false;
        };
        if baseline.subcarriers.is_empty() {
            return false;
        }
        let mean = baseline
            .subcarriers
            .iter()
            .map(|s| s.amp_mean as f64)
            .sum::<f64>()
            / baseline.subcarriers.len() as f64;
        mean.is_finite()
            && mean > 0.0
            && ((empty.median_amp as f64 - mean).abs() / mean) <= MAX_EMPTY_BASELINE_SHIFT as f64
    }

    fn contrast_ok(&self) -> bool {
        let (Some(empty), Some(occupied)) = (self.empty, self.occupied) else {
            return false;
        };
        let diff = (occupied.median_amp - empty.median_amp).abs();
        diff >= (empty.median_amp.abs() * MIN_RELATIVE_CONTRAST)
            .max(centre_uncertainty(empty, occupied))
            .max(MIN_ABSOLUTE_CONTRAST)
    }

    pub fn snapshot(&self, now_ms: u64) -> Readiness {
        let mut out = Readiness {
            target: "occupancy", state: "UNKNOWN", evidence_source: self.evidence.label(),
            room_id: self.room_id.clone(), baseline_id: self.baseline_id.clone(),
            device_id: self.device_id.clone(), channel: self.channel,
            phase: self.capture.as_ref().map(|c| match c.phase { Phase::Empty => "empty", Phase::Occupied => "occupied" }),
            frames: self.capture.as_ref().map_or(0, |c| c.scores.len()),
            target_frames: self.capture.as_ref().map_or(0, |c| c.target),
            observed_empty_amp: self.empty.map(|s| s.median_amp),
            observed_occupied_amp: self.occupied.map(|s| s.median_amp),
            signal_contrast_pct: self.empty.zip(self.occupied).and_then(|(e, o)| {
                (e.median_amp.abs() > f32::EPSILON).then_some(
                    100.0 * (o.median_amp - e.median_amp).abs() / e.median_amp.abs()
                )
            }),
            empty_frames: self.empty.map_or(0, |s| s.frames),
            occupied_frames: self.occupied.map_or(0, |s| s.frames),
            last_frame_age_ms: (self.last_frame_ms != 0)
                .then_some(now_ms.saturating_sub(self.last_frame_ms)),
            occupancy: None,
            reason: "no verified baseline", next_action: "capture an empty-room baseline, then verify empty and occupied windows",
            note: "Heuristic signal readiness only; no real-room accuracy certificate or safety-critical use.",
        };
        if self.baseline.is_none() {
            return out;
        }
        if self.mismatch {
            out.reason = "CSI source, AP channel, grid, or deviation score changed";
            out.next_action = "isolate one sensor and channel, check PHY configuration, recapture baseline, and repeat both verification phases";
            return out;
        }
        if self.capture.is_some() {
            out.reason = "guided verification in progress";
            out.next_action =
                "keep the room in the selected phase until the target frame count is reached";
            return out;
        }
        if self.last_capture_failed {
            out.reason = "verification timed out before enough frames arrived";
            out.next_action = "check ESP32 stream and retry verification";
            return out;
        }
        if self.last_frame_ms == 0 || now_ms.saturating_sub(self.last_frame_ms) > STALE_MS {
            out.reason = "live CSI stream is missing or stale";
            out.next_action =
                "restore the sensor stream, then repeat empty and occupied verification";
            return out;
        }
        let Some(empty) = self.empty else {
            out.reason = "empty-room window has not been verified";
            out.next_action = "leave the room empty and run empty verification";
            return out;
        };
        if !self.empty_compatible() {
            out.state = "DEGRADED";
            out.reason = "empty-room mean amplitude differs from the saved baseline";
            out.next_action = "clear the room, check sensor placement, recapture the baseline if changed, then repeat both phases";
            return out;
        }
        let Some(occupied) = self.occupied else {
            out.reason = "occupied-room contrast has not been verified";
            out.next_action = "stand in the room and run occupied verification";
            return out;
        };
        if self.evidence != EvidenceKind::Live {
            out.reason = "replay or synthetic contrast cannot establish live-room readiness";
            out.next_action =
                "start the service in live mode with a sensor and repeat both guided phases";
            return out;
        }
        if empty.frames < MIN_VERIFIED_FRAMES || occupied.frames < MIN_VERIFIED_FRAMES {
            out.state = "DEGRADED";
            out.reason = "operator-labelled windows contain too few frames for live readiness";
            out.next_action =
                "repeat empty then occupied verification with at least 600 frames in each phase";
            return out;
        }
        if !self.contrast_ok() {
            out.state = "DEGRADED";
            out.reason = "occupied and empty mean amplitudes lack noise-aware contrast";
            out.next_action = "move the sensor or change placement, then recapture baseline and repeat both phases";
            return out;
        }
        if self.drifted {
            out.state = "DEGRADED";
            out.reason = "live signal moved outside both operator-verified windows";
            out.next_action = "check occupancy and sensor placement; repeat empty then occupied verification, recapturing baseline if the room changed";
            return out;
        }
        if self.live.len() < MIN_FRAMES {
            out.reason = "waiting for a fresh live occupancy window";
            out.next_action = "wait for at least 32 fresh CSI frames";
            return out;
        }
        out.state = "VALID";
        out.reason = "operator-labelled windows show occupancy signal contrast";
        out.next_action = "repeat empty verification after room or sensor changes; investigate any disagreement with ground truth";
        let mut current: Vec<f32> = self.live.iter().copied().collect();
        let current_amp = median(&mut current);
        out.occupancy = Some(
            if (current_amp - occupied.median_amp).abs() < (current_amp - empty.median_amp).abs() {
                "occupied"
            } else {
                "empty"
            },
        );
        out
    }
}

fn median(values: &mut [f32]) -> f32 {
    values.sort_by(f32::total_cmp);
    let mid = values.len() / 2;
    if values.len() % 2 == 0 {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn centre_uncertainty(empty: WindowStats, occupied: WindowStats) -> f32 {
    // Compare uncertainty of the two window *centres*, not per-frame spread.
    // A small but repeatable shift can be real with hundreds of frames (#1909).
    // 1.857 × MAD / sqrt(n) approximates the standard error of a Gaussian
    // median. Serial correlation makes this only a heuristic, not a confidence
    // interval or accuracy guarantee.
    let se_empty = 1.857 * empty.mad_amp / (empty.frames as f32).sqrt();
    let se_occupied = 1.857 * occupied.mad_amp / (occupied.frames as f32).sqrt();
    NOISE_MULTIPLIER * (se_empty * se_empty + se_occupied * se_occupied).sqrt()
}

fn capture_timeout_ms(target_frames: usize) -> u64 {
    // Allow the requested target at 10 Hz plus 30 seconds of overhead, with a
    // two-minute floor for default 600-frame sessions. Below 10 Hz, report a
    // timeout rather than silently accepting a shorter verification window.
    MIN_CAPTURE_TIMEOUT_MS.max(target_frames as u64 * 100 + 30_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wifi_densepose_signal::{PhyTier, SubcarrierBaseline};

    fn baseline() -> BaselineCalibration {
        BaselineCalibration {
            tier: PhyTier::Ht20,
            captured_at_unix_s: 1,
            frame_count: 600,
            subcarriers: (0..52)
                .map(|_| SubcarrierBaseline {
                    amp_mean: 1.0,
                    amp_variance: 1.0,
                    phase_mean: 0.0,
                    phase_dispersion: 0.0,
                })
                .collect(),
        }
    }

    // These state tests use labelled scalar windows; they prove state handling,
    // not actual ESP32 detection accuracy.
    fn fill(m: &mut OccupancyReadiness, amp: f32, now: u64) {
        let c = m.capture.as_mut().unwrap();
        c.scores = vec![amp; c.target];
        m.last_frame_ms = now;
        m.live = vec![amp; LIVE_WINDOW].into();
        m.finish();
    }

    fn fill_spread(m: &mut OccupancyReadiness, centre: f32, spread: f32, now: u64) {
        let c = m.capture.as_mut().unwrap();
        c.scores = (0..c.target)
            .map(|i| {
                let x = i as f32;
                centre + spread * ((x * 0.671).sin() + 0.35 * (x * 1.113).sin())
            })
            .collect();
        m.last_frame_ms = now;
        m.live = vec![centre; LIVE_WINDOW].into();
        m.finish();
    }

    #[test]
    fn unknown_until_both_guided_phases_then_valid_and_recheck_invalidates() {
        let mut m = OccupancyReadiness::default();
        assert_eq!(m.snapshot(100).state, "UNKNOWN");
        assert!(m
            .start("room".into(), baseline(), Phase::Occupied, 600, 100)
            .is_err());
        m.start("room".into(), baseline(), Phase::Empty, 600, 100)
            .unwrap();
        fill(&mut m, 1.0, 200);
        assert_eq!(m.snapshot(200).state, "UNKNOWN");
        m.start("room".into(), baseline(), Phase::Occupied, 600, 300)
            .unwrap();
        fill(&mut m, 1.05, 400);
        let ready = m.snapshot(400);
        assert_eq!(ready.state, "VALID");
        assert_eq!(ready.occupancy, Some("occupied"));
        m.start("room".into(), baseline(), Phase::Empty, 600, 500)
            .unwrap();
        assert_eq!(m.snapshot(500).state, "UNKNOWN");
        fill(&mut m, 3.0, 600);
        assert_eq!(m.snapshot(600).state, "DEGRADED");
        assert!(m
            .start("room".into(), baseline(), Phase::Occupied, 600, 700)
            .is_err());
    }

    #[test]
    fn missing_stream_weak_contrast_and_timeout_fail_closed() {
        let mut m = OccupancyReadiness::default();
        m.start("room".into(), baseline(), Phase::Empty, 600, 100)
            .unwrap();
        fill(&mut m, 1.0, 200);
        m.start("room".into(), baseline(), Phase::Occupied, 600, 300)
            .unwrap();
        fill(&mut m, 1.002, 400);
        assert_eq!(m.snapshot(400).state, "DEGRADED");
        assert_eq!(m.snapshot(2501).state, "UNKNOWN");
        m.start("room".into(), baseline(), Phase::Empty, 600, 3000)
            .unwrap();
        m.tick(63_001);
        assert!(
            m.is_capturing(),
            "600-frame capture must survive a 60-second boundary"
        );
        m.tick(123001);
        assert_eq!(m.snapshot(123001).state, "UNKNOWN");
    }

    #[test]
    fn occupied_phase_rejects_changed_baseline_even_if_metadata_id_collides() {
        let mut m = OccupancyReadiness::default();
        m.start("room".into(), baseline(), Phase::Empty, 600, 100)
            .unwrap();
        fill(&mut m, 1.0, 200);
        let mut changed = baseline();
        changed.subcarriers[0].amp_mean += 1.0;
        assert_eq!(changed.calibration_uuid(), baseline().calibration_uuid());
        assert!(m
            .start("room".into(), changed, Phase::Occupied, 600, 300)
            .is_err());
    }

    #[test]
    fn replay_and_synthetic_can_exercise_contrast_but_never_promote_live_room() {
        for kind in [EvidenceKind::Replay, EvidenceKind::Synthetic] {
            let mut m = OccupancyReadiness::new(kind);
            m.start("room".into(), baseline(), Phase::Empty, 600, 100)
                .unwrap();
            fill(&mut m, 1.0, 200);
            m.start("room".into(), baseline(), Phase::Occupied, 600, 300)
                .unwrap();
            fill(&mut m, 1.05, 400);
            let s = m.snapshot(400);
            assert_eq!(s.state, "UNKNOWN");
            assert!(s.signal_contrast_pct.unwrap() > 4.9);
            assert!(s.evidence_source.contains("declared"));
        }
    }

    #[test]
    fn sustained_out_of_range_live_window_latches_degraded_until_revalidation() {
        let mut m = OccupancyReadiness::default();
        m.start("room".into(), baseline(), Phase::Empty, 600, 100)
            .unwrap();
        fill(&mut m, 1.0, 200);
        m.start("room".into(), baseline(), Phase::Occupied, 600, 300)
            .unwrap();
        fill(&mut m, 1.05, 400);
        assert_eq!(m.snapshot(400).state, "VALID");
        // Exercise the exact rolling-window decision with a synthetic frame
        // whose 52 active bins have amplitude 8 against mean 1 / variance 1.
        let mut packet = vec![0u8; 20 + 64 * 2];
        packet[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        packet[5] = 1;
        packet[6..8].copy_from_slice(&64u16.to_le_bytes());
        packet[8..12].copy_from_slice(&2432u32.to_le_bytes());
        for k in 0..64 {
            packet[20 + 2 * k] = 8;
        }
        let frame = crate::calibrate::parse_csi_packet(&packet, "ht20").unwrap();
        for i in 0..64 {
            m.record(&frame, 500 + i);
        }
        let degraded = m.snapshot(600);
        assert_eq!(degraded.state, "DEGRADED");
        assert_eq!(degraded.occupancy, None);
        assert!(degraded.next_action.contains("repeat empty then occupied"));
        // Merely moving back into range cannot silently revalidate a room.
        m.live = vec![1.0; LIVE_WINDOW].into();
        assert_eq!(m.snapshot(600).state, "DEGRADED");
        m.start("room".into(), baseline(), Phase::Empty, 600, 700)
            .unwrap();
        fill(&mut m, 1.0, 800);
        m.start("room".into(), baseline(), Phase::Occupied, 600, 900)
            .unwrap();
        fill(&mut m, 1.05, 1000);
        assert_eq!(m.snapshot(1000).state, "VALID");
    }

    #[test]
    fn changed_sensor_or_ap_channel_suppresses_occupancy() {
        let mut m = OccupancyReadiness::default();
        m.start("room".into(), baseline(), Phase::Empty, 600, 100)
            .unwrap();
        fill(&mut m, 1.0, 200);
        m.start("room".into(), baseline(), Phase::Occupied, 600, 300)
            .unwrap();
        fill(&mut m, 1.05, 400);
        let mut packet = vec![0u8; 20 + 64 * 2];
        packet[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        packet[4] = 1;
        packet[5] = 1;
        packet[6..8].copy_from_slice(&64u16.to_le_bytes());
        packet[8..12].copy_from_slice(&2432u32.to_le_bytes());
        for k in 0..64 {
            packet[20 + 2 * k] = 1;
        }
        let first = crate::calibrate::parse_csi_packet(&packet, "ht20").unwrap();
        m.record(&first, 500);
        packet[8..12].copy_from_slice(&2437u32.to_le_bytes());
        let changed = crate::calibrate::parse_csi_packet(&packet, "ht20").unwrap();
        m.record(&changed, 501);
        let s = m.snapshot(501);
        assert_eq!(s.state, "UNKNOWN");
        assert_eq!(s.occupancy, None);
        assert!(s.reason.contains("channel"));
    }

    #[test]
    fn measured_scale_mean_shifts_survive_broad_per_frame_spread_in_both_directions() {
        // Deterministic synthetic windows approximating the mean/spread scale
        // reported for ESP32 nodes in #1909. They do not reproduce that capture
        // or validate its real-world classification accuracy.
        for (empty_amp, occupied_amp, empty_spread, occupied_spread) in [
            (31.476, 31.944, 1.844, 1.522),
            (30.096, 28.325, 2.571, 2.533),
        ] {
            let mut b = baseline();
            for sc in &mut b.subcarriers {
                sc.amp_mean = empty_amp;
            }
            let mut m = OccupancyReadiness::default();
            m.start("room".into(), b.clone(), Phase::Empty, 1300, 100)
                .unwrap();
            fill_spread(&mut m, empty_amp, empty_spread, 200);
            m.start("room".into(), b, Phase::Occupied, 1300, 300)
                .unwrap();
            fill_spread(&mut m, occupied_amp, occupied_spread, 400);
            let s = m.snapshot(400);
            assert_eq!(
                s.state, "VALID",
                "direction {empty_amp} → {occupied_amp}: {}",
                s.reason
            );
            assert_eq!(s.occupancy, Some("occupied"));
            assert_eq!(s.empty_frames, 1300);
        }
    }
}
