//! Economic data API integrations for FRED, World Bank, and Alpha Vantage
//!
//! This module provides async clients for fetching economic indicators, global development data,
//! and stock market information, converting responses to SemanticVector format for RuVector discovery.

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
const FRED_RATE_LIMIT_MS: u64 = 100; // ~10 requests/second
const WORLDBANK_RATE_LIMIT_MS: u64 = 100; // Conservative rate
const ALPHAVANTAGE_RATE_LIMIT_MS: u64 = 12000; // 5 requests/minute for free tier
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// FRED (Federal Reserve Economic Data) Client
// ============================================================================

/// FRED API observations response
#[derive(Debug, Deserialize)]
struct FredObservationsResponse {
    #[serde(default)]
    observations: Vec<FredObservation>,
    #[serde(default)]
    error_code: Option<i32>,
    #[serde(default)]
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FredObservation {
    #[serde(default)]
    date: String,
    #[serde(default)]
    value: String,
}

/// FRED API series search response
#[derive(Debug, Deserialize)]
struct FredSeriesSearchResponse {
    seriess: Vec<FredSeries>,
}

#[derive(Debug, Deserialize)]
struct FredSeries {
    id: String,
    title: String,
    #[serde(default)]
    units: String,
    #[serde(default)]
    frequency: String,
    #[serde(default)]
    seasonal_adjustment: String,
    #[serde(default)]
    notes: String,
}

/// Client for FRED (Federal Reserve Economic Data)
///
/// Provides access to 800,000+ US economic time series including:
/// - GDP, unemployment, inflation, interest rates
/// - Money supply, consumer spending, housing data
/// - Regional economic indicators
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::FredClient;
///
/// let client = FredClient::new(None)?;
/// let gdp_data = client.get_gdp().await?;
/// let unemployment = client.get_unemployment().await?;
/// let search_results = client.search_series("inflation").await?;
/// ```
pub struct FredClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl FredClient {
    /// Create a new FRED client
    ///
    /// # Arguments
    /// * `api_key` - Optional FRED API key (get from https://fred.stlouisfed.org/docs/api/api_key.html)
    ///               Basic access works without a key, but rate limits are more restrictive
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.stlouisfed.org/fred".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(FRED_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get observations for a specific FRED series
    ///
    /// # Arguments
    /// * `series_id` - FRED series ID (e.g., "GDP", "UNRATE", "CPIAUCSL")
    /// * `limit` - Maximum number of observations to return (default: 100)
    ///
    /// # Example
    /// ```rust,ignore
    /// let gdp = client.get_series("GDP", Some(50)).await?;
    /// ```
    pub async fn get_series(
        &self,
        series_id: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        // FRED API requires an API key as of 2025
        let api_key = self.api_key.as_ref().ok_or_else(|| {
            FrameworkError::Config(
                "FRED API key required. Get one at https://fred.stlouisfed.org/docs/api/api_key.html".to_string()
            )
        })?;

        let mut url = format!(
            "{}/series/observations?series_id={}&file_type=json&api_key={}",
            self.base_url, series_id, api_key
        );

        if let Some(lim) = limit {
            url.push_str(&format!("&limit={}", lim));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let obs_response: FredObservationsResponse = response.json().await?;

        // Check for API error response
        if let Some(error_msg) = obs_response.error_message {
            return Err(FrameworkError::Ingestion(format!("FRED API error: {}", error_msg)));
        }

        let mut vectors = Vec::new();
        for obs in obs_response.observations {
            // Parse value, skip if invalid
            let value = match obs.value.parse::<f64>() {
                Ok(v) => v,
                Err(_) => continue, // Skip ".", missing values, etc.
            };

            // Parse date
            let date = NaiveDate::parse_from_str(&obs.date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            // Create text for embedding
            let text = format!("{} on {}: {}", series_id, obs.date, value);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("series_id".to_string(), series_id.to_string());
            metadata.insert("date".to_string(), obs.date.clone());
            metadata.insert("value".to_string(), value.to_string());
            metadata.insert("source".to_string(), "fred".to_string());

            vectors.push(SemanticVector {
                id: format!("FRED:{}:{}", series_id, obs.date),
                embedding,
                domain: Domain::Economic,
                timestamp: date,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Search for FRED series by keywords
    ///
    /// # Arguments
    /// * `keywords` - Search terms (e.g., "unemployment rate", "consumer price index")
    ///
    /// # Example
    /// ```rust,ignore
    /// let inflation_series = client.search_series("inflation").await?;
    /// ```
    pub async fn search_series(&self, keywords: &str) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/series/search?search_text={}&file_type=json&limit=50",
            self.base_url,
            urlencoding::encode(keywords)
        );

        if let Some(key) = &self.api_key {
            url.push_str(&format!("&api_key={}", key));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: FredSeriesSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for series in search_response.seriess {
            // Create text for embedding
            let text = format!(
                "{} {} {} {}",
                series.title, series.units, series.frequency, series.notes
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("series_id".to_string(), series.id.clone());
            metadata.insert("title".to_string(), series.title.clone());
            metadata.insert("units".to_string(), series.units);
            metadata.insert("frequency".to_string(), series.frequency);
            metadata.insert("seasonal_adjustment".to_string(), series.seasonal_adjustment);
            metadata.insert("source".to_string(), "fred_search".to_string());

            vectors.push(SemanticVector {
                id: format!("FRED_SERIES:{}", series.id),
                embedding,
                domain: Domain::Economic,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get US GDP data (Gross Domestic Product)
    ///
    /// # Example
    /// ```rust,ignore
    /// let gdp = client.get_gdp().await?;
    /// ```
    pub async fn get_gdp(&self) -> Result<Vec<SemanticVector>> {
        self.get_series("GDP", Some(100)).await
    }

    /// Get US unemployment rate
    ///
    /// # Example
    /// ```rust,ignore
    /// let unemployment = client.get_unemployment().await?;
    /// ```
    pub async fn get_unemployment(&self) -> Result<Vec<SemanticVector>> {
        self.get_series("UNRATE", Some(100)).await
    }

    /// Get US Consumer Price Index (CPI) - inflation indicator
    ///
    /// # Example
    /// ```rust,ignore
    /// let cpi = client.get_cpi().await?;
    /// ```
    pub async fn get_cpi(&self) -> Result<Vec<SemanticVector>> {
        self.get_series("CPIAUCSL", Some(100)).await
    }

    /// Get US Federal Funds Rate
    ///
    /// # Example
    /// ```rust,ignore
    /// let interest_rates = client.get_interest_rate().await?;
    /// ```
    pub async fn get_interest_rate(&self) -> Result<Vec<SemanticVector>> {
        self.get_series("DFF", Some(100)).await
    }

    /// Get US M2 Money Supply
    ///
    /// # Example
    /// ```rust,ignore
    /// let money_supply = client.get_money_supply().await?;
    /// ```
    pub async fn get_money_supply(&self) -> Result<Vec<SemanticVector>> {
        self.get_series("M2SL", Some(100)).await
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
// World Bank Open Data Client
// ============================================================================

/// World Bank API response (v2)
#[derive(Debug, Deserialize)]
struct WorldBankResponse {
    #[serde(default)]
    page: u32,
    #[serde(default)]
    pages: u32,
    #[serde(default)]
    per_page: u32,
    #[serde(default)]
    total: u32,
}

/// World Bank indicator data point
#[derive(Debug, Deserialize)]
struct WorldBankIndicator {
    indicator: WorldBankIndicatorInfo,
    country: WorldBankCountryInfo,
    #[serde(default)]
    countryiso3code: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    value: Option<f64>,
    #[serde(default)]
    unit: String,
    #[serde(default)]
    obs_status: String,
}

#[derive(Debug, Deserialize)]
struct WorldBankIndicatorInfo {
    id: String,
    value: String,
}

#[derive(Debug, Deserialize)]
struct WorldBankCountryInfo {
    id: String,
    value: String,
}

/// Client for World Bank Open Data API
///
/// Provides access to global development indicators including:
/// - GDP per capita, population, poverty rates
/// - Health expenditure, life expectancy
/// - CO2 emissions, renewable energy
/// - Education, infrastructure metrics
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::WorldBankClient;
///
/// let client = WorldBankClient::new()?;
/// let gdp_global = client.get_gdp_global().await?;
/// let climate = client.get_climate_indicators().await?;
/// let health = client.get_indicator("USA", "SH.XPD.CHEX.GD.ZS").await?;
/// ```
pub struct WorldBankClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl WorldBankClient {
    /// Create a new World Bank client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.worldbank.org/v2".to_string(),
            rate_limit_delay: Duration::from_millis(WORLDBANK_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get indicator data for a specific country
    ///
    /// # Arguments
    /// * `country` - ISO 3-letter country code (e.g., "USA", "CHN", "GBR") or "all"
    /// * `indicator` - World Bank indicator code (e.g., "NY.GDP.PCAP.CD" for GDP per capita)
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get US GDP per capita
    /// let us_gdp = client.get_indicator("USA", "NY.GDP.PCAP.CD").await?;
    /// ```
    pub async fn get_indicator(
        &self,
        country: &str,
        indicator: &str,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/country/{}/indicator/{}?format=json&per_page=100",
            self.base_url, country, indicator
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        // World Bank API returns [metadata, data]
        let json_values: Vec<serde_json::Value> = serde_json::from_str(&text)?;

        if json_values.len() < 2 {
            return Ok(Vec::new());
        }

        let indicators: Vec<WorldBankIndicator> = serde_json::from_value(json_values[1].clone())?;

        let mut vectors = Vec::new();
        for ind in indicators {
            // Skip null values
            let value = match ind.value {
                Some(v) => v,
                None => continue,
            };

            // Parse date
            let year = ind.date.parse::<i32>().unwrap_or(2020);
            let date = NaiveDate::from_ymd_opt(year, 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            // Create text for embedding
            let text = format!(
                "{} {} in {}: {}",
                ind.country.value, ind.indicator.value, ind.date, value
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("country".to_string(), ind.country.value);
            metadata.insert("country_code".to_string(), ind.countryiso3code.clone());
            metadata.insert("indicator_id".to_string(), ind.indicator.id.clone());
            metadata.insert("indicator_name".to_string(), ind.indicator.value);
            metadata.insert("date".to_string(), ind.date.clone());
            metadata.insert("value".to_string(), value.to_string());
            metadata.insert("source".to_string(), "worldbank".to_string());

            vectors.push(SemanticVector {
                id: format!("WB:{}:{}:{}", ind.countryiso3code, ind.indicator.id, ind.date),
                embedding,
                domain: Domain::Economic,
                timestamp: date,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get global GDP per capita data
    ///
    /// # Example
    /// ```rust,ignore
    /// let gdp_global = client.get_gdp_global().await?;
    /// ```
    pub async fn get_gdp_global(&self) -> Result<Vec<SemanticVector>> {
        // Get GDP per capita for major economies
        self.get_indicator("all", "NY.GDP.PCAP.CD").await
    }

    /// Get climate change indicators (CO2 emissions, renewable energy)
    ///
    /// # Example
    /// ```rust,ignore
    /// let climate = client.get_climate_indicators().await?;
    /// ```
    pub async fn get_climate_indicators(&self) -> Result<Vec<SemanticVector>> {
        // CO2 emissions (metric tons per capita)
        let mut vectors = self.get_indicator("all", "EN.ATM.CO2E.PC").await?;

        // Renewable energy consumption (% of total)
        sleep(self.rate_limit_delay).await;
        let renewable = self.get_indicator("all", "EG.FEC.RNEW.ZS").await?;
        vectors.extend(renewable);

        Ok(vectors)
    }

    /// Get health expenditure indicators
    ///
    /// # Example
    /// ```rust,ignore
    /// let health = client.get_health_indicators().await?;
    /// ```
    pub async fn get_health_indicators(&self) -> Result<Vec<SemanticVector>> {
        // Health expenditure as % of GDP
        let mut vectors = self.get_indicator("all", "SH.XPD.CHEX.GD.ZS").await?;

        // Life expectancy at birth
        sleep(self.rate_limit_delay).await;
        let life_exp = self.get_indicator("all", "SP.DYN.LE00.IN").await?;
        vectors.extend(life_exp);

        Ok(vectors)
    }

    /// Get population data
    ///
    /// # Example
    /// ```rust,ignore
    /// let population = client.get_population().await?;
    /// ```
    pub async fn get_population(&self) -> Result<Vec<SemanticVector>> {
        self.get_indicator("all", "SP.POP.TOTL").await
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

impl Default for WorldBankClient {
    fn default() -> Self {
        Self::new().expect("Failed to create WorldBank client")
    }
}

// ============================================================================
// Alpha Vantage Client (Optional - Stock Market Data)
// ============================================================================

/// Alpha Vantage time series data
#[derive(Debug, Deserialize)]
struct AlphaVantageTimeSeriesResponse {
    #[serde(rename = "Meta Data", default)]
    meta_data: Option<serde_json::Value>,
    #[serde(rename = "Time Series (Daily)", default)]
    time_series: Option<HashMap<String, AlphaVantageDailyData>>,
}

#[derive(Debug, Deserialize)]
struct AlphaVantageDailyData {
    #[serde(rename = "1. open")]
    open: String,
    #[serde(rename = "2. high")]
    high: String,
    #[serde(rename = "3. low")]
    low: String,
    #[serde(rename = "4. close")]
    close: String,
    #[serde(rename = "5. volume")]
    volume: String,
}

/// Client for Alpha Vantage API (stock market data)
///
/// Provides access to:
/// - Daily stock prices
/// - Sector performance
/// - Technical indicators
///
/// **Note**: Free tier limited to 5 requests per minute, 500 per day
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::AlphaVantageClient;
///
/// let client = AlphaVantageClient::new("YOUR_API_KEY".to_string())?;
/// let aapl = client.get_daily_stock("AAPL").await?;
/// ```
pub struct AlphaVantageClient {
    client: Client,
    base_url: String,
    api_key: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl AlphaVantageClient {
    /// Create a new Alpha Vantage client
    ///
    /// # Arguments
    /// * `api_key` - Alpha Vantage API key (get free key from https://www.alphavantage.co/support/#api-key)
    pub fn new(api_key: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://www.alphavantage.co/query".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(ALPHAVANTAGE_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get daily stock price data
    ///
    /// # Arguments
    /// * `symbol` - Stock ticker symbol (e.g., "AAPL", "MSFT", "TSLA")
    ///
    /// # Example
    /// ```rust,ignore
    /// let aapl = client.get_daily_stock("AAPL").await?;
    /// ```
    pub async fn get_daily_stock(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}?function=TIME_SERIES_DAILY&symbol={}&apikey={}",
            self.base_url, symbol, self.api_key
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let ts_response: AlphaVantageTimeSeriesResponse = response.json().await?;

        let time_series = match ts_response.time_series {
            Some(ts) => ts,
            None => return Ok(Vec::new()),
        };

        let mut vectors = Vec::new();
        for (date_str, data) in time_series.iter().take(100) {
            // Parse values
            let close = data.close.parse::<f64>().unwrap_or(0.0);
            let volume = data.volume.parse::<f64>().unwrap_or(0.0);

            // Parse date
            let date = NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            // Create text for embedding
            let text = format!(
                "{} stock on {}: close ${}, volume {}",
                symbol, date_str, close, volume
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.to_string());
            metadata.insert("date".to_string(), date_str.clone());
            metadata.insert("open".to_string(), data.open.clone());
            metadata.insert("high".to_string(), data.high.clone());
            metadata.insert("low".to_string(), data.low.clone());
            metadata.insert("close".to_string(), data.close.clone());
            metadata.insert("volume".to_string(), data.volume.clone());
            metadata.insert("source".to_string(), "alphavantage".to_string());

            vectors.push(SemanticVector {
                id: format!("AV:{}:{}", symbol, date_str),
                embedding,
                domain: Domain::Finance,
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fred_client_creation() {
        let client = FredClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_fred_client_with_key() {
        let client = FredClient::new(Some("test_key".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_worldbank_client_creation() {
        let client = WorldBankClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_alphavantage_client_creation() {
        let client = AlphaVantageClient::new("test_key".to_string());
        assert!(client.is_ok());
    }

    #[test]
    fn test_rate_limiting() {
        // Verify rate limits are set correctly
        let fred = FredClient::new(None).unwrap();
        assert_eq!(fred.rate_limit_delay, Duration::from_millis(FRED_RATE_LIMIT_MS));

        let wb = WorldBankClient::new().unwrap();
        assert_eq!(wb.rate_limit_delay, Duration::from_millis(WORLDBANK_RATE_LIMIT_MS));

        let av = AlphaVantageClient::new("test".to_string()).unwrap();
        assert_eq!(av.rate_limit_delay, Duration::from_millis(ALPHAVANTAGE_RATE_LIMIT_MS));
    }
}
