//! RuView portable compute kernel (ADR-368).
//!
//! One deterministic, bounded Rust core exposed to JavaScript through two
//! interchangeable transports:
//!
//! * **WASM** (`wasm32-unknown-unknown`, zero imports) via [`wasm_abi`], the
//!   default least-authority backend of `@ruvnet/ruview-kernel`.
//! * **napi-rs** (`crates/ruview-kernel-napi`), an optional native backend.
//!
//! Both transports call exactly one entry point, [`call`], with an operation
//! name and a JSON request, and receive a JSON envelope:
//! `{"ok":true,"result":...}` or `{"ok":false,"error":{"code","message"}}`.
//! Keeping the ABI to a single string-in/string-out function makes
//! cross-backend parity a property of the transport, not of duplicated
//! bindings.
//!
//! The signal path is the ADR-021 vitals pipeline from
//! `wifi-densepose-vitals` (EMA preprocessing, breathing and heart-rate
//! extraction, anomaly detection). Every input is untrusted: sizes, lengths,
//! and numeric ranges are validated before any processing, so the core never
//! panics on caller data (release builds use `panic = "abort"`).
//!
//! Outputs describe signal processing only. Any accuracy statement about them
//! must carry an evidence tag; data from [`synthesize`] is `SYNTHETIC`.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wifi_densepose_vitals::{
    BreathingExtractor, CsiFrame, CsiVitalPreprocessor, HeartRateExtractor, VitalAnomalyDetector,
    VitalEstimate, VitalReading, VitalSignStore, VitalStatus,
};

#[cfg(target_arch = "wasm32")]
pub mod wasm_abi;

/// Version of the JSON ABI. Bumped on any incompatible request/response change.
pub const ABI_VERSION: u32 = 1;
/// Maximum accepted request size in bytes.
pub const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum subcarriers per frame.
pub const MAX_SUBCARRIERS: usize = 512;
/// Maximum frames in one `analyze`/`session_push` request.
pub const MAX_FRAMES_PER_CALL: usize = 100_000;
/// Maximum frames `synthesize` may generate.
pub const MAX_SYNTH_FRAMES: usize = 100_000;
/// Maximum f64 values in one binary (flat) request.
pub const MAX_FLAT_VALUES: usize = MAX_INPUT_BYTES / 8;
/// Maximum concurrently open sessions.
pub const MAX_SESSIONS: usize = 64;

/// Operations accepted by [`call`].
pub const OPERATIONS: &[&str] = &[
    "info",
    "validate_config",
    "analyze",
    "synthesize",
    "session_open",
    "session_push",
    "session_summary",
    "session_close",
    "analyze_flat",
    "session_push_flat",
];

/// Structured kernel error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelError {
    pub code: &'static str,
    pub message: String,
}

impl KernelError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    fn invalid(message: impl Into<String>) -> Self {
        Self::new("invalid_request", message)
    }
}

type KResult<T> = Result<T, KernelError>;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Session configuration. All fields have ESP32-oriented defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct SessionConfig {
    pub n_subcarriers: usize,
    pub sample_rate_hz: f64,
    pub breathing_window_secs: f64,
    pub heart_window_secs: f64,
    pub preprocess_alpha: f64,
    /// Emit one vital reading every `emit_every` frames (0 = once per second).
    pub emit_every: usize,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            n_subcarriers: 56,
            sample_rate_hz: 20.0,
            breathing_window_secs: 30.0,
            heart_window_secs: 15.0,
            preprocess_alpha: 0.05,
            emit_every: 0,
        }
    }
}

fn finite_in(name: &str, v: f64, lo: f64, hi: f64) -> KResult<()> {
    if !v.is_finite() || v < lo || v > hi {
        return Err(KernelError::invalid(format!(
            "{name} must be a finite number in [{lo}, {hi}]"
        )));
    }
    Ok(())
}

