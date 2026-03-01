//! Core types for RuvLLM

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Model size variants
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModelSize {
    /// 350M parameters - edge/simple queries
    M350,
    /// 700M parameters - mobile/moderate queries
    M700,
    /// 1.2B parameters - server/complex queries
    B1_2,
    /// 2.6B parameters - escalation/judge
    B2_6,
}

impl ModelSize {
    /// Get model size from index
    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => ModelSize::M350,
            1 => ModelSize::M700,
            2 => ModelSize::B1_2,
            _ => ModelSize::B2_6,
        }
    }

    /// Get index for model size
    pub fn to_index(self) -> usize {
        match self {
            ModelSize::M350 => 0,
            ModelSize::M700 => 1,
            ModelSize::B1_2 => 2,
            ModelSize::B2_6 => 3,
        }
    }

    /// Get approximate parameter count
    pub fn params(self) -> u64 {
        match self {
            ModelSize::M350 => 350_000_000,
            ModelSize::M700 => 700_000_000,
            ModelSize::B1_2 => 1_200_000_000,
            ModelSize::B2_6 => 2_600_000_000,
        }
    }
}

/// Context size bins
pub const CONTEXT_BINS: [usize; 5] = [256, 512, 1024, 2048, 4096];

/// Request to the RuvLLM system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    /// The user query
    pub query: String,
    /// Optional session ID for multi-turn conversations
    pub session_id: Option<String>,
    /// Constraints on the request
    pub constraints: Constraints,
}

impl Request {
    /// Create a simple request with just a query
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            session_id: None,
            constraints: Constraints::default(),
        }
    }

    /// Set session ID
    pub fn with_session(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Set constraints
    pub fn with_constraints(mut self, constraints: Constraints) -> Self {
        self.constraints = constraints;
        self
    }
}

/// Constraints on request processing
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Constraints {
    /// Maximum latency in milliseconds
    pub max_latency_ms: Option<u32>,
    /// Maximum tokens to generate
    pub max_tokens: Option<u32>,
    /// Temperature for generation
    pub temperature: Option<f32>,
    /// Top-p for nucleus sampling
    pub top_p: Option<f32>,
    /// Force specific model size
    pub force_model: Option<ModelSize>,
    /// Force specific context size
    pub force_context: Option<usize>,
}

/// Response from the RuvLLM system
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    /// Unique request ID
    pub request_id: String,
    /// Generated text
    pub text: String,
    /// Confidence score (0-1)
    pub confidence: f32,
    /// Source documents used
    pub sources: Vec<Source>,
    /// Routing information
    pub routing_info: RoutingInfo,
    /// Latency breakdown
    pub latency: LatencyBreakdown,
}

/// Source document information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Source {
    /// Node ID
    pub id: String,
    /// Text preview
    pub preview: String,
    /// Relevance score
    pub relevance: f32,
}

/// Routing decision information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingInfo {
    /// Selected model
    pub model: ModelSize,
    /// Context size used
    pub context_size: usize,
    /// Temperature used
    pub temperature: f32,
    /// Top-p used
    pub top_p: f32,
    /// Router confidence
    pub confidence: f32,
}

/// Latency breakdown in milliseconds
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LatencyBreakdown {
    /// Total latency
    pub total_ms: f32,
    /// Embedding latency
    pub embedding_ms: f32,
    /// Retrieval latency
    pub retrieval_ms: f32,
    /// Routing latency
    pub routing_ms: f32,
    /// Attention latency
    pub attention_ms: f32,
    /// Generation latency
    pub generation_ms: f32,
}

/// Session state for multi-turn conversations
#[derive(Debug, Clone)]
pub struct Session {
    /// Session ID
    pub id: String,
    /// Router hidden state
    pub router_hidden: Vec<f32>,
    /// KV cache key
    pub kv_cache_key: Option<String>,
    /// Conversation history (for context)
    pub history: Vec<ConversationTurn>,
    /// Created timestamp
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// Last used timestamp
    pub last_used: chrono::DateTime<chrono::Utc>,
}

