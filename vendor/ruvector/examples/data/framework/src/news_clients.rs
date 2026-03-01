//! News & Social Media API client integrations
//!
//! This module provides async clients for fetching data from news and social media APIs
//! and converting responses into RuVector's DataRecord format with embeddings.
//!
//! ## Clients
//!
//! - **HackerNewsClient**: Hacker News stories, comments, and user data
//! - **GuardianClient**: The Guardian news articles and sections
//! - **NewsDataClient**: NewsData.io latest and historical news
//! - **RedditClient**: Reddit posts and comments via JSON endpoints

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDateTime, Utc};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use tokio::time::sleep;

use crate::{DataRecord, DataSource, FrameworkError, Relationship, Result, SimpleEmbedder};

/// Rate limiting configuration
const DEFAULT_RATE_LIMIT_DELAY_MS: u64 = 100;
const MAX_RETRIES: u32 = 3;
const RETRY_DELAY_MS: u64 = 1000;

// ============================================================================
// HackerNews API Client
// ============================================================================

/// Hacker News item (story, comment, job, poll, etc.)
#[derive(Debug, Deserialize)]
struct HNItem {
    id: i64,
    #[serde(rename = "type")]
    item_type: String,
    by: Option<String>,
    time: i64,
    text: Option<String>,
    title: Option<String>,
    url: Option<String>,
    score: Option<i64>,
    #[serde(default)]
    kids: Vec<i64>,
    descendants: Option<i64>,
}

/// Hacker News user profile
#[derive(Debug, Deserialize)]
struct HNUser {
    id: String,
    created: i64,
    karma: i64,
    about: Option<String>,
    #[serde(default)]
    submitted: Vec<i64>,
}

