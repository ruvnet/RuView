//! OpenStreetMap data via Overpass API — buildings, roads, land use.

use crate::types::{GeoBBox, GeoPoint, OsmFeature};
use anyhow::Result;

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

/// Fetch buildings within radius of a point.
pub async fn fetch_buildings(center: &GeoPoint, radius_m: f64) -> Result<Vec<OsmFeature>> {
    let bbox = GeoBBox::from_center(center, radius_m);
    let query = format!(
        r#"[out:json][timeout:10];way["building"]({},{},{},{});out body;>;out skel qt;"#,
        bbox.south, bbox.west, bbox.north, bbox.east
    );
    let resp = overpass_query(&query).await?;
    parse_buildings(&resp)
}

/// Fetch roads within radius.
pub async fn fetch_roads(center: &GeoPoint, radius_m: f64) -> Result<Vec<OsmFeature>> {
    let bbox = GeoBBox::from_center(center, radius_m);
    let query = format!(
        r#"[out:json][timeout:10];way["highway"]({},{},{},{});out body;>;out skel qt;"#,
        bbox.south, bbox.west, bbox.north, bbox.east
    );
    let resp = overpass_query(&query).await?;
    parse_roads(&resp)
}

async fn overpass_query(query: &str) -> Result<serde_json::Value> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .user_agent("RuView/0.1")
        .build()?;

    let resp = client.post(OVERPASS_URL)
        .form(&[("data", query)])
        .send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Overpass API error: {}", resp.status());
    }
    Ok(resp.json().await?)
}

fn parse_buildings(data: &serde_json::Value) -> Result<Vec<OsmFeature>> {
    let mut buildings = Vec::new();
    let mut nodes: std::collections::HashMap<u64, [f64; 2]> = std::collections::HashMap::new();

    let elements = data.get("elements").and_then(|e| e.as_array()).cloned().unwrap_or_default();

    // First pass: collect nodes
    for el in &elements {
        if el.get("type").and_then(|t| t.as_str()) == Some("node") {
            if let (Some(id), Some(lat), Some(lon)) = (
                el.get("id").and_then(|v| v.as_u64()),
                el.get("lat").and_then(|v| v.as_f64()),
                el.get("lon").and_then(|v| v.as_f64()),
            ) {
                nodes.insert(id, [lat, lon]);
            }
        }
    }

    // Second pass: build ways
    for el in &elements {
        if el.get("type").and_then(|t| t.as_str()) != Some("way") { continue; }
        let tags = el.get("tags").cloned().unwrap_or(serde_json::json!({}));
        if tags.get("building").is_none() { continue; }

        let node_ids = el.get("nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default();
        let outline: Vec<[f64; 2]> = node_ids.iter()
            .filter_map(|id| id.as_u64().and_then(|id| nodes.get(&id).copied()))
            .collect();

        if outline.len() < 3 { continue; }

        let height = tags.get("height").and_then(|h| h.as_str())
            .and_then(|s| s.trim_end_matches('m').trim().parse::<f32>().ok())
            .or(Some(8.0)); // default building height

        let name = tags.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());

        buildings.push(OsmFeature::Building { outline, height, name });
    }

    Ok(buildings)
}

fn parse_roads(data: &serde_json::Value) -> Result<Vec<OsmFeature>> {
    let mut roads = Vec::new();
    let mut nodes: std::collections::HashMap<u64, [f64; 2]> = std::collections::HashMap::new();

    let elements = data.get("elements").and_then(|e| e.as_array()).cloned().unwrap_or_default();

    for el in &elements {
        if el.get("type").and_then(|t| t.as_str()) == Some("node") {
            if let (Some(id), Some(lat), Some(lon)) = (
                el.get("id").and_then(|v| v.as_u64()),
                el.get("lat").and_then(|v| v.as_f64()),
                el.get("lon").and_then(|v| v.as_f64()),
            ) {
                nodes.insert(id, [lat, lon]);
            }
        }
    }

    for el in &elements {
        if el.get("type").and_then(|t| t.as_str()) != Some("way") { continue; }
        let tags = el.get("tags").cloned().unwrap_or(serde_json::json!({}));
        let highway = tags.get("highway").and_then(|h| h.as_str());
        if highway.is_none() { continue; }

        let node_ids = el.get("nodes").and_then(|n| n.as_array()).cloned().unwrap_or_default();
        let path: Vec<[f64; 2]> = node_ids.iter()
            .filter_map(|id| id.as_u64().and_then(|id| nodes.get(&id).copied()))
            .collect();

        if path.len() < 2 { continue; }

        let name = tags.get("name").and_then(|n| n.as_str()).map(|s| s.to_string());

        roads.push(OsmFeature::Road {
            path,
            road_type: highway.unwrap_or("unknown").to_string(),
            name,
        });
    }

    Ok(roads)
}
