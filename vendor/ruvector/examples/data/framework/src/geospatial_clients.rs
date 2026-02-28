//! Geospatial & Mapping API integrations
//!
//! This module provides async clients for:
//! - Nominatim (OpenStreetMap geocoding)
//! - Overpass API (OSM data queries)
//! - GeoNames (place name database)
//! - Open Elevation (elevation data)
//!
//! All responses are converted to SemanticVector format for RuVector discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const NOMINATIM_RATE_LIMIT_MS: u64 = 1000; // STRICT: 1 request/second
const OVERPASS_RATE_LIMIT_MS: u64 = 500; // Conservative: 2 requests/second
const GEONAMES_RATE_LIMIT_MS: u64 = 2000; // Conservative for free tier: ~0.5/sec (2000/hour limit)
const OPEN_ELEVATION_RATE_LIMIT_MS: u64 = 200; // ~5 requests/second
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;

// User-Agent for OSM services (required by policy)
const USER_AGENT: &str = "RuVector-Data-Framework/1.0 (https://github.com/ruvnet/ruvector)";

// ============================================================================
// Nominatim Client (OpenStreetMap Geocoding)
// ============================================================================

/// Nominatim geocoding response
#[derive(Debug, Deserialize)]
struct NominatimPlace {
    #[serde(default)]
    place_id: u64,
    #[serde(default)]
    licence: String,
    #[serde(default)]
    osm_type: String,
    #[serde(default)]
    osm_id: u64,
    #[serde(default)]
    lat: String,
    #[serde(default)]
    lon: String,
    #[serde(default)]
    display_name: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    importance: f64,
    #[serde(default)]
    address: Option<NominatimAddress>,
    #[serde(default)]
    geojson: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Default)]
struct NominatimAddress {
    #[serde(default)]
    house_number: Option<String>,
    #[serde(default)]
    road: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    postcode: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
}

/// Client for Nominatim (OpenStreetMap Geocoding)
///
/// Provides access to:
/// - Address to coordinates (geocoding)
/// - Coordinates to address (reverse geocoding)
/// - Place name search
///
/// **IMPORTANT**: STRICT rate limit of 1 request/second is enforced.
/// See: https://operations.osmfoundation.org/policies/nominatim/
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::NominatimClient;
///
/// let client = NominatimClient::new()?;
/// let coords = client.geocode("1600 Pennsylvania Avenue, Washington DC").await?;
/// let address = client.reverse_geocode(38.8977, -77.0365).await?;
/// let places = client.search("Eiffel Tower", 5).await?;
/// ```
pub struct NominatimClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    /// Last request time for STRICT rate limiting
    last_request: Arc<Mutex<Option<Instant>>>,
}