/// Client for Hacker News API
pub struct HackerNewsClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl HackerNewsClient {
    /// Create a new Hacker News client
    ///
    /// No authentication required. API is generous with rate limits.
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://hacker-news.firebaseio.com/v0".to_string(),
            rate_limit_delay: Duration::from_millis(DEFAULT_RATE_LIMIT_DELAY_MS),
            embedder: Arc::new(SimpleEmbedder::new(128)),
        })
    }

    /// Get top story IDs
    ///
    /// # Arguments
    /// * `limit` - Maximum number of stories (capped at 500)
    pub async fn get_top_stories(&self, limit: usize) -> Result<Vec<DataRecord>> {
        let url = format!("{}/topstories.json", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let story_ids: Vec<i64> = response.json().await?;

        self.fetch_items(&story_ids[..limit.min(story_ids.len())])
            .await
    }

    /// Get new story IDs
    ///
    /// # Arguments
    /// * `limit` - Maximum number of stories
    pub async fn get_new_stories(&self, limit: usize) -> Result<Vec<DataRecord>> {
        let url = format!("{}/newstories.json", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let story_ids: Vec<i64> = response.json().await?;

        self.fetch_items(&story_ids[..limit.min(story_ids.len())])
            .await
    }

    /// Get best story IDs
    ///
    /// # Arguments
    /// * `limit` - Maximum number of stories
    pub async fn get_best_stories(&self, limit: usize) -> Result<Vec<DataRecord>> {
        let url = format!("{}/beststories.json", self.base_url);
        let response = self.fetch_with_retry(&url).await?;
        let story_ids: Vec<i64> = response.json().await?;

        self.fetch_items(&story_ids[..limit.min(story_ids.len())])
            .await
    }

    /// Get a single item by ID
    ///
    /// # Arguments
    /// * `id` - Item ID
    pub async fn get_item(&self, id: i64) -> Result<DataRecord> {
        let url = format!("{}/item/{}.json", self.base_url, id);
        let response = self.fetch_with_retry(&url).await?;
        let item: HNItem = response.json().await?;

        self.item_to_record(item)
    }

    /// Get user profile
    ///
    /// # Arguments
    /// * `username` - HN username
    pub async fn get_user(&self, username: &str) -> Result<DataRecord> {
        let url = format!("{}/user/{}.json", self.base_url, username);
        let response = self.fetch_with_retry(&url).await?;
        let user: HNUser = response.json().await?;

        self.user_to_record(user)
    }

    /// Fetch multiple items by ID
    async fn fetch_items(&self, ids: &[i64]) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();

        for &id in ids {
            match self.get_item(id).await {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!("Failed to fetch HN item {}: {}", id, e);
                }
            }
            sleep(self.rate_limit_delay).await;
        }

        Ok(records)
    }

    /// Convert HN item to DataRecord
    fn item_to_record(&self, item: HNItem) -> Result<DataRecord> {
        let text_content = format!(
            "{} {}",
            item.title.as_deref().unwrap_or(""),
            item.text.as_deref().unwrap_or("")
        );
        let embedding = self.embedder.embed_text(&text_content);

        // Convert Unix timestamp to DateTime
        let timestamp = DateTime::from_timestamp(item.time, 0).unwrap_or_else(Utc::now);

        // Build relationships
        let mut relationships = Vec::new();

        // Author relationship
        if let Some(author) = &item.by {
            relationships.push(Relationship {
                target_id: format!("hn_user_{}", author),
                rel_type: "authored_by".to_string(),
                weight: 1.0,
                properties: {
                    let mut props = HashMap::new();
                    props.insert("username".to_string(), serde_json::json!(author));
                    props
                },
            });
        }

        // Parent/child relationships for comments
        for &kid_id in &item.kids {
            relationships.push(Relationship {
                target_id: format!("hn_item_{}", kid_id),
                rel_type: "has_comment".to_string(),
                weight: 1.0,
                properties: HashMap::new(),
            });
        }

        let mut data_map = serde_json::Map::new();
        data_map.insert("item_type".to_string(), serde_json::json!(item.item_type));
        if let Some(title) = item.title {
            data_map.insert("title".to_string(), serde_json::json!(title));
        }
        if let Some(url) = item.url {
            data_map.insert("url".to_string(), serde_json::json!(url));
        }
        if let Some(text) = item.text {
            data_map.insert("text".to_string(), serde_json::json!(text));
        }
        if let Some(score) = item.score {
            data_map.insert("score".to_string(), serde_json::json!(score));
        }
        if let Some(descendants) = item.descendants {
            data_map.insert("descendants".to_string(), serde_json::json!(descendants));
        }
        data_map.insert("comments_count".to_string(), serde_json::json!(item.kids.len()));

        Ok(DataRecord {
            id: format!("hn_item_{}", item.id),
            source: "hackernews".to_string(),
            record_type: item.item_type,
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
        })
    }

    /// Convert HN user to DataRecord
    fn user_to_record(&self, user: HNUser) -> Result<DataRecord> {
        let text_content = format!(
            "{} {}",
            user.id,
            user.about.as_deref().unwrap_or("")
        );
        let embedding = self.embedder.embed_text(&text_content);

        let timestamp = DateTime::from_timestamp(user.created, 0).unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("username".to_string(), serde_json::json!(user.id));
        data_map.insert("karma".to_string(), serde_json::json!(user.karma));
        if let Some(about) = user.about {
            data_map.insert("about".to_string(), serde_json::json!(about));
        }
        data_map.insert(
            "submissions_count".to_string(),
            serde_json::json!(user.submitted.len()),
        );

        Ok(DataRecord {
            id: format!("hn_user_{}", user.id),
            source: "hackernews".to_string(),
            record_type: "user".to_string(),
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

#[async_trait]
impl DataSource for HackerNewsClient {
    fn source_id(&self) -> &str {
        "hackernews"
    }

    async fn fetch_batch(
        &self,
        _cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let records = self.get_top_stories(batch_size).await?;
        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/maxitem.json", self.base_url))
            .send()
            .await?;
        Ok(response.status().is_success())
    }
}

// ============================================================================
// Guardian API Client
// ============================================================================

/// Guardian API response
#[derive(Debug, Deserialize)]
struct GuardianResponse {
    response: GuardianResponseBody,
}

#[derive(Debug, Deserialize)]
struct GuardianResponseBody {
    status: String,
    #[serde(default)]
    results: Vec<GuardianArticle>,
}

#[derive(Debug, Deserialize)]
struct GuardianArticle {
    id: String,
    #[serde(rename = "type")]
    article_type: String,
    #[serde(rename = "sectionId")]
    section_id: Option<String>,
    #[serde(rename = "sectionName")]
    section_name: Option<String>,
    #[serde(rename = "webPublicationDate")]
    web_publication_date: String,
    #[serde(rename = "webTitle")]
    web_title: String,
    #[serde(rename = "webUrl")]
    web_url: String,
    #[serde(rename = "apiUrl")]
    api_url: String,
    fields: Option<GuardianFields>,
    tags: Option<Vec<GuardianTag>>,
}

#[derive(Debug, Deserialize)]
struct GuardianFields {
    body: Option<String>,
    headline: Option<String>,
    standfirst: Option<String>,
    #[serde(rename = "bodyText")]
    body_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GuardianTag {
    id: String,
    #[serde(rename = "type")]
    tag_type: String,
    #[serde(rename = "webTitle")]
    web_title: String,
}

/// Guardian sections response
#[derive(Debug, Deserialize)]
struct GuardianSectionsResponse {
    response: GuardianSectionsBody,
}

#[derive(Debug, Deserialize)]
struct GuardianSectionsBody {
    #[serde(default)]
    results: Vec<GuardianSection>,
}

#[derive(Debug, Deserialize)]
struct GuardianSection {
    id: String,
    #[serde(rename = "webTitle")]
    web_title: String,
    #[serde(rename = "webUrl")]
    web_url: String,
}

/// Client for The Guardian API
pub struct GuardianClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl GuardianClient {
    /// Create a new Guardian client
    ///
    /// # Arguments
    /// * `api_key` - Guardian API key (get from https://open-platform.theguardian.com/)
    ///
    /// Free tier: 12 calls/sec, 5000/day
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://content.guardianapis.com".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(100), // ~10 calls/sec to be safe
            embedder: Arc::new(SimpleEmbedder::new(128)),
        })
    }

    /// Search articles
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `limit` - Maximum number of results (capped at 200)
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_synthetic_articles(query, limit)?);
        }

        let url = format!(
            "{}/search?q={}&page-size={}&show-fields=all&show-tags=all&api-key={}",
            self.base_url,
            urlencoding::encode(query),
            limit.min(200),
            self.api_key.as_ref().unwrap()
        );

        let response = self.fetch_with_retry(&url).await?;
        let guardian_response: GuardianResponse = response.json().await?;

        let mut records = Vec::new();
        for article in guardian_response.response.results {
            let record = self.article_to_record(article)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Get article by ID
    ///
    /// # Arguments
    /// * `id` - Article ID (e.g., "world/2024/jan/01/article-slug")
    pub async fn get_article(&self, id: &str) -> Result<DataRecord> {
        if self.api_key.is_none() {
            return Err(FrameworkError::Config(
                "Guardian API key required".to_string(),
            ));
        }

        let url = format!(
            "{}/{}?show-fields=all&show-tags=all&api-key={}",
            self.base_url,
            id,
            self.api_key.as_ref().unwrap()
        );

        let response = self.fetch_with_retry(&url).await?;
        let guardian_response: GuardianResponse = response.json().await?;

        if let Some(article) = guardian_response.response.results.into_iter().next() {
            self.article_to_record(article)
        } else {
            Err(FrameworkError::Discovery("Article not found".to_string()))
        }
    }

    /// Get all sections
    pub async fn get_sections(&self) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_synthetic_sections()?);
        }

        let url = format!("{}/sections?api-key={}", self.base_url, self.api_key.as_ref().unwrap());

        let response = self.fetch_with_retry(&url).await?;
        let sections_response: GuardianSectionsResponse = response.json().await?;

        let mut records = Vec::new();
        for section in sections_response.response.results {
            let record = self.section_to_record(section)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Search by tag
    ///
    /// # Arguments
    /// * `tag` - Tag ID
    /// * `limit` - Maximum number of results
    pub async fn search_by_tag(&self, tag: &str, limit: usize) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_synthetic_articles(tag, limit)?);
        }

        let url = format!(
            "{}/search?tag={}&page-size={}&show-fields=all&api-key={}",
            self.base_url,
            urlencoding::encode(tag),
            limit.min(200),
            self.api_key.as_ref().unwrap()
        );

        let response = self.fetch_with_retry(&url).await?;
        let guardian_response: GuardianResponse = response.json().await?;

        let mut records = Vec::new();
        for article in guardian_response.response.results {
            let record = self.article_to_record(article)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Generate synthetic articles for demo
    fn generate_synthetic_articles(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();

        for i in 0..limit.min(10) {
            let title = format!("Synthetic Guardian article about {}: Part {}", query, i + 1);
            let text = format!(
                "This is a synthetic article for demonstration. Query: {}. Content would appear here.",
                query
            );
            let embedding = self.embedder.embed_text(&format!("{} {}", title, text));

            let mut data_map = serde_json::Map::new();
            data_map.insert("title".to_string(), serde_json::json!(title));
            data_map.insert("body_text".to_string(), serde_json::json!(text));
            data_map.insert("section".to_string(), serde_json::json!("world"));
            data_map.insert(
                "url".to_string(),
                serde_json::json!(format!(
                    "https://www.theguardian.com/world/synthetic-{}",
                    i
                )),
            );

            records.push(DataRecord {
                id: format!("guardian_synthetic_{}", i),
                source: "guardian".to_string(),
                record_type: "article".to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }

        Ok(records)
    }

    /// Generate synthetic sections for demo
    fn generate_synthetic_sections(&self) -> Result<Vec<DataRecord>> {
        let sections = vec!["world", "politics", "business", "technology", "science"];
        let mut records = Vec::new();

        for (_i, section) in sections.iter().enumerate() {
            let embedding = self.embedder.embed_text(section);

            let mut data_map = serde_json::Map::new();
            data_map.insert("section_id".to_string(), serde_json::json!(section));
            data_map.insert(
                "title".to_string(),
                serde_json::json!(format!("{} News", section)),
            );

            records.push(DataRecord {
                id: format!("guardian_section_{}", section),
                source: "guardian".to_string(),
                record_type: "section".to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }

        Ok(records)
    }

    /// Convert article to DataRecord
    fn article_to_record(&self, article: GuardianArticle) -> Result<DataRecord> {
        let body_text = article
            .fields
            .as_ref()
            .and_then(|f| f.body_text.as_deref())
            .unwrap_or("");
        let text_content = format!("{} {}", article.web_title, body_text);
        let embedding = self.embedder.embed_text(&text_content);

        // Parse publication date
        let timestamp = DateTime::parse_from_rfc3339(&article.web_publication_date)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        // Build relationships for tags
        let mut relationships = Vec::new();
        if let Some(tags) = article.tags {
            for tag in tags {
                relationships.push(Relationship {
                    target_id: format!("guardian_tag_{}", tag.id),
                    rel_type: "has_tag".to_string(),
                    weight: 1.0,
                    properties: {
                        let mut props = HashMap::new();
                        props.insert("tag_type".to_string(), serde_json::json!(tag.tag_type));
                        props.insert("tag_title".to_string(), serde_json::json!(tag.web_title));
                        props
                    },
                });
            }
        }

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(article.web_title));
        data_map.insert("url".to_string(), serde_json::json!(article.web_url));
        data_map.insert("api_url".to_string(), serde_json::json!(article.api_url));
        if let Some(section_name) = article.section_name {
            data_map.insert("section".to_string(), serde_json::json!(section_name));
        }
        if let Some(fields) = article.fields {
            if let Some(headline) = fields.headline {
                data_map.insert("headline".to_string(), serde_json::json!(headline));
            }
            if let Some(standfirst) = fields.standfirst {
                data_map.insert("standfirst".to_string(), serde_json::json!(standfirst));
            }
            if let Some(body_text) = fields.body_text {
                data_map.insert("body_text".to_string(), serde_json::json!(body_text));
            }
        }

        Ok(DataRecord {
            id: format!("guardian_{}", article.id.replace('/', "_")),
            source: "guardian".to_string(),
            record_type: article.article_type,
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
        })
    }

    /// Convert section to DataRecord
    fn section_to_record(&self, section: GuardianSection) -> Result<DataRecord> {
        let embedding = self.embedder.embed_text(&section.web_title);

        let mut data_map = serde_json::Map::new();
        data_map.insert("section_id".to_string(), serde_json::json!(section.id));
        data_map.insert("title".to_string(), serde_json::json!(section.web_title));
        data_map.insert("url".to_string(), serde_json::json!(section.web_url));

        Ok(DataRecord {
            id: format!("guardian_section_{}", section.id),
            source: "guardian".to_string(),
            record_type: "section".to_string(),
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
impl DataSource for GuardianClient {
    fn source_id(&self) -> &str {
        "guardian"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let query = cursor.as_deref().unwrap_or("technology");
        let records = self.search(query, batch_size).await?;
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
// NewsData.io API Client
// ============================================================================

/// NewsData.io response
#[derive(Debug, Deserialize)]
struct NewsDataResponse {
    status: String,
    #[serde(default)]
    results: Vec<NewsDataArticle>,
    #[serde(rename = "nextPage")]
    next_page: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NewsDataArticle {
    #[serde(rename = "article_id")]
    article_id: String,
    title: String,
    link: String,
    #[serde(default)]
    keywords: Vec<String>,
    creator: Option<Vec<String>>,
    description: Option<String>,
    content: Option<String>,
    #[serde(rename = "pubDate")]
    pub_date: Option<String>,
    #[serde(rename = "image_url")]
    image_url: Option<String>,
    #[serde(rename = "source_id")]
    source_id: String,
    category: Option<Vec<String>>,
    country: Option<Vec<String>>,
    language: Option<String>,
}

/// Client for NewsData.io API
pub struct NewsDataClient {
    client: Client,
    base_url: String,
    api_key: Option<String>,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl NewsDataClient {
    /// Create a new NewsData client
    ///
    /// # Arguments
    /// * `api_key` - NewsData.io API key (get from https://newsdata.io/)
    ///
    /// Free tier: 200 requests/day
    pub fn new(api_key: Option<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://newsdata.io/api/1".to_string(),
            api_key,
            rate_limit_delay: Duration::from_millis(500), // Be conservative with free tier
            embedder: Arc::new(SimpleEmbedder::new(128)),
        })
    }

    /// Get latest news
    ///
    /// # Arguments
    /// * `query` - Search query (optional)
    /// * `country` - Country code (optional, e.g., "us", "gb")
    /// * `category` - Category (optional, e.g., "technology", "business")
    pub async fn get_latest(
        &self,
        query: Option<&str>,
        country: Option<&str>,
        category: Option<&str>,
    ) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_synthetic_news(
                query.unwrap_or("technology"),
                10,
            )?);
        }

        let mut url = format!(
            "{}/news?apikey={}",
            self.base_url,
            self.api_key.as_ref().unwrap()
        );

        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }
        if let Some(c) = country {
            url.push_str(&format!("&country={}", c));
        }
        if let Some(cat) = category {
            url.push_str(&format!("&category={}", cat));
        }

        let response = self.fetch_with_retry(&url).await?;
        let news_response: NewsDataResponse = response.json().await?;

        let mut records = Vec::new();
        for article in news_response.results {
            let record = self.article_to_record(article)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Get archived/historical news
    ///
    /// # Arguments
    /// * `query` - Search query (optional)
    /// * `from_date` - Start date (YYYY-MM-DD)
    /// * `to_date` - End date (YYYY-MM-DD)
    pub async fn get_archive(
        &self,
        query: Option<&str>,
        from_date: &str,
        to_date: &str,
    ) -> Result<Vec<DataRecord>> {
        if self.api_key.is_none() {
            return Ok(self.generate_synthetic_news(
                query.unwrap_or("archive"),
                10,
            )?);
        }

        let mut url = format!(
            "{}/archive?apikey={}&from_date={}&to_date={}",
            self.base_url,
            self.api_key.as_ref().unwrap(),
            from_date,
            to_date
        );

        if let Some(q) = query {
            url.push_str(&format!("&q={}", urlencoding::encode(q)));
        }

        let response = self.fetch_with_retry(&url).await?;
        let news_response: NewsDataResponse = response.json().await?;

        let mut records = Vec::new();
        for article in news_response.results {
            let record = self.article_to_record(article)?;
            records.push(record);
        }

        Ok(records)
    }

    /// Generate synthetic news for demo
    fn generate_synthetic_news(&self, query: &str, limit: usize) -> Result<Vec<DataRecord>> {
        let mut records = Vec::new();

        for i in 0..limit {
            let title = format!("Synthetic news article about {}: Story {}", query, i + 1);
            let description = format!(
                "This is synthetic news content for demonstration. Topic: {}",
                query
            );
            let embedding = self.embedder.embed_text(&format!("{} {}", title, description));

            let mut data_map = serde_json::Map::new();
            data_map.insert("title".to_string(), serde_json::json!(title));
            data_map.insert("description".to_string(), serde_json::json!(description));
            data_map.insert(
                "url".to_string(),
                serde_json::json!(format!("https://example.com/news/{}", i)),
            );
            data_map.insert("source".to_string(), serde_json::json!("synthetic"));
            data_map.insert("category".to_string(), serde_json::json!(["technology"]));

            records.push(DataRecord {
                id: format!("newsdata_synthetic_{}", i),
                source: "newsdata".to_string(),
                record_type: "article".to_string(),
                timestamp: Utc::now(),
                data: serde_json::Value::Object(data_map),
                embedding: Some(embedding),
                relationships: Vec::new(),
            });
        }

        Ok(records)
    }

    /// Convert article to DataRecord
    fn article_to_record(&self, article: NewsDataArticle) -> Result<DataRecord> {
        let description = article.description.as_deref().unwrap_or("");
        let content = article.content.as_deref().unwrap_or("");
        let text_content = format!("{} {} {}", article.title, description, content);
        let embedding = self.embedder.embed_text(&text_content);

        // Parse publication date
        let timestamp = article
            .pub_date
            .as_ref()
            .and_then(|d| {
                // Try multiple date formats
                DateTime::parse_from_rfc3339(d)
                    .ok()
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|| {
                        NaiveDateTime::parse_from_str(d, "%Y-%m-%d %H:%M:%S")
                            .ok()
                            .map(|ndt| ndt.and_utc())
                    })
            })
            .unwrap_or_else(Utc::now);

        let mut data_map = serde_json::Map::new();
        data_map.insert("title".to_string(), serde_json::json!(article.title));
        data_map.insert("url".to_string(), serde_json::json!(article.link));
        data_map.insert("source".to_string(), serde_json::json!(article.source_id));

        if let Some(desc) = article.description {
            data_map.insert("description".to_string(), serde_json::json!(desc));
        }
        if let Some(content) = article.content {
            data_map.insert("content".to_string(), serde_json::json!(content));
        }
        if let Some(image) = article.image_url {
            data_map.insert("image_url".to_string(), serde_json::json!(image));
        }
        if let Some(lang) = article.language {
            data_map.insert("language".to_string(), serde_json::json!(lang));
        }
        if !article.keywords.is_empty() {
            data_map.insert("keywords".to_string(), serde_json::json!(article.keywords));
        }
        if let Some(categories) = article.category {
            data_map.insert("categories".to_string(), serde_json::json!(categories));
        }
        if let Some(countries) = article.country {
            data_map.insert("countries".to_string(), serde_json::json!(countries));
        }
        if let Some(creators) = article.creator {
            data_map.insert("creators".to_string(), serde_json::json!(creators));
        }

        Ok(DataRecord {
            id: format!("newsdata_{}", article.article_id),
            source: "newsdata".to_string(),
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
impl DataSource for NewsDataClient {
    fn source_id(&self) -> &str {
        "newsdata"
    }

    async fn fetch_batch(
        &self,
        _cursor: Option<String>,
        _batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let records = self.get_latest(Some("technology"), None, None).await?;
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
// Reddit API Client (JSON endpoints)
// ============================================================================

/// Reddit listing response
#[derive(Debug, Deserialize)]
struct RedditListing {
    data: RedditListingData,
}

#[derive(Debug, Deserialize)]
struct RedditListingData {
    #[serde(default)]
    children: Vec<RedditChild>,
    after: Option<String>,
    before: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RedditChild {
    kind: String,
    data: RedditPost,
}

/// Reddit post/comment data
#[derive(Debug, Clone, Deserialize)]
struct RedditPost {
    id: String,
    name: String,
    title: Option<String>,
    selftext: Option<String>,
    body: Option<String>,
    author: String,
    subreddit: String,
    #[serde(rename = "subreddit_id")]
    subreddit_id: String,
    score: i64,
    #[serde(rename = "num_comments")]
    num_comments: Option<i64>,
    created_utc: f64,
    permalink: String,
    url: Option<String>,
    #[serde(default)]
    is_self: bool,
    domain: Option<String>,
}

/// Client for Reddit JSON endpoints
pub struct RedditClient {
    client: Client,
    base_url: String,
    rate_limit_delay: Duration,
    embedder: Arc<SimpleEmbedder>,
}

impl RedditClient {
    /// Create a new Reddit client
    ///
    /// No authentication required for .json endpoints.
    /// Be respectful with rate limiting.
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("RuVector Data Framework/1.0")
            .build()
            .map_err(|e| FrameworkError::Network(e))?;

        Ok(Self {
            client,
            base_url: "https://www.reddit.com".to_string(),
            rate_limit_delay: Duration::from_millis(1000), // Be respectful: 1 req/sec
            embedder: Arc::new(SimpleEmbedder::new(128)),
        })
    }

    /// Get subreddit posts
    ///
    /// # Arguments
    /// * `subreddit` - Subreddit name (without r/)
    /// * `sort` - Sort method: "hot", "new", "top", "rising"
    /// * `limit` - Maximum number of posts (capped at 100)
    pub async fn get_subreddit_posts(
        &self,
        subreddit: &str,
        sort: &str,
        limit: usize,
    ) -> Result<Vec<DataRecord>> {
        let url = format!(
            "{}/r/{}/{}.json?limit={}",
            self.base_url,
            subreddit,
            sort,
            limit.min(100)
        );

        let response = self.fetch_with_retry(&url).await?;
        let listing: RedditListing = response.json().await?;

        let mut records = Vec::new();
        for child in &listing.data.children {
            if child.kind == "t3" {
                // t3 = link/post
                let record = self.post_to_record(&child.data, "post")?;
                records.push(record);
            }
        }

        Ok(records)
    }

    /// Get post comments
    ///
    /// # Arguments
    /// * `post_id` - Reddit post ID (e.g., "abc123")
    pub async fn get_post_comments(&self, post_id: &str) -> Result<Vec<DataRecord>> {
        // Reddit comment API returns [post_listing, comments_listing]
        let url = format!("{}/comments/{}.json", self.base_url, post_id);

        let response = self.fetch_with_retry(&url).await?;
        let listings: Vec<RedditListing> = response.json().await?;

        let mut records = Vec::new();

        // Second listing contains comments
        if listings.len() >= 2 {
            for child in &listings[1].data.children {
                if child.kind == "t1" {
                    // t1 = comment
                    let record = self.post_to_record(&child.data, "comment")?;
                    records.push(record);
                }
            }
        }

        Ok(records)
    }

    /// Search Reddit
    ///
    /// # Arguments
    /// * `query` - Search query
    /// * `subreddit` - Optional subreddit to search within
    /// * `limit` - Maximum number of results
    pub async fn search(
        &self,
        query: &str,
        subreddit: Option<&str>,
        limit: usize,
    ) -> Result<Vec<DataRecord>> {
        let url = if let Some(sub) = subreddit {
            format!(
                "{}/r/{}/search.json?q={}&restrict_sr=on&limit={}",
                self.base_url,
                sub,
                urlencoding::encode(query),
                limit.min(100)
            )
        } else {
            format!(
                "{}/search.json?q={}&limit={}",
                self.base_url,
                urlencoding::encode(query),
                limit.min(100)
            )
        };

        let response = self.fetch_with_retry(&url).await?;
        let listing: RedditListing = response.json().await?;

        let mut records = Vec::new();
        for child in &listing.data.children {
            if child.kind == "t3" {
                let record = self.post_to_record(&child.data, "post")?;
                records.push(record);
            }
        }

        Ok(records)
    }

    /// Convert Reddit post/comment to DataRecord
    fn post_to_record(&self, post: &RedditPost, record_type: &str) -> Result<DataRecord> {
        let text_content = format!(
            "{} {} {}",
            post.title.as_deref().unwrap_or(""),
            post.selftext.as_deref().unwrap_or(""),
            post.body.as_deref().unwrap_or("")
        );
        let embedding = self.embedder.embed_text(&text_content);

        // Convert Unix timestamp
        let timestamp =
            DateTime::from_timestamp(post.created_utc as i64, 0).unwrap_or_else(Utc::now);

        // Build relationships
        let mut relationships = Vec::new();

        // Author relationship
        relationships.push(Relationship {
            target_id: format!("reddit_user_{}", post.author),
            rel_type: "authored_by".to_string(),
            weight: 1.0,
            properties: {
                let mut props = HashMap::new();
                props.insert("username".to_string(), serde_json::json!(post.author));
                props
            },
        });

        // Subreddit relationship
        relationships.push(Relationship {
            target_id: format!("reddit_sub_{}", post.subreddit),
            rel_type: "posted_in".to_string(),
            weight: 1.0,
            properties: {
                let mut props = HashMap::new();
                props.insert("subreddit".to_string(), serde_json::json!(post.subreddit));
                props
            },
        });

        let mut data_map = serde_json::Map::new();
        data_map.insert("post_id".to_string(), serde_json::json!(post.id));
        data_map.insert("name".to_string(), serde_json::json!(post.name));
        data_map.insert("author".to_string(), serde_json::json!(post.author));
        data_map.insert("subreddit".to_string(), serde_json::json!(post.subreddit));
        data_map.insert("score".to_string(), serde_json::json!(post.score));
        data_map.insert(
            "permalink".to_string(),
            serde_json::json!(format!("{}{}", self.base_url, post.permalink)),
        );

        if let Some(title) = &post.title {
            data_map.insert("title".to_string(), serde_json::json!(title));
        }
        if let Some(selftext) = &post.selftext {
            data_map.insert("selftext".to_string(), serde_json::json!(selftext));
        }
        if let Some(body) = &post.body {
            data_map.insert("body".to_string(), serde_json::json!(body));
        }
        if let Some(url) = &post.url {
            data_map.insert("url".to_string(), serde_json::json!(url));
        }
        if let Some(num_comments) = post.num_comments {
            data_map.insert("num_comments".to_string(), serde_json::json!(num_comments));
        }
        if let Some(domain) = &post.domain {
            data_map.insert("domain".to_string(), serde_json::json!(domain));
        }
        data_map.insert("is_self".to_string(), serde_json::json!(post.is_self));

        Ok(DataRecord {
            id: format!("reddit_{}", post.name),
            source: "reddit".to_string(),
            record_type: record_type.to_string(),
            timestamp,
            data: serde_json::Value::Object(data_map),
            embedding: Some(embedding),
            relationships,
        })
    }

    /// Fetch with retry logic
    async fn fetch_with_retry(&self, url: &str) -> Result<reqwest::Response> {
        let mut retries = 0;
        loop {
            sleep(self.rate_limit_delay).await; // Rate limit before request

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
impl DataSource for RedditClient {
    fn source_id(&self) -> &str {
        "reddit"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        let subreddit = cursor.as_deref().unwrap_or("technology");
        let records = self.get_subreddit_posts(subreddit, "hot", batch_size).await?;
        Ok((records, None))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        Ok(None)
    }

    async fn health_check(&self) -> Result<bool> {
        let response = self
            .client
            .get(format!("{}/r/technology/hot.json?limit=1", self.base_url))
            .send()
            .await?;
        Ok(response.status().is_success())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    // HackerNews Tests
    #[tokio::test]
    async fn test_hackernews_client_creation() {
        let client = HackerNewsClient::new();
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_hackernews_health_check() {
        let client = HackerNewsClient::new().unwrap();
        let health = client.health_check().await;
        assert!(health.is_ok());
    }

    #[test]
    fn test_hackernews_item_conversion() {
        let client = HackerNewsClient::new().unwrap();
        let item = HNItem {
            id: 123,
            item_type: "story".to_string(),
            by: Some("testuser".to_string()),
            time: 1609459200, // 2021-01-01
            text: None,
            title: Some("Test Story".to_string()),
            url: Some("https://example.com".to_string()),
            score: Some(100),
            kids: vec![456, 789],
            descendants: Some(2),
        };

        let record = client.item_to_record(item).unwrap();
        assert_eq!(record.source, "hackernews");
        assert_eq!(record.record_type, "story");
        assert!(record.embedding.is_some());
        assert_eq!(record.relationships.len(), 3); // author + 2 comments
    }

    #[test]
    fn test_hackernews_user_conversion() {
        let client = HackerNewsClient::new().unwrap();
        let user = HNUser {
            id: "testuser".to_string(),
            created: 1609459200,
            karma: 5000,
            about: Some("Test user bio".to_string()),
            submitted: vec![1, 2, 3],
        };

        let record = client.user_to_record(user).unwrap();
        assert_eq!(record.source, "hackernews");
        assert_eq!(record.record_type, "user");
        assert!(record.embedding.is_some());
    }

    // Guardian Tests
    #[tokio::test]
    async fn test_guardian_client_creation() {
        let client = GuardianClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_guardian_synthetic_articles() {
        let client = GuardianClient::new(None).unwrap();
        let records = client.search("climate", 5).await.unwrap();

        assert!(!records.is_empty());
        assert_eq!(records[0].source, "guardian");
        assert!(records[0].embedding.is_some());
    }

    #[tokio::test]
    async fn test_guardian_synthetic_sections() {
        let client = GuardianClient::new(None).unwrap();
        let records = client.get_sections().await.unwrap();

        assert!(!records.is_empty());
        assert_eq!(records[0].source, "guardian");
        assert_eq!(records[0].record_type, "section");
    }

    #[test]
    fn test_guardian_article_conversion() {
        let client = GuardianClient::new(None).unwrap();
        let article = GuardianArticle {
            id: "world/2024/jan/01/test".to_string(),
            article_type: "article".to_string(),
            section_id: Some("world".to_string()),
            section_name: Some("World news".to_string()),
            web_publication_date: "2024-01-01T12:00:00Z".to_string(),
            web_title: "Test Article".to_string(),
            web_url: "https://theguardian.com/test".to_string(),
            api_url: "https://content.guardianapis.com/test".to_string(),
            fields: Some(GuardianFields {
                body: None,
                headline: Some("Test Headline".to_string()),
                standfirst: Some("Test standfirst".to_string()),
                body_text: Some("Test body text".to_string()),
            }),
            tags: None,
        };

        let record = client.article_to_record(article).unwrap();
        assert_eq!(record.source, "guardian");
        assert!(record.embedding.is_some());
    }

    // NewsData Tests
    #[tokio::test]
    async fn test_newsdata_client_creation() {
        let client = NewsDataClient::new(None);
        assert!(client.is_ok());
    }

    #[tokio::test]
    async fn test_newsdata_synthetic_news() {
        let client = NewsDataClient::new(None).unwrap();
        let records = client.get_latest(Some("technology"), None, None).await.unwrap();

        assert!(!records.is_empty());
        assert_eq!(records[0].source, "newsdata");
        assert!(records[0].embedding.is_some());
    }

    #[test]
    fn test_newsdata_article_conversion() {
        let client = NewsDataClient::new(None).unwrap();
        let article = NewsDataArticle {
            article_id: "test123".to_string(),
            title: "Test News".to_string(),
            link: "https://example.com/news".to_string(),
            keywords: vec!["tech".to_string(), "ai".to_string()],
            creator: Some(vec!["Author Name".to_string()]),
            description: Some("Test description".to_string()),
            content: Some("Test content".to_string()),
            pub_date: Some("2024-01-01 12:00:00".to_string()),
            image_url: Some("https://example.com/image.jpg".to_string()),
            source_id: "testsource".to_string(),
            category: Some(vec!["technology".to_string()]),
            country: Some(vec!["us".to_string()]),
            language: Some("en".to_string()),
        };

        let record = client.article_to_record(article).unwrap();
        assert_eq!(record.source, "newsdata");
        assert!(record.embedding.is_some());
    }

    // Reddit Tests
    #[tokio::test]
    async fn test_reddit_client_creation() {
        let client = RedditClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_reddit_post_conversion() {
        let client = RedditClient::new().unwrap();
        let post = RedditPost {
            id: "abc123".to_string(),
            name: "t3_abc123".to_string(),
            title: Some("Test Post".to_string()),
            selftext: Some("Test content".to_string()),
            body: None,
            author: "testuser".to_string(),
            subreddit: "technology".to_string(),
            subreddit_id: "t5_2qh16".to_string(),
            score: 100,
            num_comments: Some(50),
            created_utc: 1609459200.0,
            permalink: "/r/technology/comments/abc123/test_post/".to_string(),
            url: Some("https://reddit.com/r/technology".to_string()),
            is_self: true,
            domain: Some("self.technology".to_string()),
        };

        let record = client.post_to_record(&post, "post").unwrap();
        assert_eq!(record.source, "reddit");
        assert_eq!(record.record_type, "post");
        assert!(record.embedding.is_some());
        assert_eq!(record.relationships.len(), 2); // author + subreddit
    }

    #[test]
    fn test_reddit_comment_conversion() {
        let client = RedditClient::new().unwrap();
        let comment = RedditPost {
            id: "def456".to_string(),
            name: "t1_def456".to_string(),
            title: None,
            selftext: None,
            body: Some("Test comment body".to_string()),
            author: "commenter".to_string(),
            subreddit: "technology".to_string(),
            subreddit_id: "t5_2qh16".to_string(),
            score: 10,
            num_comments: None,
            created_utc: 1609459200.0,
            permalink: "/r/technology/comments/abc123/test_post/def456/".to_string(),
            url: None,
            is_self: false,
            domain: None,
        };

        let record = client.post_to_record(&comment, "comment").unwrap();
        assert_eq!(record.source, "reddit");
        assert_eq!(record.record_type, "comment");
        assert!(record.embedding.is_some());
    }

    // Integration tests for embeddings
    #[test]
    fn test_embedding_normalization() {
        let embedder = SimpleEmbedder::new(128);
        let embedding = embedder.embed_text("machine learning artificial intelligence");

        assert_eq!(embedding.len(), 128);

        // Check normalization (L2 norm should be ~1.0)
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_timestamp_parsing() {
        // Test Unix timestamp conversion
        let ts = DateTime::from_timestamp(1609459200, 0).unwrap();
        assert_eq!(ts.year(), 2021);
        assert_eq!(ts.month(), 1);

        // Test RFC3339 parsing
        let rfc = DateTime::parse_from_rfc3339("2024-01-01T12:00:00Z").unwrap();
        assert_eq!(rfc.year(), 2024);
    }
}
