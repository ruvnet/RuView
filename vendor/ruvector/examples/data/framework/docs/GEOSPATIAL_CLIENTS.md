# Geospatial & Mapping API Clients

Comprehensive Rust client module for geospatial and mapping APIs, integrated with RuVector's semantic vector framework.

## Overview

This module provides async clients for four major geospatial data sources:

1. **NominatimClient** - OpenStreetMap geocoding and reverse geocoding
2. **OverpassClient** - OSM data queries using Overpass QL
3. **GeonamesClient** - Worldwide place name database
4. **OpenElevationClient** - Elevation data lookup

All clients convert API responses to `SemanticVector` format for RuVector discovery and analysis.

## Features

- ✅ **Async/await** with Tokio runtime
- ✅ **Strict rate limiting** (especially Nominatim 1 req/sec)
- ✅ **User-Agent headers** for OSM services (required by policy)
- ✅ **SemanticVector integration** with geographic metadata
- ✅ **Comprehensive tests** with mock responses
- ✅ **GeoJSON handling** where applicable
- ✅ **Retry logic** with exponential backoff
- ✅ **GeoUtils integration** for distance calculations

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
ruvector-data-framework = "0.1.0"
tokio = { version = "1.0", features = ["full"] }
```

## Usage

### 1. NominatimClient (OpenStreetMap Geocoding)

**Rate Limit**: 1 request/second (STRICTLY ENFORCED)

```rust
use ruvector_data_framework::NominatimClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = NominatimClient::new()?;

    // Geocode: Address → Coordinates
    let results = client.geocode("1600 Pennsylvania Avenue, Washington DC").await?;
    for result in results {
        println!("Lat: {}, Lon: {}",
            result.metadata.get("latitude").unwrap(),
            result.metadata.get("longitude").unwrap()
        );
    }

    // Reverse geocode: Coordinates → Address
    let results = client.reverse_geocode(48.8584, 2.2945).await?;
    for result in results {
        println!("Address: {}", result.metadata.get("display_name").unwrap());
    }

    // Search places
    let results = client.search("Eiffel Tower", 5).await?;
    println!("Found {} places", results.len());

    Ok(())
}
```

**Metadata Fields**:
- `place_id`, `osm_type`, `osm_id`
- `latitude`, `longitude`
- `display_name`, `place_type`
- `importance`
- `city`, `country`, `country_code` (if available)

### 2. OverpassClient (OSM Data Queries)

**Rate Limit**: ~2 requests/second (conservative)

```rust
use ruvector_data_framework::OverpassClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OverpassClient::new()?;

    // Find nearby POIs
    let cafes = client.get_nearby_pois(
        48.8584,     // Eiffel Tower lat
        2.2945,      // Eiffel Tower lon
        500.0,       // 500 meters
        "cafe"       // amenity type
    ).await?;

    println!("Found {} cafes nearby", cafes.len());

    // Get road network in bounding box
    let roads = client.get_roads(
        48.85, 2.29,  // south, west
        48.86, 2.30   // north, east
    ).await?;

    println!("Found {} road segments", roads.len());

    // Custom Overpass QL query
    let query = r#"
        [out:json];
        node["amenity"="restaurant"](around:1000,40.7128,-74.0060);
        out;
    "#;
    let results = client.query(query).await?;

    Ok(())
}
```

**Metadata Fields**:
- `osm_id`, `osm_type`
- `latitude`, `longitude`
- `name`, `amenity`, `highway`
- `osm_tag_*` (all OSM tags preserved)

**Common Amenity Types**:
- `restaurant`, `cafe`, `bar`, `pub`
- `hospital`, `pharmacy`, `school`
- `bank`, `atm`, `post_office`
- `park`, `parking`, `fuel`

### 3. GeonamesClient (Place Name Database)

**Rate Limit**: ~0.5 requests/second (free tier: 2000/hour)
**Authentication**: Requires username from [geonames.org](http://www.geonames.org/login)

```rust
use ruvector_data_framework::GeonamesClient;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let username = env::var("GEONAMES_USERNAME")?;
    let client = GeonamesClient::new(username)?;

    // Search places by name
    let results = client.search("Paris", 10).await?;
    for result in results {
        println!("{} ({}, pop: {})",
            result.metadata.get("name").unwrap(),
            result.metadata.get("country_name").unwrap(),
            result.metadata.get("population").unwrap()
        );
    }

    // Get nearby places
    let nearby = client.get_nearby(48.8566, 2.3522).await?;
    println!("Found {} nearby places", nearby.len());

    // Get timezone
    let tz = client.get_timezone(40.7128, -74.0060).await?;
    if let Some(result) = tz.first() {
        println!("Timezone: {}", result.metadata.get("timezone_id").unwrap());
    }

    // Get country information
    let info = client.get_country_info("US").await?;
    if let Some(result) = info.first() {
        println!("Capital: {}", result.metadata.get("capital").unwrap());
        println!("Population: {}", result.metadata.get("population").unwrap());
    }

    Ok(())
}
```

**Metadata Fields**:
- `geoname_id`, `name`, `toponym_name`
- `latitude`, `longitude`
- `country_code`, `country_name`
- `admin_name1` (state/province)
- `feature_class`, `feature_code`
- `population`

**Country Info Fields**:
- `capital`, `population`, `area_sq_km`, `continent`

### 4. OpenElevationClient (Elevation Data)

**Rate Limit**: ~5 requests/second
**Authentication**: None required

```rust
use ruvector_data_framework::OpenElevationClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = OpenElevationClient::new()?;

    // Single point elevation
    let result = client.get_elevation(27.9881, 86.9250).await?; // Mt. Everest
    if let Some(point) = result.first() {
        println!("Elevation: {} meters", point.metadata.get("elevation_m").unwrap());
    }

    // Batch elevation lookup
    let locations = vec![
        (40.7128, -74.0060),  // NYC
        (48.8566, 2.3522),    // Paris
        (35.6762, 139.6503),  // Tokyo
    ];

    let results = client.get_elevations(locations).await?;
    for result in results {
        println!("Lat: {}, Lon: {}, Elevation: {} m",
            result.metadata.get("latitude").unwrap(),
            result.metadata.get("longitude").unwrap(),
            result.metadata.get("elevation_m").unwrap()
        );
    }

    Ok(())
}
```

**Metadata Fields**:
- `latitude`, `longitude`
- `elevation_m` (meters above sea level)

## Geographic Utilities

All clients use `GeoUtils` for distance calculations:

```rust
use ruvector_data_framework::GeoUtils;

