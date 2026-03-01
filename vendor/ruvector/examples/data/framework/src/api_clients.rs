//! Real API client integrations for OpenAlex, NOAA, and SEC EDGAR
//!
//! This module provides async clients for fetching data from public APIs
//! and converting responses into RuVector's DataRecord format with embeddings.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::{DataRecord, DataSource, FrameworkError, Relationship, Result};

/// Rate limiting configuration
const DEFAULT_RATE_LIMIT_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// Simple Embedding Generator
// ============================================================================

/// Simple bag-of-words embedding generator
pub struct SimpleEmbedder {
    dimension: usize,
}

impl SimpleEmbedder {
    /// Create a new embedder with specified dimension
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }

    /// Generate embedding from text using simple bag-of-words
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        let lowercase_text = text.to_lowercase();
        let words: Vec<&str> = lowercase_text
            .split_whitespace()
            .filter(|w| w.len() > 2)
            .collect();

        let mut embedding = vec![0.0f32; self.dimension];

        // Simple hash-based bag-of-words
        for word in words {
            let hash = self.hash_word(word);
            let idx = (hash % self.dimension as u64) as usize;
            embedding[idx] += 1.0;
        }

        // Normalize
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in &mut embedding {
                *val /= magnitude;
            }
        }

        embedding
    }

    /// Simple hash function for words
    fn hash_word(&self, word: &str) -> u64 {
        let mut hash = 5381u64;
        for byte in word.bytes() {
            hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
        }
        hash
    }

    /// Generate embedding from JSON value
    pub fn embed_json(&self, value: &serde_json::Value) -> Vec<f32> {
        let text = self.extract_text_from_json(value);
        self.embed_text(&text)
    }

    /// Extract text content from JSON
    fn extract_text_from_json(&self, value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Object(map) => {
                let mut text = String::new();
                for (key, val) in map {
                    text.push_str(key);
                    text.push(' ');
                    text.push_str(&self.extract_text_from_json(val));
                    text.push(' ');
                }
                text
            }
            serde_json::Value::Array(arr) => {
                arr.iter()
                    .map(|v| self.extract_text_from_json(v))
                    .collect::<Vec<_>>()
                    .join(" ")
            }
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => String::new(),
        }
    }
}

// ============================================================================
// ONNX Semantic Embedder (Optional Feature)
// ============================================================================

/// ONNX-based semantic embedder for high-quality embeddings
/// Requires the `onnx-embeddings` feature flag
#[cfg(feature = "onnx-embeddings")]
pub struct OnnxEmbedder {
    embedder: std::sync::RwLock<ruvector_onnx_embeddings::Embedder>,
}

