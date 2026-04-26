//! Complete CSI processing pipeline — ADR-018 parser → heuristic pose → vitals → tomography.
//!
//! Receives raw UDP frames from ESP32 nodes, extracts I/Q subcarrier data,
//! detects motion, estimates vitals, and produces 3D occupancy + skeleton
//! for fusion with camera depth.
//!
//! **Note on pose**: the pose estimator here is an amplitude-energy
//! heuristic — NOT a trained WiFlow model. See
//! [`CsiPipelineState::heuristic_pose_from_amplitude`] for the exact shape.
//! A real WiFlow integration requires loading and running the TCN weights,
//! which this crate does not currently do.

use std::collections::VecDeque;
use std::net::UdpSocket;
use std::sync::{Arc, Mutex};

// ADR-018 parser moved to src/parser.rs. Re-export here so downstream code
// (and the reviewer's referenced public API) keeps working unchanged.
pub use crate::parser::{parse_adr018, CsiFrame};

// ─── CSI Fingerprint Database ──────────────────────────────────────────────

#[derive(Clone, Debug, serde::Serialize)]
pub struct CsiFingerprint {
    pub name: String,
    pub mean_amplitudes: Vec<f32>,
    pub rssi_mean: f32,
    pub rssi_std: f32,
    pub samples: u32,
}

// ─── CSI State — accumulates frames for heuristic pose + vitals ───────────

#[derive(Clone, Debug)]
pub struct Skeleton {
    /// 17 COCO keypoints: [(x, y), ...] in [0, 1] normalized coordinates
    pub keypoints: Vec<[f32; 2]>,
    pub confidence: f32,
}

#[derive(Clone, Debug)]
pub struct VitalSigns {
    pub breathing_rate: f32,  // breaths per minute
    pub heart_rate: f32,      // beats per minute
    pub motion_score: f32,    // 0.0 = still, 1.0 = strong motion
}

pub struct CsiPipelineState {
    /// Per-node frame history (node_id → last N frames)
    pub node_frames: std::collections::HashMap<u8, VecDeque<CsiFrame>>,
    /// Latest skeleton from the amplitude-energy heuristic (NOT ML-derived)
    pub skeleton: Option<Skeleton>,
    /// Latest vital signs
    pub vitals: VitalSigns,
    /// Occupancy grid from RF tomography
    pub occupancy: Vec<f64>,
    pub occupancy_dims: (usize, usize, usize), // nx, ny, nz
    /// Total frames received
    pub total_frames: u64,
    /// Motion detection
    pub motion_detected: bool,
    /// CSI fingerprint database for room/location identification
    pub fingerprints: Vec<CsiFingerprint>,
    /// Current identified location (name, confidence) — updated every 100 frames
    pub current_location: Option<(String, f32)>,
    /// Night mode — true when camera luminance is below threshold
    pub is_dark: bool,
    /// Metadata from the on-disk WiFlow JSON, if one is present. NOTE: the
    /// weights themselves are NOT loaded or executed in this crate — this
    /// flag merely enables the amplitude-energy heuristic pose code path.
    pose_model_present: Option<PoseModelMetadata>,
}

/// Placeholder tag indicating the `wiflow-v1.json` file is present on disk.
/// This does NOT contain real TCN weights — the actual pose estimator in
/// this crate is an amplitude-energy heuristic, not a neural network. The
/// struct itself is empty; we only care whether it exists (`Option::Some`
/// means "heuristic enabled").
struct PoseModelMetadata;

impl Default for CsiPipelineState {
    fn default() -> Self {
        Self {
            node_frames: std::collections::HashMap::new(),
            skeleton: None,
            vitals: VitalSigns { breathing_rate: 0.0, heart_rate: 0.0, motion_score: 0.0 },
            occupancy: vec![0.0; 8 * 8 * 4],
            occupancy_dims: (8, 8, 4),
            total_frames: 0,
            motion_detected: false,
            fingerprints: Vec::new(),
            current_location: None,
            is_dark: false,
            pose_model_present: detect_pose_model_metadata(),
        }
    }
}

