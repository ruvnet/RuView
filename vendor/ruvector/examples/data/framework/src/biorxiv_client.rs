//! bioRxiv and medRxiv Preprint API Integration
//!
//! This module provides async clients for fetching preprints from bioRxiv.org and medRxiv.org,
//! converting responses to SemanticVector format for RuVector discovery.
//!
//! # bioRxiv/medRxiv API Details
//! - Base URL: https://api.biorxiv.org/details/[server]/[interval]/[cursor]
//! - Free access, no authentication required
//! - Returns JSON with preprint metadata
//! - Rate limit: ~1 request per second (enforced by client)
//!
//! # Example
//! ```rust,ignore
//! use ruvector_data_framework::biorxiv_client::{BiorxivClient, MedrxivClient};
//!
//! // Life sciences preprints
//! let biorxiv = BiorxivClient::new();
//! let recent = biorxiv.search_recent(7, 50).await?;
//! let category_papers = biorxiv.search_by_category("neuroscience", 100).await?;
//!
//! // Medical preprints
//! let medrxiv = MedrxivClient::new();
//! let covid_papers = medrxiv.search_covid(100).await?;
//! let clinical = medrxiv.search_clinical(50).await?;
//! ```

use std::collections::HashMap;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const BIORXIV_RATE_LIMIT_MS: u64 = 1000; // 1 second between requests (conservative)
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const DEFAULT_EMBEDDING_DIM: usize = 384;
const DEFAULT_PAGE_SIZE: usize = 100;

// ============================================================================
// bioRxiv/medRxiv API Response Structures
// ============================================================================

/// API response from bioRxiv/medRxiv
#[derive(Debug, Deserialize)]
struct BiorxivApiResponse {
    /// Total number of results
    #[serde(default)]
    count: Option<i64>,

    /// Cursor for pagination (total number of records seen)
    #[serde(default)]
    cursor: Option<i64>,

    /// Array of preprint records
    #[serde(default)]
    collection: Vec<PreprintRecord>,
}

/// Individual preprint record
#[derive(Debug, Deserialize)]
struct PreprintRecord {
    /// DOI identifier
    doi: String,

    /// Paper title
    title: String,

    /// Authors (semicolon-separated)
    authors: String,

    /// Author corresponding information
    #[serde(default)]
    author_corresponding: Option<String>,

    /// Author corresponding institution
    #[serde(default)]
    author_corresponding_institution: Option<String>,

    /// Preprint publication date (YYYY-MM-DD)
    date: String,

    /// Subject category
    category: String,

    /// Abstract text
    #[serde(rename = "abstract")]
    abstract_text: String,

    /// Journal publication status (if accepted)
    #[serde(default)]
    published: Option<String>,

    /// Server (biorxiv or medrxiv)
    #[serde(default)]
    server: Option<String>,

    /// Version number
    #[serde(default)]
    version: Option<String>,

    /// Type (e.g., "new results")
    #[serde(rename = "type", default)]
    preprint_type: Option<String>,
}

// ============================================================================
// bioRxiv Client (Life Sciences Preprints)
// ============================================================================

/// Client for bioRxiv.org preprint API
///
/// Provides methods to search for life sciences preprints, filter by category,
/// and convert results to SemanticVector format for RuVector analysis.
///
/// # Categories
/// - neuroscience
/// - genomics
/// - bioinformatics
/// - cancer-biology
/// - immunology
/// - microbiology
/// - molecular-biology
/// - cell-biology
/// - biochemistry
/// - evolutionary-biology
/// - and many more...
///
/// # Rate Limiting
/// The client automatically enforces a rate limit of ~1 request per second.
/// Includes retry logic for transient failures.
pub struct BiorxivClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
}

