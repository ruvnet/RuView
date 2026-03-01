//! WASM integration tests for OSpipe.
//!
//! These tests run in a browser-like environment using `wasm-bindgen-test`.
//! Execute with:
//!
//! ```bash
//! wasm-pack test --headless --chrome -- --test wasm
//! ```

#![cfg(target_arch = "wasm32")]

use wasm_bindgen::JsValue;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use ospipe::wasm::bindings::OsPipeWasm;

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_create_instance() {
    let instance = OsPipeWasm::new(384);
    assert_eq!(instance.len(), 0);
    assert!(instance.is_empty());
}

#[wasm_bindgen_test]
fn test_create_with_custom_dimension() {
    let instance = OsPipeWasm::new(128);
    assert_eq!(instance.len(), 0);

    let stats_json = instance.stats();
    assert!(
        stats_json.contains("\"dimension\":128"),
        "Stats should report dimension 128, got: {}",
        stats_json
    );
}

// ---------------------------------------------------------------------------
// Insert + Search roundtrip
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_insert_and_search_roundtrip() {
    let mut instance = OsPipeWasm::new(4);

    // Insert two vectors.
    let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0];

    instance
        .insert("a", &emb_a, r#"{"label":"a"}"#, 1000.0)
        .expect("insert a");
    instance
        .insert("b", &emb_b, r#"{"label":"b"}"#, 2000.0)
        .expect("insert b");

    assert_eq!(instance.len(), 2);
    assert!(!instance.is_empty());

    // Searching with emb_a should return "a" as the top hit.
    let results: JsValue = instance.search(&emb_a, 2).expect("search");
    let results_str = js_sys::JSON::stringify(&results)
        .expect("stringify")
        .as_string()
        .expect("as_string");

    assert!(
        results_str.contains("\"id\":\"a\""),
        "Top result should be 'a', got: {}",
        results_str
    );
}

#[wasm_bindgen_test]
fn test_insert_dimension_mismatch() {
    let mut instance = OsPipeWasm::new(4);
    let wrong_dim: Vec<f32> = vec![1.0, 2.0]; // dimension 2, expects 4

    let result = instance.insert("bad", &wrong_dim, "{}", 0.0);
    assert!(result.is_err(), "Should reject mismatched dimension");
}

// ---------------------------------------------------------------------------
// Filtered search
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_search_filtered_by_time() {
    let mut instance = OsPipeWasm::new(4);

    let emb: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    instance
        .insert("early", &emb, "{}", 1000.0)
        .expect("insert early");
    instance
        .insert("late", &emb, "{}", 5000.0)
        .expect("insert late");

    // Filter to only the early entry (timestamp range [0, 2000]).
    let results: JsValue = instance
        .search_filtered(&emb, 10, 0.0, 2000.0)
        .expect("search_filtered");
    let results_str = js_sys::JSON::stringify(&results)
        .expect("stringify")
        .as_string()
        .expect("as_string");

    assert!(
        results_str.contains("\"id\":\"early\""),
        "Filtered results should include 'early', got: {}",
        results_str
    );
    assert!(
        !results_str.contains("\"id\":\"late\""),
        "Filtered results should exclude 'late', got: {}",
        results_str
    );
}

// ---------------------------------------------------------------------------
// embed_text
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_embed_text_returns_correct_dimension() {
    let instance = OsPipeWasm::new(384);
    let embedding = instance.embed_text("hello world");
    assert_eq!(
        embedding.len(),
        384,
        "embed_text should return a vector of the configured dimension"
    );
}

#[wasm_bindgen_test]
fn test_embed_text_is_deterministic() {
    let instance = OsPipeWasm::new(64);
    let a = instance.embed_text("test input");
    let b = instance.embed_text("test input");
    assert_eq!(a, b, "Same input text should produce identical embeddings");
}

#[wasm_bindgen_test]
fn test_embed_text_different_inputs_differ() {
    let instance = OsPipeWasm::new(64);
    let a = instance.embed_text("alpha");
    let b = instance.embed_text("beta");
    assert_ne!(a, b, "Different inputs should produce different embeddings");
}

// ---------------------------------------------------------------------------
// safety_check
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_safety_check_allow() {
    let instance = OsPipeWasm::new(4);
    let decision = instance.safety_check("the weather is nice today");
    assert_eq!(decision, "allow");
}

#[wasm_bindgen_test]
fn test_safety_check_deny_credit_card() {
    let instance = OsPipeWasm::new(4);
    let decision = instance.safety_check("card number 4111-1111-1111-1111");
    assert_eq!(decision, "deny");
}

#[wasm_bindgen_test]
fn test_safety_check_deny_ssn() {
    let instance = OsPipeWasm::new(4);
    let decision = instance.safety_check("my ssn is 123-45-6789");
    assert_eq!(decision, "deny");
}

#[wasm_bindgen_test]
fn test_safety_check_redact_password() {
    let instance = OsPipeWasm::new(4);
    let decision = instance.safety_check("my password is hunter2");
    assert_eq!(decision, "redact");
}

// ---------------------------------------------------------------------------
// route_query
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_route_query_temporal() {
    let instance = OsPipeWasm::new(4);
    let route = instance.route_query("what happened yesterday");
    assert_eq!(route, "Temporal");
}

#[wasm_bindgen_test]
fn test_route_query_keyword_short() {
    let instance = OsPipeWasm::new(4);
    let route = instance.route_query("rust");
    assert_eq!(route, "Keyword");
}

#[wasm_bindgen_test]
fn test_route_query_keyword_quoted() {
    let instance = OsPipeWasm::new(4);
    let route = instance.route_query("\"exact phrase\"");
    assert_eq!(route, "Keyword");
}

#[wasm_bindgen_test]
fn test_route_query_graph() {
    let instance = OsPipeWasm::new(4);
    let route = instance.route_query("things related to authentication module");
    assert_eq!(route, "Graph");
}

#[wasm_bindgen_test]
fn test_route_query_hybrid_default() {
    let instance = OsPipeWasm::new(4);
    let route = instance.route_query("explain how neural networks learn patterns");
    assert_eq!(route, "Hybrid");
}

// ---------------------------------------------------------------------------
// Deduplication
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_is_duplicate_identical() {
    let mut instance = OsPipeWasm::new(4);
    let emb: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];

    instance
        .insert("original", &emb, "{}", 0.0)
        .expect("insert");

    assert!(
        instance.is_duplicate(&emb, 0.99),
        "Identical embedding should be detected as duplicate"
    );
}

#[wasm_bindgen_test]
fn test_is_not_duplicate_orthogonal() {
    let mut instance = OsPipeWasm::new(4);
    let emb_a: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    let emb_b: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0];

    instance.insert("a", &emb_a, "{}", 0.0).expect("insert");

    assert!(
        !instance.is_duplicate(&emb_b, 0.5),
        "Orthogonal embedding should not be a duplicate at threshold 0.5"
    );
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

#[wasm_bindgen_test]
fn test_stats_json() {
    let mut instance = OsPipeWasm::new(16);
    let emb: Vec<f32> = vec![0.0; 16];

    instance.insert("x", &emb, "{}", 0.0).expect("insert");

    let stats = instance.stats();
    assert!(stats.contains("\"dimension\":16"), "Stats: {}", stats);
    assert!(stats.contains("\"total_embeddings\":1"), "Stats: {}", stats);
    assert!(
        stats.contains("\"memory_estimate_bytes\""),
        "Stats: {}",
        stats
    );
}
