//! Policy Memory Store
//!
//! Stores learned policies and thresholds in Ruvector for semantic search
//! and retrieval. Policies inform runtime decisions like quantization
//! thresholds, router weights, and EWC parameters.
//!
//! ## Policy Types
//!
//! - **Quantization**: Dynamic precision selection based on context
//! - **Router**: FastGRNN router weights and biases
//! - **EWC**: Elastic Weight Consolidation parameters
//! - **Pattern**: Learned patterns from ReasoningBank

use crate::error::{Result, RuvLLMError};
use chrono::{DateTime, Utc};
use ruvector_core::types::DbOptions;
use ruvector_core::{AgenticDB, SearchQuery, VectorEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Policy type enumeration
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyType {
    /// Quantization threshold policy
    Quantization,
    /// Router weight policy
    Router,
    /// EWC++ parameters
    Ewc,
    /// Learned pattern
    Pattern,
}

impl PolicyType {
    /// Convert to string tag
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Quantization => "quantization",
            Self::Router => "router",
            Self::Ewc => "ewc",
            Self::Pattern => "pattern",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "quantization" => Some(Self::Quantization),
            "router" => Some(Self::Router),
            "ewc" => Some(Self::Ewc),
            "pattern" => Some(Self::Pattern),
            _ => None,
        }
    }
}

/// Policy entry stored in Ruvector
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyEntry {
    /// Unique identifier
    pub id: Uuid,
    /// Policy type
    pub policy_type: PolicyType,
    /// Embedding vector for semantic search (768-D)
    pub embedding: Vec<f32>,
    /// Policy parameters as JSON
    pub parameters: serde_json::Value,
    /// Confidence score from learning (0.0 - 1.0)
    pub confidence: f32,
    /// Fisher information diagonal (for EWC++ policies)
    pub fisher_diagonal: Option<Vec<f32>>,
    /// Creation timestamp
    pub created_at: DateTime<Utc>,
    /// Last accessed timestamp
    pub last_accessed: DateTime<Utc>,
    /// Source of the policy
    pub source: PolicySource,
    /// Additional tags
    pub tags: Vec<String>,
}

/// Source of a policy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicySource {
    /// From instant (per-request) learning loop
    InstantLoop,
    /// From background (hourly) learning loop
    BackgroundLoop,
    /// From deep (weekly) learning loop
    DeepLoop,
    /// From federated learning
    Federated,
    /// Manually configured
    Manual,
}

impl PolicySource {
    /// Convert to string
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::InstantLoop => "instant_loop",
            Self::BackgroundLoop => "background_loop",
            Self::DeepLoop => "deep_loop",
            Self::Federated => "federated",
            Self::Manual => "manual",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Self {
        match s {
            "instant_loop" => Self::InstantLoop,
            "background_loop" => Self::BackgroundLoop,
            "deep_loop" => Self::DeepLoop,
            "federated" => Self::Federated,
            _ => Self::Manual,
        }
    }
}

/// Quantization threshold policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantizationPolicy {
    /// Layer indices affected (start, end)
    pub layer_range: (usize, usize),
    /// Precision level
    pub precision: String,
    /// Activation threshold triggering this precision
    pub activation_threshold: f32,
    /// Memory budget constraint (bytes)
    pub memory_budget: usize,
    /// Learned quality-latency tradeoff weight
    pub quality_weight: f32,
}

impl Default for QuantizationPolicy {
    fn default() -> Self {
        Self {
            layer_range: (0, 32),
            precision: "q4_k".to_string(),
            activation_threshold: 0.5,
            memory_budget: 1024 * 1024 * 1024, // 1GB
            quality_weight: 0.7,
        }
    }
}

/// Router weight policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterPolicy {
    /// Cell weights (flattened)
    pub cell_weights: Vec<f32>,
    /// Head biases
    pub head_biases: Vec<f32>,
    /// EWC regularization strength
    pub ewc_lambda: f32,
    /// Training loss at checkpoint
    pub training_loss: f32,
    /// Learning rate used
    pub learning_rate: f32,
}