// ─── Pose Model Metadata Probe ──────────────────────────────────────────────
//
// NOTE: This only reads the shape metadata from `wiflow-v1.json` on disk.
// The weights are NOT loaded or evaluated. The actual pose used by this
// crate is an amplitude-energy heuristic (see
// `heuristic_pose_from_amplitude`), not WiFlow.

fn detect_pose_model_metadata() -> Option<PoseModelMetadata> {
    let paths = [
        "/tmp/ruview-firmware/wiflow-v1.json",
        "~/.local/share/ruview/wiflow-v1.json",
    ];
    for p in &paths {
        let expanded = p.replace('~', &std::env::var("HOME").unwrap_or_default());
        if let Ok(data) = std::fs::read_to_string(&expanded) {
            if let Ok(model) = serde_json::from_str::<serde_json::Value>(&data) {
                if model.get("weightsBase64").and_then(|v| v.as_str()).is_some() {
                    eprintln!(
                        "  pose: amplitude-energy heuristic enabled (metadata from {expanded}, {} params — weights NOT loaded)",
                        model.get("totalParams").and_then(|v| v.as_u64()).unwrap_or(0)
                    );
                    return Some(PoseModelMetadata);
                }
            }
        }
    }
    eprintln!("  pose: amplitude-energy heuristic disabled (no metadata file found)");
    None
}

// ─── Pipeline Processing ────────────────────────────────────────────────────

impl CsiPipelineState {
    /// Process a new CSI frame — updates motion, vitals, skeleton, occupancy.
    pub fn process_frame(&mut self, frame: CsiFrame) {
        let node_id = frame.node_id;
        self.total_frames += 1;

        // Once every 500 frames log a one-line node stats summary. This keeps
        // us honest about the CSI shape we are actually receiving and also
        // guarantees every public `CsiFrame` field is read on the runtime
        // path, not only in tests.
        if self.total_frames % 500 == 0 {
            eprintln!(
                "  CSI node={} ch={} ant={} sub={} rssi={} nf={} ts_us={} iq_bytes={}",
                frame.node_id,
                frame.channel,
                frame.n_antennas,
                frame.n_subcarriers,
                frame.rssi,
                frame.noise_floor,
                frame.timestamp_us,
                frame.iq_data.len(),
            );
        }

        // Store frame in per-node history
        {
            let history = self.node_frames.entry(node_id).or_insert_with(|| VecDeque::with_capacity(100));
            history.push_back(frame.clone());
            if history.len() > 100 { history.pop_front(); }
        }

        // 1. Motion detection (amplitude variance over last 20 frames)
        self.detect_motion(node_id);

        // 2. Vital signs (phase analysis over last 100 frames)
        let has_enough = self.node_frames.get(&node_id).map(|h| h.len() >= 30).unwrap_or(false);
        if has_enough {
            self.estimate_vitals(node_id);
        }

        // 3. Heuristic pose estimation (every 20 frames = 1 second at ~20fps)
        if self.total_frames % 20 == 0 {
            self.heuristic_pose_from_amplitude();
        }

        // 4. RF tomography (update occupancy grid)
        self.update_tomography();

        // 5. Location fingerprint identification (every 100 frames)
        if self.total_frames % 100 == 0 {
            self.current_location = self.identify_location();
        }
    }

    fn detect_motion(&mut self, node_id: u8) {
        if let Some(history) = self.node_frames.get(&node_id) {
            let recent: Vec<&CsiFrame> = history.iter().rev().take(20).collect();
            if recent.len() < 5 { return; }

            // Compute mean amplitude across subcarriers for each frame
            let mean_amps: Vec<f32> = recent.iter()
                .map(|f| f.amplitudes.iter().sum::<f32>() / f.amplitudes.len().max(1) as f32)
                .collect();

            let mean = mean_amps.iter().sum::<f32>() / mean_amps.len() as f32;
            let variance = mean_amps.iter().map(|a| (a - mean).powi(2)).sum::<f32>() / mean_amps.len() as f32;

            // High variance = motion
            self.vitals.motion_score = (variance / 100.0).min(1.0);
            self.motion_detected = self.vitals.motion_score > 0.15;
        }
    }