// Calculate distance between two points (Haversine formula)
let distance_km = GeoUtils::distance_km(
    40.7128, -74.0060,  // NYC
    51.5074, -0.1278    // London
);
println!("NYC to London: {:.2} km", distance_km); // ~5570 km

// Check if point is within radius
let within = GeoUtils::within_radius(
    48.8566, 2.3522,   // Paris center
    48.8584, 2.2945,   // Eiffel Tower
    10.0               // 10 km radius
);
println!("Eiffel Tower within 10km of Paris: {}", within); // true
```

## Rate Limiting

All clients implement strict rate limiting to respect API policies:

| Client | Rate Limit | Enforcement |
|--------|------------|-------------|
| NominatimClient | 1 req/sec | **STRICT** (Mutex-based timing) |
| OverpassClient | ~2 req/sec | Conservative delay |
| GeonamesClient | ~0.5 req/sec | Conservative (2000/hour limit) |
| OpenElevationClient | ~5 req/sec | Light delay |

### Nominatim Rate Limiting

Nominatim uses a **strict rate limiter** that ensures exactly 1 request per second:

```rust
// Internal rate limiter tracks last request time
// Automatically waits if needed before each request
client.geocode("Paris").await?;  // Executes immediately
client.geocode("London").await?; // Waits ~1 second if needed
```

**IMPORTANT**: Violating Nominatim's 1 req/sec policy can result in IP blocking. The client enforces this automatically.

## SemanticVector Integration

All responses are converted to `SemanticVector` format:

```rust
pub struct SemanticVector {
    pub id: String,                    // "NOMINATIM:way:12345"
    pub embedding: Vec<f32>,           // 256-dim semantic embedding
    pub domain: Domain,                // Domain::CrossDomain
    pub timestamp: DateTime<Utc>,      // When data was fetched
    pub metadata: HashMap<String, String>, // Geographic metadata
}
```

This allows geospatial data to be:
- Stored in RuVector's vector database
- Searched semantically
- Combined with other domains (climate, finance, etc.)
- Analyzed for cross-domain patterns

## Error Handling

All clients use the framework's `Result` type:

```rust
use ruvector_data_framework::{NominatimClient, FrameworkError, Result};

async fn example() -> Result<()> {
    let client = NominatimClient::new()?;

    match client.geocode("Invalid Address").await {
        Ok(results) => {
            println!("Found {} results", results.len());
        }
        Err(FrameworkError::Network(e)) => {
            eprintln!("Network error: {}", e);
        }
        Err(e) => {
            eprintln!("Other error: {}", e);
        }
    }

    Ok(())
}
```

## Testing

Run the test suite:

```bash
# Run all geospatial tests
cargo test geospatial

# Run specific client tests
cargo test nominatim
cargo test overpass
cargo test geonames
cargo test elevation

# Run integration tests with mocked responses
cargo test --test geospatial_integration
```

Run the demo:

```bash
# Basic demo (skips GeoNames without username)
cargo run --example geospatial_demo

# Full demo with GeoNames
GEONAMES_USERNAME=your_username cargo run --example geospatial_demo
```

## Best Practices

### 1. Respect Rate Limits

```rust
// ✅ Good: Use the client's built-in rate limiting
for address in addresses {
    let results = client.geocode(address).await?;
    // Rate limiting is automatic
}

