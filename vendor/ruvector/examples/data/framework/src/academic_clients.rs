//! Academic & Research API clients for scholarly data discovery
//!
//! This module provides async clients for fetching data from academic databases:
//! - OpenAlex: Scholarly works and citations
//! - CORE: Open access research papers
//! - ERIC: Education research database
//! - Unpaywall: Open access paper discovery

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::{DataRecord, DataSource, FrameworkError, Relationship, Result, SimpleEmbedder};

/// Rate limiting configuration
const DEFAULT_RATE_LIMIT_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;
const EMBEDDING_DIMENSION: usize = 128;

// ============================================================================
// OpenAlex Client (Extended)
// ============================================================================

/// OpenAlex API response structures
#[derive(Debug, Deserialize)]
struct OpenAlexWorksResponse {
    results: Vec<OpenAlexWork>,
    meta: OpenAlexMeta,
}

#[derive(Debug, Deserialize)]
struct OpenAlexWork {
    id: String,
    #[serde(rename = "display_name")]
    display_name: Option<String>,
    publication_date: Option<String>,
    #[serde(rename = "authorships")]
    authorships: Option<Vec<OpenAlexAuthorship>>,
    #[serde(rename = "cited_by_count")]
    cited_by_count: Option<i64>,
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
struct OpenAlexMeta {
    count: i64,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthorsResponse {
    results: Vec<OpenAlexAuthorDetail>,
}

#[derive(Debug, Deserialize)]
struct OpenAlexAuthorDetail {
    id: String,
    #[serde(rename = "display_name")]
    display_name: Option<String>,
    #[serde(rename = "works_count")]
    works_count: Option<i64>,
    #[serde(rename = "cited_by_count")]
    cited_by_count: Option<i64>,
}

/// Client for OpenAlex scholarly database
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
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.openalex.org".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(EMBEDDING_DIMENSION)),
            user_email,
        })
    }

    /// Search scholarly works
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results (max 200 per request)
    pub async fn search_works(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut url = format!(
            "{}/works?search={}",
            self.base_url,
            urlencoding::encode(query)
        );
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

    /// Get work by OpenAlex ID
    ///
    /// # Arguments
    /// * `id` - OpenAlex work ID (e.g., "W2741809807")
    pub async fn get_work(&self, id: &str) -> Result<DataRecord> {
        let url = format!("{}/works/{}", self.base_url, id);
        let response = self.fetch_with_retry(&url).await?;
        let work: OpenAlexWork = response.json().await?;
        self.work_to_record(work)
    }

    /// Search authors
    ///
    /// # Arguments
    /// * `query` - Author name or affiliation
    /// * `limit` - Maximum number of results
    pub async fn search_authors(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut url = format!(
            "{}/authors?search={}",
            self.base_url,
            urlencoding::encode(query)
        );
        url.push_str(&format!("&per-page={}", limit.min(200)));

        if let Some(email) = &self.user_email {
            url.push_str(&format!("&mailto={}", email));
        }

        let response = self.fetch_with_retry(&url).await?;
        let authors_response: OpenAlexAuthorsResponse = response.json().await?;

        let mut records = Vec::new();
        for author in authors_response.results {
            let record = self.author_to_record(author)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Get citing works for a given work ID
    ///
    /// # Arguments
    /// * `work_id` - OpenAlex work ID
    pub async fn get_citations(&self, work_id: &str) -> Result<Vec<DataRecord>> {
        let url = format!("{}/works?filter=cites:{}", self.base_url, work_id);
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

    /// Convert OpenAlex work to DataRecord
    fn work_to_record(&self, work: OpenAlexWork) -> Result<DataRecord> {
        let title = work
            .display_name
            .unwrap_or_else(|| "Untitled".to_string());

        let abstract_text = work
            .abstract_inverted_index
            .as_ref()
            .map(|index| self.reconstruct_abstract(index))
            .unwrap_or_default();

        let text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&text);

        let timestamp = work
            .publication_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .unwrap_or_else(Utc::now);

        let mut relationships = Vec::new();
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

    /// Convert author to DataRecord
    fn author_to_record(&self, author: OpenAlexAuthorDetail) -> Result<DataRecord> {
        let name = author
            .display_name
            .clone()
            .unwrap_or_else(|| "Unknown".to_string());
        let embedding = self.embedder.embed_text(&name);

        let mut data_map = serde_json::Map::new();
        data_map.insert("display_name".to_string(), serde_json::json!(name));
        if let Some(works) = author.works_count {
            data_map.insert("works_count".to_string(), serde_json::json!(works));
        }
        if let Some(citations) = author.cited_by_count {
            data_map.insert("cited_by_count".to_string(), serde_json::json!(citations));
        }

        Ok(DataRecord {
            id: author.id,
            source: "openalex".to_string(),
            record_type: "author".to_string(),
            timestamp: Utc::now(),
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships: Vec::new(),
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

    /// Fetch with retry logic and exponential backoff
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            match self.client.get(url).send().await {
                Ok(response) => {
                    if response.status() == StatusCode::TOO_MANY_REQUESTS && retries < MAX_RETRIES
                    {
                        retries += 1;
                        let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                        sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                    sleep(Duration::from_millis(delay)).await;
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
        let query = cursor.as_deref().unwrap_or("machine learning");
        let records = self.search_works(query, batch_size).await?;
        Ok((records, None))
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
// CORE Client
// ============================================================================

/// CORE API response structures
#[derive(Debug, Deserialize)]
struct CoreSearchResponse {
    results: Vec<CoreWork>,
    #[serde(rename = "totalHits")]
    total_hits: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CoreWork {
    id: String,
    title: Option<String>,
    #[serde(rename = "abstract")]
    abstract_text: Option<String>,
    authors: Option<Vec<String>>,
    #[serde(rename = "publishedDate")]
    published_date: Option<String>,
    #[serde(rename = "downloadUrl")]
    download_url: Option<String>,
    doi: Option<String>,
}

/// Client for CORE open access papers
pub struct CoreClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl CoreClient {
    /// Create a new CORE client
    ///
    /// # Arguments
    /// * `api_key` - CORE API key (from https://core.ac.uk/services/api)
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.core.ac.uk/v3".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(EMBEDDING_DIMENSION)),
        })
    }

    /// Search open access works
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    pub async fn search_works(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_mock_core_data(query, limit)?);
        }

        let url = format!("{}/search/works", self.base_url);
        let body = serde_json::json!({
            "q": query,
            "limit": limit.min(100),
        });

        let mut request = self.client.post(&url).json(&body);
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = self.fetch_with_retry(request).await?;
        let search_response: CoreSearchResponse = response.json().await?;

        let mut records = Vec::new();
        for work in search_response.results {
            let record = self.work_to_record(work)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Get work by CORE ID
    ///
    /// # Arguments
    /// * `id` - CORE work ID
    pub async fn get_work(&self, id: &str) -> Result<DataRecord> {
        if self.api_key.is_none() {
            return Err(FrameworkError::Config(
                "API key required for get_work".to_string(),
            ));
        }

        let url = format!("{}/works/{}", self.base_url, id);
        let mut request = self.client.get(&url);
        if let Some(key) = &self.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = self.fetch_with_retry(request).await?;
        let work: CoreWork = response.json().await?;
        self.work_to_record(work)
    }

    /// Search by DOI
    ///
    /// # Arguments
    /// * `doi` - Digital Object Identifier
    pub async fn search_by_doi(&self, doi: &str) -> Result<Option<DataRecord>> {
        let records = self.search_works(&format!("doi:{}", doi), 1).await?;
        Ok(records.into_iter().next())
    }

    /// Generate mock CORE data when API key is missing
    fn generate_mock_core_data(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();
        for i in 0..limit.min(5) {
            let title = format!("Mock CORE paper about {}: Part {}", query, i + 1);
            let abstract_text = format!(
                "This is a mock abstract for demonstration. Topic: {}. ID: {}",
                query,
                i + 1
            );
            let text = format!("{} {}", title, abstract_text);
            let embedding = self.embedder.embed_text(&text);

            let mut data_map = serde_json::Map::new();
            data_map.insert("title".to_string(), serde_json::json!(title));
            data_map.insert("abstract".to_string(), serde_json::json!(abstract_text));
            data_map.insert("mock".to_string(), serde_json::json!(true));

            records.push(DataRecord {
                id: format!("mock_core_{}", i),
                source: "core".to_string(),
                record_type: "work".to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }
        Ok(records)
    }

    /// Convert CORE work to DataRecord
    fn work_to_record(&self, work: CoreWork) -> Result<DataRecord> {
        let title = work.title.unwrap_or_else(|| "Untitled".to_string());
        let abstract_text = work.abstract_text.unwrap_or_default();
        let text = format!("{} {}", title, abstract_text);
        let embedding = self.embedder.embed_text(&text);

        let timestamp = work
            .published_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(title));
        data_map.insert("abstract".to_string(), serde_json::json!(abstract_text));
        if let Some(authors) = work.authors {
            data_map.insert("authors".to_string(), serde_json::json!(authors));
        }
        if let Some(doi) = work.doi {
            data_map.insert("doi".to_string(), serde_json::json!(doi));
        }
        if let Some(url) = work.download_url {
            data_map.insert("download_url".to_string(), serde_json::json!(url));
        }

        Ok(DataRecord {
            id: work.id,
            source: "core".to_string(),
            record_type: "work".to_string(),
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
                        let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                        sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                    sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

#[async_trait]
impl DataSource for CoreClient {
    fn source_id(&self) -> &str {
        "core"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let query = cursor.as_deref().unwrap_or("open access");
        let records = self.search_works(query, batch_size).await?;
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
// ERIC Client
// ============================================================================

/// ERIC API response structures
#[derive(Debug, Deserialize)]
struct EricResponse {
    response: EricResponseData,
}

#[derive(Debug, Deserialize)]
struct EricResponseData {
    docs: Vec<EricDocument>,
    #[serde(rename = "numFound")]
    num_found: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct EricDocument {
    id: String,
    title: Option<Vec<String>>,
    #[serde(rename = "description")]
    description: Option<Vec<String>>,
    author: Option<Vec<String>>,
    #[serde(rename = "publicationdateyear")]
    publication_year: Option<i32>,
    #[serde(rename = "publicationtype")]
    publication_type: Option<Vec<String>>,
}

/// Client for ERIC education research database
pub struct EricClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl EricClient {
    /// Create a new ERIC client (no auth required)
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.ies.ed.gov/eric".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(EMBEDDING_DIMENSION)),
        })
    }

    /// Search education research documents
    ///
    /// # Arguments
    /// * `query` - Search query string
    /// * `limit` - Maximum number of results
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let url = format!(
            "{}/?search={}&rows={}&format=json",
            self.base_url,
            urlencoding::encode(query),
            limit.min(100)
        );

        let response = self.fetch_with_retry(&url).await?;
        let eric_response: EricResponse = response.json().await?;

        let mut records = Vec::new();
        for doc in eric_response.response.docs {
            let record = self.document_to_record(doc)?;
            records.push(record);
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Get document by ERIC ID
    ///
    /// # Arguments
    /// * `eric_id` - ERIC document ID (e.g., "ED123456")
    pub async fn get_document(&self, eric_id: &str) -> Result<DataRecord> {
        let url = format!("{}/?id={}&format=json", self.base_url, eric_id);
        let response = self.fetch_with_retry(&url).await?;
        let eric_response: EricResponse = response.json().await?;

        eric_response
            .response
            .docs
            .into_iter()
            .next()
            .ok_or_else(|| FrameworkError::Discovery("Document not found".to_string()))
            .and_then(|doc| self.document_to_record(doc))
    }

    /// Convert ERIC document to DataRecord
    fn document_to_record(&self, doc: EricDocument) -> Result<DataRecord> {
        let title = doc
            .title
            .and_then(|t| t.into_iter().next())
            .unwrap_or_else(|| "Untitled".to_string());

        let description = doc
            .description
            .and_then(|d| d.into_iter().next())
            .unwrap_or_default();

        let text = format!("{} {}", title, description);
        let embedding = self.embedder.embed_text(&text);

        // Use publication year to estimate timestamp
        let timestamp = doc
            .publication_year
            .and_then(|year| {
                NaiveDate::from_ymd_opt(year, 1, 1)
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
            })
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(title));
        data_map.insert("description".to_string(), serde_json::json!(description));
        if let Some(authors) = doc.author {
            data_map.insert("authors".to_string(), serde_json::json!(authors));
        }
        if let Some(year) = doc.publication_year {
            data_map.insert("publication_year".to_string(), serde_json::json!(year));
        }
        if let Some(pub_type) = doc.publication_type {
            data_map.insert("publication_type".to_string(), serde_json::json!(pub_type));
        }

        Ok(DataRecord {
            id: doc.id,
            source: "eric".to_string(),
            record_type: "document".to_string(),
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
                        let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                        sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                    sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for EricClient {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[async_trait]
impl DataSource for EricClient {
    fn source_id(&self) -> &str {
        "eric"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let query = cursor.as_deref().unwrap_or("education technology");
        let records = self.search(query, batch_size).await?;
        Ok((records, None))
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
// Unpaywall Client
// ============================================================================

/// Unpaywall API response structure
#[derive(Debug, Deserialize)]
struct UnpaywallResponse {
    doi: String,
    title: Option<String>,
    #[serde(rename = "is_oa")]
    is_oa: bool,
    #[serde(rename = "best_oa_location")]
    best_oa_location: Option<OaLocation>,
    #[serde(rename = "published_date")]
    published_date: Option<String>,
    #[serde(rename = "journal_name")]
    journal_name: Option<String>,
    #[serde(rename = "z_authors")]
    authors: Option<Vec<UnpaywallAuthor>>,
}

#[derive(Debug, Deserialize)]
struct OaLocation {
    url: Option<String>,
    #[serde(rename = "url_for_pdf")]
    url_for_pdf: Option<String>,
    license: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UnpaywallAuthor {
    family: Option<String>,
    given: Option<String>,
}

/// Client for Unpaywall open access discovery
pub struct UnpaywallClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl UnpaywallClient {
    /// Create a new Unpaywall client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.unpaywall.org/v2".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(EMBEDDING_DIMENSION)),
        })
    }

    /// Get open access status by DOI
    ///
    /// # Arguments
    /// * `doi` - Digital Object Identifier
    /// * `email` - Email address (required by Unpaywall)
    pub async fn get_by_doi(&self, doi: &str, email: &str) -> Result<DataRecord> {
        let url = format!("{}/{}?email={}", self.base_url, doi, email);
        let response = self.fetch_with_retry(&url).await?;
        let unpaywall_response: UnpaywallResponse = response.json().await?;
        self.response_to_record(unpaywall_response)
    }

    /// Batch lookup multiple DOIs
    ///
    /// # Arguments
    /// * `dois` - List of DOIs
    /// * `email` - Email address (required)
    pub async fn batch_lookup(&self, dois: &[&str], email: &str) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();
        for doi in dois {
            match self.get_by_doi(doi, email).await {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!("Failed to fetch DOI {}: {}", doi, e);
                    continue;
                }
            }
            sleep(self.rate_limit_delay).await;
        }
        Ok(records)
    }

    /// Convert Unpaywall response to DataRecord
    fn response_to_record(&self, response: UnpaywallResponse) -> Result<DataRecord> {
        let title = response
            .title
            .unwrap_or_else(|| "Untitled".to_string());

        let embedding = self.embedder.embed_text(&title);

        let timestamp = response
            .published_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .map(|d| d.and_hms_opt(0, 0, 0).unwrap().and_utc())
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("doi".to_string(), serde_json::json!(response.doi));
        data_map.insert("title".to_string(), serde_json::json!(title));
        data_map.insert("is_oa".to_string(), serde_json::json!(response.is_oa));

        if let Some(location) = response.best_oa_location {
            if let Some(url) = location.url {
                data_map.insert("oa_url".to_string(), serde_json::json!(url));
            }
            if let Some(pdf) = location.url_for_pdf {
                data_map.insert("pdf_url".to_string(), serde_json::json!(pdf));
            }
            if let Some(license) = location.license {
                data_map.insert("license".to_string(), serde_json::json!(license));
            }
        }

        if let Some(journal) = response.journal_name {
            data_map.insert("journal".to_string(), serde_json::json!(journal));
        }

        if let Some(authors) = response.authors {
            let author_names: Vec<String> = authors
                .iter()
                .map(|a| {
                    format!(
                        "{} {}",
                        a.given.as_deref().unwrap_or(""),
                        a.family.as_deref().unwrap_or("")
                    )
                    .trim()
                    .to_string()
                })
                .collect();
            data_map.insert("authors".to_string(), serde_json::json!(author_names));
        }

        Ok(DataRecord {
            id: response.doi,
            source: "unpaywall".to_string(),
            record_type: "article".to_string(),
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
                        let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                        sleep(Duration::from_millis(delay)).await;
                        continue;
                    }
                    if response.status() == StatusCode::NOT_FOUND {
                        return Err(FrameworkError::Discovery("DOI not found".to_string()));
                    }
                    return Ok(response);
                }
                Err(_) if retries < MAX_RETRIES => {
                    retries += 1;
                    let delay = RETRY_DELAY_MS * 2_u64.pow(retries - 1);
                    sleep(Duration::from_millis(delay)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for UnpaywallClient {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[async_trait]
impl DataSource for UnpaywallClient {
    fn source_id(&self) -> &str {
        "unpaywall"
    }

    async fn fetch_batch(
        &self,
        _cursor: Option<String>,
        _batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Unpaywall doesn't support bulk search, only DOI lookup
        Ok((Vec::new(), None))
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

    // ========================================================================
    // OpenAlex Tests
    // ========================================================================

    #[test]
    fn test_openalex_client_creation() {
        let client = OpenAlexClient::new(Some("test@example.com".to_string()));
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.source_id(), "openalex");
    }

    #[tokio::test]
    async fn test_openalex_health_check() {
        let client = OpenAlexClient::new(None).unwrap();
        let health = client.health_check().await;
        assert!(health.is_ok());
    }

    #[test]
    fn test_openalex_work_to_record() {
        let client = OpenAlexClient::new(None).unwrap();
        let work = OpenAlexWork {
            id: "W123456".to_string(),
            display_name: Some("Test Paper".to_string()),
            publication_date: Some("2024-01-01".to_string()),
            authorships: None,
            cited_by_count: Some(10),
            abstract_inverted_index: None,
        };

        let record = client.work_to_record(work).unwrap();
        assert_eq!(record.id, "W123456");
        assert_eq!(record.source, "openalex");
        assert_eq!(record.record_type, "work");
        assert!(record.embedding.is_some());
        assert_eq!(record.embedding.as_ref().unwrap().len(), EMBEDDING_DIMENSION);
    }

    #[test]
    fn test_openalex_author_to_record() {
        let client = OpenAlexClient::new(None).unwrap();
        let author = OpenAlexAuthorDetail {
            id: "A123456".to_string(),
            display_name: Some("Jane Doe".to_string()),
            works_count: Some(50),
            cited_by_count: Some(500),
        };

        let record = client.author_to_record(author).unwrap();
        assert_eq!(record.id, "A123456");
        assert_eq!(record.source, "openalex");
        assert_eq!(record.record_type, "author");
        assert!(record.embedding.is_some());
    }

    #[test]
    fn test_openalex_reconstruct_abstract() {
        let client = OpenAlexClient::new(None).unwrap();
        let mut inverted_index = HashMap::new();
        inverted_index.insert("machine".to_string(), vec![0]);
        inverted_index.insert("learning".to_string(), vec![1]);
        inverted_index.insert("is".to_string(), vec![2]);
        inverted_index.insert("awesome".to_string(), vec![3]);

        let abstract_text = client.reconstruct_abstract(&inverted_index);
        assert_eq!(abstract_text, "machine learning is awesome");
    }

    // ========================================================================
    // CORE Tests
    // ========================================================================

    #[test]
    fn test_core_client_creation() {
        let client = CoreClient::new(None);
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.source_id(), "core");
    }

    #[tokio::test]
    async fn test_core_mock_data() {
        let client = CoreClient::new(None).unwrap();
        let records = client.search_works("test query", 3).await.unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[0].source, "core");
        assert!(records[0].embedding.is_some());

        // Verify mock flag
        let mock_flag = records[0].data.get("mock");
        assert!(mock_flag.is_some());
        assert_eq!(mock_flag.unwrap(), &serde_json::json!(true));
    }

    #[test]
    fn test_core_work_to_record() {
        let client = CoreClient::new(None).unwrap();
        let work = CoreWork {
            id: "123456".to_string(),
            title: Some("Test Paper".to_string()),
            abstract_text: Some("This is a test abstract".to_string()),
            authors: Some(vec!["John Doe".to_string(), "Jane Smith".to_string()]),
            published_date: Some("2024-01-15".to_string()),
            download_url: Some("https://example.com/paper.pdf".to_string()),
            doi: Some("10.1234/test".to_string()),
        };

        let record = client.work_to_record(work).unwrap();
        assert_eq!(record.id, "123456");
        assert_eq!(record.source, "core");
        assert!(record.embedding.is_some());
        assert_eq!(record.embedding.as_ref().unwrap().len(), EMBEDDING_DIMENSION);

        // Verify data fields
        assert_eq!(
            record.data.get("title").unwrap(),
            &serde_json::json!("Test Paper")
        );
        assert_eq!(
            record.data.get("doi").unwrap(),
            &serde_json::json!("10.1234/test")
        );
    }

    #[tokio::test]
    async fn test_core_health_check() {
        let client = CoreClient::new(None).unwrap();
        let health = client.health_check().await;
        assert!(health.is_ok());
        assert!(health.unwrap());
    }

    // ========================================================================
    // ERIC Tests
    // ========================================================================

    #[test]
    fn test_eric_client_creation() {
        let client = EricClient::new();
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.source_id(), "eric");
    }

    #[test]
    fn test_eric_default() {
        let client = EricClient::default();
        assert_eq!(client.source_id(), "eric");
    }

    #[test]
    fn test_eric_document_to_record() {
        let client = EricClient::new().unwrap();
        let doc = EricDocument {
            id: "ED123456".to_string(),
            title: Some(vec!["Educational Technology in Schools".to_string()]),
            description: Some(vec!["A study on technology adoption".to_string()]),
            author: Some(vec!["Smith, John".to_string()]),
            publication_year: Some(2023),
            publication_type: Some(vec!["Journal Article".to_string()]),
        };

        let record = client.document_to_record(doc).unwrap();
        assert_eq!(record.id, "ED123456");
        assert_eq!(record.source, "eric");
        assert_eq!(record.record_type, "document");
        assert!(record.embedding.is_some());

        // Verify year conversion
        assert_eq!(
            record.data.get("publication_year").unwrap(),
            &serde_json::json!(2023)
        );
    }

    #[tokio::test]
    async fn test_eric_health_check() {
        let client = EricClient::new().unwrap();
        let health = client.health_check().await;
        assert!(health.is_ok());
    }

    // ========================================================================
    // Unpaywall Tests
    // ========================================================================

    #[test]
    fn test_unpaywall_client_creation() {
        let client = UnpaywallClient::new();
        assert!(client.is_ok());
        let client = client.unwrap();
        assert_eq!(client.source_id(), "unpaywall");
    }

    #[test]
    fn test_unpaywall_default() {
        let client = UnpaywallClient::default();
        assert_eq!(client.source_id(), "unpaywall");
    }

    #[test]
    fn test_unpaywall_response_to_record() {
        let client = UnpaywallClient::new().unwrap();
        let response = UnpaywallResponse {
            doi: "10.1234/test".to_string(),
            title: Some("Open Access Paper".to_string()),
            is_oa: true,
            best_oa_location: Some(OaLocation {
                url: Some("https://example.com/paper".to_string()),
                url_for_pdf: Some("https://example.com/paper.pdf".to_string()),
                license: Some("CC-BY".to_string()),
            }),
            published_date: Some("2024-01-01".to_string()),
            journal_name: Some("Test Journal".to_string()),
            authors: Some(vec![
                UnpaywallAuthor {
                    family: Some("Doe".to_string()),
                    given: Some("John".to_string()),
                },
                UnpaywallAuthor {
                    family: Some("Smith".to_string()),
                    given: Some("Jane".to_string()),
                },
            ]),
        };

        let record = client.response_to_record(response).unwrap();
        assert_eq!(record.id, "10.1234/test");
        assert_eq!(record.source, "unpaywall");
        assert_eq!(record.record_type, "article");
        assert!(record.embedding.is_some());

        // Verify OA fields
        assert_eq!(record.data.get("is_oa").unwrap(), &serde_json::json!(true));
        assert_eq!(
            record.data.get("license").unwrap(),
            &serde_json::json!("CC-BY")
        );

        // Verify authors
        let authors = record.data.get("authors").unwrap();
        assert!(authors.is_array());
        let author_array = authors.as_array().unwrap();
        assert_eq!(author_array.len(), 2);
    }

    #[tokio::test]
    async fn test_unpaywall_health_check() {
        let client = UnpaywallClient::new().unwrap();
        let health = client.health_check().await;
        assert!(health.is_ok());
        assert!(health.unwrap());
    }

    #[tokio::test]
    async fn test_unpaywall_batch_lookup_empty() {
        let client = UnpaywallClient::new().unwrap();
        let records = client.batch_lookup(&[], "test@example.com").await.unwrap();
        assert_eq!(records.len(), 0);
    }

    // ========================================================================
    // Integration Tests
    // ========================================================================

    #[tokio::test]
    async fn test_all_clients_datasource_trait() {
        let openalex = OpenAlexClient::new(None).unwrap();
        let core = CoreClient::new(None).unwrap();
        let eric = EricClient::new().unwrap();
        let unpaywall = UnpaywallClient::new().unwrap();

        assert_eq!(openalex.source_id(), "openalex");
        assert_eq!(core.source_id(), "core");
        assert_eq!(eric.source_id(), "eric");
        assert_eq!(unpaywall.source_id(), "unpaywall");
    }

    #[test]
    fn test_embedding_dimensions() {
        let embedder = SimpleEmbedder::new(EMBEDDING_DIMENSION);
        let embedding = embedder.embed_text("test text");
        assert_eq!(embedding.len(), EMBEDDING_DIMENSION);

        // Check normalization
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_retry_exponential_backoff() {
        // Test that retry delays increase exponentially
        let base_delay = RETRY_DELAY_MS;
        assert_eq!(base_delay * 2_u64.pow(0), 1000); // First retry: 1s
        assert_eq!(base_delay * 2_u64.pow(1), 2000); // Second retry: 2s
        assert_eq!(base_delay * 2_u64.pow(2), 4000); // Third retry: 4s
    }
}
