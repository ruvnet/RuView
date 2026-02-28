//! CrossRef API Integration
//!
//! This module provides an async client for fetching scholarly publications from CrossRef.org,
//! converting responses to SemanticVector format for RuVector discovery.
//!
//! # CrossRef API Details
//! - Base URL: https://api.crossref.org
//! - Free access, no authentication required
//! - Returns JSON responses
//! - Rate limit: ~50 requests/second with polite pool
//! - Polite pool: Include email in User-Agent or Mailto header for better rate limits
//!
//! # Example
//! ```rust,ignore
//! use ruvector_data_framework::crossref_client::CrossRefClient;
//!
//! let client = CrossRefClient::new(Some("your-email@example.com".to_string()));
//!
//! // Search publications by keywords
//! let vectors = client.search_works("machine learning", 20).await?;
//!
//! // Get work by DOI
//! let work = client.get_work("10.1038/nature12373").await?;
//!
//! // Search by funder
//! let funded = client.search_by_funder("10.13039/100000001", 10).await?;
//!
//! // Find recent publications
//! let recent = client.search_recent("quantum computing", "2024-01-01").await?;
//! ```

use std::collections::HashMap;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration for CrossRef API
const CROSSREF_RATE_LIMIT_MS: u64 = 1000; // 1 second between requests for safety (API allows ~50/sec)
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const DEFAULT_EMBEDDING_DIM: usize = 384;

// ============================================================================
// CrossRef API Structures
// ============================================================================

/// CrossRef API response for works search
#[derive(Debug, Deserialize)]
struct CrossRefResponse {
    #[serde(default)]
    message: CrossRefMessage,
}

#[derive(Debug, Default, Deserialize)]
struct CrossRefMessage {
    #[serde(default)]
    items: Vec<CrossRefWork>,
    #[serde(rename = "total-results", default)]
    total_results: Option<u64>,
}

/// CrossRef work (publication)
#[derive(Debug, Deserialize)]
struct CrossRefWork {
    #[serde(rename = "DOI")]
    doi: String,
    #[serde(default)]
    title: Vec<String>,
    #[serde(rename = "abstract", default)]
    abstract_text: Option<String>,
    #[serde(default)]
    author: Vec<CrossRefAuthor>,
    #[serde(rename = "published-print", default)]
    published_print: Option<CrossRefDate>,
    #[serde(rename = "published-online", default)]
    published_online: Option<CrossRefDate>,
    #[serde(rename = "container-title", default)]
    container_title: Vec<String>,
    #[serde(rename = "is-referenced-by-count", default)]
    citation_count: Option<u64>,
    #[serde(rename = "references-count", default)]
    references_count: Option<u64>,
    #[serde(default)]
    subject: Vec<String>,
    #[serde(default)]
    funder: Vec<CrossRefFunder>,
    #[serde(rename = "type", default)]
    work_type: Option<String>,
    #[serde(default)]
    publisher: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrossRefAuthor {
    #[serde(default)]
    given: Option<String>,
    #[serde(default)]
    family: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "ORCID", default)]
    orcid: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CrossRefDate {
    #[serde(rename = "date-parts", default)]
    date_parts: Vec<Vec<i32>>,
}

#[derive(Debug, Deserialize)]
struct CrossRefFunder {
    #[serde(default)]
    name: Option<String>,
    #[serde(rename = "DOI", default)]
    doi: Option<String>,
}

// ============================================================================
// CrossRef Client
// ============================================================================

/// Client for CrossRef.org scholarly publication API
///
/// Provides methods to search for publications, filter by various criteria,
/// and convert results to SemanticVector format for RuVector analysis.
///
/// # Rate Limiting
/// The client automatically enforces conservative rate limits (1 request/second).
/// Includes polite pool support via email configuration for better rate limits.
/// Includes retry logic for transient failures.
pub struct CrossRefClient {
    client: Client,
    embedder: SimpleEmbedder,
    base_url: String,
    polite_email: Option<String>,
}

