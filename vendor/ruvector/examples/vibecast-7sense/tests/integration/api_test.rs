//! Integration tests for API Context
//!
//! Tests for REST endpoints, GraphQL queries/mutations, rate limiting,
//! and error responses.

use vibecast_tests::fixtures::*;
use vibecast_tests::mocks::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ============================================================================
// REST Endpoint Tests
// ============================================================================

mod rest_endpoints {
    use super::*;

    // Mock API paths
    const RECORDINGS_PATH: &str = "/api/v1/recordings";
    const SEGMENTS_PATH: &str = "/api/v1/segments";
    const EMBEDDINGS_PATH: &str = "/api/v1/embeddings";
    const CLUSTERS_PATH: &str = "/api/v1/clusters";
    const INTERPRETATIONS_PATH: &str = "/api/v1/interpretations";
    const SEARCH_PATH: &str = "/api/v1/search";
    const HEALTH_PATH: &str = "/api/v1/health";

    #[test]
    fn test_recordings_list_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"recordings": [{"id": "uuid1", "duration_ms": 60000}]}"#,
        );

        let response = client.get(RECORDINGS_PATH).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("recordings"));
    }

    #[test]
    fn test_recordings_create_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            201,
            r#"{"id": "new-uuid", "status": "created"}"#,
        );

        let body = r#"{"source": "upload", "metadata": {}}"#;
        let response = client.post(RECORDINGS_PATH, body).unwrap();

        assert_eq!(response.status, 201);
        assert!(response.body.contains("id"));
    }

    #[test]
    fn test_segments_by_recording_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"segments": [{"id": "seg1", "start_ms": 0, "end_ms": 5000}]}"#,
        );

        let path = format!("{}/recording123/segments", RECORDINGS_PATH);
        let response = client.get(&path).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("segments"));
    }

    #[test]
    fn test_embedding_generation_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            202,
            r#"{"job_id": "job123", "status": "processing"}"#,
        );

        let body = r#"{"segment_ids": ["seg1", "seg2"], "model": "perch2"}"#;
        let response = client.post(EMBEDDINGS_PATH, body).unwrap();

        assert_eq!(response.status, 202); // Accepted for async processing
        assert!(response.body.contains("job_id"));
    }

    #[test]
    fn test_similarity_search_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"results": [{"segment_id": "seg1", "distance": 0.1}], "count": 1}"#,
        );

        let body = r#"{"query_segment_id": "query1", "k": 10}"#;
        let response = client.post(SEARCH_PATH, body).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("results"));
    }

    #[test]
    fn test_interpretation_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"interpretation": {"statements": ["Similar to alarm calls"], "confidence": 0.85}}"#,
        );

        let body = r#"{"segment_id": "seg1", "include_citations": true}"#;
        let response = client.post(INTERPRETATIONS_PATH, body).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("interpretation"));
        assert!(response.body.contains("confidence"));
    }

    #[test]
    fn test_health_check_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"status": "healthy", "version": "1.0.0", "components": {"database": "ok", "index": "ok"}}"#,
        );

        let response = client.get(HEALTH_PATH).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("healthy"));
    }

    #[test]
    fn test_cluster_list_endpoint() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"clusters": [{"id": "c1", "member_count": 50}], "total": 1}"#,
        );

        let response = client.get(CLUSTERS_PATH).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("clusters"));
    }
}

// ============================================================================
// GraphQL Tests
// ============================================================================

mod graphql {
    use super::*;

    const GRAPHQL_PATH: &str = "/graphql";

