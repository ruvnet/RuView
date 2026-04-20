//! Brain bridge — sends spatial observations to the ruOS brain.
//!
//! Periodically summarizes the sensor pipeline state and stores it
//! as brain memories for the agent to reason about.

use crate::csi_pipeline::PipelineOutput;
use anyhow::Result;

const BRAIN_URL: &str = "http://127.0.0.1:9876";

/// Store a spatial observation in the brain.
async fn store_memory(category: &str, content: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let body = serde_json::json!({
        "category": category,
        "content": content,
    });

    client.post(format!("{BRAIN_URL}/memories"))
        .json(&body)
        .send()
        .await?;
    Ok(())
}

/// Summarize pipeline state and store in brain (called every 60 seconds).
pub async fn sync_to_brain(pipeline: &PipelineOutput, camera_frames: u64) {
    // Only store if there's meaningful data
    if pipeline.total_frames < 10 && camera_frames < 5 { return; }

    // Store spatial summary
    let motion_str = if pipeline.motion_detected { "detected" } else { "absent" };
    let skeleton_str = if let Some(ref sk) = pipeline.skeleton {
        format!("{} keypoints ({:.0}% conf)", sk.keypoints.len(), sk.confidence * 100.0)
    } else {
        "inactive".to_string()
    };

    let summary = format!(
        "Room scan: {} camera frames, {} CSI frames from {} nodes. \
         Motion {} ({:.0}%). Breathing {:.0} BPM. Skeleton: {}. \
         Occupancy grid {}x{}x{} with {} occupied voxels.",
        camera_frames,
        pipeline.total_frames,
        pipeline.num_nodes,
        motion_str,
        pipeline.vitals.motion_score * 100.0,
        pipeline.vitals.breathing_rate,
        skeleton_str,
        pipeline.occupancy_dims.0,
        pipeline.occupancy_dims.1,
        pipeline.occupancy_dims.2,
        pipeline.occupancy.iter().filter(|&&d| d > 0.3).count(),
    );

    let _ = store_memory("spatial-observation", &summary).await;

    // Store motion events
    if pipeline.motion_detected && pipeline.vitals.motion_score > 0.3 {
        let _ = store_memory("spatial-motion",
            &format!("Strong motion detected: {:.0}% score, {} CSI frames",
                pipeline.vitals.motion_score * 100.0, pipeline.total_frames)
        ).await;
    }

    // Store vital signs if available
    if pipeline.vitals.breathing_rate > 5.0 && pipeline.vitals.breathing_rate < 35.0 {
        let _ = store_memory("spatial-vitals",
            &format!("Vital signs: breathing {:.0} BPM, motion {:.0}%",
                pipeline.vitals.breathing_rate, pipeline.vitals.motion_score * 100.0)
        ).await;
    }
}

/// Check if brain is reachable.
pub async fn brain_available() -> bool {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()
        .and_then(|c| {
            tokio::runtime::Handle::current().block_on(async {
                c.get(format!("{BRAIN_URL}/health")).send().await.ok()
            })
        })
        .is_some()
}
