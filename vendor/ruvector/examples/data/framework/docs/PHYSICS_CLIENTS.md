# Physics, Seismic, and Ocean Data Clients

## Overview

This module provides async API clients for physics, seismic, and ocean data sources, enabling cross-disciplinary discoveries through RuVector's semantic vector search and graph coherence analysis.

## New Domains

Three new domains have been added to `Domain` enum in `ruvector_native.rs`:

- **`Domain::Physics`** - Particle physics, materials science
- **`Domain::Seismic`** - Earthquake data, seismic activity
- **`Domain::Ocean`** - Ocean temperature, salinity, depth profiles

## Clients

### 1. UsgsEarthquakeClient

**USGS Earthquake Hazards Program** - Real-time and historical earthquake data worldwide.

#### Features
- No API key required (public data)
- Global earthquake coverage
- Magnitude, location, depth, tsunami warnings
- ~5 requests/second rate limit

#### Methods

```rust
use ruvector_data_framework::UsgsEarthquakeClient;

let client = UsgsEarthquakeClient::new()?;

// Get recent earthquakes above minimum magnitude
let recent = client.get_recent(4.5, 7).await?; // Mag 4.5+, last 7 days

// Search by geographic region
let la_quakes = client.search_by_region(
    34.05,    // latitude
    -118.25,  // longitude
    200.0,    // radius in km
    30        // days back
).await?;

// Get significant earthquakes only
let significant = client.get_significant(30).await?;

// Filter by magnitude range
let moderate = client.get_by_magnitude_range(4.0, 6.0, 7).await?;
```

#### SemanticVector Metadata

Each earthquake is converted to a `SemanticVector` with:

```rust
metadata: {
    "magnitude": "5.4",
    "place": "Southern California",
    "latitude": "34.05",
    "longitude": "-118.25",
    "depth_km": "10.5",
    "tsunami": "0",
    "significance": "450",
    "status": "reviewed",
    "alert": "green",
    "source": "usgs"
}
```

### 2. CernOpenDataClient

**CERN Open Data Portal** - LHC experiment data, particle physics datasets.

#### Features
- No API key required
- CMS, ATLAS, LHCb, ALICE experiments
- Collision events, particle physics data
- Educational and research datasets

#### Methods

```rust
use ruvector_data_framework::CernOpenDataClient;

let client = CernOpenDataClient::new()?;

// Search datasets by keywords
let higgs = client.search_datasets("Higgs").await?;
let top_quark = client.search_datasets("top quark").await?;

// Get specific dataset by record ID
let dataset = client.get_dataset(5500).await?;

// Search by experiment
let cms_data = client.search_by_experiment("CMS").await?;
let atlas_data = client.search_by_experiment("ATLAS").await?;
```

#### Available Experiments
- `"CMS"` - Compact Muon Solenoid
- `"ATLAS"` - A Toroidal LHC ApparatuS
- `"LHCb"` - Large Hadron Collider beauty
- `"ALICE"` - A Large Ion Collider Experiment

#### SemanticVector Metadata

```rust
metadata: {
    "recid": "12345",
    "title": "CMS 2011 Higgs to two photons dataset",
    "experiment": "CMS",
    "collision_energy": "7TeV",
    "collision_type": "pp",
    "data_type": "Dataset",
    "source": "cern"
}
```

### 3. ArgoClient

**Argo Float Ocean Data** - Global ocean temperature, salinity, pressure profiles.

#### Features
- Global ocean coverage (4000+ floats)
- Temperature and salinity profiles
- Depth measurements (0-2000m typical)
- Free public data

#### Methods

```rust
use ruvector_data_framework::ArgoClient;

let client = ArgoClient::new()?;

// Get recent profiles (placeholder - requires Argo GDAC integration)
let recent = client.get_recent_profiles(30).await?;

// Search by region
let atlantic = client.search_by_region(
    0.0,     // latitude
    -30.0,   // longitude
    500.0    // radius km
).await?;

// Temperature-focused profiles
let temp_data = client.get_temperature_profiles().await?;

// Create sample data for testing
let samples = client.create_sample_profiles(50)?;
```

