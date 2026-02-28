// API server integration tests
//
// Tests HTTP API endpoints, authentication, rate limiting, and async processing

use super::*;
use reqwest::{multipart, Client, StatusCode};
use serde_json::json;
use tokio;

#[tokio::test]
async fn test_api_post_text_with_file() {
    let test_server = TestServer::start_api()
        .await
        .expect("Failed to start API server");
    let client = Client::new();

    // Create test image
    let image = images::generate_simple_equation("x + y");
    image.save("/tmp/api_test.png").unwrap();
    let image_bytes = std::fs::read("/tmp/api_test.png").unwrap();

    // Create multipart form
    let form = multipart::Form::new().part(
        "file",
        multipart::Part::bytes(image_bytes)
            .file_name("equation.png")
            .mime_str("image/png")
            .unwrap(),
    );

    // POST to /v3/text
    let response = client
        .post(&format!("{}/v3/text", test_server.base_url()))
        .header("app_id", "test_app_id")
        .header("app_key", "test_app_key")
        .multipart(form)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(response.status(), StatusCode::OK);

    let result: serde_json::Value = response.json().await.unwrap();
    assert!(result.get("request_id").is_some(), "Should have request_id");
    assert!(result.get("text").is_some(), "Should have text field");
    assert!(
        result.get("processing_time_ms").is_some(),
        "Should have processing time"
    );

    test_server.shutdown().await;
}

#[tokio::test]
async fn test_api_authentication_validation() {
    let test_server = TestServer::start_api()
        .await
        .expect("Failed to start API server");
    let client = Client::new();

    let payload = json!({
        "src": "base64data"
    });

    // Test missing auth
    let response = client
        .post(&format!("{}/v3/text", test_server.base_url()))
        .json(&payload)
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        response.status(),
        StatusCode::UNAUTHORIZED,
        "Should require authentication"
    );

    test_server.shutdown().await;
}
