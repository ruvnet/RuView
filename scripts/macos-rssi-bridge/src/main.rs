//! macos-rssi-bridge
//!
//! Bridges a Mac's built-in WiFi card to the RuView sensing-server's UDP CSI
//! input. Spawns the `mac_wifi` Swift helper in `--watch` mode, reads its
//! JSON-lines output, maintains a per-BSSID RSSI ring buffer, and packs each
//! scan into an ESP32-format CSI frame (magic `0xC511_0001`) emitted over UDP.
//!
//! No CSI hardware is involved — RSSI from each visible AP is treated as a
//! pseudo-subcarrier amplitude. The sensing-server pipeline runs unmodified
//! against this synthetic CSI, giving you laptop-grade motion sensing while
//! you wait for ESP32 boards to arrive. See the v2/crates/wifi-densepose-wifiscan
//! crate (ADR-022) for the formal multi-AP sensing model.
//!
//! Wire format reproduced from
//! `v2/crates/wifi-densepose-sensing-server/src/csi.rs::parse_esp32_frame`:
//!
//!   bytes  0..4   u32 magic = 0xC511_0001 (LE)
//!   byte   4      u8  node_id
//!   byte   5      u8  n_antennas
//!   byte   6      u8  n_subcarriers
//!   byte   7      _   (skipped)
//!   bytes  8..10  u16 freq_mhz (LE)
//!   bytes 10..14  u32 sequence (LE)
//!   byte  14      i8  rssi
//!   byte  15      i8  noise_floor
//!   bytes 16..20  _   (header padding; iq_start = 20)
//!   bytes 20..    i8  I/Q pairs, n_antennas * n_subcarriers entries

use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::net::UdpSocket;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::Parser;
use serde::{Deserialize, Serialize};

const FRAME_MAGIC: u32 = 0xC511_0001;
const N_ANTENNAS: u8 = 1;
const N_SUBCARRIERS: u8 = 56;
const FRAME_HEADER_LEN: usize = 20;
const HISTORY_LEN: usize = 16;
/// Cap the AP set to keep things bounded and roughly aligned with the
/// `WindowsWifiPipeline` default of `max_bssids: 32`. Without real BSSIDs
/// the ordinal in the synthetic SSID can shuffle scan-to-scan, so the live
/// set drifts; capping prevents that drift from polluting downstream stats.
const MAX_APS: usize = 32;

#[derive(Parser, Debug)]
#[command(name = "macos-rssi-bridge", about, long_about = None)]
struct Args {
    /// Path to the compiled mac_wifi Swift helper.
    #[arg(long, default_value = "./mac_wifi")]
    helper: PathBuf,
    /// UDP target host for the sensing-server.
    #[arg(long, default_value = "127.0.0.1")]
    target_host: String,
    /// UDP target port (sensing-server default is 5005).
    #[arg(long, default_value_t = 5005)]
    target_port: u16,
    /// Seconds between active scans (the helper enforces a 0.5s floor).
    #[arg(long, default_value_t = 1.5)]
    interval: f64,
    /// node_id stamped into emitted frames.
    #[arg(long, default_value_t = 1)]
    node_id: u8,
    /// Print each emitted frame to stdout.
    #[arg(long)]
    verbose: bool,
    /// HTTP port for the per-AP state JSON + tomography dashboard.
    /// Set to 0 to disable.
    #[arg(long, default_value_t = 9090)]
    http_port: u16,
    /// Path to the static dashboard HTML to serve at GET /dashboard.
    /// Defaults to dashboard.html next to the binary.
    #[arg(long, default_value = "dashboard.html")]
    dashboard: PathBuf,
}

#[derive(Debug, Deserialize)]
struct ScanLine {
    ssid: String,
    #[allow(dead_code)]
    bssid: String,
    rssi: i32,
    noise: i32,
    channel: u16,
    band: String,
}

#[derive(Default, Debug, Clone)]
struct ApState {
    rssi_history: Vec<f32>,
    last_rssi: f32,
    last_noise: f32,
    channel: u16,
    band: String,
    last_seen: Option<Instant>,
}

impl ApState {
    fn record(&mut self, rssi: f32, noise: f32, channel: u16, band: &str) {
        self.rssi_history.push(rssi);
        if self.rssi_history.len() > HISTORY_LEN {
            self.rssi_history.remove(0);
        }
        self.last_rssi = rssi;
        self.last_noise = noise;
        self.channel = channel;
        if self.band.is_empty() {
            self.band = band.to_owned();
        }
        self.last_seen = Some(Instant::now());
    }