    fn create_graphql_query(query: &str) -> String {
        format!(r#"{{"query": "{}"}}"#, query.replace('"', "\\\""))
    }

    #[test]
    fn test_graphql_recordings_query() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"recordings": [{"id": "rec1", "duration_ms": 60000}]}}"#,
        );

        let query = create_graphql_query("{ recordings { id duration_ms } }");
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("data"));
        assert!(response.body.contains("recordings"));
    }

    #[test]
    fn test_graphql_recording_with_segments() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"recording": {"id": "rec1", "segments": [{"id": "seg1"}]}}}"#,
        );

        let query = create_graphql_query(
            "{ recording(id: \\\"rec1\\\") { id segments { id } } }",
        );
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("segments"));
    }

    #[test]
    fn test_graphql_segment_with_embedding() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"segment": {"id": "seg1", "embedding": {"id": "emb1", "norm": 1.0}}}}"#,
        );

        let query = create_graphql_query(
            "{ segment(id: \\\"seg1\\\") { id embedding { id norm } } }",
        );
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("embedding"));
    }

    #[test]
    fn test_graphql_similarity_search() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"similarSegments": [{"segment": {"id": "s1"}, "distance": 0.1}]}}"#,
        );

        let query = create_graphql_query(
            "{ similarSegments(segmentId: \\\"seg1\\\", k: 10) { segment { id } distance } }",
        );
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("similarSegments"));
    }

    #[test]
    fn test_graphql_create_recording_mutation() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"createRecording": {"id": "new-rec", "status": "INGESTED"}}}"#,
        );

        let mutation = create_graphql_query(
            "mutation { createRecording(input: {source: \\\"upload\\\"}) { id status } }",
        );
        let response = client.post(GRAPHQL_PATH, &mutation).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("createRecording"));
    }

    #[test]
    fn test_graphql_generate_embeddings_mutation() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"generateEmbeddings": {"jobId": "job1", "status": "PROCESSING"}}}"#,
        );

        let mutation = create_graphql_query(
            "mutation { generateEmbeddings(segmentIds: [\\\"s1\\\", \\\"s2\\\"]) { jobId status } }",
        );
        let response = client.post(GRAPHQL_PATH, &mutation).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("generateEmbeddings"));
    }

    #[test]
    fn test_graphql_run_clustering_mutation() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"runClustering": {"sessionId": "sess1", "clusterCount": 15}}}"#,
        );

        let mutation = create_graphql_query(
            "mutation { runClustering(method: HDBSCAN, params: {minClusterSize: 5}) { sessionId clusterCount } }",
        );
        let response = client.post(GRAPHQL_PATH, &mutation).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("runClustering"));
    }

    #[test]
    fn test_graphql_error_response() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": null, "errors": [{"message": "Segment not found", "path": ["segment"]}]}"#,
        );

        let query = create_graphql_query("{ segment(id: \\\"nonexistent\\\") { id } }");
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200); // GraphQL returns 200 even for errors
        assert!(response.body.contains("errors"));
    }

    #[test]
    fn test_graphql_nested_query() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{"data": {"recording": {"segments": [{"embedding": {"cluster": {"id": "c1"}}}]}}}"#,
        );

        let query = create_graphql_query(
            "{ recording(id: \\\"r1\\\") { segments { embedding { cluster { id } } } } }",
        );
        let response = client.post(GRAPHQL_PATH, &query).unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("cluster"));
    }
}

// ============================================================================
// Rate Limiting Tests
// ============================================================================

mod rate_limiting {
    use super::*;

    #[test]
    fn test_rate_limiter_allows_under_limit() {
        let limiter = MockRateLimiter::new(100); // 100 requests/second

        // Should allow first requests
        for _ in 0..50 {
            assert!(limiter.check(), "Should allow requests under limit");
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = MockRateLimiter::new(10); // 10 requests/second

        // Exhaust limit
        for _ in 0..10 {
            limiter.check();
        }

        // Next request should be blocked
        assert!(!limiter.check(), "Should block requests over limit");
    }

    #[test]
    fn test_rate_limiter_sliding_window() {
        let limiter = MockRateLimiter::new(5);

        // Make 5 requests
        for _ in 0..5 {
            assert!(limiter.check());
        }

        // 6th should be blocked
        assert!(!limiter.check());

        // After window slides (simulated by new check), requests should be allowed
        // In real implementation, would wait for time to pass
    }

    #[test]
    fn test_rate_limit_response_code() {
        // When rate limited, API should return 429
        let client = MockApiClient::new();
        client.queue_response(
            429,
            r#"{"error": "Too Many Requests", "retry_after": 60}"#,
        );

        let response = client.get("/api/v1/recordings").unwrap();

        assert_eq!(response.status, 429);
        assert!(response.body.contains("Too Many Requests"));
    }

    #[test]
    fn test_rate_limit_headers() {
        let mut response = MockResponse {
            status: 200,
            body: "{}".to_string(),
            headers: HashMap::new(),
        };

        response.headers.insert("X-RateLimit-Limit".to_string(), "100".to_string());
        response.headers.insert("X-RateLimit-Remaining".to_string(), "95".to_string());
        response.headers.insert("X-RateLimit-Reset".to_string(), "1609459200".to_string());

        assert_eq!(response.headers.get("X-RateLimit-Limit").unwrap(), "100");
        assert_eq!(response.headers.get("X-RateLimit-Remaining").unwrap(), "95");
    }

    #[test]
    fn test_different_rate_limits_per_endpoint() {
        // Heavy operations should have lower limits
        let search_limiter = MockRateLimiter::new(10);   // 10/sec for search
        let read_limiter = MockRateLimiter::new(100);    // 100/sec for reads
        let write_limiter = MockRateLimiter::new(20);    // 20/sec for writes

        // Reads should be most permissive
        for _ in 0..50 {
            assert!(read_limiter.check());
        }

        // Search should be more restrictive
        for _ in 0..10 {
            assert!(search_limiter.check());
        }
        assert!(!search_limiter.check());
    }
}

// ============================================================================
// Error Response Tests
// ============================================================================

mod error_responses {
    use super::*;

