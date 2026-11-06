//! `wifi-densepose calibrate` — empty-room baseline calibration subcommand.
//!
//! Reads CSI frames from a UDP socket (ESP32 0xC511_0001 wire format), feeds
//! them through [`wifi_densepose_signal::CalibrationRecorder`], prints a
//! real-time deviation banner (ADR-135 §risk 1), and serialises the finished
//! [`wifi_densepose_signal::BaselineCalibration`] to disk in the compact
//! little-endian binary format defined in ADR-135 §2.4.
//!
//! # Wire format parsed here (option b — local parser, no cross-crate dep)
//!
//! Authoritative layout: firmware `csi_collector.c` (ADR-018 + ADR-110).
//!
//! Offset  Size  Field
//! ──────  ────  ─────────────────────────────────────────────────────────────
//!  0      4     Magic: 0xC511_0001 (LE u32)
//!  4      1     node_id (u8)
//!  5      1     n_antennas (u8)
//!  6      2     n_subcarriers (LE u16 — 256 for ESP32-C6 HE-SU frames, #1005)
//!  8      4     freq_mhz (LE u32)
//! 12      4     sequence (LE u32)
//! 16      1     rssi (i8)
//! 17      1     noise_floor (i8)
//! 18      1     PPDU type (ADR-110: 0=HT/legacy, 1=HE-SU, 2=HE-MU, 3=HE-TB)
//! 19      1     flags (ADR-110: bit0 bw40, bit4 time-sync valid)
//! 20      2 × n_antennas × n_subcarriers   IQ pairs: i_val (i8), q_val (i8)
//!
//! This parser mirrors `parse_esp32_frame` in
//! `wifi-densepose-sensing-server/src/csi.rs` (same magic, same layout).


use anyhow::{bail, Result};
use clap::Args;
use ndarray::Array2;
use num_complex::Complex64;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::net::UdpSocket;
use wifi_densepose_core::types::{
    AntennaConfig, CsiFrame, CsiMetadata, DeviceId, FrequencyBand, Timestamp,
};
use wifi_densepose_signal::{
    BaselineCalibration, CalibrationConfig, CalibrationDeviationScore, CalibrationRecorder,
};

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

/// Arguments for the `calibrate` subcommand.
#[derive(Args, Debug, Clone)]
pub struct CalibrateArgs {
    /// UDP port to listen on for CSI frames from the ESP32.
    /// Must match the target-port written into NVS by provision.py (default 5005).
    #[arg(long, default_value_t = 5005)]
    pub udp_port: u16,

    /// Bind address for the UDP socket.
    /// Default 0.0.0.0 receives from any device on the LAN.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// Calibration duration in seconds.
    /// ADR-135 default is 30 s at 20 Hz = 600 frames.
    /// Minimum 10; values above 300 emit a warning.
    #[arg(long, default_value_t = 30)]
    pub duration_s: u32,

    /// Only accept CSI frames from this ESP32 node ID.
    /// If omitted, frames from all nodes on the UDP socket are accepted.
    #[arg(long, conflicts_with = "all_nodes")]
    pub node_id: Option<u8>,

    /// Calibrate every discovered ESP32 node during the same capture window.
    /// One independent baseline is written per node.
    #[arg(long, conflicts_with = "node_id")]
    pub all_nodes: bool,

    /// Output path for the binary baseline file (single-node mode).
    #[arg(long, default_value = "./baseline.bin")]
    pub output: String,

    /// Output directory for per-node baselines in --all-nodes mode.
    #[arg(long, default_value = "./baselines")]
    pub output_dir: String,

    /// PHY tier matching the ESP32 configuration.
    /// Valid: ht20 / ht40 / he20 / he40.
    #[arg(long, default_value = "ht20")]
    pub tier: String,

    /// Print a deviation banner to stderr every N frames during capture.
    /// 0 disables banners. Default 20 = once per second at 20 Hz.
    #[arg(long, default_value_t = 20)]
    pub banner_every: u32,

