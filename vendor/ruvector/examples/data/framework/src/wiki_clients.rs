//! Wikipedia and Wikidata API clients for knowledge graph building
//!
//! This module provides async clients for:
//! - Wikipedia: Article content, categories, links, and search
//! - Wikidata: Entity lookup, SPARQL queries, and structured knowledge
//!
//! Both clients convert responses into RuVector's DataRecord format with
//! semantic embeddings for vector search and graph analysis.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::{DataRecord, DataSource, FrameworkError, Relationship, Result};
use crate::api_clients::SimpleEmbedder;

/// Rate limiting configuration
const DEFAULT_RATE_LIMIT_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// Wikipedia API Client
// ============================================================================

/// Wikipedia API search response
#[derive(Debug, Deserialize)]
struct WikiSearchResponse {
    query: WikiSearchQuery,
}

#[derive(Debug, Deserialize)]
struct WikiSearchQuery {
    search: Vec<WikiSearchResult>,
}

#[derive(Debug, Deserialize)]
struct WikiSearchResult {
    title: String,
    pageid: u64,
    snippet: String,
}

/// Wikipedia API page response
#[derive(Debug, Deserialize)]
struct WikiPageResponse {
    query: WikiPageQuery,
}

#[derive(Debug, Deserialize)]
struct WikiPageQuery {
    pages: HashMap<String, WikiPage>,
}

#[derive(Debug, Deserialize)]
struct WikiPage {
    pageid: u64,
    title: String,
    #[serde(default)]
    extract: String,
    #[serde(default)]
    categories: Vec<WikiCategory>,
    #[serde(default)]
    links: Vec<WikiLink>,
}

#[derive(Debug, Deserialize)]
struct WikiCategory {
    title: String,
}

#[derive(Debug, Deserialize)]
struct WikiLink {
    title: String,
}

