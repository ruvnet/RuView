//! Background broadcast ticker task for pose and sensing stream clients.

use std::time::Duration;
use crate::SharedState;

/// Periodic broadcast task for ESP32 and buffered frames.
pub async fn broadcast_tick_task(state: SharedState, tick_ms: u64) {
    let mut interval = tokio::time::interval(Duration::from_millis(tick_ms));

    loop {
        interval.tick().await;
        let s = state.read().await;
        if let Some(ref update) = s.latest_update {
            if s.tx.receiver_count() > 0 {
                // Re-broadcast the latest sensing_update so pose WS clients
                // always get data even when ESP32 pauses between frames.
                //
                // Issue #618: overwrite `source` with `effective_source()`
                // before each broadcast so a stale latest_update (frozen
                // payload from a now-offline ESP32) is emitted with
                // `source: "esp32:offline"` instead of `source: "esp32"`.
                let mut tagged = update.clone();
                tagged.source = s.effective_source();
                if let Ok(json) = serde_json::to_string(&tagged) {
                    let _ = s.tx.send(json);
                }
            }
        }
    }
}

