//! `wifi-densepose calibrate-serve` — HTTP API around ADR-135 baseline calibration.
//!
//! Wraps the same [`wifi_densepose_signal::CalibrationRecorder`] used by the
//! `calibrate` subcommand in a small Axum server so a UI (or any client) can
//! drive an empty-room baseline capture remotely:
//!
//! | Method | Path                              | Purpose                                   |
//! |--------|-----------------------------------|-------------------------------------------|
//! | GET    | `/`                               | API descriptor (discovery)                |
//! | GET    | `/api/v1/calibration/health`      | liveness + UDP ingest stats               |
//! | POST   | `/api/v1/calibration/start`       | begin a baseline capture session          |
//! | GET    | `/api/v1/calibration/status`      | live session progress (poll this for UI)  |
//! | POST   | `/api/v1/calibration/stop`        | finalize the current session early        |
//! | GET    | `/api/v1/calibration/result`      | summary of the last finalized baseline    |
//! | GET    | `/api/v1/calibration/baselines`   | list persisted baseline files             |
//!
//! A single background task owns the UDP socket (ESP32 `0xC511_0001` frames) and
//! the optional active recorder; the HTTP handlers communicate with it over an
//! mpsc command channel and read a shared status snapshot. This keeps the
//! `&mut` recorder lock-free and the API non-blocking. CORS is permissive so a
//! browser UI served from any origin can call it during development.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use clap::Args;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot, RwLock};
use tower_http::cors::CorsLayer;
use wifi_densepose_calibration::extract::{AnchorFeature, Features};
use wifi_densepose_calibration::{
    AnchorLabel, AnchorQualityGate, AnchorRecorder, MixtureOfSpecialists, NodeGeometry,
    SpecialistBank,
};
use wifi_densepose_core::types::CsiFrame;
use wifi_densepose_signal::{BaselineCalibration, CalibrationRecorder};

use crate::calibrate::{parse_csi_packet, tier_config};

/// Rolling window of per-frame scalars (mean amplitude) for live `room-state`
/// inference. Maintained by the ingest task regardless of any baseline session.
const LIVE_WINDOW: usize = 256;

/// One scalar per frame: mean amplitude across subcarriers/streams.
fn frame_scalar(frame: &CsiFrame) -> f32 {
    let a = &frame.amplitude;
    if a.is_empty() {
        0.0
    } else {
        (a.sum() / a.len() as f64) as f32
    }
}

const RECV_BUF: usize = 2048;

// ---------------------------------------------------------------------------
// CLI arguments
// ---------------------------------------------------------------------------

/// Arguments for the `calibrate-serve` subcommand.
#[derive(Args, Debug, Clone)]
pub struct CalibrateServeArgs {
    /// TCP port for the HTTP API.
    #[arg(long, default_value_t = 8090)]
    pub http_port: u16,

    /// Bind address for the HTTP API. Default 127.0.0.1 (localhost only);
    /// use 0.0.0.0 to expose the API to the LAN for a remote UI.
    #[arg(long, default_value = "127.0.0.1")]
    pub http_bind: String,

    /// UDP port to receive CSI frames from the ESP32 (must match provisioned target-port).
    #[arg(long, default_value_t = 5005)]
    pub udp_port: u16,

    /// Bind address for the UDP CSI socket.
    #[arg(long, default_value = "0.0.0.0")]
    pub udp_bind: String,

    /// Default PHY tier when a start request omits one (ht20 / ht40 / he20 / he40).
    #[arg(long, default_value = "ht20")]
    pub tier: String,

    /// Directory where finalized baseline `.bin` files are written.
    #[arg(long, default_value = "./baselines")]
    pub output_dir: String,

    /// Require `Authorization: Bearer <token>` on every API request. Strongly
    /// recommended before binding to anything other than 127.0.0.1.
    #[arg(long, env = "CALIBRATE_TOKEN")]
    pub token: Option<String>,
}

/// Sanitize a client-supplied `room_id` for use in a filename (defends the
/// baseline write path against `../` / absolute-path traversal). Keeps only
/// `[A-Za-z0-9_-]`; empty result falls back to `default`.
fn sanitize_room_id(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(64)
        .collect();
    if cleaned.is_empty() {
        "default".into()
    } else {
        cleaned
    }
}

// ---------------------------------------------------------------------------
// Wire types (request / response bodies)
// ---------------------------------------------------------------------------

/// Body for `POST /start`. All fields optional — sensible defaults applied.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct StartParams {
    /// PHY tier override (falls back to the server default).
    pub tier: Option<String>,
    /// Capture duration in seconds (also bounded by the tier's min-frame target).
    pub duration_s: u32,
    /// Optional room label, used in the persisted filename and status.
    pub room_id: Option<String>,
    /// Override the tier's minimum frame count (0 = use tier default).
    pub min_frames: u32,
}

impl Default for StartParams {
    fn default() -> Self {
        Self { tier: None, duration_s: 30, room_id: None, min_frames: 0 }
    }
}

/// Live per-session status snapshot returned by `GET /status`.
#[derive(Debug, Clone, Serialize)]
pub struct SessionStatus {
    /// `recording` | `finalizing` | `complete` | `aborted`.
    pub state: String,
    pub room_id: String,
    pub tier: String,
    pub frames_recorded: usize,
    pub target_frames: usize,
    /// 0.0..=1.0 capture progress.
    pub progress: f32,
    pub z_median: f32,
    pub z_max: f32,
    pub motion_flagged: bool,
    pub elapsed_s: f32,
    pub eta_s: f32,
    /// Optional human-readable note (e.g. abort reason).
    pub note: Option<String>,
}

/// Summary of a finalized baseline, returned by `GET /result` and `POST /stop`.
#[derive(Debug, Clone, Serialize)]
pub struct ResultSummary {
    pub calibration_id: String,
    pub room_id: String,
    pub tier: String,
    pub frame_count: u64,
    pub subcarriers: usize,
    pub captured_at_unix_s: i64,
    pub amp_mean_avg: f32,
    pub amp_variance_avg: f32,
    pub phase_dispersion_avg: f32,
    pub output_path: String,
    pub saved_bytes: usize,
}