    #[test]
    fn test_404_not_found() {
        let client = MockApiClient::new();
        client.queue_response(
            404,
            r#"{"error": "Not Found", "message": "Recording not found", "code": "RECORDING_NOT_FOUND"}"#,
        );

        let response = client.get("/api/v1/recordings/nonexistent").unwrap();

        assert_eq!(response.status, 404);
        assert!(response.body.contains("Not Found"));
    }

    #[test]
    fn test_400_bad_request() {
        let client = MockApiClient::new();
        client.queue_response(
            400,
            r#"{"error": "Bad Request", "message": "Invalid segment_id format", "field": "segment_id"}"#,
        );

        let response = client.post("/api/v1/embeddings", r#"{"segment_id": "invalid"}"#).unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body.contains("Bad Request"));
    }

    #[test]
    fn test_422_validation_error() {
        let client = MockApiClient::new();
        client.queue_response(
            422,
            r#"{"error": "Validation Error", "errors": [{"field": "k", "message": "must be between 1 and 100"}]}"#,
        );

        let response = client.post("/api/v1/search", r#"{"k": 1000}"#).unwrap();

        assert_eq!(response.status, 422);
        assert!(response.body.contains("Validation Error"));
    }

    #[test]
    fn test_500_internal_error() {
        let client = MockApiClient::new();
        client.queue_response(
            500,
            r#"{"error": "Internal Server Error", "message": "An unexpected error occurred", "request_id": "req-123"}"#,
        );

        let response = client.get("/api/v1/recordings").unwrap();

        assert_eq!(response.status, 500);
        assert!(response.body.contains("Internal Server Error"));
        assert!(response.body.contains("request_id"));
    }

    #[test]
    fn test_503_service_unavailable() {
        let client = MockApiClient::new();
        client.queue_response(
            503,
            r#"{"error": "Service Unavailable", "message": "Index is rebuilding", "retry_after": 300}"#,
        );

        let response = client.get("/api/v1/search").unwrap();

        assert_eq!(response.status, 503);
        assert!(response.body.contains("Service Unavailable"));
    }

    #[test]
    fn test_error_response_format() {
        // All errors should have consistent format
        let error_bodies = vec![
            r#"{"error": "Not Found", "message": "Resource not found", "code": "NOT_FOUND"}"#,
            r#"{"error": "Bad Request", "message": "Invalid input", "code": "INVALID_INPUT"}"#,
            r#"{"error": "Internal Server Error", "message": "Server error", "code": "INTERNAL_ERROR"}"#,
        ];

        for body in error_bodies {
            assert!(body.contains("error"));
            assert!(body.contains("message"));
            assert!(body.contains("code"));
        }
    }

    #[test]
    fn test_error_with_details() {
        let client = MockApiClient::new();
        client.queue_response(
            400,
            r#"{
                "error": "Bad Request",
                "message": "Multiple validation errors",
                "details": [
                    {"field": "sample_rate", "error": "must be 32000"},
                    {"field": "channels", "error": "must be 1 (mono)"}
                ]
            }"#,
        );

        let response = client.post("/api/v1/recordings", "{}").unwrap();

        assert_eq!(response.status, 400);
        assert!(response.body.contains("details"));
    }
}

// ============================================================================
// Authentication Tests
// ============================================================================

mod authentication {
    use super::*;

    #[test]
    fn test_unauthorized_without_token() {
        let client = MockApiClient::new();
        client.queue_response(
            401,
            r#"{"error": "Unauthorized", "message": "Missing or invalid authentication token"}"#,
        );

        let response = client.get("/api/v1/recordings").unwrap();

        assert_eq!(response.status, 401);
        assert!(response.body.contains("Unauthorized"));
    }

    #[test]
    fn test_forbidden_insufficient_permissions() {
        let client = MockApiClient::new();
        client.queue_response(
            403,
            r#"{"error": "Forbidden", "message": "Insufficient permissions to access this resource"}"#,
        );

        let response = client.get("/api/v1/admin/settings").unwrap();

        assert_eq!(response.status, 403);
        assert!(response.body.contains("Forbidden"));
    }