#[cfg(feature = "onnx-embeddings")]
impl OnnxEmbedder {
    /// Create a new ONNX embedder with the default model (all-MiniLM-L6-v2)
    pub async fn new() -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let embedder = ruvector_onnx_embeddings::Embedder::default_model().await?;
        Ok(Self {
            embedder: std::sync::RwLock::new(embedder),
        })
    }

    /// Create with a specific pretrained model
    pub async fn with_model(
        model: ruvector_onnx_embeddings::PretrainedModel,
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let embedder = ruvector_onnx_embeddings::Embedder::pretrained(model).await?;
        Ok(Self {
            embedder: std::sync::RwLock::new(embedder),
        })
    }

    /// Generate semantic embedding from text
    pub fn embed_text(&self, text: &str) -> Vec<f32> {
        let mut embedder = self.embedder.write().unwrap();
        embedder.embed_one(text).unwrap_or_else(|_| vec![0.0; 384])
    }

    /// Generate embeddings for multiple texts (batch processing)
    pub fn embed_batch(&self, texts: &[&str]) -> Vec<Vec<f32>> {
        let mut embedder = self.embedder.write().unwrap();
        match embedder.embed(texts) {
            Ok(output) => (0..texts.len())
                .map(|i| output.get(i).unwrap_or(&vec![0.0; 384]).clone())
                .collect(),
            Err(_) => texts.iter().map(|_| vec![0.0; 384]).collect(),
        }
    }

    /// Generate embeddings in optimized chunks (for large batches)
    ///
    /// Processes texts in chunks of `batch_size` to:
    /// - Reduce memory pressure
    /// - Enable better GPU/CPU utilization
    /// - Allow progress tracking
    ///
    /// # Arguments
    /// * `texts` - Input texts to embed
    /// * `batch_size` - Number of texts per batch (default: 32)
    ///
    /// # Returns
    /// Vector of embeddings in the same order as input texts
    pub fn embed_batch_chunked(&self, texts: &[&str], batch_size: usize) -> Vec<Vec<f32>> {
        let batch_size = batch_size.max(1);
        let dim = self.dimension();
        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(batch_size) {
            let chunk_embeddings = self.embed_batch(chunk);
            all_embeddings.extend(chunk_embeddings);
        }

        // Ensure we have the right number of embeddings
        while all_embeddings.len() < texts.len() {
            all_embeddings.push(vec![0.0; dim]);
        }

        all_embeddings
    }

    /// Generate embeddings with progress callback (for large datasets)
    ///
    /// # Arguments
    /// * `texts` - Input texts to embed
    /// * `batch_size` - Number of texts per batch
    /// * `progress_fn` - Callback called with (processed, total) after each batch
    pub fn embed_batch_with_progress<F>(
        &self,
        texts: &[&str],
        batch_size: usize,
        mut progress_fn: F,
    ) -> Vec<Vec<f32>>
    where
        F: FnMut(usize, usize),
    {
        let batch_size = batch_size.max(1);
        let total = texts.len();
        let dim = self.dimension();
        let mut all_embeddings = Vec::with_capacity(total);
        let mut processed = 0;

        for chunk in texts.chunks(batch_size) {
            let chunk_embeddings = self.embed_batch(chunk);
            all_embeddings.extend(chunk_embeddings);
            processed += chunk.len();
            progress_fn(processed, total);
        }

        // Ensure we have the right number of embeddings
        while all_embeddings.len() < total {
            all_embeddings.push(vec![0.0; dim]);
        }

        all_embeddings
    }

    /// Get the embedding dimension (384 for MiniLM, 768 for larger models)
    pub fn dimension(&self) -> usize {
        let embedder = self.embedder.read().unwrap();
        embedder.dimension()
    }

    /// Compute cosine similarity between two texts
    pub fn similarity(&self, text1: &str, text2: &str) -> f32 {
        let mut embedder = self.embedder.write().unwrap();
        embedder.similarity(text1, text2).unwrap_or(0.0)
    }

    /// Generate embedding from JSON value by extracting text
    pub fn embed_json(&self, value: &serde_json::Value) -> Vec<f32> {
        let text = extract_text_from_json(value);
        self.embed_text(&text)
    }
}

/// Helper to extract text from JSON (used by both embedders)
fn extract_text_from_json(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Object(map) => {
            let mut text = String::new();
            for (key, val) in map {
                text.push_str(key);
                text.push(' ');
                text.push_str(&extract_text_from_json(val));
                text.push(' ');
            }
            text
        }
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(|v| extract_text_from_json(v))
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Null => String::new(),
    }
}

/// Unified embedder trait for both SimpleEmbedder and OnnxEmbedder
pub trait Embedder: Send + Sync {
    /// Generate embedding from text
    fn embed(&self, text: &str) -> Vec<f32>;
    /// Get embedding dimension
    fn dim(&self) -> usize;
}

impl Embedder for SimpleEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_text(text)
    }
    fn dim(&self) -> usize {
        self.dimension
    }
}

#[cfg(feature = "onnx-embeddings")]
impl Embedder for OnnxEmbedder {
    fn embed(&self, text: &str) -> Vec<f32> {
        self.embed_text(text)
    }
    fn dim(&self) -> usize {
        self.dimension()
    }
}

// ============================================================================
// OpenAlex API Client
// ============================================================================

/// OpenAlex API response for works search
#[derive(Debug, Deserialize)]
struct OpenAlexWorksResponse {
    results: Vec<OpenAlexWork>,
    meta: OpenAlexMeta,
}

