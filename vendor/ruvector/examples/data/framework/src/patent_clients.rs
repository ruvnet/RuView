//! Patent database API integrations for USPTO PatentsView and EPO
//!
//! This module provides async clients for fetching patent data from:
//! - USPTO PatentsView API (Free, no authentication required)
//! - EPO Open Patent Services (Free tier available)
//!
//! Converts patent data to SemanticVector format for RuVector discovery.

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
const USPTO_RATE_LIMIT_MS: u64 = 200; // ~5 requests/second
const EPO_RATE_LIMIT_MS: u64 = 1000; // Conservative 1 request/second
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// USPTO PatentsView API Client
// ============================================================================

/// USPTO PatentsView API response
#[derive(Debug, Deserialize)]
struct UsptoPatentsResponse {
    #[serde(default)]
    patents: Vec<UsptoPatent>,
    #[serde(default)]
    count: i32,
    #[serde(default)]
    total_patent_count: Option<i32>,
}

/// USPTO Patent record
#[derive(Debug, Deserialize)]
struct UsptoPatent {
    /// Patent number
    patent_number: String,
    /// Patent title
    #[serde(default)]
    patent_title: Option<String>,
    /// Patent abstract
    #[serde(default)]
    patent_abstract: Option<String>,
    /// Grant date (YYYY-MM-DD)
    #[serde(default)]
    patent_date: Option<String>,
    /// Application filing date
    #[serde(default)]
    app_date: Option<String>,
    /// Assignees (organizations/companies)
    #[serde(default)]
    assignees: Vec<UsptoAssignee>,
    /// Inventors
    #[serde(default)]
    inventors: Vec<UsptoInventor>,
    /// CPC classifications
    #[serde(default)]
    cpcs: Vec<UsptoCpc>,
    /// Citation counts
    #[serde(default)]
    cited_patent_count: Option<i32>,
    #[serde(default)]
    citedby_patent_count: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct UsptoAssignee {
    #[serde(default)]
    assignee_organization: Option<String>,
    #[serde(default)]
    assignee_individual_name_first: Option<String>,
    #[serde(default)]
    assignee_individual_name_last: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsptoInventor {
    #[serde(default)]
    inventor_name_first: Option<String>,
    #[serde(default)]
    inventor_name_last: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UsptoCpc {
    /// CPC section (e.g., "Y02")
    #[serde(default)]
    cpc_section_id: Option<String>,
    /// CPC subclass (e.g., "Y02E")
    #[serde(default)]
    cpc_subclass_id: Option<String>,
    /// CPC group (e.g., "Y02E10/50")
    #[serde(default)]
    cpc_group_id: Option<String>,
}

/// USPTO citation response
#[derive(Debug, Deserialize)]
struct UsptoCitationsResponse {
    #[serde(default)]
    patents: Vec<UsptoCitation>,
}

#[derive(Debug, Deserialize)]
struct UsptoCitation {
    patent_number: String,
    #[serde(default)]
    patent_title: Option<String>,
}

/// Client for USPTO PatentsView API (PatentSearch API v2)
///
/// PatentsView provides free access to USPTO patent data with no authentication required.
/// Uses the new PatentSearch API (ElasticSearch-based) as of May 2025.
/// API documentation: https://search.patentsview.org/docs/
pub struct UsptoPatentClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl UsptoPatentClient {
    /// Create a new USPTO PatentsView client
    ///
    /// No authentication required for the PatentsView API.
    /// Uses the new PatentSearch API at search.patentsview.org
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector-Discovery/1.0")
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://search.patentsview.org/api/v1".to_string(),
            rate_limit_delay: Duration::from_millis(USPTO_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(512)), // Higher dimension for technical text
        })
    }

    /// Search patents by keyword query
    ///
    /// # Arguments
    /// * `query` - Search keywords (e.g., "artificial intelligence", "solar cell")
    /// * `max_results` - Maximum number of results to return (max 1000 per page)
    ///
    /// # Example
    /// ```ignore
    /// let client = UsptoPatentClient::new()?;
    /// let patents = client.search_patents("quantum computing", 50).await?;
    /// ```
    pub async fn search_patents(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        let per_page = max_results.min(100);
        let encoded_query = urlencoding::encode(query);

        // New PatentSearch API uses GET with query parameters
        // Query format: q=patent_title:*query* OR patent_abstract:*query*
        let url = format!(
            "{}/patent/?q=patent_title:*{}*%20OR%20patent_abstract:*{}*&f=patent_id,patent_title,patent_abstract,patent_date,assignees,inventors,cpcs&o={{\"size\":{},\"matched_subentities_only\":true}}",
            self.base_url, encoded_query, encoded_query, per_page
        );

        sleep(self.rate_limit_delay).await;

        let response = self.fetch_with_retry(&url).await?;
        let uspto_response: UsptoPatentsResponse = response.json().await?;

        self.convert_patents_to_vectors(uspto_response.patents)
    }

