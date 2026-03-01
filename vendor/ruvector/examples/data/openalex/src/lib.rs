//! # RuVector OpenAlex Integration
//!
//! Integration with OpenAlex, the open catalog of scholarly works, authors,
//! institutions, and topics. Enables novel discovery through:
//!
//! - **Emerging Field Detection**: Find topic splits/merges as cut boundaries shift
//! - **Cross-Domain Bridges**: Identify connector subgraphs between disciplines
//! - **Funding-to-Output Causality**: Map funder → lab → venue → citation chains
//!
//! ## OpenAlex Data Model
//!
//! OpenAlex provides a rich graph structure:
//! - **Works**: 250M+ scholarly publications
//! - **Authors**: 90M+ researchers with affiliations
//! - **Institutions**: 100K+ universities, labs, companies
//! - **Topics**: Hierarchical concept taxonomy
//! - **Funders**: Research funding organizations
//! - **Sources**: Journals, conferences, repositories
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use ruvector_data_openalex::{OpenAlexClient, FrontierRadar, TopicGraph};
//!
//! // Initialize client
//! let client = OpenAlexClient::new(Some("your-email@example.com"));
//!
//! // Build topic citation graph
//! let graph = TopicGraph::build_from_works(
//!     client.works_by_topic("machine learning", 2020..2024).await?
//! )?;
//!
//! // Detect emerging research frontiers
//! let radar = FrontierRadar::new(graph);
//! let frontiers = radar.detect_emerging_fields(0.3).await?;
//!
//! for frontier in frontiers {
//!     println!("Emerging: {} (coherence shift: {:.2})",
//!              frontier.name, frontier.coherence_delta);
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

pub mod client;
pub mod frontier;
pub mod schema;

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use client::OpenAlexClient;
pub use frontier::{CrossDomainBridge, EmergingFrontier, FrontierRadar};
pub use schema::{
    Author, AuthorPosition, Authorship, Concept, Funder, Institution, Source, Topic, Work,
};

use ruvector_data_framework::{DataRecord, DataSource, FrameworkError, Relationship, Result};

/// OpenAlex-specific error types
#[derive(Error, Debug)]
pub enum OpenAlexError {
    /// API request failed
    #[error("API error: {0}")]
    Api(String),

    /// Rate limit exceeded
    #[error("Rate limit exceeded, retry after {0}s")]
    RateLimited(u64),

    /// Invalid entity ID
    #[error("Invalid OpenAlex ID: {0}")]
    InvalidId(String),

    /// Parsing failed
    #[error("Parse error: {0}")]
    Parse(String),

    /// Network error
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
}

impl From<OpenAlexError> for FrameworkError {
    fn from(e: OpenAlexError) -> Self {
        FrameworkError::Ingestion(e.to_string())
    }
}

/// Configuration for OpenAlex data source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAlexConfig {
    /// API base URL
    pub base_url: String,

    /// Email for polite pool (faster rate limits)
    pub email: Option<String>,

    /// Maximum results per page
    pub per_page: usize,

    /// Enable cursor-based pagination for bulk
    pub use_cursor: bool,

    /// Filter to specific entity types
    pub entity_types: Vec<EntityType>,
}

impl Default for OpenAlexConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.openalex.org".to_string(),
            email: None,
            per_page: 200,
            use_cursor: true,
            entity_types: vec![EntityType::Work],
        }
    }
}

/// OpenAlex entity types
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EntityType {
    /// Scholarly works
    Work,
    /// Authors
    Author,
    /// Institutions
    Institution,
    /// Topics/concepts
    Topic,
    /// Funding sources
    Funder,
    /// Publication venues
    Source,
}

impl EntityType {
    /// Get the API endpoint for this entity type
    pub fn endpoint(&self) -> &str {
        match self {
            EntityType::Work => "works",
            EntityType::Author => "authors",
            EntityType::Institution => "institutions",
            EntityType::Topic => "topics",
            EntityType::Funder => "funders",
            EntityType::Source => "sources",
        }
    }
}

/// OpenAlex data source for the framework
pub struct OpenAlexSource {
    client: OpenAlexClient,
    config: OpenAlexConfig,
    filters: HashMap<String, String>,
}

