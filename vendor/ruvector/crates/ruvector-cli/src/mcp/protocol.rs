//! MCP protocol types and utilities

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// MCP request message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// MCP response message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl McpError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

/// Standard MCP error codes
pub mod error_codes {
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;
}

impl McpResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<Value>, error: McpError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// MCP Resource definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
}

/// MCP Prompt definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPrompt {
    pub name: String,
    pub description: String,
    pub arguments: Option<Vec<PromptArgument>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptArgument {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// Tool call parameters for vector_db_create
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDbParams {
    pub path: String,
    pub dimensions: usize,
    #[serde(default)]
    pub distance_metric: Option<String>,
}

/// Tool call parameters for vector_db_insert
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertParams {
    pub db_path: String,
    pub vectors: Vec<VectorInsert>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorInsert {
    pub id: Option<String>,
    pub vector: Vec<f32>,
    pub metadata: Option<Value>,
}

/// Tool call parameters for vector_db_search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchParams {
    pub db_path: String,
    pub query: Vec<f32>,
    pub k: usize,
    pub filter: Option<Value>,
}

/// Tool call parameters for vector_db_stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatsParams {
    pub db_path: String,
}

/// Tool call parameters for vector_db_backup
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupParams {
    pub db_path: String,
    pub backup_path: String,
}

// ==================== GNN Tool Parameters ====================

/// Tool call parameters for gnn_layer_create
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnLayerCreateParams {
    pub input_dim: usize,
    pub hidden_dim: usize,
    pub heads: usize,
    #[serde(default = "default_dropout")]
    pub dropout: f32,
}

fn default_dropout() -> f32 {
    0.1
}

/// Tool call parameters for gnn_forward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnForwardParams {
    pub layer_id: String,
    pub node_embedding: Vec<f64>,
    pub neighbor_embeddings: Vec<Vec<f64>>,
    pub edge_weights: Vec<f64>,
}

/// Tool call parameters for gnn_batch_forward
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnBatchForwardParams {
    pub layer_config: GnnLayerConfigParams,
    pub operations: Vec<GnnOperationParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnLayerConfigParams {
    pub input_dim: usize,
    pub hidden_dim: usize,
    pub heads: usize,
    #[serde(default = "default_dropout")]
    pub dropout: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnOperationParams {
    pub node_embedding: Vec<f64>,
    pub neighbor_embeddings: Vec<Vec<f64>>,
    pub edge_weights: Vec<f64>,
}

/// Tool call parameters for gnn_cache_stats
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnCacheStatsParams {
    #[serde(default)]
    pub include_details: bool,
}

/// Tool call parameters for gnn_compress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnCompressParams {
    pub embedding: Vec<f64>,
    pub access_freq: f64,
}

/// Tool call parameters for gnn_decompress
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnDecompressParams {
    pub compressed_json: String,
}

/// Tool call parameters for gnn_search
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GnnSearchParams {
    pub query: Vec<f64>,
    pub candidates: Vec<Vec<f64>>,
    pub k: usize,
    #[serde(default = "default_temperature")]
    pub temperature: f64,
}

fn default_temperature() -> f64 {
    1.0
}