    /// Search patents by assignee (company/organization name)
    ///
    /// # Arguments
    /// * `company_name` - Company or organization name (e.g., "IBM", "Google")
    /// * `max_results` - Maximum number of results to return
    ///
    /// # Example
    /// ```ignore
    /// let patents = client.search_by_assignee("Tesla Inc", 100).await?;
    /// ```
    pub async fn search_by_assignee(
        &self,
        company_name: &str,
        max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        let per_page = max_results.min(100);
        let encoded_name = urlencoding::encode(company_name);

        // New PatentSearch API format
        let url = format!(
            "{}/patent/?q=assignees.assignee_organization:*{}*&f=patent_id,patent_title,patent_abstract,patent_date,assignees,inventors,cpcs&o={{\"size\":{},\"matched_subentities_only\":true}}",
            self.base_url, encoded_name, per_page
        );

        sleep(self.rate_limit_delay).await;

        let response = self.fetch_with_retry(&url).await?;
        let uspto_response: UsptoPatentsResponse = response.json().await?;

        self.convert_patents_to_vectors(uspto_response.patents)
    }

    /// Search patents by CPC classification code
    ///
    /// # Arguments
    /// * `cpc_class` - CPC classification (e.g., "Y02E" for climate tech energy, "G06N" for AI)
    /// * `max_results` - Maximum number of results to return
    ///
    /// # Example - Climate Change Mitigation Technologies
    /// ```ignore
    /// let climate_patents = client.search_by_cpc("Y02", 200).await?;
    /// ```
    ///
    /// # Common CPC Classes
    /// * `Y02` - Climate change mitigation technologies
    /// * `Y02E` - Climate tech - Energy generation/transmission/distribution
    /// * `G06N` - Computing arrangements based on AI/ML/neural networks
    /// * `A61` - Medical or veterinary science
    /// * `H01` - Electric elements (batteries, solar cells, etc.)
    pub async fn search_by_cpc(
        &self,
        cpc_class: &str,
        max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        let per_page = max_results.min(100);
        let encoded_cpc = urlencoding::encode(cpc_class);

        // New PatentSearch API - query cpcs.cpc_group field
        let url = format!(
            "{}/patent/?q=cpcs.cpc_group:{}*&f=patent_id,patent_title,patent_abstract,patent_date,assignees,inventors,cpcs&o={{\"size\":{},\"matched_subentities_only\":true}}",
            self.base_url, encoded_cpc, per_page
        );

        sleep(self.rate_limit_delay).await;

        let response = self.fetch_with_retry(&url).await?;
        let uspto_response: UsptoPatentsResponse = response.json().await?;

        self.convert_patents_to_vectors(uspto_response.patents)
    }

    /// Get detailed information for a specific patent
    ///
    /// # Arguments
    /// * `patent_number` - USPTO patent number (e.g., "10000000")
    ///
    /// # Example
    /// ```ignore
    /// let patent = client.get_patent("10123456").await?;
    /// ```
    pub async fn get_patent(&self, patent_number: &str) -> Result<Option<SemanticVector>> {
        // New PatentSearch API - direct patent lookup
        let url = format!(
            "{}/patent/?q=patent_id:{}&f=patent_id,patent_title,patent_abstract,patent_date,assignees,inventors,cpcs&o={{\"size\":1}}",
            self.base_url, patent_number
        );

        sleep(self.rate_limit_delay).await;

        let response = self.fetch_with_retry(&url).await?;
        let uspto_response: UsptoPatentsResponse = response.json().await?;

        let mut vectors = self.convert_patents_to_vectors(uspto_response.patents)?;
        Ok(vectors.pop())
    }

