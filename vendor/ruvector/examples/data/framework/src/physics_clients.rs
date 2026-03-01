//! Physics, seismic, and ocean data API integrations
//!
//! This module provides async clients for:
//! - USGS Earthquake Hazards Program
//! - CERN Open Data Portal
//! - Argo Float Ocean Data
//! - Materials Project
//!
//! All responses are converted to SemanticVector format for RuVector discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const USGS_RATE_LIMIT_MS: u64 = 200; // ~5 requests/second
const CERN_RATE_LIMIT_MS: u64 = 500; // Conservative rate
const ARGO_RATE_LIMIT_MS: u64 = 300; // ~3 requests/second
const MATERIALS_PROJECT_RATE_LIMIT_MS: u64 = 1000; // 1 request/second for free tier
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// Geographic Coordinate Utilities
// ============================================================================

/// Geographic coordinate utilities for region-based searches
pub struct GeoUtils;

impl GeoUtils {
    /// Calculate approximate distance between two lat/lon points (Haversine formula)
    /// Returns distance in kilometers
    pub fn distance_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0; // Earth radius in km
        let dlat = (lat2 - lat1).to_radians();
        let dlon = (lon2 - lon1).to_radians();
        let a = (dlat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    /// Check if a point is within a radius of a center point
    pub fn within_radius(
        center_lat: f64,
        center_lon: f64,
        point_lat: f64,
        point_lon: f64,
        radius_km: f64,
    ) -> bool {
        Self::distance_km(center_lat, center_lon, point_lat, point_lon) <= radius_km
    }
}

// ============================================================================
// USGS Earthquake Hazards Program Client
// ============================================================================

/// USGS GeoJSON response format
#[derive(Debug, Deserialize)]
struct UsgsGeoJsonResponse {
    #[serde(default)]
    features: Vec<UsgsEarthquakeFeature>,
    #[serde(default)]
    metadata: UsgsMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct UsgsMetadata {
    #[serde(default)]
    count: u32,
}

#[derive(Debug, Deserialize)]
struct UsgsEarthquakeFeature {
    id: String,
    properties: UsgsProperties,
    geometry: UsgsGeometry,
}

#[derive(Debug, Deserialize)]
struct UsgsProperties {
    #[serde(default)]
    mag: Option<f64>,
    #[serde(default)]
    place: String,
    #[serde(default)]
    time: i64, // Unix timestamp in milliseconds
    #[serde(default)]
    updated: i64,
    #[serde(default)]
    tz: Option<i32>,
    #[serde(default)]
    url: String,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    felt: Option<u32>,
    #[serde(default)]
    cdi: Option<f64>, // Community Decimal Intensity
    #[serde(default)]
    mmi: Option<f64>, // Modified Mercalli Intensity
    #[serde(default)]
    alert: Option<String>,
    #[serde(default)]
    status: String,
    #[serde(default)]
    tsunami: u8,
    #[serde(default)]
    sig: u32, // Significance
    #[serde(default)]
    net: String,
    #[serde(default)]
    code: String,
    #[serde(default)]
    r#type: String,
    #[serde(default)]
    title: String,
}

#[derive(Debug, Deserialize)]
struct UsgsGeometry {
    coordinates: Vec<f64>, // [longitude, latitude, depth]
}

/// Client for USGS Earthquake Hazards Program
///
/// Provides access to:
/// - Real-time earthquake data worldwide
/// - Historical earthquake records
/// - Magnitude, location, depth information
/// - Tsunami warnings and alerts
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::UsgsEarthquakeClient;
///
/// let client = UsgsEarthquakeClient::new()?;
/// let recent = client.get_recent(4.5, 7).await?; // Mag 4.5+, last 7 days
/// let regional = client.search_by_region(35.0, -118.0, 200.0, 30).await?;
/// let significant = client.get_significant(30).await?;
/// ```
pub struct UsgsEarthquakeClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl UsgsEarthquakeClient {
    /// Create a new USGS Earthquake client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://earthquake.usgs.gov/fdsnws/event/1".to_string(),
            rate_limit_delay: Duration::from_millis(USGS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get recent earthquakes above a minimum magnitude
    ///
    /// # Arguments
    /// * `min_magnitude` - Minimum magnitude (e.g., 4.5)
    /// * `days` - Number of days to look back (e.g., 7 for last week)
    ///
    /// # Example
    /// ```rust,ignore
    /// let earthquakes = client.get_recent(5.0, 30).await?;
    /// ```
    pub async fn get_recent(
        &self,
        min_magnitude: f64,
        days: u32,
    ) -> Result<Vec<SemanticVector>> {
        let now = Utc::now();
        let start_time = now - chrono::Duration::days(days as i64);

        let url = format!(
            "{}/query?format=geojson&starttime={}&endtime={}&minmagnitude={}",
            self.base_url,
            start_time.format("%Y-%m-%d"),
            now.format("%Y-%m-%d"),
            min_magnitude
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let geojson: UsgsGeoJsonResponse = response.json().await?;

        self.convert_earthquakes(geojson.features)
    }

    /// Search earthquakes by geographic region
    ///
    /// # Arguments
    /// * `lat` - Center latitude
    /// * `lon` - Center longitude
    /// * `radius_km` - Search radius in kilometers (max 20001.6 km)
    /// * `days` - Number of days to look back
    ///
    /// # Example
    /// ```rust,ignore
    /// // Search near Los Angeles
    /// let la_quakes = client.search_by_region(34.05, -118.25, 100.0, 7).await?;
    /// ```
    pub async fn search_by_region(
        &self,
        lat: f64,
        lon: f64,
        radius_km: f64,
        days: u32,
    ) -> Result<Vec<SemanticVector>> {
        let now = Utc::now();
        let start_time = now - chrono::Duration::days(days as i64);

        let url = format!(
            "{}/query?format=geojson&starttime={}&endtime={}&latitude={}&longitude={}&maxradiuskm={}",
            self.base_url,
            start_time.format("%Y-%m-%d"),
            now.format("%Y-%m-%d"),
            lat,
            lon,
            radius_km
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let geojson: UsgsGeoJsonResponse = response.json().await?;

        self.convert_earthquakes(geojson.features)
    }

    /// Get significant earthquakes (as determined by USGS)
    ///
    /// # Arguments
    /// * `days` - Number of days to look back
    ///
    /// # Example
    /// ```rust,ignore
    /// let significant = client.get_significant(30).await?;
    /// ```
    pub async fn get_significant(&self, days: u32) -> Result<Vec<SemanticVector>> {
        let now = Utc::now();
        let start_time = now - chrono::Duration::days(days as i64);

        let url = format!(
            "{}/query?format=geojson&starttime={}&endtime={}&orderby=magnitude&limit=100",
            self.base_url,
            start_time.format("%Y-%m-%d"),
            now.format("%Y-%m-%d")
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let geojson: UsgsGeoJsonResponse = response.json().await?;

        // Filter for significant (magnitude >= 6.0 or high significance score)
        let significant: Vec<_> = geojson
            .features
            .into_iter()
            .filter(|f| {
                f.properties.mag.unwrap_or(0.0) >= 6.0 || f.properties.sig >= 600
            })
            .collect();

        self.convert_earthquakes(significant)
    }

    /// Get earthquakes within a magnitude range
    ///
    /// # Arguments
    /// * `min` - Minimum magnitude
    /// * `max` - Maximum magnitude
    /// * `days` - Number of days to look back
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get moderate earthquakes (4.0-6.0)
    /// let moderate = client.get_by_magnitude_range(4.0, 6.0, 7).await?;
    /// ```
    pub async fn get_by_magnitude_range(
        &self,
        min: f64,
        max: f64,
        days: u32,
    ) -> Result<Vec<SemanticVector>> {
        let now = Utc::now();
        let start_time = now - chrono::Duration::days(days as i64);

        let url = format!(
            "{}/query?format=geojson&starttime={}&endtime={}&minmagnitude={}&maxmagnitude={}",
            self.base_url,
            start_time.format("%Y-%m-%d"),
            now.format("%Y-%m-%d"),
            min,
            max
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let geojson: UsgsGeoJsonResponse = response.json().await?;

        self.convert_earthquakes(geojson.features)
    }

    /// Convert USGS earthquake features to SemanticVectors
    fn convert_earthquakes(&self, features: Vec<UsgsEarthquakeFeature>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for feature in features {
            let mag = feature.properties.mag.unwrap_or(0.0);
            let coords = &feature.geometry.coordinates;
            let lon = coords.get(0).copied().unwrap_or(0.0);
            let lat = coords.get(1).copied().unwrap_or(0.0);
            let depth = coords.get(2).copied().unwrap_or(0.0);

            // Convert Unix timestamp (milliseconds) to DateTime
            let timestamp = DateTime::from_timestamp_millis(feature.properties.time)
                .unwrap_or_else(Utc::now);

            // Create text for embedding
            let text = format!(
                "Magnitude {} earthquake {} at depth {}km (lat: {}, lon: {})",
                mag, feature.properties.place, depth, lat, lon
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("magnitude".to_string(), mag.to_string());
            metadata.insert("place".to_string(), feature.properties.place);
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("depth_km".to_string(), depth.to_string());
            metadata.insert("tsunami".to_string(), feature.properties.tsunami.to_string());
            metadata.insert("significance".to_string(), feature.properties.sig.to_string());
            metadata.insert("status".to_string(), feature.properties.status);
            if let Some(alert) = feature.properties.alert {
                metadata.insert("alert".to_string(), alert);
            }
            metadata.insert("source".to_string(), "usgs".to_string());

            vectors.push(SemanticVector {
                id: format!("USGS:{}", feature.id),
                embedding,
                domain: Domain::Seismic,
                timestamp,
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

impl Default for UsgsEarthquakeClient {
    fn default() -> Self {
        Self::new().expect("Failed to create USGS client")
    }
}

// ============================================================================
// CERN Open Data Portal Client
// ============================================================================

/// CERN Open Data record
#[derive(Debug, Deserialize)]
struct CernRecord {
    id: u64,
    #[serde(default)]
    metadata: CernMetadata,
}

#[derive(Debug, Deserialize, Default)]
struct CernMetadata {
    #[serde(default)]
    titles: Vec<CernTitle>,
    #[serde(default)]
    r#abstract: Option<CernAbstract>,
    #[serde(default)]
    experiment: Option<String>,
    #[serde(default)]
    collision_information: Option<CernCollisionInfo>,
    #[serde(default)]
    date_created: Vec<String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    r#type: CernType,
}

#[derive(Debug, Deserialize)]
struct CernTitle {
    title: String,
}

#[derive(Debug, Deserialize)]
struct CernAbstract {
    description: String,
}

#[derive(Debug, Deserialize)]
struct CernCollisionInfo {
    #[serde(default)]
    energy: String,
    #[serde(default)]
    r#type: String,
}

#[derive(Debug, Deserialize, Default)]
struct CernType {
    #[serde(default)]
    primary: String,
    #[serde(default)]
    secondary: Vec<String>,
}

/// CERN API search response
#[derive(Debug, Deserialize)]
struct CernSearchResponse {
    #[serde(default)]
    hits: CernHits,
}

#[derive(Debug, Deserialize, Default)]
struct CernHits {
    #[serde(default)]
    hits: Vec<CernRecord>,
    #[serde(default)]
    total: u32,
}

/// Client for CERN Open Data Portal
///
/// Provides access to:
/// - LHC experiment data (CMS, ATLAS, LHCb, ALICE)
/// - Collision events and particle physics datasets
/// - Education and outreach materials
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::CernOpenDataClient;
///
/// let client = CernOpenDataClient::new()?;
/// let datasets = client.search_datasets("Higgs").await?;
/// let cms_data = client.search_by_experiment("CMS").await?;
/// let dataset = client.get_dataset(12345).await?;
/// ```
pub struct CernOpenDataClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl CernOpenDataClient {
    /// Create a new CERN Open Data client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://opendata.cern.ch/api/records".to_string(),
            rate_limit_delay: Duration::from_millis(CERN_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Search datasets by query string
    ///
    /// # Arguments
    /// * `query` - Search query (e.g., "Higgs", "top quark", "W boson")
    ///
    /// # Example
    /// ```rust,ignore
    /// let higgs_data = client.search_datasets("Higgs boson").await?;
    /// ```
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}?q={}&size=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: CernSearchResponse = response.json().await?;

        self.convert_records(search_response.hits.hits)
    }

    /// Get a specific dataset by record ID
    ///
    /// # Arguments
    /// * `recid` - CERN record ID
    ///
    /// # Example
    /// ```rust,ignore
    /// let dataset = client.get_dataset(5500).await?;
    /// ```
    pub async fn get_dataset(&self, recid: u64) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/{}", self.base_url, recid);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let record: CernRecord = response.json().await?;

        self.convert_records(vec![record])
    }

    /// Search datasets by experiment
    ///
    /// # Arguments
    /// * `experiment` - Experiment name: "CMS", "ATLAS", "LHCb", "ALICE"
    ///
    /// # Example
    /// ```rust,ignore
    /// let cms_data = client.search_by_experiment("CMS").await?;
    /// ```
    pub async fn search_by_experiment(&self, experiment: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}?experiment={}&size=50",
            self.base_url,
            urlencoding::encode(experiment)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: CernSearchResponse = response.json().await?;

        self.convert_records(search_response.hits.hits)
    }

    /// Convert CERN records to SemanticVectors
    fn convert_records(&self, records: Vec<CernRecord>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for record in records {
            let title = record
                .metadata
                .titles
                .first()
                .map(|t| t.title.clone())
                .unwrap_or_else(|| format!("Dataset {}", record.id));

            let description = record
                .metadata
                .r#abstract
                .as_ref()
                .map(|a| a.description.clone())
                .unwrap_or_default();

            let experiment = record.metadata.experiment.unwrap_or_default();

            let collision_energy = record
                .metadata
                .collision_information
                .as_ref()
                .map(|c| c.energy.clone())
                .unwrap_or_default();

            let collision_type = record
                .metadata
                .collision_information
                .as_ref()
                .map(|c| c.r#type.clone())
                .unwrap_or_default();

            // Create text for embedding
            let text = format!(
                "{} {} {} {} {}",
                title,
                description,
                experiment,
                collision_energy,
                collision_type
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("recid".to_string(), record.id.to_string());
            metadata.insert("title".to_string(), title);
            metadata.insert("experiment".to_string(), experiment);
            metadata.insert("collision_energy".to_string(), collision_energy);
            metadata.insert("collision_type".to_string(), collision_type);
            metadata.insert("data_type".to_string(), record.metadata.r#type.primary);
            metadata.insert("source".to_string(), "cern".to_string());

            let date = record
                .metadata
                .date_created
                .first()
                .and_then(|d| NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M:%S").ok())
                .or_else(|| {
                    record
                        .metadata
                        .date_created
                        .first()
                        .and_then(|d| NaiveDateTime::parse_from_str(d, "%Y").ok())
                })
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("CERN:{}", record.id),
                embedding,
                domain: Domain::Physics,
                timestamp: date,
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

impl Default for CernOpenDataClient {
    fn default() -> Self {
        Self::new().expect("Failed to create CERN client")
    }
}

// ============================================================================
// Argo Float Ocean Data Client
// ============================================================================

/// Argo profile data (simplified structure)
#[derive(Debug, Deserialize)]
struct ArgoProfile {
    #[serde(default)]
    platform_number: String,
    #[serde(default)]
    cycle_number: u32,
    #[serde(default)]
    latitude: f64,
    #[serde(default)]
    longitude: f64,
    #[serde(default)]
    juld: f64, // Julian date
    #[serde(default)]
    pres: Vec<f64>, // Pressure levels
    #[serde(default)]
    temp: Vec<f64>, // Temperature
    #[serde(default)]
    psal: Vec<f64>, // Practical salinity
}

/// Client for Argo Float Ocean Data
///
/// Provides access to:
/// - Ocean temperature profiles
/// - Salinity measurements
/// - Pressure/depth data
/// - Global ocean coverage
///
/// Note: This client uses a simplified Argo data access pattern.
/// For production use, consider using dedicated Argo APIs or netCDF data.
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::ArgoClient;
///
/// let client = ArgoClient::new()?;
/// let recent = client.get_recent_profiles(30).await?;
/// let regional = client.search_by_region(0.0, -30.0, 500.0).await?;
/// let temp_profiles = client.get_temperature_profiles().await?;
/// ```
pub struct ArgoClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl ArgoClient {
    /// Create a new Argo client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            // Note: Using Ifremer Argo GDAC as base URL
            base_url: "https://data-argo.ifremer.fr".to_string(),
            rate_limit_delay: Duration::from_millis(ARGO_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get recent ocean profiles
    ///
    /// # Arguments
    /// * `days` - Number of days to look back
    ///
    /// # Example
    /// ```rust,ignore
    /// let recent = client.get_recent_profiles(7).await?;
    /// ```
    pub async fn get_recent_profiles(&self, _days: u32) -> Result<Vec<SemanticVector>> {
        // This is a placeholder implementation
        // In production, you would fetch from Argo GDAC index files or use ArgoVis API

        // For demonstration, return empty vec with a note
        // Real implementation would parse Argo profile files
        Ok(Vec::new())
    }

    /// Search profiles by geographic region
    ///
    /// # Arguments
    /// * `lat` - Center latitude
    /// * `lon` - Center longitude
    /// * `radius_km` - Search radius in kilometers
    ///
    /// # Example
    /// ```rust,ignore
    /// // Search in Atlantic Ocean
    /// let atlantic = client.search_by_region(0.0, -30.0, 500.0).await?;
    /// ```
    pub async fn search_by_region(
        &self,
        _lat: f64,
        _lon: f64,
        _radius_km: f64,
    ) -> Result<Vec<SemanticVector>> {
        // Placeholder implementation
        // Real implementation would use Argo spatial index
        Ok(Vec::new())
    }

    /// Get ocean temperature profiles
    ///
    /// # Example
    /// ```rust,ignore
    /// let temp_data = client.get_temperature_profiles().await?;
    /// ```
    pub async fn get_temperature_profiles(&self) -> Result<Vec<SemanticVector>> {
        // Placeholder implementation
        // Real implementation would filter for temperature-focused profiles
        Ok(Vec::new())
    }

    /// Create sample Argo data for testing/demonstration
    ///
    /// This generates synthetic ocean profile data
    pub fn create_sample_profiles(&self, count: usize) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for i in 0..count {
            let lat = -60.0 + (120.0 * (i as f64 / count as f64));
            let lon = -180.0 + (360.0 * ((i * 7) % count) as f64 / count as f64);
            let temp = 5.0 + (15.0 * (lat.abs() / 90.0));
            let salinity = 34.0 + (2.0 * (lat / 90.0));
            let depth = 100.0 * (i % 20) as f64;

            let text = format!(
                "Ocean profile at lat {} lon {}: temp {}°C, salinity {}, depth {}m",
                lat, lon, temp, salinity, depth
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("platform_number".to_string(), format!("{}", 1900000 + i));
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("temperature".to_string(), temp.to_string());
            metadata.insert("salinity".to_string(), salinity.to_string());
            metadata.insert("depth_m".to_string(), depth.to_string());
            metadata.insert("source".to_string(), "argo".to_string());

            vectors.push(SemanticVector {
                id: format!("ARGO:{}", 1900000 + i),
                embedding,
                domain: Domain::Ocean,
                timestamp: Utc::now() - chrono::Duration::days(i as i64 % 30),
                metadata,
            });
        }

        Ok(vectors)
    }
}

impl Default for ArgoClient {
    fn default() -> Self {
        Self::new().expect("Failed to create Argo client")
    }
}

// ============================================================================
// Materials Project Client
// ============================================================================

/// Materials Project material data
#[derive(Debug, Deserialize)]
struct MaterialsProjectMaterial {
    material_id: String,
    #[serde(default)]
    formula_pretty: String,
    #[serde(default)]
    band_gap: Option<f64>,
    #[serde(default)]
    density: Option<f64>,
    #[serde(default)]
    formation_energy_per_atom: Option<f64>,
    #[serde(default)]
    energy_per_atom: Option<f64>,
    #[serde(default)]
    volume: Option<f64>,
    #[serde(default)]
    nsites: Option<u32>,
    #[serde(default)]
    elements: Vec<String>,
    #[serde(default)]
    nelements: Option<u32>,
    #[serde(default)]
    crystal_system: Option<String>,
    #[serde(default)]
    symmetry: Option<MaterialsSymmetry>,
}

#[derive(Debug, Deserialize)]
struct MaterialsSymmetry {
    #[serde(default)]
    crystal_system: String,
    #[serde(default)]
    symbol: String,
}

/// Materials Project API response
#[derive(Debug, Deserialize)]
struct MaterialsProjectResponse {
    #[serde(default)]
    data: Vec<MaterialsProjectMaterial>,
}

/// Client for Materials Project
///
/// Provides access to:
/// - Computational materials science database
/// - Crystal structures and properties
/// - Band gaps, formation energies
/// - Electronic and mechanical properties
///
/// **Note**: Requires API key from https://materialsproject.org
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::MaterialsProjectClient;
///
/// let client = MaterialsProjectClient::new("YOUR_API_KEY".to_string())?;
/// let silicon = client.search_materials("Si").await?;
/// let material = client.get_material("mp-149").await?;
/// let semiconductors = client.search_by_property("band_gap", 1.0, 3.0).await?;
/// ```
pub struct MaterialsProjectClient {
    client: Client,
    base_url: String,
    api_key: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl MaterialsProjectClient {
    /// Create a new Materials Project client
    ///
    /// # Arguments
    /// * `api_key` - Materials Project API key (get from https://materialsproject.org)
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.materialsproject.org".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(MATERIALS_PROJECT_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Search materials by chemical formula
    ///
    /// # Arguments
    /// * `formula` - Chemical formula (e.g., "Si", "Fe2O3", "LiFePO4")
    ///
    /// # Example
    /// ```rust,ignore
    /// let silicon = client.search_materials("Si").await?;
    /// let iron_oxide = client.search_materials("Fe2O3").await?;
    /// ```
    pub async fn search_materials(&self, formula: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/materials/summary/?formula={}",
            self.base_url,
            urlencoding::encode(formula)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.client
            .get(&url)
            .header("X-API-KEY", &self.api_key)
            .send()
            .await?;

        let mp_response: MaterialsProjectResponse = response.json().await?;
        self.convert_materials(mp_response.data)
    }

    /// Get a specific material by Materials Project ID
    ///
    /// # Arguments
    /// * `material_id` - Materials Project ID (e.g., "mp-149" for silicon)
    ///
    /// # Example
    /// ```rust,ignore
    /// let silicon = client.get_material("mp-149").await?;
    /// ```
    pub async fn get_material(&self, material_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/materials/{}/",
            self.base_url, material_id
        );

        sleep(self.rate_limit_delay).await;
        let response = self.client
            .get(&url)
            .header("X-API-KEY", &self.api_key)
            .send()
            .await?;

        let material: MaterialsProjectMaterial = response.json().await?;
        self.convert_materials(vec![material])
    }

    /// Search materials by property range
    ///
    /// # Arguments
    /// * `property` - Property name (e.g., "band_gap", "formation_energy_per_atom")
    /// * `min` - Minimum value
    /// * `max` - Maximum value
    ///
    /// # Example
    /// ```rust,ignore
    /// // Find semiconductors with band gap 1-3 eV
    /// let semiconductors = client.search_by_property("band_gap", 1.0, 3.0).await?;
    /// ```
    pub async fn search_by_property(
        &self,
        property: &str,
        min: f64,
        max: f64,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/materials/summary/?{}_min={}&{}_max={}",
            self.base_url, property, min, property, max
        );

        sleep(self.rate_limit_delay).await;
        let response = self.client
            .get(&url)
            .header("X-API-KEY", &self.api_key)
            .send()
            .await?;

        let mp_response: MaterialsProjectResponse = response.json().await?;
        self.convert_materials(mp_response.data)
    }

    /// Convert Materials Project materials to SemanticVectors
    fn convert_materials(&self, materials: Vec<MaterialsProjectMaterial>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for material in materials {
            let band_gap = material.band_gap.unwrap_or(0.0);
            let density = material.density.unwrap_or(0.0);
            let formation_energy = material.formation_energy_per_atom.unwrap_or(0.0);
            let crystal_system = material
                .crystal_system
                .or_else(|| material.symmetry.as_ref().map(|s| s.crystal_system.clone()))
                .unwrap_or_default();

            // Create text for embedding
            let text = format!(
                "{} {} crystal system, band gap {} eV, density {} g/cm³, formation energy {} eV/atom",
                material.formula_pretty, crystal_system, band_gap, density, formation_energy
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("material_id".to_string(), material.material_id.clone());
            metadata.insert("formula".to_string(), material.formula_pretty);
            metadata.insert("band_gap".to_string(), band_gap.to_string());
            metadata.insert("density".to_string(), density.to_string());
            metadata.insert("formation_energy".to_string(), formation_energy.to_string());
            metadata.insert("crystal_system".to_string(), crystal_system);
            metadata.insert("elements".to_string(), material.elements.join(","));
            metadata.insert("source".to_string(), "materials_project".to_string());

            vectors.push(SemanticVector {
                id: format!("MP:{}", material.material_id),
                embedding,
                domain: Domain::Physics,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geo_utils_distance() {
        // Distance from NYC to LA (approximately 3936 km)
        let dist = GeoUtils::distance_km(40.7128, -74.0060, 34.0522, -118.2437);
        assert!((dist - 3936.0).abs() < 100.0); // Within 100km tolerance
    }

    #[test]
    fn test_geo_utils_within_radius() {
        let center_lat = 34.05;
        let center_lon = -118.25;

        // Point 50km away should be within 100km radius
        let nearby = GeoUtils::within_radius(center_lat, center_lon, 34.5, -118.25, 100.0);
        assert!(nearby);

        // Point far away should not be within 10km radius
        let far = GeoUtils::within_radius(center_lat, center_lon, 40.7, -74.0, 10.0);
        assert!(!far);
    }

    #[tokio::test]
    async fn test_usgs_client_creation() {
        let client = UsgsEarthquakeClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_cern_client_creation() {
        let client = CernOpenDataClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_argo_client_creation() {
        let client = ArgoClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_materials_project_client_creation() {
        let client = MaterialsProjectClient::new("test_key".to_string());
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_argo_sample_profiles() {
        let client = ArgoClient::new().unwrap();
        let profiles = client.create_sample_profiles(10);
        assert!(profiles.is_ok());
        let vectors = profiles.unwrap();
        assert_eq!(vectors.len(), 10);
        assert_eq!(vectors[0].domain, Domain::Ocean);
    }

    #[test]
    fn test_rate_limiting() {
        let usgs = UsgsEarthquakeClient::new().unwrap();
        assert_eq!(usgs.rate_limit_delay, Duration::from_millis(USGS_RATE_LIMIT_MS));

        let cern = CernOpenDataClient::new().unwrap();
        assert_eq!(cern.rate_limit_delay, Duration::from_millis(CERN_RATE_LIMIT_MS));

        let argo = ArgoClient::new().unwrap();
        assert_eq!(argo.rate_limit_delay, Duration::from_millis(ARGO_RATE_LIMIT_MS));

        let mp = MaterialsProjectClient::new("test".to_string()).unwrap();
        assert_eq!(mp.rate_limit_delay, Duration::from_millis(MATERIALS_PROJECT_RATE_LIMIT_MS));
    }
}
