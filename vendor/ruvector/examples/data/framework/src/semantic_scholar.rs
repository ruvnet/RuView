//! Semantic Scholar API Integration
//!
//! This module provides an async client for fetching academic papers from Semantic Scholar,
//! converting responses to SemanticVector format for RuVector discovery.
//!
//! # Semantic Scholar API Details
//! - Base URL: https://api.semanticscholar.org/graph/v1
//! - Free tier: 100 requests per 5 minutes without API key
//! - With API key: Higher limits (contact Semantic Scholar)
//! - Returns JSON responses
//!
//! # Example
//! ```rust,ignore
//! use ruvector_data_framework::semantic_scholar::SemanticScholarClient;
//!
//! let client = SemanticScholarClient::new(None); // No API key
//!
//! // Search papers by keywords
//! let vectors = client.search_papers("machine learning", 10).await?;
//!
//! // Get paper details
//! let paper = client.get_paper("649def34f8be52c8b66281af98ae884c09aef38b").await?;
//!
//! // Get citations
//! let citations = client.get_citations("649def34f8be52c8b66281af98ae884c09aef38b", 20).await?;
//!
//! // Search by field of study
//! let cs_papers = client.search_by_field("Computer Science", 50).await?;
//! ```

use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration for Semantic Scholar API
const S2_RATE_LIMIT_MS: u64 = 3000; // 3 seconds between requests (100 req / 5 min = ~20 req/min = 3s/req)
const S2_WITH_KEY_RATE_LIMIT_MS: u64 = 200; // More aggressive with API key
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 2000;
const DEFAULT_EMBEDDING_DIM: usize = 384;

// ============================================================================
// Semantic Scholar API Response Structures
// ============================================================================

/// Search response from Semantic Scholar
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    total: Option<i32>,
    #[serde(default)]
    offset: Option<i32>,
    #[serde(default)]
    next: Option<i32>,
    #[serde(default)]
    data: Vec<PaperData>,
}

/// Paper data structure
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PaperData {
    #[serde(rename = "paperId")]
    paper_id: String,

    #[serde(default)]
    title: Option<String>,

    #[serde(rename = "abstract", default)]
    abstract_text: Option<String>,

    #[serde(default)]
    year: Option<i32>,

    #[serde(rename = "citationCount", default)]
    citation_count: Option<i32>,

    #[serde(rename = "referenceCount", default)]
    reference_count: Option<i32>,

    #[serde(rename = "influentialCitationCount", default)]
    influential_citation_count: Option<i32>,

    #[serde(default)]
    authors: Vec<AuthorData>,

    #[serde(rename = "fieldsOfStudy", default)]
    fields_of_study: Vec<String>,

    #[serde(default)]
    venue: Option<String>,

    #[serde(rename = "publicationVenue", default)]
    publication_venue: Option<PublicationVenue>,

    #[serde(default)]
    url: Option<String>,

    #[serde(rename = "openAccessPdf", default)]
    open_access_pdf: Option<OpenAccessPdf>,
}

/// Author information
#[derive(Debug, Clone, Deserialize, Serialize)]
struct AuthorData {
    #[serde(rename = "authorId", default)]
    author_id: Option<String>,

    #[serde(default)]
    name: Option<String>,
}

/// Publication venue details
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PublicationVenue {
    #[serde(default)]
    name: Option<String>,

    #[serde(rename = "type", default)]
    venue_type: Option<String>,
}

/// Open access PDF information
#[derive(Debug, Clone, Deserialize, Serialize)]
struct OpenAccessPdf {
    #[serde(default)]
    url: Option<String>,

    #[serde(default)]
    status: Option<String>,
}

/// Citation/reference response
#[derive(Debug, Deserialize)]
struct CitationResponse {
    #[serde(default)]
    offset: Option<i32>,

    #[serde(default)]
    next: Option<i32>,

    #[serde(default)]
    data: Vec<CitationData>,
}

/// Citation data wrapper
#[derive(Debug, Deserialize)]
struct CitationData {
    #[serde(rename = "citingPaper", default)]
    citing_paper: Option<PaperData>,

    #[serde(rename = "citedPaper", default)]
    cited_paper: Option<PaperData>,
}

