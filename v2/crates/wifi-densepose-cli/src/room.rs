//! `enroll` / `train-room` / `room-status` / `room-watch` — ADR-151 Stages 2–5 CLI.
//!
//! Drives the `wifi-densepose-calibration` pipeline against a live ESP32 CSI
//! stream (requires `edge_tier=0` raw CSI). `enroll` walks the guided anchors and
//! writes labelled features; `train-room` fits the specialist bank; `room-watch`
//! runs the mixture runtime and prints live room state.

use anyhow::{bail, Result};
use clap::Args;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use wifi_densepose_calibration::{
    Anchor, AnchorLabel, AnchorQualityGate, AnchorRecorder, EnrollmentEvent, EnrollmentSession,
    MixtureOfSpecialists, MultiNodeMixture, NodeGeometry, SpecialistBank,
};
use wifi_densepose_calibration::extract::{AnchorFeature, Features};
use wifi_densepose_core::types::CsiFrame;
use wifi_densepose_signal::BaselineCalibration;

use crate::calibrate::parse_csi_packet;

const RECV_BUF: usize = 2048;

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Per-frame scalar: mean amplitude across all subcarriers/streams.
///
/// Carries presence/motion energy plus the breathing amplitude modulation.
/// (Validated live on the ESP32 — picks up breathing where a max-variance
/// subcarrier instead locks onto motion artifacts. A phase-based carrier on a
/// *stable* subcarrier is the proper higher-SNR refinement — ADR-151 §4.)
fn frame_scalar(frame: &CsiFrame) -> f32 {
    let a = &frame.amplitude;
    if a.is_empty() {
        return 0.0;
    }
    (a.sum() / a.len() as f64) as f32
}