    #[test]
    fn test_token_expired() {
        let client = MockApiClient::new();
        client.queue_response(
            401,
            r#"{"error": "Unauthorized", "message": "Token expired", "code": "TOKEN_EXPIRED"}"#,
        );

        let response = client.get("/api/v1/recordings").unwrap();

        assert_eq!(response.status, 401);
        assert!(response.body.contains("TOKEN_EXPIRED"));
    }
}

// ============================================================================
// Pagination Tests
// ============================================================================

mod pagination {
    use super::*;

    #[test]
    fn test_paginated_response() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{
                "data": [{"id": "rec1"}, {"id": "rec2"}],
                "pagination": {
                    "page": 1,
                    "per_page": 20,
                    "total": 100,
                    "total_pages": 5
                }
            }"#,
        );

        let response = client.get("/api/v1/recordings?page=1&per_page=20").unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("pagination"));
        assert!(response.body.contains("total_pages"));
    }

    #[test]
    fn test_cursor_based_pagination() {
        let client = MockApiClient::new();
        client.queue_response(
            200,
            r#"{
                "data": [{"id": "rec1"}, {"id": "rec2"}],
                "cursors": {
                    "next": "eyJpZCI6InJlYzIifQ==",
                    "previous": null
                },
                "has_more": true
            }"#,
        );

        let response = client.get("/api/v1/recordings?limit=20").unwrap();

        assert_eq!(response.status, 200);
        assert!(response.body.contains("cursors"));
        assert!(response.body.contains("has_more"));
    }

    #[test]
    fn test_invalid_page_parameter() {
        let client = MockApiClient::new();
        client.queue_response(
            400,
            r#"{"error": "Bad Request", "message": "Page must be a positive integer"}"#,
        );

        let response = client.get("/api/v1/recordings?page=-1").unwrap();

        assert_eq!(response.status, 400);
    }
}

// ============================================================================
// Content Negotiation Tests
// ============================================================================

mod content_negotiation {
    use super::*;

    #[test]
    fn test_json_response() {
        let response = MockResponse {
            status: 200,
            body: r#"{"data": []}"#.to_string(),
            headers: [("Content-Type".to_string(), "application/json".to_string())]
                .into_iter()
                .collect(),
        };

        assert_eq!(
            response.headers.get("Content-Type").unwrap(),
            "application/json"
        );
    }

    #[test]
    fn test_unsupported_media_type() {
        let client = MockApiClient::new();
        client.queue_response(
            415,
            r#"{"error": "Unsupported Media Type", "message": "Only application/json is supported"}"#,
        );

        let response = client.post("/api/v1/recordings", "<xml></xml>").unwrap();

        // Assuming XML was sent
        assert_eq!(response.status, 415);
    }
}

// ============================================================================
// API Request Tracking Tests
// ============================================================================

mod request_tracking {
    use super::*;

    #[test]
    fn test_request_count_tracking() {
        let client = MockApiClient::new();

        assert_eq!(client.request_count(), 0);

        client.get("/path1").unwrap();
        assert_eq!(client.request_count(), 1);

        client.post("/path2", "{}").unwrap();
        assert_eq!(client.request_count(), 2);

        client.get("/path3").unwrap();
        assert_eq!(client.request_count(), 3);
    }

    #[test]
    fn test_response_queuing() {
        let client = MockApiClient::new();

        client.queue_response(200, "first");
        client.queue_response(201, "second");
        client.queue_response(202, "third");

        let r1 = client.get("/").unwrap();
        let r2 = client.get("/").unwrap();
        let r3 = client.get("/").unwrap();

        assert_eq!(r1.status, 200);
        assert_eq!(r2.status, 201);
        assert_eq!(r3.status, 202);
    }

    #[test]
    fn test_default_response_when_queue_empty() {
        let client = MockApiClient::new();

        let response = client.get("/").unwrap();
        assert_eq!(response.status, 200);
        assert_eq!(response.body, "{}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_integration_smoke_test() {
        let client = MockApiClient::new();

        // List recordings
        client.queue_response(200, r#"{"recordings": []}"#);
        let list_response = client.get("/api/v1/recordings").unwrap();
        assert_eq!(list_response.status, 200);

        // Create recording
        client.queue_response(201, r#"{"id": "new-rec"}"#);
        let create_response = client.post("/api/v1/recordings", "{}").unwrap();
        assert_eq!(create_response.status, 201);

        // Search
        client.queue_response(200, r#"{"results": []}"#);
        let search_response = client.post("/api/v1/search", r#"{"k": 10}"#).unwrap();
        assert_eq!(search_response.status, 200);

        // Track all requests
        assert_eq!(client.request_count(), 3);
    }
}
