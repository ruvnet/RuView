//! Temporal change tracking — detect changes in satellite/OSM/weather over time.

use crate::cache::TileCache;
use crate::types::GeoPoint;
#[allow(unused_imports)]
use crate::types::GeoScene;
use anyhow::Result;

/// Fetch current weather (Open Meteo, free, no key).
pub async fn fetch_weather(point: &GeoPoint) -> Result<WeatherData> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current=temperature_2m,relative_humidity_2m,wind_speed_10m,weather_code",
        point.lat, point.lon
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    let current = resp.get("current").cloned().unwrap_or(serde_json::json!({}));

    Ok(WeatherData {
        temperature_c: current.get("temperature_2m").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        humidity_pct: current.get("relative_humidity_2m").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        wind_speed_ms: current.get("wind_speed_10m").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        weather_code: current.get("weather_code").and_then(|v| v.as_u64()).unwrap_or(0) as u16,
    })
}

/// Check for OSM changes since last fetch.
pub async fn check_osm_changes(scene: &GeoScene, cache: &TileCache) -> Result<Vec<String>> {
    let mut changes = Vec::new();

    let cache_key = "osm_building_count";
    let prev_count: usize = cache.get(cache_key)
        .and_then(|d| String::from_utf8(d).ok())
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let current_count = scene.buildings.len();
    if prev_count > 0 && current_count != prev_count {
        let diff = current_count as i64 - prev_count as i64;
        changes.push(format!("Building count changed: {} → {} ({:+})", prev_count, current_count, diff));
    }

    cache.put(cache_key, current_count.to_string().as_bytes())?;
    Ok(changes)
}

/// Generate temporal summary for brain storage.
pub fn temporal_summary(weather: &WeatherData, changes: &[String]) -> String {
    let weather_desc = match weather.weather_code {
        0 => "clear sky",
        1..=3 => "partly cloudy",
        45 | 48 => "foggy",
        51..=57 => "drizzle",
        61..=67 => "rain",
        71..=77 => "snow",
        80..=82 => "showers",
        95..=99 => "thunderstorm",
        _ => "unknown",
    };

    let mut summary = format!(
        "Weather: {:.0}°C, {weather_desc}, humidity {:.0}%, wind {:.1}m/s.",
        weather.temperature_c, weather.humidity_pct, weather.wind_speed_ms,
    );

    for change in changes {
        summary.push_str(&format!(" Change: {change}."));
    }

    summary
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct WeatherData {
    pub temperature_c: f32,
    pub humidity_pct: f32,
    pub wind_speed_ms: f32,
    pub weather_code: u16,
}

// ---------------------------------------------------------------------------
// Satellite tile change detection
// ---------------------------------------------------------------------------

/// Result of comparing two tile snapshots.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TileChangeResult {
    /// 0.0 = identical, 1.0 = completely different.
    pub diff_score: f64,
    /// Number of pixels that changed.
    pub changed_pixels: usize,
    /// Total pixels compared.
    pub total_pixels: usize,
}

/// Compare a newly-fetched tile against its previously-cached version.
///
/// Returns a `TileChangeResult` with a diff score between 0.0 (identical) and
/// 1.0 (completely different).  When the diff exceeds 0.1 the function stores
/// a change event as a brain memory via the local ruOS brain endpoint.
pub async fn detect_tile_changes(
    cache_key: &str,
    new_data: &[u8],
    cache: &TileCache,
) -> Result<TileChangeResult> {
    let previous = cache.get(cache_key);

    let result = match previous {
        Some(ref old_data) => {
            let total = old_data.len().max(new_data.len()).max(1);
            let comparable = old_data.len().min(new_data.len());
            let mut changed: usize = 0;
            for i in 0..comparable {
                if old_data[i] != new_data[i] {
                    changed += 1;
                }
            }
            // Any extra bytes in the longer slice count as changed.
            changed += total - comparable;

            TileChangeResult {
                diff_score: changed as f64 / total as f64,
                changed_pixels: changed,
                total_pixels: total,
            }
        }
        None => {
            // No previous data — treat as fully new (score 1.0).
            TileChangeResult {
                diff_score: 1.0,
                changed_pixels: new_data.len(),
                total_pixels: new_data.len().max(1),
            }
        }
    };

    // Persist new snapshot into cache for future comparisons.
    cache.put(cache_key, new_data)?;

    // When significant change is detected, store a brain memory.
    if result.diff_score > 0.1 {
        let _ = store_change_event(cache_key, &result).await;
    }

    Ok(result)
}

