//! ADR-426 D5: default-off ruv-FANN shadow scoring of the per-node presence
//! classification, plus feature capture for replay.
//!
//! Built only with the `fann-shadow` cargo feature and active only when one of
//! these is set:
//!
//! - `RUVIEW_FANN_SHADOW_HEAD`: an exported `ruview-fann-head` document. Each
//!   fresh classification of a node named in the head's `sources`
//!   (`csi-node-<id>`) is also scored by the head;
//! - `RUVIEW_FANN_SHADOW_LOG`: a JSON-lines file that receives one record per
//!   observation (encoded features, the server's decision, the candidate's
//!   score when a head is loaded). Without a head this is capture only.
//!
//! The candidate has no authority. Nothing here writes to the sensing state:
//! the task holds a read lock, and its output is counters and a log file.
//! Agreement with the server's classifier is not accuracy; that needs
//! reference labels (ADR-423 presence benchmark, `scripts/build_replay_rows.py`).

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use ruview_fann_head::features::{encode, NodeFeatures, NODE_FEATURE_COUNT, NODE_FEATURE_SCHEMA};
use ruview_fann_head::shadow::{CandidateOutcome, ExternalShadow};
use ruview_fann_head::FannHead;
use serde_json::json;

pub const HEAD_ENV: &str = "RUVIEW_FANN_SHADOW_HEAD";
pub const LOG_ENV: &str = "RUVIEW_FANN_SHADOW_LOG";
/// The log stops growing here; later records are counted as dropped.
pub const MAX_LOG_BYTES: u64 = 256 << 20;
const MAX_HEAD_BYTES: u64 = 1 << 20;
const POLL_MS: u64 = 250;

static SHADOW: OnceLock<Mutex<Shadow>> = OnceLock::new();

/// One fresh per-node classification, as the server produced it.
#[derive(Debug, Clone)]
pub struct Observation<'a> {
    pub timestamp_ms: i64,
    pub node_id: u8,
    pub source: &'a str,
    pub synthetic: bool,
    pub stale: bool,
    pub features: NodeFeatures,
    pub presence: bool,
    pub confidence: f64,
    pub motion_level: &'a str,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Counters {
    pub observations: u64,
    pub skipped_stale: u64,
    pub skipped_duplicate: u64,
    pub skipped_non_finite: u64,
    /// Fresh observations from nodes the head was not fitted for.
    pub skipped_other_node: u64,
    pub synthetic_observations: u64,
    pub logged: u64,
    pub log_dropped: u64,
}

pub struct Shadow {
    head: Option<(ExternalShadow, Vec<u8>)>,
    log: Option<Box<dyn Write + Send>>,
    log_bytes: u64,
    max_log_bytes: u64,
    last_seen: HashMap<u8, i64>,
    counters: Counters,
    started_ms: i64,
}

fn node_ids(sources: &[String]) -> Result<Vec<u8>, String> {
    sources
        .iter()
        .map(|s| {
            s.strip_prefix("csi-node-")
                .and_then(|n| n.parse::<u8>().ok())
                .ok_or_else(|| format!("head source {s:?} is not csi-node-<0-255>"))
        })
        .collect()
}

impl Shadow {
    /// `head_json` is an exported head; it must use the node-feature schema.
    pub fn new(
        head_json: Option<&str>,
        log: Option<Box<dyn Write + Send>>,
        log_bytes: u64,
        now_ms: i64,
    ) -> Result<Self, String> {
        let head = match head_json {
            None => None,
            Some(text) => {
                let head = FannHead::from_json(text).map_err(|e| e.to_string())?;
                let context = head.context().clone();
                if context.feature_schema != NODE_FEATURE_SCHEMA
                    || head.features() != NODE_FEATURE_COUNT
                {
                    return Err(format!(
                        "head is for schema {:?} with {} features; this server scores {NODE_FEATURE_SCHEMA} with {NODE_FEATURE_COUNT}",
                        context.feature_schema,
                        head.features()
                    ));
                }
                let nodes = node_ids(&context.sources)?;
                let shadow = ExternalShadow::new(head, &context).map_err(|e| e.to_string())?;
                Some((shadow, nodes))
            }
        };
        Ok(Self {
            head,
            log,
            log_bytes,
            max_log_bytes: MAX_LOG_BYTES,
            last_seen: HashMap::new(),
            counters: Counters::default(),
            started_ms: now_ms,
        })
    }

