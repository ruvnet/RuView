//! OpenStreetMap data via Overpass API — buildings, roads, land use.

use crate::types::{GeoBBox, GeoPoint, OsmFeature};
use anyhow::{anyhow, Result};

const OVERPASS_URL: &str = "https://overpass-api.de/api/interpreter";

/// Maximum radius (in metres) accepted by the OSM fetchers. Requests larger
/// than this would produce Overpass queries covering hundreds of square
/// kilometres — which hammers the public endpoint and returns unworkably
/// large response payloads. Callers wanting wider areas must tile the queries.
pub const MAX_RADIUS_M: f64 = 5000.0;

fn check_radius(radius_m: f64) -> Result<()> {
    if !radius_m.is_finite() || radius_m <= 0.0 {
        return Err(anyhow!("radius_m must be positive and finite (got {radius_m})"));
    }
    if radius_m > MAX_RADIUS_M {
        return Err(anyhow!(
            "radius_m {radius_m} exceeds MAX_RADIUS_M ({MAX_RADIUS_M}); \
             tile the query into smaller chunks"
        ));
    }
    Ok(())
}

/// Fetch buildings within radius of a point.
///
/// Uses an inclusive `["building"]` filter that matches all building values
/// (residential, commercial, yes, etc.) and also queries relations for
/// multipolygon buildings.  Default recommended radius: 500 m. Max 5000 m.
pub async fn fetch_buildings(center: &GeoPoint, radius_m: f64) -> Result<Vec<OsmFeature>> {
    check_radius(radius_m)?;
    let bbox = GeoBBox::from_center(center, radius_m);
    let query = format!(
        r#"[out:json][timeout:25];(way["building"]({},{},{},{});relation["building"]({},{},{},{}););out body;>;out skel qt;"#,
        bbox.south, bbox.west, bbox.north, bbox.east,
        bbox.south, bbox.west, bbox.north, bbox.east,
    );
    let resp = overpass_query(&query).await?;
    parse_buildings(&resp)
}

/// Fetch roads within radius. Max 5000 m; returns an error otherwise.
pub async fn fetch_roads(center: &GeoPoint, radius_m: f64) -> Result<Vec<OsmFeature>> {
    check_radius(radius_m)?;
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

/// Parse an Overpass JSON response into building features.
///
/// Returns an error if the response is not a JSON object or is missing the
/// top-level `elements` array (indicative of a malformed/non-Overpass payload).
pub fn parse_overpass_json(data: &serde_json::Value) -> Result<Vec<OsmFeature>> {
    if !data.is_object() || data.get("elements").and_then(|e| e.as_array()).is_none() {
        return Err(anyhow!("malformed Overpass response: missing `elements` array"));
    }
    parse_buildings(data)
}

pub(crate) fn parse_buildings(data: &serde_json::Value) -> Result<Vec<OsmFeature>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_overpass_json_accepts_minimal_fixture() {
        // Minimal fixture: three nodes forming a triangular building.
        let j = serde_json::json!({
            "elements": [
                { "type": "node", "id": 1, "lat": 43.0, "lon": -79.0 },
                { "type": "node", "id": 2, "lat": 43.0001, "lon": -79.0 },
                { "type": "node", "id": 3, "lat": 43.0, "lon": -79.0001 },
                {
                    "type": "way", "id": 100,
                    "nodes": [1, 2, 3, 1],
                    "tags": { "building": "yes", "name": "Test Hall" }
                }
            ]
        });
        let features = parse_overpass_json(&j).expect("minimal payload should parse");
        assert_eq!(features.len(), 1);
        match &features[0] {
            OsmFeature::Building { outline, name, .. } => {
                assert_eq!(outline.len(), 4);
                assert_eq!(name.as_deref(), Some("Test Hall"));
            }
            _ => panic!("expected a Building"),
        }
    }

    #[test]
    fn parse_overpass_json_rejects_malformed() {
        // Missing the `elements` array entirely.
        let j = serde_json::json!({ "version": 0.6 });
        assert!(parse_overpass_json(&j).is_err());
        // Not even an object.
        let arr = serde_json::json!([1, 2, 3]);
        assert!(parse_overpass_json(&arr).is_err());
    }

    #[tokio::test]
    async fn fetch_buildings_rejects_oversized_radius() {
        let center = GeoPoint { lat: 43.0, lon: -79.0, alt: 0.0 };
        let err = fetch_buildings(&center, MAX_RADIUS_M + 1.0).await.err();
        assert!(err.is_some(), "should reject radius > MAX_RADIUS_M");
    }
}
