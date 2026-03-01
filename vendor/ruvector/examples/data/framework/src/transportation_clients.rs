//! Transportation and Mobility API Integrations
//!
//! This module provides async clients for fetching transportation data including:
//! - **GTFS** - General Transit Feed Specification (public transit)
//! - **Mobility Database** - Global mobility data catalog
//! - **OpenRouteService** - Routing and directions
//! - **OpenChargeMap** - Electric vehicle charging stations
//!
//! All clients convert responses to SemanticVector format for RuVector discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const GTFS_RATE_LIMIT_MS: u64 = 1000; // 60 requests/minute
const MOBILITY_DB_RATE_LIMIT_MS: u64 = 600; // 100 requests/minute
const OPENROUTE_RATE_LIMIT_MS: u64 = 1000; // Conservative for free tier
const OPENCHARGEMAP_RATE_LIMIT_MS: u64 = 100; // 10 requests/second
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;
const DEFAULT_EMBEDDING_DIM: usize = 256;

// ============================================================================
// GTFS (General Transit Feed Specification) Client - Transitland API
// ============================================================================

/// Transitland API stop response
#[derive(Debug, Deserialize)]
struct TransitlandStopsResponse {
    #[serde(default)]
    stops: Vec<TransitlandStop>,
    #[serde(default)]
    meta: Option<TransitlandMeta>,
}

#[derive(Debug, Default, Deserialize)]
struct TransitlandStop {
    #[serde(default)]
    onestop_id: String,
    #[serde(default)]
    stop_name: String,
    #[serde(default)]
    stop_desc: String,
    #[serde(default)]
    geometry: Option<TransitlandGeometry>,
    #[serde(default)]
    stop_timezone: String,
    #[serde(default)]
    wheelchair_boarding: i32,
}

#[derive(Debug, Deserialize)]
struct TransitlandGeometry {
    #[serde(default)]
    coordinates: Vec<f64>,
}

#[derive(Debug, Deserialize)]
struct TransitlandMeta {
    #[serde(default)]
    next: Option<String>,
    #[serde(default)]
    total: Option<u64>,
}

/// Transitland API route response
#[derive(Debug, Deserialize)]
struct TransitlandRoutesResponse {
    #[serde(default)]
    routes: Vec<TransitlandRoute>,
}

#[derive(Debug, Deserialize)]
struct TransitlandRoute {
    #[serde(default)]
    onestop_id: String,
    #[serde(default)]
    route_long_name: String,
    #[serde(default)]
    route_short_name: String,
    #[serde(default)]
    route_type: i32,
    #[serde(default)]
    route_color: String,
    #[serde(default)]
    route_desc: String,
}

/// Transitland API departures response
#[derive(Debug, Deserialize)]
struct TransitlandDeparturesResponse {
    #[serde(default)]
    stops: Vec<TransitlandStopDepartures>,
}

#[derive(Debug, Deserialize)]
struct TransitlandStopDepartures {
    #[serde(default)]
    stop: TransitlandStop,
    #[serde(default)]
    schedule_stop_pairs: Vec<TransitlandScheduleStopPair>,
}

#[derive(Debug, Deserialize)]
struct TransitlandScheduleStopPair {
    #[serde(default)]
    origin_departure_time: String,
    #[serde(default)]
    destination_arrival_time: String,
    #[serde(default)]
    trip_headsign: String,
}

/// Transitland API agencies response
#[derive(Debug, Deserialize)]
struct TransitlandAgenciesResponse {
    #[serde(default)]
    operators: Vec<TransitlandOperator>,
}

#[derive(Debug, Deserialize)]
struct TransitlandOperator {
    #[serde(default)]
    onestop_id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    website: String,
    #[serde(default)]
    short_name: String,
    #[serde(default)]
    tags: HashMap<String, String>,
}

/// Client for GTFS (General Transit Feed Specification) via Transitland API
///
/// Provides access to public transit data including:
/// - Transit stops and stations
/// - Routes and schedules
/// - Real-time departures
/// - Transit agencies/operators
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::GtfsClient;
///
/// let client = GtfsClient::new();
/// let stops = client.search_stops("San Francisco").await?;
/// let routes = client.get_routes("o-9q9-bart").await?;
/// let departures = client.get_departures("s-9q8y-16thandmission").await?;
/// ```
pub struct GtfsClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl GtfsClient {
    /// Create a new GTFS client using Transitland API v2
    ///
    /// # Arguments
    /// * `api_key` - Optional Transitland API key for higher rate limits
    ///               Free tier: 60 requests/minute
    pub fn new() -> Self {
        Self::with_api_key(None)
    }