    /// Abort if the per-frame amplitude z-score median exceeds this value
    /// for 20 consecutive banner intervals. 0.0 disables the abort guard.
    #[arg(long, default_value_t = 2.0)]
    pub abort_z_threshold: f32,

    /// Override the ADR-135 minimum frame count for the tier. 0 = use the
    /// tier default (600 for HT20 at 20 Hz = 30 s). Useful for debugging or
    /// low-traffic environments where the firmware emits CSI far below 20 Hz.
    /// Production deployments should leave this at 0.
    #[arg(long, default_value_t = 0)]
    pub min_frames: u32,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum UDP receive buffer.  HT20 CSI frame is well under 1 500 bytes.
const RECV_BUF: usize = 2048;

/// Number of banner intervals in the high-z abort sliding window.
const ABORT_WINDOW_INTERVALS: u32 = 20;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Execute the `calibrate` subcommand (async).
pub async fn execute(args: CalibrateArgs) -> Result<()> {
    validate_args(&args)?;

    if args.all_nodes {
        execute_all_nodes(args).await
    } else {
        execute_single_node(args).await
    }
}

async fn execute_single_node(args: CalibrateArgs) -> Result<()> {
    let mut config = tier_config(&args.tier);
    if args.min_frames > 0 {
        config.min_frames = args.min_frames;
        eprintln!(
            "[calibrate] WARN: --min-frames={} overrides ADR-135 tier default ({} for {}). \
             This relaxes the phase-concentration guarantee; do not use in production.",
            args.min_frames, tier_config(&args.tier).min_frames, args.tier
        );
    }

    let target_frames = config.min_frames as usize;
    let addr = format!("{}:{}", args.bind, args.udp_port);

    let socket = UdpSocket::bind(&addr).await
        .map_err(|e| anyhow::anyhow!("cannot bind UDP socket on {addr}: {e}"))?;

    eprintln!("[calibrate] listening on udp://{addr}");
    eprintln!(
        "[calibrate] capturing {} frames (~{} s, tier={}) — ensure room is empty",
        target_frames, args.duration_s, args.tier
    );

    let mut recorder = CalibrationRecorder::new(config);
    let mut buf = vec![0u8; RECV_BUF];
    let mut high_z_count: u32 = 0;
    let deadline = Instant::now() + Duration::from_secs(args.duration_s as u64);

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        let timeout = remaining.min(Duration::from_millis(500));
        let recv = tokio::time::timeout(timeout, socket.recv(&mut buf)).await;

        let n = match recv {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                eprintln!("[calibrate] recv error: {e}");
                continue;
            }
            Err(_) => continue,
        };

        let Some(csi_frame) = parse_csi_packet(&buf[..n], &args.tier) else {
            continue;
        };

        if let Some(node_id) = args.node_id {
            let expected = format!("esp32-node{}", node_id);
            if csi_frame.metadata.device_id.to_string() != expected {
                continue;
            }
        }

        let score = match recorder.record(&csi_frame) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[calibrate] WARN frame skipped: {e}");
                continue;
            }
        };

        let frames = recorder.frames_recorded() as usize;

        if args.banner_every > 0 && (frames as u32) % args.banner_every == 0 {
            print_banner(frames, target_frames, &score);

            if args.abort_z_threshold > 0.0
                && score.amplitude_z_median > args.abort_z_threshold
            {
                high_z_count += 1;
                if high_z_count >= ABORT_WINDOW_INTERVALS {
                    bail!(
                        "aborted: amplitude_z_median={:.2} exceeded threshold={:.2} for {} \
                         consecutive banner intervals — ensure the room is empty and retry",
                        score.amplitude_z_median,
                        args.abort_z_threshold,
                        high_z_count
                    );
                }
            } else {
                high_z_count = 0;
            }
        }

        if frames >= target_frames {
            break;
        }
    }

    finalise_and_save(recorder, &args.output)
}