    /// Welford-ish variance over the rolling RSSI window.
    fn variance(&self) -> f32 {
        let n = self.rssi_history.len();
        if n < 2 {
            return 0.0;
        }
        let mean = self.rssi_history.iter().sum::<f32>() / n as f32;
        self.rssi_history
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f32>()
            / n as f32
    }
}

/// JSON shape returned from `GET /aps`. Consumed by the tomography dashboard.
#[derive(Debug, Serialize, Clone)]
struct ApSnapshot {
    /// Stable id (synthetic SSID or real one).
    id: String,
    channel: u16,
    band: String,
    rssi_dbm: f32,
    noise_dbm: f32,
    /// Rolling-window variance — high values mean a moving body is
    /// modulating the AP↔laptop link.
    variance: f32,
    /// Most recent N RSSI samples (N = HISTORY_LEN). Newest last.
    history: Vec<f32>,
    /// Milliseconds since this AP was last seen in a scan.
    age_ms: u64,
}

#[derive(Debug, Serialize, Clone, Default)]
struct StateSnapshot {
    /// Wall-clock timestamp in seconds since UNIX epoch.
    ts: f64,
    /// Sequence number of the most recent emitted CSI frame.
    seq: u32,
    /// Strongest AP's last RSSI in dBm (a coarse proxy for proximity).
    strongest_rssi_dbm: f32,
    /// Sorted-by-RSSI snapshot of every tracked AP.
    aps: Vec<ApSnapshot>,
}

/// Map a -100..-30 dBm RSSI value into a roughly full-range i8 magnitude.
/// We map dB linearly (1.27 LSB per dB) instead of using the linear
/// amplitude `10^((rssi+100)/20)` — that exponential saturates at the i8
/// ceiling for any AP stronger than ~-65 dBm, which is most modern indoor
/// environments. Linear-in-dB keeps the strongest APs informative.
fn rssi_to_i_byte(rssi: f32) -> i8 {
    ((rssi + 100.0) * 1.27).clamp(0.0, 127.0) as i8
}

/// Map per-BSSID RSSI variance into the Q channel — high variance = a
/// person modulating that AP's reflections. ~10 dB^2 lands near full-range.
fn variance_to_q_byte(var: f32) -> i8 {
    (var * 12.0).clamp(0.0, 127.0) as i8
}

fn build_frame(seq: u32, node_id: u8, aps: &[(String, &ApState)]) -> Vec<u8> {
    let mut buf = vec![0u8; FRAME_HEADER_LEN + 2 * N_ANTENNAS as usize * N_SUBCARRIERS as usize];

    buf[0..4].copy_from_slice(&FRAME_MAGIC.to_le_bytes());
    buf[4] = node_id;
    buf[5] = N_ANTENNAS;
    buf[6] = N_SUBCARRIERS;
    buf[8..10].copy_from_slice(&2437u16.to_le_bytes()); // 2.4 GHz channel 6 reference
    buf[10..14].copy_from_slice(&seq.to_le_bytes());

    let strongest = aps.iter().map(|(_, s)| s.last_rssi).fold(-127.0f32, f32::max);
    let avg_noise = if aps.is_empty() {
        -90.0
    } else {
        aps.iter().map(|(_, s)| s.last_noise).sum::<f32>() / aps.len() as f32
    };
    buf[14] = (strongest as i32).clamp(-127, 0) as i8 as u8;
    buf[15] = (avg_noise as i32).clamp(-127, 0) as i8 as u8;

    // Pack the AP set into 56 pseudo-subcarriers as a *block* layout (not
    // interleaved). Each AP gets a contiguous slab of subcarriers, so
    // adjacent subcarriers are perfectly correlated — this produces a
    // single coherent "observed body" pattern that the sensing-server's
    // mincut person counter (estimate_persons_from_correlation) reads as
    // one person instead of fragmenting our N visible APs into N synthetic
    // people. Cap to 8 APs so each slab is wide enough (≥7 subcarriers)
    // for the correlation window to register as one group.
    let n_aps = aps.len().max(1).min(8);
    let slab = N_SUBCARRIERS as usize / n_aps;
    let leftover = N_SUBCARRIERS as usize - slab * n_aps;
    for k in 0..N_SUBCARRIERS as usize {
        // Map subcarrier index to AP index via slab boundaries; the last
        // few subcarriers (leftover) cycle the strongest AP.
        let ap_idx = if k < slab * n_aps {
            (k / slab).min(n_aps - 1)
        } else {
            // tail subcarriers go to the strongest AP (index 0 after sort)
            0
        };
        let _ = leftover; // explicit: we handle it via the else-branch above
        let (_, st) = &aps[ap_idx];
        let i_off = FRAME_HEADER_LEN + k * 2;
        buf[i_off] = rssi_to_i_byte(st.last_rssi) as u8;
        buf[i_off + 1] = variance_to_q_byte(st.variance()) as u8;
    }

    buf
}