impl OpenAlexSource {
    /// Create a new OpenAlex data source
    pub fn new(config: OpenAlexConfig) -> Self {
        let client = OpenAlexClient::new(config.email.clone());
        Self {
            client,
            config,
            filters: HashMap::new(),
        }
    }

    /// Add a filter (e.g., "publication_year" => "2023")
    pub fn with_filter(mut self, key: &str, value: &str) -> Self {
        self.filters.insert(key.to_string(), value.to_string());
        self
    }

    /// Filter to a specific year range
    pub fn with_year_range(self, start: i32, end: i32) -> Self {
        self.with_filter("publication_year", &format!("{}-{}", start, end))
    }

    /// Filter to a specific topic
    pub fn with_topic(self, topic_id: &str) -> Self {
        self.with_filter("primary_topic.id", topic_id)
    }

    /// Filter to open access works
    pub fn open_access_only(self) -> Self {
        self.with_filter("open_access.is_oa", "true")
    }
}

#[async_trait]
impl DataSource for OpenAlexSource {
    fn source_id(&self) -> &str {
        "openalex"
    }

    async fn fetch_batch(
        &self,
        cursor: Option<String>,
        batch_size: usize,
    ) -> Result<(Vec<DataRecord>, Option<String>)> {
        // Build query URL with filters
        let mut query_parts: Vec<String> = self
            .filters
            .iter()
            .map(|(k, v)| format!("{}:{}", k, v))
            .collect();

        let filter_str = if query_parts.is_empty() {
            String::new()
        } else {
            format!("filter={}", query_parts.join(","))
        };

        // Fetch works from API
        let (works, next_cursor) = self
            .client
            .fetch_works_page(&filter_str, cursor, batch_size.min(self.config.per_page))
            .await
            .map_err(|e| FrameworkError::Ingestion(e.to_string()))?;

        // Convert to DataRecords
        let records: Vec<DataRecord> = works.into_iter().map(work_to_record).collect();

        Ok((records, next_cursor))
    }

    async fn total_count(&self) -> Result<Option<u64>> {
        // OpenAlex returns count in meta
        Ok(None) // Would require separate API call
    }

    async fn health_check(&self) -> Result<bool> {
        self.client.health_check().await.map_err(|e| e.into())
    }
}

/// Convert an OpenAlex Work to a DataRecord
fn work_to_record(work: Work) -> DataRecord {
    let mut relationships = Vec::new();

    // Citations as relationships
    for cited_id in &work.referenced_works {
        relationships.push(Relationship {
            target_id: cited_id.clone(),
            rel_type: "cites".to_string(),
            weight: 1.0,
            properties: HashMap::new(),
        });
    }

    // Author relationships
    for authorship in &work.authorships {
        relationships.push(Relationship {
            target_id: authorship.author.id.clone(),
            rel_type: "authored_by".to_string(),
            weight: 1.0 / work.authorships.len() as f64,
            properties: HashMap::new(),
        });

        // Institution relationships
        for inst in &authorship.institutions {
            relationships.push(Relationship {
                target_id: inst.id.clone(),
                rel_type: "affiliated_with".to_string(),
                weight: 0.5,
                properties: HashMap::new(),
            });
        }
    }

    // Topic relationships
    if let Some(ref topic) = work.primary_topic {
        relationships.push(Relationship {
            target_id: topic.id.clone(),
            rel_type: "primary_topic".to_string(),
            weight: topic.score,
            properties: HashMap::new(),
        });
    }

    DataRecord {
        id: work.id.clone(),
        source: "openalex".to_string(),
        record_type: "work".to_string(),
        timestamp: work.publication_date.unwrap_or_else(Utc::now),
        data: serde_json::to_value(&work).unwrap_or_default(),
        embedding: None, // Would compute from title/abstract
        relationships,
    }
}

/// Topic-based citation graph for frontier detection
pub struct TopicGraph {
    /// Topics as nodes
    pub topics: HashMap<String, TopicNode>,

    /// Topic-to-topic edges (via citations)
    pub edges: Vec<TopicEdge>,

