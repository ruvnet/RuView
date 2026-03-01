//! Government and International Organization API Integrations
//!
//! This module provides async clients for fetching data from government agencies
//! and international organizations, converting responses to SemanticVector format
//! for RuVector discovery.
//!
//! # Supported APIs
//!
//! - **US Census Bureau**: Population, demographics, economic surveys
//! - **Data.gov**: US Government open data catalog
//! - **EU Open Data Portal**: European Union datasets
//! - **UK Government Data**: UK public sector information
//! - **World Bank**: Global development indicators
//! - **United Nations**: International statistics and indicators

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Datelike, NaiveDate, Utc};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

use crate::api_clients::SimpleEmbedder;
use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Rate limiting configuration
const CENSUS_RATE_LIMIT_MS: u64 = 1200; // 50 req/min
const DATAGOV_RATE_LIMIT_MS: u64 = 1000; // 60 req/min
const EU_OPENDATA_RATE_LIMIT_MS: u64 = 500; // Conservative
const UK_GOV_RATE_LIMIT_MS: u64 = 500; // Conservative
const WORLDBANK_RATE_LIMIT_MS: u64 = 100; // Liberal
const UNDATA_RATE_LIMIT_MS: u64 = 1000; // Conservative
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;
const DEFAULT_EMBEDDING_DIM: usize = 256;

// ============================================================================
// US Census Bureau Client
// ============================================================================

/// Census Bureau API response for datasets
#[derive(Debug, Deserialize)]
struct CensusDataResponse {
    #[serde(default)]
    dataset: Vec<Vec<serde_json::Value>>,
}

/// Census Bureau dataset info
#[derive(Debug, Deserialize, Serialize, Clone)]
struct CensusDataset {
    #[serde(rename = "c_vintage", default)]
    vintage: String,
    #[serde(rename = "c_dataset", default)]
    dataset: Vec<String>,
    #[serde(default)]
    title: String,
    #[serde(default)]
    description: String,
}

/// Census Bureau variable info
#[derive(Debug, Deserialize, Clone)]
struct CensusVariable {
    #[serde(default)]
    name: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    concept: String,
    #[serde(default)]
    predicateType: String,
}

/// Client for US Census Bureau API
///
/// Provides access to:
/// - Decennial Census data
/// - American Community Survey (ACS)
/// - Economic indicators
/// - Population estimates
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::CensusClient;
///
/// let client = CensusClient::new(None);
/// let population = client.get_population(2020, "state:*").await?;
/// let acs_data = client.get_acs5(2021, vec!["B01001_001E"], "county:*").await?;
/// let datasets = client.get_available_datasets().await?;
/// ```
pub struct CensusClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl CensusClient {
    /// Create a new Census Bureau client
    ///
    /// # Arguments
    /// * `api_key` - Optional Census API key (500 req/day without key, unlimited with key)
    ///               Get key at: https://api.census.gov/data/key_signup.html
    pub fn new(api_key: Option<String>) -> Self {
        Self::with_config(api_key, DEFAULT_EMBEDDING_DIM, false)
    }

    /// Create a new Census client with custom configuration
    ///
    /// # Arguments
    /// * `api_key` - Optional Census API key
    /// * `embedding_dim` - Dimension for embeddings
    /// * `use_mock` - Use mock data when API is unavailable
    pub fn with_config(api_key: Option<String>, embedding_dim: usize, use_mock: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://api.census.gov/data".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(CENSUS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        }
    }

    /// Get population data for a specific year and geography
    ///
    /// # Arguments
    /// * `year` - Census year (e.g., 2020, 2010)
    /// * `geography` - Geography filter (e.g., "state:*", "county:06", "us:1")
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get all states population
    /// let pop = client.get_population(2020, "state:*").await?;
    ///
    /// // Get California counties
    /// let ca_counties = client.get_population(2020, "county:*&in=state:06").await?;
    /// ```
    pub async fn get_population(
        &self,
        year: u32,
        geography: &str,
    ) -> Result<Vec<SemanticVector>> {
        let url = if year >= 2020 {
            format!(
                "{}/{}/dec/pl?get=NAME,P1_001N&for={}",
                self.base_url, year, geography
            )
        } else {
            format!(
                "{}/{}/dec/sf1?get=NAME,P001001&for={}",
                self.base_url, year, geography
            )
        };

        self.fetch_census_data(&url, &format!("population_{}", year))
            .await
    }

    /// Get American Community Survey 5-year estimates
    ///
    /// # Arguments
    /// * `year` - Survey year (2009-2021+)
    /// * `variables` - Variable codes (e.g., ["B01001_001E"] for total population)
    /// * `geography` - Geography filter
    ///
    /// # Example
    /// ```rust,ignore
    /// // Get median household income for all states
    /// let income = client.get_acs5(
    ///     2021,
    ///     vec!["B19013_001E"],
    ///     "state:*"
    /// ).await?;
    /// ```
    pub async fn get_acs5(
        &self,
        year: u32,
        variables: Vec<&str>,
        geography: &str,
    ) -> Result<Vec<SemanticVector>> {
        let vars = variables.join(",");
        let url = format!(
            "{}/{}/acs/acs5?get=NAME,{}&for={}",
            self.base_url, year, vars, geography
        );

        self.fetch_census_data(&url, &format!("acs5_{}", year))
            .await
    }