// ❌ Bad: Don't try to bypass rate limiting
for address in addresses {
    tokio::spawn(async move {
        client.geocode(address).await // Violates rate limits!
    });
}
```

### 2. Cache Results

```rust
use std::collections::HashMap;

struct GeocodingCache {
    cache: HashMap<String, Vec<SemanticVector>>,
    client: NominatimClient,
}

impl GeocodingCache {
    async fn geocode(&mut self, address: &str) -> Result<Vec<SemanticVector>> {
        if let Some(cached) = self.cache.get(address) {
            return Ok(cached.clone());
        }

        let results = self.client.geocode(address).await?;
        self.cache.insert(address.to_string(), results.clone());
        Ok(results)
    }
}
```

### 3. Handle Errors Gracefully

```rust
async fn batch_geocode(client: &NominatimClient, addresses: Vec<&str>) -> Vec<Option<SemanticVector>> {
    let mut results = Vec::new();

    for address in addresses {
        match client.geocode(address).await {
            Ok(mut vecs) => results.push(vecs.pop()),
            Err(e) => {
                tracing::warn!("Geocoding failed for '{}': {}", address, e);
                results.push(None);
            }
        }
    }

    results
}
```

### 4. Use Appropriate Clients

```rust
// ✅ Use Nominatim for address lookup
client.geocode("1600 Pennsylvania Avenue NW").await?;

// ✅ Use Overpass for POI search
client.get_nearby_pois(lat, lon, radius, "restaurant").await?;

// ✅ Use GeoNames for place name search
client.search("Paris").await?;

// ✅ Use OpenElevation for terrain analysis
client.get_elevations(hiking_trail_points).await?;
```

## Advanced Usage

### Cross-Domain Discovery

Combine geospatial data with other domains:

```rust
use ruvector_data_framework::{
    NominatimClient, UsgsEarthquakeClient,
    NativeDiscoveryEngine, NativeEngineConfig,
};

async fn earthquake_location_analysis() -> Result<()> {
    let geo_client = NominatimClient::new()?;
    let usgs_client = UsgsEarthquakeClient::new()?;

    // Get recent earthquakes
    let earthquakes = usgs_client.get_recent(4.0, 7).await?;

    // Create discovery engine
    let config = NativeEngineConfig::default();
    let mut engine = NativeDiscoveryEngine::new(config);

    // Add earthquake data
    for eq in earthquakes {
        engine.add_vector(eq);
    }

    // Add nearby cities for each earthquake
    for eq in &earthquakes {
        let lat: f64 = eq.metadata.get("latitude").unwrap().parse()?;
        let lon: f64 = eq.metadata.get("longitude").unwrap().parse()?;

        let nearby = geo_client.reverse_geocode(lat, lon).await?;
        for place in nearby {
            engine.add_vector(place);
        }
    }

    // Detect cross-domain patterns
    let patterns = engine.detect_patterns();
    println!("Found {} patterns linking earthquakes to locations", patterns.len());

    Ok(())
}
```

### Geofencing

```rust
use ruvector_data_framework::GeoUtils;

struct Geofence {
    center_lat: f64,
    center_lon: f64,
    radius_km: f64,
}

impl Geofence {
    fn contains(&self, lat: f64, lon: f64) -> bool {
        GeoUtils::within_radius(
            self.center_lat,
            self.center_lon,
            lat,
            lon,
            self.radius_km
        )
    }

    async fn find_pois(&self, client: &OverpassClient, amenity: &str) -> Result<Vec<SemanticVector>> {
        client.get_nearby_pois(
            self.center_lat,
            self.center_lon,
            self.radius_km * 1000.0, // Convert km to meters
            amenity
        ).await
    }
}

// Usage
let downtown = Geofence {
    center_lat: 40.7589,
    center_lon: -73.9851,
    radius_km: 2.0,
};

if downtown.contains(40.7614, -73.9776) {
    println!("Point is within downtown area");
}

let restaurants = downtown.find_pois(&overpass_client, "restaurant").await?;
```

## API Reference

See the [source code](../src/geospatial_clients.rs) for complete API documentation.

## Contributing

When contributing geospatial client improvements:

1. Maintain strict rate limiting compliance
2. Add comprehensive tests with mocked responses
3. Update this documentation
4. Follow the existing client patterns
5. Test with real APIs (but don't commit credentials)

## License

MIT License - See [LICENSE](../../../LICENSE) for details

## Resources

- [Nominatim Usage Policy](https://operations.osmfoundation.org/policies/nominatim/)
- [Overpass API Documentation](https://wiki.openstreetmap.org/wiki/Overpass_API)
- [GeoNames Web Services](http://www.geonames.org/export/web-services.html)
- [Open Elevation API](https://open-elevation.com/)
- [OpenStreetMap Tagging](https://wiki.openstreetmap.org/wiki/Map_features)
