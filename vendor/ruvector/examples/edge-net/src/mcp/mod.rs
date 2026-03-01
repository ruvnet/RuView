//! Browser-Based MCP (Model Context Protocol) for Edge-Net
//!
//! Exposes edge-net capabilities as MCP tools accessible from browsers.
//! Uses MessagePort/BroadcastChannel for cross-context communication.
//!
//! ## Usage in JavaScript
//!
//! ```javascript
//! import init, { WasmMcpServer } from '@ruvector/edge-net';
//!
//! await init();
//!
//! // Create MCP server
//! const mcp = new WasmMcpServer();
//!
//! // Handle MCP requests
//! const response = await mcp.handleRequest({
//!   jsonrpc: "2.0",
//!   id: 1,
//!   method: "tools/call",
//!   params: {
//!     name: "vector_search",
//!     arguments: { query: [0.1, 0.2, 0.3], k: 10 }
//!   }
//! });
//!
//! // Or use with a WebWorker
//! const worker = new Worker('edge-net-worker.js');
//! mcp.attachToWorker(worker);
//! ```

mod protocol;
mod handlers;
mod transport;

pub use protocol::*;
pub use handlers::*;
pub use transport::*;

use wasm_bindgen::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use parking_lot::RwLock;

use crate::identity::WasmNodeIdentity;
use crate::credits::WasmCreditLedger;
use crate::rac::CoherenceEngine;
use crate::learning::NetworkLearning;

/// Security constants
const MAX_PAYLOAD_SIZE: usize = 1_048_576; // 1MB max payload
const RATE_LIMIT_WINDOW_MS: u64 = 1000; // 1 second window
const RATE_LIMIT_MAX_REQUESTS: u64 = 100; // max 100 requests per window
const MAX_VECTOR_K: usize = 100; // max k for vector searches

/// Browser-based MCP server for edge-net
///
/// Provides Model Context Protocol interface over MessagePort or direct calls.
/// All edge-net capabilities are exposed as MCP tools.
#[wasm_bindgen]
pub struct WasmMcpServer {
    /// Identity for signing responses
    identity: Option<WasmNodeIdentity>,
    /// Credit ledger for economic operations
    ledger: Arc<RwLock<WasmCreditLedger>>,
    /// RAC coherence engine
    coherence: Arc<RwLock<CoherenceEngine>>,
    /// Learning engine for patterns
    learning: Option<NetworkLearning>,
    /// Server configuration
    config: McpServerConfig,
    /// Request counter for IDs
    request_counter: Arc<RwLock<u64>>,
    /// Rate limiting: (window_start_ms, request_count)
    rate_limit: Arc<RwLock<(u64, u64)>>,
}

/// MCP server configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Server name
    pub name: String,
    /// Protocol version
    pub version: String,
    /// Enable debug logging
    pub debug: bool,
    /// Maximum concurrent requests
    pub max_concurrent: usize,
    /// Maximum payload size in bytes
    pub max_payload_size: usize,
    /// Rate limit: max requests per second
    pub rate_limit_per_second: u64,
    /// Require authentication for credit operations
    pub require_auth_for_credits: bool,
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            name: "edge-net-mcp".to_string(),
            version: "2024-11-05".to_string(),
            debug: false,
            max_concurrent: 16,
            max_payload_size: MAX_PAYLOAD_SIZE,
            rate_limit_per_second: RATE_LIMIT_MAX_REQUESTS,
            require_auth_for_credits: true, // Secure by default
        }
    }
}

#[wasm_bindgen]
impl WasmMcpServer {
    /// Create a new MCP server with default configuration
    #[wasm_bindgen(constructor)]
    pub fn new() -> Result<WasmMcpServer, JsValue> {
        #[cfg(feature = "console_error_panic_hook")]
        console_error_panic_hook::set_once();

        // Generate a temporary node ID for the ledger
        let node_id = uuid::Uuid::new_v4().to_string();

        Ok(Self {
            identity: None,
            ledger: Arc::new(RwLock::new(WasmCreditLedger::new(node_id).map_err(|e| e)?)),
            coherence: Arc::new(RwLock::new(CoherenceEngine::new())),
            learning: None,
            config: McpServerConfig::default(),
            request_counter: Arc::new(RwLock::new(0)),
            rate_limit: Arc::new(RwLock::new((0, 0))),
        })
    }