async fn execute_all_nodes(args: CalibrateArgs) -> Result<()> {
    let addr = format!("{}:{}", args.bind, args.udp_port);
    let socket = UdpSocket::bind(&addr).await
        .map_err(|e| anyhow::anyhow!("cannot bind UDP socket on {addr}: {e}"))?;

    std::fs::create_dir_all(&args.output_dir).map_err(|e| {
        anyhow::anyhow!("cannot create output directory {}: {e}", args.output_dir)
    })?;

    let base_config = tier_config(&args.tier);
    let target_frames = if args.min_frames > 0 {
        args.min_frames as usize
    } else {
        base_config.min_frames as usize
    };

    if args.min_frames > 0 {
        eprintln!(
            "[calibrate] WARN: --min-frames={} overrides ADR-135 tier default ({} for {}). \
             This relaxes the phase-concentration guarantee; do not use in production.",
            args.min_frames, base_config.min_frames, args.tier
        );
    }

    eprintln!("[calibrate] listening on udp://{addr}");
    eprintln!(
        "[calibrate] ALL-NODES mode: one {} s capture window, tier={}, target={} valid frames per node",
        args.duration_s, args.tier, target_frames
    );
    eprintln!("[calibrate] leave the room empty and still");

    struct NodeState {
        recorder: CalibrationRecorder,
        high_z_count: u32,
        last_reported_frames: usize,
    }

    let mut nodes: HashMap<u8, NodeState> = HashMap::new();
    let mut buf = vec![0u8; RECV_BUF];
    let deadline = Instant::now() + Duration::from_secs(args.duration_s as u64);

    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }

        let timeout = remaining.min(Duration::from_millis(500));
        let recv = tokio::time::timeout(timeout, socket.recv(&mut buf)).await;

        let n = match recv {
            Ok(Ok(n)) => n,
            Ok(Err(e)) => {
                eprintln!("[calibrate] recv error: {e}");
                continue;
            }
            Err(_) => continue,
        };

        let Some(csi_frame) = parse_csi_packet(&buf[..n], &args.tier) else {
            continue;
        };

        let node_id = csi_frame
            .metadata
            .device_id
            .to_string()
            .strip_prefix("esp32-node")
            .and_then(|s| s.parse::<u8>().ok());

        let Some(node_id) = node_id else {
            continue;
        };

        if !nodes.contains_key(&node_id) {
            let mut cfg = tier_config(&args.tier);
            if args.min_frames > 0 {
                cfg.min_frames = args.min_frames;
            }

            eprintln!(
                "[calibrate] discovered node {} → starting independent recorder",
                node_id
            );

            nodes.insert(
                node_id,
                NodeState {
                    recorder: CalibrationRecorder::new(cfg),
                    high_z_count: 0,
                    last_reported_frames: 0,
                },
            );
        }

        let state = nodes.get_mut(&node_id).expect("node inserted above");

        let score = match state.recorder.record(&csi_frame) {
            Ok(s) => s,
            Err(e) => {
                eprintln!(
                    "[calibrate] node {} WARN frame skipped: {e}",
                    node_id
                );
                continue;
            }
        };

        let frames = state.recorder.frames_recorded() as usize;

        if args.banner_every > 0
            && (frames as u32) % args.banner_every == 0
            && frames != state.last_reported_frames
        {
            state.last_reported_frames = frames;

            eprintln!(
                "[calibrate] node {}: {}/{} frames | z_med={:.2} z_max={:.2} | motion={}",
                node_id,
                frames,
                target_frames,
                score.amplitude_z_median,
                score.amplitude_z_max,
                if score.motion_flagged { "YES" } else { "no" }
            );

            if args.abort_z_threshold > 0.0
                && score.amplitude_z_median > args.abort_z_threshold
            {
                state.high_z_count += 1;

                if state.high_z_count >= ABORT_WINDOW_INTERVALS {
                    bail!(
                        "node {} aborted: amplitude_z_median={:.2} exceeded threshold={:.2} \
                         for {} consecutive banner intervals — ensure the room is empty and retry",
                        node_id,
                        score.amplitude_z_median,
                        args.abort_z_threshold,
                        state.high_z_count
                    );
                }
            } else {
                state.high_z_count = 0;
            }
        }
    }

    if nodes.is_empty() {
        bail!("--all-nodes captured no recognised ESP32 nodes");
    }

    let mut node_ids: Vec<u8> = nodes.keys().copied().collect();
    node_ids.sort_unstable();

    let node_count = nodes.len();

    for node_id in node_ids {
        let state = nodes.remove(&node_id).expect("node exists");

        let frames = state.recorder.frames_recorded();
        if frames < target_frames as u32 {
            eprintln!(
                "[calibrate] node {}: only {} valid frames (target {}); finalization may fail",
                node_id, frames, target_frames
            );
        }

        let output = format!(
            "{}/baseline-node{}.bin",
            args.output_dir.trim_end_matches('/'),
            node_id
        );

        eprintln!("[calibrate] node {}: finalising {} frames…", node_id, frames);

        let baseline = state.recorder
            .finalize()
            .map_err(|e| anyhow::anyhow!("node {} calibration failed: {e}", node_id))?;

        let bytes = baseline.to_bytes();
        std::fs::write(&output, &bytes)
            .map_err(|e| anyhow::anyhow!("cannot write {output}: {e}"))?;

        eprintln!(
            "[calibrate] node {} baseline saved to {} ({} bytes), subcarriers={}",
            node_id,
            output,
            bytes.len(),
            baseline.subcarriers.len()
        );
    }

    eprintln!(
        "[calibrate] ALL-NODES complete: {} node(s) written to {}",
        node_count,
        args.output_dir
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Banner printer
// ---------------------------------------------------------------------------

fn print_banner(frames: usize, target: usize, score: &CalibrationDeviationScore) {
    let motion_str = if score.motion_flagged {
        "YES \u{2190} operator should be still"
    } else {
        "no"
    };
    eprintln!(
        "[calibrate] {}/{} frames | z_med={:.2} z_max={:.2} | motion: {}",
        frames, target, score.amplitude_z_median, score.amplitude_z_max, motion_str
    );
}

// ---------------------------------------------------------------------------
// Finalise + persist
// ---------------------------------------------------------------------------

fn finalise_and_save(recorder: CalibrationRecorder, output: &str) -> Result<()> {
    let frames = recorder.frames_recorded();
    eprintln!("[calibrate] finalising baseline from {frames} frames…");

    let baseline: BaselineCalibration = recorder
        .finalize()
        .map_err(|e| anyhow::anyhow!("calibration failed: {e}"))?;

    let bytes = baseline.to_bytes();
    std::fs::write(output, &bytes)
        .map_err(|e| anyhow::anyhow!("cannot write {output}: {e}"))?;

    eprintln!(
        "[calibrate] baseline saved to {output} ({} bytes)",
        bytes.len()
    );
    eprintln!(
        "[calibrate] summary: frames={} tier={:?} subcarriers={}",
        baseline.frame_count,
        baseline.tier,
        baseline.subcarriers.len(),
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Tier helper
// ---------------------------------------------------------------------------

pub(crate) fn tier_config(tier: &str) -> CalibrationConfig {
    match tier.to_ascii_lowercase().as_str() {
        "ht40" => CalibrationConfig::ht40(),
        "esp32_ht40_192" => CalibrationConfig::ht40_192(),
        "esp32_ht40_128" => CalibrationConfig::ht40_128(),
        "esp32_ht40_306" => CalibrationConfig::ht40_306(),
        "he20" => CalibrationConfig::he20(),
        "he40" => CalibrationConfig::he40(),
        _      => CalibrationConfig::ht20(), // ht20 or unknown → safe default
    }
}

// ---------------------------------------------------------------------------
// Local UDP packet parser (option b)
//
// Mirrors parse_esp32_frame in wifi-densepose-sensing-server/src/csi.rs.
// Magic 0xC511_0001, 20-byte header, IQ bytes follow.
// ---------------------------------------------------------------------------

/// Parse a single UDP datagram and return a `CsiFrame` ready for
/// `CalibrationRecorder::record()`.  Returns `None` on any parse failure.
pub(crate) fn parse_csi_packet(buf: &[u8], tier: &str) -> Option<CsiFrame> {
    if buf.len() < 20 {
        return None;
    }
    let magic = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if magic != 0xC511_0001 {
        return None;
    }

    let node_id       = buf[4];
    let n_antennas    = buf[5] as usize;
    // u16 since ADR-110 / #1005: ESP32-C6 HE-SU frames carry 256 bins
    // (the old single-byte read decoded 256 = 0x0100 LE as 0 subcarriers).
    let n_subcarriers = u16::from_le_bytes([buf[6], buf[7]]) as usize;
    let freq_mhz      = u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]);
    let freq_mhz      = u16::try_from(freq_mhz).unwrap_or(0);
    let _sequence     = u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]);
    let rssi          = buf[16] as i8;
    let noise_floor   = buf[17] as i8;
    let _ppdu_type    = buf[18]; // ADR-110; baseline tier gating is by count

    let n_pairs = n_antennas * n_subcarriers;
    let iq_start = 20usize;
    if buf.len() < iq_start + n_pairs * 2 {
        return None;
    }
    
    // Build an ndarray Array2<Complex64> shaped [n_antennas, n_subcarriers].
    let mut data = Array2::<Complex64>::zeros((n_antennas.max(1), n_subcarriers.max(1)));
    for s in 0..n_antennas {
    	for k in 0..n_subcarriers {
            let idx = s * n_subcarriers + k;
            let i_val = buf[iq_start + idx * 2] as i8 as f64;
            let q_val = buf[iq_start + idx * 2 + 1] as i8 as f64;
            data[[s, k]] = Complex64::new(i_val, q_val);
        }
    }   

    let band = if freq_mhz >= 5000 {
        FrequencyBand::Band5GHz
    } else {
        FrequencyBand::Band2_4GHz
    };
    let bw = tier_to_bw_mhz(tier);

    let mut meta = CsiMetadata::new(
        DeviceId::new(format!("esp32-node{}", node_id)),
        band,
        freq_mhz_to_channel(freq_mhz),
    );
    meta.bandwidth_mhz = bw;
    meta.rssi_dbm = rssi;
    meta.noise_floor_dbm = noise_floor;
    meta.antenna_config = AntennaConfig {
        tx_antennas: 1,
        rx_antennas: n_antennas as u8,
        spacing_mm: None,
    };
    meta.timestamp = Timestamp::now();

    Some(CsiFrame::new(meta, data))
}