    /// Record one classification. A stale, repeated or non-finite observation
    /// is counted and skipped: it is never scored or logged as a decision.
    pub fn observe(&mut self, o: &Observation<'_>) {
        if o.stale {
            self.counters.skipped_stale += 1;
            return;
        }
        if self.last_seen.get(&o.node_id).is_some_and(|&t| t >= o.timestamp_ms) {
            self.counters.skipped_duplicate += 1;
            return;
        }
        self.last_seen.insert(o.node_id, o.timestamp_ms);
        let Some(x) = encode(&o.features).filter(|_| o.confidence.is_finite()) else {
            self.counters.skipped_non_finite += 1;
            return;
        };
        self.counters.observations += 1;
        if o.synthetic {
            self.counters.synthetic_observations += 1;
        }
        let candidate = match &mut self.head {
            Some((shadow, nodes)) if nodes.contains(&o.node_id) => {
                Some(shadow.score(&x, o.presence))
            }
            Some(_) => {
                self.counters.skipped_other_node += 1;
                None
            }
            None => None,
        };
        let Some(log) = self.log.as_mut() else { return };
        let mut line = json!({
            "timestamp_ms": o.timestamp_ms,
            "node_id": o.node_id,
            "source": o.source,
            "synthetic": o.synthetic,
            "feature_schema": NODE_FEATURE_SCHEMA,
            "features": x,
            "incumbent": {
                "presence": o.presence,
                "confidence": o.confidence,
                "motion_level": o.motion_level,
            },
            "candidate": candidate.as_ref().map(|c| match c {
                CandidateOutcome::Scored { probability, decision } =>
                    json!({"status": "scored", "probability": probability, "decision": decision}),
                CandidateOutcome::Unavailable { reason } =>
                    json!({"status": "unavailable", "reason": reason.to_string()}),
            }),
        })
        .to_string();
        line.push('\n');
        if self.log_bytes + line.len() as u64 > self.max_log_bytes
            || log.write_all(line.as_bytes()).is_err()
        {
            self.counters.log_dropped += 1;
            return;
        }
        self.log_bytes += line.len() as u64;
        self.counters.logged += 1;
    }

    fn flush(&mut self) {
        if let Some(log) = self.log.as_mut() {
            let _ = log.flush();
        }
    }

    pub fn status(&self, now_ms: i64) -> serde_json::Value {
        let candidate = self.head.as_ref().map(|(shadow, nodes)| {
            let c = shadow.counters();
            let decided = c.agree + c.disagree;
            json!({
                "digest": shadow.digest(),
                "nodes": nodes,
                "counters": c,
                "disagreement_rate": (decided > 0).then(|| c.disagree as f64 / decided as f64),
            })
        });
        json!({
            "enabled": true,
            "mode": if self.head.is_some() { "shadow" } else { "capture" },
            "authority": "none: diagnostic only, never published",
            "feature_schema": NODE_FEATURE_SCHEMA,
            "uptime_s": (now_ms - self.started_ms).max(0) / 1000,
            "counters": self.counters,
            "log_bytes": self.log_bytes,
            "candidate": candidate,
            "note": "agreement with the server's classifier is not accuracy; \
                     accuracy needs reference-labelled replay",
        })
    }
}

/// Build the shadow from the environment. `Ok(None)` when neither variable is set.
pub fn from_env(now_ms: i64) -> Result<Option<Shadow>, String> {
    let head_path = std::env::var(HEAD_ENV).ok().filter(|s| !s.is_empty());
    let log_path = std::env::var(LOG_ENV).ok().filter(|s| !s.is_empty());
    if head_path.is_none() && log_path.is_none() {
        return Ok(None);
    }
    let head_json = match &head_path {
        None => None,
        Some(path) => {
            let size = std::fs::metadata(path)
                .map_err(|e| format!("{HEAD_ENV}={path}: {e}"))?
                .len();
            if size > MAX_HEAD_BYTES {
                return Err(format!("{HEAD_ENV}={path}: larger than {MAX_HEAD_BYTES} bytes"));
            }
            Some(std::fs::read_to_string(path).map_err(|e| format!("{HEAD_ENV}={path}: {e}"))?)
        }
    };
    let (log, log_bytes) = match &log_path {
        None => (None, 0),
        Some(path) => {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .map_err(|e| format!("{LOG_ENV}={path}: {e}"))?;
            let existing = file.metadata().map(|m| m.len()).unwrap_or(0);
            let writer: Box<dyn Write + Send> = Box::new(std::io::BufWriter::new(file));
            (Some(writer), existing)
        }
    };
    Shadow::new(head_json.as_deref(), log, log_bytes, now_ms).map(Some)
}

