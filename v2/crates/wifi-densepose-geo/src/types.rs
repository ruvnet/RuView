//! Core geospatial types.

use serde::{Deserialize, Serialize};

/// WGS84 geographic coordinate.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
    pub alt: f64,
}

/// Axis-aligned bounding box in WGS84.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoBBox {
    pub south: f64,
    pub west: f64,
    pub north: f64,
    pub east: f64,
}

impl GeoBBox {
    pub fn from_center(center: &GeoPoint, radius_m: f64) -> Self {
        let dlat = radius_m / 111_320.0;
        let dlon = radius_m / (111_320.0 * center.lat.to_radians().cos());
        Self {
            south: center.lat - dlat,
            west: center.lon - dlon,
            north: center.lat + dlat,
            east: center.lon + dlon,
        }
    }
}

/// XYZ tile address.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TileCoord {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}

/// Satellite raster tile.
#[derive(Clone, Debug)]
pub struct RasterTile {
    pub coord: TileCoord,
    pub data: Vec<u8>,
    pub bounds: GeoBBox,
}

/// Elevation grid from SRTM DEM.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ElevationGrid {
    pub origin_lat: f64,
    pub origin_lon: f64,
    pub cell_size_deg: f64,
    pub cols: usize,
    pub rows: usize,
    pub heights: Vec<f32>,
}

impl ElevationGrid {
    pub fn get(&self, lat: f64, lon: f64) -> Option<f32> {
        let row = ((self.origin_lat + (self.rows as f64 * self.cell_size_deg) - lat) / self.cell_size_deg) as usize;
        let col = ((lon - self.origin_lon) / self.cell_size_deg) as usize;
        if row < self.rows && col < self.cols {
            Some(self.heights[row * self.cols + col])
        } else {
            None
        }
    }
}

/// OpenStreetMap feature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OsmFeature {
    Building {
        outline: Vec<[f64; 2]>,
        height: Option<f32>,
        name: Option<String>,
    },
    Road {
        path: Vec<[f64; 2]>,
        road_type: String,
        name: Option<String>,
    },
}

/// Geo-registration transform.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoRegistration {
    pub origin: GeoPoint,
    pub heading_deg: f64,
    pub scale: f64,
}

impl Default for GeoRegistration {
    fn default() -> Self {
        Self {
            origin: GeoPoint { lat: 0.0, lon: 0.0, alt: 0.0 },
            heading_deg: 0.0,
            scale: 1.0,
        }
    }
}

/// Complete geo scene.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GeoScene {
    pub location: GeoPoint,
    pub bbox: GeoBBox,
    pub elevation_m: f32,
    pub buildings: Vec<OsmFeature>,
    pub roads: Vec<OsmFeature>,
    pub tile_count: usize,
    pub registration: GeoRegistration,
    pub last_updated: String,
}
