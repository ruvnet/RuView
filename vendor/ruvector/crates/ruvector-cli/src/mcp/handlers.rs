//! MCP request handlers

use super::gnn_cache::{BatchGnnRequest, GnnCache, GnnCacheConfig, GnnOperation, LayerConfig};
use super::protocol::*;
use crate::config::Config;
use anyhow::{Context, Result};
use ruvector_core::{
    types::{DbOptions, DistanceMetric, SearchQuery, VectorEntry},
    VectorDB,
};
use ruvector_gnn::{compress::TensorCompress, search::differentiable_search};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

/// MCP handler state with GNN caching for performance optimization
pub struct McpHandler {
    config: Config,
    databases: Arc<RwLock<HashMap<String, Arc<VectorDB>>>>,
    /// GNN layer cache for eliminating ~2.5s initialization overhead
    gnn_cache: Arc<GnnCache>,
    /// Tensor compressor for GNN operations
    tensor_compress: Arc<TensorCompress>,
    /// Allowed base directory for all file operations (path confinement)
    allowed_data_dir: PathBuf,
}

impl McpHandler {
    pub fn new(config: Config) -> Self {
        let gnn_cache = Arc::new(GnnCache::new(GnnCacheConfig::default()));
        let allowed_data_dir = PathBuf::from(&config.mcp.data_dir);
        // Canonicalize at startup so all later comparisons are absolute
        let allowed_data_dir = std::fs::canonicalize(&allowed_data_dir)
            .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/")));

        Self {
            config,
            databases: Arc::new(RwLock::new(HashMap::new())),
            gnn_cache,
            tensor_compress: Arc::new(TensorCompress::new()),
            allowed_data_dir,
        }
    }

    /// Initialize with preloaded GNN layers for optimal performance
    pub async fn with_preload(config: Config) -> Self {
        let handler = Self::new(config);
        handler.gnn_cache.preload_common_layers().await;
        handler
    }

    /// Validate that a user-supplied path resolves within the allowed data directory.
    ///
    /// Prevents CWE-22 path traversal by:
    /// 1. Resolving the path relative to `allowed_data_dir` (not cwd)
    /// 2. Canonicalizing to eliminate `..`, symlinks, and other tricks
    /// 3. Checking that the canonical path starts with the allowed directory
    fn validate_path(&self, user_path: &str) -> Result<PathBuf> {
        // Reject obviously malicious absolute paths outside data dir
        let path = Path::new(user_path);

        // If relative, resolve against allowed_data_dir
        let resolved = if path.is_absolute() {
            PathBuf::from(user_path)
        } else {
            self.allowed_data_dir.join(user_path)
        };

        // For existing paths, canonicalize resolves symlinks and ..
        // For non-existing paths, canonicalize the parent and append the filename
        let canonical = if resolved.exists() {
            std::fs::canonicalize(&resolved)
                .with_context(|| format!("Failed to resolve path: {}", user_path))?
        } else {
            // Canonicalize the parent directory (must exist), then append filename
            let parent = resolved.parent().unwrap_or(Path::new("/"));
            let parent_canonical = if parent.exists() {
                std::fs::canonicalize(parent).with_context(|| {
                    format!("Parent directory does not exist: {}", parent.display())
                })?
            } else {
                // Create the parent directory within allowed_data_dir if it doesn't exist
                anyhow::bail!(
                    "Path '{}' references non-existent directory '{}'",
                    user_path,
                    parent.display()
                );
            };
            let filename = resolved
                .file_name()
                .ok_or_else(|| anyhow::anyhow!("Invalid path: no filename in '{}'", user_path))?;
            parent_canonical.join(filename)
        };

        // Security check: canonical path must be inside allowed_data_dir
        if !canonical.starts_with(&self.allowed_data_dir) {
            anyhow::bail!(
                "Access denied: path '{}' resolves to '{}' which is outside the allowed data directory '{}'",
                user_path,
                canonical.display(),
                self.allowed_data_dir.display()
            );
        }

        Ok(canonical)
    }