/// Shared status the HTTP handlers read.
#[derive(Default)]
struct SharedStatus {
    udp_port: u16,
    default_tier: String,
    output_dir: String,
    frames_seen: u64,
    last_frame_unix_ms: u64,
    session: Option<SessionStatus>,
    last_result: Option<ResultSummary>,
}

/// Commands sent from HTTP handlers to the ingest task.
enum CalCommand {
    Start { params: StartParams, reply: oneshot::Sender<Result<SessionStatus, String>> },
    Stop { reply: oneshot::Sender<Result<ResultSummary, String>> },
    EnrollAnchor {
        room_id: String,
        baseline_name: String,
        label: AnchorLabel,
        duration_s: u32,
        reply: oneshot::Sender<Result<AnchorVerdict, String>>,
    },
}

/// Accumulated in-server enrollment for one room (not persisted until train).
#[derive(Default)]
struct RoomEnroll {
    baseline_id: String,
    fs_hz: f32,
    anchors: Vec<AnchorFeature>,
    /// Transceiver geometry recorded via `POST /enroll/geometry` (ADR-152
    /// §2.1.1); latest recording wins. Snapshotted into the bank at train time.
    geometry: Vec<NodeGeometry>,
}

/// Result of capturing one anchor (`POST /enroll/anchor`).
#[derive(Debug, Clone, Serialize)]
pub struct AnchorVerdict {
    /// Anchor label (snake_case).
    pub label: String,
    /// Passed the quality gate.
    pub accepted: bool,
    /// Rejection reason, if any.
    pub reason: Option<String>,
    /// Mean amplitude z-score vs baseline.
    pub presence_z: f32,
    /// Fraction of frames flagged as motion.
    pub motion_rate: f32,
    /// Frames captured.
    pub frames: u32,
    /// Accepted anchors so far for this room.
    pub accepted_count: usize,
    /// Next anchor in the sequence, if any.
    pub next: Option<String>,
}

/// In-flight anchor capture owned by the ingest task.
struct EnrollCapture {
    recorder: AnchorRecorder,
    baseline: BaselineCalibration,
    label: AnchorLabel,
    room_id: String,
    baseline_id: String,
    fs_hz: f32,
    series: Vec<f32>,
    deadline: Instant,
    reply: Option<oneshot::Sender<Result<AnchorVerdict, String>>>,
}

#[derive(Clone)]
struct ApiState {
    cmd_tx: mpsc::Sender<CalCommand>,
    status: Arc<RwLock<SharedStatus>>,
    /// Rolling per-frame scalars for live `room-state` inference.
    window: Arc<RwLock<VecDeque<f32>>>,
    /// Default sample rate for periodicity extraction.
    fs_hz: f32,
    /// In-server enrollment accumulator, keyed by `room_id`.
    enroll: Arc<RwLock<HashMap<String, RoomEnroll>>>,
}

/// Bearer-token gate (applied only when `--token` is set). Constant-time-ish
/// compare is unnecessary here (local appliance), but reject anything that
/// isn't an exact `Bearer <token>` match.
async fn require_bearer(
    axum::extract::State(token): axum::extract::State<String>,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let authorized = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(|t| t == token)
        .unwrap_or(false);
    if authorized {
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({"error": "missing or invalid bearer token"})),
        )
            .into_response()
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Build the API router (without the optional auth layer). Shared by `execute`
/// and the integration tests.
fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/", get(descriptor))
        .route("/api/v1/calibration/health", get(health))
        .route("/api/v1/calibration/start", post(start))
        .route("/api/v1/calibration/status", get(status_handler))
        .route("/api/v1/calibration/stop", post(stop))
        .route("/api/v1/calibration/result", get(result))
        .route("/api/v1/calibration/baselines", get(baselines))
        .route("/api/v1/room/state", get(room_state))
        .route("/api/v1/room/train", post(train_room))
        .route("/api/v1/enroll/anchor", post(enroll_anchor))
        .route("/api/v1/enroll/geometry", post(enroll_geometry))
        .route("/api/v1/enroll/status", get(enroll_status))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Run the calibration HTTP API server (blocks until Ctrl-C).