impl CrossRefClient {
    /// Create a new CrossRef API client
    ///
    /// # Arguments
    /// * `polite_email` - Email for polite pool access (optional but recommended for better rate limits)
    ///
    /// # Example
    /// ```rust,ignore
    /// let client = CrossRefClient::new(Some("researcher@university.edu".to_string()));
    /// ```
    pub fn new(polite_email: Option<String>) -> Self {
        Self::with_embedding_dim(polite_email, DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new CrossRef API client with custom embedding dimension
    ///
    /// # Arguments
    /// * `polite_email` - Email for polite pool access
    /// * `embedding_dim` - Dimension for text embeddings (default: 384)
    pub fn with_embedding_dim(polite_email: Option<String>, embedding_dim: usize) -> Self {
        let user_agent = if let Some(ref email) = polite_email {
            format!("RuVector-Discovery/1.0 (mailto:{})", email)
        } else {
            "RuVector-Discovery/1.0".to_string()
        };

        Self {
            client: Client::builder()
                .user_agent(&user_agent)
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: SimpleEmbedder::new(embedding_dim),
            base_url: "https://api.crossref.org".to_string(),
            polite_email,
        }
    }

    /// Search publications by keywords
    ///
    /// # Arguments
    /// * `query` - Search query (title, abstract, author, etc.)
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let vectors = client.search_works("climate change machine learning", 50).await?;
    /// ```
    pub async fn search_works(&self, query: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let encoded_query = urlencoding::encode(query);
        let mut url = format!(
            "{}/works?query={}&rows={}",
            self.base_url, encoded_query, limit
        );

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Get a single work by DOI
    ///
    /// # Arguments
    /// * `doi` - Digital Object Identifier (e.g., "10.1038/nature12373")
    ///
    /// # Example
    /// ```rust,ignore
    /// let work = client.get_work("10.1038/nature12373").await?;
    /// ```
    pub async fn get_work(&self, doi: &str) -> Result<Option<SemanticVector>> {
        let normalized_doi = Self::normalize_doi(doi);
        let mut url = format!("{}/works/{}", self.base_url, normalized_doi);

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("?mailto={}", email));
        }

        sleep(Duration::from_millis(CROSSREF_RATE_LIMIT_MS)).await;

        let response = self.fetch_with_retry(&url).await?;
        let json_response: CrossRefResponse = response.json().await?;

        if let Some(work) = json_response.message.items.into_iter().next() {
            Ok(Some(self.work_to_vector(work)))
        } else {
            Ok(None)
        }
    }

    /// Search publications funded by a specific organization
    ///
    /// # Arguments
    /// * `funder_id` - Funder DOI (e.g., "10.13039/100000001" for NSF)
    /// * `limit` - Maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// // Search NSF-funded research
    /// let nsf_works = client.search_by_funder("10.13039/100000001", 20).await?;
    /// ```
    pub async fn search_by_funder(&self, funder_id: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/funders/{}/works?rows={}",
            self.base_url, funder_id, limit
        );

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Search publications by subject area
    ///
    /// # Arguments
    /// * `subject` - Subject area or field
    /// * `limit` - Maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// let biology_works = client.search_by_subject("molecular biology", 30).await?;
    /// ```
    pub async fn search_by_subject(&self, subject: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let encoded_subject = urlencoding::encode(subject);
        let mut url = format!(
            "{}/works?filter=has-subject:true&query.subject={}&rows={}",
            self.base_url, encoded_subject, limit
        );

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Get publications that cite a specific DOI
    ///
    /// # Arguments
    /// * `doi` - DOI of the work to find citations for
    /// * `limit` - Maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// let citing_works = client.get_citations("10.1038/nature12373", 15).await?;
    /// ```
    pub async fn get_citations(&self, doi: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let normalized_doi = Self::normalize_doi(doi);
        let mut url = format!(
            "{}/works?filter=references:{}&rows={}",
            self.base_url, normalized_doi, limit
        );

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Search recent publications since a specific date
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `from_date` - Start date in YYYY-MM-DD format
    /// * `limit` - Maximum number of results
    ///
    /// # Example
    /// ```rust,ignore
    /// let recent = client.search_recent("artificial intelligence", "2024-01-01", 25).await?;
    /// ```
    pub async fn search_recent(&self, query: &str, from_date: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let encoded_query = urlencoding::encode(query);
        let mut url = format!(
            "{}/works?query={}&filter=from-pub-date:{}&rows={}",
            self.base_url, encoded_query, from_date, limit
        );

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Search publications by type
    ///
    /// # Arguments
    /// * `work_type` - Type of publication (e.g., "journal-article", "book-chapter", "proceedings-article", "dataset")
    /// * `query` - Optional search query
    /// * `limit` - Maximum number of results
    ///
    /// # Supported Types
    /// - `journal-article` - Journal articles
    /// - `book-chapter` - Book chapters
    /// - `proceedings-article` - Conference proceedings
    /// - `dataset` - Research datasets
    /// - `monograph` - Monographs
    /// - `report` - Technical reports
    ///
    /// # Example
    /// ```rust,ignore
    /// let datasets = client.search_by_type("dataset", Some("climate"), 10).await?;
    /// let articles = client.search_by_type("journal-article", None, 20).await?;
    /// ```
    pub async fn search_by_type(
        &self,
        work_type: &str,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/works?filter=type:{}&rows={}",
            self.base_url, work_type, limit
        );

        if let Some(q) = query {
            let encoded_query = urlencoding::encode(q);
            url.push_str(&format!("&query={}", encoded_query));
        }

        if let Some(email) = &self.polite_email {
            url.push_str(&format!("&mailto={}", email));
        }

        self.fetch_and_parse(&url).await
    }

    /// Fetch and parse CrossRef API response
    async fn fetch_and_parse(&self, url: &str) -> Result<Vec<SemanticVector>> {
        // Rate limiting
        sleep(Duration::from_millis(CROSSREF_RATE_LIMIT_MS)).await;

        let response = self.fetch_with_retry(url).await?;
        let crossref_response: CrossRefResponse = response.json().await?;

        // Convert works to SemanticVectors
        let vectors = crossref_response
            .message
            .items
            .into_iter()
            .map(|work| self.work_to_vector(work))
            .collect();

        Ok(vectors)
    }

    /// Convert CrossRef work to SemanticVector
    fn work_to_vector(&self, work: CrossRefWork) -> SemanticVector {
        // Extract title
        let title = work
            .title
            .first()
            .cloned()
            .unwrap_or_else(|| "Untitled".to_string());

        // Extract abstract
        let abstract_text = work.abstract_text.unwrap_or_default();

        // Parse publication date (prefer print, fallback to online)
        let timestamp = work
            .published_print
            .or(work.published_online)
            .and_then(|date| Self::parse_crossref_date(&date))
            .unwrap_or_else(Utc::now);

        // Generate embedding from title + abstract
        let combined_text = if abstract_text.is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, abstract_text)
        };
        let embedding = self.embedder.embed_text(&combined_text);

        // Extract authors
        let authors = work
            .author
            .iter()
            .map(|a| Self::format_author_name(a))
            .collect::<Vec<_>>()
            .join("; ");

        // Extract journal/container
        let journal = work
            .container_title
            .first()
            .cloned()
            .unwrap_or_default();

        // Extract subjects
        let subjects = work.subject.join(", ");

        // Extract funders
        let funders = work
            .funder
            .iter()
            .filter_map(|f| f.name.clone())
            .collect::<Vec<_>>()
            .join(", ");

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("doi".to_string(), work.doi.clone());
        metadata.insert("title".to_string(), title);
        metadata.insert("abstract".to_string(), abstract_text);
        metadata.insert("authors".to_string(), authors);
        metadata.insert("journal".to_string(), journal);
        metadata.insert("subjects".to_string(), subjects);
        metadata.insert(
            "citation_count".to_string(),
            work.citation_count.unwrap_or(0).to_string(),
        );
        metadata.insert(
            "references_count".to_string(),
            work.references_count.unwrap_or(0).to_string(),
        );
        metadata.insert("funders".to_string(), funders);
        metadata.insert(
            "type".to_string(),
            work.work_type.unwrap_or_else(|| "unknown".to_string()),
        );
        if let Some(publisher) = work.publisher {
            metadata.insert("publisher".to_string(), publisher);
        }
        metadata.insert("source".to_string(), "crossref".to_string());

        SemanticVector {
            id: format!("doi:{}", work.doi),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        }
    }

    /// Parse CrossRef date format
    fn parse_crossref_date(date: &CrossRefDate) -> Option<DateTime<Utc>> {
        if let Some(parts) = date.date_parts.first() {
            if parts.is_empty() {
                return None;
            }

            let year = parts[0];
            let month = parts.get(1).copied().unwrap_or(1).max(1).min(12);
            let day = parts.get(2).copied().unwrap_or(1).max(1).min(31);

            NaiveDate::from_ymd_opt(year, month as u32, day as u32)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
        } else {
            None
        }
    }

    /// Format author name from CrossRef author structure
    fn format_author_name(author: &CrossRefAuthor) -> String {
        if let Some(name) = &author.name {
            name.clone()
        } else {
            let given = author.given.as_deref().unwrap_or("");
            let family = author.family.as_deref().unwrap_or("");
            format!("{} {}", given, family).trim().to_string()
        }
    }

    /// Normalize DOI (remove http://, https://, doi.org/ prefixes)
    fn normalize_doi(doi: &str) -> String {
        doi.trim()
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .trim_start_matches("doi.org/")
            .trim_start_matches("dx.doi.org/")
            .to_string()
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
                        tracing::warn!(
                            "Rate limited by CrossRef, retrying in {}ms",
                            RETRY_DELAY_MS * retries as u64
                        );
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

impl Default for CrossRefClient {
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

    #[test]
    fn test_crossref_client_creation() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));
        assert_eq!(client.base_url, "https://api.crossref.org");
        assert_eq!(client.polite_email, Some("test@example.com".to_string()));
    }