/// Diagnostic only: summarize per-subcarrier amplitude z-scores.
/// This does NOT affect enrollment acceptance.
fn z_distribution(
    frame: &CsiFrame,
    baseline: &BaselineCalibration,
) -> Option<(f32, f32, f32, f32, f32)> {
    let expected = baseline.subcarriers.len();
    let n_sc = frame.num_subcarriers();

    if expected == 0 || n_sc == 0 {
        return None;
    }

    let take = expected.min(n_sc);
    let mut z: Vec<f32> = Vec::with_capacity(take);

    for k in 0..take {
        let amp = frame.data[[0, k]].norm() as f32;
        let b = &baseline.subcarriers[k];

        if !amp.is_finite()
            || !b.amp_mean.is_finite()
            || !b.amp_variance.is_finite()
        {
            continue;
        }

        let std = b.amp_variance.sqrt().max(1e-6);
        z.push(((amp - b.amp_mean) / std).abs());
    }

    if z.is_empty() {
        return None;
    }

    z.sort_by(|a, b| {
        a.partial_cmp(b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    fn percentile(sorted: &[f32], p: f32) -> f32 {
        if sorted.is_empty() {
            return 0.0;
        }

        let pos = p * (sorted.len() - 1) as f32;
        let lo = pos.floor() as usize;
        let hi = pos.ceil() as usize;

        if lo == hi {
            sorted[lo]
        } else {
            let w = pos - lo as f32;
            sorted[lo] * (1.0 - w) + sorted[hi] * w
        }
    }

    let mean = z.iter().sum::<f32>() / z.len() as f32;
    let median = percentile(&z, 0.50);
    let p75 = percentile(&z, 0.75);
    let p90 = percentile(&z, 0.90);
    let max = *z.last().unwrap_or(&0.0);

    Some((mean, median, p75, p90, max))
}

fn load_baseline(path: &str) -> Result<BaselineCalibration> {
    let bytes = std::fs::read(path)
        .map_err(|e| anyhow::anyhow!("cannot read baseline {path}: {e} — run `calibrate` first"))?;
    BaselineCalibration::from_bytes(&bytes)
        .map_err(|e| anyhow::anyhow!("invalid baseline {path}: {e}"))
}

/// Persisted enrollment output (labelled features + audit log).
#[derive(serde::Serialize, serde::Deserialize)]
struct EnrollmentData {
    room_id: String,
    baseline_id: String,
    fs_hz: f32,
    anchors: Vec<AnchorFeature>,
    session: EnrollmentSession,
}

// ---------------------------------------------------------------------------
// enroll
// ---------------------------------------------------------------------------

/// Arguments for `enroll`.
#[derive(Args, Debug, Clone)]
pub struct EnrollArgs {
    /// UDP port for ESP32 CSI frames (raw CSI; provision with `--edge-tier 0`).
    #[arg(long, default_value_t = 5005)]
    pub udp_port: u16,
    /// Bind address for the UDP socket.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,
    /// Path to the empty-room baseline produced by `calibrate`.
    #[arg(long, default_value = "./baseline.bin")]
    pub baseline: String,
    /// PHY tier (ht20 / ht40 / he20 / he40).
    #[arg(long, default_value = "ht20")]
    pub tier: String,
    /// Room label.
    #[arg(long, default_value = "default")]
    pub room_id: String,
    /// Output enrollment file.
    #[arg(long, default_value = "./enrollment.json")]
    pub output: String,
    /// CSI sample rate (Hz) used for periodicity extraction.
    #[arg(long, default_value_t = 15.0)]
    pub fs_hz: f32,
    /// Max attempts per anchor before moving on.
    #[arg(long, default_value_t = 2)]
    pub attempts: u32,

    /// Only accept CSI frames from this ESP32 node ID.
    #[arg(long, conflicts_with = "all_nodes")]
    pub node_id: Option<u8>,

    /// Enroll all discovered/configured ESP32 nodes in one shared capture.
    #[arg(long, conflicts_with = "node_id")]
    pub all_nodes: bool,

    /// Directory containing baseline-node<N>.bin files for --all-nodes.
    #[arg(long, default_value = "./baselines")]
    pub baseline_dir: String,

    /// Output directory for enrollment-node<N>.json files in --all-nodes mode.
    #[arg(long, default_value = "./enrollments")]
    pub output_dir: String,

    /// Countdown seconds before each anchor capture starts. Raise this when the
    /// operator needs time to move into position between anchors.
    #[arg(long, default_value_t = 3)]
    pub lead_in_s: u32,

    /// Presence test threshold as a relative mean-amplitude shift versus this
    /// session's own `empty` capture (0.0075 = 0.75%). `0` disables the shift
    /// test and falls back to the legacy per-subcarrier `presence_z` gate.
    #[arg(long, default_value_t = 0.0)]
    pub min_mean_shift: f32,

    /// Gate override: minimum mean z-score for "a person is present".
    #[arg(long, default_value_t = 1.5)]
    pub min_presence_z: f32,

    /// Gate override: maximum mean z-score accepted for the `empty` anchor.
    #[arg(long, default_value_t = 1.0)]
    pub empty_max_z: f32,

    /// Gate override: maximum motion rate accepted for a "still" anchor.
    #[arg(long, default_value_t = 0.6)]
    pub max_still_motion: f32,

    /// Gate override: minimum motion rate required for the `move` anchor.
    #[arg(long, default_value_t = 0.3)]
    pub min_move_motion: f32,

    /// In `--all-nodes` mode, how many nodes must see the mean-amplitude shift
    /// before the anchor counts as "a person is present" for every node. A
    /// person is a property of the room, not of one node: a node whose link is
    /// dominated by a strong direct path (live data: node 1 shifted only
    /// 0.0-2.4% while nodes 2/3 shifted up to 13.7%) must not veto the anchor.
    #[arg(long, default_value_t = 1)]
    pub presence_votes: u32,
}

/// Capture one anchor: returns (accepted feature?, anchor verdict, reason).
async fn capture_anchor(
    socket: &UdpSocket,
    baseline: &BaselineCalibration,
    gate: &AnchorQualityGate,
    label: AnchorLabel,
    tier: &str,
    fs_hz: f32,
    room_id: &str,
    node_id: Option<u8>,
    lead_in_s: u32,
    reference_mean: Option<f32>,
) -> Result<(Option<AnchorFeature>, Anchor, Option<String>)> {
    eprintln!("\n[enroll] {} — {}", label.as_str(), label.prompt());
    for c in (1..=lead_in_s.max(1)).rev() {
        eprintln!("[enroll]   starting in {c}…");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
    eprintln!("[enroll]   capturing {} s…", label.duration_s());

    let mut recorder = AnchorRecorder::new(label);
    let mut series: Vec<f32> = Vec::new();
    let mut normalized_z_sum = 0.0f64;
    let mut normalized_z_frames = 0u32;
    // ADR-135 geometry guard: a frame may only be compared against a baseline
    // that was built from the SAME subcarrier count. A 192-bin baseline must
    // never consume 64/128-bin frames (and vice versa): mixing counts compares
    // different physical bins and inflates presence_z. This is the root cause
    // from the roomB session - calibration ran with esp32_ht40_192 while enroll
    // ran with --tier ht40, which admitted 128-bin frames against 192-bin
    // baselines. The baseline's stored subcarrier count is authoritative.
    let expected_sc = baseline.subcarriers.len();
    let mut geometry_skipped = 0u32;
    let mut buf = vec![0u8; RECV_BUF];
    let deadline = Instant::now() + Duration::from_secs(label.duration_s() as u64);

    while Instant::now() < deadline {
        let timeout = Duration::from_millis(500);
        if let Ok(Ok(n)) = tokio::time::timeout(timeout, socket.recv(&mut buf)).await {
            if let Some(frame) = parse_csi_packet(&buf[..n], tier) {
                if let Some(node_id) = node_id {
                    let expected = format!("esp32-node{}", node_id);
                    if frame.metadata.device_id.to_string() != expected {
                        continue;
                    }
                }
                if frame.num_subcarriers() != expected_sc {
                    geometry_skipped += 1;
                    if geometry_skipped <= 3 {
                        eprintln!(
                            "[enroll]   WARN skipping {}-subcarrier frame; baseline expects {expected_sc} \
                             (run `enroll --tier` matching the `calibrate --tier` that built the baseline, \
                             e.g. esp32_ht40_192)",
                            frame.num_subcarriers()
                        );
                    }
                    continue;
                }
                recorder.record_frame(baseline, &frame);

                if let Some((mean_z, median_z, p75_z, p90_z, max_z)) =
                    z_distribution(&frame, baseline)
                {
                    normalized_z_sum += median_z as f64;
                    normalized_z_frames += 1;

                    if normalized_z_frames == 1 {
                        eprintln!(
                            "[enroll] diagnostic z: mean={:.2} median={:.2} p75={:.2} p90={:.2} max={:.2}",
                            mean_z, median_z, p75_z, p90_z, max_z
                        );
                    }
                }

                series.push(frame_scalar(&frame));
            }
        }
    }

    if geometry_skipped > 0 {
        eprintln!(
            "[enroll]   note: skipped {geometry_skipped} frame(s) whose subcarrier count != baseline ({expected_sc})",
        );
    }

    let normalized_z = if normalized_z_frames == 0 {
        0.0
    } else {
        (normalized_z_sum / normalized_z_frames as f64) as f32
    };

    eprintln!(
        "[enroll]   diagnostic: normalized_presence_z={:.2} frames={}",
        normalized_z, normalized_z_frames
    );

    let feature_all = AnchorFeature::from_series(room_id, label, &series, fs_hz);
    let mean_shift_rel = reference_mean.and_then(|reference| {
        if gate.min_mean_shift > 0.0 && reference.abs() > f32::EPSILON {
            Some((feature_all.features.mean - reference) / reference)
        } else {
            None
        }
    });
    if let Some(shift) = mean_shift_rel {
        eprintln!("[enroll]   presence shift: {:+.2}% vs the empty capture", 100.0 * shift);
    }
    let (anchor, reason) = recorder.finalize(gate, now_unix(), mean_shift_rel);
    eprintln!(
        "[enroll]   features: mean={:.4} variance={:.6} motion={:.6} breathing_score={:.3} breathing_hz={:.3} heart_score={:.3} heart_hz={:.3} frames={}",
        feature_all.features.mean,
        feature_all.features.variance,
        feature_all.features.motion,
        feature_all.features.breathing_score,
        feature_all.features.breathing_hz,
        feature_all.features.heart_score,
        feature_all.features.heart_hz,
        series.len(),
    );

    // The caller decides; keeping the feature lets the room-level consensus in
    // `--all-nodes` mode re-evaluate a node that a per-node gate rejected.
    Ok((Some(feature_all), anchor, reason))
}


/// State accumulated for one node during a single anchor capture.
struct NodeAnchorCapture {
    recorder: AnchorRecorder,
    series: Vec<f32>,
    normalized_z_sum: f64,
    normalized_z_frames: u32,
    printed_diagnostic: bool,
    /// Frames skipped because their subcarrier count did not match the node baseline.
    skipped_geometry: u32,
}

/// Load every baseline-node<N>.bin from a directory.
fn load_node_baselines(dir: &str) -> Result<HashMap<u8, BaselineCalibration>> {
    let mut entries: Vec<(u8, std::path::PathBuf)> = Vec::new();

    for entry in std::fs::read_dir(dir)
        .map_err(|e| anyhow::anyhow!("cannot read baseline directory {dir}: {e}"))?
    {
        let entry = entry
            .map_err(|e| anyhow::anyhow!("cannot read baseline directory entry: {e}"))?;
        let path = entry.path();

        let Some(name) = path.file_name().and_then(|x| x.to_str()) else {
            continue;
        };

        let Some(rest) = name.strip_prefix("baseline-node") else {
            continue;
        };

        let Some(id_text) = rest.strip_suffix(".bin") else {
            continue;
        };

        let Ok(node_id) = id_text.parse::<u8>() else {
            continue;
        };

        entries.push((node_id, path));
    }

    entries.sort_by_key(|(node_id, _)| *node_id);

    if entries.is_empty() {
        bail!(
            "no baseline-node<N>.bin files found in {dir} — run \
             `calibrate --all-nodes` first"
        );
    }

    let mut baselines = HashMap::new();

    for (node_id, path) in entries {
        let path_str = path.to_string_lossy();
        let baseline = load_baseline(&path_str)?;
        eprintln!(
            "[enroll] loaded node {} baseline={} subcarriers={}",
            node_id,
            &baseline.calibration_uuid().to_string()[..8],
            baseline.subcarriers.len()
        );
        baselines.insert(node_id, baseline);
    }

    Ok(baselines)
}

fn packet_node_id(frame: &CsiFrame) -> Option<u8> {
    frame
        .metadata
        .device_id
        .to_string()
        .strip_prefix("esp32-node")
        .and_then(|s| s.parse::<u8>().ok())
}

/// Capture one anchor for all configured nodes simultaneously.
async fn capture_anchor_all_nodes(
    socket: &UdpSocket,
    baselines: &HashMap<u8, BaselineCalibration>,
    gate: &AnchorQualityGate,
    label: AnchorLabel,
    tier: &str,
    fs_hz: f32,
    room_id: &str,
    lead_in_s: u32,
    references: &HashMap<u8, f32>,
) -> Result<HashMap<u8, (Option<AnchorFeature>, Anchor, Option<String>)>> {
    eprintln!("\n[enroll] {} — {}", label.as_str(), label.prompt());

    for c in (1..=lead_in_s.max(1)).rev() {
        eprintln!("[enroll]   starting in {c}…");
        tokio::time::sleep(Duration::from_secs(1)).await;
    }

    eprintln!("[enroll]   capturing {} s for {} node(s)…",
        label.duration_s(),
        baselines.len()
    );

    let mut states: HashMap<u8, NodeAnchorCapture> = HashMap::new();

    for (&node_id, _) in baselines {
        states.insert(
            node_id,
            NodeAnchorCapture {
                recorder: AnchorRecorder::new(label),
                series: Vec::new(),
                normalized_z_sum: 0.0,
                normalized_z_frames: 0,
                printed_diagnostic: false,
                skipped_geometry: 0,
            },
        );
    }

    let mut buf = vec![0u8; RECV_BUF];
    let deadline = Instant::now() + Duration::from_secs(label.duration_s() as u64);

    while Instant::now() < deadline {
        let timeout = Duration::from_millis(500);

        if let Ok(Ok(n)) = tokio::time::timeout(timeout, socket.recv(&mut buf)).await {
            let Some(frame) = parse_csi_packet(&buf[..n], tier) else {
                continue;
            };

            let Some(node_id) = packet_node_id(&frame) else {
                continue;
            };

            let Some(baseline) = baselines.get(&node_id) else {
                continue;
            };

            let Some(state) = states.get_mut(&node_id) else {
                continue;
            };

            if frame.num_subcarriers() != baseline.subcarriers.len() {
                state.skipped_geometry += 1;
                if state.skipped_geometry <= 3 {
                    eprintln!(
                        "[enroll] node {} WARN skipping {}-subcarrier frame; baseline expects {} \
                         (run `enroll --tier` matching the `calibrate --tier` that built the baseline)",
                        node_id,
                        frame.num_subcarriers(),
                        baseline.subcarriers.len()
                    );
                }
                continue;
            }

            state.recorder.record_frame(baseline, &frame);

            if let Some((mean_z, median_z, p75_z, p90_z, max_z)) =
                z_distribution(&frame, baseline)
            {
                state.normalized_z_sum += median_z as f64;
                state.normalized_z_frames += 1;

                if !state.printed_diagnostic {
                    eprintln!(
                        "[enroll] node {} diagnostic z: mean={:.2} median={:.2} p75={:.2} p90={:.2} max={:.2}",
                        node_id,
                        mean_z,
                        median_z,
                        p75_z,
                        p90_z,
                        max_z
                    );
                    state.printed_diagnostic = true;
                }
            }

            state.series.push(frame_scalar(&frame));
        }
    }

    let mut results = HashMap::new();

    let node_ids: Vec<u8> = states.keys().copied().collect();

    for node_id in node_ids {
        let mut state = states
            .remove(&node_id)
            .expect("node state exists");

        let feature_all =
            AnchorFeature::from_series(room_id, label, &state.series, fs_hz);
        let mean_shift_rel = references.get(&node_id).and_then(|reference| {
            if gate.min_mean_shift > 0.0 && reference.abs() > f32::EPSILON {
                Some((feature_all.features.mean - reference) / reference)
            } else {
                None
            }
        });
        if let Some(shift) = mean_shift_rel {
            eprintln!("[enroll] node {} presence shift: {:+.2}% vs the empty capture", node_id, 100.0 * shift);
        }
        let (anchor, reason) = state.recorder.finalize(gate, now_unix(), mean_shift_rel);

        if state.skipped_geometry > 0 {
            eprintln!(
                "[enroll] node {} note: skipped {} frame(s) whose subcarrier count != baseline",
                node_id,
                state.skipped_geometry
            );
        }

        let normalized_z = if state.normalized_z_frames == 0 {
            0.0
        } else {
            (state.normalized_z_sum / state.normalized_z_frames as f64) as f32
        };

        eprintln!(
            "[enroll] node {} diagnostic: normalized_presence_z={:.2} frames={}",
            node_id,
            normalized_z,
            state.normalized_z_frames
        );

        eprintln!(
            "[enroll] node {} features: mean={:.4} variance={:.6} motion={:.6} breathing_score={:.3} breathing_hz={:.3} heart_score={:.3} heart_hz={:.3} frames={}",
            node_id,
            feature_all.features.mean,
            feature_all.features.variance,
            feature_all.features.motion,
            feature_all.features.breathing_score,
            feature_all.features.breathing_hz,
            feature_all.features.heart_score,
            feature_all.features.heart_hz,
            state.series.len(),
        );

        results.insert(node_id, (Some(feature_all), anchor, reason));
    }

    Ok(results)
}

async fn enroll_all_nodes(args: EnrollArgs) -> Result<()> {
    let baselines = load_node_baselines(&args.baseline_dir)?;

    std::fs::create_dir_all(&args.output_dir).map_err(|e| {
        anyhow::anyhow!("cannot create output directory {}: {e}", args.output_dir)
    })?;

    let gate = AnchorQualityGate {
        min_presence_z: args.min_presence_z,
        empty_max_z: args.empty_max_z,
        max_still_motion: args.max_still_motion,
        min_move_motion: args.min_move_motion,
        min_mean_shift: args.min_mean_shift,
        ..AnchorQualityGate::default()
    };
    let mut sessions: HashMap<u8, EnrollmentSession> = HashMap::new();
    let mut features: HashMap<u8, Vec<AnchorFeature>> = HashMap::new();

    for (&node_id, baseline) in &baselines {
        let baseline_id = baseline.calibration_uuid().to_string();

        sessions.insert(
            node_id,
            EnrollmentSession::new(&args.room_id, &baseline_id, now_unix()),
        );
        features.insert(node_id, Vec::new());
    }

    // Session reference for the mean-shift presence test, per node: the mean
    // amplitude of that node's accepted `empty` capture.
    let mut references: HashMap<u8, f32> = HashMap::new();

    for label in AnchorLabel::SEQUENCE {
        // Per-anchor acceptance map. A node that accepted THIS label must not be
        // re-captured for it, but every node must still be evaluated for the
        // next label. Declaring this outside the label loop marked nodes as
        // permanently "accepted", so once `empty` passed, anchors 2..N skipped
        // every node and the run recorded only 1/8 anchors.
        let mut accepted: HashMap<u8, bool> =
            baselines.keys().map(|&id| (id, false)).collect();
        let mut label_complete = false;

        for attempt in 1..=args.attempts {
            let mut results = capture_anchor_all_nodes(
                &bind_socket(&args).await?,
                &baselines,
                &gate,
                label,
                &args.tier,
                args.fs_hz,
                &args.room_id,
                args.lead_in_s,
                &references,
            )
            .await?;

            // Room-level presence consensus (see --presence-votes): if enough
            // nodes see the shift, the anchor is presence-valid for all of them.
            if gate.min_mean_shift > 0.0 && args.presence_votes > 0 {
                let mut votes = 0u32;
                for (&node_id, value) in results.iter() {
                    let (feat, _, _) = value;
                    let (Some(feat), Some(reference)) = (feat.as_ref(), references.get(&node_id))
                    else {
                        continue;
                    };
                    if reference.abs() > f32::EPSILON {
                        let shift = (feat.features.mean - reference) / reference;
                        if shift.abs() >= gate.min_mean_shift {
                            votes += 1;
                        }
                    }
                }
                if votes >= args.presence_votes {
                    for value in results.values_mut() {
                        let (_, anchor, reason) = value;
                        let (quality, why) = gate.evaluate(
                            label,
                            anchor.quality.presence_z,
                            anchor.quality.motion_rate,
                            anchor.quality.frames,
                            Some(1.0),
                        );
                        anchor.quality = quality;
                        *reason = why;
                    }
                }
            }

            label_complete = true;

            let node_ids: Vec<u8> = baselines.keys().copied().collect();

            for node_id in node_ids {
                if *accepted.get(&node_id).unwrap_or(&false) {
                    continue;
                }

                let (feat, anchor, reason) = results
                    .remove(&node_id)
                    .expect("every configured node has a capture result");

                if anchor.quality.accepted {
                    eprintln!(
                        "[enroll] node {} ✓ accepted {} (presence_z={:.2} motion={:.0}% frames={})",
                        node_id,
                        label.as_str(),
                        anchor.quality.presence_z,
                        anchor.quality.motion_rate * 100.0,
                        anchor.quality.frames
                    );

                    if let Some(f) = feat {
                        if label == AnchorLabel::Empty {
                            references.insert(node_id, f.features.mean);
                        }
                        features.get_mut(&node_id).unwrap().push(f);
                    }

                    sessions
                        .get_mut(&node_id)
                        .unwrap()
                        .apply(EnrollmentEvent::AnchorAccepted { anchor });

                    accepted.insert(node_id, true);
                } else {
                    label_complete = false;

                    let why = reason.unwrap_or_default();

                    eprintln!(
                        "[enroll] node {} ✗ rejected {}: {why}",
                        node_id,
                        label.as_str()
                    );

                    sessions
                        .get_mut(&node_id)
                        .unwrap()
                        .apply(EnrollmentEvent::AnchorRejected {
                            label,
                            reason: why,
                            at: now_unix(),
                        });
                }
            }

            if label_complete {
                break;
            }

            if attempt < args.attempts {
                eprintln!(
                    "[enroll] some nodes rejected '{}'; repeating the same pose ({}/{})…",
                    label.as_str(),
                    attempt + 1,
                    args.attempts
                );
            }
        }

        if !label_complete {
            eprintln!(
                "[enroll] '{}' not accepted by every node; continuing",
                label.as_str()
            );
        }
    }

    for (&node_id, baseline) in &baselines {
        let session = sessions.remove(&node_id).unwrap();
        let anchors = features.remove(&node_id).unwrap();

        let mut session = session;

        if session.is_complete() {
            session.apply(EnrollmentEvent::Completed { at: now_unix() });
        }

        let baseline_id = baseline.calibration_uuid().to_string();

        let data = EnrollmentData {
            room_id: args.room_id.clone(),
            baseline_id,
            fs_hz: args.fs_hz,
            anchors,
            session,
        };

        let output = format!(
            "{}/enrollment-node{}.json",
            args.output_dir.trim_end_matches('/'),
            node_id
        );

        std::fs::write(
            &output,
            serde_json::to_string_pretty(&data)
                .map_err(|e| anyhow::anyhow!("serialize: {e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("cannot write {output}: {e}"))?;

        let (got, total) = data.session.progress();

        eprintln!(
            "[enroll] node {} done: {}/{} anchors accepted → {}",
            node_id,
            got,
            total,
            output
        );
    }

    Ok(())
}

async fn bind_socket(args: &EnrollArgs) -> Result<UdpSocket> {
    let addr = format!("{}:{}", args.bind, args.udp_port);

    UdpSocket::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))
}

/// Execute `enroll`.
/// Execute `enroll`.
pub async fn enroll(args: EnrollArgs) -> Result<()> {
    if args.all_nodes {
        enroll_all_nodes(args).await
    } else {
        enroll_single_node(args).await
    }
}

async fn enroll_single_node(args: EnrollArgs) -> Result<()> {
    let baseline = load_baseline(&args.baseline)?;
    let baseline_id = baseline.calibration_uuid().to_string();
    let gate = AnchorQualityGate {
        min_presence_z: args.min_presence_z,
        empty_max_z: args.empty_max_z,
        max_still_motion: args.max_still_motion,
        min_move_motion: args.min_move_motion,
        min_mean_shift: args.min_mean_shift,
        ..AnchorQualityGate::default()
    };

    let addr = format!("{}:{}", args.bind, args.udp_port);
    let socket = UdpSocket::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))?;

    eprintln!(
        "[enroll] room='{}' baseline={} on udp://{addr}",
        args.room_id,
        &baseline_id[..8]
    );

    match args.node_id {
        Some(id) => eprintln!("[enroll] filtering CSI to node_id={id}"),
        None => eprintln!("[enroll] WARN: accepting CSI from all node IDs"),
    }

    eprintln!("[enroll] follow each prompt; bad captures are re-prompted.");

    let mut session =
        EnrollmentSession::new(&args.room_id, &baseline_id, now_unix());

    let mut features: Vec<AnchorFeature> = Vec::new();
    // Session reference for the mean-shift presence test: the `empty` anchor's
    // own capture mean for this node.
    let mut reference_mean: Option<f32> = None;

    for label in AnchorLabel::SEQUENCE {
        let mut accepted = false;

        for attempt in 1..=args.attempts {
            let (feat, anchor, reason) = capture_anchor(
                &socket,
                &baseline,
                &gate,
                label,
                &args.tier,
                args.fs_hz,
                &args.room_id,
                args.node_id,
                args.lead_in_s,
                reference_mean,
            )
            .await?;

            if anchor.quality.accepted {
                eprintln!(
                    "[enroll]   ✓ accepted (presence_z={:.2} motion={:.0}% frames={})",
                    anchor.quality.presence_z,
                    anchor.quality.motion_rate * 100.0,
                    anchor.quality.frames
                );

                if let Some(f) = feat {
                    if label == AnchorLabel::Empty {
                        reference_mean = Some(f.features.mean);
                    }
                    features.push(f);
                }

                session.apply(EnrollmentEvent::AnchorAccepted { anchor });
                accepted = true;
                break;
            } else {
                let why = reason.unwrap_or_default();

                eprintln!("[enroll]   ✗ rejected: {why}");

                session.apply(EnrollmentEvent::AnchorRejected {
                    label,
                    reason: why,
                    at: now_unix(),
                });

                if attempt < args.attempts {
                    eprintln!(
                        "[enroll]   retrying ({}/{})…",
                        attempt + 1,
                        args.attempts
                    );
                }
            }
        }

        if !accepted {
            eprintln!(
                "[enroll]   moving on without '{}'",
                label.as_str()
            );
        }
    }

    if session.is_complete() {
        session.apply(EnrollmentEvent::Completed { at: now_unix() });
    }

    let (got, total) = session.progress();

    let data = EnrollmentData {
        room_id: args.room_id.clone(),
        baseline_id,
        fs_hz: args.fs_hz,
        anchors: features,
        session,
    };

    std::fs::write(
        &args.output,
        serde_json::to_string_pretty(&data)
            .map_err(|e| anyhow::anyhow!("serialize: {e}"))?,
    )
    .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", args.output))?;

    eprintln!(
        "\n[enroll] done: {got}/{total} anchors accepted → {} (next: `train-room`)",
        args.output
    );

    Ok(())
}


// ---------------------------------------------------------------------------
// train-room
// ---------------------------------------------------------------------------

/// Arguments for `train-room`.
#[derive(Args, Debug, Clone)]
pub struct TrainRoomArgs {
    /// Enrollment file from `enroll`.
    #[arg(long, default_value = "./enrollment.json")]
    pub enrollment: String,
    /// Output specialist-bank file.
    #[arg(long, default_value = "./room-bank.json")]
    pub output: String,
    /// Optional transceiver-geometry file: a JSON array of `NodeGeometry`
    /// records (ADR-152 §2.1.1). Recorded into the enrollment session before
    /// training so the bank carries the layout it was trained under.
    #[arg(long)]
    pub geometry: Option<String>,
}

/// Execute `train-room`.
///
/// If the enrollment session carries a transceiver-geometry snapshot (recorded
/// at enroll time or supplied here via `--geometry`), it is threaded into the
/// bank (ADR-152 §2.1.1); a geometry-free enrollment still trains a valid bank.
pub async fn train_room(args: TrainRoomArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.enrollment)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e} — run `enroll` first", args.enrollment))?;
    let mut data: EnrollmentData =
        serde_json::from_str(&raw).map_err(|e| anyhow::anyhow!("invalid enrollment: {e}"))?;
    if data.anchors.is_empty() {
        bail!("no accepted anchors in {} — re-run enroll", args.enrollment);
    }

    if let Some(path) = &args.geometry {
        let graw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read geometry {path}: {e}"))?;
        let geometry: Vec<NodeGeometry> = serde_json::from_str(&graw).map_err(|e| {
            anyhow::anyhow!("invalid geometry {path}: {e} (expected a JSON array of NodeGeometry records)")
        })?;
        data.session.record_geometry(geometry, now_unix());
    }

    let mut bank = SpecialistBank::train(&data.room_id, &data.baseline_id, &data.anchors, now_unix())
        .map_err(|e| anyhow::anyhow!("training failed: {e}"))?;
    match data.session.geometry() {
        Some(g) if !g.is_empty() => {
            bank = bank.with_geometry(g.to_vec());
            eprintln!(
                "[train-room] geometry: {} node(s) snapshotted into the bank (ADR-152 §2.1.1)",
                bank.geometry.len()
            );
        }
        _ => eprintln!(
            "[train-room] no transceiver geometry recorded — bank will not support geometry conditioning (ADR-152 §2.1.2)"
        ),
    }
    std::fs::write(&args.output, bank.to_json().map_err(|e| anyhow::anyhow!("{e}"))?)
        .map_err(|e| anyhow::anyhow!("cannot write {}: {e}", args.output))?;

    eprintln!(
        "[train-room] room='{}' trained {} specialists from {} anchors → {}",
        bank.room_id,
        bank.trained_kinds().len(),
        bank.anchor_count,
        args.output
    );
    for k in bank.trained_kinds() {
        eprintln!("[train-room]   • {k:?}");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// room-status
// ---------------------------------------------------------------------------

/// Arguments for `room-status`.
#[derive(Args, Debug, Clone)]
pub struct RoomStatusArgs {
    /// Specialist-bank file.
    #[arg(long, default_value = "./room-bank.json")]
    pub bank: String,
}

/// Execute `room-status`.
pub async fn room_status(args: RoomStatusArgs) -> Result<()> {
    let raw = std::fs::read_to_string(&args.bank)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", args.bank))?;
    let bank = SpecialistBank::from_json(&raw).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("room:        {}", bank.room_id);
    println!("baseline:    {}", bank.baseline_id);
    println!("trained_at:  {}", bank.trained_at_unix_s);
    println!("anchors:     {}", bank.anchor_count);
    println!("specialists: {:?}", bank.trained_kinds());
    Ok(())
}

// ---------------------------------------------------------------------------
// room-watch
// ---------------------------------------------------------------------------

/// Arguments for `room-watch`.
#[derive(Args, Debug, Clone)]
pub struct RoomWatchArgs {
    /// Specialist-bank file (single-node mode).
    #[arg(long, default_value = "./room-bank.json")]
    pub bank: String,
    /// Multistatic mode: map a node id to its bank as `N:path` (repeatable).
    /// When supplied, frames are grouped by node id and fused (ADR-029/151).
    #[arg(long = "node-bank", value_name = "N:PATH")]
    pub node_bank: Vec<String>,
    /// UDP port for ESP32 CSI frames (raw CSI).
    #[arg(long, default_value_t = 5005)]
    pub udp_port: u16,
    /// Bind address.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,
    /// PHY tier.
    #[arg(long, default_value = "ht20")]
    pub tier: String,
    /// CSI sample rate (Hz).
    #[arg(long, default_value_t = 15.0)]
    pub fs_hz: f32,
    /// Rolling window length (frames) for each inference.
    #[arg(long, default_value_t = 200)]
    pub window: usize,
    /// Seconds to run (0 = until Ctrl-C).
    #[arg(long, default_value_t = 0)]
    pub seconds: u32,
    /// Drift compensation time constant in seconds (0 = off).
    ///
    /// The per-node mean amplitude drifts with the environment far more than a
    /// person shifts it: measured live, the window mean sat 2-7 amplitude units
    /// away from the reference its bank was trained against, while the trained
    /// presence threshold was as small as 0.047 units. With this set, each
    /// node's window mean is re-anchored to the empty mean its bank was trained
    /// with, using a slow exponential moving average of the live mean, so the
    /// slow drift is removed and a person entering (a step) still stands out.
    #[arg(long, default_value_t = 0.0)]
    pub drift_adapt_s: f32,
    /// Print the per-node inputs and decisions behind every fused readout:
    /// window features, the bank's presence thresholds, the mean distance that
    /// drove the presence decision, and the anomaly score behind a veto.
    #[arg(long)]
    pub diagnostics: bool,
}

/// Execute `room-watch` — live (multistatic) mixture-of-specialists readout.
pub async fn room_watch(args: RoomWatchArgs) -> Result<()> {
    if !args.node_bank.is_empty() {
        return room_watch_multi(args).await;
    }
    let raw = std::fs::read_to_string(&args.bank)
        .map_err(|e| anyhow::anyhow!("cannot read {}: {e}", args.bank))?;
    let bank = SpecialistBank::from_json(&raw).map_err(|e| anyhow::anyhow!("{e}"))?;
    let baseline_id = bank.baseline_id.clone();
    let mix = MixtureOfSpecialists::new(bank);

    let addr = format!("{}:{}", args.bind, args.udp_port);
    let socket = UdpSocket::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))?;
    eprintln!("[room-watch] inferring on udp://{addr} (window={} frames)", args.window);

    let mut buf = vec![0u8; RECV_BUF];
    let mut win: std::collections::VecDeque<f32> = std::collections::VecDeque::new();
    let start = Instant::now();
    let mut last_print = Instant::now();

    loop {
        if args.seconds > 0 && start.elapsed() >= Duration::from_secs(args.seconds as u64) {
            break;
        }
        if let Ok(Ok(n)) = tokio::time::timeout(Duration::from_millis(500), socket.recv(&mut buf)).await {
            if let Some(frame) = parse_csi_packet(&buf[..n], &args.tier) {
                win.push_back(frame_scalar(&frame));
                while win.len() > args.window {
                    win.pop_front();
                }
            }
        }
        if last_print.elapsed() >= Duration::from_secs(1) && win.len() >= 32 {
            let series: Vec<f32> = win.iter().copied().collect();
            let f = Features::from_series(&series, args.fs_hz);
            let s = mix.infer(&f, &baseline_id);
            let pres = s.presence.as_ref().map(|r| r.label.clone().unwrap_or_default()).unwrap_or("-".into());
            let post = s.posture.as_ref().and_then(|r| r.label.clone()).unwrap_or("-".into());
            let br = s.breathing.as_ref().map(|r| format!("{:.1}bpm", r.value)).unwrap_or("-".into());
            let hr = s.heartbeat.as_ref().map(|r| format!("{:.0}bpm", r.value)).unwrap_or("-".into());
            let rest = s.restlessness.as_ref().map(|r| format!("{:.2}", r.value)).unwrap_or("-".into());
            let flags = format!(
                "{}{}",
                if s.vetoed { " VETO" } else { "" },
                if s.stale { " STALE" } else { "" }
            );
            println!(
                "presence={pres:<7} posture={post:<8} breathing={br:<8} heart={hr:<7} restless={rest}{flags}"
            );
            last_print = Instant::now();
        }
    }
    Ok(())
}

/// Multistatic `room-watch`: fuse several co-located nodes (ADR-029/151).
async fn room_watch_multi(args: RoomWatchArgs) -> Result<()> {
    use std::collections::{BTreeMap, VecDeque};

    let mut mix = MultiNodeMixture::new();
    let mut node_ids: Vec<u8> = Vec::new();
    for spec in &args.node_bank {
        let (id_s, path) = spec
            .split_once(':')
            .ok_or_else(|| anyhow::anyhow!("--node-bank must be N:path (got {spec:?})"))?;
        let id: u8 = id_s
            .parse()
            .map_err(|_| anyhow::anyhow!("bad node id in {spec:?}"))?;
        let raw = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("cannot read {path}: {e}"))?;
        let bank = SpecialistBank::from_json(&raw).map_err(|e| anyhow::anyhow!("{e}"))?;
        let baseline = bank.baseline_id.clone();
        mix.add_node(id, bank, baseline);
        node_ids.push(id);
    }
    eprintln!("[room-watch] multistatic over nodes {node_ids:?}");

    let addr = format!("{}:{}", args.bind, args.udp_port);
    let socket = UdpSocket::bind(&addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind {addr}: {e}"))?;
    eprintln!("[room-watch] fusing on udp://{addr} (window={} frames)", args.window);

    let mut buf = vec![0u8; RECV_BUF];
    let mut wins: BTreeMap<u8, VecDeque<f32>> = BTreeMap::new();
    // Slow per-node mean tracker used by --drift-adapt-s.
    let mut ema_by_node: BTreeMap<u8, f32> = BTreeMap::new();
    let start = Instant::now();
    let mut last_print = Instant::now();

    loop {
        if args.seconds > 0 && start.elapsed() >= Duration::from_secs(args.seconds as u64) {
            break;
        }
        if let Ok(Ok(n)) =
            tokio::time::timeout(Duration::from_millis(500), socket.recv(&mut buf)).await
        {
            if n < 5 {
                continue;
            }
            let node_id = buf[4];
            if !node_ids.contains(&node_id) {
                continue;
            }
            if let Some(frame) = parse_csi_packet(&buf[..n], &args.tier) {
                let w = wins.entry(node_id).or_default();
                w.push_back(frame_scalar(&frame));
                while w.len() > args.window {
                    w.pop_front();
                }
            }
        }
        if last_print.elapsed() >= Duration::from_secs(1) {
            let mut per_node: BTreeMap<u8, Features> = wins
                .iter()
                .filter(|(_, w)| w.len() >= 32)
                .map(|(id, w)| {
                    let series: Vec<f32> = w.iter().copied().collect();
                    (*id, Features::from_series(&series, args.fs_hz))
                })
                .collect();

            if args.drift_adapt_s > 0.0 {
                // One update per printed window (about 1 Hz).
                let alpha = (1.0 / args.drift_adapt_s).clamp(1e-4, 1.0);
                for (id, f) in per_node.iter_mut() {
                    let Some(reference) = mix
                        .node_mixture(*id)
                        .and_then(|e| e.bank().presence.as_ref())
                        .map(|p| p.empty_mean)
                    else {
                        continue;
                    };
                    let raw = f.mean;
                    let ema = ema_by_node.entry(*id).or_insert(raw);
                    f.mean = reference + (raw - *ema);
                    *ema += alpha * (raw - *ema);
                }
            }
            if !per_node.is_empty() {
                if args.diagnostics {
                    for (id, f) in per_node.iter() {
                        let frames = wins.get(id).map(|w| w.len()).unwrap_or(0);
                        let ema = ema_by_node
                            .get(id)
                            .map(|e| format!(" ema={e:.3}"))
                            .unwrap_or_default();
                        println!(
                            "[diag] node {id} frames={frames} mean={:.3}{ema} var={:.3} motion={:.3} breath={:.3}/{:.3} heart={:.3}/{:.3}",
                            f.mean, f.variance, f.motion,
                            f.breathing_score, f.breathing_hz,
                            f.heart_score, f.heart_hz
                        );
                        let Some(entry) = mix.node_mixture(*id) else {
                            continue;
                        };
                        let baseline = entry.bank().baseline_id.clone();
                        if let Some(ps) = entry.bank().presence.as_ref() {
                            let mean_dist = (f.mean - ps.empty_mean).abs();
                            let by_var = f.variance > ps.threshold;
                            let by_mean =
                                ps.mean_dist_threshold.is_some_and(|t| mean_dist > t);
                            println!(
                                "[diag] node {id} presence: mean_dist={mean_dist:.3} (thr {:?}) by_mean={by_mean} | var={:.3} vs var_thr={:.3} by_var={by_var} => {}",
                                ps.mean_dist_threshold,
                                f.variance,
                                ps.threshold,
                                if by_var || by_mean { "PRESENT" } else { "absent" }
                            );
                        }
                        let st = entry.infer(f, &baseline);
                        if let Some(a) = st.anomaly.as_ref() {
                            println!(
                                "[diag] node {id} anomaly={:.3} conf={:.3} vetoed={} stale={}",
                                a.value, a.confidence, st.vetoed, st.stale
                            );
                        }
                        if let Some(p) = st.presence.as_ref() {
                            println!(
                                "[diag] node {id} presence_reading: {:?} value={:.2} conf={:.2}",
                                p.label, p.value, p.confidence
                            );
                        }
                    }
                }
                let active: Vec<u8> = per_node.keys().copied().collect();
                let s = mix.infer(&per_node);
                let pres = s.presence.as_ref().and_then(|r| r.label.clone()).unwrap_or("-".into());
                let post = s.posture.as_ref().and_then(|r| r.label.clone()).unwrap_or("-".into());
                let br = s.breathing.as_ref().map(|r| format!("{:.1}bpm", r.value)).unwrap_or("-".into());
                let flags = format!(
                    "{}{}",
                    if s.vetoed { " VETO" } else { "" },
                    if s.stale { " STALE" } else { "" }
                );
                println!(
                    "nodes={active:?} presence={pres:<7} posture={post:<8} breathing={br:<8}{flags}"
                );
            }
            last_print = Instant::now();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feature(label: AnchorLabel, variance: f32, motion: f32) -> AnchorFeature {
        AnchorFeature {
            room_id: "t".into(),
            label,
            features: Features {
                mean: 1.0,
                variance,
                motion,
                breathing_score: 0.0,
                breathing_hz: 0.0,
                heart_score: 0.0,
                heart_hz: 0.0,
            },
        }
    }

    /// Write a minimal valid enrollment file (two anchors, no geometry event).
    fn write_enrollment(dir: &std::path::Path) -> String {
        let data = EnrollmentData {
            room_id: "t".into(),
            baseline_id: "base-1".into(),
            fs_hz: 15.0,
            anchors: vec![
                feature(AnchorLabel::Empty, 1.0, 0.1),
                feature(AnchorLabel::StandStill, 10.0, 0.2),
            ],
            session: EnrollmentSession::new("t", "base-1", 1000),
        };
        let path = dir.join("enrollment.json");
        std::fs::write(&path, serde_json::to_string(&data).unwrap()).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn trained_bank(out: &std::path::Path) -> SpecialistBank {
        SpecialistBank::from_json(&std::fs::read_to_string(out).unwrap()).unwrap()
    }

    /// ADR-152 §2.1.1: `--geometry` records into the session and the bank
    /// snapshots it — enrollment geometry reaches the trained bank.
    #[tokio::test]
    async fn train_room_threads_geometry_when_provided() {
        let dir = tempfile::tempdir().unwrap();
        let enrollment = write_enrollment(dir.path());
        let geometry = vec![
            NodeGeometry::new(1, "tape-measure").with_position(0.0, 0.0, 1.0),
            NodeGeometry::unknown(2),
        ];
        let gpath = dir.path().join("geometry.json");
        std::fs::write(&gpath, serde_json::to_string(&geometry).unwrap()).unwrap();
        let out = dir.path().join("bank.json");

        train_room(TrainRoomArgs {
            enrollment,
            output: out.to_string_lossy().into_owned(),
            geometry: Some(gpath.to_string_lossy().into_owned()),
        })
        .await
        .unwrap();

        assert_eq!(trained_bank(&out).geometry, geometry);
    }

    /// A geometry-free enrollment still trains a valid bank (optional by
    /// design) — it just carries no snapshot.
    #[tokio::test]
    async fn train_room_without_geometry_yields_geometry_free_bank() {
        let dir = tempfile::tempdir().unwrap();
        let enrollment = write_enrollment(dir.path());
        let out = dir.path().join("bank.json");

        train_room(TrainRoomArgs {
            enrollment,
            output: out.to_string_lossy().into_owned(),
            geometry: None,
        })
        .await
        .unwrap();

        let bank = trained_bank(&out);
        assert!(bank.geometry.is_empty());
        assert!(bank.presence.is_some(), "bank still trains without geometry");
    }

    /// Geometry recorded at enroll time (in the session event log) is picked up
    /// without the `--geometry` flag.
    #[tokio::test]
    async fn train_room_uses_session_geometry() {
        let dir = tempfile::tempdir().unwrap();
        let geometry = vec![NodeGeometry::new(3, "floor-plan").with_position(1.0, 2.0, 1.5)];
        let mut session = EnrollmentSession::new("t", "base-1", 1000);
        session.record_geometry(geometry.clone(), 1000);
        let data = EnrollmentData {
            room_id: "t".into(),
            baseline_id: "base-1".into(),
            fs_hz: 15.0,
            anchors: vec![
                feature(AnchorLabel::Empty, 1.0, 0.1),
                feature(AnchorLabel::StandStill, 10.0, 0.2),
            ],
            session,
        };
        let epath = dir.path().join("enrollment.json");
        std::fs::write(&epath, serde_json::to_string(&data).unwrap()).unwrap();
        let out = dir.path().join("bank.json");

        train_room(TrainRoomArgs {
            enrollment: epath.to_string_lossy().into_owned(),
            output: out.to_string_lossy().into_owned(),
            geometry: None,
        })
        .await
        .unwrap();

        assert_eq!(trained_bank(&out).geometry, geometry);
    }

    #[tokio::test]
    async fn train_room_rejects_invalid_geometry_file() {
        let dir = tempfile::tempdir().unwrap();
        let enrollment = write_enrollment(dir.path());
        let gpath = dir.path().join("geometry.json");
        std::fs::write(&gpath, r#"{"not":"an array"}"#).unwrap();

        let err = train_room(TrainRoomArgs {
            enrollment,
            output: dir.path().join("bank.json").to_string_lossy().into_owned(),
            geometry: Some(gpath.to_string_lossy().into_owned()),
        })
        .await
        .unwrap_err();
        assert!(err.to_string().contains("invalid geometry"), "{err}");
    }
}