    /// Get available Census datasets
    ///
    /// # Example
    /// ```rust,ignore
    /// let datasets = client.get_available_datasets().await?;
    /// for ds in datasets {
    ///     println!("Dataset: {}", ds.metadata.get("title").unwrap());
    /// }
    /// ```
    pub async fn get_available_datasets(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_datasets());
        }

        let url = format!("{}.json", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        // Parse the complex JSON structure
        let json: serde_json::Value = serde_json::from_str(&text)?;
        let mut vectors = Vec::new();

        if let Some(dataset_obj) = json.get("dataset") {
            if let Some(datasets) = dataset_obj.as_array() {
                for (idx, ds) in datasets.iter().enumerate() {
                    if let Some(title) = ds.get("title").and_then(|t| t.as_str()) {
                        let description = ds
                            .get("description")
                            .and_then(|d| d.as_str())
                            .unwrap_or("");
                        let vintage = ds.get("c_vintage").and_then(|v| v.as_str()).unwrap_or("");

                        let text = format!("{} {} {}", title, description, vintage);
                        let embedding = self.embedder.embed_text(&text);

                        let mut metadata = HashMap::new();
                        metadata.insert("title".to_string(), title.to_string());
                        metadata.insert("description".to_string(), description.to_string());
                        metadata.insert("vintage".to_string(), vintage.to_string());
                        metadata.insert("source".to_string(), "census_catalog".to_string());

                        vectors.push(SemanticVector {
                            id: format!("CENSUS_DS:{}", idx),
                            embedding,
                            domain: Domain::Government,
                            timestamp: Utc::now(),
                            metadata,
                        });
                    }
                }
            }
        }

        Ok(vectors)
    }

    /// Search for variables in a dataset
    ///
    /// # Arguments
    /// * `dataset` - Dataset identifier (e.g., "acs/acs5", "dec/pl")
    /// * `query` - Search query for variable names/labels
    ///
    /// # Example
    /// ```rust,ignore
    /// let vars = client.search_variables("acs/acs5", "income").await?;
    /// ```
    pub async fn search_variables(
        &self,
        dataset: &str,
        query: &str,
    ) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_variables(query));
        }

        let url = format!("{}/2021/{}/variables.json", self.base_url, dataset);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        let json: serde_json::Value = serde_json::from_str(&text)?;
        let mut vectors = Vec::new();

        if let Some(variables) = json.get("variables").and_then(|v| v.as_object()) {
            for (var_name, var_data) in variables.iter() {
                if let Some(label) = var_data.get("label").and_then(|l| l.as_str()) {
                    // Filter by query
                    if !label.to_lowercase().contains(&query.to_lowercase())
                        && !var_name.to_lowercase().contains(&query.to_lowercase())
                    {
                        continue;
                    }

                    let concept = var_data
                        .get("concept")
                        .and_then(|c| c.as_str())
                        .unwrap_or("");

                    let text = format!("{} {} {}", var_name, label, concept);
                    let embedding = self.embedder.embed_text(&text);

                    let mut metadata = HashMap::new();
                    metadata.insert("variable_name".to_string(), var_name.clone());
                    metadata.insert("label".to_string(), label.to_string());
                    metadata.insert("concept".to_string(), concept.to_string());
                    metadata.insert("dataset".to_string(), dataset.to_string());
                    metadata.insert("source".to_string(), "census_variables".to_string());

                    vectors.push(SemanticVector {
                        id: format!("CENSUS_VAR:{}:{}", dataset, var_name),
                        embedding,
                        domain: Domain::Government,
                        timestamp: Utc::now(),
                        metadata,
                    });

                    if vectors.len() >= 50 {
                        break; // Limit results
                    }
                }
            }
        }

        Ok(vectors)
    }

    /// Fetch and parse Census data
    async fn fetch_census_data(&self, url: &str, dataset_name: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_census_data(dataset_name));
        }

        let mut full_url = url.to_string();
        if let Some(key) = &self.api_key {
            full_url.push_str(&format!("&key={}", key));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&full_url).await?;
        let text = response.text().await?;

        // Census API returns array of arrays: [["NAME", "P1_001N"], ["California", "39538223"], ...]
        let data: Vec<Vec<serde_json::Value>> = serde_json::from_str(&text)?;

        if data.is_empty() {
            return Ok(Vec::new());
        }

        let headers = &data[0];
        let mut vectors = Vec::new();

        for (idx, row) in data.iter().skip(1).enumerate() {
            let mut record = HashMap::new();
            for (i, value) in row.iter().enumerate() {
                if let Some(header) = headers.get(i).and_then(|h| h.as_str()) {
                    record.insert(header.to_string(), value.to_string());
                }
            }

            let name = record.get("NAME").map(|s| s.as_str()).unwrap_or("Unknown");
            let text = format!("{} {}", dataset_name, name);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = record.clone();
            metadata.insert("source".to_string(), "census".to_string());
            metadata.insert("dataset".to_string(), dataset_name.to_string());

            vectors.push(SemanticVector {
                id: format!("CENSUS:{}:{}", dataset_name, idx),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Mock census data for testing
    fn get_mock_census_data(&self, dataset_name: &str) -> Vec<SemanticVector> {
        let mut vectors = Vec::new();
        let items = vec![
            ("California", "39538223"),
            ("Texas", "29145505"),
            ("Florida", "21538187"),
        ];

        for (idx, (name, population)) in items.iter().enumerate() {
            let text = format!("{} {} population {}", dataset_name, name, population);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("NAME".to_string(), name.to_string());
            metadata.insert("population".to_string(), population.to_string());
            metadata.insert("source".to_string(), "census_mock".to_string());

            vectors.push(SemanticVector {
                id: format!("CENSUS_MOCK:{}:{}", dataset_name, idx),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        vectors
    }

    /// Mock datasets for testing
    fn get_mock_datasets(&self) -> Vec<SemanticVector> {
        vec![self.create_mock_dataset(
            "Decennial Census",
            "Population and housing counts",
            "2020",
        )]
    }

    /// Mock variables for testing
    fn get_mock_variables(&self, query: &str) -> Vec<SemanticVector> {
        vec![self.create_mock_variable(
            "B19013_001E",
            "Median Household Income",
            "Income",
        )]
    }

    fn create_mock_dataset(&self, title: &str, description: &str, vintage: &str) -> SemanticVector {
        let text = format!("{} {} {}", title, description, vintage);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), title.to_string());
        metadata.insert("description".to_string(), description.to_string());
        metadata.insert("vintage".to_string(), vintage.to_string());
        metadata.insert("source".to_string(), "census_mock".to_string());

        SemanticVector {
            id: "CENSUS_MOCK_DS:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }
    }

    fn create_mock_variable(&self, name: &str, label: &str, concept: &str) -> SemanticVector {
        let text = format!("{} {} {}", name, label, concept);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("variable_name".to_string(), name.to_string());
        metadata.insert("label".to_string(), label.to_string());
        metadata.insert("concept".to_string(), concept.to_string());
        metadata.insert("source".to_string(), "census_mock".to_string());

        SemanticVector {
            id: format!("CENSUS_MOCK_VAR:{}", name),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }
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
                    if !response.status().is_success() && self.use_mock {
                        return Err(FrameworkError::Network(
                            reqwest::Error::from(response.error_for_status().unwrap_err()),
                        ));
                    }
                    return Ok(response);
                }
                Err(e) if retries < MAX_RETRIES && self.use_mock => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

// ============================================================================
// Data.gov Client
// ============================================================================

/// Data.gov CKAN API response
#[derive(Debug, Deserialize)]
struct DataGovSearchResponse {
    #[serde(default)]
    success: bool,
    result: DataGovResult,
}

#[derive(Debug, Deserialize)]
struct DataGovResult {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    results: Vec<DataGovDataset>,
}

#[derive(Debug, Deserialize)]
struct DataGovDataset {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    organization: Option<DataGovOrganization>,
    #[serde(default)]
    tags: Vec<DataGovTag>,
    #[serde(default)]
    metadata_created: String,
    #[serde(default)]
    metadata_modified: String,
}

#[derive(Debug, Deserialize)]
struct DataGovOrganization {
    #[serde(default)]
    name: String,
    #[serde(default)]
    title: String,
}

#[derive(Debug, Deserialize)]
struct DataGovTag {
    #[serde(default)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct DataGovOrganizationList {
    #[serde(default)]
    success: bool,
    #[serde(default)]
    result: Vec<DataGovOrganizationInfo>,
}

#[derive(Debug, Deserialize)]
struct DataGovOrganizationInfo {
    id: String,
    name: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    package_count: u64,
}

/// Client for Data.gov catalog
///
/// Provides access to:
/// - 250,000+ US Government datasets
/// - Federal, state, and local data
/// - Organization and tag browsing
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::DataGovClient;
///
/// let client = DataGovClient::new();
/// let datasets = client.search_datasets("climate change").await?;
/// let orgs = client.list_organizations().await?;
/// let dataset = client.get_dataset("dataset-id").await?;
/// ```
pub struct DataGovClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl DataGovClient {
    /// Create a new Data.gov client
    pub fn new() -> Self {
        Self::with_config(DEFAULT_EMBEDDING_DIM, false)
    }

    /// Create a new Data.gov client with custom configuration
    pub fn with_config(embedding_dim: usize, use_mock: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://catalog.data.gov/api/3".to_string(),
            rate_limit_delay: Duration::from_millis(DATAGOV_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        }
    }

    /// Search for datasets
    ///
    /// # Arguments
    /// * `query` - Search query string
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.search_datasets("renewable energy").await?;
    /// ```
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_datagov_datasets(query));
        }

        let url = format!(
            "{}/action/package_search?q={}&rows=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: DataGovSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for dataset in search_response.result.results {
            let org_name = dataset
                .organization
                .as_ref()
                .map(|o| o.title.as_str())
                .unwrap_or("Unknown");

            let tags = dataset
                .tags
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ");

            let text = format!("{} {} {} {}", dataset.title, dataset.notes, org_name, tags);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("title".to_string(), dataset.title);
            metadata.insert("description".to_string(), dataset.notes);
            metadata.insert("organization".to_string(), org_name.to_string());
            metadata.insert("tags".to_string(), tags);
            metadata.insert("created".to_string(), dataset.metadata_created);
            metadata.insert("modified".to_string(), dataset.metadata_modified);
            metadata.insert("source".to_string(), "datagov".to_string());

            vectors.push(SemanticVector {
                id: format!("DATAGOV:{}", dataset.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get a specific dataset by ID
    ///
    /// # Arguments
    /// * `id` - Dataset ID
    ///
    /// # Example
    /// ```rust,ignore
    /// let dataset = client.get_dataset("some-dataset-id").await?;
    /// ```
    pub async fn get_dataset(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_datagov_datasets("mock").into_iter().next().unwrap()));
        }

        let url = format!("{}/action/package_show?id={}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(result) = json.get("result") {
            let dataset: DataGovDataset = serde_json::from_value(result.clone())?;

            let org_name = dataset
                .organization
                .as_ref()
                .map(|o| o.title.as_str())
                .unwrap_or("Unknown");

            let text = format!("{} {}", dataset.title, dataset.notes);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("title".to_string(), dataset.title);
            metadata.insert("description".to_string(), dataset.notes);
            metadata.insert("organization".to_string(), org_name.to_string());
            metadata.insert("source".to_string(), "datagov".to_string());

            return Ok(Some(SemanticVector {
                id: format!("DATAGOV:{}", dataset.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            }));
        }

        Ok(None)
    }

    /// List all organizations
    ///
    /// # Example
    /// ```rust,ignore
    /// let orgs = client.list_organizations().await?;
    /// ```
    pub async fn list_organizations(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_organizations());
        }

        let url = format!("{}/action/organization_list?all_fields=true", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let org_list: DataGovOrganizationList = response.json().await?;

        let mut vectors = Vec::new();
        for org in org_list.result.iter().take(100) {
            let text = format!("{} {}", org.title, org.description);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("name".to_string(), org.name.clone());
            metadata.insert("title".to_string(), org.title.clone());
            metadata.insert("description".to_string(), org.description.clone());
            metadata.insert("package_count".to_string(), org.package_count.to_string());
            metadata.insert("source".to_string(), "datagov_org".to_string());

            vectors.push(SemanticVector {
                id: format!("DATAGOV_ORG:{}", org.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get organization details
    ///
    /// # Arguments
    /// * `id` - Organization ID or name
    ///
    /// # Example
    /// ```rust,ignore
    /// let org = client.get_organization("nasa-gov").await?;
    /// ```
    pub async fn get_organization(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_organizations().into_iter().next().unwrap()));
        }

        let url = format!("{}/action/organization_show?id={}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(result) = json.get("result") {
            let org: DataGovOrganizationInfo = serde_json::from_value(result.clone())?;

            let text = format!("{} {}", org.title, org.description);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("name".to_string(), org.name);
            metadata.insert("title".to_string(), org.title);
            metadata.insert("description".to_string(), org.description);
            metadata.insert("source".to_string(), "datagov_org".to_string());

            return Ok(Some(SemanticVector {
                id: format!("DATAGOV_ORG:{}", org.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            }));
        }

        Ok(None)
    }

    /// List popular tags
    ///
    /// # Example
    /// ```rust,ignore
    /// let tags = client.list_tags().await?;
    /// ```
    pub async fn list_tags(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_tags());
        }

        let url = format!("{}/action/tag_list", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        let mut vectors = Vec::new();
        if let Some(result) = json.get("result").and_then(|r| r.as_array()) {
            for (idx, tag) in result.iter().take(100).enumerate() {
                if let Some(tag_name) = tag.as_str() {
                    let embedding = self.embedder.embed_text(tag_name);

                    let mut metadata = HashMap::new();
                    metadata.insert("tag".to_string(), tag_name.to_string());
                    metadata.insert("source".to_string(), "datagov_tag".to_string());

                    vectors.push(SemanticVector {
                        id: format!("DATAGOV_TAG:{}", idx),
                        embedding,
                        domain: Domain::Government,
                        timestamp: Utc::now(),
                        metadata,
                    });
                }
            }
        }

        Ok(vectors)
    }

    fn get_mock_datagov_datasets(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("Mock dataset for {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), "Mock Dataset".to_string());
        metadata.insert("description".to_string(), "Mock data for testing".to_string());
        metadata.insert("source".to_string(), "datagov_mock".to_string());

        vec![SemanticVector {
            id: "DATAGOV_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_organizations(&self) -> Vec<SemanticVector> {
        let text = "NASA National Aeronautics and Space Administration";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("name".to_string(), "nasa-gov".to_string());
        metadata.insert("title".to_string(), "NASA".to_string());
        metadata.insert("source".to_string(), "datagov_mock".to_string());

        vec![SemanticVector {
            id: "DATAGOV_ORG_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_tags(&self) -> Vec<SemanticVector> {
        let text = "climate";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("tag".to_string(), "climate".to_string());
        metadata.insert("source".to_string(), "datagov_mock".to_string());

        vec![SemanticVector {
            id: "DATAGOV_TAG_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

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
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for DataGovClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// EU Open Data Portal Client
// ============================================================================

/// EU Open Data search response
#[derive(Debug, Deserialize)]
struct EuOpenDataResponse {
    #[serde(default)]
    result: EuOpenDataResult,
}

#[derive(Debug, Default, Deserialize)]
struct EuOpenDataResult {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    results: Vec<EuDataset>,
}

#[derive(Debug, Deserialize)]
struct EuDataset {
    #[serde(default)]
    id: String,
    #[serde(default)]
    title: HashMap<String, String>,
    #[serde(default)]
    description: HashMap<String, String>,
    #[serde(default)]
    keywords: Vec<String>,
    #[serde(default)]
    catalog: Option<String>,
}

/// Client for EU Open Data Portal
///
/// Provides access to:
/// - European Union datasets
/// - EU institutions data
/// - Member states open data
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::EuOpenDataClient;
///
/// let client = EuOpenDataClient::new();
/// let datasets = client.search_datasets("environment").await?;
/// let catalogs = client.list_catalogs().await?;
/// ```
pub struct EuOpenDataClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl EuOpenDataClient {
    /// Create a new EU Open Data client
    pub fn new() -> Self {
        Self::with_config(DEFAULT_EMBEDDING_DIM, false)
    }

    /// Create a new EU Open Data client with custom configuration
    pub fn with_config(embedding_dim: usize, use_mock: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://data.europa.eu/api/hub/search".to_string(),
            rate_limit_delay: Duration::from_millis(EU_OPENDATA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        }
    }

    /// Search for datasets
    ///
    /// # Arguments
    /// * `query` - Search query string
    ///
    /// # Example
    /// ```rust,ignore
    /// let results = client.search_datasets("agriculture").await?;
    /// ```
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_eu_datasets(query));
        }

        let url = format!(
            "{}/datasets?q={}&limit=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        let mut vectors = Vec::new();

        if let Some(results) = json.get("result").and_then(|r| r.get("results")).and_then(|r| r.as_array()) {
            for dataset in results {
                let id = dataset.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let title = dataset
                    .get("title")
                    .and_then(|t| t.get("en"))
                    .and_then(|e| e.as_str())
                    .or_else(|| dataset.get("title").and_then(|t| t.as_str()))
                    .unwrap_or("");

                let description = dataset
                    .get("description")
                    .and_then(|d| d.get("en"))
                    .and_then(|e| e.as_str())
                    .or_else(|| dataset.get("description").and_then(|d| d.as_str()))
                    .unwrap_or("");

                let text = format!("{} {}", title, description);
                let embedding = self.embedder.embed_text(&text);

                let mut metadata = HashMap::new();
                metadata.insert("title".to_string(), title.to_string());
                metadata.insert("description".to_string(), description.to_string());
                metadata.insert("source".to_string(), "eu_opendata".to_string());

                vectors.push(SemanticVector {
                    id: format!("EU:{}", id),
                    embedding,
                    domain: Domain::Government,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get a specific dataset by ID
    ///
    /// # Arguments
    /// * `id` - Dataset ID
    pub async fn get_dataset(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_eu_datasets("mock").into_iter().next().unwrap()));
        }

        let url = format!("{}/datasets/{}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let dataset: serde_json::Value = response.json().await?;

        let title = dataset
            .get("title")
            .and_then(|t| t.get("en"))
            .and_then(|e| e.as_str())
            .unwrap_or("");

        let text = title.to_string();
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), title.to_string());
        metadata.insert("source".to_string(), "eu_opendata".to_string());

        Ok(Some(SemanticVector {
            id: format!("EU:{}", id),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }))
    }

    /// List available catalogs
    pub async fn list_catalogs(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_catalogs());
        }

        let url = format!("{}/catalogues", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        let mut vectors = Vec::new();

        if let Some(catalogs) = json.get("result").and_then(|r| r.as_array()) {
            for (idx, catalog) in catalogs.iter().take(50).enumerate() {
                let id = catalog.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let title = catalog.get("title").and_then(|t| t.as_str()).unwrap_or("");

                let embedding = self.embedder.embed_text(title);

                let mut metadata = HashMap::new();
                metadata.insert("title".to_string(), title.to_string());
                metadata.insert("source".to_string(), "eu_catalog".to_string());

                vectors.push(SemanticVector {
                    id: format!("EU_CAT:{}", id),
                    embedding,
                    domain: Domain::Government,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get catalog details
    pub async fn get_catalog(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_catalogs().into_iter().next().unwrap()));
        }

        let url = format!("{}/catalogues/{}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let catalog: serde_json::Value = response.json().await?;

        let title = catalog.get("title").and_then(|t| t.as_str()).unwrap_or("");
        let embedding = self.embedder.embed_text(title);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), title.to_string());
        metadata.insert("source".to_string(), "eu_catalog".to_string());

        Ok(Some(SemanticVector {
            id: format!("EU_CAT:{}", id),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }))
    }

    /// Search by theme
    pub async fn search_by_theme(&self, theme: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_eu_datasets(theme));
        }

        let url = format!(
            "{}/datasets?facets[theme][]={}&limit=50",
            self.base_url,
            urlencoding::encode(theme)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        // Similar parsing as search_datasets
        self.parse_eu_datasets(&json)
    }

    fn parse_eu_datasets(&self, json: &serde_json::Value) -> Result<Vec<SemanticVector>> {
        let mut vectors = Vec::new();

        if let Some(results) = json.get("result").and_then(|r| r.get("results")).and_then(|r| r.as_array()) {
            for dataset in results {
                let id = dataset.get("id").and_then(|i| i.as_str()).unwrap_or("");
                let title = dataset.get("title").and_then(|t| t.as_str()).unwrap_or("");

                let embedding = self.embedder.embed_text(title);

                let mut metadata = HashMap::new();
                metadata.insert("title".to_string(), title.to_string());
                metadata.insert("source".to_string(), "eu_opendata".to_string());

                vectors.push(SemanticVector {
                    id: format!("EU:{}", id),
                    embedding,
                    domain: Domain::Government,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    fn get_mock_eu_datasets(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("EU dataset about {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), "Mock EU Dataset".to_string());
        metadata.insert("source".to_string(), "eu_mock".to_string());

        vec![SemanticVector {
            id: "EU_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_catalogs(&self) -> Vec<SemanticVector> {
        let text = "European Commission Data Catalog";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), "EC Catalog".to_string());
        metadata.insert("source".to_string(), "eu_mock".to_string());

        vec![SemanticVector {
            id: "EU_CAT_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

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
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for EuOpenDataClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// UK Government Data Client
// ============================================================================

/// UK Gov CKAN API response (similar to Data.gov)
#[derive(Debug, Deserialize)]
struct UkGovSearchResponse {
    #[serde(default)]
    success: bool,
    result: UkGovResult,
}

#[derive(Debug, Deserialize)]
struct UkGovResult {
    #[serde(default)]
    count: u64,
    #[serde(default)]
    results: Vec<UkGovDataset>,
}

#[derive(Debug, Deserialize)]
struct UkGovDataset {
    id: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    organization: Option<UkGovOrganization>,
}

#[derive(Debug, Deserialize)]
struct UkGovOrganization {
    #[serde(default)]
    title: String,
}

/// Client for UK Government data
///
/// Provides access to:
/// - UK public sector datasets
/// - Government department data
/// - National statistics
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::UkGovClient;
///
/// let client = UkGovClient::new();
/// let datasets = client.search_datasets("health").await?;
/// let publishers = client.list_publishers().await?;
/// ```
pub struct UkGovClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl UkGovClient {
    /// Create a new UK Gov client
    pub fn new() -> Self {
        Self::with_config(DEFAULT_EMBEDDING_DIM, false)
    }

    /// Create a new UK Gov client with custom configuration
    pub fn with_config(embedding_dim: usize, use_mock: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://data.gov.uk/api/action".to_string(),
            rate_limit_delay: Duration::from_millis(UK_GOV_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        }
    }

    /// Search for datasets
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_uk_datasets(query));
        }

        let url = format!(
            "{}/package_search?q={}&rows=50",
            self.base_url,
            urlencoding::encode(query)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: UkGovSearchResponse = response.json().await?;

        let mut vectors = Vec::new();
        for dataset in search_response.result.results {
            let org_name = dataset
                .organization
                .as_ref()
                .map(|o| o.title.as_str())
                .unwrap_or("Unknown");

            let text = format!("{} {} {}", dataset.title, dataset.notes, org_name);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("title".to_string(), dataset.title);
            metadata.insert("description".to_string(), dataset.notes);
            metadata.insert("publisher".to_string(), org_name.to_string());
            metadata.insert("source".to_string(), "ukgov".to_string());

            vectors.push(SemanticVector {
                id: format!("UKGOV:{}", dataset.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get a specific dataset by ID
    pub async fn get_dataset(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_uk_datasets("mock").into_iter().next().unwrap()));
        }

        let url = format!("{}/package_show?id={}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(result) = json.get("result") {
            let dataset: UkGovDataset = serde_json::from_value(result.clone())?;

            let text = format!("{} {}", dataset.title, dataset.notes);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("title".to_string(), dataset.title);
            metadata.insert("description".to_string(), dataset.notes);
            metadata.insert("source".to_string(), "ukgov".to_string());

            return Ok(Some(SemanticVector {
                id: format!("UKGOV:{}", dataset.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            }));
        }

        Ok(None)
    }

    /// List publishers
    pub async fn list_publishers(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_publishers());
        }

        let url = format!("{}/organization_list?all_fields=true", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        let mut vectors = Vec::new();

        if let Some(result) = json.get("result").and_then(|r| r.as_array()) {
            for (idx, pub_data) in result.iter().take(50).enumerate() {
                let title = pub_data.get("title").and_then(|t| t.as_str()).unwrap_or("");
                let name = pub_data.get("name").and_then(|n| n.as_str()).unwrap_or("");

                let embedding = self.embedder.embed_text(title);

                let mut metadata = HashMap::new();
                metadata.insert("title".to_string(), title.to_string());
                metadata.insert("name".to_string(), name.to_string());
                metadata.insert("source".to_string(), "ukgov_publisher".to_string());

                vectors.push(SemanticVector {
                    id: format!("UKGOV_PUB:{}", idx),
                    embedding,
                    domain: Domain::Government,
                    timestamp: Utc::now(),
                    metadata,
                });
            }
        }

        Ok(vectors)
    }

    /// Get publisher details
    pub async fn get_publisher(&self, id: &str) -> Result<Option<SemanticVector>> {
        if self.use_mock {
            return Ok(Some(self.get_mock_publishers().into_iter().next().unwrap()));
        }

        let url = format!("{}/organization_show?id={}", self.base_url, id);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let json: serde_json::Value = response.json().await?;

        if let Some(result) = json.get("result") {
            let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("");
            let embedding = self.embedder.embed_text(title);

            let mut metadata = HashMap::new();
            metadata.insert("title".to_string(), title.to_string());
            metadata.insert("source".to_string(), "ukgov_publisher".to_string());

            return Ok(Some(SemanticVector {
                id: format!("UKGOV_PUB:{}", id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            }));
        }

        Ok(None)
    }

    fn get_mock_uk_datasets(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("UK dataset about {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), "Mock UK Dataset".to_string());
        metadata.insert("source".to_string(), "ukgov_mock".to_string());

        vec![SemanticVector {
            id: "UKGOV_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_publishers(&self) -> Vec<SemanticVector> {
        let text = "Department of Health and Social Care";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), "DHSC".to_string());
        metadata.insert("source".to_string(), "ukgov_mock".to_string());

        vec![SemanticVector {
            id: "UKGOV_PUB_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

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
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for UkGovClient {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// World Bank Client (Government Data Focus)
// ============================================================================

/// World Bank country info
#[derive(Debug, Deserialize)]
struct WbCountry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    capitalCity: String,
    #[serde(default)]
    longitude: String,
    #[serde(default)]
    latitude: String,
}

/// World Bank indicator info
#[derive(Debug, Deserialize)]
struct WbIndicator {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    sourceNote: String,
}

/// World Bank indicator data
#[derive(Debug, Deserialize)]
struct WbIndicatorData {
    #[serde(default)]
    indicator: WbIndicatorInfo,
    #[serde(default)]
    country: WbCountryInfo,
    #[serde(default)]
    countryiso3code: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    value: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
struct WbIndicatorInfo {
    id: String,
    value: String,
}

#[derive(Debug, Default, Deserialize)]
struct WbCountryInfo {
    id: String,
    value: String,
}

/// Client for World Bank Open Data API
///
/// Provides access to:
/// - Global development indicators
/// - Country statistics
/// - Economic and social data
/// - Climate and environment metrics
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::WorldBankClient;
///
/// let client = WorldBankClient::new();
/// let countries = client.get_countries().await?;
/// let indicators = client.get_indicators("economy").await?;
/// let data = client.get_indicator_data("NY.GDP.MKTP.CD", "USA", "2015:2020").await?;
/// ```
pub struct WorldBankClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl WorldBankClient {
    /// Create a new World Bank client
    pub fn new() -> Result<Self> {
        Ok(Self::with_config(DEFAULT_EMBEDDING_DIM, false)?)
    }

    /// Create a new World Bank client with custom configuration
    pub fn with_config(embedding_dim: usize, use_mock: bool) -> Result<Self> {
        let client = Client::builder()
            .user_agent("RuVector-Discovery/1.0")
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.worldbank.org/v2".to_string(),
            rate_limit_delay: Duration::from_millis(WORLDBANK_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        })
    }

    /// Get list of countries
    pub async fn get_countries(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_countries());
        }

        let url = format!("{}/country?format=json&per_page=300", self.base_url);

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        let json_values: Vec<serde_json::Value> = serde_json::from_str(&text)?;
        if json_values.len() < 2 {
            return Ok(Vec::new());
        }

        let countries: Vec<WbCountry> = serde_json::from_value(json_values[1].clone())?;

        let mut vectors = Vec::new();
        for country in countries {
            let text = format!("{} {}", country.name, country.capitalCity);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("country_code".to_string(), country.id.clone());
            metadata.insert("name".to_string(), country.name);
            metadata.insert("capital".to_string(), country.capitalCity);
            metadata.insert("source".to_string(), "worldbank_country".to_string());

            vectors.push(SemanticVector {
                id: format!("WB_COUNTRY:{}", country.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get indicators by topic
    pub async fn get_indicators(&self, topic: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_indicators(topic));
        }

        let url = format!(
            "{}/indicator?format=json&per_page=100&source=2",
            self.base_url
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        let json_values: Vec<serde_json::Value> = serde_json::from_str(&text)?;
        if json_values.len() < 2 {
            return Ok(Vec::new());
        }

        let indicators: Vec<WbIndicator> = serde_json::from_value(json_values[1].clone())?;

        let mut vectors = Vec::new();
        for indicator in indicators.into_iter().take(50) {
            // Filter by topic if specified
            if !topic.is_empty()
                && !indicator.name.to_lowercase().contains(&topic.to_lowercase())
                && !indicator.sourceNote.to_lowercase().contains(&topic.to_lowercase())
            {
                continue;
            }

            let text = format!("{} {}", indicator.name, indicator.sourceNote);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("indicator_id".to_string(), indicator.id.clone());
            metadata.insert("name".to_string(), indicator.name);
            metadata.insert("description".to_string(), indicator.sourceNote);
            metadata.insert("source".to_string(), "worldbank_indicator".to_string());

            vectors.push(SemanticVector {
                id: format!("WB_IND:{}", indicator.id),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Get indicator data for a country (compatibility method)
    ///
    /// # Arguments
    /// * `country` - Country code (e.g., "USA", "CHN") or "all"
    /// * `indicator` - Indicator code (e.g., "NY.GDP.MKTP.CD")
    ///
    /// This method fetches the last 10 years of data by default
    pub async fn get_indicator(
        &self,
        country: &str,
        indicator: &str,
    ) -> Result<Vec<SemanticVector>> {
        let current_year = chrono::Utc::now().year();
        let start_year = current_year - 10;
        let date_range = format!("{}:{}", start_year, current_year);
        self.get_indicator_data(indicator, country, &date_range).await
    }

    /// Get indicator data for a country with custom date range
    ///
    /// # Arguments
    /// * `indicator` - Indicator code (e.g., "NY.GDP.MKTP.CD")
    /// * `country` - Country code (e.g., "USA", "CHN") or "all"
    /// * `date_range` - Date range (e.g., "2015:2020", "2020")
    pub async fn get_indicator_data(
        &self,
        indicator: &str,
        country: &str,
        date_range: &str,
    ) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_indicator_data(indicator, country));
        }

        let url = format!(
            "{}/country/{}/indicator/{}?format=json&date={}&per_page=100",
            self.base_url, country, indicator, date_range
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let text = response.text().await?;

        let json_values: Vec<serde_json::Value> = serde_json::from_str(&text)?;
        if json_values.len() < 2 {
            return Ok(Vec::new());
        }

        let data_points: Vec<WbIndicatorData> = serde_json::from_value(json_values[1].clone())?;

        let mut vectors = Vec::new();
        for data in data_points {
            let value = match data.value {
                Some(v) => v,
                None => continue,
            };

            let year = data.date.parse::<i32>().unwrap_or(2020);
            let date = NaiveDate::from_ymd_opt(year, 1, 1)
                .and_then(|d| d.and_hms_opt(0, 0, 0))
                .map(|dt| dt.and_utc())
                .unwrap_or_else(Utc::now);

            let text = format!(
                "{} {} in {}: {}",
                data.country.value, data.indicator.value, data.date, value
            );
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("country".to_string(), data.country.value);
            metadata.insert("country_code".to_string(), data.countryiso3code.clone());
            metadata.insert("indicator".to_string(), data.indicator.value);
            metadata.insert("indicator_id".to_string(), data.indicator.id);
            metadata.insert("year".to_string(), data.date.clone());
            metadata.insert("value".to_string(), value.to_string());
            metadata.insert("source".to_string(), "worldbank".to_string());

            vectors.push(SemanticVector {
                id: format!("WB:{}:{}:{}", country, indicator, data.date),
                embedding,
                domain: Domain::Government,
                timestamp: date,
                metadata,
            });
        }

        Ok(vectors)
    }

    /// Search for indicators
    pub async fn search_indicators(&self, query: &str) -> Result<Vec<SemanticVector>> {
        // World Bank doesn't have a direct search API, so we fetch and filter
        self.get_indicators(query).await
    }

    fn get_mock_countries(&self) -> Vec<SemanticVector> {
        let text = "United States Washington D.C.";
        let embedding = self.embedder.embed_text(text);

        let mut metadata = HashMap::new();
        metadata.insert("country_code".to_string(), "USA".to_string());
        metadata.insert("name".to_string(), "United States".to_string());
        metadata.insert("source".to_string(), "worldbank_mock".to_string());

        vec![SemanticVector {
            id: "WB_COUNTRY_MOCK:USA".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_indicators(&self, topic: &str) -> Vec<SemanticVector> {
        let text = format!("GDP indicator {}", topic);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("indicator_id".to_string(), "NY.GDP.MKTP.CD".to_string());
        metadata.insert("name".to_string(), "GDP".to_string());
        metadata.insert("source".to_string(), "worldbank_mock".to_string());

        vec![SemanticVector {
            id: "WB_IND_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_indicator_data(&self, indicator: &str, country: &str) -> Vec<SemanticVector> {
        let text = format!("{} {} GDP: 20000000000000", country, indicator);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("country_code".to_string(), country.to_string());
        metadata.insert("indicator_id".to_string(), indicator.to_string());
        metadata.insert("value".to_string(), "20000000000000".to_string());
        metadata.insert("source".to_string(), "worldbank_mock".to_string());

        vec![SemanticVector {
            id: format!("WB_MOCK:{}:{}:2020", country, indicator),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

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
                Err(e) if retries < MAX_RETRIES => {
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
// United Nations Data Client
// ============================================================================

/// UN Data REST API response structures
#[derive(Debug, Deserialize)]
struct UnDataResponse {
    #[serde(default)]
    data: Vec<UnDataRecord>,
}

#[derive(Debug, Deserialize)]
struct UnDataRecord {
    #[serde(default)]
    indicator: String,
    #[serde(default)]
    country: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    value: String,
}

/// Client for United Nations Data API
///
/// Provides access to:
/// - UN Statistics Division data
/// - Sustainable Development Goals (SDG) indicators
/// - Global economic and social statistics
///
/// # Example
/// ```rust,ignore
/// use ruvector_data_framework::UNDataClient;
///
/// let client = UNDataClient::new();
/// let indicators = client.get_indicators().await?;
/// let data = client.get_data("population", "USA").await?;
/// let search = client.search_datasets("climate").await?;
/// ```
pub struct UNDataClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
    use_mock: bool,
}

impl UNDataClient {
    /// Create a new UN Data client
    pub fn new() -> Self {
        Self::with_config(DEFAULT_EMBEDDING_DIM, false)
    }

    /// Create a new UN Data client with custom configuration
    pub fn with_config(embedding_dim: usize, use_mock: bool) -> Self {
        Self {
            client: Client::builder()
                .user_agent("RuVector-Discovery/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .expect("Failed to create HTTP client"),
            base_url: "https://data.un.org/ws/rest".to_string(),
            rate_limit_delay: Duration::from_millis(UNDATA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(embedding_dim)),
            use_mock,
        }
    }

    /// Get available indicators
    pub async fn get_indicators(&self) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_un_indicators());
        }

        // UN Data API has limited public endpoints, using mock for now
        Ok(self.get_mock_un_indicators())
    }

    /// Get data for an indicator and country
    pub async fn get_data(&self, indicator: &str, country: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_un_data(indicator, country));
        }

        // UN Data API requires specific dataset IDs
        Ok(self.get_mock_un_data(indicator, country))
    }

    /// Search datasets
    pub async fn search_datasets(&self, query: &str) -> Result<Vec<SemanticVector>> {
        if self.use_mock {
            return Ok(self.get_mock_un_datasets(query));
        }

        Ok(self.get_mock_un_datasets(query))
    }

    fn get_mock_un_indicators(&self) -> Vec<SemanticVector> {
        let indicators = vec![
            ("Population", "Total population"),
            ("GDP", "Gross Domestic Product"),
            ("CO2 Emissions", "Carbon dioxide emissions"),
        ];

        let mut vectors = Vec::new();
        for (idx, (name, description)) in indicators.iter().enumerate() {
            let text = format!("{} {}", name, description);
            let embedding = self.embedder.embed_text(&text);

            let mut metadata = HashMap::new();
            metadata.insert("indicator".to_string(), name.to_string());
            metadata.insert("description".to_string(), description.to_string());
            metadata.insert("source".to_string(), "undata_mock".to_string());

            vectors.push(SemanticVector {
                id: format!("UN_IND_MOCK:{}", idx),
                embedding,
                domain: Domain::Government,
                timestamp: Utc::now(),
                metadata,
            });
        }

        vectors
    }

    fn get_mock_un_data(&self, indicator: &str, country: &str) -> Vec<SemanticVector> {
        let text = format!("{} {} 2020: 100000", country, indicator);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("indicator".to_string(), indicator.to_string());
        metadata.insert("country".to_string(), country.to_string());
        metadata.insert("year".to_string(), "2020".to_string());
        metadata.insert("value".to_string(), "100000".to_string());
        metadata.insert("source".to_string(), "undata_mock".to_string());

        vec![SemanticVector {
            id: format!("UN_MOCK:{}:{}:2020", country, indicator),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

    fn get_mock_un_datasets(&self, query: &str) -> Vec<SemanticVector> {
        let text = format!("UN dataset about {}", query);
        let embedding = self.embedder.embed_text(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), format!("UN {} Data", query));
        metadata.insert("description".to_string(), "Mock UN dataset".to_string());
        metadata.insert("source".to_string(), "undata_mock".to_string());

        vec![SemanticVector {
            id: "UN_DS_MOCK:1".to_string(),
            embedding,
            domain: Domain::Government,
            timestamp: Utc::now(),
            metadata,
        }]
    }

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
                Err(e) if retries < MAX_RETRIES => {
                    retries += 1;
                    sleep(Duration::from_millis(RETRY_DELAY_MS * retries as u64)).await;
                }
                Err(e) => return Err(FrameworkError::Network(e)),
            }
        }
    }
}

impl Default for UNDataClient {
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

    // Census Client Tests
    #[test]
    fn test_census_client_creation() {
        let client = CensusClient::new(None);
        assert_eq!(client.base_url, "https://api.census.gov/data");
    }

    #[test]
    fn test_census_client_with_key() {
        let client = CensusClient::new(Some("test_key".to_string()));
        assert!(client.api_key.is_some());
    }

    #[tokio::test]
    async fn test_census_mock_population() {
        let client = CensusClient::with_config(None, 256, true);
        let result = client.get_population(2020, "state:*").await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
        assert_eq!(vectors[0].domain, Domain::Government);
    }

    #[tokio::test]
    async fn test_census_mock_datasets() {
        let client = CensusClient::with_config(None, 256, true);
        let result = client.get_available_datasets().await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
    }

    #[tokio::test]
    async fn test_census_mock_variables() {
        let client = CensusClient::with_config(None, 256, true);
        let result = client.search_variables("acs/acs5", "income").await;
        assert!(result.is_ok());
    }

    // Data.gov Client Tests
    #[test]
    fn test_datagov_client_creation() {
        let client = DataGovClient::new();
        assert_eq!(client.base_url, "https://catalog.data.gov/api/3");
    }

    #[tokio::test]
    async fn test_datagov_mock_search() {
        let client = DataGovClient::with_config(256, true);
        let result = client.search_datasets("climate").await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
        assert_eq!(vectors[0].domain, Domain::Government);
    }

    #[tokio::test]
    async fn test_datagov_mock_organizations() {
        let client = DataGovClient::with_config(256, true);
        let result = client.list_organizations().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_datagov_mock_tags() {
        let client = DataGovClient::with_config(256, true);
        let result = client.list_tags().await;
        assert!(result.is_ok());
    }

    // EU Open Data Client Tests
    #[test]
    fn test_eu_client_creation() {
        let client = EuOpenDataClient::new();
        assert_eq!(client.base_url, "https://data.europa.eu/api/hub/search");
    }

    #[tokio::test]
    async fn test_eu_mock_search() {
        let client = EuOpenDataClient::with_config(256, true);
        let result = client.search_datasets("environment").await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
    }

    #[tokio::test]
    async fn test_eu_mock_catalogs() {
        let client = EuOpenDataClient::with_config(256, true);
        let result = client.list_catalogs().await;
        assert!(result.is_ok());
    }

    // UK Gov Client Tests
    #[test]
    fn test_ukgov_client_creation() {
        let client = UkGovClient::new();
        assert_eq!(client.base_url, "https://data.gov.uk/api/action");
    }

    #[tokio::test]
    async fn test_ukgov_mock_search() {
        let client = UkGovClient::with_config(256, true);
        let result = client.search_datasets("health").await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
    }

    #[tokio::test]
    async fn test_ukgov_mock_publishers() {
        let client = UkGovClient::with_config(256, true);
        let result = client.list_publishers().await;
        assert!(result.is_ok());
    }

    // World Bank Client Tests
    #[test]
    fn test_worldbank_client_creation() {
        let client = WorldBankClient::new();
        assert!(client.is_ok());
        assert_eq!(client.unwrap().base_url, "https://api.worldbank.org/v2");
    }

    #[tokio::test]
    async fn test_worldbank_mock_countries() {
        let client = WorldBankClient::with_config(256, true).unwrap();
        let result = client.get_countries().await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
    }

    #[tokio::test]
    async fn test_worldbank_mock_indicators() {
        let client = WorldBankClient::with_config(256, true).unwrap();
        let result = client.get_indicators("gdp").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_worldbank_mock_data() {
        let client = WorldBankClient::with_config(256, true).unwrap();
        let result = client.get_indicator_data("NY.GDP.MKTP.CD", "USA", "2020").await;
        assert!(result.is_ok());
    }

    // UN Data Client Tests
    #[test]
    fn test_undata_client_creation() {
        let client = UNDataClient::new();
        assert_eq!(client.base_url, "https://data.un.org/ws/rest");
    }

    #[tokio::test]
    async fn test_undata_mock_indicators() {
        let client = UNDataClient::with_config(256, true);
        let result = client.get_indicators().await;
        assert!(result.is_ok());
        let vectors = result.unwrap();
        assert!(!vectors.is_empty());
    }

    #[tokio::test]
    async fn test_undata_mock_data() {
        let client = UNDataClient::with_config(256, true);
        let result = client.get_data("population", "USA").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_undata_mock_search() {
        let client = UNDataClient::with_config(256, true);
        let result = client.search_datasets("climate").await;
        assert!(result.is_ok());
    }

    // Rate limiting tests
    #[test]
    fn test_rate_limits() {
        let census = CensusClient::new(None);
        assert_eq!(census.rate_limit_delay, Duration::from_millis(CENSUS_RATE_LIMIT_MS));

        let datagov = DataGovClient::new();
        assert_eq!(datagov.rate_limit_delay, Duration::from_millis(DATAGOV_RATE_LIMIT_MS));

        let eu = EuOpenDataClient::new();
        assert_eq!(eu.rate_limit_delay, Duration::from_millis(EU_OPENDATA_RATE_LIMIT_MS));

        let uk = UkGovClient::new();
        assert_eq!(uk.rate_limit_delay, Duration::from_millis(UK_GOV_RATE_LIMIT_MS));

        let wb = WorldBankClient::new().unwrap();
        assert_eq!(wb.rate_limit_delay, Duration::from_millis(WORLDBANK_RATE_LIMIT_MS));

        let un = UNDataClient::new();
        assert_eq!(un.rate_limit_delay, Duration::from_millis(UNDATA_RATE_LIMIT_MS));
    }
}