fn spawn_helper(helper: &PathBuf, interval: f64) -> std::io::Result<Child> {
    Command::new(helper)
        .arg("--watch")
        .arg("--interval")
        .arg(interval.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
}

/// Build a `StateSnapshot` from the current AP map (already RSSI-sorted).
fn build_snapshot(seq: u32, aps: &[(String, &ApState)]) -> StateSnapshot {
    let now = Instant::now();
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let strongest_rssi_dbm = aps.first().map(|(_, s)| s.last_rssi).unwrap_or(-100.0);
    let snaps: Vec<ApSnapshot> = aps
        .iter()
        .map(|(id, s)| ApSnapshot {
            id: id.clone(),
            channel: s.channel,
            band: s.band.clone(),
            rssi_dbm: s.last_rssi,
            noise_dbm: s.last_noise,
            variance: s.variance(),
            history: s.rssi_history.clone(),
            age_ms: s
                .last_seen
                .map(|t| now.duration_since(t).as_millis() as u64)
                .unwrap_or(u64::MAX),
        })
        .collect();
    StateSnapshot {
        ts,
        seq,
        strongest_rssi_dbm,
        aps: snaps,
    }
}

/// Lightweight HTTP server: GET /aps → JSON state, GET /dashboard → static
/// HTML, GET / → small index. Runs in its own thread, never blocks the
/// scanner. Permissive CORS so a dashboard.html opened via file:// can still
/// fetch /aps if the user prefers that path over the served version.
fn spawn_http_server(
    port: u16,
    dashboard_path: PathBuf,
    snapshot: Arc<Mutex<StateSnapshot>>,
) -> std::io::Result<()> {
    let server = tiny_http::Server::http(("0.0.0.0", port))
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::AddrInUse, e.to_string()))?;
    eprintln!("[bridge] http://127.0.0.1:{port}/dashboard ← tomography UI");
    eprintln!("[bridge] http://127.0.0.1:{port}/aps        ← per-AP state JSON");

    thread::spawn(move || {
        for req in server.incoming_requests() {
            let url = req.url().to_string();
            let path = url.split('?').next().unwrap_or("/");
            let (status, content_type, body): (u16, &str, Vec<u8>) = match path {
                "/aps" => {
                    let snap = snapshot.lock().expect("snapshot mutex poisoned").clone();
                    let body = serde_json::to_vec(&snap).unwrap_or_else(|_| b"{}".to_vec());
                    (200, "application/json", body)
                }
                "/dashboard" | "/dashboard/" => match std::fs::read(&dashboard_path) {
                    Ok(html) => (200, "text/html; charset=utf-8", html),
                    Err(e) => (
                        404,
                        "text/plain",
                        format!("dashboard.html not found at {dashboard_path:?}: {e}").into_bytes(),
                    ),
                },
                "/" => (
                    200,
                    "text/html; charset=utf-8",
                    b"<!doctype html><meta charset=utf-8><title>macos-rssi-bridge</title>\
                      <body style='font:14px system-ui;margin:2em'>\
                      <h1>macos-rssi-bridge</h1>\
                      <p><a href='/dashboard'>/dashboard</a> &mdash; live tomography UI</p>\
                      <p><a href='/aps'>/aps</a> &mdash; per-AP state JSON</p>\
                      </body>"
                        .to_vec(),
                ),
                _ => (404, "text/plain", b"not found".to_vec()),
            };
            let response = tiny_http::Response::from_data(body)
                .with_status_code(status)
                .with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes())
                        .expect("static header"),
                )
                .with_header(
                    tiny_http::Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..])
                        .expect("static header"),
                );
            let _ = req.respond(response);
        }
    });
    Ok(())
}

