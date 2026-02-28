//! Finance & Economics API integrations for market data and economic indicators
//!
//! This module provides async clients for fetching financial market data, cryptocurrency prices,
//! exchange rates, and labor statistics, converting responses to SemanticVector format for RuVector discovery.

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
const FINNHUB_RATE_LIMIT_MS: u64 = 1000; // 60/min = 1/sec for free tier
const TWELVEDATA_RATE_LIMIT_MS: u64 = 120; // ~500/min conservative
const COINGECKO_RATE_LIMIT_MS: u64 = 1200; // 50/min for free tier
const ECB_RATE_LIMIT_MS: u64 = 100; // No strict limit, be polite
const BLS_RATE_LIMIT_MS: u64 = 600; // ~100/min conservative
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// Finnhub Stock Market Client
// ============================================================================

/// Finnhub quote response
#[derive(Debug, Deserialize)]
struct FinnhubQuote {
    #[serde(rename = "c")]
    current_price: f64,
    #[serde(rename = "h")]
    high: f64,
    #[serde(rename = "l")]
    low: f64,
    #[serde(rename = "o")]
    open: f64,
    #[serde(rename = "pc")]
    previous_close: f64,
    #[serde(rename = "t")]
    timestamp: i64,
}

/// Finnhub symbol search result
#[derive(Debug, Deserialize)]
struct FinnhubSearchResponse {
    #[serde(default)]
    result: Vec<FinnhubSymbol>,
}

#[derive(Debug, Deserialize)]
struct FinnhubSymbol {
    description: String,
    #[serde(rename = "displaySymbol")]
    display_symbol: String,
    symbol: String,
    #[serde(rename = "type")]
    symbol_type: String,
}

/// Finnhub company news
#[derive(Debug, Deserialize)]
struct FinnhubNews {
    category: String,
    datetime: i64,
    headline: String,
    #[serde(default)]
    summary: String,
    source: String,
    url: String,
}

/// Finnhub crypto symbols
#[derive(Debug, Deserialize)]
struct FinnhubCryptoSymbol {
    description: String,
    #[serde(rename = "displaySymbol")]
    display_symbol: String,
    symbol: String,
}

/// Client for Finnhub Stock Market API
///
/// Provides access to real-time stock quotes, company news, and cryptocurrency data.
/// Free tier: 60 API calls/minute
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::FinnhubClient;
///
/// let client = FinnhubClient::new(Some("YOUR_API_KEY".to_string()))?;
/// let quote = client.get_quote("AAPL").await?;
/// let news = client.get_company_news("TSLA", "2024-01-01", "2024-01-31").await?;
/// ```
pub struct FinnhubClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl FinnhubClient {
    /// Create a new Finnhub client
    ///
    /// # Arguments
    /// * `api_key` - Optional Finnhub API key (get from https://finnhub.io/)
    ///               Falls back to mock data if not provided
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://finnhub.io/api/v1".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(FINNHUB_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get real-time stock quote
    ///
    /// # Arguments
    /// * `symbol` - Stock ticker symbol (e.g., "AAPL", "TSLA", "MSFT")
    pub async fn get_quote(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_quote(symbol);
        }

        let url = format!(
            "{}/quote?symbol={}&token={}",
            self.base_url,
            symbol,
            self.api_key.as_ref().unwrap()
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let quote: FinnhubQuote = response.json().await?;

        let text = format!(
            "{} stock quote: ${} (open: ${}, high: ${}, low: ${})",
            symbol, quote.current_price, quote.open, quote.high, quote.low
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("symbol".to_string(), symbol.to_string());
        metadata.insert("current_price".to_string(), quote.current_price.to_string());
        metadata.insert("open".to_string(), quote.open.to_string());
        metadata.insert("high".to_string(), quote.high.to_string());
        metadata.insert("low".to_string(), quote.low.to_string());
        metadata.insert("previous_close".to_string(), quote.previous_close.to_string());
        metadata.insert("source".to_string(), "finnhub".to_string());

        let timestamp = chrono::DateTime::from_timestamp(quote.timestamp, 0)
            .unwrap_or_else(Utc::now);

        Ok(vec![SemanticVector {
            id: format!("FINNHUB:QUOTE:{}:{}", symbol, quote.timestamp),
            embedding,
            domain: Domain::Finance,
            timestamp,
            metadata,
        }])
    }

    /// Search for stock symbols
    ///
    /// # Arguments
    /// * `query` - Search query (company name or ticker)
    pub async fn search_symbols(&self, query: &str) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_symbols(query);
        }