#[derive(Debug, Deserialize)]
struct OpenAlexWork {
    id: String,
    title: Option<String>,
    #[serde(rename = "display_name")]
    display_name: Option<String>,
    publication_date: Option<String>,
    #[serde(rename = "authorships")]
    authorships: Option<Vec<OpenAlexAuthorship>>,
    #[serde(rename = "cited_by_count")]
    cited_by_count: Option<i64>,
    #[serde(rename = "concepts")]
    concepts: Option<Vec<OpenAlexConcept>>,
    #[serde(rename = "abstract_inverted_index")]
    abstract_inverted_index: Option<HashMap<String, Vec<i32>>>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthorship {
    author: Option<OpenAlexAuthor>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthor {
    id: String,
    #[serde(rename = "display_name")]
    display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexConcept {
    id: String,
    #[serde(rename = "display_name")]
    display_name: Option<String>,
    score: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexMeta {
    count: i64,
}

/// OpenAlex topics response
#[derive(Debug, Deserialize)]
struct OpenAlexTopicsResponse {
    results: Vec<OpenAlexTopic>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexTopic {
    id: String,
    #[serde(rename = "display_name")]
    display_name: String,
    description: Option<String>,
    #[serde(rename = "works_count")]
    works_count: Option<i64>,
}

/// Client for OpenAlex academic database
pub struct OpenAlexClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    user_email: Option<String>,
}

impl OpenAlexClient {
    /// Create a new OpenAlex client
    ///
    /// # Arguments
    /// * `user_email` - Email for polite API usage (optional but recommended)
    pub fn new(user_email: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://api.openalex.org".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(128)),
            user_email,
        })
    }

    /// Fetch academic works by query
    ///
    /// # Arguments
    /// * `query` - Search query (title, abstract, etc.)
    /// * `limit` - Maximum number of results
    pub async fn fetch_works(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut url = format!("{}/works?search={}", self.base_url, urlencoding::encode(query));
        url.push_str(&format!("&per-page={}", limit.min(200)));

        if let Some(email) = &self.user_email {
            url.push_str(&format!("&mailto={}", email));
        }

        let response = self.fetch_with_retry(&url).await?;
        let works_response: OpenAlexWorksResponse = response.json().await?;

        let mut records = Vec::new();
        for work in works_response.results {
            let record = self.work_to_record(work)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Fetch topics by domain
    pub async fn fetch_topics(&self, domain: &str) -> Result<Vec<DataRecord>> {
        let mut url = format!(
            "{}/topics?search={}",
            self.base_url,
            urlencoding::encode(domain)
        );
        url.push_str("&per-page=50");

        if let Some(email) = &self.user_email {
            url.push_str(&format!("&mailto={}", email));
        }

        let response = self.fetch_with_retry(&url).await?;
        let topics_response: OpenAlexTopicsResponse = response.json().await?;

        let mut records = Vec::new();
        for topic in topics_response.results {
            let record = self.topic_to_record(topic)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Convert OpenAlex work to DataRecord
    fn work_to_record(&self, work: OpenAlexWork) -> Result<DataRecord> {
        let title = work
            .display_name
            .or(work.title)
            .unwrap_or_else(|| "Untitled".to_string());

        // Reconstruct abstract from inverted index
        let abstract_text = work
            .abstract_inverted_index
            .as_ref()
            .map(|index| self.reconstruct_abstract(index))
            .unwrap_or_default();

        // Create text for embedding
        let text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&text);

        // Parse publication date
        let timestamp = work
            .publication_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .unwrap_or_else(Utc::now);

        // Build relationships
        let mut relationships = Vec::new();

        // Author relationships
        if let Some(authorships) = work.authorships {
            for authorship in authorships {
                if let Some(author) = authorship.author {
                    relationships.push(Relationship {
                        target_id: author.id,
                        rel_type: "authored_by".to_string(),
                        weight: 1.0,
                        properties: {
                            let mut props = HashMap::new();
                            if let Some(name) = author.display_name {
                                props.insert("author_name".to_string(), serde_json::json!(name));
                            }
                            props
                        },
                    });
                }
            }
        }

        // Concept relationships
        if let Some(concepts) = work.concepts {
            for concept in concepts {
                relationships.push(Relationship {
                    target_id: concept.id,
                    rel_type: "has_concept".to_string(),
                    weight: concept.score.unwrap_or(0.0),
                    properties: {
                        let mut props = HashMap::new();
                        if let Some(name) = concept.display_name {
                            props.insert("concept_name".to_string(), serde_json::json!(name));
                        }
                        props
                    },
                });
            }
        }

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(title));
        data_map.insert("abstract".to_string(), serde_json::json!(abstract_text));
        if let Some(citations) = work.cited_by_count {
            data_map.insert("citations".to_string(), serde_json::json!(citations));
        }

        Ok(DataRecord {
            id: work.id,
            source: "openalex".to_string(),
            record_type: "work".to_string(),
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
        })
    }

    /// Reconstruct abstract from inverted index
    fn reconstruct_abstract(&self, inverted_index: &HashMap<String, Vec<i32>>) -> String {
        let mut positions: Vec<(i32, String)> = Vec::new();
        for (word, indices) in inverted_index {
            for &pos in indices {
                positions.push((pos, word.clone()));
            }
        }
        positions.sort_by_key(|&(pos, _)| pos);
        positions
            .into_iter()
            .map(|(_, word)| word)
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Convert topic to DataRecord
    fn topic_to_record(&self, topic: OpenAlexTopic) -> Result<DataRecord> {
        let text = format!(
            "{} {}",
            topic.display_name,
            topic.description.as_deref().unwrap_or("")
        );
        let embedding = self.embedder.embed_text(&text);

        let mut data_map = serde_json::Map::new();
        data_map.insert(
            "display_name".to_string(),
            serde_json::json!(topic.display_name),
        );
        if let Some(desc) = topic.description {
            data_map.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(count) = topic.works_count {
            data_map.insert("works_count".to_string(), serde_json::json!(count));
        }

        Ok(DataRecord {
            id: topic.id,
            source: "openalex".to_string(),
            record_type: "topic".to_string(),
            timestamp: Utc::now(),
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships: Vec::new(),
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES
                    {
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

#[async_trait]
impl DataSource for OpenAlexClient {
    fn source_id(&self) -> &str {
        "openalex"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Default to fetching works about "machine learning"
        let query = cursor.as_deref().unwrap_or("machine learning");
        let records = self.fetch_works(query, batch_size).await?;
        Ok((records, None)) // No pagination cursor in this simple impl
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client.get(&self.base_url).send().await?;
        Ok(response.status().is_success())
    }
}

// ============================================================================
// NOAA Climate Data Client
// ============================================================================

/// NOAA NCDC API response
#[derive(Debug, Deserialize)]
struct NoaaResponse {
    results: Vec<NoaaObservation>,
}

#[derive(Debug, Deserialize)]
struct NoaaObservation {
    station: String,
    date: String,
    datatype: String,
    value: f64,
    #[serde(default)]
    attributes: String,
}

/// Client for NOAA climate data
pub struct NoaaClient {
    client: Client,
    base_url: String,
    api_token: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl NoaaClient {
    /// Create a new NOAA client
    ///
    /// # Arguments
    /// * `api_token` - NOAA API token (get from https://www.ncdc.noaa.gov/cdo-web/token)
    pub fn new(api_token: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://www.ncei.noaa.gov/cdo-web/api/v2".to_string(),
            api_token,
            rate_limit_delay: Duration::from_millis(200), // NOAA has stricter limits
            embedder: Arc::new(SimpleEmbedder::new(128)),
        })
    }

    /// Fetch climate data for a station
    ///
    /// # Arguments
    /// * `station_id` - GHCND station ID (e.g., "GHCND:USW00094728" for NYC)
    /// * `start_date` - Start date (YYYY-MM-DD)
    /// * `end_date` - End date (YYYY-MM-DD)
    pub async fn fetch_climate_data(
        &self,
        station_id: &str,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<DataRecord>> {
        if self.api_token.is_none() {
            // If no API token, return synthetic data for demo
            return Ok(self.generate_synthetic_climate_data(station_id, start_date, end_date)?);
        }

        let url = format!(
            "{}/data?datasetid=GHCND&stationid={}&startdate={}&enddate={}&limit=1000",
            self.base_url, station_id, start_date, end_date
        );

        let mut request = self.client.get(&url);
        if let Some(token) = &self.api_token {
            request = request.header("token", token);
        }

        let response = self.fetch_with_retry(request).await?;
        let noaa_response: NoaaResponse = response.json().await?;

        let mut records = Vec::new();
        for observation in noaa_response.results {
            let record = self.observation_to_record(observation)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Generate synthetic climate data for demo purposes
    fn generate_synthetic_climate_data(
        &self,
        station_id: &str,
        start_date: &str,
        _end_date: &str,
    ) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();
        let datatypes = vec!["TMAX", "TMIN", "PRCP"];

        // Generate a few synthetic observations
        for (i, datatype) in datatypes.iter().enumerate() {
            let value = match *datatype {
                "TMAX" => 250.0 + (i as f64 * 10.0),
                "TMIN" => 150.0 + (i as f64 * 10.0),
                "PRCP" => 5.0 + (i as f64),
                _ => 0.0,
            };

            let text = format!("{} {} {}", station_id, datatype, value);
            let embedding = self.embedder.embed_text(&text);

            let mut data_map = serde_json::Map::new();
            data_map.insert("station".to_string(), serde_json::json!(station_id));
            data_map.insert("datatype".to_string(), serde_json::json!(datatype));
            data_map.insert("value".to_string(), serde_json::json!(value));
            data_map.insert("unit".to_string(), serde_json::json!("tenths"));

            records.push(DataRecord {
                id: format!("{}_{}_{}_{}", station_id, datatype, start_date, i),
                source: "noaa".to_string(),
                record_type: "observation".to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }

        Ok(records)
    }

    /// Convert NOAA observation to DataRecord
    fn observation_to_record(&self, obs: NoaaObservation) -> Result<DataRecord> {
        let text = format!("{} {} {}", obs.station, obs.datatype, obs.value);
        let embedding = self.embedder.embed_text(&text);

        // Parse date
        let timestamp = NaiveDate::parse_from_str(&obs.date, "%Y-%m-%dT%H:%M:%S")
            .or_else(|_| NaiveDate::parse_from_str(&obs.date, "%Y-%m-%d"))
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("station".to_string(), serde_json::json!(obs.station));
        data_map.insert("datatype".to_string(), serde_json::json!(obs.datatype));
        data_map.insert("value".to_string(), serde_json::json!(obs.value));
        data_map.insert("attributes".to_string(), serde_json::json!(obs.attributes));

        Ok(DataRecord {
            id: format!("{}_{}", obs.station, obs.date),
            source: "noaa".to_string(),
            record_type: "observation".to_string(),
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships: Vec::new(),
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, request: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let req = request
                .try_clone()
                .ok_or_else(|| FrameworkError::Config("Failed to clone request".to_string()))?;

            match req.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES
                    {
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

#[async_trait]
impl DataSource for NoaaClient {
    fn source_id(&self) -> &str {
        "noaa"
    }

    async fn fetch_batch(
        &self,
        _cursor: Option<String>,
        _batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Fetch sample climate data
        let records = self
            .fetch_climate_data("GHCND:USW00094728", "2024-01-01", "2024-01-31")
            .await?;
        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true) // NOAA doesn't have a simple health endpoint
    }
}

// ============================================================================
// SEC EDGAR Client
// ============================================================================

/// SEC EDGAR filing metadata
#[derive(Debug, Deserialize)]
struct EdgarFilingData {
    #[serde(default)]
    filings: EdgarFilings,
}

#[derive(Debug, Default, Deserialize)]
struct EdgarFilings {
    #[serde(default)]
    recent: EdgarRecent,
}

#[derive(Debug, Default, Deserialize)]
struct EdgarRecent {
    #[serde(rename = "accessionNumber", default)]
    accession_number: Vec<String>,
    #[serde(rename = "filingDate", default)]
    filing_date: Vec<String>,
    #[serde(rename = "reportDate", default)]
    report_date: Vec<String>,
    #[serde(default)]
    form: Vec<String>,
    #[serde(rename = "primaryDocument", default)]
    primary_document: Vec<String>,
}

/// Client for SEC EDGAR filings
pub struct EdgarClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    user_agent: String,
}

impl EdgarClient {
    /// Create a new SEC EDGAR client
    ///
    /// # Arguments
    /// * `user_agent` - User agent string (required by SEC, should include email)
    pub fn new(user_agent: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent(&user_agent)
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://data.sec.gov".to_string(),
            rate_limit_delay: Duration::from_millis(100), // SEC requires 10 requests/second max
            embedder: Arc::new(SimpleEmbedder::new(128)),
            user_agent,
        })
    }

    /// Fetch company filings by CIK
    ///
    /// # Arguments
    /// * `cik` - Central Index Key (company identifier, e.g., "0000320193" for Apple)
    /// * `form_type` - Optional form type filter (e.g., "10-K", "10-Q", "8-K")
    pub async fn fetch_filings(
        &self,
        cik: &str,
        form_type: Option<&str>,
    ) -> Result<Vec<DataRecord>> {
        // Pad CIK to 10 digits
        let padded_cik = format!("{:0>10}", cik);

        let url = format!(
            "{}/submissions/CIK{}.json",
            self.base_url, padded_cik
        );

        let response = self.fetch_with_retry(&url).await?;
        let filing_data: EdgarFilingData = response.json().await?;

        let mut records = Vec::new();
        let recent = filing_data.filings.recent;

        let count = recent.accession_number.len();
        for i in 0..count.min(50) {
            // Limit to 50 most recent
            // Filter by form type if specified
            if let Some(filter_form) = form_type {
                if i < recent.form.len() && recent.form[i] != filter_form {
                    continue;
                }
            }

            let filing = EdgarFiling {
                cik: padded_cik.clone(),
                accession_number: recent.accession_number.get(i).cloned().unwrap_or_default(),
                filing_date: recent.filing_date.get(i).cloned().unwrap_or_default(),
                report_date: recent.report_date.get(i).cloned().unwrap_or_default(),
                form: recent.form.get(i).cloned().unwrap_or_default(),
                primary_document: recent.primary_document.get(i).cloned().unwrap_or_default(),
            };

            let record = self.filing_to_record(filing)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Convert filing to DataRecord
    fn filing_to_record(&self, filing: EdgarFiling) -> Result<DataRecord> {
        let text = format!(
            "CIK {} Form {} filed on {} report date {}",
            filing.cik, filing.form, filing.filing_date, filing.report_date
        );
        let embedding = self.embedder.embed_text(&text);

        // Parse filing date
        let timestamp = NaiveDate::parse_from_str(&filing.filing_date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("cik".to_string(), serde_json::json!(filing.cik));
        data_map.insert(
            "accession_number".to_string(),
            serde_json::json!(filing.accession_number),
        );
        data_map.insert(
            "filing_date".to_string(),
            serde_json::json!(filing.filing_date),
        );
        data_map.insert(
            "report_date".to_string(),
            serde_json::json!(filing.report_date),
        );
        data_map.insert("form".to_string(), serde_json::json!(filing.form));
        data_map.insert(
            "primary_document".to_string(),
            serde_json::json!(filing.primary_document),
        );

        // Build filing URL
        let filing_url = format!(
            "https://www.sec.gov/cgi-bin/viewer?action=view&cik={}&accession_number={}&xbrl_type=v",
            filing.cik, filing.accession_number
        );
        data_map.insert("filing_url".to_string(), serde_json::json!(filing_url));

        Ok(DataRecord {
            id: format!("{}_{}", filing.cik, filing.accession_number),
            source: "edgar".to_string(),
            record_type: filing.form,
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships: Vec::new(),
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES
                    {
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

/// Internal structure for SEC filing
struct EdgarFiling {
    cik: String,
    accession_number: String,
    filing_date: String,
    report_date: String,
    form: String,
    primary_document: String,
}

#[async_trait]
impl DataSource for EdgarClient {
    fn source_id(&self) -> &str {
        "edgar"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        _batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Default to Apple Inc (AAPL)
        let cik = cursor.as_deref().unwrap_or("320193");
        let records = self.fetch_filings(cik, None).await?;
        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        Ok(true)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_embedder() {
        let embedder = SimpleEmbedder::new(128);
        let embedding = embedder.embed_text("machine learning artificial intelligence");

        assert_eq!(embedding.len(), 128);

        // Check normalization
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_embedder_json() {
        let embedder = SimpleEmbedder::new(64);
        let json = serde_json::json!({
            "title": "Test Document",
            "content": "Some interesting content here"
        });

        let embedding = embedder.embed_json(&json);
        assert_eq!(embedding.len(), 64);
    }

    #[tokio::test]
    async fn test_openalex_client_creation() {
        let client = OpenAlexClient::new(Some("test@example.com".to_string()));
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_noaa_client_creation() {
        let client = NoaaClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_noaa_synthetic_data() {
        let client = NoaaClient::new(None).unwrap();
        let records = client
            .fetch_climate_data("GHCND:TEST", "2024-01-01", "2024-01-31")
            .await
            .unwrap();

        assert!(!records.is_empty());
        assert_eq!(records[0].source, "noaa");
        assert!(records[0].embedding.is_some());
    }

    #[tokio::test]
    async fn test_edgar_client_creation() {
        let client = EdgarClient::new("test-agent test@example.com".to_string());
        assert!(client.is_ok());
    }
}
