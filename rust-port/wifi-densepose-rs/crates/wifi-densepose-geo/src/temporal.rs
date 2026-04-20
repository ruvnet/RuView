//! Temporal change tracking — detect changes in satellite/OSM/weather over time.

use crate::cache::TileCache;
use crate::types::{GeoPoint, GeoScene};
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