impl Default for RouterPolicy {
    fn default() -> Self {
        Self {
            cell_weights: vec![0.0; 128 * 128], // Placeholder
            head_biases: vec![0.0; 4],          // 4 model sizes
            ewc_lambda: 0.1,
            training_loss: 0.0,
            learning_rate: 0.001,
        }
    }
}

/// Policy store backed by Ruvector
pub struct PolicyStore {
    /// Ruvector database
    db: AgenticDB,
    /// Embedding dimension
    embedding_dim: usize,
    /// In-memory cache for fast access
    cache: dashmap::DashMap<Uuid, PolicyEntry>,
}

impl PolicyStore {
    /// Create a new policy store
    pub fn new(storage_path: &str, embedding_dim: usize) -> Result<Self> {
        let mut options = DbOptions::default();
        options.storage_path = storage_path.to_string();
        options.dimensions = embedding_dim;

        let db = AgenticDB::new(options).map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        Ok(Self {
            db,
            embedding_dim,
            cache: dashmap::DashMap::new(),
        })
    }

    /// Store a policy entry
    pub fn store(&self, entry: PolicyEntry) -> Result<Uuid> {
        let id = entry.id;

        // Create metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "policy_type".to_string(),
            serde_json::json!(entry.policy_type.as_str()),
        );
        metadata.insert(
            "confidence".to_string(),
            serde_json::json!(entry.confidence),
        );
        metadata.insert(
            "source".to_string(),
            serde_json::json!(entry.source.as_str()),
        );
        metadata.insert("parameters".to_string(), entry.parameters.clone());
        metadata.insert(
            "created_at".to_string(),
            serde_json::json!(entry.created_at.to_rfc3339()),
        );
        metadata.insert("tags".to_string(), serde_json::json!(entry.tags));

        if let Some(ref fisher) = entry.fisher_diagonal {
            metadata.insert("fisher_diagonal".to_string(), serde_json::json!(fisher));
        }

        // Create vector entry
        let vector_entry = VectorEntry {
            id: Some(id.to_string()),
            vector: entry.embedding.clone(),
            metadata: Some(metadata),
        };

        // Store in Ruvector
        self.db
            .insert(vector_entry)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        // Update cache
        self.cache.insert(id, entry);

