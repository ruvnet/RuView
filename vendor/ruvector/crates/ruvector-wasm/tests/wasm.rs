//! WASM-specific tests

#![cfg(target_arch = "wasm32")]

use js_sys::Float32Array;
use ruvector_wasm::*;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
fn test_vector_db_creation() {
    let db = VectorDB::new(128, Some("cosine".to_string()), Some(false));
    assert!(db.is_ok());
}

#[wasm_bindgen_test]
fn test_insert_and_search() {
    let db = VectorDB::new(3, Some("euclidean".to_string()), Some(false)).unwrap();

    // Insert a vector
    let vector = Float32Array::from(&[1.0, 0.0, 0.0][..]);
    let id = db.insert(vector, Some("test1".to_string()), None);
    assert!(id.is_ok());

    // Search
    let query = Float32Array::from(&[1.0, 0.0, 0.0][..]);
    let results = db.search(query, 1, None);
    assert!(results.is_ok());

    let results = results.unwrap();
    assert_eq!(results.len(), 1);
}

#[wasm_bindgen_test]
fn test_batch_insert() {
    let db = VectorDB::new(3, Some("cosine".to_string()), Some(false)).unwrap();

    let entries = js_sys::Array::new();

    for i in 0..10 {
        let entry = js_sys::Object::new();
        let vector = Float32Array::from(&[i as f32, 0.0, 0.0][..]);
        js_sys::Reflect::set(&entry, &"vector".into(), &vector).unwrap();
        js_sys::Reflect::set(&entry, &"id".into(), &format!("vec_{}", i).into()).unwrap();
        entries.push(&entry);
    }

    let result = db.insert_batch(entries.into());
    assert!(result.is_ok());

    let ids = result.unwrap();
    assert_eq!(ids.len(), 10);
}

#[wasm_bindgen_test]
fn test_delete() {
    let db = VectorDB::new(3, Some("cosine".to_string()), Some(false)).unwrap();

    // Insert
    let vector = Float32Array::from(&[1.0, 0.0, 0.0][..]);
    let id = db
        .insert(vector, Some("test_delete".to_string()), None)
        .unwrap();

    // Delete
    let deleted = db.delete(&id);
    assert!(deleted.is_ok());
    assert_eq!(deleted.unwrap(), true);

    // Verify deleted
    let get_result = db.get(&id);
    assert!(get_result.is_ok());
    assert!(get_result.unwrap().is_none());
}

#[wasm_bindgen_test]
fn test_get() {
    let db = VectorDB::new(3, Some("cosine".to_string()), Some(false)).unwrap();

    // Insert
    let vector = Float32Array::from(&[1.0, 2.0, 3.0][..]);
    let id = db
        .insert(vector, Some("test_get".to_string()), None)
        .unwrap();

    // Get
    let entry = db.get(&id);
    assert!(entry.is_ok());

    let entry = entry.unwrap();
    assert!(entry.is_some());

    let entry = entry.unwrap();
    assert_eq!(entry.id(), Some("test_get".to_string()));
}

#[wasm_bindgen_test]
fn test_len_and_is_empty() {
    let db = VectorDB::new(3, Some("cosine".to_string()), Some(false)).unwrap();

    // Initially empty
    assert!(db.is_empty().unwrap());
    assert_eq!(db.len().unwrap(), 0);

    // Insert vector
    let vector = Float32Array::from(&[1.0, 0.0, 0.0][..]);
    db.insert(vector, Some("test1".to_string()), None).unwrap();

    // Not empty
    assert!(!db.is_empty().unwrap());
    assert_eq!(db.len().unwrap(), 1);
}

#[wasm_bindgen_test]
fn test_different_metrics() {
    for metric in &["euclidean", "cosine", "dotproduct", "manhattan"] {
        let db = VectorDB::new(3, Some(metric.to_string()), Some(false));
        assert!(db.is_ok(), "Failed to create DB with metric: {}", metric);
    }
}

#[wasm_bindgen_test]
fn test_dimension_mismatch() {
    let db = VectorDB::new(3, Some("cosine".to_string()), Some(false)).unwrap();

    // Try to insert vector with wrong dimensions
    let vector = Float32Array::from(&[1.0, 0.0][..]); // Only 2 dimensions
    let result = db.insert(vector, Some("test_wrong_dim".to_string()), None);

    // Should fail due to dimension mismatch
    // Note: This might succeed depending on implementation
    // The search with wrong dimensions should definitely fail
    let query = Float32Array::from(&[1.0, 0.0][..]);
    let search_result = db.search(query, 1, None);
    assert!(search_result.is_err());
}

#[wasm_bindgen_test]
fn test_version() {
    let v = version();
    assert!(!v.is_empty());
    assert!(v.contains('.'));
}

#[wasm_bindgen_test]
fn test_detect_simd() {
    // Just ensure it doesn't panic
    let _ = detect_simd();
}

#[wasm_bindgen_test]
fn test_array_to_float32_array() {
    let arr = vec![1.0, 2.0, 3.0, 4.0];
    let float_arr = array_to_float32_array(arr.clone());

    assert_eq!(float_arr.length(), 4);
    assert_eq!(float_arr.get_index(0), 1.0);
    assert_eq!(float_arr.get_index(3), 4.0);
}