/// Post a change event to the local ruOS brain.
async fn store_change_event(cache_key: &str, result: &TileChangeResult) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()?;

    let body = serde_json::json!({
        "category": "spatial-change",
        "content": format!(
            "Tile change detected for {cache_key}: diff={:.3}, changed={}/{}",
            result.diff_score, result.changed_pixels, result.total_pixels,
        ),
    });

    client
        .post("http://127.0.0.1:9876/memories")
        .json(&body)
        .send()
        .await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Night mode detection
// ---------------------------------------------------------------------------

/// Approximate check whether the current time is "night" at a given latitude.
///
/// Uses a simplified sunrise/sunset model based on the solar declination and
/// hour angle.  When it is night the system should rely on CSI data only
/// (satellite imagery is not useful in darkness).
pub fn is_night(lat_deg: f64) -> bool {
    let now = chrono::Utc::now();
    is_night_at(lat_deg, now)
}

/// Testable version of [`is_night`] that accepts an explicit timestamp.
pub fn is_night_at(lat_deg: f64, utc: chrono::DateTime<chrono::Utc>) -> bool {
    use chrono::Datelike;
    use std::f64::consts::PI;

    let day_of_year = utc.ordinal() as f64;
    let hour_utc = utc.timestamp() % 86400;
    let solar_hour = (hour_utc as f64) / 3600.0; // 0..24

    // Solar declination (Spencer, 1971 — simplified)
    let gamma = 2.0 * PI * (day_of_year - 1.0) / 365.0;
    let decl = 0.006918
        - 0.399912 * gamma.cos()
        + 0.070257 * gamma.sin()
        - 0.006758 * (2.0 * gamma).cos()
        + 0.000907 * (2.0 * gamma).sin();

    let lat_rad = lat_deg.to_radians();

    // Cosine of the hour angle at sunrise/sunset (geometric, no refraction)
    let cos_ha = -(lat_rad.tan() * decl.tan());

    // Polar day / polar night
    if cos_ha < -1.0 {
        return false; // midnight sun — never night
    }
    if cos_ha > 1.0 {
        return true; // polar night — always night
    }

    let ha_sunrise = cos_ha.acos(); // radians, symmetric about solar noon
    let daylight_hours = 2.0 * ha_sunrise * 12.0 / PI;
    let solar_noon = 12.0; // approximation (ignores longitude offset)
    let sunrise = solar_noon - daylight_hours / 2.0;
    let sunset = solar_noon + daylight_hours / 2.0;

    solar_hour < sunrise || solar_hour > sunset
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_night_at_equator_noon() {
        // Noon UTC at equator on March 20 — should be daytime.
        let dt = chrono::NaiveDate::from_ymd_opt(2025, 3, 20)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        assert!(!is_night_at(0.0, dt));
    }

    #[test]
    fn test_is_night_at_equator_midnight() {
        // Midnight UTC at equator — should be night.
        let dt = chrono::NaiveDate::from_ymd_opt(2025, 3, 20)
            .unwrap()
            .and_hms_opt(2, 0, 0)
            .unwrap()
            .and_utc();
        assert!(is_night_at(0.0, dt));
    }

    #[test]
    fn test_midnight_sun_arctic() {
        // Late June at 70 N — midnight sun, never night.
        let dt = chrono::NaiveDate::from_ymd_opt(2025, 6, 21)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        assert!(!is_night_at(70.0, dt));
    }

    #[test]
    fn test_polar_night_arctic() {
        // Late December at 80 N — polar night, always night.
        let dt = chrono::NaiveDate::from_ymd_opt(2025, 12, 21)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc();
        assert!(is_night_at(80.0, dt));
    }

    #[test]
    fn test_detect_tile_changes_identical() {
        let cache = TileCache::new("/tmp/ruview-test-tile-changes");
        let data = vec![1u8, 2, 3, 4, 5];
        // Prime the cache.
        cache.put("test_tile_ident", &data).unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(detect_tile_changes("test_tile_ident", &data, &cache)).unwrap();
        assert!((result.diff_score - 0.0).abs() < 1e-9);
        assert_eq!(result.changed_pixels, 0);
    }

    #[test]
    fn test_detect_tile_changes_fully_different() {
        let cache = TileCache::new("/tmp/ruview-test-tile-changes");
        let old = vec![0u8; 100];
        let new = vec![255u8; 100];
        cache.put("test_tile_diff", &old).unwrap();

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let result = rt.block_on(detect_tile_changes("test_tile_diff", &new, &cache)).unwrap();
        assert!((result.diff_score - 1.0).abs() < 1e-9);
    }
}