/// Author details response
#[derive(Debug, Deserialize)]
struct AuthorResponse {
    #[serde(rename = "authorId")]
    author_id: String,

    #[serde(default)]
    name: Option<String>,

    #[serde(rename = "paperCount", default)]
    paper_count: Option<i32>,

    #[serde(rename = "citationCount", default)]
    citation_count: Option<i32>,

    #[serde(rename = "hIndex", default)]
    h_index: Option<i32>,

    #[serde(default)]
    papers: Vec<PaperData>,
}

// ============================================================================
// Semantic Scholar Client
// ============================================================================

/// Client for Semantic Scholar API
///
/// Provides methods to search for academic papers, retrieve citations and references,
/// filter by fields of study, and convert results to SemanticVector format for RuVector analysis.
///
/// # Rate Limiting
/// The client automatically enforces rate limits:
/// - Without API key: 100 requests per 5 minutes (3 seconds between requests)
/// - With API key: Higher limits (200ms between requests)
///
/// # API Key
/// Set the `SEMANTIC_SCHOLAR_API_KEY` environment variable to use authenticated requests.
pub struct SemanticScholarClient {
    client: Client,
    embedder: Arc<SimpleEmbedder>,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
}

impl SemanticScholarClient {
    /// Create a new Semantic Scholar API client
    ///
    /// # Arguments
    /// * `api_key` - Optional API key. If None, checks SEMANTIC_SCHOLAR_API_KEY env var
    ///
    /// # Example
    /// ```rust,ignore
    /// // Without API key
    /// let client = SemanticScholarClient::new(None);
    ///
    /// // With API key
    /// let client = SemanticScholarClient::new(Some("your-api-key".to_string()));
    /// ```
    pub fn new(api_key: Option<String>) -> Self {
        Self::with_embedding_dim(api_key, DEFAULT_EMBEDDING_DIM)
    }

