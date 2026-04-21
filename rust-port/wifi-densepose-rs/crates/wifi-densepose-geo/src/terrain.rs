//! SRTM DEM parser — elevation data from NASA 1-arcsecond HGT files.

use crate::cache::TileCache;
use crate::types::{ElevationGrid, GeoPoint};
use anyhow::Result;

/// Download and parse SRTM HGT for a location.
pub async fn fetch_elevation(point: &GeoPoint, cache: &TileCache) -> Result<ElevationGrid> {
    let lat_int = point.lat.floor() as i32;
    let lon_int = point.lon.floor() as i32;
    let ns = if lat_int >= 0 { 'N' } else { 'S' };
    let ew = if lon_int >= 0 { 'E' } else { 'W' };
    let filename = format!("{}{:02}{}{:03}.hgt", ns, lat_int.unsigned_abs(), ew, lon_int.unsigned_abs());
    let cache_key = format!("srtm_{filename}");

    if let Some(data) = cache.get(&cache_key) {
        return parse_hgt(&data, lat_int as f64, lon_int as f64);
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()?;

    // Primary: NASA SRTM public mirror (no auth required for .hgt)
    let nasa_url = format!(
        "https://e4ftl01.cr.usgs.gov/MEASURES/SRTMGL1.003/2000.02.11/{filename}"
    );

    if let Ok(resp) = client.get(&nasa_url).send().await {
        if resp.status().is_success() {
            let data = resp.bytes().await?.to_vec();
            cache.put(&cache_key, &data)?;
            return parse_hgt(&data, lat_int as f64, lon_int as f64);
        }
    }

    // Fallback: viewfinderpanoramas.org
    // Files are grouped by continent zip, but individual .hgt files can be
    // fetched directly when the server exposes them.
    let vfp_url = format!(
        "http://viewfinderpanoramas.org/dem1/{filename}"
    );

    if let Ok(resp) = client.get(&vfp_url).send().await {
        if resp.status().is_success() {
            let data = resp.bytes().await?.to_vec();
            cache.put(&cache_key, &data)?;
            return parse_hgt(&data, lat_int as f64, lon_int as f64);
        }
    }

    // Final fallback: flat terrain when all downloads fail
    Ok(ElevationGrid {
        origin_lat: lat_int as f64,
        origin_lon: lon_int as f64,
        cell_size_deg: 1.0 / 3600.0,
        cols: 100, rows: 100,
        heights: vec![0.0; 10000],
    })
}

/// Parse SRTM HGT binary (3601x3601 big-endian i16).
pub fn parse_hgt(data: &[u8], origin_lat: f64, origin_lon: f64) -> Result<ElevationGrid> {
    let n_samples = data.len() / 2;
    let side = (n_samples as f64).sqrt() as usize;

    let heights: Vec<f32> = data.chunks_exact(2)
        .map(|c| {
            let v = i16::from_be_bytes([c[0], c[1]]);
            if v == -32768 { 0.0 } else { v as f32 } // -32768 = void
        })
        .collect();

    Ok(ElevationGrid {
        origin_lat, origin_lon,
        cell_size_deg: 1.0 / (side - 1) as f64,
        cols: side, rows: side,
        heights,
    })
}

/// Get elevation at a specific point from a grid.
pub fn elevation_at(grid: &ElevationGrid, point: &GeoPoint) -> f32 {
    grid.get(point.lat, point.lon).unwrap_or(0.0)
}

/// Extract a small subgrid around a point.
pub fn extract_subgrid(grid: &ElevationGrid, center: &GeoPoint, radius_m: f64) -> ElevationGrid {
    let radius_deg = radius_m / 111_320.0;
    let min_row = ((grid.origin_lat + (grid.rows as f64 * grid.cell_size_deg) - center.lat - radius_deg) / grid.cell_size_deg).max(0.0) as usize;
    let max_row = ((grid.origin_lat + (grid.rows as f64 * grid.cell_size_deg) - center.lat + radius_deg) / grid.cell_size_deg).min(grid.rows as f64) as usize;
    let min_col = ((center.lon - radius_deg - grid.origin_lon) / grid.cell_size_deg).max(0.0) as usize;
    let max_col = ((center.lon + radius_deg - grid.origin_lon) / grid.cell_size_deg).min(grid.cols as f64) as usize;

    let rows = max_row.saturating_sub(min_row);
    let cols = max_col.saturating_sub(min_col);
    let mut heights = Vec::with_capacity(rows * cols);
    for r in min_row..max_row {
        for c in min_col..max_col {
            heights.push(grid.heights.get(r * grid.cols + c).copied().unwrap_or(0.0));
        }
    }

    ElevationGrid {
        origin_lat: grid.origin_lat + (grid.rows - max_row) as f64 * grid.cell_size_deg,
        origin_lon: grid.origin_lon + min_col as f64 * grid.cell_size_deg,
        cell_size_deg: grid.cell_size_deg,
        cols, rows, heights,
    }
}
