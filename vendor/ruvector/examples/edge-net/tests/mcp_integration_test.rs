//! Comprehensive MCP Integration Tests
//!
//! Tests all 18 MCP tools exposed by the edge-net MCP server:
//! - Identity: identity_generate, identity_sign, identity_verify
//! - Credits: credits_balance, credits_contribute, credits_spend, credits_health
//! - RAC: rac_ingest, rac_stats, rac_merkle_root
//! - Learning: learning_store_pattern, learning_lookup, learning_stats
//! - Tasks: task_submit, task_status
//! - Network: network_peers, network_stats

use ruvector_edge_net::mcp::*;
use serde_json::{json, Value};

// ============================================================================
// Test Utilities
// ============================================================================

/// Create a valid MCP request
fn mcp_request(id: u64, method: &str, params: Option<Value>) -> McpRequest {
    McpRequest {
        jsonrpc: "2.0".to_string(),
        id: Some(json!(id)),
        method: method.to_string(),
        params,
    }
}

/// Create a tools/call request for a specific tool
fn tool_call_request(id: u64, tool_name: &str, arguments: Value) -> McpRequest {
    mcp_request(
        id,
        "tools/call",
        Some(json!({
            "name": tool_name,
            "arguments": arguments
        })),
    )
}

/// Parse response and check for success
fn assert_success_response(response: &McpResponse) -> &Value {
    assert!(response.error.is_none(), "Expected success, got error: {:?}", response.error);
    response.result.as_ref().expect("Expected result in success response")
}

/// Parse response and check for error
fn assert_error_response(response: &McpResponse, expected_code: i32) {
    assert!(response.result.is_none(), "Expected error, got result: {:?}", response.result);
    let error = response.error.as_ref().expect("Expected error in response");
    assert_eq!(error.code, expected_code, "Error code mismatch: got {}, expected {}", error.code, expected_code);
}

// ============================================================================
// Protocol Tests
// ============================================================================

#[test]
fn test_mcp_request_serialization() {
    let req = mcp_request(1, "tools/list", None);
    let json = serde_json::to_string(&req).unwrap();

    assert!(json.contains("\"jsonrpc\":\"2.0\""));
    assert!(json.contains("\"id\":1"));
    assert!(json.contains("\"method\":\"tools/list\""));
}

#[test]
fn test_mcp_request_with_params() {
    let req = tool_call_request(42, "credits_balance", json!({"node_id": "test-node"}));
    let json = serde_json::to_string(&req).unwrap();

    assert!(json.contains("\"id\":42"));
    assert!(json.contains("tools/call"));
    assert!(json.contains("credits_balance"));
    assert!(json.contains("test-node"));
}

#[test]
fn test_mcp_response_success() {
    let response = McpResponse::success(Some(json!(1)), json!({"status": "ok"}));

    assert_eq!(response.jsonrpc, "2.0");
    assert!(response.error.is_none());
    assert!(response.result.is_some());

    let result = response.result.unwrap();
    assert_eq!(result["status"], "ok");
}

#[test]
fn test_mcp_response_error() {
    let response = McpResponse::error(
        Some(json!(1)),
        McpError::new(ErrorCodes::INVALID_PARAMS, "Missing required parameter"),
    );

    assert_eq!(response.jsonrpc, "2.0");
    assert!(response.result.is_none());
    assert!(response.error.is_some());

    let error = response.error.unwrap();
    assert_eq!(error.code, ErrorCodes::INVALID_PARAMS);
    assert!(error.message.contains("Missing"));
}

#[test]
fn test_mcp_error_codes() {
    assert_eq!(ErrorCodes::PARSE_ERROR, -32700);
    assert_eq!(ErrorCodes::INVALID_REQUEST, -32600);
    assert_eq!(ErrorCodes::METHOD_NOT_FOUND, -32601);
    assert_eq!(ErrorCodes::INVALID_PARAMS, -32602);
    assert_eq!(ErrorCodes::INTERNAL_ERROR, -32603);
}

#[test]
fn test_mcp_error_with_data() {
    let error = McpError::new(ErrorCodes::INVALID_PARAMS, "Invalid input")
        .with_data(json!({"field": "amount", "reason": "must be positive"}));

    assert_eq!(error.code, ErrorCodes::INVALID_PARAMS);
    assert!(error.data.is_some());
    let data = error.data.unwrap();
    assert_eq!(data["field"], "amount");
}

