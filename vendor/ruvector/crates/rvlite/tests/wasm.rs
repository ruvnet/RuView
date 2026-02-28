//! WASM-specific tests for RvLite
//!
//! These tests run in a browser-like environment using wasm-bindgen-test.

#![cfg(target_arch = "wasm32")]

use rvlite::RvLite;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_wasm_initialization() {
    let db = RvLite::new().unwrap();
    assert!(db.is_ready());
}

#[wasm_bindgen_test]
fn test_wasm_version() {
    let db = RvLite::new().unwrap();
    let version = db.get_version();
    assert!(version.contains("0.1.0"));
}

#[wasm_bindgen_test]
fn test_wasm_features() {
    let db = RvLite::new().unwrap();
    let features = db.get_features().unwrap();
    // Features should be an array
    assert!(js_sys::Array::is_array(&features));
}

#[wasm_bindgen_test]
async fn test_wasm_sql_not_implemented() {
    let db = RvLite::new().unwrap();
    let result = db.sql("SELECT 1".to_string()).await;
    // Should return error for now (not implemented)
    assert!(result.is_err());
}

#[wasm_bindgen_test]
async fn test_wasm_cypher_not_implemented() {
    let db = RvLite::new().unwrap();
    let result = db.cypher("MATCH (n) RETURN n".to_string()).await;
    // Should return error for now (not implemented)
    assert!(result.is_err());
}

#[wasm_bindgen_test]
async fn test_wasm_sparql_not_implemented() {
    let db = RvLite::new().unwrap();
    let result = db.sparql("SELECT ?s WHERE { ?s ?p ?o }".to_string()).await;
    // Should return error for now (not implemented)
    assert!(result.is_err());
}