impl SessionConfig {
    /// Validate ranges; returns the effective emission interval in frames.
    pub fn validate(&self) -> KResult<usize> {
        if self.n_subcarriers == 0 || self.n_subcarriers > MAX_SUBCARRIERS {
            return Err(KernelError::invalid(format!(
                "n_subcarriers must be in [1, {MAX_SUBCARRIERS}]"
            )));
        }
        finite_in("sample_rate_hz", self.sample_rate_hz, 1.0, 1000.0)?;
        finite_in(
            "breathing_window_secs",
            self.breathing_window_secs,
            10.0,
            300.0,
        )?;
        finite_in("heart_window_secs", self.heart_window_secs, 5.0, 120.0)?;
        finite_in("preprocess_alpha", self.preprocess_alpha, 0.001, 1.0)?;
        if self.emit_every > MAX_FRAMES_PER_CALL {
            return Err(KernelError::invalid(format!(
                "emit_every must be <= {MAX_FRAMES_PER_CALL}"
            )));
        }
        Ok(if self.emit_every == 0 {
            (self.sample_rate_hz.round() as usize).max(1)
        } else {
            self.emit_every
        })
    }
}

// ---------------------------------------------------------------------------
// Frames and outputs
// ---------------------------------------------------------------------------

/// One CSI frame as supplied by JavaScript.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct FrameInput {
    pub amplitudes: Vec<f64>,
    #[serde(default)]
    pub phases: Vec<f64>,
}

/// Serializable vital estimate.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Estimate {
    pub bpm: f64,
    pub confidence: f64,
    pub status: String,
}

fn status_name(s: VitalStatus) -> &'static str {
    match s {
        VitalStatus::Valid => "valid",
        VitalStatus::Degraded => "degraded",
        VitalStatus::Unreliable => "unreliable",
        VitalStatus::Unavailable => "unavailable",
    }
}

impl From<&VitalEstimate> for Estimate {
    fn from(e: &VitalEstimate) -> Self {
        Self {
            bpm: e.value_bpm,
            confidence: e.confidence,
            status: status_name(e.status).to_string(),
        }
    }
}

/// Serializable anomaly alert.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Alert {
    pub vital: String,
    pub kind: String,
    pub severity: f64,
    pub message: String,
}

/// A reading emitted at a frame boundary.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Reading {
    pub frame: u64,
    pub t_secs: f64,
    pub respiratory: Estimate,
    pub heart: Estimate,
    /// Root-mean-square of the EMA residuals for this frame (signal units).
    pub motion_energy: f64,
    /// Mean confidence of the two estimates; a pipeline-internal quality score.
    pub signal_quality: f64,
    pub alerts: Vec<Alert>,
}

/// Aggregate over a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub frames: u64,
    pub readings: u64,
    pub duration_secs: f64,
    pub respiratory_mean_bpm: Option<f64>,
    pub heart_mean_bpm: Option<f64>,
    pub valid_fraction: f64,
    pub mean_motion_energy: f64,
    pub last: Option<Reading>,
    pub alerts: u64,
}

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// Streaming analysis session over the ADR-021 vitals pipeline.
pub struct Session {
    config: SessionConfig,
    emit_every: usize,
    pre: CsiVitalPreprocessor,
    breathing: BreathingExtractor,
    heart: HeartRateExtractor,
    anomaly: VitalAnomalyDetector,
    store: VitalSignStore,
    weights: Vec<f64>,
    frames: u64,
    readings: u64,
    alerts: u64,
    motion_sum: f64,
    last_rr: VitalEstimate,
    last_hr: VitalEstimate,
    last: Option<Reading>,
}