pub async fn execute(args: CalibrateServeArgs) -> Result<()> {
    std::fs::create_dir_all(&args.output_dir)
        .map_err(|e| anyhow::anyhow!("cannot create output dir {}: {e}", args.output_dir))?;

    let udp_addr = format!("{}:{}", args.udp_bind, args.udp_port);
    let socket = UdpSocket::bind(&udp_addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind UDP socket on {udp_addr}: {e}"))?;
    eprintln!("[calibrate-serve] CSI ingest on udp://{udp_addr}");

    let status = Arc::new(RwLock::new(SharedStatus {
        udp_port: args.udp_port,
        default_tier: args.tier.clone(),
        output_dir: args.output_dir.clone(),
        ..Default::default()
    }));

    let (cmd_tx, cmd_rx) = mpsc::channel::<CalCommand>(8);
    let window = Arc::new(RwLock::new(VecDeque::<f32>::with_capacity(LIVE_WINDOW)));
    let enroll = Arc::new(RwLock::new(HashMap::<String, RoomEnroll>::new()));

    // Background ingest task owns the socket + recorder.
    {
        let status = status.clone();
        let default_tier = args.tier.clone();
        let output_dir = args.output_dir.clone();
        let window = window.clone();
        let enroll = enroll.clone();
        tokio::spawn(async move {
            ingest_loop(socket, cmd_rx, status, default_tier, output_dir, window, enroll).await;
        });
    }

    let state = ApiState { cmd_tx, status, window, fs_hz: 15.0, enroll };
    let mut app = build_router(state);

    // Optional bearer auth — required before any non-loopback exposure.
    if let Some(token) = args.token.clone() {
        app = app.layer(axum::middleware::from_fn_with_state(token, require_bearer));
        eprintln!("[calibrate-serve] bearer auth ENABLED");
    } else if args.http_bind != "127.0.0.1" && args.http_bind != "localhost" {
        eprintln!(
            "[calibrate-serve] WARNING: bound to {} with NO --token — anyone on the network can drive calibration",
            args.http_bind
        );
    }

    let http_addr = format!("{}:{}", args.http_bind, args.http_port);
    let listener = tokio::net::TcpListener::bind(&http_addr)
        .await
        .map_err(|e| anyhow::anyhow!("cannot bind HTTP listener on {http_addr}: {e}"))?;
    eprintln!("[calibrate-serve] HTTP API on http://{http_addr}  (GET / for the route list)");

    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("HTTP server error: {e}"))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Ingest task — owns the UDP socket and the optional active recorder
// ---------------------------------------------------------------------------

struct ActiveSession {
    recorder: CalibrationRecorder,
    room_id: String,
    tier: String,
    started: Instant,
    deadline: Instant,
    target_frames: usize,
    z_median: f32,
    z_max: f32,
    motion_flagged: bool,
}

async fn ingest_loop(
    socket: UdpSocket,
    mut cmd_rx: mpsc::Receiver<CalCommand>,
    status: Arc<RwLock<SharedStatus>>,
    default_tier: String,
    output_dir: String,
    window: Arc<RwLock<VecDeque<f32>>>,
    enroll: Arc<RwLock<HashMap<String, RoomEnroll>>>,
) {
    let mut buf = vec![0u8; RECV_BUF];
    let mut active: Option<ActiveSession> = None;
    let mut active_enroll: Option<EnrollCapture> = None;
    let mut tick = tokio::time::interval(Duration::from_millis(200));
    // Counters mirrored to shared status only on the 200 ms tick — avoids a lock
    // + SessionStatus clone on every UDP frame (CPU starvation under flood).
    let mut frames_seen: u64 = 0;
    let mut last_frame_ms: u64 = 0;
    // Live rolling window, flushed to the shared `window` on the tick.
    let mut win_local: VecDeque<f32> = VecDeque::with_capacity(LIVE_WINDOW);

    loop {
        tokio::select! {
            // --- incoming command ---
            Some(cmd) = cmd_rx.recv() => match cmd {
                CalCommand::Start { params, reply } => {
                    if active.is_some() {
                        let _ = reply.send(Err("a calibration session is already running".into()));
                        continue;
                    }
                    let tier = params.tier.unwrap_or_else(|| default_tier.clone());
                    if !["ht20", "ht40", "he20", "he40"].contains(&tier.to_ascii_lowercase().as_str()) {
                        let _ = reply.send(Err(format!("invalid tier {tier:?}")));
                        continue;
                    }
                    let mut config = tier_config(&tier);
                    if params.min_frames > 0 {
                        config.min_frames = params.min_frames;
                    }
                    let target_frames = config.min_frames as usize;
                    let dur = params.duration_s.max(1) as u64;
                    // Sanitize: room_id is interpolated into the baseline write path.
                    let room_id = sanitize_room_id(&params.room_id.unwrap_or_else(|| "default".into()));
                    let sess = ActiveSession {
                        recorder: CalibrationRecorder::new(config),
                        room_id: room_id.clone(),
                        tier: tier.clone(),
                        started: Instant::now(),
                        deadline: Instant::now() + Duration::from_secs(dur),
                        target_frames,
                        z_median: 0.0,
                        z_max: 0.0,
                        motion_flagged: false,
                    };
                    let snap = session_snapshot(&sess, "recording", None);
                    active = Some(sess);
                    {
                        let mut s = status.write().await;
                        s.session = Some(snap.clone());
                        s.last_result = None;
                    }
                    eprintln!("[calibrate-serve] session start room={room_id} tier={tier} target={target_frames}");
                    let _ = reply.send(Ok(snap));
                }
                CalCommand::Stop { reply } => {
                    match active.take() {
                        Some(sess) => {
                            let res = finalize(sess, &output_dir, &status).await;
                            let _ = reply.send(res);
                        }
                        None => { let _ = reply.send(Err("no active calibration session".into())); }
                    }
                }
                CalCommand::EnrollAnchor { room_id, baseline_name, label, duration_s, reply } => {
                    if active.is_some() || active_enroll.is_some() {
                        let _ = reply.send(Err("a capture is already running".into()));
                        continue;
                    }
                    // Resolve the baseline as a sanitized name under output_dir.
                    let bname = sanitize_room_id(&baseline_name);
                    let bpath = format!("{output_dir}/{bname}.bin");
                    let baseline = match tokio::fs::read(&bpath).await {
                        Ok(bytes) => match BaselineCalibration::from_bytes(&bytes) {
                            Ok(b) => b,
                            Err(e) => { let _ = reply.send(Err(format!("invalid baseline {bname}: {e}"))); continue; }
                        },
                        Err(e) => { let _ = reply.send(Err(format!("baseline {bname} not found: {e}"))); continue; }
                    };
                    let baseline_id = baseline.calibration_uuid().to_string();
                    eprintln!("[calibrate-serve] enroll anchor room={room_id} label={} ({}s)", label.as_str(), duration_s);
                    active_enroll = Some(EnrollCapture {
                        recorder: AnchorRecorder::new(label),
                        baseline,
                        label,
                        room_id,
                        baseline_id,
                        fs_hz: 15.0,
                        series: Vec::new(),
                        deadline: Instant::now() + Duration::from_secs(duration_s.max(1) as u64),
                        reply: Some(reply),
                    });
                }
            },

            // --- incoming CSI frame (no shared-status lock here; flushed on tick) ---
            Ok(n) = socket.recv(&mut buf) => {
                frames_seen += 1;
                last_frame_ms = unix_ms();
                let parse_tier = active.as_ref().map(|s| s.tier.clone()).unwrap_or_else(|| default_tier.clone());
                if let Some(frame) = parse_csi_packet(&buf[..n], &parse_tier) {
                    // Always maintain the live window (drives /room/state).
                    win_local.push_back(frame_scalar(&frame));
                    while win_local.len() > LIVE_WINDOW {
                        win_local.pop_front();
                    }
                    if let Some(sess) = active.as_mut() {
                        if let Ok(score) = sess.recorder.record(&frame) {
                            sess.z_median = score.amplitude_z_median;
                            sess.z_max = score.amplitude_z_max;
                            sess.motion_flagged = score.motion_flagged;
                        }
                        if sess.recorder.frames_recorded() as usize >= sess.target_frames {
                            if let Some(done) = active.take() {
                                let _ = finalize(done, &output_dir, &status).await;
                            }
                        }
                    }
                    if let Some(ec) = active_enroll.as_mut() {
                        ec.recorder.record_frame(&ec.baseline, &frame);
                        ec.series.push(frame_scalar(&frame));
                    }
                }
            },

            // --- 200 ms tick: flush counters + window + session snapshot, deadline check ---
            _ = tick.tick() => {
                {
                    let mut s = status.write().await;
                    s.frames_seen = frames_seen;
                    s.last_frame_unix_ms = last_frame_ms;
                    if let Some(sess) = active.as_ref() {
                        s.session = Some(session_snapshot(sess, "recording", None));
                    }
                }
                {
                    let mut w = window.write().await;
                    w.clear();
                    w.extend(win_local.iter().copied());
                }
                if let Some(sess) = active.as_ref() {
                    if Instant::now() >= sess.deadline {
                        let frames = sess.recorder.frames_recorded() as usize;
                        if frames >= sess.target_frames {
                            if let Some(done) = active.take() {
                                let _ = finalize(done, &output_dir, &status).await;
                            }
                        } else if let Some(mut done) = active.take() {
                            // not enough frames — abort honestly rather than emit a bad baseline
                            done.motion_flagged = false;
                            let note = format!(
                                "aborted: only {frames} frames in the time window (need >= {}) — \
                                 is the ESP32 streaming to udp:{}? ",
                                done.target_frames,
                                status.read().await.udp_port
                            );
                            let snap = session_snapshot(&done, "aborted", Some(note.clone()));
                            status.write().await.session = Some(snap);
                            eprintln!("[calibrate-serve] {note}");
                        }
                    }
                }
                // Enroll-anchor capture finished?
                let enroll_done = active_enroll.as_ref().map(|ec| Instant::now() >= ec.deadline).unwrap_or(false);
                if enroll_done {
                    if let Some(mut ec) = active_enroll.take() {
                        let gate = AnchorQualityGate::default();
                        let (anchor, reason) = ec.recorder.finalize(&gate, (unix_ms() / 1000) as i64);
                        let mut verdict = AnchorVerdict {
                            label: ec.label.as_str().into(),
                            accepted: anchor.quality.accepted,
                            reason,
                            presence_z: anchor.quality.presence_z,
                            motion_rate: anchor.quality.motion_rate,
                            frames: anchor.quality.frames,
                            accepted_count: 0,
                            next: None,
                        };
                        if anchor.quality.accepted {
                            let feat = AnchorFeature::from_series(&ec.room_id, ec.label, &ec.series, ec.fs_hz);
                            let mut map = enroll.write().await;
                            let re = map.entry(ec.room_id.clone()).or_insert_with(RoomEnroll::default);
                            if re.baseline_id.is_empty() {
                                re.baseline_id = ec.baseline_id.clone();
                                re.fs_hz = ec.fs_hz;
                            }
                            if let Some(slot) = re.anchors.iter_mut().find(|a| a.label == ec.label) {
                                *slot = feat;
                            } else {
                                re.anchors.push(feat);
                            }
                            verdict.accepted_count = re.anchors.len();
                            verdict.next = AnchorLabel::SEQUENCE.iter().copied()
                                .find(|l| !re.anchors.iter().any(|a| a.label == *l))
                                .map(|l| l.as_str().to_string());
                        } else {
                            verdict.accepted_count = enroll.read().await.get(&ec.room_id).map(|re| re.anchors.len()).unwrap_or(0);
                        }
                        eprintln!("[calibrate-serve] enroll anchor {} accepted={} ({} total)", verdict.label, verdict.accepted, verdict.accepted_count);
                        if let Some(tx) = ec.reply.take() {
                            let _ = tx.send(Ok(verdict));
                        }
                    }
                }
            },
        }
    }
}

/// Finalize a session: persist the baseline and publish the result summary.
async fn finalize(
    sess: ActiveSession,
    output_dir: &str,
    status: &Arc<RwLock<SharedStatus>>,
) -> Result<ResultSummary, String> {
    let room_id = sess.room_id.clone();
    let tier = sess.tier.clone();
    // mark finalizing
    {
        let snap = session_snapshot(&sess, "finalizing", None);
        status.write().await.session = Some(snap);
    }

    // `CalibrationRecorder::finalize` consumes the recorder, so retain a
    // terminal snapshot before attempting the fallible operation.
    let mut aborted_snapshot = session_snapshot(&sess, "aborted", None);
    let baseline: BaselineCalibration = match sess.recorder.finalize() {
        Ok(baseline) => baseline,
        Err(e) => {
            let message = format!("finalize failed: {e}");
            let note = format!("aborted: {message}");
            aborted_snapshot.note = Some(note.clone());
            status.write().await.session = Some(aborted_snapshot);
            eprintln!("[calibrate-serve] {note}");
            return Err(message);
        }
    };

    let (amp_mean_avg, amp_var_avg, disp_avg) = baseline_averages(&baseline);
    let uuid = baseline.calibration_uuid().to_string();
    let path = format!("{output_dir}/{room_id}-{uuid}.bin");
    let bytes = baseline.to_bytes();
    // Async write — never block the ingest task's UDP/command path.
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|e| format!("cannot write {path}: {e}"))?;

    let summary = ResultSummary {
        calibration_id: uuid,
        room_id: room_id.clone(),
        tier,
        frame_count: baseline.frame_count,
        subcarriers: baseline.subcarriers.len(),
        captured_at_unix_s: baseline.captured_at_unix_s,
        amp_mean_avg,
        amp_variance_avg: amp_var_avg,
        phase_dispersion_avg: disp_avg,
        output_path: path.clone(),
        saved_bytes: bytes.len(),
    };

    {
        let mut s = status.write().await;
        // reflect completion in the session snapshot, then store the result
        if let Some(sess_status) = s.session.as_mut() {
            sess_status.state = "complete".into();
            sess_status.progress = 1.0;
        }
        s.last_result = Some(summary.clone());
    }
    eprintln!(
        "[calibrate-serve] session complete room={room_id} frames={} -> {path} ({} bytes)",
        summary.frame_count, summary.saved_bytes
    );
    Ok(summary)
}

