//! Medical data API integrations for PubMed, ClinicalTrials.gov, and FDA
//!
//! This module provides async clients for fetching medical literature, clinical trials,
//! and FDA data, converting responses to SemanticVector format for RuVector discovery.

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

/// Custom deserializer that handles both string and integer values
fn deserialize_number_from_string<'de, D>(deserializer: D) -> std::result::Result<Option<i32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};

    struct NumberOrStringVisitor;

    impl<'de> Visitor<'de> for NumberOrStringVisitor {
        type Value = Option<i32>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a number or numeric string")
        }

        fn visit_i64<E>(self, v: i64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as i32))
        }

        fn visit_u64<E>(self, v: u64) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(v as i32))
        }

        fn visit_str<E>(self, v: &str) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            v.parse::<i32>().map(Some).map_err(de::Error::custom)
        }

        fn visit_none<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> std::result::Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(NumberOrStringVisitor)
}

/// Rate limiting configuration
const NCBI_RATE_LIMIT_MS: u64 = 334; // ~3 requests/second without API key
const NCBI_WITH_KEY_RATE_LIMIT_MS: u64 = 100; // 10 requests/second with key
const FDA_RATE_LIMIT_MS: u64 = 250; // Conservative 4 requests/second
const CLINICALTRIALS_RATE_LIMIT_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// PubMed E-utilities Client
// ============================================================================

/// PubMed ESearch API response
#[derive(Debug, Deserialize)]
struct PubMedSearchResponse {
    esearchresult: ESearchResult,
}

#[derive(Debug, Deserialize)]
struct ESearchResult {
    #[serde(default)]
    idlist: Vec<String>,
    #[serde(default)]
    count: String,
}

/// PubMed EFetch API response (simplified)
#[derive(Debug, Deserialize)]
struct PubMedFetchResponse {
    #[serde(rename = "PubmedArticleSet")]
    pubmed_article_set: Option<PubmedArticleSet>,
}

#[derive(Debug, Deserialize)]
struct PubmedArticleSet {
    #[serde(rename = "PubmedArticle", default)]
    articles: Vec<PubmedArticle>,
}

#[derive(Debug, Deserialize)]
struct PubmedArticle {
    #[serde(rename = "MedlineCitation")]
    medline_citation: MedlineCitation,
}

#[derive(Debug, Deserialize)]
struct MedlineCitation {
    #[serde(rename = "PMID")]
    pmid: PmidObject,
    #[serde(rename = "Article")]
    article: Article,
    #[serde(rename = "DateCompleted", default)]
    date_completed: Option<DateCompleted>,
}

#[derive(Debug, Deserialize)]
struct PmidObject {
    #[serde(rename = "$value", default)]
    value: String,
}

#[derive(Debug, Deserialize)]
struct Article {
    #[serde(rename = "ArticleTitle", default)]
    article_title: Option<String>,
    #[serde(rename = "Abstract", default)]
    abstract_data: Option<AbstractData>,
    #[serde(rename = "AuthorList", default)]
    author_list: Option<AuthorList>,
}

#[derive(Debug, Deserialize)]
struct AbstractData {
    #[serde(rename = "AbstractText", default)]
    abstract_text: Vec<AbstractText>,
}

#[derive(Debug, Deserialize)]
struct AbstractText {
    #[serde(rename = "$value", default)]
    value: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorList {
    #[serde(rename = "Author", default)]
    authors: Vec<Author>,
}

#[derive(Debug, Deserialize)]
struct Author {
    #[serde(rename = "LastName", default)]
    last_name: Option<String>,
    #[serde(rename = "ForeName", default)]
    fore_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DateCompleted {
    #[serde(rename = "Year", default)]
    year: Option<String>,
    #[serde(rename = "Month", default)]
    month: Option<String>,
    #[serde(rename = "Day", default)]
    day: Option<String>,
}

/// Client for PubMed medical literature database
pub struct PubMedClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl PubMedClient {
    /// Create a new PubMed client
    ///
    /// # Arguments
    /// * `api_key` - Optional NCBI API key (get from https://www.ncbi.nlm.nih.gov/account/)
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        let rate_limit_delay = if api_key.is_some() {
            Duration::from_millis(NCBI_WITH_KEY_RATE_LIMIT_MS)
        } else {
            Duration::from_millis(NCBI_RATE_LIMIT_MS)
        };

