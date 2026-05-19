//! Structured JSON event publisher — one line per event on stdout.
//!
//! Format is the ADR-100 runtime contract: `{ts, level, event, fields}`.

use serde::Serialize;
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize)]
pub struct Event<'a> {
    pub ts: f64,
    pub level: &'a str,
    pub event: &'a str,
    pub fields: Value,
}

impl<'a> Event<'a> {
    pub fn health_ok(cog_id: &'a str) -> Self {
        Self {
            ts: now_secs(),
            level: "info",
            event: "health.ok",
            fields: serde_json::json!({ "cog": cog_id }),
        }
    }

    pub fn run_started(cog_id: &'a str, cfg: &crate::config::CogConfig) -> Self {
        Self {
            ts: now_secs(),
            level: "info",
            event: "run.started",
            fields: serde_json::json!({
                "cog": cog_id,
                "sensing_url": cfg.sensing_url,
                "model_path": cfg.model_path,
                "poll_ms": cfg.poll_ms,
            }),
        }
    }

    pub fn pose_frame(tick: u64, n_persons: usize, persons: Value) -> Self {
        Self {
            ts: now_secs(),
            level: "info",
            event: "pose.frame",
            fields: serde_json::json!({
                "tick": tick,
                "n_persons": n_persons,
                "persons": persons,
            }),
        }
    }
}

pub fn emit_event(ev: &Event<'_>) {
    if let Ok(line) = serde_json::to_string(ev) {
        println!("{line}");
    }
}

fn now_secs() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}