// ---------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------

async fn descriptor() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "wifi-densepose calibration API",
        "adr": "ADR-135 (baseline) / ADR-151 (room calibration & training)",
        "endpoints": {
            "GET  /api/v1/calibration/health": "liveness + UDP ingest stats",
            "POST /api/v1/calibration/start": "{ tier?, duration_s?, room_id?, min_frames? }",
            "GET  /api/v1/calibration/status": "live session progress (poll for UI)",
            "POST /api/v1/calibration/stop": "finalize current session early",
            "GET  /api/v1/calibration/result": "last finalized baseline summary",
            "GET  /api/v1/calibration/baselines": "list persisted baseline files",
            "GET  /api/v1/room/state?bank=<name>": "live mixture-of-specialists RoomState over the CSI window",
            "POST /api/v1/room/train": "{ room_id, baseline_id, anchors[]?, geometry[]? } → train + persist a specialist bank (anchors[]/geometry[] optional if enrolled in-server)",
            "POST /api/v1/enroll/anchor": "{ room_id, baseline, label, duration_s? } → capture one guided anchor (blocks for the capture)",
            "POST /api/v1/enroll/geometry": "{ room_id, geometry: [NodeGeometry…] } → record transceiver geometry for the room (ADR-152 §2.1.1; latest wins)",
            "GET  /api/v1/enroll/status?room=<id>": "enrollment progress (accepted anchors, next, complete)"
        }
    }))
}