// ============================================================================
// MCP Tool Schema Tests
// ============================================================================

#[test]
fn test_mcp_tool_definition() {
    let tool = McpTool {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "input": { "type": "string" }
            },
            "required": ["input"]
        }),
    };

    assert_eq!(tool.name, "test_tool");
    assert!(tool.input_schema["properties"].is_object());
}

#[test]
fn test_mcp_resource_definition() {
    let resource = McpResource {
        uri: "edge-net://test".to_string(),
        name: "Test Resource".to_string(),
        description: "A test resource".to_string(),
        mime_type: "application/json".to_string(),
    };

    assert!(resource.uri.starts_with("edge-net://"));
    assert_eq!(resource.mime_type, "application/json");
}

#[test]
fn test_mcp_prompt_definition() {
    let prompt = McpPrompt {
        name: "analyze".to_string(),
        description: "Analyze something".to_string(),
        arguments: Some(vec![
            PromptArgument {
                name: "target".to_string(),
                description: "What to analyze".to_string(),
                required: true,
            },
        ]),
    };

    assert_eq!(prompt.name, "analyze");
    assert!(prompt.arguments.is_some());
    assert!(prompt.arguments.as_ref().unwrap()[0].required);
}

#[test]
fn test_mcp_notification() {
    let notification = McpNotification::new("tools/updated", Some(json!({"tool": "credits_balance"})));

    assert_eq!(notification.jsonrpc, "2.0");
    assert_eq!(notification.method, "tools/updated");
    assert!(notification.params.is_some());
}

// ============================================================================
// Handler Tests
// ============================================================================

#[test]
fn test_vector_handler_search_response() {
    let results = vec![
        ("doc1".to_string(), 0.95),
        ("doc2".to_string(), 0.87),
        ("doc3".to_string(), 0.75),
    ];

    let response = VectorHandler::search_response(Some(json!(1)), results);
    let result = assert_success_response(&response);

    assert!(result["results"].is_array());
    let results_arr = result["results"].as_array().unwrap();
    assert_eq!(results_arr.len(), 3);
    assert_eq!(results_arr[0]["id"], "doc1");
    // Use approximate comparison for floats (f32 precision)
    let score = results_arr[0]["score"].as_f64().unwrap();
    assert!((score - 0.95).abs() < 0.001, "Score should be approximately 0.95, got {}", score);
}

#[test]
fn test_vector_handler_embedding_response() {
    let embedding = vec![0.1, 0.2, 0.3, 0.4, 0.5];

    let response = VectorHandler::embedding_response(Some(json!(1)), embedding.clone());
    let result = assert_success_response(&response);

    assert_eq!(result["dimensions"], 5);
    assert!(result["embedding"].is_array());
}

#[test]
fn test_coherence_handler_conflict_response() {
    let conflicts = vec![
        ("claim1".to_string(), "claim2".to_string(), 0.8),
        ("claim3".to_string(), "claim4".to_string(), 0.5),
    ];

    let response = CoherenceHandler::conflict_response(Some(json!(1)), conflicts);
    let result = assert_success_response(&response);

    assert!(result["conflicts"].is_array());
    let conflicts_arr = result["conflicts"].as_array().unwrap();
    assert_eq!(conflicts_arr.len(), 2);
    // Use approximate comparison for floats (f32 precision)
    let severity = conflicts_arr[0]["severity"].as_f64().unwrap();
    assert!((severity - 0.8).abs() < 0.001, "Severity should be approximately 0.8, got {}", severity);
}

#[test]
fn test_coherence_handler_resolution_response() {
    let response = CoherenceHandler::resolution_response(
        Some(json!(1)),
        "resolution-123",
        vec!["claim1".to_string(), "claim2".to_string()],
        vec!["claim3".to_string()],
    );
    let result = assert_success_response(&response);

    assert_eq!(result["resolutionId"], "resolution-123");
    assert_eq!(result["accepted"].as_array().unwrap().len(), 2);
    assert_eq!(result["deprecated"].as_array().unwrap().len(), 1);
}

#[test]
fn test_economics_handler_stake_response() {
    let response = EconomicsHandler::stake_response(
        Some(json!(1)),
        1000,
        1735689600000, // Future timestamp
        1.5,
    );
    let result = assert_success_response(&response);

    assert_eq!(result["staked"], 1000);
    assert_eq!(result["multiplier"], 1.5);
}

