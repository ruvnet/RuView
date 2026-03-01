//! Real-Time Data Feed Integration
//!
//! RSS/Atom feed parsing, WebSocket streaming, and REST API polling
//! for continuous data ingestion into RuVector discovery framework.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tokio::time::interval;

use crate::ruvector_native::{Domain, SemanticVector};
use crate::{FrameworkError, Result};

/// Real-time engine for streaming data feeds
pub struct RealTimeEngine {
    feeds: Vec<FeedSource>,
    update_interval: Duration,
    on_new_data: Option<Arc<dyn Fn(Vec<SemanticVector>) + Send + Sync>>,
    dedup_cache: Arc<RwLock<HashSet<String>>>,
    running: Arc<RwLock<bool>>,
}

/// Types of feed sources
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FeedSource {
    /// RSS or Atom feed
    Rss { url: String, category: String },
    /// REST API with polling
    RestPolling { url: String, interval: Duration },
    /// WebSocket streaming endpoint
    WebSocket { url: String },
}

/// News aggregator for multiple RSS feeds
pub struct NewsAggregator {
    sources: Vec<NewsSource>,
    client: reqwest::Client,
}

/// Individual news source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsSource {
    pub name: String,
    pub feed_url: String,
    pub domain: Domain,
}

/// Parsed feed item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedItem {
    pub id: String,
    pub title: String,
    pub description: String,
    pub link: String,
    pub published: Option<chrono::DateTime<Utc>>,
    pub author: Option<String>,
    pub categories: Vec<String>,
}

impl RealTimeEngine {
    /// Create a new real-time engine
    pub fn new(update_interval: Duration) -> Self {
        Self {
            feeds: Vec::new(),
            update_interval,
            on_new_data: None,
            dedup_cache: Arc::new(RwLock::new(HashSet::new())),
            running: Arc::new(RwLock::new(false)),
        }
    }

    /// Add a feed source to monitor
    pub fn add_feed(&mut self, source: FeedSource) {
        self.feeds.push(source);
    }

    /// Set callback for new data
    pub fn set_callback<F>(&mut self, callback: F)
    where
        F: Fn(Vec<SemanticVector>) + Send + Sync + 'static,
    {
        self.on_new_data = Some(Arc::new(callback));
    }

    /// Start the real-time engine
    pub async fn start(&mut self) -> Result<()> {
        {
            let mut running = self.running.write().await;
            if *running {
                return Err(FrameworkError::Config(
                    "Engine already running".to_string(),
                ));
            }
            *running = true;
        }

        let feeds = self.feeds.clone();
        let callback = self.on_new_data.clone();
        let dedup_cache = self.dedup_cache.clone();
        let update_interval = self.update_interval;
        let running = self.running.clone();

        tokio::spawn(async move {
            let mut ticker = interval(update_interval);

            loop {
                ticker.tick().await;

                // Check if we should stop
                {
                    let is_running = running.read().await;
                    if !*is_running {
                        break;
                    }
                }

                // Process all feeds
                for feed in &feeds {
                    match Self::process_feed(feed, &dedup_cache).await {
                        Ok(vectors) => {
                            if !vectors.is_empty() {
                                if let Some(ref cb) = callback {
                                    cb(vectors);
                                }
                            }
                        }
                        Err(e) => {
                            tracing::error!("Feed processing error: {}", e);
                        }
                    }
                }
            }
        });

        Ok(())
    }

