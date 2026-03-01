//! NASA Earthdata client and schemas

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::{BoundingBox, ClimateError, ClimateObservation, DataSourceType, QualityFlag, WeatherVariable};

/// NASA MODIS product types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ModisProduct {
    /// Land Surface Temperature
    LandSurfaceTemp,
    /// Vegetation Index (NDVI)
    VegetationIndex,
    /// Surface Reflectance
    SurfaceReflectance,
    /// Snow Cover
    SnowCover,
    /// Fire Detection
    FireDetection,
    /// Ocean Color
    OceanColor,
}

impl ModisProduct {
    /// Get product short name
    pub fn short_name(&self) -> &str {
        match self {
            ModisProduct::LandSurfaceTemp => "MOD11A1",
            ModisProduct::VegetationIndex => "MOD13A1",
            ModisProduct::SurfaceReflectance => "MOD09GA",
            ModisProduct::SnowCover => "MOD10A1",
            ModisProduct::FireDetection => "MOD14A1",
            ModisProduct::OceanColor => "MODOCGA",
        }
    }
}

/// Satellite observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SatelliteObservation {
    /// Granule ID
    pub granule_id: String,

    /// Product type
    pub product: String,

    /// Acquisition time
    pub time_start: DateTime<Utc>,

    /// Time end
    pub time_end: DateTime<Utc>,

    /// Bounding box
    pub bounding_box: BoundingBox,

    /// Cloud cover percentage
    pub cloud_cover: Option<f64>,

    /// Day/night flag
    pub day_night: Option<String>,

    /// Download URLs
    pub links: Vec<String>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// NASA Earthdata API client
pub struct NasaClient {
    client: Client,
    token: Option<String>,
    base_url: String,
}

/// CMR (Common Metadata Repository) search response
#[derive(Debug, Deserialize)]
pub struct CmrResponse {
    /// Feed
    pub feed: CmrFeed,
}

/// CMR feed
#[derive(Debug, Deserialize)]
pub struct CmrFeed {
    /// Entries
    pub entry: Vec<CmrEntry>,
}

/// CMR entry (granule)
#[derive(Debug, Deserialize)]
pub struct CmrEntry {
    /// ID
    pub id: String,

    /// Title
    pub title: String,

    /// Time start
    pub time_start: String,

    /// Time end
    pub time_end: String,

    /// Bounding box
    pub boxes: Option<Vec<String>>,

    /// Links
    pub links: Option<Vec<CmrLink>>,

    /// Cloud cover
    pub cloud_cover: Option<String>,

    /// Day/night flag
    pub day_night_flag: Option<String>,
}

/// CMR link
#[derive(Debug, Deserialize)]
pub struct CmrLink {
    /// Relation
    pub rel: String,

    /// Href
    pub href: String,

    /// Type
    #[serde(rename = "type")]
    pub link_type: Option<String>,
}

