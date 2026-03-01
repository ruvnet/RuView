//! NASA and space data API integrations
//!
//! This module provides async clients for fetching space and astronomy data from:
//! - NASA Open APIs (APOD, NEO, Mars weather, DONKI)
//! - NASA Exoplanet Archive
//! - SpaceX API
//! - Open Astronomy Catalogs
//!
//! All responses are converted to SemanticVector format for RuVector discovery.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const NASA_RATE_LIMIT_MS: u64 = 100; // ~10 requests/second
const SPACEX_RATE_LIMIT_MS: u64 = 100; // Conservative rate
const ASTRONOMY_RATE_LIMIT_MS: u64 = 200; // Conservative rate
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// NASA Open APIs Client
// ============================================================================

/// NASA APOD (Astronomy Picture of the Day) response
#[derive(Debug, Deserialize)]
struct ApodResponse {
    #[serde(default)]
    date: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    explanation: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    media_type: String,
    #[serde(default)]
    copyright: String,
}

/// NASA NEO (Near Earth Objects) response
#[derive(Debug, Deserialize)]
struct NeoResponse {
    #[serde(default)]
    near_earth_objects: HashMap<String, Vec<NeoObject>>,
}

#[derive(Debug, Deserialize)]
struct NeoObject {
    id: String,
    name: String,
    #[serde(default)]
    nasa_jpl_url: String,
    #[serde(default)]
    absolute_magnitude_h: f64,
    #[serde(default)]
    is_potentially_hazardous_asteroid: bool,
    #[serde(default)]
    close_approach_data: Vec<CloseApproachData>,
}

#[derive(Debug, Deserialize)]
struct CloseApproachData {
    #[serde(default)]
    close_approach_date: String,
    #[serde(default)]
    relative_velocity: HashMap<String, String>,
    #[serde(default)]
    miss_distance: HashMap<String, String>,
}

/// Mars Rover photos response
#[derive(Debug, Deserialize)]
struct MarsPhotosResponse {
    #[serde(default)]
    photos: Vec<MarsPhoto>,
}

#[derive(Debug, Deserialize)]
struct MarsPhoto {
    id: u64,
    #[serde(default)]
    sol: u32,
    #[serde(default)]
    img_src: String,
    #[serde(default)]
    earth_date: String,
    #[serde(default)]
    camera: MarsCamera,
    #[serde(default)]
    rover: MarsRover,
}

#[derive(Debug, Deserialize, Default)]
struct MarsCamera {
    #[serde(default)]
    name: String,
    #[serde(default)]
    full_name: String,
}

#[derive(Debug, Deserialize, Default)]
struct MarsRover {
    #[serde(default)]
    name: String,
    #[serde(default)]
    status: String,
}

/// DONKI (Space Weather Database Of Notifications, Knowledge, Information) events
#[derive(Debug, Deserialize)]
struct DonkiEvent {
    #[serde(default)]
    #[serde(rename = "activityID")]
    activity_id: String,
    #[serde(default)]
    #[serde(rename = "startTime")]
    start_time: String,
    #[serde(default)]
    #[serde(rename = "classType")]
    class_type: String,
    #[serde(default)]
    #[serde(rename = "sourceLocation")]
    source_location: String,
    #[serde(default)]
    note: String,
}