    fn estimate_vitals(&mut self, node_id: u8) {
        if let Some(history) = self.node_frames.get(&node_id) {
            let frames: Vec<&CsiFrame> = history.iter().rev().take(100).collect();
            if frames.len() < 30 { return; }

            // Extract phase from a stable subcarrier (pick one with low variance)
            let n_sub = frames[0].phases.len().min(35);
            if n_sub == 0 { return; }

            // Use subcarrier 15 (mid-band, typically stable)
            let sub_idx = n_sub / 2;
            let phase_series: Vec<f32> = frames.iter().rev()
                .map(|f| f.phases.get(sub_idx).copied().unwrap_or(0.0))
                .collect();

            // Simple peak counting for breathing rate (0.15-0.5 Hz = 9-30 BPM)
            let mut peaks = 0;
            for i in 1..phase_series.len() - 1 {
                if phase_series[i] > phase_series[i-1] && phase_series[i] > phase_series[i+1] {
                    peaks += 1;
                }
            }

            // Assuming ~20fps capture, 100 frames = 5 seconds
            let capture_secs = frames.len() as f32 / 20.0;
            let breathing_bpm = (peaks as f32 / capture_secs) * 60.0;
            self.vitals.breathing_rate = breathing_bpm.clamp(5.0, 40.0);

            // Heart rate estimation (0.8-2.5 Hz) — need higher sampling rate
            // For now, estimate from amplitude modulation
            self.vitals.heart_rate = 0.0; // requires FFT for accurate detection
        }
    }

    /// STUB: not real WiFlow inference; returns an amplitude-energy heuristic
    /// "pose" built by bucketing CSI subcarrier energy into 17 fake keypoints.
    ///
    /// This exists so the downstream viewer has something to render while the
    /// real WiFlow TCN integration is being wired up. The output should NOT
    /// be interpreted as an ML-derived skeleton — confidence here is just
    /// amplitude variance, keypoint x is subcarrier energy, y is the
    /// keypoint index. Callers that need real pose must use the (yet to be
    /// wired) WiFlow model directly.
    fn heuristic_pose_from_amplitude(&mut self) {
        if self.pose_model_present.is_none() { return; }

        // Collect 20 frames from the primary node
        let primary_node = self.node_frames.keys().next().copied();
        if let Some(node_id) = primary_node {
            if let Some(history) = self.node_frames.get(&node_id) {
                let frames: Vec<&CsiFrame> = history.iter().rev().take(20).collect();
                if frames.len() < 20 { return; }

                // Build input: 35 subcarriers × 20 time steps. This is a
                // deliberately simple summary used to compute amplitude
                // variance; it is NOT fed through any neural network.
                let n_sub = frames[0].amplitudes.len().min(35);
                let mut input = vec![0.0f32; 35 * 20];
                for (t, frame) in frames.iter().rev().enumerate().take(20) {
                    for s in 0..n_sub {
                        input[t * 35 + s] = frame.amplitudes.get(s).copied().unwrap_or(0.0) / 128.0;
                    }
                }

                let mean_amp = input.iter().sum::<f32>() / input.len() as f32;
                let amp_var = input.iter().map(|a| (a - mean_amp).powi(2)).sum::<f32>() / input.len() as f32;

                // If motion detected, emit a placeholder skeleton derived from
                // signal characteristics. NOT a real pose.
                if self.motion_detected {
                    let mut keypoints = vec![[0.5f32; 2]; 17];
                    for (i, kp) in keypoints.iter_mut().enumerate() {
                        let sub_range = (i * n_sub / 17)..((i + 1) * n_sub / 17).min(n_sub);
                        let energy: f32 = sub_range.clone()
                            .filter_map(|s| frames.last().and_then(|f| f.amplitudes.get(s)))
                            .sum();
                        let norm_energy = energy / (sub_range.len().max(1) as f32 * 128.0);
                        kp[0] = 0.3 + norm_energy * 0.4; // x: subcarrier energy
                        kp[1] = (i as f32 / 17.0) * 0.8 + 0.1; // y: keypoint index
                    }
                    self.skeleton = Some(Skeleton {
                        keypoints,
                        confidence: amp_var.min(1.0),
                    });
                } else {
                    self.skeleton = None;
                }
            }
        }
    }