    /// Handle MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id).await,
            "tools/list" => self.handle_tools_list(request.id).await,
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            "resources/list" => self.handle_resources_list(request.id).await,
            "resources/read" => self.handle_resources_read(request.id, request.params).await,
            "prompts/list" => self.handle_prompts_list(request.id).await,
            "prompts/get" => self.handle_prompts_get(request.id, request.params).await,
            _ => McpResponse::error(
                request.id,
                McpError::new(error_codes::METHOD_NOT_FOUND, "Method not found"),
            ),
        }
    }

    async fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {}
                },
                "serverInfo": {
                    "name": "ruvector-mcp",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    async fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        let tools = vec![
            McpTool {
                name: "vector_db_create".to_string(),
                description: "Create a new vector database".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string", "description": "Database file path"},
                        "dimensions": {"type": "integer", "description": "Vector dimensions"},
                        "distance_metric": {"type": "string", "enum": ["euclidean", "cosine", "dotproduct", "manhattan"]}
                    },
                    "required": ["path", "dimensions"]
                }),
            },
            McpTool {
                name: "vector_db_insert".to_string(),
                description: "Insert vectors into database".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "db_path": {"type": "string"},
                        "vectors": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "id": {"type": "string"},
                                    "vector": {"type": "array", "items": {"type": "number"}},
                                    "metadata": {"type": "object"}
                                }
                            }
                        }
                    },
                    "required": ["db_path", "vectors"]
                }),
            },
            McpTool {
                name: "vector_db_search".to_string(),
                description: "Search for similar vectors".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "db_path": {"type": "string"},
                        "query": {"type": "array", "items": {"type": "number"}},
                        "k": {"type": "integer", "default": 10},
                        "filter": {"type": "object"}
                    },
                    "required": ["db_path", "query"]
                }),
            },
            McpTool {
                name: "vector_db_stats".to_string(),
                description: "Get database statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "db_path": {"type": "string"}
                    },
                    "required": ["db_path"]
                }),
            },
            McpTool {
                name: "vector_db_backup".to_string(),
                description: "Backup database to file".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "db_path": {"type": "string"},
                        "backup_path": {"type": "string"}
                    },
                    "required": ["db_path", "backup_path"]
                }),
            },
            // GNN Tools with persistent caching (~250-500x faster)
            McpTool {
                name: "gnn_layer_create".to_string(),
                description: "Create/cache a GNN layer (eliminates ~2.5s init overhead)"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "input_dim": {"type": "integer", "description": "Input embedding dimension"},
                        "hidden_dim": {"type": "integer", "description": "Hidden layer dimension"},
                        "heads": {"type": "integer", "description": "Number of attention heads"},
                        "dropout": {"type": "number", "default": 0.1, "description": "Dropout rate"}
                    },
                    "required": ["input_dim", "hidden_dim", "heads"]
                }),
            },
            McpTool {
                name: "gnn_forward".to_string(),
                description: "Forward pass through cached GNN layer (~5-10ms vs ~2.5s)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "layer_id": {"type": "string", "description": "Layer config: input_hidden_heads"},
                        "node_embedding": {"type": "array", "items": {"type": "number"}},
                        "neighbor_embeddings": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                        "edge_weights": {"type": "array", "items": {"type": "number"}}
                    },
                    "required": ["layer_id", "node_embedding", "neighbor_embeddings", "edge_weights"]
                }),
            },
            McpTool {
                name: "gnn_batch_forward".to_string(),
                description: "Batch GNN forward passes with result caching (amortized cost)"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "layer_config": {
                            "type": "object",
                            "properties": {
                                "input_dim": {"type": "integer"},
                                "hidden_dim": {"type": "integer"},
                                "heads": {"type": "integer"},
                                "dropout": {"type": "number", "default": 0.1}
                            },
                            "required": ["input_dim", "hidden_dim", "heads"]
                        },
                        "operations": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "node_embedding": {"type": "array", "items": {"type": "number"}},
                                    "neighbor_embeddings": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                                    "edge_weights": {"type": "array", "items": {"type": "number"}}
                                }
                            }
                        }
                    },
                    "required": ["layer_config", "operations"]
                }),
            },
            McpTool {
                name: "gnn_cache_stats".to_string(),
                description: "Get GNN cache statistics (hit rates, counts)".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "include_details": {"type": "boolean", "default": false}
                    }
                }),
            },
            McpTool {
                name: "gnn_compress".to_string(),
                description: "Compress embedding based on access frequency".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "embedding": {"type": "array", "items": {"type": "number"}},
                        "access_freq": {"type": "number", "description": "Access frequency 0.0-1.0"}
                    },
                    "required": ["embedding", "access_freq"]
                }),
            },
            McpTool {
                name: "gnn_decompress".to_string(),
                description: "Decompress a compressed tensor".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "compressed_json": {"type": "string", "description": "Compressed tensor JSON"}
                    },
                    "required": ["compressed_json"]
                }),
            },
            McpTool {
                name: "gnn_search".to_string(),
                description: "Differentiable search with soft attention".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": {"type": "array", "items": {"type": "number"}},
                        "candidates": {"type": "array", "items": {"type": "array", "items": {"type": "number"}}},
                        "k": {"type": "integer", "description": "Number of results"},
                        "temperature": {"type": "number", "default": 1.0}
                    },
                    "required": ["query", "candidates", "k"]
                }),
            },
        ];

        McpResponse::success(id, json!({ "tools": tools }))
    }

    async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return McpResponse::error(
                    id,
                    McpError::new(error_codes::INVALID_PARAMS, "Missing params"),
                )
            }
        };

        let tool_name = params["name"].as_str().unwrap_or("");
        let arguments = &params["arguments"];

        let result = match tool_name {
            // Vector DB tools
            "vector_db_create" => self.tool_create_db(arguments).await,
            "vector_db_insert" => self.tool_insert(arguments).await,
            "vector_db_search" => self.tool_search(arguments).await,
            "vector_db_stats" => self.tool_stats(arguments).await,
            "vector_db_backup" => self.tool_backup(arguments).await,
            // GNN tools with caching
            "gnn_layer_create" => self.tool_gnn_layer_create(arguments).await,
            "gnn_forward" => self.tool_gnn_forward(arguments).await,
            "gnn_batch_forward" => self.tool_gnn_batch_forward(arguments).await,
            "gnn_cache_stats" => self.tool_gnn_cache_stats(arguments).await,
            "gnn_compress" => self.tool_gnn_compress(arguments).await,
            "gnn_decompress" => self.tool_gnn_decompress(arguments).await,
            "gnn_search" => self.tool_gnn_search(arguments).await,
            _ => Err(anyhow::anyhow!("Unknown tool: {}", tool_name)),
        };

        match result {
            Ok(value) => {
                McpResponse::success(id, json!({ "content": [{"type": "text", "text": value}] }))
            }
            Err(e) => McpResponse::error(
                id,
                McpError::new(error_codes::INTERNAL_ERROR, e.to_string()),
            ),
        }
    }

    async fn handle_resources_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "resources": [
                    {
                        "uri": "database://local/default",
                        "name": "Default Database",
                        "description": "Default vector database",
                        "mimeType": "application/x-ruvector-db"
                    }
                ]
            }),
        )
    }

    async fn handle_resources_read(
        &self,
        id: Option<Value>,
        _params: Option<Value>,
    ) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "contents": [{
                    "uri": "database://local/default",
                    "mimeType": "application/json",
                    "text": "{\"status\": \"available\"}"
                }]
            }),
        )
    }

    async fn handle_prompts_list(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "prompts": [
                    {
                        "name": "semantic-search",
                        "description": "Generate a semantic search query",
                        "arguments": [
                            {
                                "name": "query",
                                "description": "Natural language query",
                                "required": true
                            }
                        ]
                    }
                ]
            }),
        )
    }

    async fn handle_prompts_get(&self, id: Option<Value>, _params: Option<Value>) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "description": "Semantic search template",
                "messages": [
                    {
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": "Search for vectors related to: {{query}}"
                        }
                    }
                ]
            }),
        )
    }

    // Tool implementations
    async fn tool_create_db(&self, args: &Value) -> Result<String> {
        let params: CreateDbParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        // Validate path to prevent directory traversal (CWE-22)
        let validated_path = self.validate_path(&params.path)?;

        let mut db_options = self.config.to_db_options();
        db_options.storage_path = validated_path.to_string_lossy().to_string();
        db_options.dimensions = params.dimensions;

        if let Some(metric) = params.distance_metric {
            db_options.distance_metric = match metric.as_str() {
                "euclidean" => DistanceMetric::Euclidean,
                "cosine" => DistanceMetric::Cosine,
                "dotproduct" => DistanceMetric::DotProduct,
                "manhattan" => DistanceMetric::Manhattan,
                _ => DistanceMetric::Cosine,
            };
        }

        let db = VectorDB::new(db_options)?;
        let path_str = validated_path.to_string_lossy().to_string();
        self.databases
            .write()
            .await
            .insert(path_str.clone(), Arc::new(db));

        Ok(format!("Database created at: {}", path_str))
    }

    async fn tool_insert(&self, args: &Value) -> Result<String> {
        let params: InsertParams = serde_json::from_value(args.clone())?;
        let db = self.get_or_open_db(&params.db_path).await?;

        let entries: Vec<VectorEntry> = params
            .vectors
            .into_iter()
            .map(|v| VectorEntry {
                id: v.id,
                vector: v.vector,
                metadata: v.metadata.and_then(|m| serde_json::from_value(m).ok()),
            })
            .collect();

        let ids = db.insert_batch(entries)?;
        Ok(format!("Inserted {} vectors", ids.len()))
    }

    async fn tool_search(&self, args: &Value) -> Result<String> {
        let params: SearchParams = serde_json::from_value(args.clone())?;
        let db = self.get_or_open_db(&params.db_path).await?;

        let results = db.search(SearchQuery {
            vector: params.query,
            k: params.k,
            filter: params.filter.and_then(|f| serde_json::from_value(f).ok()),
            ef_search: None,
        })?;

        serde_json::to_string_pretty(&results).context("Failed to serialize results")
    }

    async fn tool_stats(&self, args: &Value) -> Result<String> {
        let params: StatsParams = serde_json::from_value(args.clone())?;
        let db = self.get_or_open_db(&params.db_path).await?;

        let count = db.len()?;
        let options = db.options();

        Ok(json!({
            "count": count,
            "dimensions": options.dimensions,
            "distance_metric": format!("{:?}", options.distance_metric),
            "hnsw_enabled": options.hnsw_config.is_some()
        })
        .to_string())
    }

    async fn tool_backup(&self, args: &Value) -> Result<String> {
        let params: BackupParams = serde_json::from_value(args.clone())?;

        // Validate both paths to prevent directory traversal (CWE-22)
        let validated_db_path = self.validate_path(&params.db_path)?;
        let validated_backup_path = self.validate_path(&params.backup_path)?;

        std::fs::copy(&validated_db_path, &validated_backup_path)
            .context("Failed to backup database")?;

        Ok(format!("Backed up to: {}", validated_backup_path.display()))
    }

    async fn get_or_open_db(&self, path: &str) -> Result<Arc<VectorDB>> {
        // Validate path to prevent directory traversal (CWE-22)
        let validated_path = self.validate_path(path)?;
        let path_str = validated_path.to_string_lossy().to_string();

        let databases = self.databases.read().await;
        if let Some(db) = databases.get(&path_str) {
            return Ok(db.clone());
        }
        drop(databases);

        // Open new database
        let mut db_options = self.config.to_db_options();
        db_options.storage_path = path_str.clone();

        let db = Arc::new(VectorDB::new(db_options)?);
        self.databases.write().await.insert(path_str, db.clone());

        Ok(db)
    }

    // ==================== GNN Tool Implementations ====================
    // These tools eliminate ~2.5s overhead per operation via persistent caching

    /// Create or retrieve a cached GNN layer
    async fn tool_gnn_layer_create(&self, args: &Value) -> Result<String> {
        let params: GnnLayerCreateParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let start = Instant::now();

        let _layer = self
            .gnn_cache
            .get_or_create_layer(
                params.input_dim,
                params.hidden_dim,
                params.heads,
                params.dropout,
            )
            .await;

        let elapsed = start.elapsed();
        let layer_id = format!(
            "{}_{}_{}_{}",
            params.input_dim,
            params.hidden_dim,
            params.heads,
            (params.dropout * 1000.0) as u32
        );

        Ok(json!({
            "layer_id": layer_id,
            "input_dim": params.input_dim,
            "hidden_dim": params.hidden_dim,
            "heads": params.heads,
            "dropout": params.dropout,
            "creation_time_ms": elapsed.as_secs_f64() * 1000.0,
            "cached": elapsed.as_millis() < 50 // <50ms indicates cache hit
        })
        .to_string())
    }

    /// Forward pass through a cached GNN layer
    async fn tool_gnn_forward(&self, args: &Value) -> Result<String> {
        let params: GnnForwardParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let start = Instant::now();

        // Parse layer_id format: "input_hidden_heads_dropout"
        let parts: Vec<&str> = params.layer_id.split('_').collect();
        if parts.len() < 3 {
            return Err(anyhow::anyhow!(
                "Invalid layer_id format. Expected: input_hidden_heads[_dropout]"
            ));
        }

        let input_dim: usize = parts[0].parse()?;
        let hidden_dim: usize = parts[1].parse()?;
        let heads: usize = parts[2].parse()?;
        let dropout: f32 = parts
            .get(3)
            .map(|s| s.parse::<u32>().unwrap_or(100) as f32 / 1000.0)
            .unwrap_or(0.1);

        let layer = self
            .gnn_cache
            .get_or_create_layer(input_dim, hidden_dim, heads, dropout)
            .await;

        // Convert f64 to f32
        let node_f32: Vec<f32> = params.node_embedding.iter().map(|&x| x as f32).collect();
        let neighbors_f32: Vec<Vec<f32>> = params
            .neighbor_embeddings
            .iter()
            .map(|v| v.iter().map(|&x| x as f32).collect())
            .collect();
        let weights_f32: Vec<f32> = params.edge_weights.iter().map(|&x| x as f32).collect();

        let result = layer.forward(&node_f32, &neighbors_f32, &weights_f32);
        let elapsed = start.elapsed();

        // Convert back to f64 for JSON
        let result_f64: Vec<f64> = result.iter().map(|&x| x as f64).collect();

        Ok(json!({
            "result": result_f64,
            "output_dim": result.len(),
            "latency_ms": elapsed.as_secs_f64() * 1000.0
        })
        .to_string())
    }

    /// Batch forward passes with caching
    async fn tool_gnn_batch_forward(&self, args: &Value) -> Result<String> {
        let params: GnnBatchForwardParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let request = BatchGnnRequest {
            layer_config: LayerConfig {
                input_dim: params.layer_config.input_dim,
                hidden_dim: params.layer_config.hidden_dim,
                heads: params.layer_config.heads,
                dropout: params.layer_config.dropout,
            },
            operations: params
                .operations
                .into_iter()
                .map(|op| GnnOperation {
                    node_embedding: op.node_embedding.iter().map(|&x| x as f32).collect(),
                    neighbor_embeddings: op
                        .neighbor_embeddings
                        .iter()
                        .map(|v| v.iter().map(|&x| x as f32).collect())
                        .collect(),
                    edge_weights: op.edge_weights.iter().map(|&x| x as f32).collect(),
                })
                .collect(),
        };

        let batch_result = self.gnn_cache.batch_forward(request).await;

        // Convert results to f64
        let results_f64: Vec<Vec<f64>> = batch_result
            .results
            .iter()
            .map(|r| r.iter().map(|&x| x as f64).collect())
            .collect();

        Ok(json!({
            "results": results_f64,
            "cached_count": batch_result.cached_count,
            "computed_count": batch_result.computed_count,
            "total_time_ms": batch_result.total_time_ms,
            "avg_time_per_op_ms": batch_result.total_time_ms / (batch_result.cached_count + batch_result.computed_count) as f64
        })
        .to_string())
    }

    /// Get GNN cache statistics
    async fn tool_gnn_cache_stats(&self, args: &Value) -> Result<String> {
        let params: GnnCacheStatsParams =
            serde_json::from_value(args.clone()).unwrap_or(GnnCacheStatsParams {
                include_details: false,
            });

        let stats = self.gnn_cache.stats().await;
        let layer_count = self.gnn_cache.layer_count().await;
        let query_count = self.gnn_cache.query_result_count().await;

        let mut result = json!({
            "layer_hits": stats.layer_hits,
            "layer_misses": stats.layer_misses,
            "layer_hit_rate": format!("{:.2}%", stats.layer_hit_rate() * 100.0),
            "query_hits": stats.query_hits,
            "query_misses": stats.query_misses,
            "query_hit_rate": format!("{:.2}%", stats.query_hit_rate() * 100.0),
            "total_queries": stats.total_queries,
            "evictions": stats.evictions,
            "cached_layers": layer_count,
            "cached_queries": query_count
        });

        if params.include_details {
            result["estimated_memory_saved_ms"] = json!((stats.layer_hits as f64) * 2500.0);
            // ~2.5s per hit
        }

        Ok(result.to_string())
    }

    /// Compress embedding based on access frequency
    async fn tool_gnn_compress(&self, args: &Value) -> Result<String> {
        let params: GnnCompressParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let embedding_f32: Vec<f32> = params.embedding.iter().map(|&x| x as f32).collect();

        let compressed = self
            .tensor_compress
            .compress(&embedding_f32, params.access_freq as f32)
            .map_err(|e| anyhow::anyhow!("Compression error: {}", e))?;

        let compressed_json = serde_json::to_string(&compressed)?;

        Ok(json!({
            "compressed_json": compressed_json,
            "original_size": params.embedding.len() * 4,
            "compressed_size": compressed_json.len(),
            "compression_ratio": (params.embedding.len() * 4) as f64 / compressed_json.len() as f64
        })
        .to_string())
    }

    /// Decompress a compressed tensor
    async fn tool_gnn_decompress(&self, args: &Value) -> Result<String> {
        let params: GnnDecompressParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let compressed: ruvector_gnn::compress::CompressedTensor =
            serde_json::from_str(&params.compressed_json)
                .context("Invalid compressed tensor JSON")?;

        let decompressed = self
            .tensor_compress
            .decompress(&compressed)
            .map_err(|e| anyhow::anyhow!("Decompression error: {}", e))?;

        let decompressed_f64: Vec<f64> = decompressed.iter().map(|&x| x as f64).collect();

        Ok(json!({
            "embedding": decompressed_f64,
            "dimensions": decompressed.len()
        })
        .to_string())
    }

    /// Differentiable search with soft attention
    async fn tool_gnn_search(&self, args: &Value) -> Result<String> {
        let params: GnnSearchParams =
            serde_json::from_value(args.clone()).context("Invalid parameters")?;

        let start = Instant::now();

        let query_f32: Vec<f32> = params.query.iter().map(|&x| x as f32).collect();
        let candidates_f32: Vec<Vec<f32>> = params
            .candidates
            .iter()
            .map(|v| v.iter().map(|&x| x as f32).collect())
            .collect();

        let (indices, weights) = differentiable_search(
            &query_f32,
            &candidates_f32,
            params.k,
            params.temperature as f32,
        );

        let elapsed = start.elapsed();

        Ok(json!({
            "indices": indices,
            "weights": weights.iter().map(|&w| w as f64).collect::<Vec<f64>>(),
            "k": params.k,
            "latency_ms": elapsed.as_secs_f64() * 1000.0
        })
        .to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn handler_with_data_dir(data_dir: &Path) -> McpHandler {
        let mut config = Config::default();
        config.mcp.data_dir = data_dir.to_string_lossy().to_string();
        McpHandler::new(config)
    }

    #[test]
    fn test_validate_path_allows_relative_within_data_dir() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        // Create a file to validate against
        std::fs::write(dir.path().join("test.db"), b"test").unwrap();

        let result = handler.validate_path("test.db");
        assert!(result.is_ok(), "Should allow relative path within data dir");
        assert!(result.unwrap().starts_with(dir.path()));
    }

    #[test]
    fn test_validate_path_blocks_absolute_outside_data_dir() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        let result = handler.validate_path("/etc/passwd");
        assert!(result.is_err(), "Should block /etc/passwd");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("outside the allowed data directory"),
            "Error should mention path confinement: {}",
            err
        );
    }

    #[test]
    fn test_validate_path_blocks_dot_dot_traversal() {
        let dir = tempdir().unwrap();
        // Create a subdir so ../.. resolves to something real
        let subdir = dir.path().join("sub");
        std::fs::create_dir_all(&subdir).unwrap();
        let handler = handler_with_data_dir(&subdir);

        let result = handler.validate_path("../../../etc/passwd");
        assert!(result.is_err(), "Should block ../ traversal: {:?}", result);
    }

    #[test]
    fn test_validate_path_blocks_dot_dot_in_middle() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        // Create the inner directory
        std::fs::create_dir_all(dir.path().join("a")).unwrap();

        let result = handler.validate_path("a/../../etc/passwd");
        assert!(result.is_err(), "Should block ../  in the middle of path");
    }

    #[test]
    fn test_validate_path_allows_subdirectory_within_data_dir() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        // Create subdirectory
        std::fs::create_dir_all(dir.path().join("backups")).unwrap();

        let result = handler.validate_path("backups/mydb.bak");
        assert!(
            result.is_ok(),
            "Should allow path in subdirectory: {:?}",
            result
        );
        assert!(result.unwrap().starts_with(dir.path()));
    }

    #[test]
    fn test_validate_path_allows_new_file_in_data_dir() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        let result = handler.validate_path("new_database.db");
        assert!(
            result.is_ok(),
            "Should allow new file in data dir: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_blocks_absolute_path_to_etc() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        // Test all 3 POCs from the issue
        for path in &["/etc/passwd", "/etc/shadow", "/etc/hosts"] {
            let result = handler.validate_path(path);
            assert!(result.is_err(), "Should block {}", path);
        }
    }

    #[test]
    fn test_validate_path_blocks_home_ssh_keys() {
        let dir = tempdir().unwrap();
        let handler = handler_with_data_dir(dir.path());

        let result = handler.validate_path("~/.ssh/id_rsa");
        // This is a relative path so it won't expand ~, but test the principle
        let result2 = handler.validate_path("/root/.ssh/id_rsa");
        assert!(result2.is_err(), "Should block /root/.ssh/id_rsa");
    }
}