        Ok(id)
    }

    /// Search for policies by semantic similarity
    pub fn search(&self, query_embedding: &[f32], limit: usize) -> Result<Vec<PolicyEntry>> {
        let query = SearchQuery {
            vector: query_embedding.to_vec(),
            k: limit,
            filter: None,
            ef_search: None,
        };

        let results = self
            .db
            .search(query)
            .map_err(|e| RuvLLMError::Storage(e.to_string()))?;

        let mut entries = Vec::with_capacity(results.len());

        for result in results {
            if let Some(metadata) = &result.metadata {
                if let Some(entry) = self.entry_from_metadata(&result.id, query_embedding, metadata)
                {
                    entries.push(entry);
                }
            }
        }

        Ok(entries)
    }

    /// Get policy by ID
    pub fn get(&self, id: &Uuid) -> Option<PolicyEntry> {
        // Check cache first
        if let Some(entry) = self.cache.get(id) {
            return Some(entry.clone());
        }
        None
    }

    /// Search by policy type
    pub fn search_by_type(&self, policy_type: &PolicyType, limit: usize) -> Vec<PolicyEntry> {
        self.cache
            .iter()
            .filter(|e| &e.policy_type == policy_type)
            .map(|e| e.clone())
            .take(limit)
            .collect()
    }

    /// Delete a policy
    pub fn delete(&self, id: &Uuid) {
        self.cache.remove(id);
    }

    /// Store a quantization policy
    pub fn store_quantization_policy(
        &self,
        embedding: Vec<f32>,
        policy: QuantizationPolicy,
        confidence: f32,
        source: PolicySource,
    ) -> Result<Uuid> {
        let entry = PolicyEntry {
            id: Uuid::new_v4(),
            policy_type: PolicyType::Quantization,
            embedding,
            parameters: serde_json::to_value(&policy)?,
            confidence,
            fisher_diagonal: None,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            source,
            tags: vec!["quantization".to_string()],
        };

        self.store(entry)
    }

    /// Store a router policy
    pub fn store_router_policy(
        &self,
        embedding: Vec<f32>,
        policy: RouterPolicy,
        confidence: f32,
        source: PolicySource,
    ) -> Result<Uuid> {
        let entry = PolicyEntry {
            id: Uuid::new_v4(),
            policy_type: PolicyType::Router,
            embedding,
            parameters: serde_json::to_value(&policy)?,
            confidence,
            fisher_diagonal: None,
            created_at: Utc::now(),
            last_accessed: Utc::now(),
            source,
            tags: vec!["router".to_string()],
        };

        self.store(entry)
    }

    /// Get statistics
    pub fn stats(&self) -> PolicyStoreStats {
        PolicyStoreStats {
            total_policies: self.cache.len(),
            quantization_policies: self
                .cache
                .iter()
                .filter(|e| e.policy_type == PolicyType::Quantization)
                .count(),
            router_policies: self
                .cache
                .iter()
                .filter(|e| e.policy_type == PolicyType::Router)
                .count(),
            ewc_policies: self
                .cache
                .iter()
                .filter(|e| e.policy_type == PolicyType::Ewc)
                .count(),
            pattern_policies: self
                .cache
                .iter()
                .filter(|e| e.policy_type == PolicyType::Pattern)
                .count(),
        }
    }

    /// Reconstruct PolicyEntry from metadata
    fn entry_from_metadata(
        &self,
        id: &str,
        embedding: &[f32],
        metadata: &HashMap<String, serde_json::Value>,
    ) -> Option<PolicyEntry> {
        let uuid = Uuid::parse_str(id).ok()?;
        let policy_type_str = metadata.get("policy_type")?.as_str()?;
        let policy_type = PolicyType::from_str(policy_type_str)?;

        let confidence = metadata.get("confidence")?.as_f64()? as f32;
        let source_str = metadata.get("source")?.as_str()?;
        let source = PolicySource::from_str(source_str);

        let parameters = metadata.get("parameters")?.clone();
        let created_at_str = metadata.get("created_at")?.as_str()?;
        let created_at = DateTime::parse_from_rfc3339(created_at_str)
            .ok()?
            .with_timezone(&Utc);

        let tags: Vec<String> = metadata
            .get("tags")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let fisher_diagonal: Option<Vec<f32>> = metadata
            .get("fisher_diagonal")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_f64().map(|f| f as f32))
                    .collect()
            });

        Some(PolicyEntry {
            id: uuid,
            policy_type,
            embedding: embedding.to_vec(),
            parameters,
            confidence,
            fisher_diagonal,
            created_at,
            last_accessed: Utc::now(),
            source,
            tags,
        })
    }
}

/// Policy store statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicyStoreStats {
    /// Total number of policies
    pub total_policies: usize,
    /// Number of quantization policies
    pub quantization_policies: usize,
    /// Number of router policies
    pub router_policies: usize,
    /// Number of EWC policies
    pub ewc_policies: usize,
    /// Number of pattern policies
    pub pattern_policies: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_type() {
        assert_eq!(PolicyType::Quantization.as_str(), "quantization");
        assert_eq!(PolicyType::Router.as_str(), "router");
        assert_eq!(
            PolicyType::from_str("quantization"),
            Some(PolicyType::Quantization)
        );
    }

    #[test]
    fn test_quantization_policy_default() {
        let policy = QuantizationPolicy::default();
        assert_eq!(policy.precision, "q4_k");
        assert_eq!(policy.quality_weight, 0.7);
    }

    #[test]
    fn test_router_policy_default() {
        let policy = RouterPolicy::default();
        assert_eq!(policy.head_biases.len(), 4);
        assert_eq!(policy.ewc_lambda, 0.1);
    }
}