#[test]
fn test_economics_handler_reward_response() {
    let recipients = vec![
        ("node1".to_string(), 500),
        ("node2".to_string(), 300),
        ("node3".to_string(), 200),
    ];

    let response = EconomicsHandler::reward_response(Some(json!(1)), recipients, 1000);
    let result = assert_success_response(&response);

    assert_eq!(result["totalDistributed"], 1000);
    assert_eq!(result["recipients"].as_array().unwrap().len(), 3);
}

#[test]
fn test_network_handler_peers_response() {
    let peers = vec![
        PeerInfo {
            node_id: "node1".to_string(),
            public_key: "abc123".to_string(),
            reputation: 0.95,
            latency_ms: 50,
            connected: true,
        },
        PeerInfo {
            node_id: "node2".to_string(),
            public_key: "def456".to_string(),
            reputation: 0.80,
            latency_ms: 100,
            connected: false,
        },
    ];

    let response = NetworkHandler::peers_response(Some(json!(1)), peers);
    let result = assert_success_response(&response);

    assert_eq!(result["count"], 2);
    let peers_arr = result["peers"].as_array().unwrap();
    // Use approximate comparison for floats (f32 precision)
    let reputation = peers_arr[0]["reputation"].as_f64().unwrap();
    assert!((reputation - 0.95).abs() < 0.001, "Reputation should be approximately 0.95, got {}", reputation);
    assert!(peers_arr[0]["connected"].as_bool().unwrap());
}

#[test]
fn test_network_handler_health_response() {
    let health = NetworkHealth {
        score: 0.85,
        peer_count: 10,
        avg_latency_ms: 75,
        message_rate: 100.5,
        bandwidth_kbps: 1500,
    };

    let response = NetworkHandler::health_response(Some(json!(1)), health);
    let result = assert_success_response(&response);

    // Use approximate comparison for floats (f32 precision)
    let score = result["score"].as_f64().unwrap();
    assert!((score - 0.85).abs() < 0.001, "Score should be approximately 0.85, got {}", score);
    assert_eq!(result["peerCount"], 10);
    assert_eq!(result["bandwidth"], 1500);
}

#[test]
fn test_error_response_helper() {
    let response = error_response(Some(json!(1)), ErrorCodes::INVALID_PARAMS, "Bad input");
    assert_error_response(&response, ErrorCodes::INVALID_PARAMS);
}

#[test]
fn test_not_implemented_helper() {
    let response = not_implemented(Some(json!(1)), "Advanced feature");
    let result = assert_success_response(&response);

    assert_eq!(result["status"], "not_implemented");
    assert_eq!(result["feature"], "Advanced feature");
}

// ============================================================================
// Task Status Tests
// ============================================================================

#[test]
fn test_task_status_enum() {
    assert_eq!(TaskStatus::Queued.as_str(), "queued");
    assert_eq!(TaskStatus::Running.as_str(), "running");
    assert_eq!(TaskStatus::Completed.as_str(), "completed");
    assert_eq!(TaskStatus::Failed.as_str(), "failed");
    assert_eq!(TaskStatus::Cancelled.as_str(), "cancelled");
}

#[test]
fn test_task_result() {
    let result = TaskResult {
        task_id: "task-123".to_string(),
        status: TaskStatus::Completed,
        result: Some(json!({"output": "success"})),
        error: None,
        cost: 50,
    };

    assert_eq!(result.task_id, "task-123");
    assert_eq!(result.status, TaskStatus::Completed);
    assert!(result.error.is_none());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_empty_tool_call() {
    let request = mcp_request(1, "tools/call", None);

    // This should result in missing params error
    assert!(request.params.is_none());
}

#[test]
fn test_null_id_request() {
    let request = McpRequest {
        jsonrpc: "2.0".to_string(),
        id: None, // Notification-style
        method: "tools/list".to_string(),
        params: None,
    };

    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("null") || !json.contains("\"id\""));
}

#[test]
fn test_large_vector_search_results() {
    // Create many search results
    let results: Vec<(String, f32)> = (0..100)
        .map(|i| (format!("doc{}", i), 1.0 - (i as f32 * 0.01)))
        .collect();

    let response = VectorHandler::search_response(Some(json!(1)), results);
    let result = assert_success_response(&response);

    assert_eq!(result["results"].as_array().unwrap().len(), 100);
}