/// Client for Wikipedia API
pub struct WikipediaClient {
    client: Client,
    base_url: String,
    language: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl WikipediaClient {
    /// Create a new Wikipedia client
    ///
    /// # Arguments
    /// * `language` - Wikipedia language code (e.g., "en", "de", "fr")
    pub fn new(language: String) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (https://github.com/ruvnet/ruvector)")
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        let base_url = format!("https://{}.wikipedia.org/w/api.php", language);

        Ok(Self {
            client,
            base_url,
            language,
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)), // Larger dimension for richer content
        })
    }

    /// Search Wikipedia articles
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `limit` - Maximum number of results (max 500)
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let url = format!(
            "{}?action=query&list=search&srsearch={}&srlimit={}&format=json",
            self.base_url,
            urlencoding::encode(query),
            limit.min(500)
        );

        let response = self.fetch_with_retry(&url).await?;
        let search_response: WikiSearchResponse = response.json().await?;

        let mut records = Vec::new();
        for result in search_response.query.search {
            // Get full article for each search result
            if let Ok(article) = self.get_article(&result.title).await {
                records.push(article);
                sleep(self.rate_limit_delay).await;
            }
        }

        Ok(records)
    }

    /// Get a Wikipedia article by title
    ///
    /// # Arguments
    /// * `title` - Article title
    pub async fn get_article(&self, title: &str) -> Result<DataRecord> {
        let url = format!(
            "{}?action=query&prop=extracts|categories|links&titles={}&exintro=1&explaintext=1&format=json&cllimit=50&pllimit=50",
            self.base_url,
            urlencoding::encode(title)
        );

        let response = self.fetch_with_retry(&url).await?;
        let page_response: WikiPageResponse = response.json().await?;

        // Extract the page (should be only one)
        let page = page_response
            .query
            .pages
            .values()
            .next()
            .ok_or_else(|| FrameworkError::Discovery("No page found".to_string()))?;

        self.page_to_record(page)
    }

    /// Get categories for an article
    ///
    /// # Arguments
    /// * `title` - Article title
    pub async fn get_categories(&self, title: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}?action=query&prop=categories&titles={}&cllimit=500&format=json",
            self.base_url,
            urlencoding::encode(title)
        );

        let response = self.fetch_with_retry(&url).await?;
        let page_response: WikiPageResponse = response.json().await?;

        let categories = page_response
            .query
            .pages
            .values()
            .next()
            .map(|page| page.categories.iter().map(|c| c.title.clone()).collect())
            .unwrap_or_default();

        Ok(categories)
    }

    /// Get links from an article
    ///
    /// # Arguments
    /// * `title` - Article title
    pub async fn get_links(&self, title: &str) -> Result<Vec<String>> {
        let url = format!(
            "{}?action=query&prop=links&titles={}&pllimit=500&format=json",
            self.base_url,
            urlencoding::encode(title)
        );

        let response = self.fetch_with_retry(&url).await?;
        let page_response: WikiPageResponse = response.json().await?;

        let links = page_response
            .query
            .pages
            .values()
            .next()
            .map(|page| page.links.iter().map(|l| l.title.clone()).collect())
            .unwrap_or_default();

        Ok(links)
    }

    /// Convert Wikipedia page to DataRecord
    fn page_to_record(&self, page: &WikiPage) -> Result<DataRecord> {
        // Create embedding from title and extract
        let text = format!("{} {}", page.title, page.extract);
        let embedding = self.embedder.embed_text(&text);

        // Build relationships from categories
        let mut relationships = Vec::new();
        for category in &page.categories {
            relationships.push(Relationship {
                target_id: category.title.clone(),
                rel_type: "in_category".to_string(),
                weight: 1.0,
                properties: HashMap::new(),
            });
        }

        // Build relationships from links (limit to first 20)
        for link in page.links.iter().take(20) {
            relationships.push(Relationship {
                target_id: link.title.clone(),
                rel_type: "links_to".to_string(),
                weight: 0.5,
                properties: HashMap::new(),
            });
        }

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(page.title));
        data_map.insert("extract".to_string(), serde_json::json!(page.extract));
        data_map.insert("pageid".to_string(), serde_json::json!(page.pageid));
        data_map.insert("language".to_string(), serde_json::json!(self.language));
        data_map.insert(
            "url".to_string(),
            serde_json::json!(format!(
                "https://{}.wikipedia.org/wiki/{}",
                self.language,
                urlencoding::encode(&page.title)
            )),
        );

        Ok(DataRecord {
            id: format!("wikipedia_{}_{}", self.language, page.pageid),
            source: "wikipedia".to_string(),
            record_type: "article".to_string(),
            timestamp: Utc::now(),
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
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
impl DataSource for WikipediaClient {
    fn source_id(&self) -> &str {
        "wikipedia"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Default to searching for "machine learning" if no cursor provided
        let query = cursor.as_deref().unwrap_or("machine learning");
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
// Wikidata API Client
// ============================================================================

/// Wikidata entity search response
#[derive(Debug, Deserialize)]
struct WikidataSearchResponse {
    search: Vec<WikidataSearchResult>,
}

#[derive(Debug, Deserialize)]
struct WikidataSearchResult {
    id: String,
    label: String,
    description: Option<String>,
}

/// Wikidata entity response
#[derive(Debug, Deserialize)]
struct WikidataEntityResponse {
    entities: HashMap<String, WikidataEntityData>,
}

#[derive(Debug, Deserialize)]
struct WikidataEntityData {
    id: String,
    labels: HashMap<String, WikidataLabel>,
    descriptions: HashMap<String, WikidataLabel>,
    aliases: HashMap<String, Vec<WikidataLabel>>,
    claims: HashMap<String, Vec<WikidataClaim>>,
}

#[derive(Debug, Deserialize)]
struct WikidataLabel {
    value: String,
}

#[derive(Debug, Deserialize)]
struct WikidataClaim {
    mainsnak: WikidataSnak,
}

#[derive(Debug, Deserialize)]
struct WikidataSnak {
    datavalue: Option<WikidataValue>,
}

#[derive(Debug, Deserialize)]
struct WikidataValue {
    value: serde_json::Value,
}

/// Wikidata SPARQL response
#[derive(Debug, Deserialize)]
struct WikidataSparqlResponse {
    results: WikidataSparqlResults,
}

#[derive(Debug, Deserialize)]
struct WikidataSparqlResults {
    bindings: Vec<HashMap<String, WikidataSparqlBinding>>,
}

#[derive(Debug, Deserialize)]
struct WikidataSparqlBinding {
    value: String,
}

/// Structured Wikidata entity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikidataEntity {
    /// Wikidata Q-identifier
    pub qid: String,
    /// Primary label
    pub label: String,
    /// Description
    pub description: String,
    /// Alternative names
    pub aliases: Vec<String>,
    /// Property claims (property ID -> values)
    pub claims: HashMap<String, Vec<String>>,
}

/// Client for Wikidata API and SPARQL endpoint
pub struct WikidataClient {
    client: Client,
    api_url: String,
    sparql_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl WikidataClient {
    /// Create a new Wikidata client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector/1.0 (https://github.com/ruvnet/ruvector)")
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            api_url: "https://www.wikidata.org/w/api.php".to_string(),
            sparql_url: "https://query.wikidata.org/sparql".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(256)),
        })
    }

    /// Search for Wikidata entities
    ///
    /// # Arguments
    /// * `query` - Search query
    pub async fn search_entities(&self, query: &str) -> Result<Vec<WikidataEntity>> {
        let url = format!(
            "{}?action=wbsearchentities&search={}&language=en&format=json&limit=50",
            self.api_url,
            urlencoding::encode(query)
        );

        let response = self.fetch_with_retry(&url).await?;
        let search_response: WikidataSearchResponse = response.json().await?;

        let mut entities = Vec::new();
        for result in search_response.search {
            entities.push(WikidataEntity {
                qid: result.id,
                label: result.label,
                description: result.description.unwrap_or_default(),
                aliases: Vec::new(),
                claims: HashMap::new(),
            });
        }

        Ok(entities)
    }

    /// Get a Wikidata entity by QID
    ///
    /// # Arguments
    /// * `qid` - Wikidata Q-identifier (e.g., "Q42" for Douglas Adams)
    pub async fn get_entity(&self, qid: &str) -> Result<WikidataEntity> {
        let url = format!(
            "{}?action=wbgetentities&ids={}&format=json",
            self.api_url, qid
        );

        let response = self.fetch_with_retry(&url).await?;
        let entity_response: WikidataEntityResponse = response.json().await?;

        let entity_data = entity_response
            .entities
            .get(qid)
            .ok_or_else(|| FrameworkError::Discovery(format!("Entity {} not found", qid)))?;

        self.entity_data_to_entity(entity_data)
    }

    /// Execute a SPARQL query
    ///
    /// # Arguments
    /// * `query` - SPARQL query string
    pub async fn sparql_query(&self, query: &str) -> Result<Vec<HashMap<String, String>>> {
        let response = self
            .client
            .get(&self.sparql_url)
            .query(&[("query", query), ("format", "json")])
            .send()
            .await?;

        let sparql_response: WikidataSparqlResponse = response.json().await?;

        let results = sparql_response
            .results
            .bindings
            .into_iter()
            .map(|binding| {
                binding
                    .into_iter()
                    .map(|(k, v)| (k, v.value))
                    .collect::<HashMap<String, String>>()
            })
            .collect();

        Ok(results)
    }

    /// Query climate change related entities
    pub async fn query_climate_entities(&self) -> Result<Vec<DataRecord>> {
        let query = r#"
SELECT ?item ?itemLabel ?itemDescription WHERE {
  {
    ?item wdt:P31 wd:Q125977.  # climate change
  } UNION {
    ?item wdt:P279* wd:Q125977.  # subclass of climate change
  } UNION {
    ?item wdt:P921 wd:Q125977.  # main subject climate change
  }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
LIMIT 100
"#;

        self.sparql_to_records(query, "climate").await
    }

    /// Query pharmaceutical companies
    pub async fn query_pharmaceutical_companies(&self) -> Result<Vec<DataRecord>> {
        let query = r#"
SELECT ?item ?itemLabel ?itemDescription ?founded ?employees WHERE {
  ?item wdt:P31/wdt:P279* wd:Q507443.  # pharmaceutical company
  OPTIONAL { ?item wdt:P571 ?founded. }
  OPTIONAL { ?item wdt:P1128 ?employees. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
LIMIT 100
"#;

        self.sparql_to_records(query, "pharma").await
    }

    /// Query disease outbreaks
    pub async fn query_disease_outbreaks(&self) -> Result<Vec<DataRecord>> {
        let query = r#"
SELECT ?item ?itemLabel ?itemDescription ?disease ?diseaseLabel ?startTime ?location ?locationLabel WHERE {
  ?item wdt:P31 wd:Q3241045.  # epidemic
  OPTIONAL { ?item wdt:P828 ?disease. }
  OPTIONAL { ?item wdt:P580 ?startTime. }
  OPTIONAL { ?item wdt:P276 ?location. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
LIMIT 100
"#;

        self.sparql_to_records(query, "disease").await
    }

    /// Convert SPARQL results to DataRecords
    async fn sparql_to_records(&self, query: &str, category: &str) -> Result<Vec<DataRecord>> {
        let results = self.sparql_query(query).await?;

        let mut records = Vec::new();
        for result in results {
            // Extract QID from URI
            let item_uri = result.get("item").cloned().unwrap_or_default();
            let qid = item_uri
                .split('/')
                .last()
                .unwrap_or(&item_uri)
                .to_string();

            let label = result
                .get("itemLabel")
                .cloned()
                .unwrap_or_else(|| qid.clone());
            let description = result.get("itemDescription").cloned().unwrap_or_default();

            // Create embedding from label and description
            let text = format!("{} {}", label, description);
            let embedding = self.embedder.embed_text(&text);

            let mut data_map = serde_json::Map::new();
            data_map.insert("qid".to_string(), serde_json::json!(qid));
            data_map.insert("label".to_string(), serde_json::json!(label));
            data_map.insert("description".to_string(), serde_json::json!(description));
            data_map.insert("category".to_string(), serde_json::json!(category));

            // Add all other SPARQL result fields
            for (key, value) in result.iter() {
                if !key.ends_with("Label") && key != "item" && key != "itemDescription" {
                    data_map.insert(key.clone(), serde_json::json!(value));
                }
            }

            records.push(DataRecord {
                id: format!("wikidata_{}", qid),
                source: "wikidata".to_string(),
                record_type: category.to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }

        Ok(records)
    }

    /// Convert entity data to WikidataEntity
    fn entity_data_to_entity(&self, data: &WikidataEntityData) -> Result<WikidataEntity> {
        let label = data
            .labels
            .get("en")
            .map(|l| l.value.clone())
            .unwrap_or_else(|| data.id.clone());

        let description = data
            .descriptions
            .get("en")
            .map(|d| d.value.clone())
            .unwrap_or_default();

        let aliases = data
            .aliases
            .get("en")
            .map(|aliases| aliases.iter().map(|a| a.value.clone()).collect())
            .unwrap_or_default();

        let mut claims = HashMap::new();
        for (property, claim_list) in &data.claims {
            let values: Vec<String> = claim_list
                .iter()
                .filter_map(|claim| {
                    claim
                        .mainsnak
                        .datavalue
                        .as_ref()
                        .map(|dv| dv.value.to_string())
                })
                .collect();

            if !values.is_empty() {
                claims.insert(property.clone(), values);
            }
        }

        Ok(WikidataEntity {
            qid: data.id.clone(),
            label,
            description,
            aliases,
            claims,
        })
    }

    /// Convert WikidataEntity to DataRecord
    fn entity_to_record(&self, entity: &WikidataEntity) -> Result<DataRecord> {
        // Create embedding from label, description, and aliases
        let text = format!(
            "{} {} {}",
            entity.label,
            entity.description,
            entity.aliases.join(" ")
        );
        let embedding = self.embedder.embed_text(&text);

        // Build relationships from claims
        let mut relationships = Vec::new();
        for (property, values) in &entity.claims {
            for value in values {
                // Try to extract QID if value is an entity reference
                if let Some(qid) = value.strip_prefix("Q") {
                    if qid.chars().all(|c| c.is_ascii_digit()) {
                        relationships.push(Relationship {
                            target_id: value.clone(),
                            rel_type: property.clone(),
                            weight: 1.0,
                            properties: HashMap::new(),
                        });
                    }
                }
            }
        }

        let mut data_map = serde_json::Map::new();
        data_map.insert("qid".to_string(), serde_json::json!(entity.qid));
        data_map.insert("label".to_string(), serde_json::json!(entity.label));
        data_map.insert(
            "description".to_string(),
            serde_json::json!(entity.description),
        );
        data_map.insert("aliases".to_string(), serde_json::json!(entity.aliases));
        data_map.insert(
            "url".to_string(),
            serde_json::json!(format!(
                "https://www.wikidata.org/wiki/{}",
                entity.qid
            )),
        );

        // Add claims as structured data
        let claims_json: serde_json::Value = serde_json::to_value(&entity.claims)?;
        data_map.insert("claims".to_string(), claims_json);

        Ok(DataRecord {
            id: format!("wikidata_{}", entity.qid),
            source: "wikidata".to_string(),
            record_type: "entity".to_string(),
            timestamp: Utc::now(),
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
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

impl Default for WikidataClient {
    fn default() -> Self {
        Self::new().expect("Failed to create WikidataClient")
    }
}

#[async_trait]
impl DataSource for WikidataClient {
    fn source_id(&self) -> &str {
        "wikidata"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        _batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Use cursor to determine which query to run
        let records = match cursor.as_deref() {
            Some("climate") => self.query_climate_entities().await?,
            Some("pharma") => self.query_pharmaceutical_companies().await?,
            Some("disease") => self.query_disease_outbreaks().await?,
            _ => {
                // Default: search for "artificial intelligence"
                let entities = self.search_entities("artificial intelligence").await?;
                let mut records = Vec::new();
                for entity in entities.iter().take(20) {
                    records.push(self.entity_to_record(entity)?);
                }
                records
            }
        };

        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self.client.get(&self.api_url).send().await?;
        Ok(response.status().is_success())
    }
}

// ============================================================================
// Example SPARQL Queries
// ============================================================================

/// Pre-defined SPARQL query templates
pub mod sparql_queries {
    /// Query for climate change related entities
    pub const CLIMATE_CHANGE: &str = r#"
SELECT ?item ?itemLabel ?itemDescription WHERE {
  {
    ?item wdt:P31 wd:Q125977.  # instance of climate change
  } UNION {
    ?item wdt:P279* wd:Q125977.  # subclass of climate change
  } UNION {
    ?item wdt:P921 wd:Q125977.  # main subject climate change
  }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
LIMIT 100
"#;

    /// Query for pharmaceutical companies
    pub const PHARMACEUTICAL_COMPANIES: &str = r#"
SELECT ?item ?itemLabel ?itemDescription ?founded ?employees ?headquarters ?headquartersLabel WHERE {
  ?item wdt:P31/wdt:P279* wd:Q507443.  # pharmaceutical company
  OPTIONAL { ?item wdt:P571 ?founded. }
  OPTIONAL { ?item wdt:P1128 ?employees. }
  OPTIONAL { ?item wdt:P159 ?headquarters. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY DESC(?employees)
LIMIT 100
"#;

    /// Query for disease outbreaks
    pub const DISEASE_OUTBREAKS: &str = r#"
SELECT ?item ?itemLabel ?itemDescription ?disease ?diseaseLabel ?startTime ?endTime ?location ?locationLabel ?deaths WHERE {
  ?item wdt:P31 wd:Q3241045.  # epidemic
  OPTIONAL { ?item wdt:P828 ?disease. }
  OPTIONAL { ?item wdt:P580 ?startTime. }
  OPTIONAL { ?item wdt:P582 ?endTime. }
  OPTIONAL { ?item wdt:P276 ?location. }
  OPTIONAL { ?item wdt:P1120 ?deaths. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY DESC(?startTime)
LIMIT 100
"#;

    /// Query for scientific research institutions
    pub const RESEARCH_INSTITUTIONS: &str = r#"
SELECT ?item ?itemLabel ?itemDescription ?country ?countryLabel ?founded WHERE {
  ?item wdt:P31/wdt:P279* wd:Q31855.  # research institute
  OPTIONAL { ?item wdt:P17 ?country. }
  OPTIONAL { ?item wdt:P571 ?founded. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
LIMIT 100
"#;

    /// Query for Nobel Prize winners in specific field
    pub const NOBEL_LAUREATES: &str = r#"
SELECT ?item ?itemLabel ?itemDescription ?award ?awardLabel ?year ?field ?fieldLabel WHERE {
  ?item wdt:P166 ?award.
  ?award wdt:P279* wd:Q7191.  # Nobel Prize
  OPTIONAL { ?item wdt:P166 ?award. ?award wdt:P585 ?year. }
  OPTIONAL { ?award wdt:P101 ?field. }
  SERVICE wikibase:label { bd:serviceParam wikibase:language "en". }
}
ORDER BY DESC(?year)
LIMIT 100
"#;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wikipedia_client_creation() {
        let client = WikipediaClient::new("en".to_string());
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_wikidata_client_creation() {
        let client = WikidataClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_wikidata_entity_serialization() {
        let mut claims = HashMap::new();
        claims.insert("P31".to_string(), vec!["Q5".to_string()]);

        let entity = WikidataEntity {
            qid: "Q42".to_string(),
            label: "Douglas Adams".to_string(),
            description: "English writer and humorist".to_string(),
            aliases: vec!["Douglas Noel Adams".to_string()],
            claims,
        };

        let json = serde_json::to_string(&entity).unwrap();
        let parsed: WikidataEntity = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.qid, "Q42");
        assert_eq!(parsed.label, "Douglas Adams");
    }

    #[test]
    fn test_sparql_query_templates() {
        assert!(!sparql_queries::CLIMATE_CHANGE.is_empty());
        assert!(!sparql_queries::PHARMACEUTICAL_COMPANIES.is_empty());
        assert!(!sparql_queries::DISEASE_OUTBREAKS.is_empty());
    }
}