/// Client for NASA Open APIs (api.nasa.gov)
///
/// Provides access to:
/// - Astronomy Picture of the Day (APOD)
/// - Near Earth Objects (NEO) - asteroids
/// - Mars weather and rover photos
/// - Space weather events (DONKI)
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::NasaClient;
///
/// let client = NasaClient::new(Some("YOUR_API_KEY".to_string()))?;
/// let apod = client.get_apod(None).await?;
/// let asteroids = client.search_neo("2024-01-01", "2024-01-07").await?;
/// let mars_photos = client.search_mars_photos(1000, Some("NAVCAM")).await?;
/// ```
pub struct NasaClient {
    client: Client,
    base_url: String,
    api_key: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl NasaClient {
    /// Create a new NASA client
    ///
    /// # Arguments
    /// * `api_key` - Optional NASA API key (get from https://api.nasa.gov/)
    ///               If None, uses "DEMO_KEY" (limited to 30 requests per hour)
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ruvector-data-framework/1.0")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.nasa.gov".to_string(),
            api_key: api_key.unwrap_or_else(|| "DEMO_KEY".to_string()),
            rate_limit_delay: Duration::from_millis(NASA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Get Astronomy Picture of the Day
    ///
    /// # Arguments
    /// * `date` - Optional date in format "YYYY-MM-DD". If None, returns today's APOD
    ///
    /// # Example
    /// ```rust,ignore
    /// let today = client.get_apod(None).await?;
    /// let specific = client.get_apod(Some("2024-01-01")).await?;
    /// ```
    pub async fn get_apod(&self, date: Option<&str>) -> Result<Vec<SemanticVector>> {
        let mut url = format!("{}/planetary/apod?api_key={}", self.base_url, self.api_key);

        if let Some(d) = date {
            url.push_str(&format!("&date={}", d));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let apod: ApodResponse = response.json().await?;

        // Create text for embedding
        let text = format!(
            "Astronomy Picture of the Day {}: {} - {}",
            apod.date, apod.title, apod.explanation
        );
        let embedding = self.embedder.embed_text(&text);

        // Parse date
        let timestamp = NaiveDate::parse_from_str(&apod.date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), apod.title);
        metadata.insert("date".to_string(), apod.date.clone());
        metadata.insert("media_type".to_string(), apod.media_type);
        metadata.insert("url".to_string(), apod.url);
        metadata.insert("copyright".to_string(), apod.copyright);
        metadata.insert("source".to_string(), "nasa_apod".to_string());

        Ok(vec![SemanticVector {
            id: format!("NASA:APOD:{}", apod.date),
            embedding,
            domain: Domain::Space,
            timestamp,
            metadata,
        }])
    }

    /// Search for Near Earth Objects (asteroids) within a date range
    ///
    /// # Arguments
    /// * `start_date` - Start date in format "YYYY-MM-DD"
    /// * `end_date` - End date in format "YYYY-MM-DD" (max 7 days from start)
    ///
    /// # Example
    /// ```rust,ignore
    /// let asteroids = client.search_neo("2024-01-01", "2024-01-07").await?;
    /// ```
    pub async fn search_neo(&self, start_date: &str, end_date: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/neo/rest/v1/feed?start_date={}&end_date={}&api_key={}",
            self.base_url, start_date, end_date, self.api_key
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let neo_response: NeoResponse = response.json().await?;

        let mut vectors = Vec::new();

        for (date, objects) in neo_response.near_earth_objects {
            for obj in objects {
                // Get close approach details
                let approach = obj.close_approach_data.first();
                let velocity = approach
                    .and_then(|a| a.relative_velocity.get("kilometers_per_hour"))
                    .map(|v| v.as_str())
                    .unwrap_or("unknown");
                let miss_distance = approach
                    .and_then(|a| a.miss_distance.get("kilometers"))
                    .map(|d| d.as_str())
                    .unwrap_or("unknown");

                // Create text for embedding
                let text = format!(
                    "Near Earth Object {}: magnitude {:.2}, potentially hazardous: {}, velocity {} km/h, miss distance {} km",
                    obj.name,
                    obj.absolute_magnitude_h,
                    obj.is_potentially_hazardous_asteroid,
                    velocity,
                    miss_distance
                );
                let embedding = self.embedder.embed_text(&text);

                // Parse date
                let timestamp = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(Utc::now);

                let mut metadata = HashMap::new();
                metadata.insert("neo_id".to_string(), obj.id.clone());
                metadata.insert("name".to_string(), obj.name.clone());
                metadata.insert("date".to_string(), date.clone());
                metadata.insert("magnitude".to_string(), obj.absolute_magnitude_h.to_string());
                metadata.insert("hazardous".to_string(), obj.is_potentially_hazardous_asteroid.to_string());
                metadata.insert("velocity_kph".to_string(), velocity.to_string());
                metadata.insert("miss_distance_km".to_string(), miss_distance.to_string());
                metadata.insert("source".to_string(), "nasa_neo".to_string());

                vectors.push(SemanticVector {
                    id: format!("NASA:NEO:{}:{}", obj.id, date),
                    embedding,
                    domain: Domain::Space,
                    timestamp,
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get Mars weather data (note: InSight mission ended, returns historical data)
    ///
    /// # Example
    /// ```rust,ignore
    /// let weather = client.get_mars_weather().await?;
    /// ```
    pub async fn get_mars_weather(&self) -> Result<Vec<SemanticVector>> {
        // Note: InSight mission ended in Dec 2022, this endpoint may return limited data
        let url = format!("{}/insight_weather/?api_key={}&feedtype=json&ver=1.0",
            self.base_url, self.api_key);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        // Return empty vec as InSight mission has ended
        // In a production system, you might want to return historical data
        Ok(Vec::new())
    }

    /// Search Mars Rover photos
    ///
    /// # Arguments
    /// * `sol` - Martian day (sol) number
    /// * `camera` - Optional camera name (FHAZ, RHAZ, MAST, CHEMCAM, MAHLI, MARDI, NAVCAM, PANCAM, MINITES)
    ///
    /// # Example
    /// ```rust,ignore
    /// let photos = client.search_mars_photos(1000, Some("NAVCAM")).await?;
    /// let all_cameras = client.search_mars_photos(1000, None).await?;
    /// ```
    pub async fn search_mars_photos(&self, sol: u32, camera: Option<&str>) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/mars-photos/api/v1/rovers/curiosity/photos?sol={}&api_key={}",
            self.base_url, sol, self.api_key
        );

        if let Some(cam) = camera {
            url.push_str(&format!("&camera={}", cam));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let photos_response: MarsPhotosResponse = response.json().await?;

        let mut vectors = Vec::new();

        for photo in photos_response.photos.iter().take(50) {
            // Create text for embedding
            let text = format!(
                "Mars rover {} photo from {} camera on sol {} ({})",
                photo.rover.name, photo.camera.full_name, photo.sol, photo.earth_date
            );
            let embedding = self.embedder.embed_text(&text);

            // Parse date
            let timestamp = NaiveDate::parse_from_str(&photo.earth_date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let mut metadata = HashMap::new();
            metadata.insert("photo_id".to_string(), photo.id.to_string());
            metadata.insert("sol".to_string(), photo.sol.to_string());
            metadata.insert("camera".to_string(), photo.camera.name.clone());
            metadata.insert("camera_full_name".to_string(), photo.camera.full_name.clone());
            metadata.insert("rover".to_string(), photo.rover.name.clone());
            metadata.insert("rover_status".to_string(), photo.rover.status.clone());
            metadata.insert("earth_date".to_string(), photo.earth_date.clone());
            metadata.insert("img_src".to_string(), photo.img_src.clone());
            metadata.insert("source".to_string(), "nasa_mars_photos".to_string());

            vectors.push(SemanticVector {
                id: format!("NASA:MARS:{}:{}", photo.id, photo.sol),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get space weather events from DONKI (Database Of Notifications, Knowledge, Information)
    ///
    /// # Arguments
    /// * `event_type` - Event type: "CME" (Coronal Mass Ejection), "FLR" (Solar Flare), "SEP" (Solar Energetic Particle), etc.
    /// * `start_date` - Start date in format "YYYY-MM-DD"
    /// * `end_date` - End date in format "YYYY-MM-DD" (max 30 days)
    ///
    /// # Example
    /// ```rust,ignore
    /// let flares = client.get_donki_events("FLR", "2024-01-01", "2024-01-31").await?;
    /// let cmes = client.get_donki_events("CME", "2024-01-01", "2024-01-31").await?;
    /// ```
    pub async fn get_donki_events(
        &self,
        event_type: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/DONKI/{}?startDate={}&endDate={}&api_key={}",
            self.base_url, event_type, start_date, end_date, self.api_key
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let events: Vec<DonkiEvent> = response.json().await?;

        let mut vectors = Vec::new();

        for event in events {
            // Create text for embedding
            let text = format!(
                "Space weather event {}: {} at {} - {}",
                event_type, event.activity_id, event.source_location, event.note
            );
            let embedding = self.embedder.embed_text(&text);

            // Parse timestamp
            let timestamp = chrono::DateTime::parse_from_rfc3339(&event.start_time)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let mut metadata = HashMap::new();
            metadata.insert("activity_id".to_string(), event.activity_id.clone());
            metadata.insert("event_type".to_string(), event_type.to_string());
            metadata.insert("start_time".to_string(), event.start_time.clone());
            metadata.insert("class_type".to_string(), event.class_type);
            metadata.insert("source_location".to_string(), event.source_location);
            metadata.insert("note".to_string(), event.note);
            metadata.insert("source".to_string(), "nasa_donki".to_string());

            vectors.push(SemanticVector {
                id: format!("NASA:DONKI:{}:{}", event_type, event.activity_id),
                embedding,
                domain: Domain::Space,
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

// ============================================================================
// NASA Exoplanet Archive Client
// ============================================================================

/// Exoplanet data from NASA Exoplanet Archive
#[derive(Debug, Deserialize)]
struct ExoplanetData {
    #[serde(default)]
    pl_name: String,
    #[serde(default)]
    hostname: String,
    #[serde(default)]
    discoverymethod: String,
    #[serde(default)]
    disc_year: Option<i32>,
    #[serde(default)]
    pl_orbper: Option<f64>, // Orbital period (days)
    #[serde(default)]
    pl_rade: Option<f64>, // Planet radius (Earth radii)
    #[serde(default)]
    pl_masse: Option<f64>, // Planet mass (Earth masses)
    #[serde(default)]
    pl_eqt: Option<f64>, // Equilibrium temperature (K)
    #[serde(default)]
    sy_dist: Option<f64>, // Distance from Earth (parsecs)
}

/// Client for NASA Exoplanet Archive
///
/// Provides access to confirmed exoplanets and their properties:
/// - Planetary mass, radius, orbital period
/// - Discovery method (transit, radial velocity, imaging, etc.)
/// - Habitable zone candidates
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::ExoplanetClient;
///
/// let client = ExoplanetClient::new()?;
/// let all = client.search_exoplanets(None).await?;
/// let habitable = client.get_habitable_zone().await?;
/// let transit = client.get_by_discovery_method("Transit").await?;
/// ```
pub struct ExoplanetClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl ExoplanetClient {
    /// Create a new Exoplanet Archive client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ruvector-data-framework/1.0")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://exoplanetarchive.ipac.caltech.edu/TAP/sync".to_string(),
            rate_limit_delay: Duration::from_millis(NASA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search for exoplanets with optional query
    ///
    /// # Arguments
    /// * `query` - Optional WHERE clause (e.g., "pl_rade>2" for super-Earths)
    ///
    /// # Example
    /// ```rust,ignore
    /// let all = client.search_exoplanets(None).await?;
    /// let large = client.search_exoplanets(Some("pl_rade>10")).await?;
    /// ```
    pub async fn search_exoplanets(&self, query: Option<&str>) -> Result<Vec<SemanticVector>> {
        let base_query = "SELECT pl_name,hostname,discoverymethod,disc_year,pl_orbper,pl_rade,pl_masse,pl_eqt,sy_dist FROM ps";
        let full_query = if let Some(q) = query {
            format!("{} WHERE {}", base_query, q)
        } else {
            base_query.to_string()
        };

        let url = format!(
            "{}?query={}&format=json",
            self.base_url,
            urlencoding::encode(&full_query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let exoplanets: Vec<ExoplanetData> = response.json().await?;

        let mut vectors = Vec::new();

        for (idx, planet) in exoplanets.iter().take(100).enumerate() {
            // Create text for embedding
            let text = format!(
                "Exoplanet {} orbiting {}, discovered via {} in {:?}, radius {:.2}R⊕, mass {:.2}M⊕, temp {:.0}K",
                planet.pl_name,
                planet.hostname,
                planet.discoverymethod,
                planet.disc_year,
                planet.pl_rade.unwrap_or(0.0),
                planet.pl_masse.unwrap_or(0.0),
                planet.pl_eqt.unwrap_or(0.0)
            );
            let embedding = self.embedder.embed_text(&text);

            // Use discovery year for timestamp
            let timestamp = planet.disc_year
                .and_then(|y| NaiveDate::from_ymd_opt(y, 1, 1))
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let mut metadata = HashMap::new();
            metadata.insert("planet_name".to_string(), planet.pl_name.clone());
            metadata.insert("host_star".to_string(), planet.hostname.clone());
            metadata.insert("discovery_method".to_string(), planet.discoverymethod.clone());
            metadata.insert("discovery_year".to_string(), planet.disc_year.map(|y| y.to_string()).unwrap_or_default());
            metadata.insert("orbital_period_days".to_string(), planet.pl_orbper.map(|p| p.to_string()).unwrap_or_default());
            metadata.insert("radius_earth".to_string(), planet.pl_rade.map(|r| r.to_string()).unwrap_or_default());
            metadata.insert("mass_earth".to_string(), planet.pl_masse.map(|m| m.to_string()).unwrap_or_default());
            metadata.insert("temperature_k".to_string(), planet.pl_eqt.map(|t| t.to_string()).unwrap_or_default());
            metadata.insert("distance_parsecs".to_string(), planet.sy_dist.map(|d| d.to_string()).unwrap_or_default());
            metadata.insert("source".to_string(), "nasa_exoplanet_archive".to_string());

            vectors.push(SemanticVector {
                id: format!("EXOPLANET:{}:{}", planet.pl_name, idx),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get planets in the habitable zone (potentially life-supporting temperatures)
    ///
    /// # Example
    /// ```rust,ignore
    /// let habitable = client.get_habitable_zone().await?;
    /// ```
    pub async fn get_habitable_zone(&self) -> Result<Vec<SemanticVector>> {
        // Habitable zone: temperature between 200-350K (conservative estimate)
        self.search_exoplanets(Some("pl_eqt>200 and pl_eqt<350")).await
    }

    /// Get planets discovered by a specific method
    ///
    /// # Arguments
    /// * `method` - Discovery method: "Transit", "Radial Velocity", "Imaging", "Microlensing", etc.
    ///
    /// # Example
    /// ```rust,ignore
    /// let transit = client.get_by_discovery_method("Transit").await?;
    /// let imaging = client.get_by_discovery_method("Imaging").await?;
    /// ```
    pub async fn get_by_discovery_method(&self, method: &str) -> Result<Vec<SemanticVector>> {
        let query = format!("discoverymethod='{}'", method);
        self.search_exoplanets(Some(&query)).await
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

impl Default for ExoplanetClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ExoplanetClient")
    }
}

// ============================================================================
// SpaceX API Client
// ============================================================================

/// SpaceX launch data
#[derive(Debug, Deserialize)]
struct SpaceXLaunch {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    date_utc: String,
    #[serde(default)]
    success: Option<bool>,
    #[serde(default)]
    details: Option<String>,
    #[serde(default)]
    flight_number: u32,
    #[serde(default)]
    rocket: String,
    #[serde(default)]
    launchpad: String,
}

/// SpaceX rocket data
#[derive(Debug, Deserialize)]
struct SpaceXRocket {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    height: Option<SpaceXDimension>,
    #[serde(default)]
    mass: Option<SpaceXMass>,
    #[serde(default)]
    first_flight: String,
    #[serde(default)]
    success_rate_pct: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SpaceXDimension {
    meters: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct SpaceXMass {
    kg: Option<f64>,
}

/// SpaceX Starlink satellite data
#[derive(Debug, Deserialize)]
struct StarlinkSatellite {
    #[serde(default)]
    id: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    launch: String,
    #[serde(default)]
    longitude: Option<f64>,
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    height_km: Option<f64>,
}

/// Client for SpaceX API (api.spacexdata.com)
///
/// Provides access to:
/// - Launch history and upcoming launches
/// - Rocket specifications
/// - Starlink satellite constellation
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::SpaceXClient;
///
/// let client = SpaceXClient::new()?;
/// let launches = client.get_launches(Some(50)).await?;
/// let upcoming = client.get_upcoming_launches().await?;
/// let rockets = client.get_rockets().await?;
/// let starlink = client.get_starlink_satellites().await?;
/// ```
pub struct SpaceXClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl SpaceXClient {
    /// Create a new SpaceX client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ruvector-data-framework/1.0")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.spacexdata.com/v4".to_string(),
            rate_limit_delay: Duration::from_millis(SPACEX_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Get launch history
    ///
    /// # Arguments
    /// * `limit` - Optional limit on number of launches to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let launches = client.get_launches(Some(50)).await?;
    /// ```
    pub async fn get_launches(&self, limit: Option<usize>) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/launches", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let launches: Vec<SpaceXLaunch> = response.json().await?;

        let mut vectors = Vec::new();
        let launches_to_process = if let Some(lim) = limit {
            &launches[..launches.len().min(lim)]
        } else {
            &launches
        };

        for launch in launches_to_process {
            // Create text for embedding
            let success_str = match launch.success {
                Some(true) => "successful",
                Some(false) => "failed",
                None => "pending",
            };
            let details = launch.details.as_deref().unwrap_or("No details");

            let text = format!(
                "SpaceX launch {} (flight #{}): {} - {}",
                launch.name, launch.flight_number, success_str, details
            );
            let embedding = self.embedder.embed_text(&text);

            // Parse timestamp
            let timestamp = chrono::DateTime::parse_from_rfc3339(&launch.date_utc)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let mut metadata = HashMap::new();
            metadata.insert("launch_id".to_string(), launch.id.clone());
            metadata.insert("name".to_string(), launch.name.clone());
            metadata.insert("flight_number".to_string(), launch.flight_number.to_string());
            metadata.insert("date".to_string(), launch.date_utc.clone());
            metadata.insert("success".to_string(), launch.success.map(|s| s.to_string()).unwrap_or_default());
            metadata.insert("rocket_id".to_string(), launch.rocket.clone());
            metadata.insert("launchpad".to_string(), launch.launchpad.clone());
            metadata.insert("source".to_string(), "spacex_launches".to_string());

            vectors.push(SemanticVector {
                id: format!("SPACEX:LAUNCH:{}", launch.id),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get upcoming launches
    ///
    /// # Example
    /// ```rust,ignore
    /// let upcoming = client.get_upcoming_launches().await?;
    /// ```
    pub async fn get_upcoming_launches(&self) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/launches/upcoming", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let launches: Vec<SpaceXLaunch> = response.json().await?;

        let mut vectors = Vec::new();

        for launch in launches.iter().take(20) {
            let details = launch.details.as_deref().unwrap_or("No details");

            let text = format!(
                "Upcoming SpaceX launch {} (flight #{}): {}",
                launch.name, launch.flight_number, details
            );
            let embedding = self.embedder.embed_text(&text);

            let timestamp = chrono::DateTime::parse_from_rfc3339(&launch.date_utc)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now());

            let mut metadata = HashMap::new();
            metadata.insert("launch_id".to_string(), launch.id.clone());
            metadata.insert("name".to_string(), launch.name.clone());
            metadata.insert("flight_number".to_string(), launch.flight_number.to_string());
            metadata.insert("date".to_string(), launch.date_utc.clone());
            metadata.insert("rocket_id".to_string(), launch.rocket.clone());
            metadata.insert("status".to_string(), "upcoming".to_string());
            metadata.insert("source".to_string(), "spacex_upcoming".to_string());

            vectors.push(SemanticVector {
                id: format!("SPACEX:UPCOMING:{}", launch.id),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get rocket specifications
    ///
    /// # Example
    /// ```rust,ignore
    /// let rockets = client.get_rockets().await?;
    /// ```
    pub async fn get_rockets(&self) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/rockets", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let rockets: Vec<SpaceXRocket> = response.json().await?;

        let mut vectors = Vec::new();

        for rocket in rockets {
            let height = rocket.height
                .as_ref()
                .and_then(|h| h.meters)
                .unwrap_or(0.0);
            let mass = rocket.mass
                .as_ref()
                .and_then(|m| m.kg)
                .unwrap_or(0.0);
            let success_rate = rocket.success_rate_pct.unwrap_or(0.0);

            let text = format!(
                "SpaceX rocket {}: {} - height {:.1}m, mass {:.0}kg, {:.1}% success rate, first flight {}",
                rocket.name, rocket.description, height, mass, success_rate, rocket.first_flight
            );
            let embedding = self.embedder.embed_text(&text);

            // Use first flight date for timestamp
            let timestamp = NaiveDate::parse_from_str(&rocket.first_flight, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let mut metadata = HashMap::new();
            metadata.insert("rocket_id".to_string(), rocket.id.clone());
            metadata.insert("name".to_string(), rocket.name.clone());
            metadata.insert("description".to_string(), rocket.description);
            metadata.insert("height_meters".to_string(), height.to_string());
            metadata.insert("mass_kg".to_string(), mass.to_string());
            metadata.insert("first_flight".to_string(), rocket.first_flight);
            metadata.insert("success_rate_pct".to_string(), success_rate.to_string());
            metadata.insert("source".to_string(), "spacex_rockets".to_string());

            vectors.push(SemanticVector {
                id: format!("SPACEX:ROCKET:{}", rocket.id),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get Starlink satellite constellation data
    ///
    /// # Example
    /// ```rust,ignore
    /// let starlink = client.get_starlink_satellites().await?;
    /// ```
    pub async fn get_starlink_satellites(&self) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/starlink", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let satellites: Vec<StarlinkSatellite> = response.json().await?;

        let mut vectors = Vec::new();

        // Limit to 100 satellites to avoid overwhelming the system
        for satellite in satellites.iter().take(100) {
            let lon = satellite.longitude.unwrap_or(0.0);
            let lat = satellite.latitude.unwrap_or(0.0);
            let height = satellite.height_km.unwrap_or(0.0);

            let text = format!(
                "Starlink satellite {} version {}, orbit: {:.2}°N, {:.2}°E at {:.0}km",
                satellite.id, satellite.version, lat, lon, height
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("satellite_id".to_string(), satellite.id.clone());
            metadata.insert("version".to_string(), satellite.version.clone());
            metadata.insert("launch".to_string(), satellite.launch.clone());
            metadata.insert("longitude".to_string(), lon.to_string());
            metadata.insert("latitude".to_string(), lat.to_string());
            metadata.insert("height_km".to_string(), height.to_string());
            metadata.insert("source".to_string(), "spacex_starlink".to_string());

            vectors.push(SemanticVector {
                id: format!("SPACEX:STARLINK:{}", satellite.id),
                embedding,
                domain: Domain::Space,
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

impl Default for SpaceXClient {
    fn default() -> Self {
        Self::new().expect("Failed to create SpaceXClient")
    }
}

// ============================================================================
// Open Astronomy Catalogs Client
// ============================================================================

/// Supernova data from Open Supernova Catalog
#[derive(Debug, Deserialize)]
struct SupernovaData {
    #[serde(default)]
    name: String,
    #[serde(default)]
    ra: Option<String>,
    #[serde(default)]
    dec: Option<String>,
    #[serde(default)]
    discoveryear: Option<String>,
    #[serde(default)]
    claimedtype: Option<String>,
    #[serde(default)]
    redshift: Option<String>,
    #[serde(default)]
    maxappmag: Option<String>,
}

/// Client for Open Astronomy Catalogs
///
/// Provides access to:
/// - Open Supernova Catalog
/// - Transient Name Server events
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::AstronomyClient;
///
/// let client = AstronomyClient::new()?;
/// let supernovae = client.search_supernovae(Some(50)).await?;
/// ```
pub struct AstronomyClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl AstronomyClient {
    /// Create a new Open Astronomy Catalogs client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("ruvector-data-framework/1.0")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.astrocats.space".to_string(),
            rate_limit_delay: Duration::from_millis(ASTRONOMY_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search for supernovae
    ///
    /// # Arguments
    /// * `limit` - Optional limit on number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// let supernovae = client.search_supernovae(Some(50)).await?;
    /// ```
    pub async fn search_supernovae(&self, limit: Option<usize>) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/catalog", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        // Note: The actual API response format may vary
        // This is a simplified implementation
        let text = response.text().await?;
        let data: HashMap<String, SupernovaData> = serde_json::from_str(&text)
            .unwrap_or_default();

        let mut vectors = Vec::new();
        let take_count = limit.unwrap_or(50);

        for (id, sn) in data.iter().take(take_count) {
            let sn_type = sn.claimedtype.as_deref().unwrap_or("unknown");
            let year = sn.discoveryear.as_deref().unwrap_or("unknown");
            let redshift = sn.redshift.as_deref().unwrap_or("unknown");

            let text = format!(
                "Supernova {} (type {}), discovered {}, redshift {}, coords: {} {}",
                sn.name,
                sn_type,
                year,
                redshift,
                sn.ra.as_deref().unwrap_or("unknown"),
                sn.dec.as_deref().unwrap_or("unknown")
            );
            let embedding = self.embedder.embed_text(&text);

            // Use discovery year for timestamp
            let timestamp = sn.discoveryear
                .as_ref()
                .and_then(|y| y.parse::<i32>().ok())
                .and_then(|y| NaiveDate::from_ymd_opt(y, 1, 1))
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let mut metadata = HashMap::new();
            metadata.insert("name".to_string(), sn.name.clone());
            metadata.insert("type".to_string(), sn_type.to_string());
            metadata.insert("discovery_year".to_string(), year.to_string());
            metadata.insert("ra".to_string(), sn.ra.clone().unwrap_or_default());
            metadata.insert("dec".to_string(), sn.dec.clone().unwrap_or_default());
            metadata.insert("redshift".to_string(), redshift.to_string());
            metadata.insert("max_magnitude".to_string(), sn.maxappmag.clone().unwrap_or_default());
            metadata.insert("source".to_string(), "open_supernova_catalog".to_string());

            vectors.push(SemanticVector {
                id: format!("SUPERNOVA:{}", id),
                embedding,
                domain: Domain::Space,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Search for transient astronomical events
    ///
    /// # Example
    /// ```rust,ignore
    /// let transients = client.search_transients().await?;
    /// ```
    pub async fn search_transients(&self) -> Result<Vec<SemanticVector>> {
        // This is a placeholder - TNS API requires registration
        // In production, you would implement TNS API integration here
        Ok(Vec::new())
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

impl Default for AstronomyClient {
    fn default() -> Self {
        Self::new().expect("Failed to create AstronomyClient")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_nasa_client_creation() {
        let client = NasaClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_nasa_client_with_key() {
        let client = NasaClient::new(Some("test_key".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_exoplanet_client_creation() {
        let client = ExoplanetClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_spacex_client_creation() {
        let client = SpaceXClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_astronomy_client_creation() {
        let client = AstronomyClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_rate_limiting() {
        let nasa = NasaClient::new(None).unwrap();
        assert_eq!(nasa.rate_limit_delay, Duration::from_millis(NASA_RATE_LIMIT_MS));

        let exoplanet = ExoplanetClient::new().unwrap();
        assert_eq!(exoplanet.rate_limit_delay, Duration::from_millis(NASA_RATE_LIMIT_MS));

        let spacex = SpaceXClient::new().unwrap();
        assert_eq!(spacex.rate_limit_delay, Duration::from_millis(SPACEX_RATE_LIMIT_MS));

        let astronomy = AstronomyClient::new().unwrap();
        assert_eq!(astronomy.rate_limit_delay, Duration::from_millis(ASTRONOMY_RATE_LIMIT_MS));
    }

    #[test]
    fn test_domain_is_space() {
        let embedder = SimpleEmbedder::new(384);
        let embedding = embedder.embed_text("test");

        let vector = SemanticVector {
            id: "test".to_string(),
            embedding,
            domain: Domain::Space,
            timestamp: Utc::now(),
            metadata: HashMap::new(),
        };

        assert_eq!(vector.domain, Domain::Space);
    }
}