#### Note on Implementation

The current Argo client includes a `create_sample_profiles()` method for demonstration. For production use, integrate with:

- **Argo GDAC** (Global Data Assembly Center): https://data-argo.ifremer.fr
- **ArgoVis API**: https://argovis-api.colorado.edu
- Direct netCDF file parsing

#### SemanticVector Metadata

```rust
metadata: {
    "platform_number": "1900001",
    "latitude": "35.5",
    "longitude": "-45.2",
    "temperature": "18.3",
    "salinity": "35.1",
    "depth_m": "500.0",
    "source": "argo"
}
```

### 4. MaterialsProjectClient

**Materials Project** - Computational materials science database (150,000+ materials).

#### Features
- Crystal structures and properties
- Band gaps, formation energies
- Electronic and mechanical properties
- **Requires free API key** from https://materialsproject.org

#### Methods

```rust
use ruvector_data_framework::MaterialsProjectClient;

// API key required
let api_key = std::env::var("MATERIALS_PROJECT_API_KEY")?;
let client = MaterialsProjectClient::new(api_key)?;

// Search by chemical formula
let silicon = client.search_materials("Si").await?;
let iron_oxide = client.search_materials("Fe2O3").await?;
let battery = client.search_materials("LiFePO4").await?;

// Get specific material by ID
let mp_149 = client.get_material("mp-149").await?; // Silicon

// Search by property range
let semiconductors = client.search_by_property(
    "band_gap",
    1.0,  // min eV
    3.0   // max eV
).await?;

let stable = client.search_by_property(
    "formation_energy_per_atom",
    -2.0,  // min eV/atom
    0.0    // max eV/atom
).await?;
```

#### Common Properties

- `"band_gap"` - Electronic band gap (eV)
- `"formation_energy_per_atom"` - Formation energy (eV/atom)
- `"energy_per_atom"` - Total energy per atom
- `"density"` - Density (g/cm³)
- `"volume"` - Volume per atom

#### SemanticVector Metadata

```rust
metadata: {
    "material_id": "mp-149",
    "formula": "Si",
    "band_gap": "1.14",
    "density": "2.33",
    "formation_energy": "0.0",
    "crystal_system": "cubic",
    "elements": "Si",
    "source": "materials_project"
}
```

## Geographic Utilities

The `GeoUtils` helper provides geographic calculations:

```rust
use ruvector_data_framework::GeoUtils;

// Calculate distance between two points (Haversine formula)
let distance_km = GeoUtils::distance_km(
    40.7128, -74.0060,  // NYC
    34.0522, -118.2437  // LA
);
// Returns: ~3936 km

// Check if point is within radius
let within = GeoUtils::within_radius(
    34.05, -118.25,     // Center (LA)
    32.72, -117.16,     // Point (San Diego)
    200.0               // Radius in km
);
// Returns: true
```

## Rate Limiting

All clients implement automatic rate limiting and retry logic:

| Client | Rate Limit | Max Retries | Retry Delay |
|--------|------------|-------------|-------------|
| USGS | 200ms (~5 req/s) | 3 | 1s exponential |
| CERN | 500ms (~2 req/s) | 3 | 1s exponential |
| Argo | 300ms (~3 req/s) | 3 | 1s exponential |
| Materials Project | 1000ms (1 req/s) | 3 | 1s exponential |

## Cross-Domain Discovery Examples

### 1. Earthquake-Climate Correlations

```rust
use ruvector_data_framework::{
    UsgsEarthquakeClient, NoaaClient,
    NativeDiscoveryEngine, NativeEngineConfig
};

let mut engine = NativeDiscoveryEngine::new(NativeEngineConfig::default());

// Add earthquake data
let usgs = UsgsEarthquakeClient::new()?;
let earthquakes = usgs.get_recent(5.0, 30).await?;
for eq in earthquakes {
    engine.add_vector(eq);
}

// Add climate data
let noaa = NoaaClient::new(None)?;
let climate = noaa.get_climate_data("GHCND:USW00023174", 30).await?;
for record in climate {
    engine.add_vector(record);
}

// Discover patterns
let patterns = engine.detect_patterns();
for pattern in patterns {
    if !pattern.cross_domain_links.is_empty() {
        println!("Found cross-domain pattern: {}", pattern.description);
    }
}
```