    /// Create with custom configuration
    #[wasm_bindgen(js_name = withConfig)]
    pub fn with_config(config: JsValue) -> Result<WasmMcpServer, JsValue> {
        let config: McpServerConfig = serde_wasm_bindgen::from_value(config)?;

        let mut server = Self::new()?;
        server.config = config;
        Ok(server)
    }

    /// Set identity for authenticated operations
    #[wasm_bindgen(js_name = setIdentity)]
    pub fn set_identity(&mut self, identity: WasmNodeIdentity) {
        self.identity = Some(identity);
    }

    /// Initialize learning engine
    #[wasm_bindgen(js_name = initLearning)]
    pub fn init_learning(&mut self) -> Result<(), JsValue> {
        self.learning = Some(NetworkLearning::new());
        Ok(())
    }

    /// Handle an MCP request (JSON string)
    #[wasm_bindgen(js_name = handleRequest)]
    pub async fn handle_request(&self, request_json: &str) -> Result<String, JsValue> {
        // SECURITY: Check payload size before parsing (prevent DoS)
        if request_json.len() > self.config.max_payload_size {
            return Err(JsValue::from_str(&format!(
                "Payload too large: {} bytes exceeds {} limit",
                request_json.len(),
                self.config.max_payload_size
            )));
        }

        // SECURITY: Check rate limit
        if let Err(e) = self.check_rate_limit() {
            return Err(JsValue::from_str(&e));
        }

        let request: McpRequest = serde_json::from_str(request_json)
            .map_err(|e| JsValue::from_str(&format!("Parse error: {}", e)))?;

        let response = self.process_request(request).await;

        serde_json::to_string(&response)
            .map_err(|e| JsValue::from_str(&format!("Serialize error: {}", e)))
    }

