# ADR-044: Geospatial Satellite Integration

## Status
Accepted

## Context
RuView generates real-time 3D point clouds from camera + WiFi CSI, but these exist in a local coordinate frame with no geographic reference. Integrating free satellite imagery, terrain elevation, and map data provides environmental context that enables the ruOS brain to reason about the physical world beyond the room.

## Decision

### Data Sources (all free, no API keys)
| Source | Data | Resolution | Update | Format |
|--------|------|-----------|--------|--------|
| EOX Sentinel-2 Cloudless | Satellite tiles | 10m | Static mosaic | XYZ/JPEG |
| SRTM GL1 (NASA) | Elevation/DEM | 30m (1-arcsec) | Static | Binary HGT |
| Overpass API (OSM) | Buildings, roads | Vector | Real-time | JSON |
| ip-api.com | IP geolocation | ~1km | Per-request | JSON |
| Sentinel-2 STAC | Temporal satellite | 10m | Every 5 days | COG/STAC |
| Open Meteo | Weather | Point | Hourly | JSON |

### Architecture
Pure Rust implementation in `wifi-densepose-geo` crate. No GDAL/PROJ/GEOS — coordinate transforms implemented directly (~250 LOC). Tile caching on disk at `~/.local/share/ruview/geo-cache/`.

### Coordinate System
- WGS84 for geographic coordinates
- ENU (East-North-Up) as the bridge between local sensor frame and world
- Local sensor frame: camera origin, +Z forward, +Y up

### Temporal Awareness
Nightly scheduled fetch of Sentinel-2 latest imagery + OSM diffs + weather.
Changes detected via image comparison and stored as brain memories for
contrastive learning.

### Brain Integration
Geospatial context stored as brain memories:
- `spatial-geo`: location, elevation, nearby landmarks
- `spatial-change`: detected changes in satellite/OSM data
- `spatial-weather`: current conditions + forecast
- `spatial-season`: vegetation index, snow cover, seasonal patterns

## Consequences
### Positive
- Agent gains environmental awareness beyond the room
- Temporal data enables seasonal calibration of CSI sensing
- Change detection finds construction, vegetation, weather effects
- All data sources are genuinely free with no API keys

### Negative
- Initial data fetch requires internet (~2MB tiles + ~25MB DEM)
- Cached data becomes stale (mitigated by nightly refresh)
- IP geolocation has ~1km accuracy (mitigated by manual override)
