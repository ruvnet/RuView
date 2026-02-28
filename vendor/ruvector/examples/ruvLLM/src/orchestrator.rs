//! Main orchestrator for RuvLLM
//!
//! Coordinates all components to process requests through the self-learning pipeline.

use crate::attention::GraphAttentionEngine;
use crate::config::Config;
use crate::embedding::EmbeddingService;
use crate::error::{Error, Result};
use crate::inference::InferencePool;
use crate::learning::LearningService;
use crate::memory::MemoryService;
use crate::router::FastGRNNRouter;
use crate::types::{
    Constraints, Feedback, LatencyBreakdown, Request, Response, RoutingInfo, Session, Source,
};

use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;
use uuid::Uuid;

/// Main RuvLLM system orchestrator
pub struct RuvLLM {
    /// Configuration
    config: Config,
    /// Embedding service
    embedding: Arc<EmbeddingService>,
    /// Memory service
    memory: Arc<MemoryService>,
    /// Router
    router: Arc<RwLock<FastGRNNRouter>>,
    /// Graph attention engine
    attention: Arc<GraphAttentionEngine>,
    /// Inference pool
    inference: Arc<InferencePool>,
    /// Learning service
    learning: Arc<LearningService>,
    /// Active sessions
    sessions: DashMap<String, Session>,
    /// Metrics collector
    #[cfg(feature = "metrics")]
    metrics: Arc<Metrics>,
}

impl RuvLLM {
    /// Create a new RuvLLM instance
    pub async fn new(config: Config) -> Result<Self> {
        tracing::info!("Initializing RuvLLM v{}", crate::VERSION);

        // Initialize components
        let embedding = Arc::new(EmbeddingService::new(&config.embedding)?);
        let memory = Arc::new(MemoryService::new(&config.memory).await?);
        let router = Arc::new(RwLock::new(FastGRNNRouter::new(&config.router)?));
        let attention = Arc::new(GraphAttentionEngine::new(&config.embedding)?);
        let inference = Arc::new(InferencePool::new(&config.inference).await?);

        let learning = Arc::new(LearningService::new(
            &config.learning,
            router.clone(),
            memory.clone(),
            config.embedding.dimension,
        )?);

        // Start background services
        if config.learning.enabled {
            learning.start_background_training().await;
        }

        Ok(Self {
            config,
            embedding,
            memory,
            router,
            attention,
            inference,
            learning,
            sessions: DashMap::new(),
            #[cfg(feature = "metrics")]
            metrics: Arc::new(Metrics::new()),
        })
    }

    /// Process a simple query
    pub async fn query(&self, query: impl Into<String>) -> Result<Response> {
        self.process(Request::new(query)).await
    }

    /// Process a query with session
    pub async fn query_session(
        &self,
        session: &Session,
        query: impl Into<String>,
    ) -> Result<Response> {
        self.process(Request::new(query).with_session(&session.id))
            .await
    }

