//! ArXiv Preprint API Integration
//!
//! This module provides an async client for fetching academic preprints from ArXiv.org,
//! converting responses to SemanticVector format for RuVector discovery.
//!
//! # ArXiv API Details
//! - Base URL: https://export.arxiv.org/api/query
//! - Free access, no authentication required
//! - Returns Atom XML feed
//! - Rate limit: 1 request per 3 seconds (enforced by client)
//!
//! # Example
//! ```rust,ignore
//! use ruvector_data_framework::arxiv_client::ArxivClient;
//!
//! let client = ArxivClient::new();
//!
//! // Search papers by keywords
//! let vectors = client.search("machine learning", 10).await?;
//!
//! // Search by category
//! let ai_papers = client.search_category("cs.AI", 20).await?;
//!
//! // Get recent papers in a category
//! let recent = client.search_recent("cs.LG", 7).await?;
//! ```

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration for ArXiv API
const ARXIV_RATE_LIMIT_MS: u64 = 3000; // 3 seconds between requests
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const DEFAULT_EMBEDDING_DIM: usize = 384;

// ============================================================================
// ArXiv Atom Feed Structures
// ============================================================================

/// ArXiv API Atom feed response
#[derive(Debug, Deserialize)]
struct ArxivFeed {
    #[serde(rename = "entry", default)]
    entries: Vec<ArxivEntry>,
    #[serde(rename = "totalResults", default)]
    total_results: Option<TotalResults>,
}

#[derive(Debug, Deserialize)]
struct TotalResults {
    #[serde(rename = "$value", default)]
    value: Option<String>,
}

/// ArXiv entry (paper)
#[derive(Debug, Deserialize)]
struct ArxivEntry {
    #[serde(rename = "id")]
    id: String,
    #[serde(rename = "title")]
    title: String,
    #[serde(rename = "summary")]
    summary: String,
    #[serde(rename = "published")]
    published: String,
    #[serde(rename = "updated", default)]
    updated: Option<String>,
    #[serde(rename = "author", default)]
    authors: Vec<ArxivAuthor>,
    #[serde(rename = "category", default)]
    categories: Vec<ArxivCategory>,
    #[serde(rename = "link", default)]
    links: Vec<ArxivLink>,
}

#[derive(Debug, Deserialize)]
struct ArxivAuthor {
    #[serde(rename = "name")]
    name: String,
}

#[derive(Debug, Deserialize)]
struct ArxivCategory {
    #[serde(rename = "@term")]
    term: String,
}

#[derive(Debug, Deserialize)]
struct ArxivLink {
    #[serde(rename = "@href")]
    href: String,
    #[serde(rename = "@type", default)]
    link_type: Option<String>,
    #[serde(rename = "@title", default)]
    title: Option<String>,
}

// ============================================================================
// ArXiv Client
// ============================================================================

/// Client for ArXiv.org preprint API
///
/// Provides methods to search for academic papers, filter by category,
/// and convert results to SemanticVector format for RuVector analysis.
///
/// # Rate Limiting
/// The client automatically enforces ArXiv's rate limit of 1 request per 3 seconds.
/// Includes retry logic for transient failures.
pub struct ArxivClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
}

impl ArxivClient {
    /// Create a new ArXiv API client
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = ArxivClient::new();
    /// ```
    pub fn new() -> Self {
        Self::with_embedding_dim(DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new ArXiv API client with custom embedding dimension
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
            base_url: "https://export.arxiv.org/api/query".to_string(),
        }
    }

    /// Search papers by keywords
    ///
    /// # Arguments
    /// * `query` - Search query (keywords, title, author, etc.)
    /// * `max_results` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let vectors = client.search("quantum computing", 50).await?;
    /// ```
    pub async fn search(&self, query: &str, max_results: usize) -> Result<Vec<SemanticVector>> {
        let encoded_query = urlencoding::encode(query);
        let url = format!(
            "{}?search_query=all:{}&start=0&max_results={}",
            self.base_url, encoded_query, max_results
        );

        self.fetch_and_parse(&url).await
    }

