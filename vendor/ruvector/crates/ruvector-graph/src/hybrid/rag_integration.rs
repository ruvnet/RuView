//! RAG (Retrieval Augmented Generation) integration
//!
//! Provides graph-based context retrieval and multi-hop reasoning for LLMs.

use crate::error::{GraphError, Result};
use crate::hybrid::semantic_search::{SemanticPath, SemanticSearch};
use crate::types::{EdgeId, NodeId, Properties};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Configuration for RAG engine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagConfig {
    /// Maximum context size (in tokens)
    pub max_context_tokens: usize,
    /// Number of top documents to retrieve
    pub top_k_docs: usize,
    /// Maximum reasoning depth (hops in graph)
    pub max_reasoning_depth: usize,
    /// Minimum relevance score
    pub min_relevance: f32,
    /// Enable multi-hop reasoning
    pub multi_hop_reasoning: bool,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 4096,
            top_k_docs: 5,
            max_reasoning_depth: 3,
            min_relevance: 0.7,
            multi_hop_reasoning: true,
        }
    }
}

/// RAG engine for graph-based retrieval
pub struct RagEngine {
    /// Semantic search engine
    semantic_search: SemanticSearch,
    /// Configuration
    config: RagConfig,
}

impl RagEngine {
    /// Create a new RAG engine
    pub fn new(semantic_search: SemanticSearch, config: RagConfig) -> Self {
        Self {
            semantic_search,
            config,
        }
    }

    /// Retrieve relevant context for a query
    pub fn retrieve_context(&self, query: &[f32]) -> Result<Context> {
        // Find top-k most relevant documents
        let matches = self
            .semantic_search
            .find_similar_nodes(query, self.config.top_k_docs)?;

        let mut documents = Vec::new();
        for match_result in matches {
            if match_result.score >= self.config.min_relevance {
                documents.push(Document {
                    node_id: match_result.node_id.clone(),
                    content: format!("Document {}", match_result.node_id),
                    metadata: HashMap::new(),
                    relevance_score: match_result.score,
                });
            }
        }

        let total_tokens = self.estimate_tokens(&documents);

        Ok(Context {
            documents,
            total_tokens,
            query_embedding: query.to_vec(),
        })
    }

    /// Build multi-hop reasoning paths
    pub fn build_reasoning_paths(
        &self,
        start_node: &NodeId,
        query: &[f32],
    ) -> Result<Vec<ReasoningPath>> {
        if !self.config.multi_hop_reasoning {
            return Ok(Vec::new());
        }

        // Find semantic paths through the graph
        let semantic_paths =
            self.semantic_search
                .find_semantic_paths(start_node, query, self.config.top_k_docs)?;

        // Convert semantic paths to reasoning paths
        let reasoning_paths = semantic_paths
            .into_iter()
            .map(|path| self.convert_to_reasoning_path(path))
            .collect();

        Ok(reasoning_paths)
    }

    /// Aggregate evidence from multiple sources
    pub fn aggregate_evidence(&self, paths: &[ReasoningPath]) -> Result<Vec<Evidence>> {
        let mut evidence_map: HashMap<NodeId, Evidence> = HashMap::new();

        for path in paths {
            for step in &path.steps {
                evidence_map
                    .entry(step.node_id.clone())
                    .and_modify(|e| {
                        e.support_count += 1;
                        e.confidence = e.confidence.max(step.confidence);
                    })
                    .or_insert_with(|| Evidence {
                        node_id: step.node_id.clone(),
                        content: step.content.clone(),
                        support_count: 1,
                        confidence: step.confidence,
                        sources: vec![step.node_id.clone()],
                    });
            }
        }

        let mut evidence: Vec<_> = evidence_map.into_values().collect();
        evidence.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(evidence)
    }

    /// Generate context-aware prompt
    pub fn generate_prompt(&self, query: &str, context: &Context) -> String {
        let mut prompt = String::new();

        prompt.push_str("Based on the following context, answer the question.\n\n");
        prompt.push_str("Context:\n");

        for (i, doc) in context.documents.iter().enumerate() {
            prompt.push_str(&format!(
                "{}. {} (relevance: {:.2})\n",
                i + 1,
                doc.content,
                doc.relevance_score
            ));
        }

        prompt.push_str("\nQuestion: ");
        prompt.push_str(query);
        prompt.push_str("\n\nAnswer:");

        prompt
    }

