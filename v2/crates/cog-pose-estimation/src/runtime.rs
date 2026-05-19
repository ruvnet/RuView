//! Long-running inference loop. Polls the appliance's sensing-server,
//! runs a CSI window through the engine, emits `pose.frame` events.

use crate::config::CogConfig;
use crate::inference::{CsiWindow, InferenceEngine, INPUT_SUBCARRIERS, INPUT_TIMESTEPS};
use crate::publisher::{emit_event, Event};
use std::time::Duration;
use tokio::time::sleep;

pub async fn run_loop(
    cfg: CogConfig,
    engine: InferenceEngine,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut buffer: Vec<f32> = Vec::with_capacity(INPUT_SUBCARRIERS * INPUT_TIMESTEPS);
    let mut tick: u64 = 0;

    loop {
        // Poll one frame from the sensing-server. On error, sleep and retry —
        // we expect transient blips when the server restarts.
        match fetch_frame(&cfg.sensing_url).await {
            Ok(amplitudes) => {
                tick += 1;
                buffer.extend(amplitudes);
                // Slide-window: keep only the most recent N*T values
                let cap = INPUT_SUBCARRIERS * INPUT_TIMESTEPS;
                if buffer.len() >= cap {
                    let window = CsiWindow {
                        data: buffer.split_off(buffer.len() - cap),
                    };
                    if let Ok(out) = engine.infer(&window) {
                        if out.confidence >= cfg.min_confidence {
                            // Flatten persons array (single-person v0.0.1)
                            let persons = serde_json::json!([{
                                "keypoints": chunk_pairs(&out.keypoints),
                                "confidence": out.confidence,
                            }]);
                            emit_event(&Event::pose_frame(tick, 1, persons));
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "sensing-server fetch failed");
            }
        }
        sleep(Duration::from_millis(cfg.poll_ms)).await;
    }
}

async fn fetch_frame(url: &str) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    // Synchronous ureq inside an async fn — we accept the blocking call
    // here because the per-frame cost (~1 ms loopback) is dwarfed by the
    // inference cost. Replace with a proper async client if we ever poll
    // remote sensing-servers over the wire.
    let url = url.to_string();
    let body = tokio::task::spawn_blocking(move || -> Result<String, ureq::Error> {
        Ok(ureq::get(&url).call()?.into_string()?)
    })
    .await??;
    let json: serde_json::Value = serde_json::from_str(&body)?;
    let snapshot = json.get("snapshot").unwrap_or(&json);
    let nodes = snapshot
        .get("nodes")
        .and_then(|v| v.as_array())
        .ok_or("missing nodes[]")?;
    // Take node 0's amplitude vector — we'll add multi-node fusion later.
    let amplitude = nodes
        .first()
        .and_then(|n| n.get("amplitude"))
        .and_then(|v| v.as_array())
        .ok_or("missing nodes[0].amplitude[]")?;
    Ok(amplitude
        .iter()
        .filter_map(|v| v.as_f64().map(|f| f as f32))
        .collect())
}

fn chunk_pairs(flat: &[f32]) -> Vec<[f32; 2]> {
    flat.chunks_exact(2).map(|c| [c[0], c[1]]).collect()
}