impl NominatimClient {
    /// Create a new Nominatim client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(USER_AGENT)
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://nominatim.openstreetmap.org".to_string(),
            rate_limit_delay: Duration::from_millis(NOMINATIM_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
            last_request: Arc::new(Mutex::new(None)),
        })
    }

    /// Enforce STRICT rate limiting (1 request/second)
    async fn enforce_rate_limit(&self) {
        let mut last = self.last_request.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.rate_limit_delay {
                let wait_time = self.rate_limit_delay - elapsed;
                sleep(wait_time).await;
            }
        }

        *last = Some(Instant::now());
    }

    /// Geocode an address to coordinates
    ///
    /// # Arguments
    /// * `address` - Address string (e.g., "1600 Pennsylvania Avenue, Washington DC")
    ///
    /// # Example
    /// ```rust,ignore
    /// let coords = client.geocode("Eiffel Tower, Paris").await?;
    /// ```
    pub async fn geocode(&self, address: &str) -> Result<Vec<SemanticVector>> {
        self.enforce_rate_limit().await;

        let url = format!(
            "{}/search?q={}&format=json&addressdetails=1&limit=1",
            self.base_url,
            urlencoding::encode(address)
        );

        let response = self.fetch_with_retry(&url).await?;
        let places: Vec<NominatimPlace> = response.json().await?;

        self.convert_places(places)
    }

    /// Reverse geocode coordinates to address
    ///
    /// # Arguments
    /// * `lat` - Latitude
    /// * `lon` - Longitude
    ///
    /// # Example
    /// ```rust,ignore
    /// let address = client.reverse_geocode(48.8584, 2.2945).await?;
    /// ```
    pub async fn reverse_geocode(&self, lat: f64, lon: f64) -> Result<Vec<SemanticVector>> {
        self.enforce_rate_limit().await;

        let url = format!(
            "{}/reverse?lat={}&lon={}&format=json&addressdetails=1",
            self.base_url, lat, lon
        );

        let response = self.fetch_with_retry(&url).await?;
        let place: NominatimPlace = response.json().await?;

        self.convert_places(vec![place])
    }

    /// Search for places by name
    ///
    /// # Arguments
    /// * `query` - Search query (e.g., "Central Park")
    /// * `limit` - Maximum number of results (max 50)
    ///
    /// # Example
    /// ```rust,ignore
    /// let places = client.search("Times Square", 5).await?;
    /// ```
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        self.enforce_rate_limit().await;

        let limit = limit.min(50); // Nominatim max is 50
        let url = format!(
            "{}/search?q={}&format=json&addressdetails=1&limit={}",
            self.base_url,
            urlencoding::encode(query),
            limit
        );

        let response = self.fetch_with_retry(&url).await?;
        let places: Vec<NominatimPlace> = response.json().await?;

        self.convert_places(places)
    }

    /// Convert Nominatim places to SemanticVectors
    fn convert_places(&self, places: Vec<NominatimPlace>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for place in places {
            let lat = place.lat.parse::<f64>().unwrap_or(0.0);
            let lon = place.lon.parse::<f64>().unwrap_or(0.0);

            // Build address string
            let address_str = if let Some(addr) = &place.address {
                format!(
                    "{}, {}, {}, {}",
                    addr.road.as_deref().unwrap_or(""),
                    addr.city.as_deref().unwrap_or(""),
                    addr.state.as_deref().unwrap_or(""),
                    addr.country.as_deref().unwrap_or("")
                )
            } else {
                place.display_name.clone()
            };

            // Create text for embedding
            let text = format!(
                "{} at lat: {}, lon: {} - {} (OSM type: {})",
                place.display_name, lat, lon, address_str, place.osm_type
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("place_id".to_string(), place.place_id.to_string());
            metadata.insert("osm_type".to_string(), place.osm_type.clone());
            metadata.insert("osm_id".to_string(), place.osm_id.to_string());
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("display_name".to_string(), place.display_name.clone());
            metadata.insert("place_type".to_string(), place.r#type.clone());
            metadata.insert("importance".to_string(), place.importance.to_string());

            if let Some(addr) = &place.address {
                if let Some(city) = &addr.city {
                    metadata.insert("city".to_string(), city.clone());
                }
                if let Some(country) = &addr.country {
                    metadata.insert("country".to_string(), country.clone());
                }
                if let Some(country_code) = &addr.country_code {
                    metadata.insert("country_code".to_string(), country_code.clone());
                }
            }
            metadata.insert("source".to_string(), "nominatim".to_string());

            vectors.push(SemanticVector {
                id: format!("NOMINATIM:{}:{}", place.osm_type, place.osm_id),
                embedding,
                domain: Domain::CrossDomain, // Geographic data spans multiple domains
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for NominatimClient {
    fn default() -> Self {
        Self::new().expect("Failed to create Nominatim client")
    }
}

// ============================================================================
// Overpass API Client (OSM Data Queries)
// ============================================================================

/// Overpass API response element
#[derive(Debug, Deserialize)]
struct OverpassElement {
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    id: u64,
    #[serde(default)]
    lat: Option<f64>,
    #[serde(default)]
    lon: Option<f64>,
    #[serde(default)]
    tags: HashMap<String, String>,
    #[serde(default)]
    center: Option<OverpassCenter>,
}

#[derive(Debug, Deserialize)]
struct OverpassCenter {
    lat: f64,
    lon: f64,
}

/// Overpass API response
#[derive(Debug, Deserialize)]
struct OverpassResponse {
    #[serde(default)]
    elements: Vec<OverpassElement>,
}

/// Client for Overpass API (OSM Data Queries)
///
/// Provides access to:
/// - Custom Overpass QL queries
/// - Nearby POI (Points of Interest) search
/// - Road network extraction
/// - OSM tag-based queries
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::OverpassClient;
///
/// let client = OverpassClient::new()?;
/// let pois = client.get_nearby_pois(48.8584, 2.2945, 500.0, "restaurant").await?;
/// let roads = client.get_roads(48.85, 2.29, 48.86, 2.30).await?;
/// ```
pub struct OverpassClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl OverpassClient {
    /// Create a new Overpass API client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60)) // Overpass can be slow
            .user_agent(USER_AGENT)
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://overpass-api.de/api/interpreter".to_string(),
            rate_limit_delay: Duration::from_millis(OVERPASS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Execute a custom Overpass QL query
    ///
    /// # Arguments
    /// * `query` - Overpass QL query string
    ///
    /// # Example
    /// ```rust,ignore
    /// let query = r#"
    ///     [out:json];
    ///     node["amenity"="cafe"](around:1000,48.8584,2.2945);
    ///     out;
    /// "#;
    /// let results = client.query(query).await?;
    /// ```
    pub async fn query(&self, query: &str) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let response = self.client
            .post(&self.base_url)
            .body(query.to_string())
            .send()
            .await?;

        let overpass_response: OverpassResponse = response.json().await?;
        self.convert_elements(overpass_response.elements)
    }

    /// Get nearby POIs (Points of Interest)
    ///
    /// # Arguments
    /// * `lat` - Center latitude
    /// * `lon` - Center longitude
    /// * `radius` - Search radius in meters
    /// * `amenity_type` - OSM amenity type (e.g., "restaurant", "cafe", "hospital")
    ///
    /// # Example
    /// ```rust,ignore
    /// let cafes = client.get_nearby_pois(48.8584, 2.2945, 1000.0, "cafe").await?;
    /// ```
    pub async fn get_nearby_pois(
        &self,
        lat: f64,
        lon: f64,
        radius: f64,
        amenity_type: &str,
    ) -> Result<Vec<SemanticVector>> {
        let query = format!(
            r#"[out:json];node["amenity"="{}"](around:{},{},{});out;"#,
            amenity_type, radius, lat, lon
        );

        self.query(&query).await
    }

    /// Get road network in a bounding box
    ///
    /// # Arguments
    /// * `south` - Southern latitude
    /// * `west` - Western longitude
    /// * `north` - Northern latitude
    /// * `east` - Eastern longitude
    ///
    /// # Example
    /// ```rust,ignore
    /// let roads = client.get_roads(48.85, 2.29, 48.86, 2.30).await?;
    /// ```
    pub async fn get_roads(
        &self,
        south: f64,
        west: f64,
        north: f64,
        east: f64,
    ) -> Result<Vec<SemanticVector>> {
        let query = format!(
            r#"[out:json];way["highway"]({},{},{},{});out geom;"#,
            south, west, north, east
        );

        self.query(&query).await
    }

    /// Convert Overpass elements to SemanticVectors
    fn convert_elements(&self, elements: Vec<OverpassElement>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for element in elements {
            // Get coordinates (from element or center)
            let (lat, lon) = if let (Some(lat), Some(lon)) = (element.lat, element.lon) {
                (lat, lon)
            } else if let Some(center) = element.center {
                (center.lat, center.lon)
            } else {
                continue; // Skip elements without coordinates
            };

            // Extract name and tags
            let name = element.tags.get("name").cloned().unwrap_or_else(|| {
                format!("OSM {} {}", element.r#type, element.id)
            });

            let amenity = element.tags.get("amenity").cloned().unwrap_or_default();
            let highway = element.tags.get("highway").cloned().unwrap_or_default();

            // Create text for embedding
            let text = format!(
                "{} at lat: {}, lon: {} - amenity: {}, highway: {}, tags: {:?}",
                name, lat, lon, amenity, highway, element.tags
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("osm_id".to_string(), element.id.to_string());
            metadata.insert("osm_type".to_string(), element.r#type.clone());
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("name".to_string(), name);

            if !amenity.is_empty() {
                metadata.insert("amenity".to_string(), amenity);
            }
            if !highway.is_empty() {
                metadata.insert("highway".to_string(), highway);
            }

            // Add all OSM tags
            for (key, value) in element.tags {
                metadata.insert(format!("osm_tag_{}", key), value);
            }
            metadata.insert("source".to_string(), "overpass".to_string());

            vectors.push(SemanticVector {
                id: format!("OVERPASS:{}:{}", element.r#type, element.id),
                embedding,
                domain: Domain::CrossDomain,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }
}

impl Default for OverpassClient {
    fn default() -> Self {
        Self::new().expect("Failed to create Overpass client")
    }
}

// ============================================================================
// GeoNames Client
// ============================================================================

/// GeoNames search result
#[derive(Debug, Deserialize)]
struct GeoNamesSearchResult {
    #[serde(default)]
    geonames: Vec<GeoName>,
}

#[derive(Debug, Deserialize)]
struct GeoName {
    #[serde(default)]
    geonameId: u64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    lat: String,
    #[serde(default)]
    lng: String,
    #[serde(default)]
    countryCode: String,
    #[serde(default)]
    countryName: String,
    #[serde(default)]
    fcl: String, // feature class
    #[serde(default)]
    fcode: String, // feature code
    #[serde(default)]
    population: u64,
    #[serde(default)]
    adminName1: String, // state/province
    #[serde(default)]
    toponymName: String,
}

/// GeoNames timezone result
#[derive(Debug, Deserialize)]
struct GeoNamesTimezone {
    #[serde(default)]
    timezoneId: String,
    #[serde(default)]
    countryCode: String,
    #[serde(default)]
    lat: f64,
    #[serde(default)]
    lng: f64,
}

/// GeoNames country info
#[derive(Debug, Deserialize)]
struct GeoNamesCountryInfo {
    #[serde(default)]
    geonames: Vec<GeoNamesCountry>,
}

#[derive(Debug, Deserialize)]
struct GeoNamesCountry {
    #[serde(default)]
    countryCode: String,
    #[serde(default)]
    countryName: String,
    #[serde(default)]
    capital: String,
    #[serde(default)]
    population: u64,
    #[serde(default)]
    areaInSqKm: String,
    #[serde(default)]
    continent: String,
}

/// Client for GeoNames
///
/// Provides access to:
/// - Place name search
/// - Nearby places lookup
/// - Timezone information
/// - Country details
///
/// **Note**: Requires username (set GEONAMES_USERNAME env var)
/// Free tier: 2000 requests/hour, 30000/day
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::GeonamesClient;
///
/// let client = GeonamesClient::new("your_username".to_string())?;
/// let places = client.search("Paris", 10).await?;
/// let nearby = client.get_nearby(48.8566, 2.3522).await?;
/// let tz = client.get_timezone(40.7128, -74.0060).await?;
/// ```
pub struct GeonamesClient {
    client: Client,
    base_url: String,
    username: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl GeonamesClient {
    /// Create a new GeoNames client
    ///
    /// # Arguments
    /// * `username` - GeoNames username (register at geonames.org)
    pub fn new(username: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "http://api.geonames.org".to_string(),
            username,
            rate_limit_delay: Duration::from_millis(GEONAMES_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Search for places by name
    ///
    /// # Arguments
    /// * `query` - Place name to search
    /// * `limit` - Maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.search("New York", 10).await?;
    /// ```
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let url = format!(
            "{}/searchJSON?q={}&maxRows={}&username={}",
            self.base_url,
            urlencoding::encode(query),
            limit,
            self.username
        );

        let response = self.fetch_with_retry(&url).await?;
        let result: GeoNamesSearchResult = response.json().await?;

        self.convert_geonames(result.geonames)
    }

    /// Get nearby places
    ///
    /// # Arguments
    /// * `lat` - Latitude
    /// * `lon` - Longitude
    ///
    /// # Example
    /// ```rust,ignore
    /// let nearby = client.get_nearby(40.7128, -74.0060).await?;
    /// ```
    pub async fn get_nearby(&self, lat: f64, lon: f64) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let url = format!(
            "{}/findNearbyJSON?lat={}&lng={}&username={}",
            self.base_url, lat, lon, self.username
        );

        let response = self.fetch_with_retry(&url).await?;
        let result: GeoNamesSearchResult = response.json().await?;

        self.convert_geonames(result.geonames)
    }

    /// Get timezone for coordinates
    ///
    /// # Arguments
    /// * `lat` - Latitude
    /// * `lon` - Longitude
    ///
    /// # Example
    /// ```rust,ignore
    /// let tz = client.get_timezone(51.5074, -0.1278).await?;
    /// ```
    pub async fn get_timezone(&self, lat: f64, lon: f64) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let url = format!(
            "{}/timezoneJSON?lat={}&lng={}&username={}",
            self.base_url, lat, lon, self.username
        );

        let response = self.fetch_with_retry(&url).await?;
        let tz: GeoNamesTimezone = response.json().await?;

        let text = format!(
            "Timezone {} for coordinates ({}, {}), country: {}",
            tz.timezoneId, lat, lon, tz.countryCode
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("timezone_id".to_string(), tz.timezoneId.clone());
        metadata.insert("country_code".to_string(), tz.countryCode);
        metadata.insert("latitude".to_string(), lat.to_string());
        metadata.insert("longitude".to_string(), lon.to_string());
        metadata.insert("source".to_string(), "geonames".to_string());

        Ok(vec![SemanticVector {
            id: format!("GEONAMES:TZ:{}", tz.timezoneId),
            embedding,
            domain: Domain::CrossDomain,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    /// Get country information
    ///
    /// # Arguments
    /// * `country_code` - ISO 2-letter country code (e.g., "US", "FR")
    ///
    /// # Example
    /// ```rust,ignore
    /// let info = client.get_country_info("US").await?;
    /// ```
    pub async fn get_country_info(&self, country_code: &str) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let url = format!(
            "{}/countryInfoJSON?country={}&username={}",
            self.base_url, country_code, self.username
        );

        let response = self.fetch_with_retry(&url).await?;
        let result: GeoNamesCountryInfo = response.json().await?;

        let mut vectors = Vec::new();
        for country in result.geonames {
            let text = format!(
                "{} ({}) - Capital: {}, Population: {}, Area: {} sq km, Continent: {}",
                country.countryName,
                country.countryCode,
                country.capital,
                country.population,
                country.areaInSqKm,
                country.continent
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("country_code".to_string(), country.countryCode.clone());
            metadata.insert("country_name".to_string(), country.countryName);
            metadata.insert("capital".to_string(), country.capital);
            metadata.insert("population".to_string(), country.population.to_string());
            metadata.insert("area_sq_km".to_string(), country.areaInSqKm);
            metadata.insert("continent".to_string(), country.continent);
            metadata.insert("source".to_string(), "geonames".to_string());

            vectors.push(SemanticVector {
                id: format!("GEONAMES:COUNTRY:{}", country.countryCode),
                embedding,
                domain: Domain::CrossDomain,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Convert GeoNames results to SemanticVectors
    fn convert_geonames(&self, geonames: Vec<GeoName>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for place in geonames {
            let lat = place.lat.parse::<f64>().unwrap_or(0.0);
            let lon = place.lng.parse::<f64>().unwrap_or(0.0);

            let text = format!(
                "{} ({}) in {}, {} - lat: {}, lon: {}, population: {}",
                place.name,
                place.toponymName,
                place.adminName1,
                place.countryName,
                lat,
                lon,
                place.population
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("geoname_id".to_string(), place.geonameId.to_string());
            metadata.insert("name".to_string(), place.name);
            metadata.insert("toponym_name".to_string(), place.toponymName);
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("country_code".to_string(), place.countryCode);
            metadata.insert("country_name".to_string(), place.countryName);
            metadata.insert("admin_name1".to_string(), place.adminName1);
            metadata.insert("feature_class".to_string(), place.fcl);
            metadata.insert("feature_code".to_string(), place.fcode);
            metadata.insert("population".to_string(), place.population.to_string());
            metadata.insert("source".to_string(), "geonames".to_string());

            vectors.push(SemanticVector {
                id: format!("GEONAMES:{}", place.geonameId),
                embedding,
                domain: Domain::CrossDomain,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

// ============================================================================
// Open Elevation Client
// ============================================================================

/// Open Elevation result
#[derive(Debug, Deserialize)]
struct OpenElevationResponse {
    #[serde(default)]
    results: Vec<ElevationPoint>,
}

#[derive(Debug, Deserialize, Serialize)]
struct ElevationPoint {
    latitude: f64,
    longitude: f64,
    elevation: f64,
}

/// Request for batch elevation lookup
#[derive(Debug, Serialize)]
struct ElevationRequest {
    locations: Vec<ElevationLocation>,
}

#[derive(Debug, Serialize)]
struct ElevationLocation {
    latitude: f64,
    longitude: f64,
}

/// Client for Open Elevation API
///
/// Provides access to:
/// - Single point elevation lookup
/// - Batch elevation lookups
/// - Worldwide coverage using SRTM data
///
/// No authentication required. Free and open service.
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::OpenElevationClient;
///
/// let client = OpenElevationClient::new()?;
/// let elevation = client.get_elevation(46.9480, 7.4474).await?; // Mt. Everest base
/// let elevations = client.get_elevations(vec![(40.7128, -74.0060), (48.8566, 2.3522)]).await?;
/// ```
pub struct OpenElevationClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl OpenElevationClient {
    /// Create a new Open Elevation client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.open-elevation.com/api/v1".to_string(),
            rate_limit_delay: Duration::from_millis(OPEN_ELEVATION_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get elevation for a single point
    ///
    /// # Arguments
    /// * `lat` - Latitude
    /// * `lon` - Longitude
    ///
    /// # Example
    /// ```rust,ignore
    /// let elevation = client.get_elevation(27.9881, 86.9250).await?; // Mt. Everest
    /// ```
    pub async fn get_elevation(&self, lat: f64, lon: f64) -> Result<Vec<SemanticVector>> {
        self.get_elevations(vec![(lat, lon)]).await
    }

    /// Get elevations for multiple points
    ///
    /// # Arguments
    /// * `locations` - Vec of (latitude, longitude) tuples
    ///
    /// # Example
    /// ```rust,ignore
    /// let elevations = client.get_elevations(vec![
    ///     (40.7128, -74.0060), // NYC
    ///     (48.8566, 2.3522),   // Paris
    /// ]).await?;
    /// ```
    pub async fn get_elevations(&self, locations: Vec<(f64, f64)>) -> Result<Vec<SemanticVector>> {
        sleep(self.rate_limit_delay).await;

        let request = ElevationRequest {
            locations: locations
                .iter()
                .map(|(lat, lon)| ElevationLocation {
                    latitude: *lat,
                    longitude: *lon,
                })
                .collect(),
        };

        let url = format!("{}/lookup", self.base_url);

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        let elevation_response: OpenElevationResponse = response.json().await?;
        self.convert_elevations(elevation_response.results)
    }

    /// Convert elevation points to SemanticVectors
    fn convert_elevations(&self, points: Vec<ElevationPoint>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for point in points {
            let text = format!(
                "Elevation {} meters at lat: {}, lon: {}",
                point.elevation, point.latitude, point.longitude
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("latitude".to_string(), point.latitude.to_string());
            metadata.insert("longitude".to_string(), point.longitude.to_string());
            metadata.insert("elevation_m".to_string(), point.elevation.to_string());
            metadata.insert("source".to_string(), "open_elevation".to_string());

            vectors.push(SemanticVector {
                id: format!("ELEVATION:{}:{}", point.latitude, point.longitude),
                embedding,
                domain: Domain::CrossDomain,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }
}

impl Default for OpenElevationClient {
    fn default() -> Self {
        Self::new().expect("Failed to create OpenElevation client")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nominatim_client_creation() {
        let client = NominatimClient::new();
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.rate_limit_delay, Duration::from_millis(NOMINATIM_RATE_LIMIT_MS));
    }

    #[tokio::test]
    async fn test_nominatim_rate_limiting() {
        let client = NominatimClient::new().unwrap();

        // First request should be immediate
        let start = Instant::now();
        client.enforce_rate_limit().await;
        let first_elapsed = start.elapsed();
        assert!(first_elapsed < Duration::from_millis(100));

        // Second request should be delayed
        let start = Instant::now();
        client.enforce_rate_limit().await;
        let second_elapsed = start.elapsed();
        assert!(second_elapsed >= Duration::from_millis(900)); // Allow some tolerance
    }

    #[tokio::test]
    async fn test_overpass_client_creation() {
        let client = OverpassClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_geonames_client_creation() {
        let client = GeonamesClient::new("test_user".to_string());
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_open_elevation_client_creation() {
        let client = OpenElevationClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_nominatim_place_conversion() {
        let client = NominatimClient::new().unwrap();

        let places = vec![NominatimPlace {
            place_id: 12345,
            licence: "ODbL".to_string(),
            osm_type: "way".to_string(),
            osm_id: 67890,
            lat: "48.8584".to_string(),
            lon: "2.2945".to_string(),
            display_name: "Eiffel Tower, Paris, France".to_string(),
            r#type: "attraction".to_string(),
            importance: 0.9,
            address: Some(NominatimAddress {
                house_number: None,
                road: Some("Champ de Mars".to_string()),
                city: Some("Paris".to_string()),
                state: Some("Île-de-France".to_string()),
                postcode: Some("75007".to_string()),
                country: Some("France".to_string()),
                country_code: Some("fr".to_string()),
            }),
            geojson: None,
        }];

        let vectors = client.convert_places(places).unwrap();
        assert_eq!(vectors.len(), 1);

        let vec = &vectors[0];
        assert_eq!(vec.id, "NOMINATIM:way:67890");
        assert_eq!(vec.metadata.get("city").unwrap(), "Paris");
        assert_eq!(vec.metadata.get("country").unwrap(), "France");
        assert_eq!(vec.domain, Domain::CrossDomain);
    }

    #[test]
    fn test_overpass_element_conversion() {
        let client = OverpassClient::new().unwrap();

        let mut tags = HashMap::new();
        tags.insert("name".to_string(), "Central Park".to_string());
        tags.insert("amenity".to_string(), "park".to_string());

        let elements = vec![OverpassElement {
            r#type: "node".to_string(),
            id: 123456,
            lat: Some(40.7829),
            lon: Some(-73.9654),
            tags,
            center: None,
        }];

        let vectors = client.convert_elements(elements).unwrap();
        assert_eq!(vectors.len(), 1);

        let vec = &vectors[0];
        assert_eq!(vec.id, "OVERPASS:node:123456");
        assert_eq!(vec.metadata.get("name").unwrap(), "Central Park");
        assert_eq!(vec.metadata.get("amenity").unwrap(), "park");
    }

    #[test]
    fn test_geonames_conversion() {
        let client = GeonamesClient::new("test".to_string()).unwrap();

        let geonames = vec![GeoName {
            geonameId: 2988507,
            name: "Paris".to_string(),
            lat: "48.85341".to_string(),
            lng: "2.3488".to_string(),
            countryCode: "FR".to_string(),
            countryName: "France".to_string(),
            fcl: "P".to_string(),
            fcode: "PPLC".to_string(),
            population: 2138551,
            adminName1: "Île-de-France".to_string(),
            toponymName: "Paris".to_string(),
        }];

        let vectors = client.convert_geonames(geonames).unwrap();
        assert_eq!(vectors.len(), 1);

        let vec = &vectors[0];
        assert_eq!(vec.id, "GEONAMES:2988507");
        assert_eq!(vec.metadata.get("name").unwrap(), "Paris");
        assert_eq!(vec.metadata.get("country_code").unwrap(), "FR");
        assert_eq!(vec.metadata.get("population").unwrap(), "2138551");
    }

    #[test]
    fn test_elevation_conversion() {
        let client = OpenElevationClient::new().unwrap();

        let points = vec![
            ElevationPoint {
                latitude: 27.9881,
                longitude: 86.9250,
                elevation: 8848.86,
            },
            ElevationPoint {
                latitude: 40.7128,
                longitude: -74.0060,
                elevation: 10.0,
            },
        ];

        let vectors = client.convert_elevations(points).unwrap();
        assert_eq!(vectors.len(), 2);

        assert_eq!(vectors[0].metadata.get("elevation_m").unwrap(), "8848.86");
        assert_eq!(vectors[1].metadata.get("elevation_m").unwrap(), "10");
    }

    #[test]
    fn test_rate_limits() {
        assert_eq!(NOMINATIM_RATE_LIMIT_MS, 1000); // 1/sec
        assert!(OVERPASS_RATE_LIMIT_MS <= 500); // At least 2/sec
        assert!(GEONAMES_RATE_LIMIT_MS >= 1800); // Conservative for free tier
        assert!(OPEN_ELEVATION_RATE_LIMIT_MS <= 200); // At least 5/sec
    }

    #[test]
    fn test_user_agent_constant() {
        assert!(USER_AGENT.contains("RuVector"));
        assert!(USER_AGENT.contains("github"));
    }

    #[test]
    fn test_geo_utils_integration() {
        // Test GeoUtils distance calculation (from physics_clients)
        let paris_lat = 48.8566;
        let paris_lon = 2.3522;
        let london_lat = 51.5074;
        let london_lon = -0.1278;

        let distance = GeoUtils::distance_km(paris_lat, paris_lon, london_lat, london_lon);

        // Paris to London is approximately 344 km
        assert!((distance - 344.0).abs() < 50.0);
    }

    #[test]
    fn test_geo_utils_within_radius() {
        let center_lat = 48.8566;
        let center_lon = 2.3522;

        // Eiffel Tower is about 2.5km from center of Paris
        let eiffel_lat = 48.8584;
        let eiffel_lon = 2.2945;

        assert!(GeoUtils::within_radius(
            center_lat,
            center_lon,
            eiffel_lat,
            eiffel_lon,
            5.0
        ));

        assert!(!GeoUtils::within_radius(
            center_lat,
            center_lon,
            eiffel_lat,
            eiffel_lon,
            1.0
        ));
    }
}