    /// Handle MCP request from JsValue (for direct JS calls)
    #[wasm_bindgen(js_name = handleRequestJs)]
    pub async fn handle_request_js(&self, request: JsValue) -> Result<JsValue, JsValue> {
        let request: McpRequest = serde_wasm_bindgen::from_value(request)?;
        let response = self.process_request(request).await;
        serde_wasm_bindgen::to_value(&response)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Check rate limit - returns error if limit exceeded
    fn check_rate_limit(&self) -> Result<(), String> {
        let now = js_sys::Date::now() as u64;
        let mut rate_limit = self.rate_limit.write();

        let (window_start, count) = *rate_limit;

        // Check if we're in a new window
        if now - window_start > RATE_LIMIT_WINDOW_MS {
            // New window
            *rate_limit = (now, 1);
            Ok(())
        } else if count >= self.config.rate_limit_per_second {
            // Rate limit exceeded
            Err(format!(
                "Rate limit exceeded: {} requests per second",
                self.config.rate_limit_per_second
            ))
        } else {
            // Increment counter
            *rate_limit = (window_start, count + 1);
            Ok(())
        }
    }

    /// Check if identity is set (for authenticated operations)
    fn require_identity(&self) -> Result<&WasmNodeIdentity, McpError> {
        self.identity.as_ref().ok_or_else(|| {
            McpError::new(
                ErrorCodes::INVALID_PARAMS,
                "Authentication required: set identity with setIdentity() first",
            )
        })
    }

    /// Process MCP request internally
    async fn process_request(&self, request: McpRequest) -> McpResponse {
        // Increment request counter
        {
            let mut counter = self.request_counter.write();
            *counter += 1;
        }

        match request.method.as_str() {
            "initialize" => self.handle_initialize(request.id),
            "tools/list" => self.handle_tools_list(request.id),
            "tools/call" => self.handle_tools_call(request.id, request.params).await,
            "resources/list" => self.handle_resources_list(request.id),
            "resources/read" => self.handle_resources_read(request.id, request.params),
            "prompts/list" => self.handle_prompts_list(request.id),
            "prompts/get" => self.handle_prompts_get(request.id, request.params),
            _ => McpResponse::error(
                request.id,
                McpError::new(ErrorCodes::METHOD_NOT_FOUND, "Method not found"),
            ),
        }
    }

    /// Handle initialize request
    fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(
            id,
            json!({
                "protocolVersion": self.config.version,
                "capabilities": {
                    "tools": {
                        "listChanged": true
                    },
                    "resources": {
                        "subscribe": true,
                        "listChanged": true
                    },
                    "prompts": {
                        "listChanged": true
                    },
                    "logging": {}
                },
                "serverInfo": {
                    "name": self.config.name,
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        )
    }

    /// Handle tools/list request
    fn handle_tools_list(&self, id: Option<Value>) -> McpResponse {
        let tools = self.get_available_tools();
        McpResponse::success(id, json!({ "tools": tools }))
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Missing params"),
            ),
        };

        let tool_name = params.get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let arguments = params.get("arguments")
            .cloned()
            .unwrap_or(json!({}));

        match tool_name {
            // Identity tools
            "identity_generate" => self.tool_identity_generate(id, arguments),
            "identity_sign" => self.tool_identity_sign(id, arguments),
            "identity_verify" => self.tool_identity_verify(id, arguments),

            // Credit/Economic tools
            "credits_balance" => self.tool_credits_balance(id, arguments),
            "credits_contribute" => self.tool_credits_contribute(id, arguments),
            "credits_spend" => self.tool_credits_spend(id, arguments),
            "credits_health" => self.tool_credits_health(id),

            // RAC/Coherence tools
            "rac_ingest" => self.tool_rac_ingest(id, arguments),
            "rac_stats" => self.tool_rac_stats(id),
            "rac_merkle_root" => self.tool_rac_merkle_root(id),

            // Learning tools
            "learning_store_pattern" => self.tool_learning_store(id, arguments),
            "learning_lookup" => self.tool_learning_lookup(id, arguments),
            "learning_stats" => self.tool_learning_stats(id),

            // Task tools
            "task_submit" => self.tool_task_submit(id, arguments).await,
            "task_status" => self.tool_task_status(id, arguments),

            // Network tools
            "network_peers" => self.tool_network_peers(id),
            "network_stats" => self.tool_network_stats(id),

            _ => McpResponse::error(
                id,
                McpError::new(ErrorCodes::METHOD_NOT_FOUND, format!("Unknown tool: {}", tool_name)),
            ),
        }
    }

    /// Handle resources/list request
    fn handle_resources_list(&self, id: Option<Value>) -> McpResponse {
        let resources = vec![
            McpResource {
                uri: "edge-net://identity".to_string(),
                name: "Node Identity".to_string(),
                description: "Current node identity and public key".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "edge-net://ledger".to_string(),
                name: "Credit Ledger".to_string(),
                description: "CRDT-based credit ledger state".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "edge-net://coherence".to_string(),
                name: "RAC State".to_string(),
                description: "Adversarial coherence protocol state".to_string(),
                mime_type: "application/json".to_string(),
            },
            McpResource {
                uri: "edge-net://learning".to_string(),
                name: "Learning Patterns".to_string(),
                description: "Stored learning patterns and trajectories".to_string(),
                mime_type: "application/json".to_string(),
            },
        ];

        McpResponse::success(id, json!({ "resources": resources }))
    }

    /// Handle resources/read request
    fn handle_resources_read(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let uri = params
            .as_ref()
            .and_then(|p| p.get("uri"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match uri {
            "edge-net://identity" => {
                let content = match &self.identity {
                    Some(id) => json!({
                        "nodeId": id.node_id(),
                        "siteId": id.site_id(),
                        "publicKey": id.public_key_hex(),
                    }),
                    None => json!({ "status": "not_initialized" }),
                };
                McpResponse::success(id, json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": content.to_string()
                    }]
                }))
            }
            "edge-net://ledger" => {
                let ledger = self.ledger.read();
                let stats = json!({
                    "balance": ledger.balance(),
                    "totalEarned": ledger.total_earned(),
                    "totalSpent": ledger.total_spent(),
                });
                McpResponse::success(id, json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": stats.to_string()
                    }]
                }))
            }
            "edge-net://coherence" => {
                let coherence = self.coherence.read();
                let stats = json!({
                    "eventCount": coherence.event_count(),
                    "conflictCount": coherence.conflict_count(),
                    "quarantinedCount": coherence.quarantined_count(),
                });
                McpResponse::success(id, json!({
                    "contents": [{
                        "uri": uri,
                        "mimeType": "application/json",
                        "text": stats.to_string()
                    }]
                }))
            }
            _ => McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Unknown resource: {}", uri)),
            ),
        }
    }

    /// Handle prompts/list request
    fn handle_prompts_list(&self, id: Option<Value>) -> McpResponse {
        let prompts = vec![
            McpPrompt {
                name: "analyze_network".to_string(),
                description: "Analyze edge-net network health and suggest optimizations".to_string(),
                arguments: Some(vec![
                    PromptArgument {
                        name: "focus".to_string(),
                        description: "Focus area: performance, security, or economics".to_string(),
                        required: false,
                    }
                ]),
            },
            McpPrompt {
                name: "debug_coherence".to_string(),
                description: "Debug RAC coherence issues and conflicts".to_string(),
                arguments: None,
            },
        ];

        McpResponse::success(id, json!({ "prompts": prompts }))
    }

    /// Handle prompts/get request
    fn handle_prompts_get(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let name = params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        match name {
            "analyze_network" => {
                let coherence = self.coherence.read();
                let ledger = self.ledger.read();

                McpResponse::success(id, json!({
                    "description": "Network analysis prompt",
                    "messages": [{
                        "role": "user",
                        "content": {
                            "type": "text",
                            "text": format!(
                                "Analyze this edge-net node:\n\
                                - Events: {}\n\
                                - Conflicts: {}\n\
                                - Balance: {} credits\n\
                                - Earned: {} | Spent: {}\n\n\
                                Suggest optimizations for performance and reliability.",
                                coherence.event_count(),
                                coherence.conflict_count(),
                                ledger.balance(),
                                ledger.total_earned(),
                                ledger.total_spent()
                            )
                        }
                    }]
                }))
            }
            _ => McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Unknown prompt: {}", name)),
            ),
        }
    }

    /// Get list of available tools
    fn get_available_tools(&self) -> Vec<McpTool> {
        vec![
            // Identity tools
            McpTool {
                name: "identity_generate".to_string(),
                description: "Generate a new node identity with Ed25519 keypair".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "site_id": { "type": "string", "description": "Site identifier" }
                    },
                    "required": ["site_id"]
                }),
            },
            McpTool {
                name: "identity_sign".to_string(),
                description: "Sign a message with the node's private key".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string", "description": "Message to sign (base64)" }
                    },
                    "required": ["message"]
                }),
            },
            McpTool {
                name: "identity_verify".to_string(),
                description: "Verify a signature from any node".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "public_key": { "type": "string", "description": "Public key (hex)" },
                        "message": { "type": "string", "description": "Original message (base64)" },
                        "signature": { "type": "string", "description": "Signature (hex)" }
                    },
                    "required": ["public_key", "message", "signature"]
                }),
            },

            // Credit tools
            McpTool {
                name: "credits_balance".to_string(),
                description: "Get credit balance for a node".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "node_id": { "type": "string", "description": "Node ID to check" }
                    },
                    "required": ["node_id"]
                }),
            },
            McpTool {
                name: "credits_contribute".to_string(),
                description: "Record a compute contribution and earn credits".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "amount": { "type": "number", "description": "Contribution amount" },
                        "task_type": { "type": "string", "description": "Type of task completed" }
                    },
                    "required": ["amount"]
                }),
            },
            McpTool {
                name: "credits_spend".to_string(),
                description: "Spend credits on a task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "amount": { "type": "number", "description": "Amount to spend" },
                        "purpose": { "type": "string", "description": "What the credits are for" }
                    },
                    "required": ["amount"]
                }),
            },
            McpTool {
                name: "credits_health".to_string(),
                description: "Get economic health metrics for the network".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },

            // RAC tools
            McpTool {
                name: "rac_ingest".to_string(),
                description: "Ingest an event into the coherence engine".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "event": { "type": "object", "description": "Event to ingest" }
                    },
                    "required": ["event"]
                }),
            },
            McpTool {
                name: "rac_stats".to_string(),
                description: "Get RAC coherence statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpTool {
                name: "rac_merkle_root".to_string(),
                description: "Get current Merkle root of event log".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },

            // Learning tools
            McpTool {
                name: "learning_store_pattern".to_string(),
                description: "Store a learned pattern with embedding".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "embedding": { "type": "array", "items": { "type": "number" } },
                        "metadata": { "type": "object" }
                    },
                    "required": ["embedding"]
                }),
            },
            McpTool {
                name: "learning_lookup".to_string(),
                description: "Lookup similar patterns".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "array", "items": { "type": "number" } },
                        "k": { "type": "integer", "default": 5 }
                    },
                    "required": ["query"]
                }),
            },
            McpTool {
                name: "learning_stats".to_string(),
                description: "Get learning engine statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },

            // Task tools
            McpTool {
                name: "task_submit".to_string(),
                description: "Submit a compute task to the network".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task_type": {
                            "type": "string",
                            "enum": ["vector_search", "embedding", "semantic_match", "neural", "encryption", "compression"]
                        },
                        "payload": { "type": "object" },
                        "max_cost": { "type": "number" }
                    },
                    "required": ["task_type", "payload"]
                }),
            },
            McpTool {
                name: "task_status".to_string(),
                description: "Check status of a submitted task".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "task_id": { "type": "string" }
                    },
                    "required": ["task_id"]
                }),
            },

            // Network tools
            McpTool {
                name: "network_peers".to_string(),
                description: "Get list of connected peers".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            McpTool {
                name: "network_stats".to_string(),
                description: "Get network statistics".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ]
    }

    // ========================================================================
    // Tool Implementations
    // ========================================================================

    fn tool_identity_generate(&self, id: Option<Value>, args: Value) -> McpResponse {
        let site_id = args.get("site_id")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        match WasmNodeIdentity::generate(site_id) {
            Ok(identity) => {
                McpResponse::success(id, json!({
                    "content": [{
                        "type": "text",
                        "text": format!(
                            "Generated identity:\n- Node ID: {}\n- Public Key: {}",
                            identity.node_id(),
                            identity.public_key_hex()
                        )
                    }],
                    "nodeId": identity.node_id(),
                    "publicKey": identity.public_key_hex()
                }))
            }
            Err(e) => McpResponse::error(
                id,
                McpError::new(ErrorCodes::INTERNAL_ERROR, format!("Failed to generate identity: {:?}", e)),
            ),
        }
    }

    fn tool_identity_sign(&self, id: Option<Value>, args: Value) -> McpResponse {
        let identity = match &self.identity {
            Some(i) => i,
            None => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "No identity set"),
            ),
        };

        let message_b64 = args.get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let message = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, message_b64) {
            Ok(m) => m,
            Err(e) => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Invalid base64: {}", e)),
            ),
        };

        let signature = identity.sign(&message);
        let sig_hex = hex::encode(&signature);

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Signature: {}", sig_hex)
            }],
            "signature": sig_hex
        }))
    }

    fn tool_identity_verify(&self, id: Option<Value>, args: Value) -> McpResponse {
        let public_key_hex = args.get("public_key").and_then(|v| v.as_str()).unwrap_or("");
        let message_b64 = args.get("message").and_then(|v| v.as_str()).unwrap_or("");
        let signature_hex = args.get("signature").and_then(|v| v.as_str()).unwrap_or("");

        let public_key = match hex::decode(public_key_hex) {
            Ok(k) => k,
            Err(e) => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Invalid public key hex: {}", e)),
            ),
        };

        let message = match base64::Engine::decode(&base64::engine::general_purpose::STANDARD, message_b64) {
            Ok(m) => m,
            Err(e) => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Invalid message base64: {}", e)),
            ),
        };

        let signature = match hex::decode(signature_hex) {
            Ok(s) => s,
            Err(e) => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, format!("Invalid signature hex: {}", e)),
            ),
        };

        let valid = WasmNodeIdentity::verify_from(&public_key, &message, &signature);

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": if valid { "Signature is valid ✓" } else { "Signature is INVALID ✗" }
            }],
            "valid": valid
        }))
    }

    fn tool_credits_balance(&self, id: Option<Value>, _args: Value) -> McpResponse {
        let ledger = self.ledger.read();
        let balance = ledger.balance();
        let earned = ledger.total_earned();
        let spent = ledger.total_spent();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Balance: {} rUv (earned: {}, spent: {})", balance, earned, spent)
            }],
            "balance": balance,
            "totalEarned": earned,
            "totalSpent": spent
        }))
    }

    fn tool_credits_contribute(&self, id: Option<Value>, args: Value) -> McpResponse {
        // SECURITY: Require authentication for credit operations
        if self.config.require_auth_for_credits {
            if let Err(e) = self.require_identity() {
                return McpResponse::error(id, e);
            }
        }

        let amount = args.get("amount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // SECURITY: Validate amount bounds
        if amount < 0.0 || amount > u64::MAX as f64 {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Invalid amount: must be non-negative"),
            );
        }
        let amount = amount as u64;

        // SECURITY: Limit max credit per transaction
        const MAX_CREDIT_PER_TX: u64 = 1_000_000;
        if amount > MAX_CREDIT_PER_TX {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS,
                    format!("Amount {} exceeds max {} per transaction", amount, MAX_CREDIT_PER_TX)),
            );
        }

        let task_type = args.get("task_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let mut ledger = self.ledger.write();
        if let Err(e) = ledger.credit(amount, task_type) {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INTERNAL_ERROR, "Credit operation failed"),
            );
        }

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Contributed {} rUv for {} task", amount, task_type)
            }],
            "credited": amount,
            "newBalance": ledger.balance()
        }))
    }

    fn tool_credits_spend(&self, id: Option<Value>, args: Value) -> McpResponse {
        // SECURITY: Require authentication for credit operations
        if self.config.require_auth_for_credits {
            if let Err(e) = self.require_identity() {
                return McpResponse::error(id, e);
            }
        }

        let amount = args.get("amount")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        // SECURITY: Validate amount bounds
        if amount < 0.0 || amount > u64::MAX as f64 {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Invalid amount"),
            );
        }
        let amount = amount as u64;

        let purpose = args.get("purpose")
            .and_then(|v| v.as_str())
            .unwrap_or("task");

        let mut ledger = self.ledger.write();
        let current_balance = ledger.balance();

        if current_balance < amount {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Insufficient balance"),
            );
        }

        if let Err(_) = ledger.deduct(amount) {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INTERNAL_ERROR, "Deduct operation failed"),
            );
        }

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Spent {} rUv on {}", amount, purpose)
            }],
            "spent": amount,
            "newBalance": ledger.balance(),
            "purpose": purpose
        }))
    }

    fn tool_credits_health(&self, id: Option<Value>) -> McpResponse {
        let ledger = self.ledger.read();
        let balance = ledger.balance();
        let earned = ledger.total_earned();
        let spent = ledger.total_spent();
        let staked = ledger.staked_amount();
        let multiplier = ledger.current_multiplier();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "Economic Health:\n- Balance: {} rUv\n- Earned: {}\n- Spent: {}\n- Staked: {}\n- Multiplier: {}x",
                    balance, earned, spent, staked, multiplier
                )
            }],
            "balance": balance,
            "totalEarned": earned,
            "totalSpent": spent,
            "staked": staked,
            "multiplier": multiplier
        }))
    }

    fn tool_rac_ingest(&self, id: Option<Value>, _args: Value) -> McpResponse {
        // Simplified - would parse event from args
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": "Event ingestion requires proper Event struct parsing"
            }],
            "status": "not_implemented"
        }))
    }

    fn tool_rac_stats(&self, id: Option<Value>) -> McpResponse {
        let coherence = self.coherence.read();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!(
                    "RAC Statistics:\n- Events: {}\n- Conflicts: {}\n- Quarantined: {}",
                    coherence.event_count(),
                    coherence.conflict_count(),
                    coherence.quarantined_count()
                )
            }],
            "eventCount": coherence.event_count(),
            "conflictCount": coherence.conflict_count(),
            "quarantinedCount": coherence.quarantined_count()
        }))
    }

    fn tool_rac_merkle_root(&self, id: Option<Value>) -> McpResponse {
        let coherence = self.coherence.read();
        let root = coherence.get_merkle_root();
        let root_hex = hex::encode(&root);

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Merkle Root: {}", root_hex)
            }],
            "merkleRoot": root_hex
        }))
    }

    fn tool_learning_store(&self, id: Option<Value>, args: Value) -> McpResponse {
        let learning = match &self.learning {
            Some(l) => l,
            None => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Learning engine not initialized"),
            ),
        };

        // The learning engine expects a JSON string with pattern data
        let pattern_json = serde_json::to_string(&args).unwrap_or_default();

        let pattern_id = learning.store_pattern(&pattern_json);

        if pattern_id < 0 {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Invalid pattern format"),
            );
        }

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Stored pattern with ID {}", pattern_id)
            }],
            "patternId": pattern_id
        }))
    }

    fn tool_learning_lookup(&self, id: Option<Value>, args: Value) -> McpResponse {
        let learning = match &self.learning {
            Some(l) => l,
            None => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Learning engine not initialized"),
            ),
        };

        let query: Vec<f32> = args.get("query")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64().map(|f| f as f32)).collect())
            .unwrap_or_default();

        let k = args.get("k")
            .and_then(|v| v.as_u64())
            .unwrap_or(5) as usize;

        // SECURITY: Limit k to prevent memory exhaustion
        let k = k.min(MAX_VECTOR_K);

        if query.is_empty() {
            return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Empty query"),
            );
        }

        // SECURITY: Validate vector dimensions (prevent NaN/Infinity)
        for val in &query {
            if !val.is_finite() {
                return McpResponse::error(
                    id,
                    McpError::new(ErrorCodes::INVALID_PARAMS, "Invalid vector values"),
                );
            }
        }

        // Convert query to JSON for the learning engine
        let query_json = serde_json::to_string(&query).unwrap_or("[]".to_string());
        let results = learning.lookup_patterns(&query_json, k);

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Found {} similar patterns", results.len())
            }],
            "results": results
        }))
    }

    fn tool_learning_stats(&self, id: Option<Value>) -> McpResponse {
        let learning = match &self.learning {
            Some(l) => l,
            None => return McpResponse::error(
                id,
                McpError::new(ErrorCodes::INVALID_PARAMS, "Learning engine not initialized"),
            ),
        };

        let stats = learning.get_stats();

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Learning Stats:\n{}", stats)
            }],
            "stats": stats
        }))
    }

    async fn tool_task_submit(&self, id: Option<Value>, args: Value) -> McpResponse {
        let task_type = args.get("task_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let _payload = args.get("payload").cloned().unwrap_or(json!({}));
        let max_cost = args.get("max_cost")
            .and_then(|v| v.as_f64())
            .unwrap_or(10.0) as u64;

        // Generate task ID
        let task_id = format!("task-{}", uuid::Uuid::new_v4());

        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Task {} submitted (type: {}, max_cost: {} rUv)", task_id, task_type, max_cost)
            }],
            "taskId": task_id,
            "status": "queued",
            "estimatedCost": max_cost / 2
        }))
    }

    fn tool_task_status(&self, id: Option<Value>, args: Value) -> McpResponse {
        let task_id = args.get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Would look up actual task status
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": format!("Task {} status: pending", task_id)
            }],
            "taskId": task_id,
            "status": "pending",
            "progress": 0.0
        }))
    }

    fn tool_network_peers(&self, id: Option<Value>) -> McpResponse {
        // Would return actual connected peers
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": "Connected peers: 0 (P2P not yet implemented)"
            }],
            "peers": [],
            "count": 0
        }))
    }

    fn tool_network_stats(&self, id: Option<Value>) -> McpResponse {
        McpResponse::success(id, json!({
            "content": [{
                "type": "text",
                "text": "Network stats:\n- Connected: false\n- Peers: 0"
            }],
            "connected": false,
            "peerCount": 0,
            "messagesSent": 0,
            "messagesReceived": 0
        }))
    }

    /// Get server info
    #[wasm_bindgen(js_name = getServerInfo)]
    pub fn get_server_info(&self) -> JsValue {
        let info = json!({
            "name": self.config.name,
            "version": env!("CARGO_PKG_VERSION"),
            "protocolVersion": self.config.version,
            "toolCount": self.get_available_tools().len(),
            "hasIdentity": self.identity.is_some(),
            "hasLearning": self.learning.is_some()
        });

        JsValue::from_str(&info.to_string())
    }
}

impl Default for WasmMcpServer {
    fn default() -> Self {
        Self::new().expect("Failed to create default MCP server")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let server = WasmMcpServer::new().unwrap();
        assert!(!server.config.name.is_empty());
    }

    #[test]
    fn test_tools_list() {
        let server = WasmMcpServer::new().unwrap();
        let tools = server.get_available_tools();
        assert!(!tools.is_empty());
        assert!(tools.iter().any(|t| t.name == "credits_balance"));
    }
}
