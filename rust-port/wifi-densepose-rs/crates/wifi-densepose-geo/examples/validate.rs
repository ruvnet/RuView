use wifi_densepose_geo::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║  ruview-geo — Real Data Validation           ║");
    println!("╚══════════════════════════════════════════════╝\n");

    let t0 = std::time::Instant::now();
    let cache = cache::TileCache::new("/tmp/ruview-geo-validate");

    let loc = locate::get_location(&format!("{}/location.json", cache.base_dir.display())).await?;
    println!("  Location: {:.4}N, {:.4}W", loc.lat, loc.lon);

    let bbox = GeoBBox::from_center(&loc, 300.0);
    let tiles_list = tiles::fetch_area(&tiles::TileProvider::Sentinel2Cloudless, &bbox, 16, &cache).await?;
    println!("  Tiles: {} ({:.0}KB)", tiles_list.len(),
        tiles_list.iter().map(|t| t.data.len()).sum::<usize>() as f64 / 1024.0);

    let dem = terrain::fetch_elevation(&loc, &cache).await?;
    println!("  Elevation: {:.0}m (grid {}x{})", terrain::elevation_at(&dem, &loc), dem.cols, dem.rows);

    let buildings = osm::fetch_buildings(&loc, 300.0).await.unwrap_or_default();
    let roads = osm::fetch_roads(&loc, 300.0).await.unwrap_or_default();
    println!("  OSM: {} buildings, {} roads", buildings.len(), roads.len());

    let weather = temporal::fetch_weather(&loc).await?;
    println!("  Weather: {:.0}°C humidity={:.0}% wind={:.1}m/s",
        weather.temperature_c, weather.humidity_pct, weather.wind_speed_ms);

    let scene = GeoScene {
        location: loc.clone(), bbox, elevation_m: terrain::elevation_at(&dem, &loc),
        buildings, roads, tile_count: tiles_list.len(),
        registration: register::auto_register(&loc),
        last_updated: chrono::Utc::now().to_rfc3339(),
    };
    println!("\n  {}", fuse::summarize(&scene));

    match brain::store_geo_context(&scene).await {
        Ok(n) => println!("  Brain: {} memories stored", n),
        Err(e) => println!("  Brain: {e}"),
    }

    println!("\n  Total: {}ms | Cache: {:.0}KB",
        t0.elapsed().as_millis(), cache.size_bytes() as f64 / 1024.0);
    Ok(())
}