    /// Stop the real-time engine
    pub async fn stop(&mut self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Process a single feed source
    async fn process_feed(
        feed: &FeedSource,
        dedup_cache: &Arc<RwLock<HashSet<String>>>,
    ) -> Result<Vec<SemanticVector>> {
        match feed {
            FeedSource::Rss { url, category } => {
                Self::process_rss_feed(url, category, dedup_cache).await
            }
            FeedSource::RestPolling { url, .. } => {
                Self::process_rest_feed(url, dedup_cache).await
            }
            FeedSource::WebSocket { url } => Self::process_websocket_feed(url, dedup_cache).await,
        }
    }

    /// Process RSS/Atom feed
    async fn process_rss_feed(
        url: &str,
        category: &str,
        dedup_cache: &Arc<RwLock<HashSet<String>>>,
    ) -> Result<Vec<SemanticVector>> {
        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;
        let content = response.text().await?;

        // Parse RSS/Atom feed
        let items = Self::parse_rss(&content)?;

        let mut vectors = Vec::new();
        let mut cache = dedup_cache.write().await;

        for item in items {
            // Check for duplicates
            if cache.contains(&item.id) {
                continue;
            }

            // Add to dedup cache
            cache.insert(item.id.clone());

            // Convert to SemanticVector
            let domain = Self::category_to_domain(category);
            let vector = Self::item_to_vector(item, domain);
            vectors.push(vector);
        }

        Ok(vectors)
    }

    /// Process REST API polling
    async fn process_rest_feed(
        url: &str,
        dedup_cache: &Arc<RwLock<HashSet<String>>>,
    ) -> Result<Vec<SemanticVector>> {
        let client = reqwest::Client::new();
        let response = client.get(url).send().await?;
        let items: Vec<FeedItem> = response.json().await?;

        let mut vectors = Vec::new();
        let mut cache = dedup_cache.write().await;

        for item in items {
            if cache.contains(&item.id) {
                continue;
            }

            cache.insert(item.id.clone());
            let vector = Self::item_to_vector(item, Domain::Research);
            vectors.push(vector);
        }

        Ok(vectors)
    }

    /// Process WebSocket stream (simplified implementation)
    async fn process_websocket_feed(
        _url: &str,
        _dedup_cache: &Arc<RwLock<HashSet<String>>>,
    ) -> Result<Vec<SemanticVector>> {
        // WebSocket implementation would require tokio-tungstenite
        // For now, return empty - can be extended with actual WebSocket client
        tracing::warn!("WebSocket feeds not yet implemented");
        Ok(Vec::new())
    }

    /// Parse RSS/Atom XML into feed items
    fn parse_rss(content: &str) -> Result<Vec<FeedItem>> {
        // Simple XML parsing for RSS 2.0
        // In production, use feed-rs or rss crate
        let mut items = Vec::new();

        // Basic RSS parsing (simplified)
        for item_block in content.split("<item>").skip(1) {
            if let Some(end) = item_block.find("</item>") {
                let item_xml = &item_block[..end];
                if let Some(item) = Self::parse_rss_item(item_xml) {
                    items.push(item);
                }
            }
        }

        Ok(items)
    }

    /// Parse a single RSS item from XML
    fn parse_rss_item(xml: &str) -> Option<FeedItem> {
        let title = Self::extract_tag(xml, "title")?;
        let description = Self::extract_tag(xml, "description").unwrap_or_default();
        let link = Self::extract_tag(xml, "link").unwrap_or_default();
        let guid = Self::extract_tag(xml, "guid").unwrap_or_else(|| link.clone());

        let published = Self::extract_tag(xml, "pubDate")
            .and_then(|date_str| chrono::DateTime::parse_from_rfc2822(&date_str).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let author = Self::extract_tag(xml, "author");

        Some(FeedItem {
            id: guid,
            title,
            description,
            link,
            published,
            author,
            categories: Vec::new(),
        })
    }

    /// Extract content between XML tags
    fn extract_tag(xml: &str, tag: &str) -> Option<String> {
        let start_tag = format!("<{}>", tag);
        let end_tag = format!("</{}>", tag);

        let start = xml.find(&start_tag)? + start_tag.len();
        let end = xml.find(&end_tag)?;

        if start < end {
            let content = &xml[start..end];
            // Basic HTML entity decoding
            let decoded = content
                .replace("&lt;", "<")
                .replace("&gt;", ">")
                .replace("&amp;", "&")
                .replace("&quot;", "\"")
                .replace("&#39;", "'");
            Some(decoded.trim().to_string())
        } else {
            None
        }
    }

    /// Convert category string to Domain enum
    fn category_to_domain(category: &str) -> Domain {
        match category.to_lowercase().as_str() {
            "climate" | "weather" | "environment" => Domain::Climate,
            "finance" | "economy" | "market" | "stock" => Domain::Finance,
            "research" | "science" | "academic" | "medical" => Domain::Research,
            _ => Domain::CrossDomain,
        }
    }

    /// Convert FeedItem to SemanticVector
    fn item_to_vector(item: FeedItem, domain: Domain) -> SemanticVector {
        use std::collections::HashMap;

        // Create a simple embedding from title + description
        // In production, use actual embedding model
        let text = format!("{} {}", item.title, item.description);
        let embedding = Self::simple_embedding(&text);

        let mut metadata = HashMap::new();
        metadata.insert("title".to_string(), item.title.clone());
        metadata.insert("link".to_string(), item.link.clone());
        if let Some(author) = item.author {
            metadata.insert("author".to_string(), author);
        }

        SemanticVector {
            id: item.id,
            embedding,
            domain,
            timestamp: item.published.unwrap_or_else(Utc::now),
            metadata,
        }
    }

    /// Simple embedding generation (hash-based for demo)
    fn simple_embedding(text: &str) -> Vec<f32> {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Create 384-dimensional embedding from text hash
        let mut embedding = vec![0.0f32; 384];

        for (i, word) in text.split_whitespace().take(384).enumerate() {
            let mut hasher = DefaultHasher::new();
            word.hash(&mut hasher);
            let hash = hasher.finish();
            embedding[i] = (hash as f32 / u64::MAX as f32) * 2.0 - 1.0;
        }

        // Normalize
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        if magnitude > 0.0 {
            for val in &mut embedding {
                *val /= magnitude;
            }
        }

        embedding
    }
}

impl NewsAggregator {
    /// Create a new news aggregator
    pub fn new() -> Self {
        Self {
            sources: Vec::new(),
            client: reqwest::Client::builder()
                .user_agent("RuVector/1.0")
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap(),
        }
    }

    /// Add a news source
    pub fn add_source(&mut self, source: NewsSource) {
        self.sources.push(source);
    }

    /// Add default free news sources
    pub fn add_default_sources(&mut self) {
        // Climate sources
        self.add_source(NewsSource {
            name: "NASA Earth Observatory".to_string(),
            feed_url: "https://earthobservatory.nasa.gov/feeds/image-of-the-day.rss".to_string(),
            domain: Domain::Climate,
        });

        // Financial sources
        self.add_source(NewsSource {
            name: "Yahoo Finance - Top Stories".to_string(),
            feed_url: "https://finance.yahoo.com/news/rssindex".to_string(),
            domain: Domain::Finance,
        });

        // Medical/Research sources
        self.add_source(NewsSource {
            name: "PubMed Recent".to_string(),
            feed_url: "https://pubmed.ncbi.nlm.nih.gov/rss/search/1nKx2zx8g-9UCGpQD5qVmN6jTvSRRxYqjD3T_nA-pSMjDlXr4u/?limit=100&utm_campaign=pubmed-2&fc=20210421200858".to_string(),
            domain: Domain::Research,
        });

        // General news sources
        self.add_source(NewsSource {
            name: "Reuters Top News".to_string(),
            feed_url: "https://www.reutersagency.com/feed/?taxonomy=best-topics&post_type=best".to_string(),
            domain: Domain::CrossDomain,
        });

        self.add_source(NewsSource {
            name: "AP News Top Stories".to_string(),
            feed_url: "https://apnews.com/index.rss".to_string(),
            domain: Domain::CrossDomain,
        });
    }

    /// Fetch latest items from all sources
    pub async fn fetch_latest(&self, limit: usize) -> Result<Vec<SemanticVector>> {
        let mut all_vectors = Vec::new();
        let mut seen = HashSet::new();

        for source in &self.sources {
            match self.fetch_source(source, limit).await {
                Ok(vectors) => {
                    for vector in vectors {
                        if !seen.contains(&vector.id) {
                            seen.insert(vector.id.clone());
                            all_vectors.push(vector);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch {}: {}", source.name, e);
                }
            }
        }

        // Sort by timestamp, most recent first
        all_vectors.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Limit results
        all_vectors.truncate(limit);

        Ok(all_vectors)
    }

    /// Fetch from a single source
    async fn fetch_source(&self, source: &NewsSource, limit: usize) -> Result<Vec<SemanticVector>> {
        let response = self.client.get(&source.feed_url).send().await?;
        let content = response.text().await?;

        let items = RealTimeEngine::parse_rss(&content)?;
        let mut vectors = Vec::new();

        for item in items.into_iter().take(limit) {
            let vector = RealTimeEngine::item_to_vector(item, source.domain);
            vectors.push(vector);
        }

        Ok(vectors)
    }
}

impl Default for NewsAggregator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_tag() {
        let xml = "<title>Test Title</title><description>Test Description</description>";
        assert_eq!(
            RealTimeEngine::extract_tag(xml, "title"),
            Some("Test Title".to_string())
        );
        assert_eq!(
            RealTimeEngine::extract_tag(xml, "description"),
            Some("Test Description".to_string())
        );
        assert_eq!(RealTimeEngine::extract_tag(xml, "missing"), None);
    }

    #[test]
    fn test_category_to_domain() {
        assert_eq!(
            RealTimeEngine::category_to_domain("climate"),
            Domain::Climate
        );
        assert_eq!(
            RealTimeEngine::category_to_domain("Finance"),
            Domain::Finance
        );
        assert_eq!(
            RealTimeEngine::category_to_domain("research"),
            Domain::Research
        );
        assert_eq!(
            RealTimeEngine::category_to_domain("other"),
            Domain::CrossDomain
        );
    }

    #[test]
    fn test_simple_embedding() {
        let embedding = RealTimeEngine::simple_embedding("climate change impacts");
        assert_eq!(embedding.len(), 384);

        // Check normalization
        let magnitude: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((magnitude - 1.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_realtime_engine_lifecycle() {
        let mut engine = RealTimeEngine::new(Duration::from_secs(1));

        engine.add_feed(FeedSource::Rss {
            url: "https://example.com/feed.rss".to_string(),
            category: "climate".to_string(),
        });

        // Start and stop
        assert!(engine.start().await.is_ok());
        engine.stop().await;
    }

    #[test]
    fn test_news_aggregator() {
        let mut aggregator = NewsAggregator::new();
        aggregator.add_default_sources();
        assert!(aggregator.sources.len() >= 5);
    }

    #[test]
    fn test_parse_rss_item() {
        let xml = r#"
            <title>Test Article</title>
            <description>This is a test article</description>
            <link>https://example.com/article</link>
            <guid>article-123</guid>
            <pubDate>Mon, 01 Jan 2024 12:00:00 GMT</pubDate>
        "#;

        let item = RealTimeEngine::parse_rss_item(xml);
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item.title, "Test Article");
        assert_eq!(item.description, "This is a test article");
        assert_eq!(item.link, "https://example.com/article");
        assert_eq!(item.id, "article-123");
    }
}
