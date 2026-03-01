//! MCP Server utilities and builders

use crate::*;
use std::sync::Arc;

/// MCP Server builder
pub struct ServerBuilder {
    name: String,
    version: String,
}

impl ServerBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "0.1.0".to_string(),
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn build(self) -> McpServer {
        McpServer::new(self.name, self.version)
    }
}

/// Helper to create a tool handler from a closure
pub fn tool<F>(f: F) -> ToolHandler
where
    F: Fn(Value) -> Result<ToolResult> + Send + Sync + 'static,
{
    Arc::new(f)
}

/// Helper to create a text response
pub fn text_response(text: impl Into<String>) -> ToolResult {
    ToolResult {
        content: vec![ContentItem::Text {
            text: text.into(),
        }],
        is_error: None,
    }
}

/// Helper to create an error response
pub fn error_response(error: impl Into<String>) -> ToolResult {
    ToolResult {
        content: vec![ContentItem::Text {
            text: error.into(),
        }],
        is_error: Some(true),
    }
}
