//! Geo-registration — maps local sensor coordinates to WGS84.

use crate::coord;
use crate::types::{GeoPoint, GeoRegistration};

/// Auto-register using IP location (sensor at IP location, facing north).
pub fn auto_register(ip_location: &GeoPoint) -> GeoRegistration {
    GeoRegistration {
        origin: ip_location.clone(),
        heading_deg: 0.0,
        scale: 1.0,
    }
}

/// Transform local point [x, y, z] to WGS84.
pub fn local_to_wgs84(reg: &GeoRegistration, local: &[f32; 3]) -> GeoPoint {
    let heading_rad = reg.heading_deg.to_radians();
    let cos_h = heading_rad.cos();
    let sin_h = heading_rad.sin();

    // Rotate local by heading (local X → East when heading=0)
    let east = (local[0] as f64 * cos_h - local[2] as f64 * sin_h) * reg.scale;
    let north = (local[0] as f64 * sin_h + local[2] as f64 * cos_h) * reg.scale;
    let up = local[1] as f64 * reg.scale;

    coord::enu_to_wgs84(&[east, north, up], &reg.origin)
}

/// Transform WGS84 to local point.
pub fn wgs84_to_local(reg: &GeoRegistration, geo: &GeoPoint) -> [f32; 3] {
    let enu = coord::wgs84_to_enu(geo, &reg.origin);
    let heading_rad = (-reg.heading_deg).to_radians();
    let cos_h = heading_rad.cos();
    let sin_h = heading_rad.sin();

    let x = ((enu[0] * cos_h - enu[1] * sin_h) / reg.scale) as f32;
    let z = ((enu[0] * sin_h + enu[1] * cos_h) / reg.scale) as f32;
    let y = (enu[2] / reg.scale) as f32;

    [x, y, z]
}