    /// Process a full request
    pub async fn process(&self, request: Request) -> Result<Response> {
        let request_id = Uuid::new_v4().to_string();
        let start = Instant::now();
        let mut latency = LatencyBreakdown::default();

        tracing::debug!(request_id = %request_id, query = %request.query, "Processing request");

        // Step 1: Get or create session
        let session = self.get_or_create_session(&request.session_id);

        // Step 2: Embed query
        let embed_start = Instant::now();
        let query_embedding = self.embedding.embed(&request.query)?;
        latency.embedding_ms = embed_start.elapsed().as_secs_f32() * 1000.0;

        // Step 3: Memory retrieval with graph expansion
        let retrieval_start = Instant::now();
        let ef_search = self.adaptive_ef_search(&request.constraints);
        let search_result = self
            .memory
            .search_with_graph(&query_embedding.vector, 64, ef_search, 2)
            .await?;
        latency.retrieval_ms = retrieval_start.elapsed().as_secs_f32() * 1000.0;

        // Step 4: Router decision
        let routing_start = Instant::now();
        let router_features =
            self.build_router_features(&query_embedding, &search_result, &request.constraints);

        let routing_decision = {
            let router = self.router.read();
            router.forward(&router_features, &session.router_hidden)?
        };
        latency.routing_ms = routing_start.elapsed().as_secs_f32() * 1000.0;

        // Step 5: Graph attention for context ranking
        let attention_start = Instant::now();
        let graph_context = self
            .attention
            .attend(&query_embedding.vector, &search_result.subgraph)?;
        latency.attention_ms = attention_start.elapsed().as_secs_f32() * 1000.0;

        // Step 6: Build context
        let context =
            self.build_context(&graph_context.ranked_nodes, routing_decision.context_size);

        // Step 7: Generate response
        let generation_start = Instant::now();
        let prompt = self.format_prompt(&request.query, &context);

        let generation_result = self
            .inference
            .generate(
                routing_decision.model,
                &prompt,
                crate::inference::GenerationConfig {
                    max_tokens: request.constraints.max_tokens.unwrap_or(512) as usize,
                    temperature: routing_decision.temperature,
                    top_p: routing_decision.top_p,
                    top_k: 40,
                    repeat_penalty: 1.1,
                },
                session.kv_cache_key.as_deref(),
            )
            .await?;
        latency.generation_ms = generation_start.elapsed().as_secs_f32() * 1000.0;

        latency.total_ms = start.elapsed().as_secs_f32() * 1000.0;

        // Step 8: Quality evaluation and learning (async, non-blocking)
        let response_text = generation_result.text.clone();
        let context_for_learning = context.clone();
        let query_for_learning = request.query.clone();
        let learning = self.learning.clone();

        tokio::spawn(async move {
            if let Err(e) = learning
                .on_interaction(&query_for_learning, &response_text, &context_for_learning)
                .await
            {
                tracing::warn!("Learning service error: {}", e);
            }
        });

        // Update session
        if let Some(mut session_entry) = self.sessions.get_mut(&session.id) {
            session_entry.router_hidden = routing_decision.new_hidden.clone();
            session_entry.add_turn(request.query.clone(), generation_result.text.clone());
        }

        // Build response
        let sources: Vec<Source> = graph_context
            .ranked_nodes
            .iter()
            .take(5)
            .zip(graph_context.attention_weights.iter())
            .map(|(node, &weight)| Source {
                id: node.id.clone(),
                preview: node.text.chars().take(100).collect(),
                relevance: weight,
            })
            .collect();

        Ok(Response {
            request_id,
            text: generation_result.text,
            confidence: routing_decision.confidence,
            sources,
            routing_info: RoutingInfo {
                model: routing_decision.model,
                context_size: routing_decision.context_size,
                temperature: routing_decision.temperature,
                top_p: routing_decision.top_p,
                confidence: routing_decision.confidence,
            },
            latency,
        })
    }

    /// Provide feedback on a response
    pub async fn feedback(&self, feedback: Feedback) -> Result<()> {
        self.learning.record_feedback(feedback).await
    }

    /// Create a new session
    pub fn new_session(&self) -> Session {
        let session = Session::new(self.config.router.hidden_dim);
        self.sessions.insert(session.id.clone(), session.clone());
        session
    }

    /// Get or create session
    fn get_or_create_session(&self, session_id: &Option<String>) -> Session {
        match session_id {
            Some(id) => self.sessions.get(id).map(|s| s.clone()).unwrap_or_else(|| {
                let session = Session::new(self.config.router.hidden_dim);
                self.sessions.insert(id.clone(), session.clone());
                session
            }),
            None => Session::new(self.config.router.hidden_dim),
        }
    }

    /// Adaptive ef_search based on latency budget
    fn adaptive_ef_search(&self, constraints: &Constraints) -> usize {
        match constraints.max_latency_ms {
            Some(budget) if budget < 100 => 32,
            Some(budget) if budget < 300 => 64,
            Some(budget) if budget < 500 => 128,
            _ => self.config.memory.hnsw_ef_search,
        }
    }