impl Session {
    /// Create a new session
    pub fn new(hidden_dim: usize) -> Self {
        let id = Uuid::new_v4().to_string();
        let now = chrono::Utc::now();
        Self {
            id,
            router_hidden: vec![0.0; hidden_dim],
            kv_cache_key: None,
            history: Vec::new(),
            created_at: now,
            last_used: now,
        }
    }

    /// Add a turn to the conversation
    pub fn add_turn(&mut self, query: String, response: String) {
        self.history.push(ConversationTurn { query, response });
        self.last_used = chrono::Utc::now();
    }
}

/// A single turn in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTurn {
    /// User query
    pub query: String,
    /// System response
    pub response: String,
}

/// Feedback on a response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Feedback {
    /// Request ID to provide feedback for
    pub request_id: String,
    /// Rating (1-5)
    pub rating: Option<u8>,
    /// Correction text
    pub correction: Option<String>,
    /// Task outcome
    pub task_success: Option<bool>,
}

/// Node types in memory
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    /// User query
    Query,
    /// Document/passage
    Document,
    /// Q&A pair
    QAPair,
    /// Agent reasoning step
    AgentStep,
    /// Factual statement
    Fact,
    /// Abstract concept (from compression)
    Concept,
}

/// Edge types in graph
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    /// Citation relationship
    Cites,
    /// Sequential relationship
    Follows,
    /// Same topic relationship
    SameTopic,
    /// Agent step relationship
    AgentStep,
    /// Derived from relationship
    Derived,
    /// Contains relationship (concept to detail)
    Contains,
}

/// Memory node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryNode {
    /// Unique ID
    pub id: String,
    /// Vector embedding
    pub vector: Vec<f32>,
    /// Text content
    pub text: String,
    /// Node type
    pub node_type: NodeType,
    /// Source identifier
    pub source: String,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Memory edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEdge {
    /// Unique ID
    pub id: String,
    /// Source node ID
    pub src: String,
    /// Destination node ID
    pub dst: String,
    /// Edge type
    pub edge_type: EdgeType,
    /// Edge weight
    pub weight: f32,
    /// Metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

/// Router output decision
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Selected model
    pub model: ModelSize,
    /// Selected context size
    pub context_size: usize,
    /// Temperature
    pub temperature: f32,
    /// Top-p
    pub top_p: f32,
    /// Confidence
    pub confidence: f32,
    /// Model probabilities
    pub model_probs: [f32; 4],
    /// Updated hidden state
    pub new_hidden: Vec<f32>,
    /// Input features (for logging)
    pub features: Vec<f32>,
}

impl Default for RoutingDecision {
    fn default() -> Self {
        Self::safe_default()
    }
}

impl RoutingDecision {
    /// Safe default routing decision
    pub fn safe_default() -> Self {
        Self {
            model: ModelSize::B1_2,
            context_size: 2048,
            temperature: 0.7,
            top_p: 0.9,
            confidence: 0.5,
            model_probs: [0.1, 0.2, 0.5, 0.2],
            new_hidden: vec![0.0; 64],
            features: vec![],
        }
    }

    /// Get context bin index
    pub fn context_bin(&self) -> usize {
        CONTEXT_BINS
            .iter()
            .position(|&c| c >= self.context_size)
            .unwrap_or(CONTEXT_BINS.len() - 1)
    }
}

/// Training sample for router
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterSample {
    /// Input features
    pub features: Vec<f32>,
    /// Label: which model was best
    pub label_model: usize,
    /// Label: which context size was best
    pub label_context: usize,
    /// Label: optimal temperature
    pub label_temperature: f32,
    /// Label: optimal top_p
    pub label_top_p: f32,
    /// Quality score achieved
    pub quality: f32,
    /// Latency achieved
    pub latency_ms: f32,
}

/// Interaction outcome for learning
#[derive(Debug, Clone)]
pub struct InteractionOutcome {
    /// Quality score (0-1)
    pub quality_score: f32,
    /// Node IDs used in this interaction
    pub used_nodes: Vec<String>,
    /// Whether the task succeeded
    pub task_success: bool,
    /// Explicit user rating if any
    pub user_rating: Option<u8>,
}