    #[test]
    fn test_crossref_client_without_email() {
        let client = CrossRefClient::new(None);
        assert_eq!(client.base_url, "https://api.crossref.org");
        assert_eq!(client.polite_email, None);
    }

    #[test]
    fn test_custom_embedding_dim() {
        let client = CrossRefClient::with_embedding_dim(None, 512);
        let embedding = client.embedder.embed_text("test");
        assert_eq!(embedding.len(), 512);
    }

    #[test]
    fn test_normalize_doi() {
        assert_eq!(
            CrossRefClient::normalize_doi("10.1038/nature12373"),
            "10.1038/nature12373"
        );
        assert_eq!(
            CrossRefClient::normalize_doi("http://doi.org/10.1038/nature12373"),
            "10.1038/nature12373"
        );
        assert_eq!(
            CrossRefClient::normalize_doi("https://dx.doi.org/10.1038/nature12373"),
            "10.1038/nature12373"
        );
        assert_eq!(
            CrossRefClient::normalize_doi("  10.1038/nature12373  "),
            "10.1038/nature12373"
        );
    }

    #[test]
    fn test_parse_crossref_date() {
        // Full date
        let date1 = CrossRefDate {
            date_parts: vec![vec![2024, 3, 15]],
        };
        let parsed1 = CrossRefClient::parse_crossref_date(&date1);
        assert!(parsed1.is_some());
        let dt1 = parsed1.unwrap();
        assert_eq!(dt1.format("%Y-%m-%d").to_string(), "2024-03-15");

        // Year and month only
        let date2 = CrossRefDate {
            date_parts: vec![vec![2024, 3]],
        };
        let parsed2 = CrossRefClient::parse_crossref_date(&date2);
        assert!(parsed2.is_some());

        // Year only
        let date3 = CrossRefDate {
            date_parts: vec![vec![2024]],
        };
        let parsed3 = CrossRefClient::parse_crossref_date(&date3);
        assert!(parsed3.is_some());

        // Empty date parts
        let date4 = CrossRefDate {
            date_parts: vec![vec![]],
        };
        let parsed4 = CrossRefClient::parse_crossref_date(&date4);
        assert!(parsed4.is_none());
    }