    /// Build router features from query and search results
    fn build_router_features(
        &self,
        embedding: &crate::embedding::Embedding,
        search_result: &crate::memory::SearchResult,
        constraints: &Constraints,
    ) -> Vec<f32> {
        // Build 128-dimensional feature vector
        let mut features = vec![0.0f32; self.config.router.input_dim];

        // Query features (first 32 dims)
        let norm = embedding.vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        features[0] = (embedding.token_count as f32 / 512.0).min(1.0);
        features[1] = norm / 10.0;

        // Search stats (dims 32-80)
        if !search_result.candidates.is_empty() {
            let distances: Vec<f32> = search_result
                .candidates
                .iter()
                .map(|c| c.distance)
                .collect();
            let mean = distances.iter().sum::<f32>() / distances.len() as f32;
            let std = (distances.iter().map(|d| (d - mean).powi(2)).sum::<f32>()
                / distances.len() as f32)
                .sqrt();

            features[32] = (search_result.candidates.len() as f32 / 64.0).min(1.0);
            features[33] = mean / 2.0;
            features[34] = std;
            features[35] = distances.iter().cloned().fold(f32::INFINITY, f32::min) / 2.0;
            features[36] = distances.iter().cloned().fold(f32::NEG_INFINITY, f32::max) / 2.0;
        }

        // Constraints (dims 96-128)
        features[96] = constraints
            .max_latency_ms
            .map(|l| l as f32 / 5000.0)
            .unwrap_or(0.5);
        features[97] = match self.config.system.device_class.as_str() {
            "edge" => 0.25,
            "mobile" => 0.5,
            "server" => 0.75,
            "gpu" => 1.0,
            _ => 0.5,
        };

        features
    }

    /// Build context from ranked nodes
    fn build_context(&self, nodes: &[crate::types::MemoryNode], max_tokens: usize) -> Vec<String> {
        let mut context = Vec::new();
        let mut total_tokens = 0;

        for node in nodes {
            let node_tokens = node.text.split_whitespace().count();
            if total_tokens + node_tokens > max_tokens {
                break;
            }
            context.push(node.text.clone());
            total_tokens += node_tokens;
        }

        context
    }

    /// Format prompt with context
    fn format_prompt(&self, query: &str, context: &[String]) -> String {
        let context_text = context
            .iter()
            .enumerate()
            .map(|(i, text)| format!("[{}] {}", i + 1, text))
            .collect::<Vec<_>>()
            .join("\n\n");

        format!(
            "You are a helpful assistant. Answer the question based on the provided context.\n\n\
            Context:\n{}\n\n\
            Question: {}\n\n\
            Answer:",
            context_text, query
        )
    }

    /// Shutdown the system gracefully
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("Shutting down RuvLLM");

        // Stop learning service
        self.learning.stop().await;

        // Flush memory
        self.memory.flush().await?;

        // Save router weights
        if let Some(path) = &self.config.router.weights_path {
            let router = self.router.read();
            router.save_weights(path)?;
        }

        tracing::info!("RuvLLM shutdown complete");
        Ok(())
    }
}

#[cfg(feature = "metrics")]
struct Metrics {
    request_counter: prometheus::IntCounter,
    latency_histogram: prometheus::Histogram,
    quality_gauge: prometheus::Gauge,
}

#[cfg(feature = "metrics")]
impl Metrics {
    fn new() -> Self {
        use once_cell::sync::Lazy;

        // Use lazy statics to ensure metrics are only registered once
        static REQUEST_COUNTER: Lazy<prometheus::IntCounter> = Lazy::new(|| {
            prometheus::register_int_counter!("ruvllm_requests_total", "Total number of requests")
                .unwrap()
        });

        static LATENCY_HISTOGRAM: Lazy<prometheus::Histogram> = Lazy::new(|| {
            prometheus::register_histogram!(
                "ruvllm_request_latency_seconds",
                "Request latency in seconds"
            )
            .unwrap()
        });

        static QUALITY_GAUGE: Lazy<prometheus::Gauge> = Lazy::new(|| {
            prometheus::register_gauge!("ruvllm_quality_score", "Average quality score").unwrap()
        });

        Self {
            request_counter: REQUEST_COUNTER.clone(),
            latency_histogram: LATENCY_HISTOGRAM.clone(),
            quality_gauge: QUALITY_GAUGE.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_orchestrator_creation() {
        // This would require mock implementations
        // For now, just verify types compile
    }
}