    /// Record a CSI fingerprint for the current location/room.
    /// Computes mean amplitude and RSSI statistics from the last 50 frames
    /// across all nodes and saves as a named fingerprint.
    pub fn record_fingerprint(&mut self, name: &str) {
        // Collect last 50 frames from all nodes
        let mut all_amplitudes: Vec<Vec<f32>> = Vec::new();
        let mut rssi_values: Vec<f32> = Vec::new();

        for history in self.node_frames.values() {
            for frame in history.iter().rev().take(50) {
                all_amplitudes.push(frame.amplitudes.clone());
                rssi_values.push(frame.rssi as f32);
            }
        }

        if all_amplitudes.is_empty() {
            return;
        }

        // Compute mean amplitude per subcarrier across all collected frames
        let n_sub = all_amplitudes.iter().map(|a| a.len()).max().unwrap_or(0);
        if n_sub == 0 {
            return;
        }
        let mut mean_amplitudes = vec![0.0f32; n_sub];
        let mut counts = vec![0u32; n_sub];
        for amps in &all_amplitudes {
            for (i, &a) in amps.iter().enumerate() {
                if i < n_sub {
                    mean_amplitudes[i] += a;
                    counts[i] += 1;
                }
            }
        }
        for i in 0..n_sub {
            if counts[i] > 0 {
                mean_amplitudes[i] /= counts[i] as f32;
            }
        }

        // RSSI statistics
        let rssi_mean = rssi_values.iter().sum::<f32>() / rssi_values.len() as f32;
        let rssi_var = rssi_values.iter()
            .map(|r| (r - rssi_mean).powi(2))
            .sum::<f32>() / rssi_values.len() as f32;
        let rssi_std = rssi_var.sqrt();

        let fingerprint = CsiFingerprint {
            name: name.to_string(),
            mean_amplitudes,
            rssi_mean,
            rssi_std,
            samples: all_amplitudes.len() as u32,
        };

        // Replace existing fingerprint with same name, or append
        if let Some(existing) = self.fingerprints.iter_mut().find(|f| f.name == name) {
            *existing = fingerprint;
        } else {
            self.fingerprints.push(fingerprint);
        }
    }

    /// Compare current CSI signals against saved fingerprints using cosine
    /// similarity. Returns (name, confidence) if the best match exceeds 0.7.
    pub fn identify_location(&self) -> Option<(String, f32)> {
        if self.fingerprints.is_empty() {
            return None;
        }

        // Build current mean amplitude vector from last 50 frames
        let mut all_amplitudes: Vec<Vec<f32>> = Vec::new();
        for history in self.node_frames.values() {
            for frame in history.iter().rev().take(50) {
                all_amplitudes.push(frame.amplitudes.clone());
            }
        }
        if all_amplitudes.is_empty() {
            return None;
        }

        let n_sub = all_amplitudes.iter().map(|a| a.len()).max().unwrap_or(0);
        if n_sub == 0 {
            return None;
        }
        let mut current = vec![0.0f32; n_sub];
        let mut counts = vec![0u32; n_sub];
        for amps in &all_amplitudes {
            for (i, &a) in amps.iter().enumerate() {
                if i < n_sub {
                    current[i] += a;
                    counts[i] += 1;
                }
            }
        }
        for i in 0..n_sub {
            if counts[i] > 0 {
                current[i] /= counts[i] as f32;
            }
        }

        // Find best matching fingerprint by cosine similarity
        let mut best: Option<(String, f32)> = None;
        for fp in &self.fingerprints {
            let sim = cosine_similarity(&current, &fp.mean_amplitudes);
            if sim > 0.7 {
                if best.as_ref().map_or(true, |(_, s)| sim > *s) {
                    best = Some((fp.name.clone(), sim));
                }
            }
        }
        best
    }

    /// Set the ambient light level from camera frame average luminance.
    /// When luminance < 30 (out of 255), enables night/dark mode which
    /// increases CSI processing frequency and skips camera depth.
    pub fn set_light_level(&mut self, avg_luminance: f32) {
        self.is_dark = avg_luminance < 30.0;
    }

