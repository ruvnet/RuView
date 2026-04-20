//! IP geolocation — determine location from public IP.

use crate::types::GeoPoint;
use anyhow::Result;

/// Locate by IP address (free, no API key).
pub async fn locate_by_ip() -> Result<GeoPoint> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    // Primary: ip-api.com (free, 45 req/min)
    let resp: serde_json::Value = client
        .get("http://ip-api.com/json/?fields=lat,lon,city,regionName,country")
        .send().await?
        .json().await?;

    let lat = resp.get("lat").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let lon = resp.get("lon").and_then(|v| v.as_f64()).unwrap_or(0.0);

    if lat == 0.0 && lon == 0.0 {
        anyhow::bail!("IP geolocation returned (0,0)");
    }

    Ok(GeoPoint { lat, lon, alt: 0.0 })
}

/// Get location with caching.
pub async fn get_location(cache_path: &str) -> Result<GeoPoint> {
    // Check cache
    if let Ok(data) = std::fs::read_to_string(cache_path) {
        if let Ok(point) = serde_json::from_str::<GeoPoint>(&data) {
            return Ok(point);
        }
    }

    let point = locate_by_ip().await?;
    let _ = std::fs::write(cache_path, serde_json::to_string(&point)?);
    Ok(point)
}