async fn health(State(st): State<ApiState>) -> impl IntoResponse {
    let s = st.status.read().await;
    let age = if s.last_frame_unix_ms == 0 { None } else { Some(unix_ms().saturating_sub(s.last_frame_unix_ms)) };
    Json(serde_json::json!({
        "status": "ok",
        "udp_port": s.udp_port,
        "frames_seen": s.frames_seen,
        "last_frame_age_ms": age,
        "streaming": age.map(|a| a < 2000).unwrap_or(false),
        "default_tier": s.default_tier,
        "output_dir": s.output_dir,
        "session_active": s.session.as_ref().map(|x| x.state == "recording").unwrap_or(false),
    }))
}

async fn start(State(st): State<ApiState>, Json(params): Json<StartParams>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    if st.cmd_tx.send(CalCommand::Start { params, reply: tx }).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"ingest task unavailable"}))).into_response();
    }
    match rx.await {
        Ok(Ok(snap)) => (StatusCode::ACCEPTED, Json(serde_json::to_value(snap).unwrap())).into_response(),
        Ok(Err(e)) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"no reply"}))).into_response(),
    }
}

async fn status_handler(State(st): State<ApiState>) -> impl IntoResponse {
    let s = st.status.read().await;
    match &s.session {
        Some(sess) => (StatusCode::OK, Json(serde_json::to_value(sess).unwrap())).into_response(),
        None => (StatusCode::OK, Json(serde_json::json!({"state":"idle"}))).into_response(),
    }
}

async fn stop(State(st): State<ApiState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    if st.cmd_tx.send(CalCommand::Stop { reply: tx }).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"ingest task unavailable"}))).into_response();
    }
    match rx.await {
        Ok(Ok(summary)) => (StatusCode::OK, Json(serde_json::to_value(summary).unwrap())).into_response(),
        Ok(Err(e)) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"no reply"}))).into_response(),
    }
}

async fn result(State(st): State<ApiState>) -> impl IntoResponse {
    let s = st.status.read().await;
    match &s.last_result {
        Some(r) => (StatusCode::OK, Json(serde_json::to_value(r).unwrap())).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error":"no finalized baseline yet"}))).into_response(),
    }
}

/// Body for `POST /api/v1/room/train` — an enrollment (CLI `enroll` output or
/// any client that gathered labelled anchor features).
#[derive(Deserialize)]
struct TrainRequest {
    room_id: String,
    baseline_id: String,
    #[serde(default)]
    anchors: Vec<AnchorFeature>,
    /// Optional transceiver geometry (ADR-152 §2.1.1). Falls back to the
    /// geometry recorded in-server via `POST /enroll/geometry`; absent both,
    /// the bank trains geometry-free (valid, but no geometry conditioning).
    #[serde(default)]
    geometry: Vec<NodeGeometry>,
}

/// Train a per-room specialist bank and persist it as `<output_dir>/<room_id>.json`
/// (the name `room-state` reads back). Uses the posted `anchors` if present, else
/// falls back to the in-server enrollment accumulated via `POST /enroll/anchor`.
/// The enrollment's transceiver-geometry snapshot (posted `geometry` or the
/// `POST /enroll/geometry` record) is threaded into the bank (ADR-152 §2.1.1).
async fn train_room(State(st): State<ApiState>, Json(req): Json<TrainRequest>) -> impl IntoResponse {
    let (anchors, baseline_id) = if !req.anchors.is_empty() {
        (req.anchors.clone(), req.baseline_id.clone())
    } else {
        match st.enroll.read().await.get(&req.room_id) {
            Some(re) if !re.anchors.is_empty() => (re.anchors.clone(), re.baseline_id.clone()),
            _ => {
                return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"no anchors in request and none enrolled for this room"}))).into_response();
            }
        }
    };
    let geometry = if !req.geometry.is_empty() {
        req.geometry.clone()
    } else {
        st.enroll.read().await.get(&req.room_id).map(|re| re.geometry.clone()).unwrap_or_default()
    };
    let at = (unix_ms() / 1000) as i64;
    let bank = match SpecialistBank::train(&req.room_id, &baseline_id, &anchors, at) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("training failed: {e}")}))).into_response(),
    };
    let bank = if geometry.is_empty() {
        eprintln!(
            "[calibrate-serve] no transceiver geometry recorded for room '{}' — bank will not support geometry conditioning (ADR-152 §2.1.2)",
            req.room_id
        );
        bank
    } else {
        bank.with_geometry(geometry)
    };
    let name = sanitize_room_id(&req.room_id);
    let dir = { st.status.read().await.output_dir.clone() };
    let path = format!("{dir}/{name}.json");
    let json = match bank.to_json() {
        Ok(j) => j,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("serialize: {e}")}))).into_response(),
    };
    if let Err(e) = tokio::fs::write(&path, json).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("cannot write {path}: {e}")}))).into_response();
    }
    let kinds: Vec<String> = bank.trained_kinds().iter().map(|k| format!("{k:?}")).collect();
    (StatusCode::OK, Json(serde_json::json!({
        "room_id": bank.room_id,
        "bank": name,                  // pass as ?bank=<name> to /room/state
        "anchor_count": bank.anchor_count,
        "specialists": kinds,
        "geometry_nodes": bank.geometry.len(),
        "path": path,
    }))).into_response()
}