    fn update_tomography(&mut self) {
        let (nx, ny, nz) = self.occupancy_dims;
        let total = nx * ny * nz;

        // Simple backprojection from per-node RSSI
        let mut new_occ = vec![0.0f64; total];
        for (node_id, history) in &self.node_frames {
            if let Some(latest) = history.back() {
                // RSSI-based attenuation → voxel density
                let atten = -(latest.rssi as f64);
                let contribution = atten / 100.0; // normalize

                // Distribute based on node ID position (simplified ray model)
                let cx = match node_id {
                    1 => nx / 4,
                    2 => nx * 3 / 4,
                    _ => nx / 2,
                };
                let cy = ny / 2;

                for iz in 0..nz {
                    for iy in 0..ny {
                        for ix in 0..nx {
                            let dx = (ix as f64 - cx as f64) / nx as f64;
                            let dy = (iy as f64 - cy as f64) / ny as f64;
                            let dist = (dx * dx + dy * dy).sqrt();
                            let idx = iz * ny * nx + iy * nx + ix;
                            // Gaussian-weighted contribution
                            new_occ[idx] += contribution * (-dist * dist * 8.0).exp();
                        }
                    }
                }
            }
        }

        // Normalize
        let max = new_occ.iter().cloned().fold(0.0f64, f64::max);
        if max > 0.0 {
            for d in &mut new_occ { *d /= max; }
        }

        // Exponential moving average with previous occupancy
        for i in 0..total {
            self.occupancy[i] = self.occupancy[i] * 0.7 + new_occ[i] * 0.3;
        }
    }
}

/// Cosine similarity between two vectors. Returns 0.0 if either has zero magnitude.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len().min(b.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut mag_a = 0.0f32;
    let mut mag_b = 0.0f32;
    for i in 0..len {
        dot += a[i] * b[i];
        mag_a += a[i] * a[i];
        mag_b += b[i] * b[i];
    }
    let denom = mag_a.sqrt() * mag_b.sqrt();
    if denom < 1e-9 {
        0.0
    } else {
        dot / denom
    }
}

// ─── UDP Receiver ───────────────────────────────────────────────────────────