    /// Time window
    pub time_window: (DateTime<Utc>, DateTime<Utc>),
}

/// A topic node in the graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicNode {
    /// OpenAlex topic ID
    pub id: String,

    /// Topic display name
    pub name: String,

    /// Number of works in this topic
    pub work_count: usize,

    /// Average citation count
    pub avg_citations: f64,

    /// Growth rate (works per year)
    pub growth_rate: f64,
}

/// An edge between topics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicEdge {
    /// Source topic ID
    pub source: String,

    /// Target topic ID
    pub target: String,

    /// Number of citations across boundary
    pub citation_count: usize,

    /// Normalized weight
    pub weight: f64,
}

impl TopicGraph {
    /// Build topic graph from works
    pub fn from_works(works: &[Work]) -> Self {
        let mut topics: HashMap<String, TopicNode> = HashMap::new();
        let mut edge_counts: HashMap<(String, String), usize> = HashMap::new();

        let mut min_date = Utc::now();
        let mut max_date = DateTime::<Utc>::MIN_UTC;

        for work in works {
            if let Some(date) = work.publication_date {
                if date < min_date {
                    min_date = date;
                }
                if date > max_date {
                    max_date = date;
                }
            }

            // Get work's primary topic
            let source_topic = match &work.primary_topic {
                Some(t) => t.id.clone(),
                None => continue,
            };

            // Update or create topic node
            let node = topics.entry(source_topic.clone()).or_insert_with(|| TopicNode {
                id: source_topic.clone(),
                name: work
                    .primary_topic
                    .as_ref()
                    .map(|t| t.display_name.clone())
                    .unwrap_or_default(),
                work_count: 0,
                avg_citations: 0.0,
                growth_rate: 0.0,
            });
            node.work_count += 1;
            node.avg_citations = (node.avg_citations * (node.work_count - 1) as f64
                + work.cited_by_count as f64)
                / node.work_count as f64;

            // For simplicity, we'd need referenced works' topics
            // This is a simplified model
        }

        // Calculate growth rates
        let time_span_years = (max_date - min_date).num_days() as f64 / 365.0;
        for node in topics.values_mut() {
            node.growth_rate = if time_span_years > 0.0 {
                node.work_count as f64 / time_span_years
            } else {
                0.0
            };
        }

        // Build edges
        let edges: Vec<TopicEdge> = edge_counts
            .into_iter()
            .map(|((src, tgt), count)| {
                let src_count = topics.get(&src).map(|n| n.work_count).unwrap_or(1);
                let tgt_count = topics.get(&tgt).map(|n| n.work_count).unwrap_or(1);
                let weight = count as f64 / (src_count * tgt_count) as f64;

                TopicEdge {
                    source: src,
                    target: tgt,
                    citation_count: count,
                    weight,
                }
            })
            .collect();

        Self {
            topics,
            edges,
            time_window: (min_date, max_date),
        }
    }

    /// Get number of topics
    pub fn topic_count(&self) -> usize {
        self.topics.len()
    }

    /// Get number of edges
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Get topics by growth rate
    pub fn fastest_growing(&self, top_k: usize) -> Vec<&TopicNode> {
        let mut nodes: Vec<_> = self.topics.values().collect();
        nodes.sort_by(|a, b| {
            b.growth_rate
                .partial_cmp(&a.growth_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        nodes.into_iter().take(top_k).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_entity_endpoints() {
        assert_eq!(EntityType::Work.endpoint(), "works");
        assert_eq!(EntityType::Author.endpoint(), "authors");
        assert_eq!(EntityType::Topic.endpoint(), "topics");
    }

    #[test]
    fn test_default_config() {
        let config = OpenAlexConfig::default();
        assert_eq!(config.base_url, "https://api.openalex.org");
        assert!(config.use_cursor);
    }

    #[test]
    fn test_source_with_filters() {
        let config = OpenAlexConfig::default();
        let source = OpenAlexSource::new(config)
            .with_year_range(2020, 2024)
            .open_access_only();

        assert!(source.filters.contains_key("publication_year"));
        assert!(source.filters.contains_key("open_access.is_oa"));
    }
}