        Ok(Self {
            client,
            base_url: "https://eutils.ncbi.nlm.nih.gov/entrez/eutils".to_string(),
            api_key,
            rate_limit_delay,
            embedder: Arc::new(SimpleEmbedder::new(384)), // Higher dimension for medical text
        })
    }

    /// Search PubMed articles by query
    ///
    /// # Arguments
    /// * `query` - Search query (e.g., "COVID-19 vaccine", "alzheimer's treatment")
    /// * `max_results` - Maximum number of results to return
    pub async fn search_articles(
        &self,
        query: &str,
        max_results: usize,
    ) -> Result<Vec<SemanticVector>> {
        // Step 1: Search for PMIDs
        let pmids = self.search_pmids(query, max_results).await?;

        if pmids.is_empty() {
            return Ok(Vec::new());
        }

        // Step 2: Fetch full abstracts for PMIDs
        self.fetch_abstracts(&pmids).await
    }

    /// Search for PMIDs matching query
    async fn search_pmids(&self, query: &str, max_results: usize) -> Result<Vec<String>> {
        let mut url = format!(
            "{}/esearch.fcgi?db=pubmed&term={}&retmode=json&retmax={}",
            self.base_url,
            urlencoding::encode(query),
            max_results
        );

        if let Some(key) = &self.api_key {
            url.push_str(&format!("&api_key={}", key));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let search_response: PubMedSearchResponse = response.json().await?;

        Ok(search_response.esearchresult.idlist)
    }

    /// Fetch full article abstracts by PMIDs
    ///
    /// # Arguments
    /// * `pmids` - List of PubMed IDs to fetch
    pub async fn fetch_abstracts(&self, pmids: &[String]) -> Result<Vec<SemanticVector>> {
        if pmids.is_empty() {
            return Ok(Vec::new());
        }

        // Batch PMIDs (max 200 per request)
        let mut all_vectors = Vec::new();

        for chunk in pmids.chunks(200) {
            let pmid_list = chunk.join(",");
            let mut url = format!(
                "{}/efetch.fcgi?db=pubmed&id={}&retmode=xml",
                self.base_url, pmid_list
            );

            if let Some(key) = &self.api_key {
                url.push_str(&format!("&api_key={}", key));
            }

            sleep(self.rate_limit_delay).await;
            let response = self.fetch_with_retry(&url).await?;
            let xml_text = response.text().await?;

            // Parse XML response
            let vectors = self.parse_xml_to_vectors(&xml_text)?;
            all_vectors.extend(vectors);
        }

        Ok(all_vectors)
    }

    /// Parse PubMed XML response to SemanticVectors
    fn parse_xml_to_vectors(&self, xml: &str) -> Result<Vec<SemanticVector>> {
        // Use quick-xml for parsing
        let fetch_response: PubMedFetchResponse = quick_xml::de::from_str(xml)
            .map_err(|e| FrameworkError::Config(format!("XML parse error: {}", e)))?;

        let mut vectors = Vec::new();

        if let Some(article_set) = fetch_response.pubmed_article_set {
            for pubmed_article in article_set.articles {
                let citation = pubmed_article.medline_citation;
                let article = citation.article;

                let pmid = citation.pmid.value;
                let title = article.article_title.unwrap_or_else(|| "Untitled".to_string());

                // Extract abstract text
                let abstract_text = article
                    .abstract_data
                    .as_ref()
                    .map(|abs| {
                        abs.abstract_text
                            .iter()
                            .filter_map(|at| at.value.clone())
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default();

                // Create combined text for embedding
                let text = format!("{} {}", title, abstract_text);
                let embedding = self.embedder.embed_text(&text);

                // Parse publication date
                let timestamp = citation
                    .date_completed
                    .as_ref()
                    .and_then(|date| {
                        let year = date.year.as_ref()?.parse::<i32>().ok()?;
                        let month = date.month.as_ref()?.parse::<u32>().ok()?;
                        let day = date.day.as_ref()?.parse::<u32>().ok()?;
                        NaiveDate::from_ymd_opt(year, month, day)
                    })
                    .and_then(|d| d.and_hms_opt(0, 0, 0))
                    .map(|dt| dt.and_utc())
                    .unwrap_or_else(Utc::now);

                // Extract author names
                let authors = article
                    .author_list
                    .as_ref()
                    .map(|al| {
                        al.authors
                            .iter()
                            .filter_map(|a| {
                                let last = a.last_name.as_deref().unwrap_or("");
                                let first = a.fore_name.as_deref().unwrap_or("");
                                if !last.is_empty() {
                                    Some(format!("{} {}", first, last))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();

                let mut metadata = HashMap::new();
                metadata.insert("pmid".to_string(), pmid.clone());
                metadata.insert("title".to_string(), title);
                metadata.insert("abstract".to_string(), abstract_text);
                metadata.insert("authors".to_string(), authors);
                metadata.insert("source".to_string(), "pubmed".to_string());

                vectors.push(SemanticVector {
                    id: format!("PMID:{}", pmid),
                    embedding,
                    domain: Domain::Medical,
                    timestamp,
                    metadata,
                });
            }
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
// ClinicalTrials.gov Client
// ============================================================================

/// ClinicalTrials.gov API response
#[derive(Debug, Deserialize)]
struct ClinicalTrialsResponse {
    #[serde(default)]
    studies: Vec<ClinicalStudy>,
}

#[derive(Debug, Deserialize)]
struct ClinicalStudy {
    #[serde(rename = "protocolSection")]
    protocol_section: ProtocolSection,
}

#[derive(Debug, Deserialize)]
struct ProtocolSection {
    #[serde(rename = "identificationModule")]
    identification: IdentificationModule,
    #[serde(rename = "statusModule")]
    status: StatusModule,
    #[serde(rename = "descriptionModule", default)]
    description: Option<DescriptionModule>,
    #[serde(rename = "conditionsModule", default)]
    conditions: Option<ConditionsModule>,
}

#[derive(Debug, Deserialize)]
struct IdentificationModule {
    #[serde(rename = "nctId")]
    nct_id: String,
    #[serde(rename = "briefTitle", default)]
    brief_title: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StatusModule {
    #[serde(rename = "overallStatus", default)]
    overall_status: Option<String>,
    #[serde(rename = "startDateStruct", default)]
    start_date: Option<DateStruct>,
}

#[derive(Debug, Deserialize)]
struct DateStruct {
    #[serde(default)]
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DescriptionModule {
    #[serde(rename = "briefSummary", default)]
    brief_summary: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConditionsModule {
    #[serde(default)]
    conditions: Vec<String>,
}

/// Client for ClinicalTrials.gov database
pub struct ClinicalTrialsClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl ClinicalTrialsClient {
    /// Create a new ClinicalTrials.gov client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://clinicaltrials.gov/api/v2".to_string(),
            rate_limit_delay: Duration::from_millis(CLINICALTRIALS_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search clinical trials by condition
    ///
    /// # Arguments
    /// * `condition` - Medical condition to search (e.g., "diabetes", "cancer")
    /// * `status` - Optional recruitment status filter (e.g., "RECRUITING", "COMPLETED")
    pub async fn search_trials(
        &self,
        condition: &str,
        status: Option<&str>,
    ) -> Result<Vec<SemanticVector>> {
        let mut url = format!(
            "{}/studies?query.cond={}&pageSize=100",
            self.base_url,
            urlencoding::encode(condition)
        );

        if let Some(s) = status {
            url.push_str(&format!("&filter.overallStatus={}", s));
        }

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;
        let trials_response: ClinicalTrialsResponse = response.json().await?;

        let mut vectors = Vec::new();
        for study in trials_response.studies {
            let vector = self.study_to_vector(study)?;
            vectors.push(vector);
        }

        Ok(vectors)
    }

    /// Convert clinical study to SemanticVector
    fn study_to_vector(&self, study: ClinicalStudy) -> Result<SemanticVector> {
        let protocol = study.protocol_section;
        let nct_id = protocol.identification.nct_id;
        let title = protocol
            .identification
            .brief_title
            .unwrap_or_else(|| "Untitled Study".to_string());

        let summary = protocol
            .description
            .as_ref()
            .and_then(|d| d.brief_summary.clone())
            .unwrap_or_default();

        let conditions = protocol
            .conditions
            .as_ref()
            .map(|c| c.conditions.join(", "))
            .unwrap_or_default();

        let status = protocol
            .status
            .overall_status
            .unwrap_or_else(|| "UNKNOWN".to_string());

        // Create text for embedding
        let text = format!("{} {} {}", title, summary, conditions);
        let embedding = self.embedder.embed_text(&text);

        // Parse start date
        let timestamp = protocol
            .status
            .start_date
            .as_ref()
            .and_then(|sd| sd.date.as_ref())
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut metadata = HashMap::new();
        metadata.insert("nct_id".to_string(), nct_id.clone());
        metadata.insert("title".to_string(), title);
        metadata.insert("summary".to_string(), summary);
        metadata.insert("conditions".to_string(), conditions);
        metadata.insert("status".to_string(), status);
        metadata.insert("source".to_string(), "clinicaltrials".to_string());

        Ok(SemanticVector {
            id: format!("NCT:{}", nct_id),
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

impl Default for ClinicalTrialsClient {
    fn default() -> Self {
        Self::new().expect("Failed to create ClinicalTrials client")
    }
}

// ============================================================================
// FDA OpenFDA Client
// ============================================================================

/// OpenFDA drug adverse event response
#[derive(Debug, Deserialize)]
struct FdaDrugEventResponse {
    results: Vec<FdaDrugEvent>,
}

#[derive(Debug, Deserialize)]
struct FdaDrugEvent {
    #[serde(rename = "safetyreportid")]
    safety_report_id: String,
    #[serde(rename = "receivedate", default)]
    receive_date: Option<String>,
    #[serde(default)]
    patient: Option<FdaPatient>,
    #[serde(default, deserialize_with = "deserialize_number_from_string")]
    serious: Option<i32>,
}

#[derive(Debug, Deserialize)]
struct FdaPatient {
    #[serde(default)]
    drug: Vec<FdaDrug>,
    #[serde(default)]
    reaction: Vec<FdaReaction>,
}

#[derive(Debug, Deserialize)]
struct FdaDrug {
    #[serde(rename = "medicinalproduct", default)]
    medicinal_product: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FdaReaction {
    #[serde(rename = "reactionmeddrapt", default)]
    reaction_meddra_pt: Option<String>,
}

/// OpenFDA device recall response
#[derive(Debug, Deserialize)]
struct FdaRecallResponse {
    results: Vec<FdaRecall>,
}

#[derive(Debug, Deserialize)]
struct FdaRecall {
    #[serde(rename = "recall_number")]
    recall_number: String,
    #[serde(default)]
    reason_for_recall: Option<String>,
    #[serde(default)]
    product_description: Option<String>,
    #[serde(default)]
    report_date: Option<String>,
    #[serde(default)]
    classification: Option<String>,
}

/// Client for FDA OpenFDA API
pub struct FdaClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl FdaClient {
    /// Create a new FDA OpenFDA client
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(FrameworkError::Network)?;

        Ok(Self {
            client,
            base_url: "https://api.fda.gov".to_string(),
            rate_limit_delay: Duration::from_millis(FDA_RATE_LIMIT_MS),
            embedder: Arc::new(SimpleEmbedder::new(384)),
        })
    }

    /// Search drug adverse events
    ///
    /// # Arguments
    /// * `drug_name` - Name of drug to search (e.g., "aspirin", "ibuprofen")
    pub async fn search_drug_events(&self, drug_name: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/drug/event.json?search=patient.drug.medicinalproduct:\"{}\"&limit=100",
            self.base_url,
            urlencoding::encode(drug_name)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        // FDA API may return 404 if no results - handle gracefully
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let events_response: FdaDrugEventResponse = response.json().await?;

        let mut vectors = Vec::new();
        for event in events_response.results {
            let vector = self.drug_event_to_vector(event)?;
            vectors.push(vector);
        }

        Ok(vectors)
    }

    /// Search device recalls
    ///
    /// # Arguments
    /// * `reason` - Reason for recall to search
    pub async fn search_recalls(&self, reason: &str) -> Result<Vec<SemanticVector>> {
        let url = format!(
            "{}/device/recall.json?search=reason_for_recall:\"{}\"&limit=100",
            self.base_url,
            urlencoding::encode(reason)
        );

        sleep(self.rate_limit_delay).await;
        let response = self.fetch_with_retry(&url).await?;

        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }

        let recalls_response: FdaRecallResponse = response.json().await?;

        let mut vectors = Vec::new();
        for recall in recalls_response.results {
            let vector = self.recall_to_vector(recall)?;
            vectors.push(vector);
        }

        Ok(vectors)
    }

    /// Convert drug event to SemanticVector
    fn drug_event_to_vector(&self, event: FdaDrugEvent) -> Result<SemanticVector> {
        let mut drug_names = Vec::new();
        let mut reactions = Vec::new();

        if let Some(patient) = &event.patient {
            for drug in &patient.drug {
                if let Some(name) = &drug.medicinal_product {
                    drug_names.push(name.clone());
                }
            }
            for reaction in &patient.reaction {
                if let Some(r) = &reaction.reaction_meddra_pt {
                    reactions.push(r.clone());
                }
            }
        }

        let drugs_text = drug_names.join(", ");
        let reactions_text = reactions.join(", ");
        let serious = if event.serious == Some(1) {
            "serious"
        } else {
            "non-serious"
        };

        // Create text for embedding
        let text = format!("Drug: {} Reactions: {} Severity: {}", drugs_text, reactions_text, serious);
        let embedding = self.embedder.embed_text(&text);

        // Parse receive date
        let timestamp = event
            .receive_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut metadata = HashMap::new();
        metadata.insert("report_id".to_string(), event.safety_report_id.clone());
        metadata.insert("drugs".to_string(), drugs_text);
        metadata.insert("reactions".to_string(), reactions_text);
        metadata.insert("serious".to_string(), serious.to_string());
        metadata.insert("source".to_string(), "fda_drug_events".to_string());

        Ok(SemanticVector {
            id: format!("FDA_EVENT:{}", event.safety_report_id),
            embedding,
            domain: Domain::Medical,
            timestamp,
            metadata,
        })
    }

    /// Convert recall to SemanticVector
    fn recall_to_vector(&self, recall: FdaRecall) -> Result<SemanticVector> {
        let reason = recall.reason_for_recall.unwrap_or_else(|| "Unknown reason".to_string());
        let product = recall.product_description.unwrap_or_else(|| "Unknown product".to_string());
        let classification = recall.classification.unwrap_or_else(|| "Unknown".to_string());

        // Create text for embedding
        let text = format!("Product: {} Reason: {} Classification: {}", product, reason, classification);
        let embedding = self.embedder.embed_text(&text);

        // Parse report date
        let timestamp = recall
            .report_date
            .as_ref()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y%m%d").ok())
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .map(|dt| dt.and_utc())
            .unwrap_or_else(Utc::now);

        let mut metadata = HashMap::new();
        metadata.insert("recall_number".to_string(), recall.recall_number.clone());
        metadata.insert("reason".to_string(), reason);
        metadata.insert("product".to_string(), product);
        metadata.insert("classification".to_string(), classification);
        metadata.insert("source".to_string(), "fda_recalls".to_string());

        Ok(SemanticVector {
            id: format!("FDA_RECALL:{}", recall.recall_number),
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

impl Default for FdaClient {
    fn default() -> Self {
        Self::new().expect("Failed to create FDA client")
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_pubmed_client_creation() {
        let client = PubMedClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_clinical_trials_client_creation() {
        let client = ClinicalTrialsClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_fda_client_creation() {
        let client = FdaClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_rate_limiting() {
        // Verify rate limits are set correctly
        let pubmed_without_key = PubMedClient::new(None).unwrap();
        assert_eq!(
            pubmed_without_key.rate_limit_delay,
            Duration::from_millis(NCBI_RATE_LIMIT_MS)
        );

        let pubmed_with_key = PubMedClient::new(Some("test_key".to_string())).unwrap();
        assert_eq!(
            pubmed_with_key.rate_limit_delay,
            Duration::from_millis(NCBI_WITH_KEY_RATE_LIMIT_MS)
        );
    }
}
