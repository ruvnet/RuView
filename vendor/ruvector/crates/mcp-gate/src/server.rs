//! MCP protocol server implementation
//!
//! Implements the Model Context Protocol for stdio-based communication
//! with AI agents and tool orchestrators.

use crate::tools::McpGateTools;
use crate::types::*;
use cognitum_gate_tilezero::{GateThresholds, TileZero};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// MCP Gate Server
pub struct McpGateServer {
    /// Tools handler
    tools: McpGateTools,
    /// Server info
    server_info: ServerInfo,
}

/// Server information
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerInfo {
    /// Server name
    pub name: String,
    /// Server version
    pub version: String,
    /// Protocol version
    pub protocol_version: String,
}

impl Default for ServerInfo {
    fn default() -> Self {
        Self {
            name: "mcp-gate".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: "2024-11-05".to_string(),
        }
    }
}

/// Server capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServerCapabilities {
    /// Tool capabilities
    pub tools: ToolCapabilities,
}

/// Tool capabilities
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolCapabilities {
    /// Whether tool listing changes are supported
    #[serde(rename = "listChanged")]
    pub list_changed: bool,
}

impl Default for ServerCapabilities {
    fn default() -> Self {
        Self {
            tools: ToolCapabilities {
                list_changed: false,
            },
        }
    }
}

impl McpGateServer {
    /// Create a new server with default configuration
    pub fn new() -> Self {
        let thresholds = GateThresholds::default();
        let tilezero = Arc::new(RwLock::new(TileZero::new(thresholds)));
        Self {
            tools: McpGateTools::new(tilezero),
            server_info: ServerInfo::default(),
        }
    }

    /// Create a new server with custom thresholds
    pub fn with_thresholds(thresholds: GateThresholds) -> Self {
        let tilezero = Arc::new(RwLock::new(TileZero::new(thresholds)));
        Self {
            tools: McpGateTools::new(tilezero),
            server_info: ServerInfo::default(),
        }
    }

    /// Create a new server with a shared TileZero instance
    pub fn with_tilezero(tilezero: Arc<RwLock<TileZero>>) -> Self {
        Self {
            tools: McpGateTools::new(tilezero),
            server_info: ServerInfo::default(),
        }
    }

    /// Run the server on stdio
    pub async fn run_stdio(&self) -> Result<(), std::io::Error> {
        info!("Starting MCP Gate server on stdio");

        let stdin = tokio::io::stdin();
        let mut stdout = tokio::io::stdout();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        while let Ok(Some(line)) = lines.next_line().await {
            if line.trim().is_empty() {
                continue;
            }

            debug!("Received: {}", line);

            let response = self.handle_message(&line).await;

            if let Some(resp) = response {
                let resp_json = serde_json::to_string(&resp).unwrap_or_default();
                debug!("Sending: {}", resp_json);
                stdout.write_all(resp_json.as_bytes()).await?;
                stdout.write_all(b"\n").await?;
                stdout.flush().await?;
            }
        }

        info!("MCP Gate server shutting down");
        Ok(())
    }

    /// Handle a single message
    async fn handle_message(&self, message: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(message) {
            Ok(req) => req,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                return Some(JsonRpcResponse::error(
                    serde_json::Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                ));
            }
        };

        let result = self.handle_request(&request).await;
        Some(result)
    }

    /// Handle a JSON-RPC request
    async fn handle_request(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        match request.method.as_str() {
            "initialize" => self.handle_initialize(request),
            "initialized" => {
                // Notification, no response needed
                JsonRpcResponse::success(request.id.clone(), serde_json::json!({}))
            }
            "tools/list" => self.handle_tools_list(request),
            "tools/call" => self.handle_tools_call(request).await,
            "shutdown" => {
                info!("Received shutdown request");
                JsonRpcResponse::success(request.id.clone(), serde_json::json!({}))
            }
            _ => {
                warn!("Unknown method: {}", request.method);
                JsonRpcResponse::error(
                    request.id.clone(),
                    -32601,
                    format!("Method not found: {}", request.method),
                )
            }
        }
    }

    /// Handle initialize request
    fn handle_initialize(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling initialize request");

        let result = serde_json::json!({
            "protocolVersion": self.server_info.protocol_version,
            "capabilities": ServerCapabilities::default(),
            "serverInfo": {
                "name": self.server_info.name,
                "version": self.server_info.version
            }
        });

        JsonRpcResponse::success(request.id.clone(), result)
    }

    /// Handle tools/list request
    fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling tools/list request");

        let tools = McpGateTools::list_tools();
        let result = serde_json::json!({
            "tools": tools
        });

        JsonRpcResponse::success(request.id.clone(), result)
    }

    /// Handle tools/call request
    async fn handle_tools_call(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        info!("Handling tools/call request");

        // Parse the tool call from params
        let tool_call: McpToolCall = match serde_json::from_value(request.params.clone()) {
            Ok(tc) => tc,
            Err(e) => {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    -32602,
                    format!("Invalid params: {}", e),
                );
            }
        };

        // Call the tool
        match self.tools.call_tool(tool_call).await {
            Ok(result) => {
                let response_content = match result {
                    McpToolResult::Success { content } => serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": serde_json::to_string_pretty(&content).unwrap_or_default()
                        }]
                    }),
                    McpToolResult::Error { error } => serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": error
                        }],
                        "isError": true
                    }),
                };
                JsonRpcResponse::success(request.id.clone(), response_content)
            }
            Err(e) => JsonRpcResponse::error(request.id.clone(), e.code(), e.to_string()),
        }
    }
}

impl Default for McpGateServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration for the MCP Gate server
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct McpGateConfig {
    /// Gate thresholds
    #[serde(default)]
    pub thresholds: GateThresholds,
    /// Log level
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for McpGateConfig {
    fn default() -> Self {
        Self {
            thresholds: GateThresholds::default(),
            log_level: default_log_level(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_info_default() {
        let info = ServerInfo::default();
        assert_eq!(info.name, "mcp-gate");
        assert_eq!(info.protocol_version, "2024-11-05");
    }

    #[test]
    fn test_server_capabilities_default() {
        let caps = ServerCapabilities::default();
        assert!(!caps.tools.list_changed);
    }

    #[tokio::test]
    async fn test_handle_initialize() {
        let server = McpGateServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "initialize".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());

        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], "2024-11-05");
    }

    #[tokio::test]
    async fn test_handle_tools_list() {
        let server = McpGateServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "tools/list".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request).await;
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let tools = result["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
    }

    #[tokio::test]
    async fn test_handle_tools_call() {
        let server = McpGateServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": "permit_action",
                "arguments": {
                    "action_id": "test-1",
                    "action_type": "config_change"
                }
            }),
        };

        let response = server.handle_request(&request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_handle_unknown_method() {
        let server = McpGateServer::new();
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::json!(1),
            method: "unknown/method".to_string(),
            params: serde_json::json!({}),
        };

        let response = server.handle_request(&request).await;
        assert!(response.error.is_some());
        assert_eq!(response.error.unwrap().code, -32601);
    }
}