### 2. Materials for Particle Detectors

```rust
use ruvector_data_framework::{
    CernOpenDataClient, MaterialsProjectClient
};

let cern = CernOpenDataClient::new()?;
let materials = MaterialsProjectClient::new(api_key)?;

// Get particle physics requirements
let detector_data = cern.search_datasets("detector").await?;

// Find materials with suitable properties
let semiconductors = materials.search_by_property("band_gap", 1.0, 3.0).await?;

// Add to discovery engine to find correlations
let mut engine = NativeDiscoveryEngine::new(config);
for data in detector_data {
    engine.add_vector(data);
}
for material in semiconductors {
    engine.add_vector(material);
}

let patterns = engine.detect_patterns();
```

### 3. Ocean Temperature & Seismic Activity

```rust
use ruvector_data_framework::{
    ArgoClient, UsgsEarthquakeClient
};

let argo = ArgoClient::new()?;
let usgs = UsgsEarthquakeClient::new()?;

// Get ocean data for a region
let ocean = argo.search_by_region(0.0, -30.0, 1000.0).await?;

// Get earthquakes in same region
let quakes = usgs.search_by_region(0.0, -30.0, 1000.0, 90).await?;

// Discover correlations
let mut engine = NativeDiscoveryEngine::new(config);
for profile in ocean {
    engine.add_vector(profile);
}
for eq in quakes {
    engine.add_vector(eq);
}

// Look for cross-domain patterns
let patterns = engine.detect_patterns();
for pattern in patterns.iter().filter(|p| {
    p.cross_domain_links.iter().any(|l|
        (l.source_domain == Domain::Ocean && l.target_domain == Domain::Seismic) ||
        (l.source_domain == Domain::Seismic && l.target_domain == Domain::Ocean)
    )
}) {
    println!("Ocean-Seismic correlation: {}", pattern.description);
}
```

## Running the Example

```bash
# Basic example (no API keys required)
cargo run --example physics_discovery

# With Materials Project API key
export MATERIALS_PROJECT_API_KEY="your_key_here"
cargo run --example physics_discovery
```

## Integration with RuVector

All clients convert data to `SemanticVector` format, enabling:

1. **Vector Similarity Search** - Find similar earthquakes, materials, experiments
2. **Graph Coherence Analysis** - Detect network fragmentation/consolidation
3. **Cross-Domain Pattern Discovery** - Bridge physics, seismic, ocean domains
4. **Temporal Analysis** - Track changes over time
5. **Spatial Analysis** - Geographic clustering and correlation

## Testing

```bash
# Run all physics client tests
cargo test physics_clients

# Run specific client tests
cargo test usgs_client
cargo test cern_client
cargo test argo_client
cargo test materials_project_client

# Run geographic utilities tests
cargo test geo_utils
```

## API Documentation

### USGS Earthquake API
- Docs: https://earthquake.usgs.gov/fdsnws/event/1/
- No registration required
- Global coverage
- Real-time updates

### CERN Open Data Portal
- Portal: https://opendata.cern.ch
- API: https://opendata.cern.ch/docs/api
- No registration required
- Datasets from LHC experiments

### Argo Data
- GDAC: https://data-argo.ifremer.fr
- ArgoVis: https://argovis.colorado.edu
- Free public access
- NetCDF and JSON formats

### Materials Project
- Website: https://materialsproject.org
- API Docs: https://materialsproject.org/api
- **Free API key required** (easy registration)
- 150,000+ computed materials

## Future Enhancements

1. **Full Argo GDAC Integration** - Parse netCDF files directly
2. **CERN Data Caching** - Local cache for large datasets
3. **USGS Historical Data** - Access to complete historical catalog
4. **Materials Project Batch Queries** - Optimize multi-material searches
5. **Real-time Earthquake Streaming** - WebSocket for live data
6. **Ocean Current Prediction** - ML models for temperature forecasting

## License

Part of RuVector Data Discovery Framework. See main LICENSE file.
