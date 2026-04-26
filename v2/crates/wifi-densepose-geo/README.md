# wifi-densepose-geo — Geospatial Satellite Integration

Free satellite imagery, terrain elevation, and map data for RuView spatial sensing. No API keys required.

## What It Does

Integrates your local sensor data (camera + WiFi CSI point cloud) with geographic context:

- **Satellite tiles** — 10m Sentinel-2 cloudless imagery for your location
- **Elevation** — SRTM 30m DEM for terrain modeling
- **Buildings + roads** — OpenStreetMap data via Overpass API
- **Weather** — Open Meteo current conditions + forecast
- **Geo-registration** — maps local sensor coordinates to WGS84
- **Temporal tracking** — detects changes over time (construction, vegetation, weather)
- **Brain integration** — stores geospatial context as ruOS brain memories

## Data Sources (all free, no API keys)

| Source | Data | Resolution | License |
|--------|------|-----------|---------|
| [EOX S2 Cloudless](https://s2maps.eu/) | Satellite tiles | 10m | CC-BY-4.0 |
| [SRTM GL1](https://portal.opentopography.org/) | Elevation/DEM | 30m | Public domain |
| [Overpass API](https://overpass-api.de/) | OSM buildings/roads | Vector | ODbL |
| [ip-api.com](http://ip-api.com/) | IP geolocation | ~1km | Free |
| [Open Meteo](https://open-meteo.com/) | Weather | Point | CC-BY-4.0 |

## Modules

| Module | LOC | Purpose |
|--------|-----|---------|
| `types.rs` | 140 | GeoPoint, GeoBBox, TileCoord, ElevationGrid, OsmFeature |
| `coord.rs` | 80 | WGS84/ENU transforms, tile math, haversine distance |
| `locate.rs` | 45 | IP geolocation with caching |
| `cache.rs` | 55 | Disk cache (`~/.local/share/ruview/geo-cache/`) |
| `tiles.rs` | 80 | Sentinel-2/ESRI/OSM tile fetcher |
| `terrain.rs` | 100 | SRTM HGT parser, elevation lookup |
| `osm.rs` | 150 | Overpass API client, building/road extraction |
| `register.rs` | 50 | Local-to-WGS84 coordinate registration |
| `fuse.rs` | 70 | Multi-source scene builder + summary |
| `brain.rs` | 30 | Store geo context in ruOS brain |
| `temporal.rs` | 100 | Weather, OSM change detection |

## Usage

```rust
use wifi_densepose_geo::{fuse, brain, temporal};

// Build geo scene for current location
let scene = fuse::build_scene(500.0).await?; // 500m radius
println!("{}", fuse::summarize(&scene));
// "Location: 43.6532N, 79.3832W, elevation 76m ASL.
//  23 buildings within view. 8 roads nearby (King St, Queen St).
//  12 satellite tiles at zoom 16."

// Store in brain
brain::store_geo_context(&scene).await?;

// Fetch weather
let weather = temporal::fetch_weather(&scene.location).await?;
// temperature: 12°C, partly cloudy, humidity 65%
```

## Brain Integration

Geospatial context is stored as brain memories:

| Category | Content | Frequency |
|----------|---------|-----------|
| `spatial-geo` | Location, elevation, buildings, roads | On startup + daily |
| `spatial-weather` | Temperature, conditions, humidity, wind | Nightly |
| `spatial-change` | New/removed buildings, road changes | Nightly diff |

The ruOS agent can search: "what buildings are near me?" or "what's the weather?" and get geospatial context from the brain.

## Security

- No API keys stored or transmitted
- IP geolocation uses HTTP (not HTTPS) — location is approximate (~1km)
- All tile fetches use HTTPS except ip-api.com
- Path traversal protection in cache key sanitization
- No user data sent to external services
- All data cached locally after first fetch

## Architecture

```
IP Geolocation ──→ (lat, lon)
                      │
        ┌─────────────┼─────────────┐
        ▼             ▼             ▼
   Sentinel-2     SRTM DEM     Overpass API
   (tiles)       (elevation)   (buildings/roads)
        │             │             │
        └─────────────┼─────────────┘
                      ▼
               GeoScene (fused)
                      │
              ┌───────┴───────┐
              ▼               ▼
        Brain Memory    Three.js Viewer
```

## License

MIT (same as RuView)
