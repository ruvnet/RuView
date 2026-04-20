//! Brain integration — store geospatial context in ruOS brain.

use crate::fuse;
use crate::types::GeoScene;
use anyhow::Result;

const BRAIN_URL: &str = "http://127.0.0.1:9876";

/// Store geospatial context in the brain.
pub async fn store_geo_context(scene: &GeoScene) -> Result<u32> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let mut stored = 0u32;

    // Store location summary
    let summary = fuse::summarize(scene);
    let body = serde_json::json!({
        "category": "spatial-geo",
        "content": summary,
    });
    if client.post(format!("{BRAIN_URL}/memories")).json(&body).send().await.is_ok() {
        stored += 1;
    }

    Ok(stored)
}
