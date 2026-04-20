use ruview_geo::*;
use ruview_geo::coord;

#[test]
fn test_haversine() {
    let toronto = GeoPoint { lat: 43.6532, lon: -79.3832, alt: 0.0 };
    let ottawa = GeoPoint { lat: 45.4215, lon: -75.6972, alt: 0.0 };
    let dist = coord::haversine(&toronto, &ottawa);
    assert!((dist - 353_000.0).abs() < 5_000.0, "Toronto-Ottawa ~353km, got {:.0}m", dist);
}

#[test]
fn test_wgs84_to_enu() {
    let origin = GeoPoint { lat: 43.0, lon: -79.0, alt: 100.0 };
    let point = GeoPoint { lat: 43.001, lon: -79.0, alt: 100.0 };
    let enu = coord::wgs84_to_enu(&point, &origin);
    assert!((enu[1] - 111.0).abs() < 5.0, "0.001 deg lat ~111m north, got {:.1}m", enu[1]);
    assert!(enu[0].abs() < 1.0, "same longitude should have ~0 east, got {:.1}m", enu[0]);
}

#[test]
fn test_enu_roundtrip() {
    let origin = GeoPoint { lat: 43.6532, lon: -79.3832, alt: 76.0 };
    let local = [100.0, 200.0, 5.0]; // 100m east, 200m north, 5m up
    let geo = coord::enu_to_wgs84(&local, &origin);
    let back = coord::wgs84_to_enu(&geo, &origin);
    assert!((back[0] - local[0]).abs() < 0.01);
    assert!((back[1] - local[1]).abs() < 0.01);
    assert!((back[2] - local[2]).abs() < 0.01);
}

#[test]
fn test_tile_coords() {
    let tile = coord::wgs84_to_tile(43.6532, -79.3832, 16);
    assert!(tile.x > 0 && tile.y > 0);
    assert_eq!(tile.z, 16);
    let bounds = coord::tile_bounds(&tile);
    assert!(bounds.south < 43.66 && bounds.north > 43.64);
}

#[test]
fn test_tiles_for_bbox() {
    let bbox = GeoBBox::from_center(
        &GeoPoint { lat: 43.6532, lon: -79.3832, alt: 0.0 },
        500.0,
    );
    let tiles = coord::tiles_for_bbox(&bbox, 16);
    assert!(tiles.len() >= 4 && tiles.len() <= 25, "500m radius should need 4-25 tiles, got {}", tiles.len());
}

#[test]
fn test_geo_bbox_from_center() {
    let center = GeoPoint { lat: 43.0, lon: -79.0, alt: 0.0 };
    let bbox = GeoBBox::from_center(&center, 1000.0);
    assert!(bbox.south < 43.0 && bbox.north > 43.0);
    assert!(bbox.west < -79.0 && bbox.east > -79.0);
}

#[test]
fn test_hgt_parse() {
    // Create minimal 3x3 HGT data (big-endian i16)
    let mut data = Vec::new();
    for h in [100i16, 110, 120, 105, 115, 125, 110, 120, 130] {
        data.extend_from_slice(&h.to_be_bytes());
    }
    let grid = ruview_geo::terrain::parse_hgt(&data, 43.0, -79.0).unwrap();
    assert_eq!(grid.heights[0], 100.0);
    assert_eq!(grid.heights[4], 115.0);
}

#[test]
fn test_registration() {
    let origin = GeoPoint { lat: 43.6532, lon: -79.3832, alt: 76.0 };
    let reg = ruview_geo::register::auto_register(&origin);
    
    let local = [10.0f32, 0.0, 20.0]; // 10m east, 20m forward
    let geo = ruview_geo::register::local_to_wgs84(&reg, &local);
    assert!((geo.lat - origin.lat).abs() < 0.001);
    assert!((geo.lon - origin.lon).abs() < 0.001);
    
    let back = ruview_geo::register::wgs84_to_local(&reg, &geo);
    assert!((back[0] - local[0]).abs() < 0.1);
    assert!((back[2] - local[2]).abs() < 0.1);
}
