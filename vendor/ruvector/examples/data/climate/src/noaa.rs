//! NOAA data client and schemas

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use crate::{BoundingBox, ClimateError, ClimateObservation, DataSourceType, QualityFlag};

/// Weather variable types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum WeatherVariable {
    /// Temperature (Celsius)
    Temperature,
    /// Precipitation (mm)
    Precipitation,
    /// Snow depth (mm)
    SnowDepth,
    /// Wind speed (m/s)
    WindSpeed,
    /// Wind direction (degrees)
    WindDirection,
    /// Humidity (%)
    Humidity,
    /// Pressure (hPa)
    Pressure,
    /// Solar radiation (W/m^2)
    SolarRadiation,
    /// Other variable
    Other,
}

impl WeatherVariable {
    /// Get NOAA element code
    pub fn noaa_code(&self) -> &str {
        match self {
            WeatherVariable::Temperature => "TMAX",
            WeatherVariable::Precipitation => "PRCP",
            WeatherVariable::SnowDepth => "SNWD",
            WeatherVariable::WindSpeed => "AWND",
            WeatherVariable::WindDirection => "WDF2",
            WeatherVariable::Humidity => "RHAV",
            WeatherVariable::Pressure => "PRES",
            WeatherVariable::SolarRadiation => "TSUN",
            WeatherVariable::Other => "TAVG",
        }
    }

    /// Parse from NOAA code
    pub fn from_noaa_code(code: &str) -> Self {
        match code {
            "TMAX" | "TMIN" | "TAVG" => WeatherVariable::Temperature,
            "PRCP" => WeatherVariable::Precipitation,
            "SNWD" | "SNOW" => WeatherVariable::SnowDepth,
            "AWND" | "WSF2" | "WSF5" => WeatherVariable::WindSpeed,
            "WDF2" | "WDF5" => WeatherVariable::WindDirection,
            "RHAV" => WeatherVariable::Humidity,
            "PRES" => WeatherVariable::Pressure,
            "TSUN" => WeatherVariable::SolarRadiation,
            _ => WeatherVariable::Other,
        }
    }
}

/// GHCN (Global Historical Climatology Network) station
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhcnStation {
    /// Station ID
    pub id: String,

    /// Station name
    pub name: String,

    /// Latitude
    pub latitude: f64,

    /// Longitude
    pub longitude: f64,

    /// Elevation (meters)
    pub elevation: Option<f64>,

    /// State/province
    pub state: Option<String>,

    /// Country code
    pub country: String,

    /// Data coverage start
    pub mindate: Option<String>,

    /// Data coverage end
    pub maxdate: Option<String>,
}

/// GHCN observation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhcnObservation {
    /// Station ID
    pub station: String,

    /// Observation date
    pub date: String,

    /// Data type (element code)
    pub datatype: String,

    /// Value
    pub value: f64,

    /// Quality flags
    #[serde(default)]
    pub attributes: String,
}

/// NOAA API client
pub struct NoaaClient {
    client: Client,
    token: Option<String>,
    base_url: String,
}

/// NOAA API response
#[derive(Debug, Deserialize)]
pub struct NoaaResponse<T> {
    /// Metadata
    pub metadata: Option<NoaaMetadata>,

    /// Results
    pub results: Option<Vec<T>>,
}

/// NOAA response metadata
#[derive(Debug, Deserialize)]
pub struct NoaaMetadata {
    /// Result set info
    pub resultset: Option<ResultSet>,
}

/// Result set info
#[derive(Debug, Deserialize)]
pub struct ResultSet {
    /// Offset
    pub offset: u32,

    /// Count
    pub count: u32,

    /// Limit
    pub limit: u32,
}