/// Start the shadow task when configured. A bad configuration is logged and
/// leaves the shadow off; it never stops the server.
pub fn spawn(state: crate::SharedState) {
    let now_ms = chrono::Utc::now().timestamp_millis();
    match from_env(now_ms) {
        Ok(None) => return,
        Err(e) => {
            tracing::warn!("fann shadow disabled: {e}");
            return;
        }
        Ok(Some(shadow)) => {
            tracing::info!("fann shadow active: {}", shadow.status(now_ms)["mode"]);
            let _ = SHADOW.set(Mutex::new(shadow));
        }
    }
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_millis(POLL_MS));
        loop {
            tick.tick().await;
            let (update, source) = {
                let s = state.read().await;
                (s.latest_update.clone(), s.effective_source())
            };
            let (Some(update), Some(shadow)) = (update, SHADOW.get()) else { continue };
            let Some(nodes) = update.node_features.as_ref() else { continue };
            let timestamp_ms = (update.timestamp * 1000.0).round() as i64;
            let offline = source.ends_with(":offline");
            let synthetic = !crate::source_fusion::is_hardware_source(&source);
            let Ok(mut shadow) = shadow.lock() else { continue };
            for nf in nodes {
                shadow.observe(&Observation {
                    timestamp_ms,
                    node_id: nf.node_id,
                    source: &source,
                    synthetic,
                    stale: nf.stale || offline,
                    features: NodeFeatures {
                        mean_rssi: nf.features.mean_rssi,
                        variance: nf.features.variance,
                        motion_band_power: nf.features.motion_band_power,
                        breathing_band_power: nf.features.breathing_band_power,
                        dominant_freq_hz: nf.features.dominant_freq_hz,
                        change_points: nf.features.change_points as f64,
                        spectral_power: nf.features.spectral_power,
                        rssi_dbm: nf.rssi_dbm,
                    },
                    presence: nf.classification.presence,
                    confidence: nf.classification.confidence,
                    motion_level: &nf.classification.motion_level,
                });
            }
            shadow.flush();
        }
    });
}