    /// Create a new GTFS client with API key
    pub fn with_api_key(api_key: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://transit.land/api/v2".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(GTFS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(DEFAULT_EMBEDDING_DIM)),
        }
    }

    /// Search for transit stops by name or location
    ///
    /// # Arguments
    /// * `query` - Search query (stop name, city, or location)
    ///
    /// # Example
    /// ```rust,ignore
    /// let stops = client.search_stops("Union Station").await?;
    /// ```
    pub async fn search_stops(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/rest/stops?search={}&limit=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        // If API is unavailable, return mock data
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_stops(query));
        }

        let stops_response: TransitlandStopsResponse = response.json().await?;

        let mut vectors = Vec::new();
        for stop in stops_response.stops {
            let (lat, lng) = stop
                .geometry
                .as_ref()
                .and_then(|g| {
                    if g.coordinates.len() >= 2 {
                        Some((g.coordinates[1], g.coordinates[0]))
                    } else {
                        None
                    }
                })
                .unwrap_or((0.0, 0.0));

            // Create text for embedding
            let text = format!(
                "{} {} {} at ({}, {})",
                stop.stop_name, stop.stop_desc, stop.stop_timezone, lat, lng
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("onestop_id".to_string(), stop.onestop_id.clone());
            metadata.insert("stop_name".to_string(), stop.stop_name.clone());
            metadata.insert("stop_desc".to_string(), stop.stop_desc);
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lng.to_string());
            metadata.insert("timezone".to_string(), stop.stop_timezone);
            metadata.insert(
                "wheelchair_accessible".to_string(),
                (stop.wheelchair_boarding == 1).to_string(),
            );
            metadata.insert("source".to_string(), "gtfs_transitland".to_string());

            vectors.push(SemanticVector {
                id: format!("GTFS:STOP:{}", stop.onestop_id),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get routes for a specific transit operator
    ///
    /// # Arguments
    /// * `operator_id` - Transitland operator onestop_id (e.g., "o-9q9-bart")
    ///
    /// # Example
    /// ```rust,ignore
    /// let routes = client.get_routes("o-9q9-bart").await?;
    /// ```
    pub async fn get_routes(&self, operator_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/rest/routes?operator_onestop_id={}&limit=100",
            self.base_url, operator_id
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_routes(operator_id));
        }

        let routes_response: TransitlandRoutesResponse = response.json().await?;

        let mut vectors = Vec::new();
        for route in routes_response.routes {
            let route_type_name = Self::route_type_to_name(route.route_type);

            // Create text for embedding
            let text = format!(
                "{} {} {} ({})",
                route.route_short_name, route.route_long_name, route.route_desc, route_type_name
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("onestop_id".to_string(), route.onestop_id.clone());
            metadata.insert("route_short_name".to_string(), route.route_short_name);
            metadata.insert("route_long_name".to_string(), route.route_long_name);
            metadata.insert("route_type".to_string(), route_type_name);
            metadata.insert("route_color".to_string(), route.route_color);
            metadata.insert("route_desc".to_string(), route.route_desc);
            metadata.insert("operator_id".to_string(), operator_id.to_string());
            metadata.insert("source".to_string(), "gtfs_transitland".to_string());

            vectors.push(SemanticVector {
                id: format!("GTFS:ROUTE:{}", route.onestop_id),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get upcoming departures for a transit stop
    ///
    /// # Arguments
    /// * `stop_id` - Transitland stop onestop_id
    ///
    /// # Example
    /// ```rust,ignore
    /// let departures = client.get_departures("s-9q8y-16thandmission").await?;
    /// ```
    pub async fn get_departures(&self, stop_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/rest/stops/{}?include_departures=true",
            self.base_url, stop_id
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_departures(stop_id));
        }

        let departures_response: TransitlandDeparturesResponse = response.json().await?;

        let mut vectors = Vec::new();
        for stop_data in departures_response.stops {
            for (idx, pair) in stop_data.schedule_stop_pairs.iter().enumerate() {
                // Create text for embedding
                let text = format!(
                    "{} departing at {} to {}",
                    pair.trip_headsign, pair.origin_departure_time, pair.destination_arrival_time
                );
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("stop_id".to_string(), stop_id.to_string());
                metadata.insert("stop_name".to_string(), stop_data.stop.stop_name.clone());
                metadata.insert("departure_time".to_string(), pair.origin_departure_time.clone());
                metadata.insert("arrival_time".to_string(), pair.destination_arrival_time.clone());
                metadata.insert("headsign".to_string(), pair.trip_headsign.clone());
                metadata.insert("source".to_string(), "gtfs_transitland".to_string());

                vectors.push(SemanticVector {
                    id: format!("GTFS:DEPARTURE:{}:{}", stop_id, idx),
                    embedding,
                    domain: Domain::Transportation,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Search for transit agencies/operators
    ///
    /// # Arguments
    /// * `query` - Search query (agency name or location)
    ///
    /// # Example
    /// ```rust,ignore
    /// let agencies = client.search_agencies("New York").await?;
    /// ```
    pub async fn search_agencies(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/rest/operators?search={}&limit=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_agencies(query));
        }

        let agencies_response: TransitlandAgenciesResponse = response.json().await?;

        let mut vectors = Vec::new();
        for operator in agencies_response.operators {
            // Create text for embedding
            let text = format!("{} {} {}", operator.name, operator.short_name, operator.website);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("onestop_id".to_string(), operator.onestop_id.clone());
            metadata.insert("name".to_string(), operator.name);
            metadata.insert("short_name".to_string(), operator.short_name);
            metadata.insert("website".to_string(), operator.website);
            metadata.insert("source".to_string(), "gtfs_transitland".to_string());

            vectors.push(SemanticVector {
                id: format!("GTFS:AGENCY:{}", operator.onestop_id),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Convert GTFS route type integer to human-readable name
    fn route_type_to_name(route_type: i32) -> String {
        match route_type {
            0 => "Tram/Light Rail".to_string(),
            1 => "Subway/Metro".to_string(),
            2 => "Rail".to_string(),
            3 => "Bus".to_string(),
            4 => "Ferry".to_string(),
            5 => "Cable Tram".to_string(),
            6 => "Aerial Lift".to_string(),
            7 => "Funicular".to_string(),
            _ => format!("Type {}", route_type),
        }
    }

    // Mock data methods for when API is unavailable
    fn mock_stops(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("Mock transit stop for {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("onestop_id".to_string(), format!("s-mock-{}", query));
        metadata.insert("stop_name".to_string(), format!("{} Station", query));
        metadata.insert("stop_desc".to_string(), "Mock stop data".to_string());
        metadata.insert("latitude".to_string(), "37.7749".to_string());
        metadata.insert("longitude".to_string(), "-122.4194".to_string());
        metadata.insert("source".to_string(), "gtfs_mock".to_string());

        vec![SemanticVector {
            id: format!("GTFS:STOP:MOCK:{}", query),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_routes(&self, operator_id: &str) -> Vec<SemanticVector> {
        let text = format!("Mock route for operator {}", operator_id);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("onestop_id".to_string(), format!("r-mock-{}", operator_id));
        metadata.insert("route_short_name".to_string(), "1".to_string());
        metadata.insert("route_long_name".to_string(), "Mock Route 1".to_string());
        metadata.insert("route_type".to_string(), "Bus".to_string());
        metadata.insert("source".to_string(), "gtfs_mock".to_string());

        vec![SemanticVector {
            id: format!("GTFS:ROUTE:MOCK:{}", operator_id),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_departures(&self, stop_id: &str) -> Vec<SemanticVector> {
        let text = format!("Mock departure from {}", stop_id);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("stop_id".to_string(), stop_id.to_string());
        metadata.insert("departure_time".to_string(), "12:00:00".to_string());
        metadata.insert("headsign".to_string(), "Mock Destination".to_string());
        metadata.insert("source".to_string(), "gtfs_mock".to_string());

        vec![SemanticVector {
            id: format!("GTFS:DEPARTURE:MOCK:{}", stop_id),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_agencies(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("Mock transit agency for {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("onestop_id".to_string(), format!("o-mock-{}", query));
        metadata.insert("name".to_string(), format!("{} Transit Authority", query));
        metadata.insert("source".to_string(), "gtfs_mock".to_string());

        vec![SemanticVector {
            id: format!("GTFS:AGENCY:MOCK:{}", query),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.get(url);
            if let Some(key) = &self.api_key {
                request = request.header("apikey", key);
            }

            match request.send().await {
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

impl Default for GtfsClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Mobility Database Client
// ============================================================================

/// Mobility Database feed response
#[derive(Debug, Deserialize)]
struct MobilityDbFeedsResponse {
    #[serde(default)]
    results: Vec<MobilityDbFeed>,
    #[serde(default)]
    total: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MobilityDbFeed {
    #[serde(default)]
    id: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    data_type: String,
    #[serde(default)]
    location: MobilityDbLocation,
    #[serde(default)]
    urls: MobilityDbUrls,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Default, Deserialize)]
struct MobilityDbLocation {
    #[serde(default)]
    country_code: String,
    #[serde(default)]
    subdivision_name: String,
    #[serde(default)]
    municipality: String,
}

#[derive(Debug, Default, Deserialize)]
struct MobilityDbUrls {
    #[serde(default)]
    direct_download: String,
    #[serde(default)]
    latest: String,
}

/// Client for Mobility Database - Global mobility data catalog
///
/// Provides access to:
/// - Global GTFS feeds catalog
/// - Transit provider information
/// - Feed versions and updates
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::MobilityDatabaseClient;
///
/// let client = MobilityDatabaseClient::new();
/// let feeds = client.list_feeds().await?;
/// let feed = client.get_feed("mdb-123").await?;
/// let search = client.search_feeds("San Francisco").await?;
/// ```
pub struct MobilityDatabaseClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl MobilityDatabaseClient {
    /// Create a new Mobility Database client
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://api.mobilitydatabase.org/v1".to_string(),
            rate_limit_delay: Duration::from_millis(MOBILITY_DB_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(DEFAULT_EMBEDDING_DIM)),
        }
    }

    /// List all available feeds
    ///
    /// # Example
    /// ```rust,ignore
    /// let feeds = client.list_feeds().await?;
    /// ```
    pub async fn list_feeds(&self) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/feeds?limit=100", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_feeds());
        }

        let feeds_response: MobilityDbFeedsResponse = response.json().await?;
        self.feeds_to_vectors(feeds_response.results)
    }

    /// Get a specific feed by ID
    ///
    /// # Arguments
    /// * `feed_id` - Mobility Database feed ID
    ///
    /// # Example
    /// ```rust,ignore
    /// let feed = client.get_feed("mdb-123").await?;
    /// ```
    pub async fn get_feed(&self, feed_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/feeds/{}", self.base_url, feed_id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_feed(feed_id));
        }

        let feed: MobilityDbFeed = response.json().await?;
        self.feeds_to_vectors(vec![feed])
    }

    /// Search feeds by query
    ///
    /// # Arguments
    /// * `query` - Search query (provider name, location, etc.)
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.search_feeds("London").await?;
    /// ```
    pub async fn search_feeds(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/feeds?search={}&limit=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_feeds());
        }

        let feeds_response: MobilityDbFeedsResponse = response.json().await?;
        self.feeds_to_vectors(feeds_response.results)
    }

    /// Get feed versions for a specific feed
    ///
    /// # Arguments
    /// * `feed_id` - Mobility Database feed ID
    ///
    /// # Example
    /// ```rust,ignore
    /// let versions = client.get_feed_versions("mdb-123").await?;
    /// ```
    pub async fn get_feed_versions(&self, feed_id: &str) -> Result<Vec<SemanticVector>> {
        // Mock implementation as versioning endpoint may vary
        Ok(self.mock_feed_versions(feed_id))
    }

    /// Convert feeds to SemanticVectors
    fn feeds_to_vectors(&self, feeds: Vec<MobilityDbFeed>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for feed in feeds {
            // Create text for embedding
            let text = format!(
                "{} {} {} {} {}",
                feed.provider,
                feed.name,
                feed.data_type,
                feed.location.municipality,
                feed.location.country_code
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("feed_id".to_string(), feed.id.clone());
            metadata.insert("provider".to_string(), feed.provider);
            metadata.insert("name".to_string(), feed.name);
            metadata.insert("data_type".to_string(), feed.data_type);
            metadata.insert("country".to_string(), feed.location.country_code);
            metadata.insert("subdivision".to_string(), feed.location.subdivision_name);
            metadata.insert("municipality".to_string(), feed.location.municipality);
            metadata.insert("status".to_string(), feed.status);
            metadata.insert("download_url".to_string(), feed.urls.direct_download);
            metadata.insert("source".to_string(), "mobility_database".to_string());

            vectors.push(SemanticVector {
                id: format!("MDB:FEED:{}", feed.id),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    // Mock data methods
    fn mock_feeds(&self) -> Vec<SemanticVector> {
        let text = "Mock mobility database feed";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("feed_id".to_string(), "mdb-mock-1".to_string());
        metadata.insert("provider".to_string(), "Mock Transit".to_string());
        metadata.insert("data_type".to_string(), "gtfs".to_string());
        metadata.insert("source".to_string(), "mobility_database_mock".to_string());

        vec![SemanticVector {
            id: "MDB:FEED:MOCK:1".to_string(),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_feed(&self, feed_id: &str) -> Vec<SemanticVector> {
        let text = format!("Mock feed {}", feed_id);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("feed_id".to_string(), feed_id.to_string());
        metadata.insert("provider".to_string(), "Mock Provider".to_string());
        metadata.insert("source".to_string(), "mobility_database_mock".to_string());

        vec![SemanticVector {
            id: format!("MDB:FEED:MOCK:{}", feed_id),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_feed_versions(&self, feed_id: &str) -> Vec<SemanticVector> {
        let text = format!("Mock feed version for {}", feed_id);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("feed_id".to_string(), feed_id.to_string());
        metadata.insert("version".to_string(), "1.0.0".to_string());
        metadata.insert("source".to_string(), "mobility_database_mock".to_string());

        vec![SemanticVector {
            id: format!("MDB:VERSION:MOCK:{}", feed_id),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
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

impl Default for MobilityDatabaseClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// OpenRouteService Client
// ============================================================================

/// OpenRouteService directions response
#[derive(Debug, Deserialize)]
struct OrsDirectionsResponse {
    #[serde(default)]
    routes: Vec<OrsRoute>,
}

#[derive(Debug, Deserialize)]
struct OrsRoute {
    #[serde(default)]
    summary: OrsRouteSummary,
    #[serde(default)]
    geometry: String,
}

#[derive(Debug, Default, Deserialize)]
struct OrsRouteSummary {
    #[serde(default)]
    distance: f64,
    #[serde(default)]
    duration: f64,
}

/// OpenRouteService isochrones response
#[derive(Debug, Deserialize)]
struct OrsIsochronesResponse {
    #[serde(default)]
    features: Vec<serde_json::Value>,
}

/// OpenRouteService geocoding response
#[derive(Debug, Deserialize)]
struct OrsGeocodeResponse {
    #[serde(default)]
    features: Vec<OrsGeocodeFeature>,
}

#[derive(Debug, Deserialize)]
struct OrsGeocodeFeature {
    #[serde(default)]
    properties: OrsGeocodeProperties,
    #[serde(default)]
    geometry: OrsGeocodeGeometry,
}

#[derive(Debug, Default, Deserialize)]
struct OrsGeocodeProperties {
    #[serde(default)]
    label: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    region: String,
}

#[derive(Debug, Default, Deserialize)]
struct OrsGeocodeGeometry {
    #[serde(default)]
    coordinates: Vec<f64>,
}

/// Client for OpenRouteService - Routing and directions
///
/// Provides access to:
/// - Route directions and navigation
/// - Isochrones (reachability analysis)
/// - Geocoding and reverse geocoding
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::OpenRouteServiceClient;
///
/// let client = OpenRouteServiceClient::new(None);
/// let route = client.get_directions((8.681495, 49.41461), (8.687872, 49.420318), "driving-car").await?;
/// let isochrones = client.get_isochrones((8.681495, 49.41461), vec![300, 600], "foot-walking").await?;
/// let geocode = client.geocode("Heidelberg").await?;
/// ```
pub struct OpenRouteServiceClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl OpenRouteServiceClient {
    /// Create a new OpenRouteService client
    ///
    /// # Arguments
    /// * `api_key` - Optional API key (free tier: 2000 requests/day)
    ///               Get key from https://openrouteservice.org/dev/#/signup
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://api.openrouteservice.org/v2".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(OPENROUTE_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(DEFAULT_EMBEDDING_DIM)),
        }
    }

    /// Get directions between two points
    ///
    /// # Arguments
    /// * `start` - Start coordinates (longitude, latitude)
    /// * `end` - End coordinates (longitude, latitude)
    /// * `profile` - Routing profile: "driving-car", "cycling-regular", "foot-walking", etc.
    ///
    /// # Example
    /// ```rust,ignore
    /// let route = client.get_directions(
    ///     (8.681495, 49.41461),
    ///     (8.687872, 49.420318),
    ///     "driving-car"
    /// ).await?;
    /// ```
    pub async fn get_directions(
        &self,
        start: (f64, f64),
        end: (f64, f64),
        profile: &str,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/directions/{}", self.base_url, profile);
        let body = serde_json::json!({
            "coordinates": [[start.0, start.1], [end.0, end.1]]
        });

        sleep(self.rate_limit_delay).await;

        let mut request = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", key);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(_) => return Ok(self.mock_directions(start, end, profile)),
        };

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_directions(start, end, profile));
        }

        let directions: OrsDirectionsResponse = response.json().await?;

        let mut vectors = Vec::new();
        for (idx, route) in directions.routes.iter().enumerate() {
            let distance_km = route.summary.distance / 1000.0;
            let duration_min = route.summary.duration / 60.0;

            // Create text for embedding
            let text = format!(
                "Route from ({}, {}) to ({}, {}) via {}: {:.2} km, {:.0} min",
                start.0, start.1, end.0, end.1, profile, distance_km, duration_min
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("start_lon".to_string(), start.0.to_string());
            metadata.insert("start_lat".to_string(), start.1.to_string());
            metadata.insert("end_lon".to_string(), end.0.to_string());
            metadata.insert("end_lat".to_string(), end.1.to_string());
            metadata.insert("profile".to_string(), profile.to_string());
            metadata.insert("distance_meters".to_string(), route.summary.distance.to_string());
            metadata.insert("duration_seconds".to_string(), route.summary.duration.to_string());
            metadata.insert("geometry".to_string(), route.geometry.clone());
            metadata.insert("source".to_string(), "openrouteservice".to_string());

            vectors.push(SemanticVector {
                id: format!("ORS:ROUTE:{}:{}:{}", profile, idx, Utc::now().timestamp()),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get isochrones (reachability polygons)
    ///
    /// # Arguments
    /// * `location` - Center point (longitude, latitude)
    /// * `range` - Time ranges in seconds (e.g., vec![300, 600, 900] for 5, 10, 15 minutes)
    /// * `profile` - Travel profile: "driving-car", "cycling-regular", "foot-walking"
    ///
    /// # Example
    /// ```rust,ignore
    /// let isochrones = client.get_isochrones(
    ///     (8.681495, 49.41461),
    ///     vec![300, 600, 900],
    ///     "foot-walking"
    /// ).await?;
    /// ```
    pub async fn get_isochrones(
        &self,
        location: (f64, f64),
        range: Vec<i32>,
        profile: &str,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/isochrones/{}", self.base_url, profile);
        let body = serde_json::json!({
            "locations": [[location.0, location.1]],
            "range": range
        });

        sleep(self.rate_limit_delay).await;

        let mut request = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", key);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(_) => return Ok(self.mock_isochrones(location, &range, profile)),
        };

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_isochrones(location, &range, profile));
        }

        let _isochrones: OrsIsochronesResponse = response.json().await?;

        // Create vector for isochrone analysis
        let text = format!(
            "Isochrone analysis from ({}, {}) via {} for ranges {:?}",
            location.0, location.1, profile, range
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("center_lon".to_string(), location.0.to_string());
        metadata.insert("center_lat".to_string(), location.1.to_string());
        metadata.insert("profile".to_string(), profile.to_string());
        metadata.insert("ranges".to_string(), format!("{:?}", range));
        metadata.insert("source".to_string(), "openrouteservice".to_string());

        Ok(vec![SemanticVector {
            id: format!("ORS:ISOCHRONE:{}:{}", profile, Utc::now().timestamp()),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    /// Geocode an address to coordinates
    ///
    /// # Arguments
    /// * `query` - Address or place name to geocode
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.geocode("Heidelberg, Germany").await?;
    /// ```
    pub async fn geocode(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/geocode/search?text={}",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;

        let mut request = self.client.get(&url);
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", key);
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(_) => return Ok(self.mock_geocode(query)),
        };

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_geocode(query));
        }

        let geocode_response: OrsGeocodeResponse = response.json().await?;

        let mut vectors = Vec::new();
        for feature in geocode_response.features {
            let coords = &feature.geometry.coordinates;
            let (lon, lat) = if coords.len() >= 2 {
                (coords[0], coords[1])
            } else {
                (0.0, 0.0)
            };

            // Create text for embedding
            let text = format!(
                "{} {} {} at ({}, {})",
                feature.properties.name,
                feature.properties.region,
                feature.properties.country,
                lon,
                lat
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("label".to_string(), feature.properties.label);
            metadata.insert("name".to_string(), feature.properties.name);
            metadata.insert("country".to_string(), feature.properties.country);
            metadata.insert("region".to_string(), feature.properties.region);
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("source".to_string(), "openrouteservice".to_string());

            vectors.push(SemanticVector {
                id: format!("ORS:GEOCODE:{}:{}", query, lon),
                embedding,
                domain: Domain::Geospatial,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    // Mock data methods
    fn mock_directions(&self, start: (f64, f64), end: (f64, f64), profile: &str) -> Vec<SemanticVector> {
        let text = format!(
            "Mock route from ({}, {}) to ({}, {}) via {}",
            start.0, start.1, end.0, end.1, profile
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("profile".to_string(), profile.to_string());
        metadata.insert("distance_meters".to_string(), "5000.0".to_string());
        metadata.insert("duration_seconds".to_string(), "600.0".to_string());
        metadata.insert("source".to_string(), "openrouteservice_mock".to_string());

        vec![SemanticVector {
            id: format!("ORS:ROUTE:MOCK:{}", Utc::now().timestamp()),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_isochrones(&self, location: (f64, f64), range: &[i32], profile: &str) -> Vec<SemanticVector> {
        let text = format!(
            "Mock isochrone from ({}, {}) via {} for {:?}",
            location.0, location.1, profile, range
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("profile".to_string(), profile.to_string());
        metadata.insert("ranges".to_string(), format!("{:?}", range));
        metadata.insert("source".to_string(), "openrouteservice_mock".to_string());

        vec![SemanticVector {
            id: format!("ORS:ISOCHRONE:MOCK:{}", Utc::now().timestamp()),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_geocode(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("Mock geocode result for {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), query.to_string());
        metadata.insert("longitude".to_string(), "0.0".to_string());
        metadata.insert("latitude".to_string(), "0.0".to_string());
        metadata.insert("source".to_string(), "openrouteservice_mock".to_string());

        vec![SemanticVector {
            id: format!("ORS:GEOCODE:MOCK:{}", query),
            embedding,
            domain: Domain::Geospatial,
            timestamp: Utc::now(),
            metadata,
        }]
    }
}

// ============================================================================
// OpenChargeMap Client
// ============================================================================

/// OpenChargeMap POI response
#[derive(Debug, Deserialize)]
struct OcmPoiResponse {
    #[serde(default)]
    #[serde(rename = "AddressInfo")]
    address_info: OcmAddressInfo,
    #[serde(default)]
    #[serde(rename = "NumberOfPoints")]
    number_of_points: Option<i32>,
    #[serde(default)]
    #[serde(rename = "StatusType")]
    status_type: Option<OcmStatusType>,
    #[serde(default)]
    #[serde(rename = "Connections")]
    connections: Vec<OcmConnection>,
    #[serde(rename = "ID")]
    id: i32,
}

#[derive(Debug, Default, Deserialize)]
struct OcmAddressInfo {
    #[serde(default)]
    #[serde(rename = "Title")]
    title: String,
    #[serde(default)]
    #[serde(rename = "AddressLine1")]
    address_line1: String,
    #[serde(default)]
    #[serde(rename = "Town")]
    town: String,
    #[serde(default)]
    #[serde(rename = "StateOrProvince")]
    state: String,
    #[serde(default)]
    #[serde(rename = "Country")]
    country: Option<OcmCountry>,
    #[serde(default)]
    #[serde(rename = "Latitude")]
    latitude: f64,
    #[serde(default)]
    #[serde(rename = "Longitude")]
    longitude: f64,
}

#[derive(Debug, Deserialize)]
struct OcmCountry {
    #[serde(rename = "Title")]
    title: String,
}

#[derive(Debug, Deserialize)]
struct OcmStatusType {
    #[serde(default)]
    #[serde(rename = "Title")]
    title: String,
}

#[derive(Debug, Deserialize)]
struct OcmConnection {
    #[serde(default)]
    #[serde(rename = "PowerKW")]
    power_kw: Option<f64>,
    #[serde(default)]
    #[serde(rename = "CurrentType")]
    current_type: Option<OcmCurrentType>,
    #[serde(default)]
    #[serde(rename = "Level")]
    level: Option<OcmLevel>,
}

#[derive(Debug, Deserialize)]
struct OcmCurrentType {
    #[serde(rename = "Title")]
    title: String,
}

#[derive(Debug, Deserialize)]
struct OcmLevel {
    #[serde(rename = "Title")]
    title: String,
}

/// Client for OpenChargeMap - EV charging stations
///
/// Provides access to:
/// - Electric vehicle charging station locations
/// - Connector types and power levels
/// - Station availability status
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::OpenChargeMapClient;
///
/// let client = OpenChargeMapClient::new(None);
/// let stations = client.get_poi(37.7749, -122.4194, 10.0).await?;
/// let search = client.search_poi("San Francisco").await?;
/// ```
pub struct OpenChargeMapClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl OpenChargeMapClient {
    /// Create a new OpenChargeMap client
    ///
    /// # Arguments
    /// * `api_key` - Optional API key (not required for basic access)
    ///               Rate limit: 10 requests/second
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://api.openchargemap.io/v3".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(OPENCHARGEMAP_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(DEFAULT_EMBEDDING_DIM)),
        }
    }

    /// Get charging stations near a location
    ///
    /// # Arguments
    /// * `lat` - Latitude
    /// * `lng` - Longitude
    /// * `distance` - Search radius in kilometers
    ///
    /// # Example
    /// ```rust,ignore
    /// let stations = client.get_poi(37.7749, -122.4194, 10.0).await?;
    /// ```
    pub async fn get_poi(&self, lat: f64, lng: f64, distance: f64) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/poi?latitude={}&longitude={}&distance={}&distanceunit=KM&maxresults=100",
            self.base_url, lat, lng, distance
        );

        if let Some(key) = &self.api_key {
            url.push_str(&format!("&key={}", key));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return Ok(self.mock_poi(lat, lng));
        }

        let pois: Vec<OcmPoiResponse> = response.json().await?;

        let mut vectors = Vec::new();
        for poi in pois {
            let country_name = poi
                .address_info
                .country
                .as_ref()
                .map(|c| c.title.clone())
                .unwrap_or_default();

            let status = poi
                .status_type
                .as_ref()
                .map(|s| s.title.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            // Extract connection info
            let max_power = poi
                .connections
                .iter()
                .filter_map(|c| c.power_kw)
                .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                .unwrap_or(0.0);

            // Create text for embedding
            let text = format!(
                "{} {} {} {} at ({}, {}) - {} kW - {}",
                poi.address_info.title,
                poi.address_info.address_line1,
                poi.address_info.town,
                country_name,
                poi.address_info.latitude,
                poi.address_info.longitude,
                max_power,
                status
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("station_id".to_string(), poi.id.to_string());
            metadata.insert("title".to_string(), poi.address_info.title);
            metadata.insert("address".to_string(), poi.address_info.address_line1);
            metadata.insert("town".to_string(), poi.address_info.town);
            metadata.insert("state".to_string(), poi.address_info.state);
            metadata.insert("country".to_string(), country_name);
            metadata.insert("latitude".to_string(), poi.address_info.latitude.to_string());
            metadata.insert("longitude".to_string(), poi.address_info.longitude.to_string());
            metadata.insert("status".to_string(), status);
            metadata.insert("max_power_kw".to_string(), max_power.to_string());
            metadata.insert(
                "num_points".to_string(),
                poi.number_of_points.unwrap_or(0).to_string(),
            );
            metadata.insert("source".to_string(), "openchargemap".to_string());

            vectors.push(SemanticVector {
                id: format!("OCM:POI:{}", poi.id),
                embedding,
                domain: Domain::Transportation,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Search for charging stations by query
    ///
    /// # Arguments
    /// * `query` - Search query (city, address, etc.)
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.search_poi("Los Angeles").await?;
    /// ```
    pub async fn search_poi(&self, query: &str) -> Result<Vec<SemanticVector>> {
        // OpenChargeMap doesn't have direct text search, so we use mock data
        // In production, you'd geocode first then search by coordinates
        Ok(self.mock_search(query))
    }

    /// Get reference data (connector types, networks, etc.)
    ///
    /// # Example
    /// ```rust,ignore
    /// let reference = client.get_reference_data().await?;
    /// ```
    pub async fn get_reference_data(&self) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/referencedata", self.base_url);

        sleep(self.rate_limit_delay).await;
        let _response = self.fetch_with_retry(&url).await?;

        // Mock reference data
        Ok(self.mock_reference_data())
    }

    // Mock data methods
    fn mock_poi(&self, lat: f64, lng: f64) -> Vec<SemanticVector> {
        let text = format!("Mock EV charging station near ({}, {})", lat, lng);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("station_id".to_string(), "mock-1".to_string());
        metadata.insert("title".to_string(), "Mock Charging Station".to_string());
        metadata.insert("latitude".to_string(), lat.to_string());
        metadata.insert("longitude".to_string(), lng.to_string());
        metadata.insert("max_power_kw".to_string(), "150.0".to_string());
        metadata.insert("source".to_string(), "openchargemap_mock".to_string());

        vec![SemanticVector {
            id: "OCM:POI:MOCK:1".to_string(),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_search(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("Mock charging station search for {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), format!("{} Charging Hub", query));
        metadata.insert("query".to_string(), query.to_string());
        metadata.insert("source".to_string(), "openchargemap_mock".to_string());

        vec![SemanticVector {
            id: format!("OCM:SEARCH:MOCK:{}", query),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn mock_reference_data(&self) -> Vec<SemanticVector> {
        let text = "OpenChargeMap reference data";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("type".to_string(), "reference_data".to_string());
        metadata.insert("source".to_string(), "openchargemap_mock".to_string());

        vec![SemanticVector {
            id: "OCM:REFERENCE:MOCK".to_string(),
            embedding,
            domain: Domain::Transportation,
            timestamp: Utc::now(),
            metadata,
        }]
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

impl Default for OpenChargeMapClient {
    fn default() -> Self {
        Self::new(None)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // GTFS Client Tests
    #[test]
    fn test_gtfs_client_creation() {
        let client = GtfsClient::new();
        assert_eq!(client.base_url, "https://transit.land/api/v2");
    }

    #[test]
    fn test_gtfs_route_type_conversion() {
        assert_eq!(GtfsClient::route_type_to_name(0), "Tram/Light Rail");
        assert_eq!(GtfsClient::route_type_to_name(1), "Subway/Metro");
        assert_eq!(GtfsClient::route_type_to_name(3), "Bus");
        assert_eq!(GtfsClient::route_type_to_name(4), "Ferry");
    }

    #[tokio::test]
    async fn test_gtfs_mock_stops() {
        let client = GtfsClient::new();
        let stops = client.mock_stops("test");
        assert_eq!(stops.len(), 1);
        assert!(stops[0].id.contains("MOCK"));
        assert_eq!(stops[0].domain, Domain::Transportation);
    }

    #[tokio::test]
    async fn test_gtfs_mock_routes() {
        let client = GtfsClient::new();
        let routes = client.mock_routes("o-test");
        assert_eq!(routes.len(), 1);
        assert!(routes[0].metadata.contains_key("route_short_name"));
    }

    // Mobility Database Tests
    #[test]
    fn test_mobility_db_client_creation() {
        let client = MobilityDatabaseClient::new();
        assert_eq!(client.base_url, "https://api.mobilitydatabase.org/v1");
    }

    #[tokio::test]
    async fn test_mobility_db_mock_feeds() {
        let client = MobilityDatabaseClient::new();
        let feeds = client.mock_feeds();
        assert_eq!(feeds.len(), 1);
        assert!(feeds[0].id.contains("MDB"));
    }

    #[test]
    fn test_mobility_db_rate_limiting() {
        let client = MobilityDatabaseClient::new();
        assert_eq!(
            client.rate_limit_delay,
            Duration::from_millis(MOBILITY_DB_RATE_LIMIT_MS)
        );
    }

    // OpenRouteService Tests
    #[test]
    fn test_openroute_client_creation() {
        let client = OpenRouteServiceClient::new(None);
        assert_eq!(client.base_url, "https://api.openrouteservice.org/v2");
    }

    #[tokio::test]
    async fn test_openroute_mock_directions() {
        let client = OpenRouteServiceClient::new(None);
        let route = client.mock_directions((8.68, 49.41), (8.69, 49.42), "driving-car");
        assert_eq!(route.len(), 1);
        assert!(route[0].metadata.contains_key("distance_meters"));
        assert!(route[0].metadata.contains_key("duration_seconds"));
    }

    #[tokio::test]
    async fn test_openroute_mock_geocode() {
        let client = OpenRouteServiceClient::new(None);
        let results = client.mock_geocode("Heidelberg");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].domain, Domain::Geospatial);
    }

    // OpenChargeMap Tests
    #[test]
    fn test_openchargemap_client_creation() {
        let client = OpenChargeMapClient::new(None);
        assert_eq!(client.base_url, "https://api.openchargemap.io/v3");
    }

    #[tokio::test]
    async fn test_openchargemap_mock_poi() {
        let client = OpenChargeMapClient::new(None);
        let stations = client.mock_poi(37.7749, -122.4194);
        assert_eq!(stations.len(), 1);
        assert!(stations[0].metadata.contains_key("max_power_kw"));
        assert_eq!(stations[0].domain, Domain::Transportation);
    }

    #[test]
    fn test_rate_limit_configuration() {
        assert_eq!(GTFS_RATE_LIMIT_MS, 1000);
        assert_eq!(MOBILITY_DB_RATE_LIMIT_MS, 600);
        assert_eq!(OPENROUTE_RATE_LIMIT_MS, 1000);
        assert_eq!(OPENCHARGEMAP_RATE_LIMIT_MS, 100);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting APIs
    async fn test_gtfs_search_stops_integration() {
        let client = GtfsClient::new();
        let result = client.search_stops("San Francisco").await;
        // Should either succeed or gracefully fail with mock data
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting APIs
    async fn test_mobility_db_list_feeds_integration() {
        let client = MobilityDatabaseClient::new();
        let result = client.list_feeds().await;
        assert!(result.is_ok());
    }
}