impl NoaaClient {
    /// Create a new NOAA client
    pub fn new(token: Option<String>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/0.1.0")
            .build()
            .expect("Failed to build HTTP client");

        Self {
            client,
            token,
            base_url: "https://www.ncdc.noaa.gov/cdo-web/api/v2".to_string(),
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<bool, ClimateError> {
        let url = format!("{}/datasets", self.base_url);
        let mut req = self.client.get(&url);

        if let Some(ref token) = self.token {
            req = req.header("token", token);
        }

        let response = req.send().await?;
        Ok(response.status().is_success())
    }

    /// Fetch GHCN observations
    pub async fn fetch_ghcn_observations(
        &self,
        bounds: Option<BoundingBox>,
        variables: &[WeatherVariable],
        cursor: Option<String>,
        limit: usize,
    ) -> Result<(Vec<ClimateObservation>, Option<String>), ClimateError> {
        // Build query
        let datatypes: Vec<_> = variables.iter().map(|v| v.noaa_code()).collect();
        let datatype_param = datatypes.join(",");

        let mut params = format!(
            "datasetid=GHCND&datatypeid={}&limit={}",
            datatype_param,
            limit.min(1000)
        );

        if let Some(ref c) = cursor {
            let offset: u32 = c.parse().unwrap_or(0);
            params.push_str(&format!("&offset={}", offset));
        }

        if let Some(bbox) = bounds {
            params.push_str(&format!(
                "&extent={},{},{},{}",
                bbox.min_lat, bbox.min_lon, bbox.max_lat, bbox.max_lon
            ));
        }

        // Add date range (last 30 days for demo)
        let end_date = Utc::now();
        let start_date = end_date - chrono::Duration::days(30);
        params.push_str(&format!(
            "&startdate={}&enddate={}",
            start_date.format("%Y-%m-%d"),
            end_date.format("%Y-%m-%d")
        ));

        let url = format!("{}/data?{}", self.base_url, params);

        let mut req = self.client.get(&url);
        if let Some(ref token) = self.token {
            req = req.header("token", token);
        }

        let response = req.send().await?;

        match response.status() {
            StatusCode::OK => {
                let api_response: NoaaResponse<GhcnObservation> = response.json().await?;

                let observations: Vec<ClimateObservation> = api_response
                    .results
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|obs| self.convert_observation(obs).ok())
                    .collect();

                // Compute next cursor
                let next_cursor = api_response.metadata.and_then(|m| {
                    m.resultset.and_then(|rs| {
                        if rs.offset + rs.count < rs.limit {
                            Some((rs.offset + rs.count).to_string())
                        } else {
                            None
                        }
                    })
                });

                Ok((observations, next_cursor))
            }
            StatusCode::UNAUTHORIZED => Err(ClimateError::Api("Invalid or missing API token".to_string())),
            StatusCode::TOO_MANY_REQUESTS => Err(ClimateError::Api("Rate limit exceeded".to_string())),
            status => Err(ClimateError::Api(format!("Unexpected status: {}", status))),
        }
    }

    /// Convert GHCN observation to generic format
    fn convert_observation(&self, obs: GhcnObservation) -> Result<ClimateObservation, ClimateError> {
        // Parse date
        let timestamp = DateTime::parse_from_str(
            &format!("{}T00:00:00Z", obs.date),
            "%Y-%m-%dT%H:%M:%SZ",
        )
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| ClimateError::DataFormat(format!("Invalid date: {}", obs.date)))?;

        // Parse quality flag
        let quality = if obs.attributes.contains("S") {
            QualityFlag::Suspect
        } else if obs.attributes.contains("X") {
            QualityFlag::Erroneous
        } else {
            QualityFlag::Good
        };

        Ok(ClimateObservation {
            station_id: obs.station,
            timestamp,
            location: (0.0, 0.0), // Would fetch from station metadata
            variable: WeatherVariable::from_noaa_code(&obs.datatype),
            value: obs.value,
            quality,
            source: DataSourceType::NoaaGhcn,
            metadata: HashMap::new(),
        })
    }

    /// Fetch stations in a bounding box
    pub async fn fetch_stations(&self, bounds: BoundingBox) -> Result<Vec<GhcnStation>, ClimateError> {
        let params = format!(
            "datasetid=GHCND&extent={},{},{},{}&limit=1000",
            bounds.min_lat, bounds.min_lon, bounds.max_lat, bounds.max_lon
        );

        let url = format!("{}/stations?{}", self.base_url, params);

        let mut req = self.client.get(&url);
        if let Some(ref token) = self.token {
            req = req.header("token", token);
        }

        let response = req.send().await?;

        match response.status() {
            StatusCode::OK => {
                let api_response: NoaaResponse<GhcnStation> = response.json().await?;
                Ok(api_response.results.unwrap_or_default())
            }
            status => Err(ClimateError::Api(format!("Unexpected status: {}", status))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weather_variable_codes() {
        assert_eq!(WeatherVariable::Temperature.noaa_code(), "TMAX");
        assert_eq!(WeatherVariable::Precipitation.noaa_code(), "PRCP");
    }

    #[test]
    fn test_variable_from_code() {
        assert_eq!(
            WeatherVariable::from_noaa_code("TMAX"),
            WeatherVariable::Temperature
        );
        assert_eq!(
            WeatherVariable::from_noaa_code("PRCP"),
            WeatherVariable::Precipitation
        );
    }

    #[test]
    fn test_client_creation() {
        let client = NoaaClient::new(None);
        assert!(client.token.is_none());
    }
}