/// `GET /api/v1/shadow/fann`.
pub async fn status_endpoint() -> axum::Json<serde_json::Value> {
    let now_ms = chrono::Utc::now().timestamp_millis();
    axum::Json(match SHADOW.get().and_then(|m| m.lock().ok()) {
        Some(shadow) => shadow.status(now_ms),
        None => json!({
            "enabled": false,
            "hint": format!("set {HEAD_ENV} and/or {LOG_ENV}"),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruview_fann_head::train::{fit, FitConfig};
    use ruview_fann_head::HeadContext;
    use std::sync::Arc;

    #[derive(Clone, Default)]
    struct Sink(Arc<Mutex<Vec<u8>>>);
    impl Write for Sink {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    impl Sink {
        fn lines(&self) -> Vec<serde_json::Value> {
            String::from_utf8(self.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .map(|l| serde_json::from_str(l).unwrap())
                .collect()
        }
    }

    fn features(motion: f64) -> NodeFeatures {
        NodeFeatures {
            mean_rssi: -60.0,
            variance: 2.0,
            motion_band_power: motion,
            breathing_band_power: 0.3,
            dominant_freq_hz: 0.3,
            change_points: 2.0,
            spectral_power: 500.0,
            rssi_dbm: -61.0,
        }
    }

    /// SYNTHETIC head: occupied when motion band power is high.
    fn head_json(schema: &str, source: &str) -> String {
        let mut x = Vec::new();
        let mut labels = Vec::new();
        for i in 0..64 {
            let occupied = i % 2 == 0;
            let motion = if occupied { 4000.0 + i as f64 } else { 1.0 + i as f64 * 0.01 };
            x.extend(encode(&features(motion)).unwrap());
            labels.push(occupied);
        }
        let context = HeadContext {
            feature_schema: schema.into(),
            sources: vec![source.into()],
            room_revision: "test".into(),
        };
        fit(&x, &labels, context, FitConfig::candidate()).unwrap().head.to_json()
    }

    fn obs(ts: i64, node: u8, motion: f64, presence: bool) -> Observation<'static> {
        Observation {
            timestamp_ms: ts,
            node_id: node,
            source: "esp32",
            synthetic: false,
            stale: false,
            features: features(motion),
            presence,
            confidence: 0.8,
            motion_level: if presence { "present_still" } else { "absent" },
        }
    }

    #[test]
    fn adr426_shadow_scores_logs_and_counts_disagreement() {
        let sink = Sink::default();
        let json = head_json(NODE_FEATURE_SCHEMA, "csi-node-1");
        let mut s = Shadow::new(Some(&json), Some(Box::new(sink.clone())), 0, 0).unwrap();
        s.observe(&obs(1000, 1, 5000.0, true)); // agree
        s.observe(&obs(2000, 1, 1.0, false)); // agree
        s.observe(&obs(3000, 1, 5000.0, false)); // candidate occupied, server empty
        s.observe(&obs(3000, 1, 5000.0, false)); // same update again
        s.observe(&obs(4000, 2, 5000.0, true)); // a node the head was not fitted for
        let mut stale = obs(5000, 1, 5000.0, true);
        stale.stale = true;
        s.observe(&stale);
        let mut broken = obs(6000, 1, f64::NAN, true);
        broken.synthetic = true;
        s.observe(&broken);

        let status = s.status(10_000);
        assert_eq!(status["mode"], "shadow");
        assert_eq!(status["counters"]["observations"], 4);
        assert_eq!(status["counters"]["skipped_duplicate"], 1);
        assert_eq!(status["counters"]["skipped_stale"], 1);
        assert_eq!(status["counters"]["skipped_non_finite"], 1);
        assert_eq!(status["counters"]["skipped_other_node"], 1);
        let c = &status["candidate"]["counters"];
        assert_eq!((c["agree"].as_u64(), c["disagree"].as_u64()), (Some(2), Some(1)));
        assert!((status["candidate"]["disagreement_rate"].as_f64().unwrap() - 1.0 / 3.0).abs() < 1e-9);

        let lines = sink.lines();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0]["features"].as_array().unwrap().len(), NODE_FEATURE_COUNT);
        assert_eq!(lines[2]["candidate"]["decision"], "occupied");
        assert_eq!(lines[2]["incumbent"]["presence"], false);
        // The other node is captured, but not scored.
        assert!(lines[3]["candidate"].is_null());
    }

    #[test]
    fn adr426_capture_mode_logs_without_a_candidate_and_respects_the_cap() {
        let sink = Sink::default();
        let mut s = Shadow::new(None, Some(Box::new(sink.clone())), 0, 0).unwrap();
        s.max_log_bytes = 600;
        for i in 0..5 {
            s.observe(&obs(1000 * (i + 1), 1, 10.0, false));
        }
        let status = s.status(0);
        assert_eq!(status["mode"], "capture");
        assert!(status["candidate"].is_null());
        let logged = status["counters"]["logged"].as_u64().unwrap();
        let dropped = status["counters"]["log_dropped"].as_u64().unwrap();
        assert!(logged >= 1 && dropped >= 1 && logged + dropped == 5, "{status}");
        assert_eq!(sink.lines().len() as u64, logged);
    }

    #[test]
    fn adr426_a_head_for_another_schema_or_source_naming_is_refused() {
        assert!(Shadow::new(Some(&head_json("aether-skip/1", "csi-node-1")), None, 0, 0).is_err());
        assert!(Shadow::new(Some(&head_json(NODE_FEATURE_SCHEMA, "radar-1")), None, 0, 0).is_err());
        assert!(Shadow::new(Some("{not json"), None, 0, 0).is_err());
    }
}
