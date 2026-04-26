//! Satellite tile fetcher — XYZ/TMS tile download with caching.

use crate::cache::TileCache;
use crate::coord;
use crate::types::{GeoBBox, RasterTile, TileCoord};
use anyhow::Result;

/// Tile provider (all free, no API keys).
pub enum TileProvider {
    /// Sentinel-2 cloudless mosaic (EOX, 10m, CC-BY-4.0)
    Sentinel2Cloudless,
    /// ESRI World Imagery (sub-meter, free tier)
    EsriWorldImagery,
    /// OpenStreetMap (map tiles, not satellite)
    Osm,
}

impl TileProvider {
    pub fn url(&self, coord: &TileCoord) -> String {
        match self {
            Self::Sentinel2Cloudless => format!(
                "https://tiles.maps.eox.at/wmts/1.0.0/s2cloudless-2021_3857/default/g/{}/{}/{}.jpg",
                coord.z, coord.y, coord.x
            ),
            Self::EsriWorldImagery => format!(
                "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/{}/{}/{}",
                coord.z, coord.y, coord.x
            ),
            Self::Osm => format!(
                "https://tile.openstreetmap.org/{}/{}/{}.png",
                coord.z, coord.x, coord.y
            ),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Self::Sentinel2Cloudless => "sentinel2",
            Self::EsriWorldImagery => "esri",
            Self::Osm => "osm",
        }
    }
}

/// Fetch a single tile with caching.
pub async fn fetch_tile(provider: &TileProvider, coord: &TileCoord, cache: &TileCache) -> Result<RasterTile> {
    let cache_key = format!("tiles_{}_{}_{}.dat", coord.z, coord.x, coord.y);

    if let Some(data) = cache.get(&cache_key) {
        return Ok(RasterTile { coord: coord.clone(), data, bounds: coord::tile_bounds(coord) });
    }

    let url = provider.url(coord);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("RuView/0.1 (https://github.com/ruvnet/RuView)")
        .build()?;

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Tile fetch failed: {} → {}", url, resp.status());
    }
    let data = resp.bytes().await?.to_vec();
    cache.put(&cache_key, &data)?;

    Ok(RasterTile { coord: coord.clone(), data, bounds: coord::tile_bounds(coord) })
}

/// Fetch all tiles covering a bounding box.
pub async fn fetch_area(provider: &TileProvider, bbox: &GeoBBox, zoom: u8, cache: &TileCache) -> Result<Vec<RasterTile>> {
    let coords = coord::tiles_for_bbox(bbox, zoom);
    let mut tiles = Vec::with_capacity(coords.len());
    for c in &coords {
        match fetch_tile(provider, c, cache).await {
            Ok(t) => tiles.push(t),
            Err(e) => eprintln!("  Tile {}/{}/{} failed: {}", c.z, c.x, c.y, e),
        }
    }
    Ok(tiles)
}