    /// Search papers by ArXiv category
    ///
    /// # Arguments
    /// * `category` - ArXiv category code (e.g., "cs.AI", "physics.ao-ph", "q-fin.ST")
    /// * `max_results` - Maximum number of results to return
    ///
    /// # Supported Categories
    /// - `cs.AI` - Artificial Intelligence
    /// - `cs.LG` - Machine Learning
    /// - `cs.CL` - Computation and Language
    /// - `stat.ML` - Statistics - Machine Learning
    /// - `q-fin.*` - Quantitative Finance (ST, PM, TR, etc.)
    /// - `physics.ao-ph` - Atmospheric and Oceanic Physics
    /// - `econ.*` - Economics
    ///
    /// # Example
    /// ```rust,ignore
    /// let ai_papers = client.search_category("cs.AI", 100).await?;
    /// let climate_papers = client.search_category("physics.ao-ph", 50).await?;
    /// ```
    pub async fn search_category(
        &self,
        category: &str,
        max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}?search_query=cat:{}&start=0&max_results={}&sortBy=submittedDate&sortOrder=descending",
            self.base_url, category, max_results
        );

        self.fetch_and_parse(&url).await
    }

    /// Get a single paper by ArXiv ID
    ///
    /// # Arguments
    /// * `arxiv_id` - ArXiv paper ID (e.g., "2401.12345" or "arXiv:2401.12345")
    ///
    /// # Example
    /// ```rust,ignore
    /// let paper = client.get_paper("2401.12345").await?;
    /// ```
    pub async fn get_paper(&self, arxiv_id: &str) -> Result<Option<SemanticVector>> {
        // Strip "arXiv:" prefix if present
        let id = arxiv_id.trim_start_matches("arXiv:");

        let url = format!("{}?id_list={}", self.base_url, id);
        let mut results = self.fetch_and_parse(&url).await?;

        Ok(results.pop())
    }

    /// Search recent papers in a category within the last N days
    ///
    /// # Arguments
    /// * `category` - ArXiv category code
    /// * `days` - Number of days to look back (default: 7)
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get ML papers from the last 3 days
    /// let recent = client.search_recent("cs.LG", 3).await?;
    /// ```
    pub async fn search_recent(
        &self,
        category: &str,
        days: u64,
    ) -> Result<Vec<SemanticVector>> {
        let cutoff_date = Utc::now() - chrono::Duration::days(days as i64);

        let url = format!(
            "{}?search_query=cat:{}&start=0&max_results=100&sortBy=submittedDate&sortOrder=descending",
            self.base_url, category
        );

        let all_results = self.fetch_and_parse(&url).await?;

        // Filter by date
        Ok(all_results
            .into_iter()
            .filter(|v| v.timestamp >= cutoff_date)
            .collect())
    }

    /// Search papers across multiple categories
    ///
    /// # Arguments
    /// * `categories` - List of ArXiv category codes
    /// * `max_results_per_category` - Maximum results per category
    ///
    /// # Example
    /// ```rust,ignore
    /// let categories = vec!["cs.AI", "cs.LG", "stat.ML"];
    /// let papers = client.search_multiple_categories(&categories, 20).await?;
    /// ```
    pub async fn search_multiple_categories(
        &self,
        categories: &[&str],
        max_results_per_category: usize,
    ) -> Result<Vec<SemanticVector>> {
        let mut all_vectors = Vec::new();

        for category in categories {
            match self.search_category(category, max_results_per_category).await {
                Ok(mut vectors) => {
                    all_vectors.append(&mut vectors);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch category {}: {}", category, e);
                }
            }
            // Rate limiting between categories
            sleep(Duration::from_millis(ARXIV_RATE_LIMIT_MS)).await;
        }

        Ok(all_vectors)
    }

    /// Fetch and parse ArXiv Atom feed
    async fn fetch_and_parse(&self, url: &str) -> Result<Vec<SemanticVector>> {
        // Rate limiting
        sleep(Duration::from_millis(ARXIV_RATE_LIMIT_MS)).await;

        let response = self.fetch_with_retry(url).await?;
        let xml = response.text().await?;

        // Parse XML feed
        let feed: ArxivFeed = quick_xml::de::from_str(&xml).map_err(|e| {
            FrameworkError::Ingestion(format!("Failed to parse ArXiv XML: {}", e))
        })?;

        // Convert entries to SemanticVectors
        let mut vectors = Vec::new();
        for entry in feed.entries {
            if let Some(vector) = self.entry_to_vector(entry) {
                vectors.push(vector);
            }
        }

        Ok(vectors)
    }

    /// Convert ArXiv entry to SemanticVector
    fn entry_to_vector(&self, entry: ArxivEntry) -> Option<SemanticVector> {
        // Extract ArXiv ID from full URL
        let arxiv_id = entry
            .id
            .split('/')
            .last()
            .unwrap_or(&entry.id)
            .to_string();

        // Clean up title and abstract
        let title = entry.title.trim().replace('\n', " ");
        let abstract_text = entry.summary.trim().replace('\n', " ");

        // Parse publication date
        let timestamp = Self::parse_arxiv_date(&entry.published)?;

        // Generate embedding from title + abstract
        let combined_text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&combined_text);

        // Extract authors
        let authors = entry
            .authors
            .iter()
            .map(|a| a.name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        // Extract categories
        let categories = entry
            .categories
            .iter()
            .map(|c| c.term.clone())
            .collect::<Vec<_>>()
            .join(", ");

        // Find PDF URL
        let pdf_url = entry
            .links
            .iter()
            .find(|l| l.title.as_deref() == Some("pdf"))
            .map(|l| l.href.clone())
            .unwrap_or_else(|| format!("https://arxiv.org/pdf/{}.pdf", arxiv_id));

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("arxiv_id".to_string(), arxiv_id.clone());
        metadata.insert("title".to_string(), title);
        metadata.insert("abstract".to_string(), abstract_text);
        metadata.insert("authors".to_string(), authors);
        metadata.insert("categories".to_string(), categories);
        metadata.insert("pdf_url".to_string(), pdf_url);
        metadata.insert("source".to_string(), "arxiv".to_string());

        Some(SemanticVector {
            id: format!("arXiv:{}", arxiv_id),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        })
    }

    /// Parse ArXiv date format (ISO 8601)
    fn parse_arxiv_date(date_str: &str) -> Option<DateTime<Utc>> {
        // ArXiv uses ISO 8601 format: 2024-01-15T12:30:00Z
        DateTime::parse_from_rfc3339(date_str)
            .ok()
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|| {
                // Fallback: try parsing without timezone
                NaiveDateTime::parse_from_str(date_str, "%Y-%m-%dT%H:%M:%S")
                    .ok()
                    .map(|ndt| DateTime::from_naive_utc_and_offset(ndt, Utc))
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
                        tracing::warn!("Rate limited by ArXiv, retrying in {}ms", RETRY_DELAY_MS * retries as u64);
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

impl Default for ArxivClient {
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
    fn test_arxiv_client_creation() {
        let client = ArxivClient::new();
        assert_eq!(client.base_url, "https://export.arxiv.org/api/query");
    }

    #[test]
    fn test_custom_embedding_dim() {
        let client = ArxivClient::with_embedding_dim(512);
        let embedding = client.embedder.embed_text("test");
        assert_eq!(embedding.len(), 512);
    }

    #[test]
    fn test_parse_arxiv_date() {
        // Standard ISO 8601
        let date1 = ArxivClient::parse_arxiv_date("2024-01-15T12:30:00Z");
        assert!(date1.is_some());

        // Without Z suffix
        let date2 = ArxivClient::parse_arxiv_date("2024-01-15T12:30:00");
        assert!(date2.is_some());
    }

    #[test]
    fn test_entry_to_vector() {
        let client = ArxivClient::new();

        let entry = ArxivEntry {
            id: "http://arxiv.org/abs/2401.12345v1".to_string(),
            title: "Deep Learning for Climate Science".to_string(),
            summary: "We propose a novel approach...".to_string(),
            published: "2024-01-15T12:00:00Z".to_string(),
            updated: None,
            authors: vec![
                ArxivAuthor {
                    name: "John Doe".to_string(),
                },
                ArxivAuthor {
                    name: "Jane Smith".to_string(),
                },
            ],
            categories: vec![
                ArxivCategory {
                    term: "cs.LG".to_string(),
                },
                ArxivCategory {
                    term: "physics.ao-ph".to_string(),
                },
            ],
            links: vec![],
        };

        let vector = client.entry_to_vector(entry);
        assert!(vector.is_some());

        let v = vector.unwrap();
        assert_eq!(v.id, "arXiv:2401.12345v1");
        assert_eq!(v.domain, Domain::Research);
        assert_eq!(v.metadata.get("arxiv_id").unwrap(), "2401.12345v1");
        assert_eq!(
            v.metadata.get("title").unwrap(),
            "Deep Learning for Climate Science"
        );
        assert_eq!(v.metadata.get("authors").unwrap(), "John Doe, Jane Smith");
        assert_eq!(v.metadata.get("categories").unwrap(), "cs.LG, physics.ao-ph");
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting ArXiv API in tests
    async fn test_search_integration() {
        let client = ArxivClient::new();
        let results = client.search("machine learning", 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        if !vectors.is_empty() {
            let first = &vectors[0];
            assert!(first.id.starts_with("arXiv:"));
            assert_eq!(first.domain, Domain::Research);
            assert!(first.metadata.contains_key("title"));
            assert!(first.metadata.contains_key("abstract"));
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting ArXiv API in tests
    async fn test_search_category_integration() {
        let client = ArxivClient::new();
        let results = client.search_category("cs.AI", 3).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 3);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting ArXiv API in tests
    async fn test_get_paper_integration() {
        let client = ArxivClient::new();

        // Try to fetch a known paper (this is a real arXiv ID)
        let result = client.get_paper("2301.00001").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting ArXiv API in tests
    async fn test_search_recent_integration() {
        let client = ArxivClient::new();
        let results = client.search_recent("cs.LG", 7).await;
        assert!(results.is_ok());

        // Check that returned papers are within date range
        let cutoff = Utc::now() - chrono::Duration::days(7);
        for vector in results.unwrap() {
            assert!(vector.timestamp >= cutoff);
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting ArXiv API in tests
    async fn test_multiple_categories_integration() {
        let client = ArxivClient::new();
        let categories = vec!["cs.AI", "cs.LG"];
        let results = client.search_multiple_categories(&categories, 2).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 4); // 2 categories * 2 results each
    }
}