impl Session {
    pub fn new(config: SessionConfig) -> KResult<Self> {
        let emit_every = config.validate()?;
        let n = config.n_subcarriers;
        Ok(Self {
            pre: CsiVitalPreprocessor::new(n, config.preprocess_alpha),
            breathing: BreathingExtractor::new(
                n,
                config.sample_rate_hz,
                config.breathing_window_secs,
            ),
            heart: HeartRateExtractor::new(n, config.sample_rate_hz, config.heart_window_secs),
            anomaly: VitalAnomalyDetector::default_config(),
            store: VitalSignStore::new(3600),
            weights: vec![1.0 / n as f64; n],
            emit_every,
            frames: 0,
            readings: 0,
            alerts: 0,
            motion_sum: 0.0,
            last_rr: VitalEstimate::unavailable(),
            last_hr: VitalEstimate::unavailable(),
            last: None,
            config,
        })
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    fn check_frame(&self, idx: usize, f: &FrameInput) -> KResult<()> {
        let n = self.config.n_subcarriers;
        if f.amplitudes.len() != n {
            return Err(KernelError::invalid(format!(
                "frames[{idx}].amplitudes must have {n} values"
            )));
        }
        if !f.phases.is_empty() && f.phases.len() != n {
            return Err(KernelError::invalid(format!(
                "frames[{idx}].phases must be empty or have {n} values"
            )));
        }
        if f.amplitudes
            .iter()
            .chain(f.phases.iter())
            .any(|v| !v.is_finite() || v.abs() > 1.0e9)
        {
            return Err(KernelError::invalid(format!(
                "frames[{idx}] contains a non-finite or out-of-range value"
            )));
        }
        Ok(())
    }

    /// Validate every frame first (all-or-nothing), then process them in order.
    pub fn push(&mut self, frames: &[FrameInput]) -> KResult<Vec<Reading>> {
        if frames.len() > MAX_FRAMES_PER_CALL {
            return Err(KernelError::new(
                "limit_exceeded",
                format!("at most {MAX_FRAMES_PER_CALL} frames per call"),
            ));
        }
        for (i, f) in frames.iter().enumerate() {
            self.check_frame(i, f)?;
        }
        let mut out = Vec::new();
        for f in frames {
            let phases = (!f.phases.is_empty()).then_some(f.phases.as_slice());
            if let Some(r) = self.step(&f.amplitudes, phases) {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Binary fast path (ADR-368): `data` holds every frame's amplitudes
    /// back to back (`frames * n` values), followed by the same layout for
    /// phases when `with_phases`. Validated all-or-nothing like [`push`].
    pub fn push_flat(&mut self, data: &[f64], with_phases: bool) -> KResult<Vec<Reading>> {
        let n = self.config.n_subcarriers;
        let planes = if with_phases { 2 } else { 1 };
        if data.len() % (n * planes) != 0 {
            return Err(KernelError::invalid(format!(
                "flat data length {} is not a multiple of n_subcarriers * {planes} ({})",
                data.len(),
                n * planes
            )));
        }
        let count = data.len() / (n * planes);
        if count > MAX_FRAMES_PER_CALL {
            return Err(KernelError::new(
                "limit_exceeded",
                format!("at most {MAX_FRAMES_PER_CALL} frames per call"),
            ));
        }
        if let Some(i) = data.iter().position(|v| !v.is_finite() || v.abs() > 1.0e9) {
            return Err(KernelError::invalid(format!(
                "frames[{}] contains a non-finite or out-of-range value",
                (i % (count * n).max(1)) / n
            )));
        }
        let (amps, phases) = data.split_at(count * n);
        let mut out = Vec::new();
        for k in 0..count {
            let a = &amps[k * n..(k + 1) * n];
            let p = with_phases.then(|| &phases[k * n..(k + 1) * n]);
            if let Some(r) = self.step(a, p) {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Process one validated frame; returns a reading at emission boundaries.
    fn step(&mut self, amplitudes: &[f64], phases: Option<&[f64]>) -> Option<Reading> {
        let n = self.config.n_subcarriers;
        let frame = CsiFrame {
            amplitudes: amplitudes.to_vec(),
            phases: phases.map_or_else(|| vec![0.0; n], <[f64]>::to_vec),
            n_subcarriers: n,
            sample_index: self.frames,
            sample_rate_hz: self.config.sample_rate_hz,
        };
        self.frames += 1;
        let residuals = self.pre.process(&frame)?;
        let motion = (residuals.iter().map(|r| r * r).sum::<f64>() / n as f64).sqrt();
        self.motion_sum += motion;
        if let Some(rr) = self.breathing.extract(&residuals, &self.weights) {
            self.last_rr = rr;
        }
        if let Some(hr) = self.heart.extract(&residuals, &frame.phases) {
            self.last_hr = hr;
        }
        if self.frames % self.emit_every as u64 != 0 {
            return None;
        }
        let t_secs = self.frames as f64 / self.config.sample_rate_hz;
        let signal_quality = (self.last_rr.confidence + self.last_hr.confidence) / 2.0;
        let vital = VitalReading {
            respiratory_rate: self.last_rr.clone(),
            heart_rate: self.last_hr.clone(),
            subcarrier_count: n,
            signal_quality,
            timestamp_secs: t_secs,
        };
        let alerts: Vec<Alert> = if self.last_rr.status == VitalStatus::Unavailable
            && self.last_hr.status == VitalStatus::Unavailable
        {
            Vec::new() // warm-up: nothing to judge yet
        } else {
            self.anomaly
                .check(&vital)
                .into_iter()
                .map(|a| Alert {
                    vital: a.vital_type,
                    kind: a.alert_type,
                    severity: a.severity,
                    message: a.message,
                })
                .collect()
        };
        self.alerts += alerts.len() as u64;
        self.store.push(vital);
        self.readings += 1;
        let reading = Reading {
            frame: self.frames,
            t_secs,
            respiratory: (&self.last_rr).into(),
            heart: (&self.last_hr).into(),
            motion_energy: motion,
            signal_quality,
            alerts,
        };
        self.last = Some(reading.clone());
        Some(reading)
    }

    pub fn summary(&self) -> Summary {
        let stats = self.store.stats();
        let (rr, hr, valid) = match &stats {
            Some(s) if s.count > 0 => (
                (s.rr_mean > 0.0).then_some(s.rr_mean),
                (s.hr_mean > 0.0).then_some(s.hr_mean),
                s.valid_fraction,
            ),
            _ => (None, None, 0.0),
        };
        Summary {
            frames: self.frames,
            readings: self.readings,
            duration_secs: self.frames as f64 / self.config.sample_rate_hz,
            respiratory_mean_bpm: rr,
            heart_mean_bpm: hr,
            valid_fraction: valid,
            mean_motion_energy: if self.frames == 0 {
                0.0
            } else {
                self.motion_sum / self.frames as f64
            },
            last: self.last.clone(),
            alerts: self.alerts,
        }
    }
}

// ---------------------------------------------------------------------------
// Deterministic synthetic CSI (evidence: SYNTHETIC)
// ---------------------------------------------------------------------------

/// Parameters for [`synthesize`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, default)]
pub struct SynthRequest {
    pub n_subcarriers: usize,
    pub sample_rate_hz: f64,
    pub seconds: f64,
    pub breathing_bpm: f64,
    pub heart_bpm: f64,
    pub breathing_amplitude: f64,
    pub heart_amplitude: f64,
    pub noise: f64,
    pub seed: u64,
}

impl Default for SynthRequest {
    fn default() -> Self {
        Self {
            n_subcarriers: 56,
            sample_rate_hz: 20.0,
            seconds: 60.0,
            breathing_bpm: 15.0,
            heart_bpm: 72.0,
            breathing_amplitude: 0.5,
            heart_amplitude: 0.05,
            noise: 0.01,
            seed: 7,
        }
    }
}

/// SplitMix64: tiny, portable, bit-identical on every target.
struct SplitMix64(u64);
impl SplitMix64 {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [-1, 1).
    fn signed_unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Generate deterministic synthetic CSI with known breathing and heart tones.
pub fn synthesize(req: &SynthRequest) -> KResult<Vec<FrameInput>> {
    if req.n_subcarriers == 0 || req.n_subcarriers > MAX_SUBCARRIERS {
        return Err(KernelError::invalid(format!(
            "n_subcarriers must be in [1, {MAX_SUBCARRIERS}]"
        )));
    }
    finite_in("sample_rate_hz", req.sample_rate_hz, 1.0, 1000.0)?;
    finite_in("seconds", req.seconds, 0.0, 3600.0)?;
    finite_in("breathing_bpm", req.breathing_bpm, 0.0, 120.0)?;
    finite_in("heart_bpm", req.heart_bpm, 0.0, 240.0)?;
    finite_in("breathing_amplitude", req.breathing_amplitude, 0.0, 100.0)?;
    finite_in("heart_amplitude", req.heart_amplitude, 0.0, 100.0)?;
    finite_in("noise", req.noise, 0.0, 100.0)?;
    let count = (req.seconds * req.sample_rate_hz).round() as usize;
    if count > MAX_SYNTH_FRAMES {
        return Err(KernelError::new(
            "limit_exceeded",
            format!("seconds * sample_rate_hz must be <= {MAX_SYNTH_FRAMES}"),
        ));
    }
    let n = req.n_subcarriers;
    let mut rng = SplitMix64(req.seed);
    let tau = std::f64::consts::TAU;
    let fb = req.breathing_bpm / 60.0;
    let fh = req.heart_bpm / 60.0;
    let mut frames = Vec::with_capacity(count);
    for k in 0..count {
        let t = k as f64 / req.sample_rate_hz;
        let b = req.breathing_amplitude * (tau * fb * t).sin();
        let h = req.heart_amplitude * (tau * fh * t).sin();
        let mut amplitudes = Vec::with_capacity(n);
        let mut phases = Vec::with_capacity(n);
        for i in 0..n {
            // Per-subcarrier sensitivity in (0.5, 1.0]; static multipath baseline.
            let gain = 0.5 + 0.5 * ((i + 1) as f64 / n as f64);
            let base = 10.0 + (i as f64 * 0.37).sin();
            amplitudes.push(base + gain * (b + h) + req.noise * rng.signed_unit());
            phases.push(0.1 * i as f64 + 0.2 * h + req.noise * rng.signed_unit());
        }
        frames.push(FrameInput { amplitudes, phases });
    }
    Ok(frames)
}

// ---------------------------------------------------------------------------
// Session registry (handles are opaque u32s shared by both transports)
// ---------------------------------------------------------------------------

struct Registry {
    next: u32,
    sessions: BTreeMap<u32, Session>,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    next: 1,
    sessions: BTreeMap::new(),
});

fn with_registry<T>(f: impl FnOnce(&mut Registry) -> KResult<T>) -> KResult<T> {
    let mut guard = REGISTRY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    f(&mut guard)
}

// ---------------------------------------------------------------------------
// JSON dispatch
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyzeRequest {
    #[serde(default)]
    config: SessionConfig,
    frames: Vec<FrameInput>,
    #[serde(default)]
    include_readings: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenRequest {
    #[serde(default)]
    config: SessionConfig,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PushRequest {
    session: u32,
    frames: Vec<FrameInput>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AnalyzeFlatRequest {
    #[serde(default)]
    config: SessionConfig,
    #[serde(default)]
    phases: bool,
    #[serde(default)]
    include_readings: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PushFlatRequest {
    session: u32,
    #[serde(default)]
    phases: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HandleRequest {
    session: u32,
}

fn parse<T: for<'de> Deserialize<'de>>(input: &str) -> KResult<T> {
    let src = if input.trim().is_empty() { "{}" } else { input };
    serde_json::from_str(src).map_err(|e| KernelError::invalid(format!("request JSON: {e}")))
}

fn to_value<T: Serialize>(v: &T) -> KResult<Value> {
    serde_json::to_value(v).map_err(|e| KernelError::new("internal", e.to_string()))
}

/// Static description of this kernel build.
pub fn info() -> Value {
    json!({
        "name": "ruview-kernel",
        "version": env!("CARGO_PKG_VERSION"),
        "abi": ABI_VERSION,
        "operations": OPERATIONS,
        "pipeline": "ADR-021 vitals (wifi-densepose-vitals): EMA preprocessing, breathing 0.1-0.5 Hz, heart 0.8-2.0 Hz, anomaly z-score",
        "limits": {
            "max_input_bytes": MAX_INPUT_BYTES,
            "max_subcarriers": MAX_SUBCARRIERS,
            "max_frames_per_call": MAX_FRAMES_PER_CALL,
            "max_synth_frames": MAX_SYNTH_FRAMES,
            "max_sessions": MAX_SESSIONS,
            "max_flat_values": MAX_FLAT_VALUES,
        },
        "target": if cfg!(target_arch = "wasm32") { "wasm32" } else { "native" },
        "evidence_policy": "Outputs are signal-processing estimates, not clinical or camera-grade measurements; synthesize() output is SYNTHETIC.",
    })
}

fn dispatch(op: &str, input: &str) -> KResult<Value> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(KernelError::new(
            "limit_exceeded",
            format!("request exceeds {MAX_INPUT_BYTES} bytes"),
        ));
    }
    match op {
        "info" => Ok(info()),
        "validate_config" => {
            let cfg: SessionConfig = parse(input)?;
            let emit_every = cfg.validate()?;
            Ok(json!({ "config": cfg, "emit_every": emit_every }))
        }
        "analyze" => {
            let req: AnalyzeRequest = parse(input)?;
            let mut s = Session::new(req.config)?;
            let readings = s.push(&req.frames)?;
            let mut out = json!({ "config": s.config(), "summary": s.summary() });
            if req.include_readings {
                out["readings"] = to_value(&readings)?;
            }
            Ok(out)
        }
        "synthesize" => {
            let req: SynthRequest = parse(input)?;
            let frames = synthesize(&req)?;
            Ok(json!({ "evidence": "SYNTHETIC", "request": req, "frames": frames }))
        }
        "session_open" => {
            let req: OpenRequest = parse(input)?;
            let session = Session::new(req.config)?;
            with_registry(|r| {
                if r.sessions.len() >= MAX_SESSIONS {
                    return Err(KernelError::new(
                        "limit_exceeded",
                        format!("at most {MAX_SESSIONS} open sessions"),
                    ));
                }
                let id = r.next;
                r.next = r.next.checked_add(1).unwrap_or(1);
                r.sessions.insert(id, session);
                Ok(json!({ "session": id }))
            })
        }
        "session_push" => {
            let req: PushRequest = parse(input)?;
            with_registry(|r| {
                let s = r
                    .sessions
                    .get_mut(&req.session)
                    .ok_or_else(|| KernelError::new("unknown_session", "no such session"))?;
                let readings = s.push(&req.frames)?;
                Ok(json!({ "session": req.session, "readings": readings, "frames": s.frames }))
            })
        }
        "session_summary" => {
            let req: HandleRequest = parse(input)?;
            with_registry(|r| {
                let s = r
                    .sessions
                    .get(&req.session)
                    .ok_or_else(|| KernelError::new("unknown_session", "no such session"))?;
                Ok(json!({ "session": req.session, "summary": s.summary() }))
            })
        }
        "session_close" => {
            let req: HandleRequest = parse(input)?;
            with_registry(|r| {
                let closed = r.sessions.remove(&req.session).is_some();
                Ok(json!({ "session": req.session, "closed": closed }))
            })
        }
        "analyze_flat" | "session_push_flat" => Err(KernelError::invalid(format!(
            "{op} takes binary frame data; use call_f64"
        ))),
        _ => Err(KernelError::new(
            "unknown_operation",
            format!(
                "unknown operation; expected one of {}",
                OPERATIONS.join(", ")
            ),
        )),
    }
}

fn dispatch_f64(op: &str, input: &str, data: &[f64]) -> KResult<Value> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(KernelError::new(
            "limit_exceeded",
            format!("request exceeds {MAX_INPUT_BYTES} bytes"),
        ));
    }
    if data.len() > MAX_FLAT_VALUES {
        return Err(KernelError::new(
            "limit_exceeded",
            format!("flat data exceeds {MAX_FLAT_VALUES} values"),
        ));
    }
    match op {
        "analyze_flat" => {
            let req: AnalyzeFlatRequest = parse(input)?;
            let mut s = Session::new(req.config)?;
            let readings = s.push_flat(data, req.phases)?;
            let mut out = json!({ "config": s.config(), "summary": s.summary() });
            if req.include_readings {
                out["readings"] = to_value(&readings)?;
            }
            Ok(out)
        }
        "session_push_flat" => {
            let req: PushFlatRequest = parse(input)?;
            with_registry(|r| {
                let s = r
                    .sessions
                    .get_mut(&req.session)
                    .ok_or_else(|| KernelError::new("unknown_session", "no such session"))?;
                let readings = s.push_flat(data, req.phases)?;
                Ok(json!({ "session": req.session, "readings": readings, "frames": s.frames }))
            })
        }
        _ => Err(KernelError::invalid(format!(
            "{op} does not take binary frame data"
        ))),
    }
}

fn envelope(result: KResult<Value>) -> String {
    match result {
        Ok(result) => json!({ "ok": true, "result": result }),
        Err(e) => json!({ "ok": false, "error": { "code": e.code, "message": e.message } }),
    }
    .to_string()
}

/// The single ABI entry point: operation name + JSON request → JSON envelope.
/// Never panics on caller input.
pub fn call(op: &str, input: &str) -> String {
    envelope(dispatch(op, input))
}

/// Binary fast path: same envelope, with frame data as a flat `f64` slice
/// (`analyze_flat`, `session_push_flat`) instead of JSON numbers.
pub fn call_f64(op: &str, input: &str, data: &[f64]) -> String {
    envelope(dispatch_f64(op, input, data))
}