/// Body for `POST /api/v1/enroll/geometry`.
#[derive(Deserialize)]
struct EnrollGeometryBody {
    room_id: String,
    /// Per-node transceiver geometry records (ADR-152 §2.1.1).
    geometry: Vec<NodeGeometry>,
}

/// Record the room's transceiver geometry (ADR-152 §2.1.1) into the in-server
/// enrollment; the next `POST /room/train` snapshots it into the bank. A later
/// POST supersedes an earlier one (latest wins), mirroring
/// `EnrollmentSession::record_geometry`.
async fn enroll_geometry(State(st): State<ApiState>, Json(b): Json<EnrollGeometryBody>) -> impl IntoResponse {
    if b.geometry.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error":"geometry must be a non-empty array of NodeGeometry records"}))).into_response();
    }
    let nodes = b.geometry.len();
    {
        let mut map = st.enroll.write().await;
        let re = map.entry(b.room_id.clone()).or_insert_with(RoomEnroll::default);
        re.geometry = b.geometry;
    }
    eprintln!("[calibrate-serve] enroll geometry room={} nodes={nodes}", b.room_id);
    (StatusCode::OK, Json(serde_json::json!({"room_id": b.room_id, "geometry_nodes": nodes}))).into_response()
}

/// Body for `POST /api/v1/enroll/anchor`.
#[derive(Deserialize)]
struct EnrollAnchorBody {
    room_id: String,
    /// Baseline name (without `.bin`), resolved under `output_dir`.
    baseline: String,
    /// Anchor label (snake_case, e.g. `stand_still`).
    label: String,
    /// Capture duration (s); defaults to the anchor's recommended length.
    duration_s: Option<u32>,
}

/// Capture one guided anchor against a baseline. Blocks for the capture
/// duration, then returns the gate verdict (accept/reject + progress).
async fn enroll_anchor(State(st): State<ApiState>, Json(b): Json<EnrollAnchorBody>) -> impl IntoResponse {
    let label = match AnchorLabel::from_str(&b.label) {
        Some(l) => l,
        None => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("unknown anchor label {:?}", b.label)}))).into_response(),
    };
    let duration_s = b.duration_s.unwrap_or_else(|| label.duration_s());
    let (tx, rx) = oneshot::channel();
    let cmd = CalCommand::EnrollAnchor {
        room_id: b.room_id,
        baseline_name: b.baseline,
        label,
        duration_s,
        reply: tx,
    };
    if st.cmd_tx.send(cmd).await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"ingest task unavailable"}))).into_response();
    }
    match rx.await {
        Ok(Ok(v)) => (StatusCode::OK, Json(serde_json::to_value(v).unwrap())).into_response(),
        Ok(Err(e)) => (StatusCode::CONFLICT, Json(serde_json::json!({"error": e}))).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"no reply"}))).into_response(),
    }
}

/// Query for `GET /api/v1/enroll/status`.
#[derive(Deserialize)]
struct EnrollStatusQuery {
    room: String,
}

/// Enrollment progress for a room.
async fn enroll_status(State(st): State<ApiState>, Query(q): Query<EnrollStatusQuery>) -> impl IntoResponse {
    let map = st.enroll.read().await;
    let (accepted, baseline_id): (Vec<String>, String) = match map.get(&q.room) {
        Some(re) => (
            re.anchors.iter().map(|a| a.label.as_str().to_string()).collect(),
            re.baseline_id.clone(),
        ),
        None => (Vec::new(), String::new()),
    };
    let next = AnchorLabel::SEQUENCE
        .iter()
        .copied()
        .find(|l| !accepted.iter().any(|a| a == l.as_str()))
        .map(|l| l.as_str().to_string());
    Json(serde_json::json!({
        "room": q.room,
        "baseline_id": baseline_id,
        "accepted": accepted,
        "count": accepted.len(),
        "total": AnchorLabel::SEQUENCE.len(),
        "next": next,
        "complete": next.is_none() && !accepted.is_empty(),
    }))
}

/// Query for `GET /api/v1/room/state`.
#[derive(Deserialize)]
struct RoomStateQuery {
    /// Bank name (sanitized; resolved as `<output_dir>/<bank>.json`).
    bank: String,
    /// Sample rate override (Hz).
    fs: Option<f32>,
}

/// Live mixture-of-specialists readout over the current CSI window.
async fn room_state(State(st): State<ApiState>, Query(q): Query<RoomStateQuery>) -> impl IntoResponse {
    // Resolve the bank as a sanitized name under output_dir — no arbitrary file read.
    let name = sanitize_room_id(&q.bank);
    let dir = { st.status.read().await.output_dir.clone() };
    let path = format!("{dir}/{name}.json");
    let raw = match tokio::fs::read_to_string(&path).await {
        Ok(r) => r,
        Err(e) => {
            return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": format!("bank '{name}' not found: {e}")}))).into_response();
        }
    };
    let bank = match SpecialistBank::from_json(&raw) {
        Ok(b) => b,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("invalid bank: {e}")}))).into_response(),
    };

    let series: Vec<f32> = { st.window.read().await.iter().copied().collect() };
    if series.len() < 32 {
        return (StatusCode::OK, Json(serde_json::json!({"state":"warming_up","frames":series.len()}))).into_response();
    }
    let fs = q.fs.unwrap_or(st.fs_hz);
    let features = Features::from_series(&series, fs);
    let baseline_id = bank.baseline_id.clone();
    let mix = MixtureOfSpecialists::new(bank);
    let room = mix.infer(&features, &baseline_id);
    (StatusCode::OK, Json(serde_json::to_value(room).unwrap())).into_response()
}