    #[test]
    fn test_format_author_name() {
        // Full name
        let author1 = CrossRefAuthor {
            given: Some("John".to_string()),
            family: Some("Doe".to_string()),
            name: None,
            orcid: None,
        };
        assert_eq!(
            CrossRefClient::format_author_name(&author1),
            "John Doe"
        );

        // Name field only
        let author2 = CrossRefAuthor {
            given: None,
            family: None,
            name: Some("Jane Smith".to_string()),
            orcid: None,
        };
        assert_eq!(
            CrossRefClient::format_author_name(&author2),
            "Jane Smith"
        );

        // Family name only
        let author3 = CrossRefAuthor {
            given: None,
            family: Some("Einstein".to_string()),
            name: None,
            orcid: None,
        };
        assert_eq!(
            CrossRefClient::format_author_name(&author3),
            "Einstein"
        );
    }

    #[test]
    fn test_work_to_vector() {
        let client = CrossRefClient::new(None);

        let work = CrossRefWork {
            doi: "10.1234/example.2024".to_string(),
            title: vec!["Deep Learning for Climate Science".to_string()],
            abstract_text: Some("We propose a novel approach to climate modeling...".to_string()),
            author: vec![
                CrossRefAuthor {
                    given: Some("Alice".to_string()),
                    family: Some("Johnson".to_string()),
                    name: None,
                    orcid: Some("0000-0001-2345-6789".to_string()),
                },
                CrossRefAuthor {
                    given: Some("Bob".to_string()),
                    family: Some("Smith".to_string()),
                    name: None,
                    orcid: None,
                },
            ],
            published_print: Some(CrossRefDate {
                date_parts: vec![vec![2024, 6, 15]],
            }),
            published_online: None,
            container_title: vec!["Nature Climate Change".to_string()],
            citation_count: Some(42),
            references_count: Some(35),
            subject: vec!["Climate Science".to_string(), "Machine Learning".to_string()],
            funder: vec![CrossRefFunder {
                name: Some("National Science Foundation".to_string()),
                doi: Some("10.13039/100000001".to_string()),
            }],
            work_type: Some("journal-article".to_string()),
            publisher: Some("Nature Publishing Group".to_string()),
        };

        let vector = client.work_to_vector(work);

        assert_eq!(vector.id, "doi:10.1234/example.2024");
        assert_eq!(vector.domain, Domain::Research);
        assert_eq!(
            vector.metadata.get("doi").unwrap(),
            "10.1234/example.2024"
        );
        assert_eq!(
            vector.metadata.get("title").unwrap(),
            "Deep Learning for Climate Science"
        );
        assert_eq!(
            vector.metadata.get("authors").unwrap(),
            "Alice Johnson; Bob Smith"
        );
        assert_eq!(
            vector.metadata.get("journal").unwrap(),
            "Nature Climate Change"
        );
        assert_eq!(vector.metadata.get("citation_count").unwrap(), "42");
        assert_eq!(
            vector.metadata.get("subjects").unwrap(),
            "Climate Science, Machine Learning"
        );
        assert_eq!(
            vector.metadata.get("funders").unwrap(),
            "National Science Foundation"
        );
        assert_eq!(vector.metadata.get("type").unwrap(), "journal-article");
        assert_eq!(
            vector.metadata.get("publisher").unwrap(),
            "Nature Publishing Group"
        );
        assert_eq!(vector.embedding.len(), DEFAULT_EMBEDDING_DIM);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting CrossRef API in tests
    async fn test_search_works_integration() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));
        let results = client.search_works("machine learning", 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        if !vectors.is_empty() {
            let first = &vectors[0];
            assert!(first.id.starts_with("doi:"));
            assert_eq!(first.domain, Domain::Research);
            assert!(first.metadata.contains_key("title"));
            assert!(first.metadata.contains_key("doi"));
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting CrossRef API in tests
    async fn test_get_work_integration() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));

        // Try to fetch a known work (Nature paper on AlphaFold)
        let result = client.get_work("10.1038/s41586-021-03819-2").await;
        assert!(result.is_ok());

        let work = result.unwrap();
        assert!(work.is_some());

        let vector = work.unwrap();
        assert_eq!(vector.id, "doi:10.1038/s41586-021-03819-2");
        assert_eq!(vector.domain, Domain::Research);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting CrossRef API in tests
    async fn test_search_by_funder_integration() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));

        // Search NSF-funded works
        let results = client.search_by_funder("10.13039/100000001", 3).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 3);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting CrossRef API in tests
    async fn test_search_by_type_integration() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));

        // Search for datasets
        let results = client.search_by_type("dataset", Some("climate"), 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting CrossRef API in tests
    async fn test_search_recent_integration() {
        let client = CrossRefClient::new(Some("test@example.com".to_string()));

        // Search recent papers
        let results = client
            .search_recent("quantum computing", "2024-01-01", 5)
            .await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);
    }
}
