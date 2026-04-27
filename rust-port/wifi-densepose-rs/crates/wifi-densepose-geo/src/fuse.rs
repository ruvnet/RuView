//! Multi-source fusion — satellite + terrain + OSM + local sensor data.

use crate::cache::TileCache;
use crate::types::*;
use crate::{locate, osm, terrain, tiles};
use anyhow::Result;

/// Build a complete geo scene for a location.
pub async fn build_scene(radius_m: f64) -> Result<GeoScene> {
    let cache = TileCache::default_cache();

    // 1. Locate
    let cache_path = cache.base_dir.join("location.json");
    let location = locate::get_location(cache_path.to_str().unwrap_or("")).await?;
    eprintln!("  Geo: located at {:.4}N, {:.4}W", location.lat, location.lon);

    // 2. Fetch satellite tiles
    let bbox = GeoBBox::from_center(&location, radius_m);
    let tile_list = tiles::fetch_area(&tiles::TileProvider::Sentinel2Cloudless, &bbox, 16, &cache).await?;
    eprintln!("  Geo: fetched {} satellite tiles", tile_list.len());

    // 3. Fetch elevation
    let dem = terrain::fetch_elevation(&location, &cache).await?;
    let elevation = terrain::elevation_at(&dem, &location);
    eprintln!("  Geo: elevation {:.0}m ASL", elevation);

    // 4. Fetch OSM buildings + roads
    let buildings = osm::fetch_buildings(&location, radius_m).await.unwrap_or_default();
    let roads = osm::fetch_roads(&location, radius_m).await.unwrap_or_default();
    eprintln!("  Geo: {} buildings, {} roads", buildings.len(), roads.len());

    // 5. Build registration
    let mut reg_origin = location.clone();
    reg_origin.alt = elevation as f64;
    let registration = crate::register::auto_register(&reg_origin);

    Ok(GeoScene {
        location: reg_origin,
        bbox,
        elevation_m: elevation,
        buildings,
        roads,
        tile_count: tile_list.len(),
        registration,
        last_updated: chrono::Utc::now().to_rfc3339(),
    })
}

/// Generate a text summary of the geo scene.
pub fn summarize(scene: &GeoScene) -> String {
    let building_count = scene.buildings.len();
    let road_count = scene.roads.len();
    let road_names: Vec<&str> = scene.roads.iter()
        .filter_map(|r| match r {
            OsmFeature::Road { name, .. } => name.as_deref(),
            _ => None,
        })
        .take(3)
        .collect();

    format!(
        "Location: {:.4}N, {:.4}W, elevation {:.0}m ASL. \
         {} buildings within view. {} roads nearby{}. \
         {} satellite tiles at zoom 16. Updated: {}.",
        scene.location.lat, scene.location.lon, scene.elevation_m,
        building_count, road_count,
        if road_names.is_empty() { String::new() }
        else { format!(" ({})", road_names.join(", ")) },
        scene.tile_count,
        &scene.last_updated[..10],
    )
}