/// Map a tier string to a bandwidth in MHz.
fn tier_to_bw_mhz(tier: &str) -> u16 {
    match tier.to_ascii_lowercase().as_str() {
        "ht40" | "he40" => 40,
        _ => 20,
    }
}

/// Rough 802.11 channel from centre frequency.
fn freq_mhz_to_channel(freq_mhz: u16) -> u8 {
    // 2.4 GHz: ch = (freq - 2407) / 5
    if freq_mhz < 3000 {
        ((freq_mhz.saturating_sub(2407)) / 5) as u8
    } else {
        // 5 GHz: ch = (freq - 5000) / 5
        ((freq_mhz.saturating_sub(5000)) / 5) as u8
    }
}

// ---------------------------------------------------------------------------
// Input validation
// ---------------------------------------------------------------------------

fn validate_args(args: &CalibrateArgs) -> Result<()> {
    if args.all_nodes && args.node_id.is_some() {
        bail!("--all-nodes cannot be combined with --node-id");
    }
    if args.duration_s < 10 {
        bail!(
            "--duration-s must be at least 10 s (got {}). \
             Fewer frames produce unreliable phase-concentration estimates (ADR-135 §2.3).",
            args.duration_s
        );
    }
    if args.duration_s > 300 {
        eprintln!(
            "[calibrate] WARN: --duration-s={} exceeds 300 s; this is unusual.",
            args.duration_s
        );
    }
    let valid = [
        "ht20",
        "ht40",
        "esp32_ht40_192",
        "esp32_ht40_128",
        "esp32_ht40_306",
        "he20",
        "he40",
    ];
    if !valid.contains(&args.tier.to_ascii_lowercase().as_str()) {
        bail!(
            "--tier must be one of {:?} (got {:?})",
            valid, args.tier
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_args_min_duration() {
        let mut args = default_args();
        args.duration_s = 5;
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_validate_args_ok() {
        let args = default_args();
        assert!(validate_args(&args).is_ok());
    }

    #[test]
    fn test_validate_args_bad_tier() {
        let mut args = default_args();
        args.tier = "ht80".into();
        assert!(validate_args(&args).is_err());
    }

    #[test]
    fn test_tier_config_ht20() {
        let cfg = tier_config("ht20");
        assert_eq!(cfg.num_active, 52);
    }

    #[test]
    fn test_tier_config_ht40() {
        let cfg = tier_config("ht40");
        assert_eq!(cfg.num_active, 114);
    }

    #[test]
    fn test_tier_config_esp32_ht40_306() {
        let cfg = tier_config("esp32_ht40_306");
        assert_eq!(cfg.num_subcarriers, 306);
        assert_eq!(cfg.num_active, 306);
    }

    #[test]
    fn test_tier_config_esp32_ht40_192() {
        let cfg = tier_config("esp32_ht40_192");
        assert_eq!(cfg.num_subcarriers, 192);
        assert_eq!(cfg.num_active, 192);
    }

    #[test]
    fn test_tier_config_he20() {
        let cfg = tier_config("he20");
        // Issue #1009 §1b: HE20 baseline records all 256 delivered bins
        // (no tone map in the recorder), not the 242 active tones.
        assert_eq!(cfg.num_active, 256);
    }

    #[test]
    fn test_parse_csi_packet_bad_magic() {
        let buf = vec![0u8; 32];
        assert!(parse_csi_packet(&buf, "ht20").is_none());
    }

    #[test]
    fn test_parse_csi_packet_too_short() {
        let buf = vec![0u8; 10];
        assert!(parse_csi_packet(&buf, "ht20").is_none());
    }

    /// Build an ADR-018 frame (correct firmware layout, ADR-110 bytes 18-19).
    fn build_frame(n_subcarriers: u16, ppdu: u8) -> Vec<u8> {
        let mut buf = vec![0u8; 20 + n_subcarriers as usize * 2];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        buf[4] = 12; // node_id
        buf[5] = 1; // n_antennas
        buf[6..8].copy_from_slice(&n_subcarriers.to_le_bytes());
        buf[8..12].copy_from_slice(&2432u32.to_le_bytes()); // freq_mhz
        buf[12..16].copy_from_slice(&11610u32.to_le_bytes()); // sequence
        buf[16] = (-40i8) as u8; // rssi
        buf[17] = (-87i8) as u8; // noise floor
        buf[18] = ppdu;
        buf[19] = 0x10; // time-sync valid
        for k in 0..n_subcarriers as usize {
            buf[20 + k * 2] = (10 + (k % 100) as i8) as u8;
            buf[20 + k * 2 + 1] = (k % 50) as u8;
        }
        buf
    }

    #[test]
    fn test_parse_csi_packet_valid() {
        let buf = build_frame(2, 0);
        let frame = parse_csi_packet(&buf, "ht20");
        assert!(frame.is_some());
        let f = frame.unwrap();
        assert_eq!(f.num_spatial_streams(), 1);
        assert_eq!(f.num_subcarriers(), 2);
        assert_eq!(f.metadata.rssi_dbm, -40);
        assert_eq!(f.metadata.noise_floor_dbm, -87);
    }

    #[test]
    fn test_parse_csi_packet_he_su_256_bins() {
        // ESP32-C6 HE-SU frame (issue #1005): n_subcarriers = 256 = 0x0100 LE.
        // The pre-#1005 single-byte read decoded this as 0 subcarriers.
        let buf = build_frame(256, 1);
        assert_eq!(buf.len(), 532); // matches the live wire size
        let f = parse_csi_packet(&buf, "he20").expect("256-bin HE frame must parse");
        assert_eq!(f.num_subcarriers(), 256);
        assert_eq!(f.metadata.rssi_dbm, -40);
        // A 256-bin frame is accepted by the he20 recorder (num_subcarriers
        // tier total) and rejected by ht20 (52/64) — no HT/HE mixing.
        let mut he = wifi_densepose_signal::CalibrationRecorder::new(tier_config("he20"));
        assert!(he.record(&f).is_ok());
        let mut ht = wifi_densepose_signal::CalibrationRecorder::new(tier_config("ht20"));
        assert!(ht.record(&f).is_err());
    }

    /// Security pin (review 2026-06, ADR-127): the UDP parser is the CLI's
    /// widest attack surface — `calibrate` / `enroll` / `room-watch` bind it to
    /// 0.0.0.0 by default, so any host on the LAN can send arbitrary bytes. A
    /// header that *claims* a huge `n_antennas * n_subcarriers` must be rejected
    /// by the length check BEFORE the `Array2::zeros` allocation, so a single
    /// small datagram can never trigger a multi-MB allocation (unbounded-memory
    /// DoS). The largest possible claim (255 × 65535 pairs ≈ 33 MB of IQ) inside
    /// a RECV_BUF-sized (2048-byte) datagram parses to `None`, never OOMs.
    #[test]
    fn test_parse_csi_packet_oversized_claim_is_rejected_not_allocated() {
        let mut buf = vec![0u8; RECV_BUF];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        buf[4] = 1; // node_id
        buf[5] = 255; // n_antennas (max)
        buf[6..8].copy_from_slice(&65535u16.to_le_bytes()); // n_subcarriers (max)
        buf[8..12].copy_from_slice(&2432u32.to_le_bytes());
        // n_pairs = 255 * 65535 = 16_711_425 → needs ~33 MB of IQ bytes that a
        // 2048-byte datagram cannot carry → length check fails → None.
        assert!(parse_csi_packet(&buf, "ht20").is_none());
    }

    /// Security pin (review 2026-06): the parser must never panic on ANY byte
    /// string — truncated headers, lying length fields, odd sizes. IQ-loop
    /// indexing is guarded by the length check; this sweeps a spread of
    /// adversarial inputs to lock in panic-on-adversarial-input = 0.
    #[test]
    fn test_parse_csi_packet_never_panics_on_arbitrary_bytes() {
        let mut st = 0x1234_5678u64;
        let mut next = move || {
            st = st
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (st >> 33) as u8
        };
        for len in 0..600usize {
            let buf: Vec<u8> = (0..len).map(|_| next()).collect();
            for tier in ["ht20", "he20", "garbage"] {
                let _ = parse_csi_packet(&buf, tier);
            }
        }
        // Valid magic, lying n_subcarriers, no payload → None (not a panic).
        let mut buf = vec![0u8; 20];
        buf[0..4].copy_from_slice(&0xC511_0001u32.to_le_bytes());
        buf[5] = 3;
        buf[6..8].copy_from_slice(&500u16.to_le_bytes());
        assert!(parse_csi_packet(&buf, "ht20").is_none());
    }

    #[test]
    fn test_freq_to_channel_24ghz() {
        assert_eq!(freq_mhz_to_channel(2437), 6);
    }

    #[test]
    fn test_freq_to_channel_5ghz() {
        assert_eq!(freq_mhz_to_channel(5180), 36);
    }

    fn default_args() -> CalibrateArgs {
        CalibrateArgs {
            udp_port: 5005,
            bind: "0.0.0.0".into(),
            duration_s: 30,
            node_id: None,
            all_nodes: false,
            output: "./baseline.bin".into(),
            output_dir: "./baselines".into(),
            tier: "ht20".into(),
            banner_every: 20,
            abort_z_threshold: 2.0,
            min_frames: 0,
        }
    }
}