fn main() -> std::io::Result<()> {
    let args = Args::parse();

    let socket = UdpSocket::bind("0.0.0.0:0")?;
    let target = format!("{}:{}", args.target_host, args.target_port);
    socket.connect(&target)?;
    eprintln!("[bridge] sending CSI frames to udp://{}", target);

    // Shared snapshot consumed by the HTTP /aps endpoint. Starts empty;
    // populated on each emit tick. Mutex over a small struct — contention
    // is negligible at single-digit Hz.
    let snapshot = Arc::new(Mutex::new(StateSnapshot::default()));
    if args.http_port != 0 {
        // Resolve dashboard path relative to the binary if the user passed
        // the default — keeps `make run` working regardless of cwd.
        let mut dashboard_path = args.dashboard.clone();
        if dashboard_path.is_relative() && !dashboard_path.exists() {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let candidate = dir.join(&args.dashboard);
                    if candidate.exists() {
                        dashboard_path = candidate;
                    }
                }
            }
        }
        spawn_http_server(args.http_port, dashboard_path, Arc::clone(&snapshot))?;
    }

    let mut child = spawn_helper(&args.helper, args.interval)?;
    let stdout = child
        .stdout
        .take()
        .expect("helper stdout was piped at spawn time");
    let reader = BufReader::new(stdout);

    let running = Arc::new(AtomicBool::new(true));
    {
        let running = running.clone();
        let _ = ctrlc::set_handler(move || {
            running.store(false, Ordering::SeqCst);
        });
    }

    let mut aps: HashMap<String, ApState> = HashMap::new();
    let mut seq: u32 = 0;
    let mut last_emit = Instant::now() - Duration::from_secs(10);
    let emit_interval = Duration::from_millis(100);

    for line in reader.lines() {
        if !running.load(Ordering::SeqCst) {
            break;
        }
        let Ok(line) = line else { continue };
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let Ok(scan) = serde_json::from_str::<ScanLine>(line) else {
            continue;
        };
        let key = if scan.ssid.is_empty() {
            format!("ch{}_n{}", scan.channel, scan.rssi)
        } else {
            scan.ssid.clone()
        };
        aps.entry(key)
            .or_default()
            .record(scan.rssi as f32, scan.noise as f32, scan.channel, &scan.band);

        let now = Instant::now();
        if now.duration_since(last_emit) < emit_interval {
            continue;
        }
        last_emit = now;

        // Drop APs we haven't seen in 30s so the picture stays current.
        aps.retain(|_, s| s.last_seen.map_or(false, |t| now.duration_since(t) < Duration::from_secs(30)));
        if aps.is_empty() {
            continue;
        }

        // Sort by recent RSSI so the strongest APs lead the subcarrier
        // layout — they're the most informative — then cap to MAX_APS.
        let mut sorted: Vec<(String, &ApState)> =
            aps.iter().map(|(k, v)| (k.clone(), v)).collect();
        sorted.sort_by(|a, b| b.1.last_rssi.partial_cmp(&a.1.last_rssi).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(MAX_APS);

        seq = seq.wrapping_add(1);
        let frame = build_frame(seq, args.node_id, &sorted);
        // UDP sends can return ECONNREFUSED on macOS when the receiver
        // emits an ICMP unreachable. Log and continue — the receiver may
        // come back and we don't want a blip to kill the bridge.
        if let Err(e) = socket.send(&frame) {
            eprintln!("[bridge] udp send failed (continuing): {e}");
        }

        // Refresh the shared snapshot the HTTP /aps endpoint reads from.
        if let Ok(mut s) = snapshot.lock() {
            *s = build_snapshot(seq, &sorted);
        }
        if args.verbose {
            let strongest = sorted[0].1.last_rssi;
            let max_var = sorted.iter().map(|(_, s)| s.variance()).fold(0f32, f32::max);
            eprintln!(
                "[bridge] seq={} aps={:>2} strongest={:>4.0} dBm max_var={:>5.1}",
                seq, sorted.len(), strongest, max_var
            );
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    thread::sleep(Duration::from_millis(50));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rssi_mapping_baseline() {
        assert_eq!(rssi_to_i_byte(-100.0), 0);
        assert_eq!(rssi_to_i_byte(-30.0), 88);
        assert_eq!(rssi_to_i_byte(-50.0), 63);
        // Saturates cleanly past the calibrated range.
        assert_eq!(rssi_to_i_byte(0.0), 127);
        assert_eq!(rssi_to_i_byte(-200.0), 0);
    }

    #[test]
    fn variance_starts_at_zero() {
        let st = ApState::default();
        assert_eq!(st.variance(), 0.0);
    }

    #[test]
    fn variance_grows_with_jitter() {
        let mut st = ApState::default();
        for r in [-60.0, -64.0, -58.0, -65.0, -61.0, -67.0] {
            st.record(r, -90.0, 6);
        }
        assert!(st.variance() > 1.0);
    }

    #[test]
    fn frame_starts_with_magic() {
        let mut st = ApState::default();
        st.record(-50.0, -90.0, 6);
        let frame = build_frame(42, 1, &[("test".into(), &st)]);
        assert_eq!(&frame[0..4], &FRAME_MAGIC.to_le_bytes());
        assert_eq!(frame[5], N_ANTENNAS);
        assert_eq!(frame[6], N_SUBCARRIERS);
        assert_eq!(frame.len(), 20 + 2 * 1 * 56);
    }
}