    /// Create a new client with custom embedding dimension
    ///
    /// # Arguments
    /// * `api_key` - Optional API key
    /// * `embedding_dim` - Dimension for text embeddings (default: 384)
    pub fn with_embedding_dim(api_key: Option<String>, embedding_dim: usize) -> Self {
        // Try API key from parameter, then environment variable
        let api_key = api_key.or_else(|| env::var("SEMANTIC_SCHOLAR_API_KEY").ok());

        let rate_limit_delay = if api_key.is_some() {
            Duration::from_millis(S2_WITH_KEY_RATE_LIMIT_MS)
        } else {
            Duration::from_millis(S2_RATE_LIMIT_MS)
        };

        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            base_url: "https://api.semanticscholar.org/graph/v1".to_string(),
            api_key,
            rate_limit_delay,
        }
    }

    /// Search papers by keywords
    ///
    /// # Arguments
    /// * `query` - Search query (keywords, title, etc.)
    /// * `limit` - Maximum number of results to return (max 100 per request)
    ///
    /// # Example
    /// ```rust,ignore
    /// let vectors = client.search_papers("deep learning transformers", 50).await?;
    /// ```
    pub async fn search_papers(&self, query: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let limit = limit.min(100); // API limit
        let encoded_query = urlencoding::encode(query);

        let url = format!(
            "{}/paper/search?query={}&limit={}&fields=paperId,title,abstract,year,citationCount,referenceCount,influentialCitationCount,authors,fieldsOfStudy,venue,publicationVenue,url,openAccessPdf",
            self.base_url, encoded_query, limit
        );

        let response: SearchResponse = self.fetch_json(&url).await?;

        let mut vectors = Vec::new();
        for paper in response.data {
            if let Some(vector) = self.paper_to_vector(paper) {
                vectors.push(vector);
            }
        }

        Ok(vectors)
    }

    /// Get a single paper by Semantic Scholar paper ID
    ///
    /// # Arguments
    /// * `paper_id` - Semantic Scholar paper ID (e.g., "649def34f8be52c8b66281af98ae884c09aef38b")
    ///
    /// # Example
    /// ```rust,ignore
    /// let paper = client.get_paper("649def34f8be52c8b66281af98ae884c09aef38b").await?;
    /// ```
    pub async fn get_paper(&self, paper_id: &str) -> Result<Option<SemanticVector>> {
        let url = format!(
            "{}/paper/{}?fields=paperId,title,abstract,year,citationCount,referenceCount,influentialCitationCount,authors,fieldsOfStudy,venue,publicationVenue,url,openAccessPdf",
            self.base_url, paper_id
        );

        let paper: PaperData = self.fetch_json(&url).await?;
        Ok(self.paper_to_vector(paper))
    }

    /// Get papers that cite this paper
    ///
    /// # Arguments
    /// * `paper_id` - Semantic Scholar paper ID
    /// * `limit` - Maximum number of citations to return (max 1000)
    ///
    /// # Example
    /// ```rust,ignore
    /// let citations = client.get_citations("649def34f8be52c8b66281af98ae884c09aef38b", 50).await?;
    /// ```
    pub async fn get_citations(&self, paper_id: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let limit = limit.min(1000); // API limit

        let url = format!(
            "{}/paper/{}/citations?limit={}&fields=paperId,title,abstract,year,citationCount,referenceCount,authors,fieldsOfStudy,venue,url",
            self.base_url, paper_id, limit
        );

        let response: CitationResponse = self.fetch_json(&url).await?;

        let mut vectors = Vec::new();
        for citation in response.data {
            if let Some(citing_paper) = citation.citing_paper {
                if let Some(vector) = self.paper_to_vector(citing_paper) {
                    vectors.push(vector);
                }
            }
        }

        Ok(vectors)
    }

    /// Get papers this paper references
    ///
    /// # Arguments
    /// * `paper_id` - Semantic Scholar paper ID
    /// * `limit` - Maximum number of references to return (max 1000)
    ///
    /// # Example
    /// ```rust,ignore
    /// let references = client.get_references("649def34f8be52c8b66281af98ae884c09aef38b", 50).await?;
    /// ```
    pub async fn get_references(&self, paper_id: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        let limit = limit.min(1000); // API limit

        let url = format!(
            "{}/paper/{}/references?limit={}&fields=paperId,title,abstract,year,citationCount,referenceCount,authors,fieldsOfStudy,venue,url",
            self.base_url, paper_id, limit
        );

        let response: CitationResponse = self.fetch_json(&url).await?;

        let mut vectors = Vec::new();
        for reference in response.data {
            if let Some(cited_paper) = reference.cited_paper {
                if let Some(vector) = self.paper_to_vector(cited_paper) {
                    vectors.push(vector);
                }
            }
        }

        Ok(vectors)
    }

    /// Search papers by field of study
    ///
    /// # Arguments
    /// * `field_of_study` - Field name (e.g., "Computer Science", "Medicine", "Biology", "Physics", "Economics")
    /// * `limit` - Maximum number of results to return
    ///
    /// # Example
    /// ```rust,ignore
    /// let cs_papers = client.search_by_field("Computer Science", 100).await?;
    /// let medical_papers = client.search_by_field("Medicine", 50).await?;
    /// ```
    pub async fn search_by_field(&self, field_of_study: &str, limit: usize) -> Result<Vec<SemanticVector>> {
        // Search for papers in this field, sorted by citation count
        let query = format!("fieldsOfStudy:{}", field_of_study);
        self.search_papers(&query, limit).await
    }

    /// Get author details and their papers
    ///
    /// # Arguments
    /// * `author_id` - Semantic Scholar author ID
    ///
    /// # Example
    /// ```rust,ignore
    /// let author_papers = client.get_author("1741101").await?;
    /// ```
    pub async fn get_author(&self, author_id: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/author/{}?fields=authorId,name,paperCount,citationCount,hIndex,papers.paperId,papers.title,papers.abstract,papers.year,papers.citationCount,papers.fieldsOfStudy",
            self.base_url, author_id
        );

        let author: AuthorResponse = self.fetch_json(&url).await?;

        let mut vectors = Vec::new();
        for paper in author.papers {
            if let Some(vector) = self.paper_to_vector(paper) {
                vectors.push(vector);
            }
        }

        Ok(vectors)
    }

    /// Search recent papers published after a minimum year
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `year_min` - Minimum publication year (e.g., 2020)
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get papers about "climate change" published since 2020
    /// let recent = client.search_recent("climate change", 2020).await?;
    /// ```
    pub async fn search_recent(&self, query: &str, year_min: i32) -> Result<Vec<SemanticVector>> {
        let all_results = self.search_papers(query, 100).await?;

        // Filter by year
        Ok(all_results
            .into_iter()
            .filter(|v| {
                v.metadata
                    .get("year")
                    .and_then(|y| y.parse::<i32>().ok())
                    .map(|year| year >= year_min)
                    .unwrap_or(false)
            })
            .collect())
    }

    /// Build citation graph for a paper
    ///
    /// Returns a tuple of (paper, citations, references) as SemanticVectors
    ///
    /// # Arguments
    /// * `paper_id` - Semantic Scholar paper ID
    /// * `max_citations` - Maximum citations to retrieve
    /// * `max_references` - Maximum references to retrieve
    ///
    /// # Example
    /// ```rust,ignore
    /// let (paper, citations, references) = client.build_citation_graph(
    ///     "649def34f8be52c8b66281af98ae884c09aef38b",
    ///     50,
    ///     50
    /// ).await?;
    /// ```
    pub async fn build_citation_graph(
        &self,
        paper_id: &str,
        max_citations: usize,
        max_references: usize,
    ) -> Result<(Option<SemanticVector>, Vec<SemanticVector>, Vec<SemanticVector>)> {
        // Fetch paper, citations, and references in parallel
        let paper_result = self.get_paper(paper_id);
        let citations_result = self.get_citations(paper_id, max_citations);
        let references_result = self.get_references(paper_id, max_references);

        // Wait for all with proper spacing for rate limiting
        let paper = paper_result.await?;
        sleep(self.rate_limit_delay).await;

        let citations = citations_result.await?;
        sleep(self.rate_limit_delay).await;

        let references = references_result.await?;

        Ok((paper, citations, references))
    }

    /// Convert PaperData to SemanticVector
    fn paper_to_vector(&self, paper: PaperData) -> Option<SemanticVector> {
        let title = paper.title.clone().unwrap_or_default();
        let abstract_text = paper.abstract_text.clone().unwrap_or_default();

        // Skip papers without title
        if title.is_empty() {
            return None;
        }

        // Generate embedding from title + abstract
        let combined_text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&combined_text);

        // Convert year to timestamp
        let timestamp = paper.year
            .and_then(|y| NaiveDate::from_ymd_opt(y, 1, 1))
            .map(|d| DateTime::from_naive_utc_and_offset(d.and_hms_opt(0, 0, 0).unwrap(), Utc))
            .unwrap_or_else(Utc::now);

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert("paper_id".to_string(), paper.paper_id.clone());
        metadata.insert("title".to_string(), title);

        if !abstract_text.is_empty() {
            metadata.insert("abstract".to_string(), abstract_text);
        }

        if let Some(year) = paper.year {
            metadata.insert("year".to_string(), year.to_string());
        }

        if let Some(count) = paper.citation_count {
            metadata.insert("citationCount".to_string(), count.to_string());
        }

        if let Some(count) = paper.reference_count {
            metadata.insert("referenceCount".to_string(), count.to_string());
        }

        if let Some(count) = paper.influential_citation_count {
            metadata.insert("influentialCitationCount".to_string(), count.to_string());
        }

        // Authors
        let authors = paper
            .authors
            .iter()
            .filter_map(|a| a.name.as_ref())
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        if !authors.is_empty() {
            metadata.insert("authors".to_string(), authors);
        }

        // Fields of study
        if !paper.fields_of_study.is_empty() {
            metadata.insert("fieldsOfStudy".to_string(), paper.fields_of_study.join(", "));
        }

        // Venue
        if let Some(venue) = paper.venue.or_else(|| paper.publication_venue.and_then(|pv| pv.name)) {
            metadata.insert("venue".to_string(), venue);
        }

        // URL
        if let Some(url) = paper.url {
            metadata.insert("url".to_string(), url);
        } else {
            metadata.insert(
                "url".to_string(),
                format!("https://www.semanticscholar.org/paper/{}", paper.paper_id),
            );
        }

        // Open access PDF
        if let Some(pdf) = paper.open_access_pdf.and_then(|p| p.url) {
            metadata.insert("pdf_url".to_string(), pdf);
        }

        metadata.insert("source".to_string(), "semantic_scholar".to_string());

        Some(SemanticVector {
            id: format!("s2:{}", paper.paper_id),
            embedding,
            domain: Domain::Research,
            timestamp,
            metadata,
        })
    }

    /// Fetch JSON from URL with rate limiting and retry logic
    async fn fetch_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        // Rate limiting
        sleep(self.rate_limit_delay).await;

        let response = self.fetch_with_retry(url).await?;
        let json = response.json::<T>().await?;

        Ok(json)
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            let mut request = self.client.get(url);

            // Add API key header if available
            if let Some(ref api_key) = self.api_key {
                request = request.header("x-api-key", api_key);
            }

            match request.send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES {
                        retries += 1;
                        let delay = RETRY_DELAY_MS * (2_u64.pow(retries - 1)); // Exponential backoff
                        tracing::warn!(
                            "Rate limited by Semantic Scholar, retrying in {}ms",
                            delay
                        );
                        sleep(Duration::from_millis(delay)).await;
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
                    let delay = RETRY_DELAY_MS * (2_u64.pow(retries - 1)); // Exponential backoff
                    tracing::warn!("Request failed, retrying ({}/{}) in {}ms", retries, MAX_RETRIES, delay);
                    sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for SemanticScholarClient {
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
    fn test_client_creation() {
        let client = SemanticScholarClient::new(None);
        assert_eq!(client.base_url, "https://api.semanticscholar.org/graph/v1");
        assert_eq!(client.rate_limit_delay, Duration::from_millis(S2_RATE_LIMIT_MS));
    }

    #[test]
    fn test_client_with_api_key() {
        let client = SemanticScholarClient::new(Some("test-key".to_string()));
        assert_eq!(client.api_key, Some("test-key".to_string()));
        assert_eq!(client.rate_limit_delay, Duration::from_millis(S2_WITH_KEY_RATE_LIMIT_MS));
    }

    #[test]
    fn test_custom_embedding_dim() {
        let client = SemanticScholarClient::with_embedding_dim(None, 512);
        let embedding = client.embedder.embed_text("test");
        assert_eq!(embedding.len(), 512);
    }

    #[test]
    fn test_paper_to_vector() {
        let client = SemanticScholarClient::new(None);

        let paper = PaperData {
            paper_id: "649def34f8be52c8b66281af98ae884c09aef38b".to_string(),
            title: Some("Attention Is All You Need".to_string()),
            abstract_text: Some("The dominant sequence transduction models...".to_string()),
            year: Some(2017),
            citation_count: Some(50000),
            reference_count: Some(35),
            influential_citation_count: Some(5000),
            authors: vec![
                AuthorData {
                    author_id: Some("1741101".to_string()),
                    name: Some("Ashish Vaswani".to_string()),
                },
                AuthorData {
                    author_id: Some("1699545".to_string()),
                    name: Some("Noam Shazeer".to_string()),
                },
            ],
            fields_of_study: vec!["Computer Science".to_string(), "Mathematics".to_string()],
            venue: Some("NeurIPS".to_string()),
            publication_venue: None,
            url: Some("https://arxiv.org/abs/1706.03762".to_string()),
            open_access_pdf: Some(OpenAccessPdf {
                url: Some("https://arxiv.org/pdf/1706.03762.pdf".to_string()),
                status: Some("GREEN".to_string()),
            }),
        };

        let vector = client.paper_to_vector(paper);
        assert!(vector.is_some());

        let v = vector.unwrap();
        assert_eq!(v.id, "s2:649def34f8be52c8b66281af98ae884c09aef38b");
        assert_eq!(v.domain, Domain::Research);
        assert_eq!(v.metadata.get("paper_id").unwrap(), "649def34f8be52c8b66281af98ae884c09aef38b");
        assert_eq!(v.metadata.get("title").unwrap(), "Attention Is All You Need");
        assert_eq!(v.metadata.get("year").unwrap(), "2017");
        assert_eq!(v.metadata.get("citationCount").unwrap(), "50000");
        assert_eq!(v.metadata.get("referenceCount").unwrap(), "35");
        assert_eq!(v.metadata.get("authors").unwrap(), "Ashish Vaswani, Noam Shazeer");
        assert_eq!(v.metadata.get("fieldsOfStudy").unwrap(), "Computer Science, Mathematics");
        assert_eq!(v.metadata.get("venue").unwrap(), "NeurIPS");
        assert!(v.metadata.contains_key("pdf_url"));
    }

    #[test]
    fn test_paper_to_vector_minimal() {
        let client = SemanticScholarClient::new(None);

        let paper = PaperData {
            paper_id: "test123".to_string(),
            title: Some("Minimal Paper".to_string()),
            abstract_text: None,
            year: None,
            citation_count: None,
            reference_count: None,
            influential_citation_count: None,
            authors: vec![],
            fields_of_study: vec![],
            venue: None,
            publication_venue: None,
            url: None,
            open_access_pdf: None,
        };

        let vector = client.paper_to_vector(paper);
        assert!(vector.is_some());

        let v = vector.unwrap();
        assert_eq!(v.id, "s2:test123");
        assert_eq!(v.metadata.get("title").unwrap(), "Minimal Paper");
        assert!(v.metadata.get("url").unwrap().contains("semanticscholar.org"));
    }

    #[test]
    fn test_paper_without_title() {
        let client = SemanticScholarClient::new(None);

        let paper = PaperData {
            paper_id: "test456".to_string(),
            title: None,
            abstract_text: Some("Has abstract but no title".to_string()),
            year: Some(2020),
            citation_count: None,
            reference_count: None,
            influential_citation_count: None,
            authors: vec![],
            fields_of_study: vec![],
            venue: None,
            publication_venue: None,
            url: None,
            open_access_pdf: None,
        };

        // Papers without titles should be skipped
        let vector = client.paper_to_vector(paper);
        assert!(vector.is_none());
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting Semantic Scholar API in tests
    async fn test_search_papers_integration() {
        let client = SemanticScholarClient::new(None);
        let results = client.search_papers("machine learning", 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);

        if !vectors.is_empty() {
            let first = &vectors[0];
            assert!(first.id.starts_with("s2:"));
            assert_eq!(first.domain, Domain::Research);
            assert!(first.metadata.contains_key("title"));
            assert!(first.metadata.contains_key("paper_id"));
        }
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting Semantic Scholar API
    async fn test_get_paper_integration() {
        let client = SemanticScholarClient::new(None);

        // Well-known paper: "Attention Is All You Need"
        let result = client.get_paper("649def34f8be52c8b66281af98ae884c09aef38b").await;
        assert!(result.is_ok());

        let paper = result.unwrap();
        assert!(paper.is_some());

        let p = paper.unwrap();
        assert_eq!(p.id, "s2:649def34f8be52c8b66281af98ae884c09aef38b");
        assert!(p.metadata.get("title").unwrap().contains("Attention"));
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting Semantic Scholar API
    async fn test_get_citations_integration() {
        let client = SemanticScholarClient::new(None);

        // Get citations for "Attention Is All You Need"
        let result = client.get_citations("649def34f8be52c8b66281af98ae884c09aef38b", 10).await;
        assert!(result.is_ok());

        let citations = result.unwrap();
        assert!(citations.len() <= 10);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting Semantic Scholar API
    async fn test_search_by_field_integration() {
        let client = SemanticScholarClient::new(None);
        let results = client.search_by_field("Computer Science", 5).await;
        assert!(results.is_ok());

        let vectors = results.unwrap();
        assert!(vectors.len() <= 5);
    }

    #[tokio::test]
    #[ignore] // Ignore by default to avoid hitting Semantic Scholar API
    async fn test_build_citation_graph_integration() {
        let client = SemanticScholarClient::new(None);

        let result = client.build_citation_graph(
            "649def34f8be52c8b66281af98ae884c09aef38b",
            5,
            5
        ).await;
        assert!(result.is_ok());

        let (paper, citations, references) = result.unwrap();
        assert!(paper.is_some());
    }
}