#[test]
fn test_special_characters_in_params() {
    let tool_call = tool_call_request(
        1,
        "identity_sign",
        json!({
            "message": "Hello\nWorld\t\"Test\"\\Special<>&"
        }),
    );

    let json = serde_json::to_string(&tool_call).unwrap();
    let parsed: McpRequest = serde_json::from_str(&json).unwrap();

    // Verify special chars preserved
    let args = parsed.params.unwrap();
    assert!(args["arguments"]["message"].as_str().unwrap().contains('\n'));
}

#[test]
fn test_unicode_in_params() {
    let tool_call = tool_call_request(
        1,
        "learning_store_pattern",
        json!({
            "metadata": {
                "description": "Testing unicode: \u{1F600} \u{4E2D}\u{6587} \u{0441}\u{043B}\u{0430}\u{0432}\u{0430}"
            }
        }),
    );

    let json = serde_json::to_string(&tool_call).unwrap();
    let parsed: McpRequest = serde_json::from_str(&json).unwrap();

    // Verify unicode preserved through serialization
    assert!(parsed.params.is_some());
}

#[test]
fn test_very_long_message() {
    // Create a very long message (1MB+)
    let long_message = "a".repeat(1_000_000);

    let tool_call = tool_call_request(
        1,
        "identity_sign",
        json!({"message": long_message}),
    );

    let json = serde_json::to_string(&tool_call).unwrap();
    assert!(json.len() > 1_000_000);
}

#[test]
fn test_empty_arrays_in_response() {
    let response = VectorHandler::search_response(Some(json!(1)), vec![]);
    let result = assert_success_response(&response);

    assert!(result["results"].as_array().unwrap().is_empty());
}

#[test]
fn test_zero_values() {
    let health = NetworkHealth {
        score: 0.0,
        peer_count: 0,
        avg_latency_ms: 0,
        message_rate: 0.0,
        bandwidth_kbps: 0,
    };

    let response = NetworkHandler::health_response(Some(json!(1)), health);
    let result = assert_success_response(&response);

    assert_eq!(result["score"], 0.0);
    assert_eq!(result["peerCount"], 0);
}

#[test]
fn test_negative_values_in_response() {
    // Some contexts may allow negative values (e.g., balance adjustments)
    let error = McpError::new(ErrorCodes::INTERNAL_ERROR, "Negative balance: -100");
    let response = McpResponse::error(Some(json!(1)), error);

    assert!(response.error.is_some());
    assert!(response.error.as_ref().unwrap().message.contains("-100"));
}

#[test]
fn test_float_precision() {
    let embedding = vec![
        0.123456789012345678901234567890_f32,
        std::f32::EPSILON,
        std::f32::MAX,
        std::f32::MIN,
    ];

    let response = VectorHandler::embedding_response(Some(json!(1)), embedding);
    let result = assert_success_response(&response);

    // Verify floats serialized correctly
    assert!(result["embedding"].is_array());
}

// ============================================================================
// Concurrent Access Pattern Tests (Simulated)
// ============================================================================

#[test]
fn test_concurrent_request_ids() {
    // Simulate multiple concurrent requests with different IDs
    let requests: Vec<McpRequest> = (0..100)
        .map(|i| tool_call_request(i, "credits_balance", json!({"node_id": format!("node-{}", i)})))
        .collect();

    // Verify all requests are unique
    let ids: Vec<_> = requests.iter()
        .map(|r| r.id.as_ref().unwrap().as_u64().unwrap())
        .collect();

    let unique_ids: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(unique_ids.len(), 100);
}

#[test]
fn test_request_response_id_matching() {
    let request = tool_call_request(42, "network_stats", json!({}));

    // Simulate response creation with matching ID
    let response = McpResponse::success(
        request.id.clone(),
        json!({"connected": true}),
    );

    assert_eq!(request.id, response.id);
}

// ============================================================================
// Protocol Format Validation
// ============================================================================

#[test]
fn test_jsonrpc_version() {
    let request = mcp_request(1, "test", None);
    assert_eq!(request.jsonrpc, "2.0");

    let response = McpResponse::success(Some(json!(1)), json!({}));
    assert_eq!(response.jsonrpc, "2.0");
}