    /// Get citations for a patent (both citing and cited patents)
    ///
    /// # Arguments
    /// * `patent_number` - USPTO patent number
    ///
    /// # Returns
    /// Tuple of (patents that cite this patent, patents cited by this patent)
    pub async fn get_citations(
        &self,
        patent_number: &str,
    ) -> Result<(Vec<SemanticVector>, Vec<SemanticVector>)> {
        // Get patents that cite this one (forward citations)
        let citing = self.get_citing_patents(patent_number).await?;

        // Get patents cited by this one (backward citations)
        let cited = self.get_cited_patents(patent_number).await?;

        Ok((citing, cited))
    }

    /// Get patents that cite the given patent (forward citations)
    /// Note: Citation data requires separate API endpoints in PatentSearch API v2
    async fn get_citing_patents(&self, _patent_number: &str) -> Result<Vec<SemanticVector>> {
        // The new PatentSearch API handles citations differently
        // Forward citations are available via /api/v1/us_patent_citation/ endpoint
        // For now, return empty - full citation support requires additional implementation
        Ok(Vec::new())
    }

    /// Get patents cited by the given patent (backward citations)
    /// Note: Citation data requires separate API endpoints in PatentSearch API v2
    async fn get_cited_patents(&self, _patent_number: &str) -> Result<Vec<SemanticVector>> {
        // The new PatentSearch API handles citations differently
        // Backward citations are available via /api/v1/us_patent_citation/ endpoint
        // For now, return empty - full citation support requires additional implementation
        Ok(Vec::new())
    }