impl BiorxivClient {
    /// Create a new bioRxiv API client
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = BiorxivClient::new();
    /// ```
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new bioRxiv API client with custom embedding dimension
    ///
    /// # Arguments
    /// * `embedding_dim` - Dimension for text embeddings (default: 384)
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://api.biorxiv.org".to_string(),
        }
    }

    /// Get recent preprints from the last N days
    ///
    /// # Arguments
    /// * `days` - Number of days to look back (e.g., 7 for last week)
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get preprints from the last 7 days
    /// let recent = client.search_recent(7, 100).await?;
    /// ```
    pub async fn search_recent(&self, days: u64, limit: usize) -> Result<Vec<SemanticVector>> {
        let end_date = Utc::now().date_naive();
        let start_date = end_date - chrono::Duration::days(days as i64);

        self.search_by_date_range(start_date, end_date, Some(limit)).await
    }

    /// Search preprints by date range
    ///
    /// # Arguments
    /// * `start_date` - Start date (inclusive)
    /// * `end_date` - End date (inclusive)
    /// * `limit` - Optional maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// use chrono::NaiveDate;
    ///
    /// let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    /// let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    /// let papers = client.search_by_date_range(start, end, Some(200)).await?;
    /// ```
    pub async fn search_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        let interval = format!("{}/{}", start_date, end_date);
        self.fetch_with_pagination("biorxiv", &interval, limit).await
    }

    /// Search preprints by subject category
    ///
    /// # Arguments
    /// * `category` - Subject category (e.g., "neuroscience", "genomics")
    /// * `limit` - Maximum number of results to return
    ///
    /// # Categories
    /// - neuroscience
    /// - genomics
    /// - bioinformatics
    /// - cancer-biology
    /// - immunology
    /// - microbiology
    /// - molecular-biology
    /// - cell-biology
    /// - biochemistry
    /// - evolutionary-biology
    /// - ecology
    /// - genetics
    /// - developmental-biology
    /// - synthetic-biology
    /// - systems-biology
    ///
    /// # Example
    /// ```rust,ignore
    /// let neuroscience_papers = client.search_by_category("neuroscience", 100).await?;
    /// ```
    pub async fn search_by_category(
        &self,
        category: &str,
        limit: usize,
    ) -> Result<Vec<SemanticVector>> {
        // Get recent papers (last 365 days) and filter by category
        let end_date = Utc::now().date_naive();
        let start_date = end_date - chrono::Duration::days(365);

        let all_papers = self.search_by_date_range(start_date, end_date, Some(limit * 2)).await?;

        // Filter by category
        Ok(all_papers
            .into_iter()
            .filter(|v| {
                v.metadata
                    .get("category")
                    .map(|cat| cat.to_lowercase().contains(&category.to_lowercase()))
                    .unwrap_or(false)
            })
            .take(limit)
            .collect())
    }

    /// Fetch preprints with pagination support
    async fn fetch_with_pagination(
        &self,
        server: &str,
        interval: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        let mut all_vectors = Vec::new();
        let mut cursor = 0;
        let limit = limit.unwrap_or(usize::MAX);

        loop {
            if all_vectors.len() >= limit {
                break;
            }

            let url = format!("{}/details/{}/{}/{}", self.base_url, server, interval, cursor);

            // Rate limiting
            sleep(Duration::from_millis(BIORXIV_RATE_LIMIT_MS)).await;

            let response = self.fetch_with_retry(&url).await?;
            let api_response: BiorxivApiResponse = response.json().await?;

            if api_response.collection.is_empty() {
                break;
            }

            // Convert records to vectors
            for record in api_response.collection {
                if all_vectors.len() >= limit {
                    break;
                }

                if let Some(vector) = self.record_to_vector(record, server) {
                    all_vectors.push(vector);
                }
            }

            // Update cursor for next page
            if let Some(new_cursor) = api_response.cursor {
                if new_cursor as usize <= cursor {
                    // No more pages
                    break;
                }
                cursor = new_cursor as usize;
            } else {
                break;
            }

            // Safety check: don't paginate indefinitely
            if cursor > 10000 {
                tracing::warn!("Pagination cursor exceeded 10000, stopping");
                break;
            }
        }

        Ok(all_vectors)
    }

    /// Convert preprint record to SemanticVector
    fn record_to_vector(&self, record: PreprintRecord, server: &str) -> Option<SemanticVector> {
        // Clean up title and abstract
        let title = record.title.trim().replace('\n', " ");
        let abstract_text = record.abstract_text.trim().replace('\n', " ");

        // Parse publication date
        let timestamp = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        // Generate embedding from title + abstract
        let combined_text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&combined_text);

        // Determine publication status
        let published_status = record.published.unwrap_or_else(|| "preprint".to_string());

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("doi".to_string(), record.doi.clone());
        metadata.insert("title".to_string(), title);
        metadata.insert("abstract".to_string(), abstract_text);
        metadata.insert("authors".to_string(), record.authors);
        metadata.insert("category".to_string(), record.category);
        metadata.insert("server".to_string(), server.to_string());
        metadata.insert("published_status".to_string(), published_status);

        if let Some(corr) = record.author_corresponding {
            metadata.insert("corresponding_author".to_string(), corr);
        }
        if let Some(inst) = record.author_corresponding_institution {
            metadata.insert("institution".to_string(), inst);
        }
        if let Some(version) = record.version {
            metadata.insert("version".to_string(), version);
        }
        if let Some(ptype) = record.preprint_type {
            metadata.insert("type".to_string(), ptype);
        }

        metadata.insert("source".to_string(), "biorxiv".to_string());

        // bioRxiv papers are research domain
        Some(SemanticVector {
            id: format!("doi:{}", record.doi),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        tracing::warn!("Rate limited, retrying in {}ms", RETRY_DELAY_MS * retries as u64);
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    tracing::warn!("Request failed, retrying ({}/{})", retries, MAX_RETRIES);
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for BiorxivClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// medRxiv Client (Medical Preprints)
// ============================================================================

/// Client for medRxiv.org preprint API
///
/// Provides methods to search for medical and health sciences preprints,
/// filter by specialty, and convert results to SemanticVector format.
///
/// # Categories
/// - Cardiovascular Medicine
/// - Infectious Diseases
/// - Oncology
/// - Public Health
/// - Epidemiology
/// - Psychiatry
/// - and many more...
///
/// # Rate Limiting
/// The client automatically enforces a rate limit of ~1 request per second.
/// Includes retry logic for transient failures.
pub struct MedrxivClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
}

impl MedrxivClient {
    /// Create a new medRxiv API client
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = MedrxivClient::new();
    /// ```
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new medRxiv API client with custom embedding dimension
    ///
    /// # Arguments
    /// * `embedding_dim` - Dimension for text embeddings (default: 384)
    pub fn with_embedding_dim(embedding_dim: usize) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://api.biorxiv.org".to_string(),
        }
    }

    /// Get recent preprints from the last N days
    ///
    /// # Arguments
    /// * `days` - Number of days to look back (e.g., 7 for last week)
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get medical preprints from the last 7 days
    /// let recent = client.search_recent(7, 100).await?;
    /// ```
    pub async fn search_recent(&self, days: u64, limit: usize) -> Result<Vec<SemanticVector>> {
        let end_date = Utc::now().date_naive();
        let start_date = end_date - chrono::Duration::days(days as i64);

        self.search_by_date_range(start_date, end_date, Some(limit)).await
    }

    /// Search preprints by date range
    ///
    /// # Arguments
    /// * `start_date` - Start date (inclusive)
    /// * `end_date` - End date (inclusive)
    /// * `limit` - Optional maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// use chrono::NaiveDate;
    ///
    /// let start = NaiveDate::from_ymd_opt(2024, 1, 1).unwrap();
    /// let end = NaiveDate::from_ymd_opt(2024, 12, 31).unwrap();
    /// let papers = client.search_by_date_range(start, end, Some(200)).await?;
    /// ```
    pub async fn search_by_date_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        let interval = format!("{}/{}", start_date, end_date);
        self.fetch_with_pagination("medrxiv", &interval, limit).await
    }

    /// Search COVID-19 related preprints
    ///
    /// # Arguments
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let covid_papers = client.search_covid(100).await?;
    /// ```
    pub async fn search_covid(&self, limit: usize) -> Result<Vec<SemanticVector>> {
        // Search for COVID-19 related papers from 2020 onwards
        let end_date = Utc::now().date_naive();
        let start_date = NaiveDate::from_ymd_opt(2020, 1, 1).expect("Valid date");

        let all_papers = self.search_by_date_range(start_date, end_date, Some(limit * 2)).await?;

        // Filter by COVID-19 related keywords
        Ok(all_papers
            .into_iter()
            .filter(|v| {
                let title = v.metadata.get("title").map(|s| s.to_lowercase()).unwrap_or_default();
                let abstract_text = v.metadata.get("abstract").map(|s| s.to_lowercase()).unwrap_or_default();
                let category = v.metadata.get("category").map(|s| s.to_lowercase()).unwrap_or_default();

                let keywords = ["covid", "sars-cov-2", "coronavirus", "pandemic"];
                keywords.iter().any(|kw| {
                    title.contains(kw) || abstract_text.contains(kw) || category.contains(kw)
                })
            })
            .take(limit)
            .collect())
    }

    /// Search clinical research preprints
    ///
    /// # Arguments
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let clinical_papers = client.search_clinical(50).await?;
    /// ```
    pub async fn search_clinical(&self, limit: usize) -> Result<Vec<SemanticVector>> {
        // Get recent papers and filter for clinical research
        let end_date = Utc::now().date_naive();
        let start_date = end_date - chrono::Duration::days(365);

        let all_papers = self.search_by_date_range(start_date, end_date, Some(limit * 2)).await?;

        // Filter by clinical keywords
        Ok(all_papers
            .into_iter()
            .filter(|v| {
                let title = v.metadata.get("title").map(|s| s.to_lowercase()).unwrap_or_default();
                let abstract_text = v.metadata.get("abstract").map(|s| s.to_lowercase()).unwrap_or_default();
                let category = v.metadata.get("category").map(|s| s.to_lowercase()).unwrap_or_default();

                let keywords = ["clinical", "trial", "patient", "treatment", "therapy", "diagnosis"];
                keywords.iter().any(|kw| {
                    title.contains(kw) || abstract_text.contains(kw) || category.contains(kw)
                })
            })
            .take(limit)
            .collect())
    }

    /// Fetch preprints with pagination support
    async fn fetch_with_pagination(
        &self,
        server: &str,
        interval: &str,
        limit: Option<usize>,
    ) -> Result<Vec<SemanticVector>> {
        let mut all_vectors = Vec::new();
        let mut cursor = 0;
        let limit = limit.unwrap_or(usize::MAX);

        loop {
            if all_vectors.len() >= limit {
                break;
            }

            let url = format!("{}/details/{}/{}/{}", self.base_url, server, interval, cursor);

            // Rate limiting
            sleep(Duration::from_millis(BIORXIV_RATE_LIMIT_MS)).await;

            let response = self.fetch_with_retry(&url).await?;
            let api_response: BiorxivApiResponse = response.json().await?;

            if api_response.collection.is_empty() {
                break;
            }

            // Convert records to vectors
            for record in api_response.collection {
                if all_vectors.len() >= limit {
                    break;
                }

                if let Some(vector) = self.record_to_vector(record, server) {
                    all_vectors.push(vector);
                }
            }

            // Update cursor for next page
            if let Some(new_cursor) = api_response.cursor {
                if new_cursor as usize <= cursor {
                    // No more pages
                    break;
                }
                cursor = new_cursor as usize;
            } else {
                break;
            }

            // Safety check: don't paginate indefinitely
            if cursor > 10000 {
                tracing::warn!("Pagination cursor exceeded 10000, stopping");
                break;
            }
        }

        Ok(all_vectors)
    }

    /// Convert preprint record to SemanticVector
    fn record_to_vector(&self, record: PreprintRecord, server: &str) -> Option<SemanticVector> {
        // Clean up title and abstract
        let title = record.title.trim().replace('\n', " ");
        let abstract_text = record.abstract_text.trim().replace('\n', " ");

        // Parse publication date
        let timestamp = NaiveDate::parse_from_str(&record.date, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        // Generate embedding from title + abstract
        let combined_text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&combined_text);

        // Determine publication status
        let published_status = record.published.unwrap_or_else(|| "preprint".to_string());

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("doi".to_string(), record.doi.clone());
        metadata.insert("title".to_string(), title);
        metadata.insert("abstract".to_string(), abstract_text);
        metadata.insert("authors".to_string(), record.authors);
        metadata.insert("category".to_string(), record.category);
        metadata.insert("server".to_string(), server.to_string());
        metadata.insert("published_status".to_string(), published_status);

        if let Some(corr) = record.author_corresponding {
            metadata.insert("corresponding_author".to_string(), corr);
        }
        if let Some(inst) = record.author_corresponding_institution {
            metadata.insert("institution".to_string(), inst);
        }
        if let Some(version) = record.version {
            metadata.insert("version".to_string(), version);
        }
        if let Some(ptype) = record.preprint_type {
            metadata.insert("type".to_string(), ptype);
        }

        metadata.insert("source".to_string(), "medrxiv".to_string());

        // medRxiv papers are medical domain
        Some(SemanticVector {
            id: format!("doi:{}", record.doi),
            embedding,
            domain: Domain::Medical,
            timestamp,
            metadata,
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        tracing::warn!("Rate limited, retrying in {}ms", RETRY_DELAY_MS * retries as u64);
                        sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                        continue;
                    }
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    tracing::warn!("Request failed, retrying ({}/{})", retries, MAX_RETRIES);
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for MedrxivClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biorxiv_client_creation() {
        let client = BiorxivClient::new();
        assert_eq!(client.base_url, "https://api.biorxiv.org");
    }

    #[test]
    fn test_medrxiv_client_creation() {
        let client = MedrxivClient::new();
        assert_eq!(client.base_url, "https://api.biorxiv.org");
    }

    #[test]
    fn test_custom_embedding_dim() {
        let client = BiorxivClient::with_embedding_dim(512);
        let embedding = client.embedder.embed_text("test");
        assert_eq!(embedding.len(), 512);
    }

    #[test]
    fn test_record_to_vector_biorxiv() {
        let client = BiorxivClient::new();

        let record = PreprintRecord {
            doi: "10.1101/2024.01.01.123456".to_string(),
            title: "Deep Learning for Neuroscience".to_string(),
            authors: "John Doe; Jane Smith".to_string(),
            author_corresponding: Some("John Doe".to_string()),
            author_corresponding_institution: Some("MIT".to_string()),
            date: "2024-01-15".to_string(),
            category: "Neuroscience".to_string(),
            abstract_text: "We propose a novel approach for analyzing neural data...".to_string(),
            published: None,
            server: Some("biorxiv".to_string()),
            version: Some("1".to_string()),
            preprint_type: Some("new results".to_string()),
        };

        let vector = client.record_to_vector(record, "biorxiv");
        assert!(vector.is_some());

        let v = vector.unwrap();
        assert_eq!(v.id, "doi:10.1101/2024.01.01.123456");
        assert_eq!(v.domain, Domain::Research);
        assert_eq!(v.metadata.get("doi").unwrap(), "10.1101/2024.01.01.123456");
        assert_eq!(v.metadata.get("title").unwrap(), "Deep Learning for Neuroscience");
        assert_eq!(v.metadata.get("authors").unwrap(), "John Doe; Jane Smith");
        assert_eq!(v.metadata.get("category").unwrap(), "Neuroscience");
        assert_eq!(v.metadata.get("server").unwrap(), "biorxiv");
        assert_eq!(v.metadata.get("published_status").unwrap(), "preprint");
    }

    #[test]
    fn test_record_to_vector_medrxiv() {
        let client = MedrxivClient::new();

        let record = PreprintRecord {
            doi: "10.1101/2024.01.01.654321".to_string(),
            title: "COVID-19 Vaccine Efficacy Study".to_string(),
            authors: "Alice Johnson; Bob Williams".to_string(),
            author_corresponding: Some("Alice Johnson".to_string()),
            author_corresponding_institution: Some("Harvard Medical School".to_string()),
            date: "2024-03-20".to_string(),
            category: "Infectious Diseases".to_string(),
            abstract_text: "This study evaluates the efficacy of mRNA vaccines...".to_string(),
            published: Some("Nature Medicine".to_string()),
            server: Some("medrxiv".to_string()),
            version: Some("2".to_string()),
            preprint_type: Some("new results".to_string()),
        };

        let vector = client.record_to_vector(record, "medrxiv");
        assert!(vector.is_some());

        let v = vector.unwrap();
        assert_eq!(v.id, "doi:10.1101/2024.01.01.654321");
        assert_eq!(v.domain, Domain::Medical);
        assert_eq!(v.metadata.get("doi").unwrap(), "10.1101/2024.01.01.654321");
        assert_eq!(v.metadata.get("title").unwrap(), "COVID-19 Vaccine Efficacy Study");
        assert_eq!(v.metadata.get("category").unwrap(), "Infectious Diseases");
        assert_eq!(v.metadata.get("server").unwrap(), "medrxiv");
        assert_eq!(v.metadata.get("published_status").unwrap(), "Nature Medicine");
    }

    #[test]
    fn test_date_parsing() {
        let client = BiorxivClient::new();

        let record = PreprintRecord {
            doi: "10.1101/test".to_string(),
            title: "Test".to_string(),
            authors: "Author".to_string(),
            author_corresponding: None,
            author_corresponding_institution: None,
            date: "2024-01-15".to_string(),
            category: "Test".to_string(),
            abstract_text: "Abstract".to_string(),
            published: None,
            server: None,
            version: None,
            preprint_type: None,
        };

        let vector = client.record_to_vector(record, "biorxiv").unwrap();

        // Check that date was parsed correctly
        let expected_date = NaiveDate::from_ymd_opt(2024, 1, 15)
            .unwrap()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();

        assert_eq!(vector.timestamp, expected_date);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting bioRxiv API in tests
    async fn test_search_recent_integration() {
        let client = BiorxivClient::new();
        let results = client.search_recent(7, 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        if !vectors.is_empty() {
            let first = &vectors[0];
            assert!(first.id.starts_with("doi:"));
            assert_eq!(first.domain, Domain::Research);
            assert!(first.metadata.contains_key("title"));
            assert!(first.metadata.contains_key("abstract"));
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting medRxiv API in tests
    async fn test_medrxiv_search_recent_integration() {
        let client = MedrxivClient::new();
        let results = client.search_recent(7, 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        if !vectors.is_empty() {
            let first = &vectors[0];
            assert!(first.id.starts_with("doi:"));
            assert_eq!(first.domain, Domain::Medical);
            assert!(first.metadata.contains_key("title"));
            assert!(first.metadata.contains_key("server"));
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting API
    async fn test_search_covid_integration() {
        let client = MedrxivClient::new();
        let results = client.search_covid(10).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();

        // Verify that results contain COVID-related keywords
        for v in &vectors {
            let title = v.metadata.get("title").unwrap().to_lowercase();
            let abstract_text = v.metadata.get("abstract").unwrap().to_lowercase();

            let has_covid_keyword = title.contains("covid")
                || title.contains("sars-cov-2")
                || abstract_text.contains("covid")
                || abstract_text.contains("sars-cov-2");

            assert!(has_covid_keyword, "Expected COVID-related keywords in results");
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting API
    async fn test_search_by_category_integration() {
        let client = BiorxivClient::new();
        let results = client.search_by_category("neuroscience", 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        // Verify category filtering
        for v in &vectors {
            let category = v.metadata.get("category").unwrap().to_lowercase();
            assert!(category.contains("neuroscience"));
        }
    }
}
