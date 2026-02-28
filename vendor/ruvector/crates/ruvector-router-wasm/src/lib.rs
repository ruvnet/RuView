//! WASM bindings for browser and WASI environments

use ruvector_router_core::{
    DistanceMetric as CoreDistanceMetric, SearchQuery as CoreSearchQuery, VectorDB as CoreVectorDB,
    VectorEntry as CoreVectorEntry,
};
use std::collections::HashMap;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

#[wasm_bindgen]
#[derive(Clone, Copy)]
pub enum DistanceMetric {
    Euclidean,
    Cosine,
    DotProduct,
    Manhattan,
}

impl From<DistanceMetric> for CoreDistanceMetric {
    fn from(metric: DistanceMetric) -> Self {
        match metric {
            DistanceMetric::Euclidean => CoreDistanceMetric::Euclidean,
            DistanceMetric::Cosine => CoreDistanceMetric::Cosine,
            DistanceMetric::DotProduct => CoreDistanceMetric::DotProduct,
            DistanceMetric::Manhattan => CoreDistanceMetric::Manhattan,
        }
    }
}

#[wasm_bindgen]
pub struct VectorDB {
    db: CoreVectorDB,
}

#[wasm_bindgen]
impl VectorDB {
    #[wasm_bindgen(constructor)]
    pub fn new(dimensions: usize, storage_path: Option<String>) -> Result<VectorDB, JsValue> {
        console_log!("Initializing VectorDB with {} dimensions", dimensions);

        let mut builder = CoreVectorDB::builder().dimensions(dimensions);

        if let Some(path) = storage_path {
            builder = builder.storage_path(path);
        }

        let db = builder
            .build()
            .map_err(|e| JsValue::from_str(&format!("Failed to create database: {}", e)))?;

        Ok(VectorDB { db })
    }

    #[wasm_bindgen]
    pub fn insert(&mut self, id: String, vector: Vec<f32>) -> Result<String, JsValue> {
        let entry = CoreVectorEntry {
            id: id.clone(),
            vector,
            metadata: HashMap::new(),
            timestamp: 0, // WASM doesn't have chrono in no_std easily
        };

        self.db
            .insert(entry)
            .map_err(|e| JsValue::from_str(&format!("Insert failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn search(&self, vector: Vec<f32>, k: usize) -> Result<JsValue, JsValue> {
        let query = CoreSearchQuery {
            vector,
            k,
            filters: None,
            threshold: None,
            ef_search: None,
        };

        let results = self
            .db
            .search(query)
            .map_err(|e| JsValue::from_str(&format!("Search failed: {}", e)))?;

        // Convert results to JS value
        let js_results: Vec<JsValue> = results
            .into_iter()
            .map(|r| {
                let obj = js_sys::Object::new();
                js_sys::Reflect::set(&obj, &"id".into(), &r.id.into()).ok();
                js_sys::Reflect::set(&obj, &"score".into(), &r.score.into()).ok();
                obj.into()
            })
            .collect();

        Ok(js_sys::Array::from_iter(js_results).into())
    }

    #[wasm_bindgen]
    pub fn delete(&mut self, id: String) -> Result<bool, JsValue> {
        self.db
            .delete(&id)
            .map_err(|e| JsValue::from_str(&format!("Delete failed: {}", e)))
    }

    #[wasm_bindgen]
    pub fn count(&self) -> Result<usize, JsValue> {
        self.db
            .count()
            .map_err(|e| JsValue::from_str(&format!("Count failed: {}", e)))
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    console_log!("Ruvector WASM module loaded");
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::*;

    #[wasm_bindgen_test]
    fn test_vector_db_creation() {
        let db = VectorDB::new(3, None);
        assert!(db.is_ok());
    }
}
