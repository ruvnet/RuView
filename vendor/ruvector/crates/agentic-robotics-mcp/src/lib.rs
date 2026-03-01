//! Model Context Protocol (MCP) Server for Agentic Robotics
//!
//! Provides MCP 2025-11 compliant server with stdio and SSE transports
//! for exposing robot capabilities to AI assistants.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod transport;
pub mod server;

/// MCP Protocol version
pub const MCP_VERSION: &str = "2025-11-15";

/// MCP Tool definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// MCP Request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

/// MCP Response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP Error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// Tool execution result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub content: Vec<ContentItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

/// Content item in response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentItem {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "resource")]
    Resource { uri: String, mimeType: String, data: String },
    #[serde(rename = "image")]
    Image { data: String, mimeType: String },
}

/// Tool handler function type
pub type ToolHandler = Arc<dyn Fn(Value) -> Result<ToolResult> + Send + Sync>;

/// MCP Server implementation
pub struct McpServer {
    tools: Arc<RwLock<HashMap<String, (McpTool, ToolHandler)>>>,
    server_info: ServerInfo,
}

/// Server information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl McpServer {
    /// Create a new MCP server
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
            server_info: ServerInfo {
                name: name.into(),
                version: version.into(),
                description: Some("Agentic Robotics MCP Server".to_string()),
            },
        }
    }

    /// Register a tool
    pub async fn register_tool(
        &self,
        tool: McpTool,
        handler: ToolHandler,
    ) -> Result<()> {
        let mut tools = self.tools.write().await;
        tools.insert(tool.name.clone(), (tool, handler));
        Ok(())
    }

    /// Handle MCP request
    pub async fn handle_request(&self, request: McpRequest) -> McpResponse {
        let id = request.id.clone();

        match request.method.as_str() {
            "initialize" => self.handle_initialize(id).await,
            "tools/list" => self.handle_list_tools(id).await,
            "tools/call" => self.handle_call_tool(id, request.params).await,
            _ => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(McpError {
                    code: -32601,
                    message: "Method not found".to_string(),
                    data: None,
                }),
            },
        }
    }

    async fn handle_initialize(&self, id: Option<Value>) -> McpResponse {
        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "protocolVersion": MCP_VERSION,
                "capabilities": {
                    "tools": {},
                    "resources": {},
                },
                "serverInfo": self.server_info,
            })),
            error: None,
        }
    }

    async fn handle_list_tools(&self, id: Option<Value>) -> McpResponse {
        let tools = self.tools.read().await;
        let tool_list: Vec<McpTool> = tools.values()
            .map(|(tool, _)| tool.clone())
            .collect();

        McpResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(json!({
                "tools": tool_list,
            })),
            error: None,
        }
    }

    async fn handle_call_tool(&self, id: Option<Value>, params: Option<Value>) -> McpResponse {
        let params = match params {
            Some(p) => p,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Invalid params".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let tool_name = match params.get("name").and_then(|v| v.as_str()) {
            Some(name) => name,
            None => {
                return McpResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: None,
                    error: Some(McpError {
                        code: -32602,
                        message: "Missing tool name".to_string(),
                        data: None,
                    }),
                };
            }
        };

        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

        let tools = self.tools.read().await;
        match tools.get(tool_name) {
            Some((_, handler)) => {
                match handler(arguments) {
                    Ok(result) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(serde_json::to_value(result).unwrap()),
                        error: None,
                    },
                    Err(e) => McpResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(McpError {
                            code: -32000,
                            message: format!("Tool execution failed: {}", e),
                            data: None,
                        }),
                    },
                }
            }
            None => McpResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(McpError {
                    code: -32602,
                    message: format!("Tool not found: {}", tool_name),
                    data: None,
                }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mcp_initialize() {
        let server = McpServer::new("test-server", "1.0.0");

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "initialize".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }

    #[tokio::test]
    async fn test_mcp_list_tools() {
        let server = McpServer::new("test-server", "1.0.0");

        // Register a test tool
        let tool = McpTool {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {},
            }),
        };

        let handler: ToolHandler = Arc::new(|_args| {
            Ok(ToolResult {
                content: vec![ContentItem::Text {
                    text: "Test result".to_string(),
                }],
                is_error: None,
            })
        });

        server.register_tool(tool, handler).await.unwrap();

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/list".to_string(),
            params: None,
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());

        let result = response.result.unwrap();
        let tools = result.get("tools").unwrap().as_array().unwrap();
        assert_eq!(tools.len(), 1);
    }

    #[tokio::test]
    async fn test_mcp_call_tool() {
        let server = McpServer::new("test-server", "1.0.0");

        // Register a test tool
        let tool = McpTool {
            name: "echo".to_string(),
            description: "Echo tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" }
                },
            }),
        };

        let handler: ToolHandler = Arc::new(|args| {
            let message = args.get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("empty");

            Ok(ToolResult {
                content: vec![ContentItem::Text {
                    text: format!("Echo: {}", message),
                }],
                is_error: None,
            })
        });

        server.register_tool(tool, handler).await.unwrap();

        let request = McpRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            method: "tools/call".to_string(),
            params: Some(json!({
                "name": "echo",
                "arguments": {
                    "message": "Hello, Robot!"
                }
            })),
        };

        let response = server.handle_request(request).await;
        assert!(response.result.is_some());
        assert!(response.error.is_none());
    }
}