impl NasaClient {
    /// Create a new NASA Earthdata client
    pub fn new(token: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("RuVector/0.1.0")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            token,
            base_url: "https://cmr.earthdata.nasa.gov/search".to_string(),
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, ClimateError> {
        let url = format!("{}/collections?page_size=1", self.base_url);
        let response = self.client.get(&url).send().await?;
        Ok(response.status().is_success())
    }

    /// Search for MODIS granules
    pub async fn search_modis(
        &self,
        product: ModisProduct,
        bounds: Option<BoundingBox>,
        start_date: DateTime<Utc>,
        end_date: DateTime<Utc>,
        limit: usize,
    ) -> Result<Vec<SatelliteObservation>, ClimateError> {
        let mut params = format!(
            "short_name={}&temporal={},{}&page_size={}",
            product.short_name(),
            start_date.format("%Y-%m-%dT%H:%M:%SZ"),
            end_date.format("%Y-%m-%dT%H:%M:%SZ"),
            limit.min(2000)
        );

        if let Some(bbox) = bounds {
            params.push_str(&format!(
                "&bounding_box={},{},{},{}",
                bbox.min_lon, bbox.min_lat, bbox.max_lon, bbox.max_lat
            ));
        }

        let url = format!("{}/granules.json?{}", self.base_url, params);

        let mut req = self.client.get(&url);
        if let Some(ref token) = self.token {
            req = req.header("Authorization", format!("Bearer {}", token));
        }

        let response = req.send().await?;

        if !response.status().is_success() {
            return Err(ClimateError::Api(format!(
                "CMR search failed: {}",
                response.status()
            )));
        }

        let cmr_response: CmrResponse = response.json().await?;

        let observations: Vec<SatelliteObservation> = cmr_response
            .feed
            .entry
            .into_iter()
            .filter_map(|entry| self.convert_entry(entry, &product).ok())
            .collect();

        Ok(observations)
    }

    /// Convert CMR entry to satellite observation
    fn convert_entry(
        &self,
        entry: CmrEntry,
        product: &ModisProduct,
    ) -> Result<SatelliteObservation, ClimateError> {
        // Parse times
        let time_start = DateTime::parse_from_rfc3339(&entry.time_start)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| ClimateError::DataFormat("Invalid time_start".to_string()))?;

        let time_end = DateTime::parse_from_rfc3339(&entry.time_end)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|_| ClimateError::DataFormat("Invalid time_end".to_string()))?;

        // Parse bounding box
        let bounding_box = entry
            .boxes
            .as_ref()
            .and_then(|boxes| boxes.first())
            .and_then(|box_str| self.parse_box(box_str))
            .unwrap_or(BoundingBox::global());

        // Extract download links
        let links: Vec<String> = entry
            .links
            .unwrap_or_default()
            .into_iter()
            .filter(|l| l.rel == "http://esipfed.org/ns/fedsearch/1.1/data#")
            .map(|l| l.href)
            .collect();

        // Parse cloud cover
        let cloud_cover = entry
            .cloud_cover
            .as_ref()
            .and_then(|s| s.parse().ok());

        Ok(SatelliteObservation {
            granule_id: entry.id,
            product: product.short_name().to_string(),
            time_start,
            time_end,
            bounding_box,
            cloud_cover,
            day_night: entry.day_night_flag,
            links,
            metadata: HashMap::new(),
        })
    }

    /// Parse bounding box string
    fn parse_box(&self, box_str: &str) -> Option<BoundingBox> {
        let parts: Vec<f64> = box_str
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        if parts.len() == 4 {
            Some(BoundingBox::new(parts[0], parts[2], parts[1], parts[3]))
        } else {
            None
        }
    }

    /// Convert satellite observation to climate observation
    pub fn to_climate_observation(
        &self,
        sat_obs: &SatelliteObservation,
        value: f64,
        variable: WeatherVariable,
    ) -> ClimateObservation {
        let center = sat_obs.bounding_box.center();

        ClimateObservation {
            station_id: sat_obs.granule_id.clone(),
            timestamp: sat_obs.time_start,
            location: center,
            variable,
            value,
            quality: if sat_obs.cloud_cover.unwrap_or(0.0) < 20.0 {
                QualityFlag::Good
            } else {
                QualityFlag::Suspect
            },
            source: DataSourceType::NasaModis,
            metadata: sat_obs.metadata.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modis_product_names() {
        assert_eq!(ModisProduct::LandSurfaceTemp.short_name(), "MOD11A1");
        assert_eq!(ModisProduct::VegetationIndex.short_name(), "MOD13A1");
    }

    #[test]
    fn test_client_creation() {
        let client = NasaClient::new(None);
        assert!(client.token.is_none());
    }

    #[test]
    fn test_parse_box() {
        let client = NasaClient::new(None);
        let bbox = client.parse_box("30.0 -100.0 40.0 -90.0");
        assert!(bbox.is_some());
        let bbox = bbox.unwrap();
        assert!((bbox.min_lat - 30.0).abs() < 0.01);
    }
}