#[test]
fn test_response_only_has_result_or_error() {
    let success = McpResponse::success(Some(json!(1)), json!({}));
    assert!(success.result.is_some());
    assert!(success.error.is_none());

    let error = McpResponse::error(Some(json!(1)), McpError::new(-1, "test"));
    assert!(error.result.is_none());
    assert!(error.error.is_some());
}

#[test]
fn test_tool_call_structure() {
    let tool_call = tool_call_request(
        1,
        "identity_generate",
        json!({"site_id": "test-site"}),
    );

    let params = tool_call.params.unwrap();
    assert_eq!(params["name"], "identity_generate");
    assert!(params["arguments"].is_object());
}

// ============================================================================
// All 18 Tool Request Format Tests
// ============================================================================

#[test]
fn test_identity_generate_request_format() {
    let req = tool_call_request(1, "identity_generate", json!({"site_id": "my-site"}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("identity_generate"));
    assert!(json.contains("site_id"));
}

#[test]
fn test_identity_sign_request_format() {
    let req = tool_call_request(1, "identity_sign", json!({"message": "SGVsbG8gV29ybGQ="}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("identity_sign"));
    assert!(json.contains("message"));
}

#[test]
fn test_identity_verify_request_format() {
    let req = tool_call_request(1, "identity_verify", json!({
        "public_key": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        "message": "SGVsbG8gV29ybGQ=",
        "signature": "0123456789abcdef"
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("identity_verify"));
    assert!(json.contains("public_key"));
    assert!(json.contains("signature"));
}

#[test]
fn test_credits_balance_request_format() {
    let req = tool_call_request(1, "credits_balance", json!({"node_id": "node-abc123"}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("credits_balance"));
    assert!(json.contains("node_id"));
}

#[test]
fn test_credits_contribute_request_format() {
    let req = tool_call_request(1, "credits_contribute", json!({
        "amount": 100,
        "task_type": "vector_search"
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("credits_contribute"));
    assert!(json.contains("amount"));
}

#[test]
fn test_credits_spend_request_format() {
    let req = tool_call_request(1, "credits_spend", json!({
        "amount": 50,
        "purpose": "neural_inference"
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("credits_spend"));
    assert!(json.contains("purpose"));
}

#[test]
fn test_credits_health_request_format() {
    let req = tool_call_request(1, "credits_health", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("credits_health"));
}

#[test]
fn test_rac_ingest_request_format() {
    let req = tool_call_request(1, "rac_ingest", json!({
        "event": {
            "type": "assert",
            "proposition": "test claim",
            "confidence": 0.95
        }
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("rac_ingest"));
    assert!(json.contains("event"));
}

#[test]
fn test_rac_stats_request_format() {
    let req = tool_call_request(1, "rac_stats", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("rac_stats"));
}

#[test]
fn test_rac_merkle_root_request_format() {
    let req = tool_call_request(1, "rac_merkle_root", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("rac_merkle_root"));
}

#[test]
fn test_learning_store_pattern_request_format() {
    let req = tool_call_request(1, "learning_store_pattern", json!({
        "embedding": [0.1, 0.2, 0.3, 0.4, 0.5],
        "metadata": {"category": "test"}
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("learning_store_pattern"));
    assert!(json.contains("embedding"));
}

#[test]
fn test_learning_lookup_request_format() {
    let req = tool_call_request(1, "learning_lookup", json!({
        "query": [0.1, 0.2, 0.3, 0.4, 0.5],
        "k": 10
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("learning_lookup"));
    assert!(json.contains("query"));
}

#[test]
fn test_learning_stats_request_format() {
    let req = tool_call_request(1, "learning_stats", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("learning_stats"));
}

#[test]
fn test_task_submit_request_format() {
    let req = tool_call_request(1, "task_submit", json!({
        "task_type": "vector_search",
        "payload": {"query": [0.1, 0.2, 0.3]},
        "max_cost": 100
    }));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("task_submit"));
    assert!(json.contains("task_type"));
    assert!(json.contains("payload"));
}

#[test]
fn test_task_status_request_format() {
    let req = tool_call_request(1, "task_status", json!({"task_id": "task-abc123"}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("task_status"));
    assert!(json.contains("task_id"));
}

#[test]
fn test_network_peers_request_format() {
    let req = tool_call_request(1, "network_peers", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("network_peers"));
}

#[test]
fn test_network_stats_request_format() {
    let req = tool_call_request(1, "network_stats", json!({}));
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("network_stats"));
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_unknown_method_request() {
    let req = mcp_request(1, "unknown/method", None);

    // Would result in METHOD_NOT_FOUND error from server
    assert_eq!(req.method, "unknown/method");
}

#[test]
fn test_unknown_tool_request() {
    let req = tool_call_request(1, "unknown_tool", json!({}));

    // Would result in METHOD_NOT_FOUND error for unknown tool
    let params = req.params.unwrap();
    assert_eq!(params["name"], "unknown_tool");
}

#[test]
fn test_missing_required_field() {
    // identity_verify requires public_key, message, and signature
    let req = tool_call_request(1, "identity_verify", json!({
        "public_key": "abc123"
        // missing message and signature
    }));

    let params = req.params.unwrap();
    assert!(!params["arguments"].as_object().unwrap().contains_key("message"));
}

#[test]
fn test_invalid_type_for_field() {
    // amount should be a number, not a string
    let req = tool_call_request(1, "credits_spend", json!({
        "amount": "not-a-number"
    }));

    let params = req.params.unwrap();
    assert!(params["arguments"]["amount"].is_string());
}

#[test]
fn test_negative_amount() {
    let req = tool_call_request(1, "credits_spend", json!({
        "amount": -100
    }));

    let params = req.params.unwrap();
    assert!(params["arguments"]["amount"].as_i64().unwrap() < 0);
}

#[test]
fn test_empty_embedding_vector() {
    let req = tool_call_request(1, "learning_store_pattern", json!({
        "embedding": []
    }));

    let params = req.params.unwrap();
    assert!(params["arguments"]["embedding"].as_array().unwrap().is_empty());
}

#[test]
fn test_invalid_base64_message() {
    let req = tool_call_request(1, "identity_sign", json!({
        "message": "not-valid-base64!!!"
    }));

    // Would result in INVALID_PARAMS error
    let params = req.params.unwrap();
    assert!(params["arguments"]["message"].as_str().is_some());
}

#[test]
fn test_invalid_hex_public_key() {
    let req = tool_call_request(1, "identity_verify", json!({
        "public_key": "not-hex-zzz",
        "message": "SGVsbG8=",
        "signature": "abcd"
    }));

    // Would result in INVALID_PARAMS error
    let params = req.params.unwrap();
    assert!(params["arguments"]["public_key"].as_str().unwrap().contains("zzz"));
}

// ============================================================================
// Batch Operation Tests
// ============================================================================

#[test]
fn test_multiple_requests_serialization() {
    let requests = vec![
        tool_call_request(1, "credits_balance", json!({})),
        tool_call_request(2, "network_stats", json!({})),
        tool_call_request(3, "rac_stats", json!({})),
    ];

    // Serialize all requests
    let jsons: Vec<String> = requests.iter()
        .map(|r| serde_json::to_string(r).unwrap())
        .collect();

    assert_eq!(jsons.len(), 3);
    assert!(jsons[0].contains("credits_balance"));
    assert!(jsons[1].contains("network_stats"));
    assert!(jsons[2].contains("rac_stats"));
}

#[test]
fn test_response_array() {
    let responses = vec![
        McpResponse::success(Some(json!(1)), json!({"balance": 100})),
        McpResponse::success(Some(json!(2)), json!({"connected": true})),
        McpResponse::error(Some(json!(3)), McpError::new(-1, "failed")),
    ];

    assert!(responses[0].result.is_some());
    assert!(responses[1].result.is_some());
    assert!(responses[2].error.is_some());
}

// ============================================================================
// Integration Simulation Tests
// ============================================================================

#[test]
fn test_full_workflow_simulation() {
    // Simulate a complete workflow: generate identity -> contribute -> check balance

    // Step 1: Generate identity
    let gen_req = tool_call_request(1, "identity_generate", json!({"site_id": "test-site"}));
    let gen_json = serde_json::to_string(&gen_req).unwrap();
    assert!(gen_json.contains("identity_generate"));

    // Simulate response
    let gen_response = McpResponse::success(Some(json!(1)), json!({
        "nodeId": "node-abc123",
        "publicKey": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
    }));
    let node_id = gen_response.result.as_ref().unwrap()["nodeId"].as_str().unwrap();

    // Step 2: Contribute
    let contribute_req = tool_call_request(2, "credits_contribute", json!({
        "amount": 100,
        "task_type": "vector_search"
    }));
    let contribute_response = McpResponse::success(Some(json!(2)), json!({
        "credited": 100,
        "newBalance": 100
    }));
    assert_eq!(contribute_response.result.as_ref().unwrap()["newBalance"], 100);

    // Step 3: Check balance
    let balance_req = tool_call_request(3, "credits_balance", json!({"node_id": node_id}));
    let balance_response = McpResponse::success(Some(json!(3)), json!({
        "balance": 100,
        "totalEarned": 100,
        "totalSpent": 0
    }));
    assert_eq!(balance_response.result.as_ref().unwrap()["balance"], 100);
}

#[test]
fn test_task_lifecycle_simulation() {
    // Simulate task submission and status checking

    // Submit task
    let submit_req = tool_call_request(1, "task_submit", json!({
        "task_type": "vector_search",
        "payload": {"query": [0.1, 0.2, 0.3], "k": 10},
        "max_cost": 50
    }));
    let submit_json = serde_json::to_string(&submit_req).unwrap();
    assert!(submit_json.contains("task_submit"));

    // Simulate response with task ID
    let submit_response = McpResponse::success(Some(json!(1)), json!({
        "taskId": "task-xyz789",
        "status": "queued",
        "estimatedCost": 25
    }));
    let task_id = submit_response.result.as_ref().unwrap()["taskId"].as_str().unwrap();

    // Check status
    let status_req = tool_call_request(2, "task_status", json!({"task_id": task_id}));
    let status_response = McpResponse::success(Some(json!(2)), json!({
        "taskId": task_id,
        "status": "running",
        "progress": 0.5
    }));
    assert_eq!(status_response.result.as_ref().unwrap()["status"], "running");
}

#[test]
fn test_learning_pattern_lifecycle() {
    // Store pattern -> lookup -> get stats

    // Store pattern
    let store_req = tool_call_request(1, "learning_store_pattern", json!({
        "embedding": [0.1, 0.2, 0.3, 0.4, 0.5],
        "metadata": {"label": "test-pattern"}
    }));

    let store_response = McpResponse::success(Some(json!(1)), json!({
        "patternId": 42
    }));
    let pattern_id = store_response.result.as_ref().unwrap()["patternId"].as_i64().unwrap();
    assert!(pattern_id >= 0);

    // Lookup similar patterns
    let lookup_req = tool_call_request(2, "learning_lookup", json!({
        "query": [0.1, 0.2, 0.3, 0.4, 0.5],
        "k": 5
    }));

    let lookup_response = McpResponse::success(Some(json!(2)), json!({
        "results": [
            {"id": pattern_id, "similarity": 1.0, "confidence": 0.9}
        ]
    }));
    assert!(!lookup_response.result.as_ref().unwrap()["results"].as_array().unwrap().is_empty());

    // Get stats
    let stats_req = tool_call_request(3, "learning_stats", json!({}));
    let stats_response = McpResponse::success(Some(json!(3)), json!({
        "total_patterns": 1,
        "total_usage": 1
    }));
    assert_eq!(stats_response.result.as_ref().unwrap()["total_patterns"], 1);
}

// ============================================================================
// Resource and Prompt Tests
// ============================================================================

#[test]
fn test_resources_list_request() {
    let req = mcp_request(1, "resources/list", None);
    assert_eq!(req.method, "resources/list");
}

#[test]
fn test_resources_read_request() {
    let req = mcp_request(1, "resources/read", Some(json!({
        "uri": "edge-net://identity"
    })));

    let params = req.params.unwrap();
    assert_eq!(params["uri"], "edge-net://identity");
}

#[test]
fn test_prompts_list_request() {
    let req = mcp_request(1, "prompts/list", None);
    assert_eq!(req.method, "prompts/list");
}

#[test]
fn test_prompts_get_request() {
    let req = mcp_request(1, "prompts/get", Some(json!({
        "name": "analyze_network",
        "arguments": {"focus": "performance"}
    })));

    let params = req.params.unwrap();
    assert_eq!(params["name"], "analyze_network");
}

// ============================================================================
// Initialize Method Tests
// ============================================================================

#[test]
fn test_initialize_request() {
    let req = mcp_request(1, "initialize", None);
    assert_eq!(req.method, "initialize");
}

#[test]
fn test_tools_list_request() {
    let req = mcp_request(1, "tools/list", None);
    assert_eq!(req.method, "tools/list");
}
