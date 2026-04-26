//! Brain integration — store geospatial context in ruOS brain.
//!
//! Brain URL is read from `RUVIEW_BRAIN_URL` env var (default
//! `http://127.0.0.1:9876`). The resolved URL is logged once on first use.

use crate::fuse;
use crate::types::GeoScene;
use anyhow::Result;
use std::sync::OnceLock;

const DEFAULT_BRAIN_URL: &str = "http://127.0.0.1:9876";

pub(crate) fn brain_url() -> &'static str {
    static BRAIN_URL: OnceLock<String> = OnceLock::new();
    BRAIN_URL.get_or_init(|| {
        let url = std::env::var("RUVIEW_BRAIN_URL")
            .unwrap_or_else(|_| DEFAULT_BRAIN_URL.to_string());
        eprintln!("  wifi-densepose-geo: using brain URL {url}");
        url
    })
}

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
    if client.post(format!("{}/memories", brain_url())).json(&body).send().await.is_ok() {
        stored += 1;
    }

    Ok(stored)
}
