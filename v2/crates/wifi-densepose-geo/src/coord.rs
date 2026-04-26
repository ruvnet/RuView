//! Coordinate transforms — WGS84, UTM, ENU, tile math.

use crate::types::{GeoPoint, GeoBBox, TileCoord};

const WGS84_A: f64 = 6_378_137.0;
#[allow(dead_code)]
const WGS84_F: f64 = 1.0 / 298.257_223_563;
#[allow(dead_code)]
const WGS84_E2: f64 = 2.0 * WGS84_F - WGS84_F * WGS84_F;

/// Haversine distance in meters.
pub fn haversine(a: &GeoPoint, b: &GeoPoint) -> f64 {
    let dlat = (b.lat - a.lat).to_radians();
    let dlon = (b.lon - a.lon).to_radians();
    let lat1 = a.lat.to_radians();
    let lat2 = b.lat.to_radians();
    let h = (dlat / 2.0).sin().powi(2) + lat1.cos() * lat2.cos() * (dlon / 2.0).sin().powi(2);
    2.0 * WGS84_A * h.sqrt().asin()
}

/// WGS84 to local ENU (East-North-Up) relative to origin, in meters.
pub fn wgs84_to_enu(point: &GeoPoint, origin: &GeoPoint) -> [f64; 3] {
    let dlat = (point.lat - origin.lat).to_radians();
    let dlon = (point.lon - origin.lon).to_radians();
    let lat = origin.lat.to_radians();
    let east = dlon * WGS84_A * lat.cos();
    let north = dlat * WGS84_A;
    let up = point.alt - origin.alt;
    [east, north, up]
}

/// Local ENU to WGS84.
pub fn enu_to_wgs84(enu: &[f64; 3], origin: &GeoPoint) -> GeoPoint {
    let lat = origin.lat.to_radians();
    let dlat = enu[1] / WGS84_A;
    let dlon = enu[0] / (WGS84_A * lat.cos());
    GeoPoint {
        lat: origin.lat + dlat.to_degrees(),
        lon: origin.lon + dlon.to_degrees(),
        alt: origin.alt + enu[2],
    }
}

/// WGS84 to XYZ tile coordinates (Slippy Map).
pub fn wgs84_to_tile(lat: f64, lon: f64, zoom: u8) -> TileCoord {
    let n = 2f64.powi(zoom as i32);
    let x = ((lon + 180.0) / 360.0 * n).floor() as u32;
    let lat_rad = lat.to_radians();
    let y = ((1.0 - lat_rad.tan().asinh() / std::f64::consts::PI) / 2.0 * n).floor() as u32;
    TileCoord { z: zoom, x, y }
}

/// Tile bounds in WGS84.
pub fn tile_bounds(coord: &TileCoord) -> GeoBBox {
    let n = 2f64.powi(coord.z as i32);
    let west = coord.x as f64 / n * 360.0 - 180.0;
    let east = (coord.x + 1) as f64 / n * 360.0 - 180.0;
    let north = (std::f64::consts::PI * (1.0 - 2.0 * coord.y as f64 / n)).sinh().atan().to_degrees();
    let south = (std::f64::consts::PI * (1.0 - 2.0 * (coord.y + 1) as f64 / n)).sinh().atan().to_degrees();
    GeoBBox { south, west, north, east }
}

/// Get all tile coordinates covering a bounding box at a zoom level.
pub fn tiles_for_bbox(bbox: &GeoBBox, zoom: u8) -> Vec<TileCoord> {
    let tl = wgs84_to_tile(bbox.north, bbox.west, zoom);
    let br = wgs84_to_tile(bbox.south, bbox.east, zoom);
    let mut tiles = Vec::new();
    for y in tl.y..=br.y {
        for x in tl.x..=br.x {
            tiles.push(TileCoord { z: zoom, x, y });
        }
    }
    tiles
}