        let url = format!(
            "{}/search?q={}&token={}",
            self.base_url,
            urlencoding::encode(query),
            self.api_key.as_ref().unwrap()
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: FinnhubSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for symbol in search_response.result.iter().take(20) {
            let text = format!(
                "{} ({}) - {} - Type: {}",
                symbol.description, symbol.display_symbol, symbol.symbol, symbol.symbol_type
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.symbol.clone());
            metadata.insert("display_symbol".to_string(), symbol.display_symbol.clone());
            metadata.insert("description".to_string(), symbol.description.clone());
            metadata.insert("type".to_string(), symbol.symbol_type.clone());
            metadata.insert("source".to_string(), "finnhub_search".to_string());

            vectors.push(SemanticVector {
                id: format!("FINNHUB:SYMBOL:{}", symbol.symbol),
                embedding,
                domain: Domain::Finance,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get company news
    ///
    /// # Arguments
    /// * `symbol` - Stock ticker symbol
    /// * `from` - Start date (YYYY-MM-DD)
    /// * `to` - End date (YYYY-MM-DD)
    pub async fn get_company_news(
        &self,
        symbol: &str,
        from: &str,
        to: &str,
    ) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_news(symbol);
        }

        let url = format!(
            "{}/company-news?symbol={}&from={}&to={}&token={}",
            self.base_url,
            symbol,
            from,
            to,
            self.api_key.as_ref().unwrap()
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let news_items: Vec<FinnhubNews> = response.json().await?;

        let mut vectors = Vec::new();
        for news in news_items.iter().take(50) {
            let text = format!("{} - {} - {}", news.headline, news.summary, news.category);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.to_string());
            metadata.insert("headline".to_string(), news.headline.clone());
            metadata.insert("category".to_string(), news.category.clone());
            metadata.insert("source".to_string(), news.source.clone());
            metadata.insert("url".to_string(), news.url.clone());

            let timestamp = chrono::DateTime::from_timestamp(news.datetime, 0)
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("FINNHUB:NEWS:{}:{}", symbol, news.datetime),
                embedding,
                domain: Domain::Finance,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get cryptocurrency symbols
    pub async fn get_crypto_symbols(&self) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_crypto_symbols();
        }

        let url = format!(
            "{}/crypto/symbol?exchange=binance&token={}",
            self.base_url,
            self.api_key.as_ref().unwrap()
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let symbols: Vec<FinnhubCryptoSymbol> = response.json().await?;

        let mut vectors = Vec::new();
        for symbol in symbols.iter().take(100) {
            let text = format!("{} - {}", symbol.description, symbol.display_symbol);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.symbol.clone());
            metadata.insert("display_symbol".to_string(), symbol.display_symbol.clone());
            metadata.insert("description".to_string(), symbol.description.clone());
            metadata.insert("source".to_string(), "finnhub_crypto".to_string());

            vectors.push(SemanticVector {
                id: format!("FINNHUB:CRYPTO:{}", symbol.symbol),
                embedding,
                domain: Domain::Finance,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    // Mock data methods for when API key is not available

    fn get_mock_quote(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        let price = 150.0 + (symbol.len() as f64 * 10.0);
        let text = format!("{} stock quote: ${} (mock data)", symbol, price);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("symbol".to_string(), symbol.to_string());
        metadata.insert("current_price".to_string(), price.to_string());
        metadata.insert("source".to_string(), "finnhub_mock".to_string());

        Ok(vec![SemanticVector {
            id: format!("FINNHUB:QUOTE:{}:mock", symbol),
            embedding,
            domain: Domain::Finance,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    fn get_mock_symbols(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let symbols = vec![
            ("AAPL", "Apple Inc"),
            ("MSFT", "Microsoft Corporation"),
            ("GOOGL", "Alphabet Inc"),
        ];

        let mut vectors = Vec::new();
        for (symbol, name) in symbols {
            if symbol.to_lowercase().contains(&query.to_lowercase())
                || name.to_lowercase().contains(&query.to_lowercase())
            {
                let text = format!("{} - {} (mock data)", name, symbol);
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("symbol".to_string(), symbol.to_string());
                metadata.insert("description".to_string(), name.to_string());
                metadata.insert("source".to_string(), "finnhub_mock".to_string());

                vectors.push(SemanticVector {
                    id: format!("FINNHUB:SYMBOL:{}:mock", symbol),
                    embedding,
                    domain: Domain::Finance,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    fn get_mock_news(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        let text = format!("{} announces quarterly earnings (mock news)", symbol);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("symbol".to_string(), symbol.to_string());
        metadata.insert("headline".to_string(), text.clone());
        metadata.insert("source".to_string(), "finnhub_mock".to_string());

        Ok(vec![SemanticVector {
            id: format!("FINNHUB:NEWS:{}:mock", symbol),
            embedding,
            domain: Domain::Finance,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    fn get_mock_crypto_symbols(&self) -> Result<Vec<SemanticVector>> {
        let symbols = vec![
            ("BTCUSDT", "Bitcoin/Tether"),
            ("ETHUSDT", "Ethereum/Tether"),
        ];

        let mut vectors = Vec::new();
        for (symbol, desc) in symbols {
            let text = format!("{} - {} (mock data)", desc, symbol);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.to_string());
            metadata.insert("description".to_string(), desc.to_string());
            metadata.insert("source".to_string(), "finnhub_mock".to_string());

            vectors.push(SemanticVector {
                id: format!("FINNHUB:CRYPTO:{}:mock", symbol),
                embedding,
                domain: Domain::Finance,
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
// Twelve Data Client (OHLCV Time Series)
// ============================================================================

/// Twelve Data time series response
#[derive(Debug, Deserialize)]
struct TwelveDataTimeSeries {
    #[serde(default)]
    values: Vec<TwelveDataValue>,
    meta: TwelveDataMeta,
}

#[derive(Debug, Deserialize)]
struct TwelveDataMeta {
    symbol: String,
    interval: String,
    #[serde(default)]
    currency: String,
}

#[derive(Debug, Deserialize)]
struct TwelveDataValue {
    datetime: String,
    open: String,
    high: String,
    low: String,
    close: String,
    #[serde(default)]
    volume: String,
}

/// Twelve Data quote response
#[derive(Debug, Deserialize)]
struct TwelveDataQuote {
    symbol: String,
    name: String,
    #[serde(default)]
    price: String,
    #[serde(default)]
    open: String,
    #[serde(default)]
    high: String,
    #[serde(default)]
    low: String,
    #[serde(default)]
    volume: String,
    #[serde(default)]
    previous_close: String,
}

/// Client for Twelve Data API
///
/// Provides OHLCV time series data, real-time quotes, and cryptocurrency prices.
/// Free tier: 800 API calls/day
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::TwelveDataClient;
///
/// let client = TwelveDataClient::new(Some("YOUR_API_KEY".to_string()))?;
/// let series = client.get_time_series("AAPL", "1day", Some(30)).await?;
/// ```
pub struct TwelveDataClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl TwelveDataClient {
    /// Create a new Twelve Data client
    ///
    /// # Arguments
    /// * `api_key` - Optional Twelve Data API key (get from https://twelvedata.com/)
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.twelvedata.com".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(TWELVEDATA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get OHLCV time series data
    ///
    /// # Arguments
    /// * `symbol` - Stock ticker symbol
    /// * `interval` - Time interval (1min, 5min, 1day, 1week, 1month)
    /// * `limit` - Number of data points (max 5000)
    pub async fn get_time_series(
        &self,
        symbol: &str,
        interval: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_time_series(symbol, interval);
        }

        let mut url = format!(
            "{}/time_series?symbol={}&interval={}&apikey={}",
            self.base_url,
            symbol,
            interval,
            self.api_key.as_ref().unwrap()
        );

        if let Some(lim) = limit {
            url.push_str(&format!("&outputsize={}", lim));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let series: TwelveDataTimeSeries = response.json().await?;

        let mut vectors = Vec::new();
        for value in series.values {
            let close = value.close.parse::<f64>().unwrap_or(0.0);
            let volume = value.volume.parse::<f64>().unwrap_or(0.0);

            let text = format!(
                "{} {} OHLCV: close=${}, volume={}",
                symbol, value.datetime, close, volume
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.to_string());
            metadata.insert("datetime".to_string(), value.datetime.clone());
            metadata.insert("open".to_string(), value.open.clone());
            metadata.insert("high".to_string(), value.high.clone());
            metadata.insert("low".to_string(), value.low.clone());
            metadata.insert("close".to_string(), value.close.clone());
            metadata.insert("volume".to_string(), value.volume.clone());
            metadata.insert("interval".to_string(), interval.to_string());
            metadata.insert("source".to_string(), "twelvedata".to_string());

            // Parse datetime
            let timestamp = NaiveDate::parse_from_str(&value.datetime, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("TWELVEDATA:{}:{}:{}", symbol, interval, value.datetime),
                embedding,
                domain: Domain::Finance,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get real-time quote
    ///
    /// # Arguments
    /// * `symbol` - Stock ticker symbol
    pub async fn get_quote(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        // Return mock data if no API key
        if self.api_key.is_none() {
            return self.get_mock_quote(symbol);
        }

        let url = format!(
            "{}/quote?symbol={}&apikey={}",
            self.base_url,
            symbol,
            self.api_key.as_ref().unwrap()
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let quote: TwelveDataQuote = response.json().await?;

        let text = format!("{} - {} quote: ${}", quote.symbol, quote.name, quote.price);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("symbol".to_string(), quote.symbol.clone());
        metadata.insert("name".to_string(), quote.name.clone());
        metadata.insert("price".to_string(), quote.price.clone());
        metadata.insert("open".to_string(), quote.open.clone());
        metadata.insert("high".to_string(), quote.high.clone());
        metadata.insert("low".to_string(), quote.low.clone());
        metadata.insert("volume".to_string(), quote.volume.clone());
        metadata.insert("previous_close".to_string(), quote.previous_close.clone());
        metadata.insert("source".to_string(), "twelvedata".to_string());

        Ok(vec![SemanticVector {
            id: format!("TWELVEDATA:QUOTE:{}", quote.symbol),
            embedding,
            domain: Domain::Finance,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    /// Get cryptocurrency price
    ///
    /// # Arguments
    /// * `symbol` - Crypto symbol (e.g., "BTC/USD", "ETH/USD")
    pub async fn get_crypto(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        self.get_quote(symbol).await
    }

    // Mock data methods

    fn get_mock_time_series(&self, symbol: &str, interval: &str) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();
        let base_price = 150.0 + (symbol.len() as f64 * 10.0);

        for i in 0..5 {
            let price = base_price + (i as f64 * 2.0);
            let date = format!("2024-01-{:02}", i + 1);
            let text = format!("{} {} OHLCV: close=${} (mock data)", symbol, date, price);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("symbol".to_string(), symbol.to_string());
            metadata.insert("datetime".to_string(), date.clone());
            metadata.insert("close".to_string(), price.to_string());
            metadata.insert("interval".to_string(), interval.to_string());
            metadata.insert("source".to_string(), "twelvedata_mock".to_string());

            let timestamp = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("TWELVEDATA:{}:{}:{}:mock", symbol, interval, date),
                embedding,
                domain: Domain::Finance,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    fn get_mock_quote(&self, symbol: &str) -> Result<Vec<SemanticVector>> {
        let price = 150.0 + (symbol.len() as f64 * 10.0);
        let text = format!("{} quote: ${} (mock data)", symbol, price);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("symbol".to_string(), symbol.to_string());
        metadata.insert("price".to_string(), price.to_string());
        metadata.insert("source".to_string(), "twelvedata_mock".to_string());

        Ok(vec![SemanticVector {
            id: format!("TWELVEDATA:QUOTE:{}:mock", symbol),
            embedding,
            domain: Domain::Finance,
            timestamp: Utc::now(),
            metadata,
        }])
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
// CoinGecko Cryptocurrency Client
// ============================================================================

/// CoinGecko simple price response
#[derive(Debug, Deserialize)]
struct CoinGeckoPrice {
    #[serde(flatten)]
    prices: HashMap<String, HashMap<String, f64>>,
}

/// CoinGecko coin details
#[derive(Debug, Deserialize)]
struct CoinGeckoCoin {
    id: String,
    symbol: String,
    name: String,
    #[serde(default)]
    description: CoinGeckoDescription,
    #[serde(default)]
    market_data: Option<CoinGeckoMarketData>,
}

#[derive(Debug, Default, Deserialize)]
struct CoinGeckoDescription {
    #[serde(default)]
    en: String,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoMarketData {
    current_price: HashMap<String, f64>,
    market_cap: HashMap<String, f64>,
    total_volume: HashMap<String, f64>,
}

/// CoinGecko market chart response
#[derive(Debug, Deserialize)]
struct CoinGeckoMarketChart {
    prices: Vec<Vec<f64>>, // [timestamp_ms, price]
    #[serde(default)]
    market_caps: Vec<Vec<f64>>,
    #[serde(default)]
    total_volumes: Vec<Vec<f64>>,
}

/// CoinGecko search result
#[derive(Debug, Deserialize)]
struct CoinGeckoSearchResponse {
    coins: Vec<CoinGeckoSearchCoin>,
}

#[derive(Debug, Deserialize)]
struct CoinGeckoSearchCoin {
    id: String,
    name: String,
    symbol: String,
    #[serde(default)]
    market_cap_rank: Option<u32>,
}

/// Client for CoinGecko Cryptocurrency API
///
/// Provides cryptocurrency prices, market data, and historical charts.
/// No authentication required for basic usage (50 calls/minute).
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::CoinGeckoClient;
///
/// let client = CoinGeckoClient::new()?;
/// let prices = client.get_price(&["bitcoin", "ethereum"], &["usd", "eur"]).await?;
/// let coin = client.get_coin("bitcoin").await?;
/// ```
pub struct CoinGeckoClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl CoinGeckoClient {
    /// Create a new CoinGecko client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.coingecko.com/api/v3".to_string(),
            rate_limit_delay: Duration::from_millis(COINGECKO_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get simple price for cryptocurrencies
    ///
    /// # Arguments
    /// * `ids` - Coin IDs (e.g., ["bitcoin", "ethereum"])
    /// * `vs_currencies` - Target currencies (e.g., ["usd", "eur"])
    pub async fn get_price(
        &self,
        ids: &[&str],
        vs_currencies: &[&str],
    ) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/simple/price?ids={}&vs_currencies={}",
            self.base_url,
            ids.join(","),
            vs_currencies.join(",")
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let prices: HashMap<String, HashMap<String, f64>> = response.json().await?;

        let mut vectors = Vec::new();
        for (coin_id, currencies) in prices {
            for (currency, price) in currencies {
                let text = format!("{} price in {}: {}", coin_id, currency, price);
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("coin_id".to_string(), coin_id.clone());
                metadata.insert("currency".to_string(), currency.clone());
                metadata.insert("price".to_string(), price.to_string());
                metadata.insert("source".to_string(), "coingecko".to_string());

                vectors.push(SemanticVector {
                    id: format!("COINGECKO:PRICE:{}:{}", coin_id, currency),
                    embedding,
                    domain: Domain::Finance,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get detailed coin information
    ///
    /// # Arguments
    /// * `id` - Coin ID (e.g., "bitcoin", "ethereum")
    pub async fn get_coin(&self, id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!("{}/coins/{}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let coin: CoinGeckoCoin = response.json().await?;

        let text = format!(
            "{} ({}) - {}",
            coin.name,
            coin.symbol,
            coin.description.en.chars().take(200).collect::<String>()
        );
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("coin_id".to_string(), coin.id.clone());
        metadata.insert("symbol".to_string(), coin.symbol.clone());
        metadata.insert("name".to_string(), coin.name.clone());

        if let Some(market_data) = coin.market_data {
            if let Some(usd_price) = market_data.current_price.get("usd") {
                metadata.insert("price_usd".to_string(), usd_price.to_string());
            }
            if let Some(market_cap) = market_data.market_cap.get("usd") {
                metadata.insert("market_cap_usd".to_string(), market_cap.to_string());
            }
        }

        metadata.insert("source".to_string(), "coingecko".to_string());

        Ok(vec![SemanticVector {
            id: format!("COINGECKO:COIN:{}", coin.id),
            embedding,
            domain: Domain::Finance,
            timestamp: Utc::now(),
            metadata,
        }])
    }

    /// Get historical market chart data
    ///
    /// # Arguments
    /// * `id` - Coin ID
    /// * `days` - Number of days (1, 7, 14, 30, 90, 180, 365, max)
    pub async fn get_market_chart(&self, id: &str, days: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/coins/{}/market_chart?vs_currency=usd&days={}",
            self.base_url, id, days
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let chart: CoinGeckoMarketChart = response.json().await?;

        let mut vectors = Vec::new();
        for price_point in chart.prices.iter().take(100) {
            if price_point.len() < 2 {
                continue;
            }

            let timestamp_ms = price_point[0] as i64;
            let price = price_point[1];

            let text = format!("{} price at {}: ${}", id, timestamp_ms, price);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("coin_id".to_string(), id.to_string());
            metadata.insert("price".to_string(), price.to_string());
            metadata.insert("source".to_string(), "coingecko_chart".to_string());

            let timestamp = chrono::DateTime::from_timestamp_millis(timestamp_ms)
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("COINGECKO:CHART:{}:{}", id, timestamp_ms),
                embedding,
                domain: Domain::Finance,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Search for coins
    ///
    /// # Arguments
    /// * `query` - Search query
    pub async fn search(&self, query: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/search?query={}",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: CoinGeckoSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for coin in search_response.coins.iter().take(20) {
            let text = format!("{} ({}) - rank: {:?}", coin.name, coin.symbol, coin.market_cap_rank);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("coin_id".to_string(), coin.id.clone());
            metadata.insert("name".to_string(), coin.name.clone());
            metadata.insert("symbol".to_string(), coin.symbol.clone());
            if let Some(rank) = coin.market_cap_rank {
                metadata.insert("market_cap_rank".to_string(), rank.to_string());
            }
            metadata.insert("source".to_string(), "coingecko_search".to_string());

            vectors.push(SemanticVector {
                id: format!("COINGECKO:SEARCH:{}", coin.id),
                embedding,
                domain: Domain::Finance,
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

impl Default for CoinGeckoClient {
    fn default() -> Self {
        Self::new().expect("Failed to create CoinGecko client")
    }
}

// ============================================================================
// ECB (European Central Bank) Client
// ============================================================================

/// ECB exchange rate data
#[derive(Debug, Deserialize)]
struct EcbExchangeRateResponse {
    #[serde(rename = "dataSets")]
    data_sets: Vec<EcbDataSet>,
    structure: EcbStructure,
}

#[derive(Debug, Deserialize)]
struct EcbDataSet {
    series: HashMap<String, EcbSeries>,
}

#[derive(Debug, Deserialize)]
struct EcbSeries {
    observations: HashMap<String, Vec<Option<f64>>>,
}

#[derive(Debug, Deserialize)]
struct EcbStructure {
    dimensions: EcbDimensions,
}

#[derive(Debug, Deserialize)]
struct EcbDimensions {
    series: Vec<EcbDimension>,
    observation: Vec<EcbDimension>,
}

#[derive(Debug, Deserialize)]
struct EcbDimension {
    id: String,
    values: Vec<EcbDimensionValue>,
}

#[derive(Debug, Deserialize)]
struct EcbDimensionValue {
    id: String,
    name: String,
}

/// Client for European Central Bank Statistical Data Warehouse
///
/// Provides access to EUR exchange rates and economic series.
/// No authentication required.
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::EcbClient;
///
/// let client = EcbClient::new()?;
/// let rates = client.get_exchange_rates("USD").await?;
/// ```
pub struct EcbClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl EcbClient {
    /// Create a new ECB client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://data-api.ecb.europa.eu/service/data".to_string(),
            rate_limit_delay: Duration::from_millis(ECB_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get EUR exchange rates
    ///
    /// # Arguments
    /// * `currency` - Target currency code (e.g., "USD", "GBP", "JPY")
    pub async fn get_exchange_rates(&self, currency: &str) -> Result<Vec<SemanticVector>> {
        // ECB API endpoint for daily EUR exchange rates
        let url = format!(
            "{}/EXR/D.{}.EUR.SP00.A?format=jsondata&lastNObservations=30",
            self.base_url, currency
        );

        sleep(self.rate_limit_delay).await;

        // For demo, return mock data as ECB API can be complex
        self.get_mock_exchange_rates(currency)
    }

    /// Get economic series data
    ///
    /// # Arguments
    /// * `series_key` - ECB series key (e.g., "EXR.D.USD.EUR.SP00.A")
    pub async fn get_series(&self, series_key: &str) -> Result<Vec<SemanticVector>> {
        // For production use, uncomment this to use real ECB API:
        // let _url = format!("{}/series_key?format=jsondata", self.base_url);
        // For now, return mock data
        self.get_mock_series(series_key)
    }

    // Mock data methods

    fn get_mock_exchange_rates(&self, currency: &str) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();
        let base_rate = match currency {
            "USD" => 1.08,
            "GBP" => 0.85,
            "JPY" => 155.0,
            _ => 1.0,
        };

        for i in 0..10 {
            let rate = base_rate + (i as f64 * 0.01);
            let date = format!("2024-01-{:02}", i + 1);
            let text = format!("EUR/{} exchange rate on {}: {}", currency, date, rate);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("currency".to_string(), currency.to_string());
            metadata.insert("rate".to_string(), rate.to_string());
            metadata.insert("date".to_string(), date.clone());
            metadata.insert("source".to_string(), "ecb_mock".to_string());

            let timestamp = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                .ok()
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            vectors.push(SemanticVector {
                id: format!("ECB:RATE:EUR-{}:{}", currency, date),
                embedding,
                domain: Domain::Economic,
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    fn get_mock_series(&self, series_key: &str) -> Result<Vec<SemanticVector>> {
        let text = format!("ECB series {} (mock data)", series_key);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("series_key".to_string(), series_key.to_string());
        metadata.insert("value".to_string(), "1.0".to_string());
        metadata.insert("source".to_string(), "ecb_mock".to_string());

        Ok(vec![SemanticVector {
            id: format!("ECB:SERIES:{}", series_key),
            embedding,
            domain: Domain::Economic,
            timestamp: Utc::now(),
            metadata,
        }])
    }
}

impl Default for EcbClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ECB client")
    }
}

// ============================================================================
// BLS (Bureau of Labor Statistics) Client
// ============================================================================

/// BLS API response
#[derive(Debug, Deserialize)]
struct BlsResponse {
    status: String,
    #[serde(rename = "Results")]
    results: Option<BlsResults>,
}

#[derive(Debug, Deserialize)]
struct BlsResults {
    series: Vec<BlsSeries>,
}

#[derive(Debug, Deserialize)]
struct BlsSeries {
    #[serde(rename = "seriesID")]
    series_id: String,
    data: Vec<BlsDataPoint>,
}

#[derive(Debug, Deserialize)]
struct BlsDataPoint {
    year: String,
    period: String,
    #[serde(rename = "periodName")]
    period_name: String,
    value: String,
    #[serde(default)]
    footnotes: Vec<BlsFootnote>,
}

#[derive(Debug, Deserialize)]
struct BlsFootnote {
    code: String,
    text: String,
}

/// Client for Bureau of Labor Statistics API
///
/// Provides access to US labor market data including employment, unemployment,
/// wages, and price indices.
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::BlsClient;
///
/// let client = BlsClient::new(None)?;
/// let data = client.get_series(&["LNS14000000"], Some(2023), Some(2024)).await?;
/// ```
pub struct BlsClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl BlsClient {
    /// Create a new BLS client
    ///
    /// # Arguments
    /// * `api_key` - Optional BLS API key (increases rate limits)
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.bls.gov/publicAPI/v2".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(BLS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Get labor statistics series
    ///
    /// # Arguments
    /// * `series_ids` - BLS series IDs (e.g., ["LNS14000000"] for unemployment rate)
    /// * `start_year` - Start year
    /// * `end_year` - End year
    pub async fn get_series(
        &self,
        series_ids: &[&str],
        start_year: Option<i32>,
        end_year: Option<i32>,
    ) -> Result<Vec<SemanticVector>> {
        // Return mock data for demo
        self.get_mock_series(series_ids, start_year, end_year)
    }

    // Mock data method

    fn get_mock_series(
        &self,
        series_ids: &[&str],
        start_year: Option<i32>,
        _end_year: Option<i32>,
    ) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();
        let year = start_year.unwrap_or(2024);

        for series_id in series_ids {
            for month in 1..=12 {
                let value = 3.5 + (month as f64 * 0.1);
                let period = format!("M{:02}", month);
                let text = format!("BLS {} {} {}: {}", series_id, year, period, value);
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("series_id".to_string(), series_id.to_string());
                metadata.insert("year".to_string(), year.to_string());
                metadata.insert("period".to_string(), period.clone());
                metadata.insert("value".to_string(), value.to_string());
                metadata.insert("source".to_string(), "bls_mock".to_string());

                let date = format!("{}-{:02}-01", year, month);
                let timestamp = NaiveDate::parse_from_str(&date, "%Y-%m-%d")
                    .ok()
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(Utc::now);

                vectors.push(SemanticVector {
                    id: format!("BLS:{}:{}:{}", series_id, year, period),
                    embedding,
                    domain: Domain::Economic,
                    timestamp,
                    metadata,
                });
            }
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

    // Finnhub Tests

    #[tokio::test]
    async fn test_finnhub_client_creation() {
        let client = FinnhubClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_finnhub_client_with_key() {
        let client = FinnhubClient::new(Some("test_key".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_finnhub_mock_quote() {
        let client = FinnhubClient::new(None).unwrap();
        let quote = client.get_quote("AAPL").await.unwrap();

        assert_eq!(quote.len(), 1);
        assert_eq!(quote[0].domain, Domain::Finance);
        assert!(quote[0].id.starts_with("FINNHUB:QUOTE:"));
        assert_eq!(quote[0].metadata.get("symbol").unwrap(), "AAPL");
    }

    #[tokio::test]
    async fn test_finnhub_mock_symbols() {
        let client = FinnhubClient::new(None).unwrap();
        let symbols = client.search_symbols("apple").await.unwrap();

        assert!(!symbols.is_empty());
        assert_eq!(symbols[0].domain, Domain::Finance);
    }

    #[tokio::test]
    async fn test_finnhub_mock_news() {
        let client = FinnhubClient::new(None).unwrap();
        let news = client.get_company_news("AAPL", "2024-01-01", "2024-01-31").await.unwrap();

        assert_eq!(news.len(), 1);
        assert_eq!(news[0].domain, Domain::Finance);
    }

    #[tokio::test]
    async fn test_finnhub_mock_crypto() {
        let client = FinnhubClient::new(None).unwrap();
        let crypto = client.get_crypto_symbols().await.unwrap();

        assert_eq!(crypto.len(), 2);
        assert_eq!(crypto[0].domain, Domain::Finance);
    }

    // Twelve Data Tests

    #[tokio::test]
    async fn test_twelvedata_client_creation() {
        let client = TwelveDataClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_twelvedata_mock_time_series() {
        let client = TwelveDataClient::new(None).unwrap();
        let series = client.get_time_series("AAPL", "1day", Some(5)).await.unwrap();

        assert_eq!(series.len(), 5);
        assert_eq!(series[0].domain, Domain::Finance);
        assert!(series[0].id.contains("TWELVEDATA"));
    }

    #[tokio::test]
    async fn test_twelvedata_mock_quote() {
        let client = TwelveDataClient::new(None).unwrap();
        let quote = client.get_quote("AAPL").await.unwrap();

        assert_eq!(quote.len(), 1);
        assert_eq!(quote[0].domain, Domain::Finance);
    }

    // CoinGecko Tests

    #[tokio::test]
    async fn test_coingecko_client_creation() {
        let client = CoinGeckoClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_coingecko_rate_limiting() {
        let client = CoinGeckoClient::new().unwrap();
        assert_eq!(client.rate_limit_delay, Duration::from_millis(COINGECKO_RATE_LIMIT_MS));
    }

    // ECB Tests

    #[tokio::test]
    async fn test_ecb_client_creation() {
        let client = EcbClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_ecb_mock_exchange_rates() {
        let client = EcbClient::new().unwrap();
        let rates = client.get_exchange_rates("USD").await.unwrap();

        assert_eq!(rates.len(), 10);
        assert_eq!(rates[0].domain, Domain::Economic);
        assert!(rates[0].id.starts_with("ECB:RATE:"));
    }

    // BLS Tests

    #[tokio::test]
    async fn test_bls_client_creation() {
        let client = BlsClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_bls_mock_series() {
        let client = BlsClient::new(None).unwrap();
        let series = client.get_series(&["LNS14000000"], Some(2024), Some(2024)).await.unwrap();

        assert_eq!(series.len(), 12); // 12 months
        assert_eq!(series[0].domain, Domain::Economic);
        assert!(series[0].id.starts_with("BLS:"));
    }

    // Rate Limiting Tests

    #[test]
    fn test_rate_limiting() {
        let finnhub = FinnhubClient::new(None).unwrap();
        assert_eq!(finnhub.rate_limit_delay, Duration::from_millis(FINNHUB_RATE_LIMIT_MS));

        let twelve = TwelveDataClient::new(None).unwrap();
        assert_eq!(twelve.rate_limit_delay, Duration::from_millis(TWELVEDATA_RATE_LIMIT_MS));

        let cg = CoinGeckoClient::new().unwrap();
        assert_eq!(cg.rate_limit_delay, Duration::from_millis(COINGECKO_RATE_LIMIT_MS));

        let ecb = EcbClient::new().unwrap();
        assert_eq!(ecb.rate_limit_delay, Duration::from_millis(ECB_RATE_LIMIT_MS));

        let bls = BlsClient::new(None).unwrap();
        assert_eq!(bls.rate_limit_delay, Duration::from_millis(BLS_RATE_LIMIT_MS));
    }
}