async fn baselines(State(st): State<ApiState>) -> impl IntoResponse {
    let dir = { st.status.read().await.output_dir.clone() };
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("bin") {
                let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
                out.push(serde_json::json!({
                    "file": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                    "path": path.to_string_lossy(),
                    "bytes": bytes,
                }));
            }
        }
    }
    Json(serde_json::json!({ "dir": dir, "baselines": out }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn session_snapshot(sess: &ActiveSession, state: &str, note: Option<String>) -> SessionStatus {
    let frames = sess.recorder.frames_recorded() as usize;
    let progress = if sess.target_frames == 0 {
        0.0
    } else {
        (frames as f32 / sess.target_frames as f32).clamp(0.0, 1.0)
    };
    let elapsed = sess.started.elapsed().as_secs_f32();
    let eta = if frames == 0 {
        sess.deadline.saturating_duration_since(Instant::now()).as_secs_f32()
    } else {
        let per = elapsed / frames as f32;
        (per * (sess.target_frames.saturating_sub(frames)) as f32).max(0.0)
    };
    SessionStatus {
        state: state.into(),
        room_id: sess.room_id.clone(),
        tier: sess.tier.clone(),
        frames_recorded: frames,
        target_frames: sess.target_frames,
        progress,
        z_median: sess.z_median,
        z_max: sess.z_max,
        motion_flagged: sess.motion_flagged,
        elapsed_s: elapsed,
        eta_s: eta,
        note,
    }
}

fn baseline_averages(b: &BaselineCalibration) -> (f32, f32, f32) {
    let n = b.subcarriers.len().max(1) as f32;
    let mut amp = 0.0f32;
    let mut var = 0.0f32;
    let mut disp = 0.0f32;
    for s in &b.subcarriers {
        amp += s.amp_mean;
        var += s.amp_variance;
        disp += s.phase_dispersion;
    }
    (amp / n, var / n, disp / n)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_params_defaults() {
        let p = StartParams::default();
        assert_eq!(p.duration_s, 30);
        assert_eq!(p.min_frames, 0);
        assert!(p.tier.is_none());
    }

    #[test]
    fn start_params_partial_json() {
        let p: StartParams = serde_json::from_str(r#"{"room_id":"living-room","tier":"he20"}"#).unwrap();
        assert_eq!(p.room_id.as_deref(), Some("living-room"));
        assert_eq!(p.tier.as_deref(), Some("he20"));
        assert_eq!(p.duration_s, 30); // default applied
    }

    #[test]
    fn args_defaults() {
        let a = CalibrateServeArgs {
            http_port: 8090,
            http_bind: "127.0.0.1".into(),
            udp_port: 5005,
            udp_bind: "0.0.0.0".into(),
            tier: "ht20".into(),
            output_dir: "./baselines".into(),
            token: None,
        };
        assert_eq!(a.http_port, 8090);
        assert_eq!(a.udp_port, 5005);
    }

    #[tokio::test]
    async fn finalize_insufficient_frames_marks_session_aborted() {
        let dir = tempfile::tempdir().unwrap();
        let status = Arc::new(RwLock::new(SharedStatus::default()));
        let sess = ActiveSession {
            recorder: CalibrationRecorder::new(tier_config("ht20")),
            room_id: "test".into(),
            tier: "ht20".into(),
            started: Instant::now(),
            deadline: Instant::now(),
            target_frames: 1,
            z_median: 0.0,
            z_max: 0.0,
            motion_flagged: false,
        };

        let err = finalize(sess, dir.path().to_str().unwrap(), &status)
            .await
            .expect_err("an empty recorder cannot finalize");
        assert!(err.contains("finalize failed"));

        let guard = status.read().await;
        let session = guard.session.as_ref().expect("terminal session status");
        assert_eq!(session.state, "aborted");
        assert!(session.note.as_deref().unwrap_or_default().contains("finalize failed"));
    }

    #[test]
    fn sanitize_blocks_path_traversal() {
        assert_eq!(sanitize_room_id("../../etc/passwd"), "etcpasswd");
        assert_eq!(sanitize_room_id("/abs/path"), "abspath");
        assert_eq!(sanitize_room_id("living-room_1"), "living-room_1");
        assert_eq!(sanitize_room_id(""), "default");
        assert_eq!(sanitize_room_id("..\\..\\win"), "win");
        assert!(!sanitize_room_id("a/b/c").contains('/'));
    }

    // ---- HTTP integration tests (router via tower oneshot, no network/ingest) ----

    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt; // for `oneshot`

    fn test_state(dir: &str) -> ApiState {
        let (cmd_tx, _rx) = mpsc::channel::<CalCommand>(8);
        let status = Arc::new(RwLock::new(SharedStatus {
            output_dir: dir.to_string(),
            ..Default::default()
        }));
        let window = Arc::new(RwLock::new(VecDeque::<f32>::new()));
        let enroll = Arc::new(RwLock::new(HashMap::<String, RoomEnroll>::new()));
        // Tested handlers never use cmd_tx; drop the receiver.
        drop(_rx);
        ApiState { cmd_tx, status, window, fs_hz: 15.0, enroll }
    }

    async fn req(app: Router, method: &str, uri: &str, body: Option<&str>) -> StatusCode {
        let b = body.map(|s| Body::from(s.to_string())).unwrap_or_else(Body::empty);
        let r = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(b)
            .unwrap();
        app.oneshot(r).await.unwrap().status()
    }

    #[tokio::test]
    async fn health_and_descriptor_ok() {
        let dir = tempfile::tempdir().unwrap();
        let app = build_router(test_state(dir.path().to_str().unwrap()));
        assert_eq!(req(app.clone(), "GET", "/", None).await, StatusCode::OK);
        assert_eq!(req(app, "GET", "/api/v1/calibration/health", None).await, StatusCode::OK);
    }

    #[tokio::test]
    async fn train_then_state_and_traversal_defense() {
        let dir = tempfile::tempdir().unwrap();
        let state = test_state(dir.path().to_str().unwrap());
        // Fill the live window with a 0.3 Hz breathing sine.
        {
            let mut w = state.window.write().await;
            for i in 0..200 {
                w.push_back((2.0 * std::f32::consts::PI * 0.3 * i as f32 / 15.0).sin());
            }
        }
        let app = build_router(state);

        // POST /room/train with two anchors → bank persisted as t.json.
        let body = r#"{"room_id":"t","baseline_id":"b","anchors":[
            {"room_id":"t","label":"empty","features":{"mean":1.0,"variance":1.0,"motion":0.1,"breathing_score":0.0,"breathing_hz":0.0,"heart_score":0.0,"heart_hz":0.0}},
            {"room_id":"t","label":"stand_still","features":{"mean":1.0,"variance":10.0,"motion":0.2,"breathing_score":0.0,"breathing_hz":0.0,"heart_score":0.0,"heart_hz":0.0}}
        ]}"#;
        assert_eq!(req(app.clone(), "POST", "/api/v1/room/train", Some(body)).await, StatusCode::OK);
        assert!(dir.path().join("t.json").exists(), "bank file written");

        // GET /room/state?bank=t → 200 (trained bank loaded, window present).
        assert_eq!(req(app.clone(), "GET", "/api/v1/room/state?bank=t", None).await, StatusCode::OK);

        // Path-traversal: ?bank=../../etc/passwd is sanitized → NOT_FOUND, never reads outside dir.
        assert_eq!(
            req(app.clone(), "GET", "/api/v1/room/state?bank=../../etc/passwd", None).await,
            StatusCode::NOT_FOUND
        );

        // Train with no anchors and nothing enrolled → 400.
        assert_eq!(
            req(app, "POST", "/api/v1/room/train", Some(r#"{"room_id":"none","baseline_id":"b","anchors":[]}"#)).await,
            StatusCode::BAD_REQUEST
        );
    }

    /// ADR-152 §2.1.1: geometry threads into the trained bank through both API
    /// paths — inline in the train request, or recorded via /enroll/geometry —
    /// and a geometry-free train still produces a valid (unconditioned) bank.
    #[tokio::test]
    async fn train_threads_geometry_into_bank() {
        let dir = tempfile::tempdir().unwrap();
        let app = build_router(test_state(dir.path().to_str().unwrap()));
        let anchors = r#"[
            {"room_id":"g","label":"empty","features":{"mean":1.0,"variance":1.0,"motion":0.1,"breathing_score":0.0,"breathing_hz":0.0,"heart_score":0.0,"heart_hz":0.0}},
            {"room_id":"g","label":"stand_still","features":{"mean":1.0,"variance":10.0,"motion":0.2,"breathing_score":0.0,"breathing_hz":0.0,"heart_score":0.0,"heart_hz":0.0}}
        ]"#;
        let load_bank = |name: &str| {
            let raw = std::fs::read_to_string(dir.path().join(format!("{name}.json"))).unwrap();
            SpecialistBank::from_json(&raw).unwrap()
        };

        // (1) geometry inline in the train request.
        let body = format!(
            r#"{{"room_id":"g1","baseline_id":"b","anchors":{anchors},
                "geometry":[{{"node_id":1,"position":{{"x_m":0.0,"y_m":0.0,"z_m":1.0}},"method":"tape-measure"}},{{"node_id":2}}]}}"#
        );
        assert_eq!(req(app.clone(), "POST", "/api/v1/room/train", Some(&body)).await, StatusCode::OK);
        let bank = load_bank("g1");
        assert_eq!(bank.geometry.len(), 2);
        assert_eq!(bank.geometry[0].method, "tape-measure");
        assert_eq!(bank.geometry[1].node_id, 2);

        // (2) geometry recorded via /enroll/geometry; train body omits it.
        assert_eq!(
            req(app.clone(), "POST", "/api/v1/enroll/geometry",
                Some(r#"{"room_id":"g2","geometry":[{"node_id":7,"method":"floor-plan"}]}"#)).await,
            StatusCode::OK
        );
        let body2 = format!(r#"{{"room_id":"g2","baseline_id":"b","anchors":{anchors}}}"#);
        assert_eq!(req(app.clone(), "POST", "/api/v1/room/train", Some(&body2)).await, StatusCode::OK);
        let bank2 = load_bank("g2");
        assert_eq!(bank2.geometry.len(), 1);
        assert_eq!(bank2.geometry[0].node_id, 7);

        // (3) no geometry anywhere → valid geometry-free bank (note logged).
        let body3 = format!(r#"{{"room_id":"g3","baseline_id":"b","anchors":{anchors}}}"#);
        assert_eq!(req(app.clone(), "POST", "/api/v1/room/train", Some(&body3)).await, StatusCode::OK);
        let bank3 = load_bank("g3");
        assert!(bank3.geometry.is_empty());
        assert!(bank3.presence.is_some(), "bank still trains without geometry");

        // (4) empty geometry array is rejected.
        assert_eq!(
            req(app, "POST", "/api/v1/enroll/geometry", Some(r#"{"room_id":"g4","geometry":[]}"#)).await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn enroll_status_empty_and_bad_label() {
        let dir = tempfile::tempdir().unwrap();
        let app = build_router(test_state(dir.path().to_str().unwrap()));
        // No enrollment yet → 200 with next=empty.
        assert_eq!(req(app.clone(), "GET", "/api/v1/enroll/status?room=x", None).await, StatusCode::OK);
        // Unknown anchor label → 400.
        assert_eq!(
            req(app, "POST", "/api/v1/enroll/anchor", Some(r#"{"room_id":"x","baseline":"b","label":"nope"}"#)).await,
            StatusCode::BAD_REQUEST
        );
    }
}