    /// Rerank results based on graph structure
    pub fn rerank_results(
        &self,
        initial_results: Vec<Document>,
        _query: &[f32],
    ) -> Result<Vec<Document>> {
        // Simple reranking based on score
        // Real implementation would consider:
        // - Graph centrality
        // - Cross-document connections
        // - Temporal relevance
        // - User preferences

        let mut results = initial_results;
        results.sort_by(|a, b| {
            b.relevance_score
                .partial_cmp(&a.relevance_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        Ok(results)
    }

    /// Convert semantic path to reasoning path
    fn convert_to_reasoning_path(&self, semantic_path: SemanticPath) -> ReasoningPath {
        let steps = semantic_path
            .nodes
            .iter()
            .map(|node_id| ReasoningStep {
                node_id: node_id.clone(),
                content: format!("Step at node {}", node_id),
                relationship: "RELATED_TO".to_string(),
                confidence: semantic_path.semantic_score,
            })
            .collect();

        ReasoningPath {
            steps,
            total_confidence: semantic_path.combined_score,
            explanation: format!("Reasoning path with {} steps", semantic_path.nodes.len()),
        }
    }

    /// Estimate token count for documents
    fn estimate_tokens(&self, documents: &[Document]) -> usize {
        // Rough estimation: ~4 characters per token
        documents.iter().map(|doc| doc.content.len() / 4).sum()
    }
}

/// Retrieved context for generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// Retrieved documents
    pub documents: Vec<Document>,
    /// Total estimated tokens
    pub total_tokens: usize,
    /// Original query embedding
    pub query_embedding: Vec<f32>,
}

/// A retrieved document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub node_id: NodeId,
    pub content: String,
    pub metadata: HashMap<String, String>,
    pub relevance_score: f32,
}

/// A multi-hop reasoning path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningPath {
    /// Steps in the reasoning chain
    pub steps: Vec<ReasoningStep>,
    /// Overall confidence in this path
    pub total_confidence: f32,
    /// Human-readable explanation
    pub explanation: String,
}

/// A single step in reasoning
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningStep {
    pub node_id: NodeId,
    pub content: String,
    pub relationship: String,
    pub confidence: f32,
}

/// Aggregated evidence from multiple paths
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Evidence {
    pub node_id: NodeId,
    pub content: String,
    pub support_count: usize,
    pub confidence: f32,
    pub sources: Vec<NodeId>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hybrid::semantic_search::SemanticSearchConfig;
    use crate::hybrid::vector_index::{EmbeddingConfig, HybridIndex};

    #[test]
    fn test_rag_engine_creation() {
        let index = HybridIndex::new(EmbeddingConfig::default()).unwrap();
        let semantic_search = SemanticSearch::new(index, SemanticSearchConfig::default());
        let _rag = RagEngine::new(semantic_search, RagConfig::default());
    }

    #[test]
    fn test_context_retrieval() -> Result<()> {
        use crate::hybrid::vector_index::VectorIndexType;

        let config = EmbeddingConfig {
            dimensions: 4,
            ..Default::default()
        };
        let index = HybridIndex::new(config)?;
        // Initialize the node index
        index.initialize_index(VectorIndexType::Node)?;

        // Add test embeddings so search returns results
        index.add_node_embedding("doc1".to_string(), vec![1.0, 0.0, 0.0, 0.0])?;
        index.add_node_embedding("doc2".to_string(), vec![0.9, 0.1, 0.0, 0.0])?;

        let semantic_search = SemanticSearch::new(index, SemanticSearchConfig::default());
        let rag = RagEngine::new(semantic_search, RagConfig::default());

        let query = vec![1.0, 0.0, 0.0, 0.0];
        let context = rag.retrieve_context(&query)?;

        assert_eq!(context.query_embedding, query);
        // Should find at least one document
        assert!(!context.documents.is_empty());
        Ok(())
    }

    #[test]
    fn test_prompt_generation() {
        let index = HybridIndex::new(EmbeddingConfig::default()).unwrap();
        let semantic_search = SemanticSearch::new(index, SemanticSearchConfig::default());
        let rag = RagEngine::new(semantic_search, RagConfig::default());

        let context = Context {
            documents: vec![Document {
                node_id: "doc1".to_string(),
                content: "Test content".to_string(),
                metadata: HashMap::new(),
                relevance_score: 0.9,
            }],
            total_tokens: 100,
            query_embedding: vec![1.0; 4],
        };

        let prompt = rag.generate_prompt("What is the answer?", &context);
        assert!(prompt.contains("Test content"));
        assert!(prompt.contains("What is the answer?"));
    }
}