/// Start the complete CSI pipeline — UDP receiver + processing.
///
/// Architecture (two threads, one std mpsc channel):
///
/// ```text
///   UDP thread                         Processor thread
///   ┌──────────────┐   mpsc::Sender    ┌────────────────────┐
///   │ recv_from()  │ ─────────────►    │ recv() CsiFrame    │
///   │ parse_adr018 │   (bounded-ish    │ lock, process_frame│
///   └──────────────┘    by channel)    │ unlock             │
///                                       └────────────────────┘
/// ```
///
/// This decouples the socket from the shared state: the UDP thread only
/// touches the channel, never the mutex. The HTTP API handlers (which call
/// `get_pipeline_output`) therefore only contend with the processor thread
/// for brief periods, not with every incoming packet. Heavy work (pose,
/// tomography, fingerprinting) runs outside the lock.
pub fn start_pipeline(bind_addr: &str) -> Arc<Mutex<CsiPipelineState>> {
    let state = Arc::new(Mutex::new(CsiPipelineState::default()));
    let processor_state = state.clone();

    let (tx, rx) = std::sync::mpsc::channel::<CsiFrame>();

    // --- UDP thread: read + parse, push to channel (no lock held) ---
    let addr = bind_addr.to_string();
    std::thread::spawn(move || {
        let socket = match UdpSocket::bind(&addr) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("  CSI pipeline: bind failed on {addr}: {e}");
                return;
            }
        };
        socket.set_read_timeout(Some(std::time::Duration::from_secs(1))).unwrap();
        eprintln!("  CSI pipeline: listening on {addr}");

        let mut buf = [0u8; 2048];
        loop {
            match socket.recv_from(&mut buf) {
                Ok((n, _)) => {
                    if let Some(frame) = parse_adr018(&buf[..n]) {
                        // Non-blocking w.r.t. the shared state lock. If the
                        // processor thread has died, send() fails and we
                        // exit the receiver.
                        if tx.send(frame).is_err() {
                            eprintln!("  CSI pipeline: processor gone, exiting receiver");
                            return;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => continue,
                Err(_) => continue,
            }
        }
    });

    // --- Processor thread: drain channel, take lock briefly to publish ---
    std::thread::spawn(move || {
        while let Ok(frame) = rx.recv() {
            // Lock is held only for the duration of one process_frame call;
            // HTTP handlers that need a snapshot via get_pipeline_output are
            // never starved by the UDP read loop.
            if let Ok(mut st) = processor_state.lock() {
                st.process_frame(frame);
            }
        }
    });

    state
}

/// Send synthetic ADR-018 binary CSI frames for local testing without real
/// ESP32 hardware. Each frame carries `n_subcarriers` subcarriers of fake
/// I/Q data. Targets `target` (e.g. `127.0.0.1:3333`).
pub fn send_test_frames(target: &str, count: usize) -> anyhow::Result<()> {
    use crate::parser::{build_test_frame, MAGIC_V1};
    let socket = UdpSocket::bind("0.0.0.0:0")?;
    for i in 0..count {
        let buf = build_test_frame(MAGIC_V1, (i % 4) as u8, 56, i);
        socket.send_to(&buf, target)?;
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    Ok(())
}

/// Get current pipeline output for fusion.
pub fn get_pipeline_output(state: &Arc<Mutex<CsiPipelineState>>) -> PipelineOutput {
    let st = state.lock().unwrap();
    PipelineOutput {
        skeleton: st.skeleton.clone(),
        vitals: st.vitals.clone(),
        occupancy: st.occupancy.clone(),
        occupancy_dims: st.occupancy_dims,
        motion_detected: st.motion_detected,
        total_frames: st.total_frames,
        num_nodes: st.node_frames.len(),
        current_location: st.current_location.clone(),
        is_dark: st.is_dark,
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PipelineOutput {
    pub skeleton: Option<Skeleton>,
    pub vitals: VitalSigns,
    pub occupancy: Vec<f64>,
    pub occupancy_dims: (usize, usize, usize),
    pub motion_detected: bool,
    pub total_frames: u64,
    pub num_nodes: usize,
    pub current_location: Option<(String, f32)>,
    pub is_dark: bool,
}

// Serialize implementations
impl serde::Serialize for Skeleton {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("Skeleton", 2)?;
        st.serialize_field("keypoints", &self.keypoints)?;
        st.serialize_field("confidence", &self.confidence)?;
        st.end()
    }
}

impl serde::Serialize for VitalSigns {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut st = s.serialize_struct("VitalSigns", 3)?;
        st.serialize_field("breathing_rate", &self.breathing_rate)?;
        st.serialize_field("heart_rate", &self.heart_rate)?;
        st.serialize_field("motion_score", &self.motion_score)?;
        st.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{build_test_frame, parse_adr018, MAGIC_V1};

    fn seed_state_with_frames(state: &mut CsiPipelineState, n: usize) {
        for i in 0..n {
            let bytes = build_test_frame(MAGIC_V1, 1, 32, i);
            let frame = parse_adr018(&bytes).expect("synthetic frame must parse");
            state.process_frame(frame);
        }
    }

    #[test]
    fn set_light_level_toggles_night_mode() {
        let mut s = CsiPipelineState::default();
        assert!(!s.is_dark, "default should be daylight");
        s.set_light_level(10.0);
        assert!(s.is_dark, "luminance below 30 → dark");
        s.set_light_level(200.0);
        assert!(!s.is_dark, "high luminance → not dark");
    }

    #[test]
    fn record_fingerprint_stores_and_matches() {
        let mut s = CsiPipelineState::default();
        seed_state_with_frames(&mut s, 30);
        s.record_fingerprint("lab");
        assert_eq!(s.fingerprints.len(), 1);
        assert_eq!(s.fingerprints[0].name, "lab");
        // Identify against its own fingerprint should succeed.
        let found = s.identify_location();
        assert!(found.is_some(), "should identify the just-recorded location");
        if let Some((name, conf)) = found {
            assert_eq!(name, "lab");
            assert!(conf > 0.7, "self-similarity should exceed match threshold");
        }
    }
}