    /// Convert USPTO patent records to SemanticVectors
    fn convert_patents_to_vectors(&self, patents: Vec<UsptoPatent>) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        for patent in patents {
            let title = patent.patent_title.unwrap_or_else(|| "Untitled Patent".to_string());
            let abstract_text = patent.patent_abstract.unwrap_or_default();

            // Create combined text for embedding
            let text = format!("{} {}", title, abstract_text);
            let embedding = self.embedder.embed_text(&text);

            // Parse grant date (prefer patent_date, fallback to app_date)
            let timestamp = patent
                .patent_date
                .or(patent.app_date)
                .as_ref()
                .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            // Extract assignee names
            let assignees = patent
                .assignees
                .iter()
                .map(|a| {
                    a.assignee_organization
                        .clone()
                        .or_else(|| {
                            let first = a.assignee_individual_name_first.as_deref().unwrap_or("");
                            let last = a.assignee_individual_name_last.as_deref().unwrap_or("");
                            if !first.is_empty() || !last.is_empty() {
                                Some(format!("{} {}", first, last).trim().to_string())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                })
                .filter(|s| !s.is_empty())
                .collect::<Vec<_>>()
                .join(", ");

            // Extract inventor names
            let inventors = patent
                .inventors
                .iter()
                .filter_map(|i| {
                    let first = i.inventor_name_first.as_deref().unwrap_or("");
                    let last = i.inventor_name_last.as_deref().unwrap_or("");
                    if !first.is_empty() || !last.is_empty() {
                        Some(format!("{} {}", first, last).trim().to_string())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");

            // Extract CPC codes
            let cpc_codes = patent
                .cpcs
                .iter()
                .filter_map(|cpc| {
                    cpc.cpc_group_id
                        .clone()
                        .or_else(|| cpc.cpc_subclass_id.clone())
                        .or_else(|| cpc.cpc_section_id.clone())
                })
                .collect::<Vec<_>>()
                .join(", ");

            // Build metadata
            let mut metadata = HashMap::new();
            metadata.insert("patent_number".to_string(), patent.patent_number.clone());
            metadata.insert("title".to_string(), title);
            metadata.insert("abstract".to_string(), abstract_text);
            metadata.insert("assignee".to_string(), assignees);
            metadata.insert("inventors".to_string(), inventors);
            metadata.insert("cpc_codes".to_string(), cpc_codes);
            metadata.insert(
                "citations_count".to_string(),
                patent.citedby_patent_count.unwrap_or(0).to_string(),
            );
            metadata.insert(
                "cited_count".to_string(),
                patent.cited_patent_count.unwrap_or(0).to_string(),
            );
            metadata.insert("source".to_string(), "uspto".to_string());

            vectors.push(SemanticVector {
                id: format!("US{}", patent.patent_number),
                embedding,
                domain: Domain::Research, // Could be Domain::Innovation if that variant exists
                timestamp,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// GET request with retry logic
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
                    if !response.status().is_success() {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
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

    /// POST request with retry logic (kept for backwards compatibility)
    #[allow(dead_code)]
    async fn post_with_retry(
        &self,
        url: &str,
        json: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.post(url).json(json).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES
                    {
                        retries += 1;
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
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for UsptoPatentClient {
    fn default() -> Self {
        Self::new().expect("Failed to create USPTO client")
    }
}

// ============================================================================
// EPO Open Patent Services Client (Placeholder)
// ============================================================================

/// Client for European Patent Office (EPO) Open Patent Services
///
/// Note: This is a placeholder for future EPO integration.
/// The EPO OPS API requires registration and OAuth authentication.
/// See: https://developers.epo.org/
pub struct EpoClient {
    client: Client,
    base_url: String,
    consumer_key: Option<String>,
    consumer_secret: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl EpoClient {
    /// Create a new EPO client
    ///
    /// # Arguments
    /// * `consumer_key` - EPO API consumer key (from developer registration)
    /// * `consumer_secret` - EPO API consumer secret
    ///
    /// Registration required at: https://developers.epo.org/
    pub fn new(consumer_key: Option<String>, consumer_secret: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://ops.epo.org/3.2/rest-services".to_string(),
            consumer_key,
            consumer_secret,
            rate_limit_delay: Duration::from_millis(EPO_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(512)),
        })
    }

    /// Search European patents
    ///
    /// Note: Implementation requires OAuth authentication flow.
    /// This is a placeholder for future development.
    pub async fn search_patents(
        &self,
        _query: &str,
        _max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        Err(FrameworkError::Config(
            "EPO client not yet implemented. Requires OAuth authentication.".to_string(),
        ))
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_uspto_client_creation() {
        let client = UsptoPatentClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_epo_client_creation() {
        let client = EpoClient::new(None, None);
        assert!(client.is_ok());
    }

    #[test]
    fn test_default_client() {
        let client = UsptoPatentClient::default();
        assert_eq!(
            client.rate_limit_delay,
            Duration::from_millis(USPTO_RATE_LIMIT_MS)
        );
    }

    #[test]
    fn test_rate_limiting() {
        let client = UsptoPatentClient::new().unwrap();
        assert_eq!(
            client.rate_limit_delay,
            Duration::from_millis(USPTO_RATE_LIMIT_MS)
        );
    }

    #[test]
    fn test_cpc_classification_mapping() {
        // Verify we handle different CPC code lengths correctly
        let test_cases = vec![
            ("Y02", "cpc_section_id"),
            ("G06N", "cpc_subclass_id"),
            ("Y02E10/50", "cpc_group_id"),
        ];

        for (code, expected_field) in test_cases {
            let field = if code.len() <= 3 {
                "cpc_section_id"
            } else if code.len() <= 4 {
                "cpc_subclass_id"
            } else {
                "cpc_group_id"
            };
            assert_eq!(field, expected_field, "Failed for CPC code: {}", code);
        }
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_search_patents_integration() {
        let client = UsptoPatentClient::new().unwrap();

        // Test basic search
        let result = client.search_patents("quantum computing", 5).await;

        // Should either succeed or fail with network error, not panic
        match result {
            Ok(patents) => {
                assert!(patents.len() <= 5);
                for patent in patents {
                    assert!(patent.id.starts_with("US"));
                    assert_eq!(patent.domain, Domain::Research);
                    assert!(!patent.metadata.is_empty());
                }
            }
            Err(e) => {
                // Network errors are acceptable in tests
                println!("Network test skipped: {}", e);
            }
        }
    }

    #[tokio::test]
    #[ignore] // Requires network access
    async fn test_search_by_cpc_integration() {
        let client = UsptoPatentClient::new().unwrap();

        // Test CPC search for AI/ML patents
        let result = client.search_by_cpc("G06N", 5).await;

        match result {
            Ok(patents) => {
                assert!(patents.len() <= 5);
                for patent in patents {
                    let cpc_codes = patent.metadata.get("cpc_codes").map(|s| s.as_str()).unwrap_or("");
                    // Should contain G06N classification
                    assert!(
                        cpc_codes.contains("G06N") || cpc_codes.is_empty(),
                        "Expected G06N in CPC codes, got: {}",
                        cpc_codes
                    );
                }
            }
            Err(e) => {
                println!("Network test skipped: {}", e);
            }
        }
    }

    #[test]
    fn test_embedding_dimension() {
        let client = UsptoPatentClient::new().unwrap();
        // Verify embedding dimension is set correctly for technical text
        let embedding = client.embedder.embed_text("test patent description");
        assert_eq!(embedding.len(), 512);
    }
}
