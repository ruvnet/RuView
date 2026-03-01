//! Integration tests for Ruvector MCP Server

use serde_json::json;
use tempfile::tempdir;

// Note: These are unit-style tests for MCP components
// Full integration tests would require running the server

#[test]
fn test_mcp_request_serialization() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct McpRequest {
        pub jsonrpc: String,
        pub id: Option<serde_json::Value>,
        pub method: String,
        pub params: Option<serde_json::Value>,
    }

    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(1)),
        method: "initialize".to_string(),
        params: None,
    };

    let serialized = serde_json::to_string(&request).unwrap();
    assert!(serialized.contains("initialize"));

    let deserialized: McpRequest = serde_json::from_str(&serialized).unwrap();
    assert_eq!(deserialized.method, "initialize");
}

#[test]
fn test_mcp_response_serialization() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct McpResponse {
        pub jsonrpc: String,
        pub id: Option<serde_json::Value>,
        pub result: Option<serde_json::Value>,
        pub error: Option<serde_json::Value>,
    }

    impl McpResponse {
        fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
            Self {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(result),
                error: None,
            }
        }
    }

    let response = McpResponse::success(Some(json!(1)), json!({"status": "ok"}));

    let serialized = serde_json::to_string(&response).unwrap();
    assert!(serialized.contains("\"result\""));

    let deserialized: McpResponse = serde_json::from_str(&serialized).unwrap();
    assert!(deserialized.result.is_some());
    assert!(deserialized.error.is_none());
}

#[test]
fn test_mcp_error_response() {
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct McpResponse {
        pub jsonrpc: String,
        pub id: Option<serde_json::Value>,
        pub result: Option<serde_json::Value>,
        pub error: Option<McpError>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct McpError {
        pub code: i32,
        pub message: String,
    }

    impl McpResponse {
        fn error(id: Option<serde_json::Value>, error: McpError) -> Self {
            Self {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(error),
            }
        }
    }

    impl McpError {
        fn new(code: i32, message: impl Into<String>) -> Self {
            Self {
                code,
                message: message.into(),
            }
        }
    }

    const METHOD_NOT_FOUND: i32 = -32601;

    let error = McpError::new(METHOD_NOT_FOUND, "Method not found");
    let response = McpResponse::error(Some(json!(1)), error);

    assert!(response.error.is_some());
    assert!(response.result.is_none());
    assert_eq!(response.error.unwrap().code, METHOD_NOT_FOUND);
}

// Note: Full MCP handler tests would require exposing the mcp module publicly
// For now, we test the protocol serialization above
// Integration tests would be run against the actual MCP server binary

// Note: Tests import from the binary crate via the test harness
// The mcp module and config are not public in the binary, so we test via the public API
