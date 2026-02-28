//! IndexedDB storage implementation for WASM
//!
//! Uses web-sys bindings to interact with the browser's IndexedDB API
//! for persistent storage of RvLite state.

use super::state::RvLiteState;
use js_sys::{Object, Reflect};
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{IdbDatabase, IdbObjectStore, IdbRequest, IdbTransaction, IdbTransactionMode};

const DB_NAME: &str = "rvlite_db";
const DB_VERSION: u32 = 1;
const STORE_NAME: &str = "state";
const STATE_KEY: &str = "main";

/// IndexedDB storage backend for RvLite persistence
pub struct IndexedDBStorage {
    db: Option<IdbDatabase>,
}

impl IndexedDBStorage {
    /// Create a new IndexedDB storage instance
    pub fn new() -> Self {
        Self { db: None }
    }

    /// Initialize and open the IndexedDB database
    pub async fn init(&mut self) -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("No window"))?;
        let indexed_db = window
            .indexed_db()?
            .ok_or_else(|| JsValue::from_str("IndexedDB not available"))?;

        let open_request = indexed_db.open_with_u32(DB_NAME, DB_VERSION)?;

        // Handle database upgrade (create object store if needed)
        let onupgradeneeded = Closure::once(Box::new(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let request: IdbRequest = target.unchecked_into();
            let db: IdbDatabase = request.result().unwrap().unchecked_into();

            // Create object store if it doesn't exist
            if !db.object_store_names().contains(STORE_NAME) {
                db.create_object_store(STORE_NAME).unwrap();
            }
        }) as Box<dyn FnOnce(_)>);

        open_request.set_onupgradeneeded(Some(onupgradeneeded.as_ref().unchecked_ref()));
        onupgradeneeded.forget(); // Prevent closure from being dropped

        // Wait for database to open using JsFuture
        let db_result = wait_for_request(&open_request).await?;
        let db: IdbDatabase = db_result.unchecked_into();

        self.db = Some(db);
        Ok(())
    }

    /// Check if IndexedDB is available
    pub fn is_available() -> bool {
        web_sys::window()
            .and_then(|w| w.indexed_db().ok().flatten())
            .is_some()
    }

    /// Save state to IndexedDB
    pub async fn save(&self, state: &RvLiteState) -> Result<(), JsValue> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Database not initialized. Call init() first."))?;

        // Convert state to JsValue
        let js_state = serde_wasm_bindgen::to_value(state)?;

        // Start transaction
        let store_names = js_sys::Array::new();
        store_names.push(&JsValue::from_str(STORE_NAME));

        let transaction =
            db.transaction_with_str_sequence_and_mode(&store_names, IdbTransactionMode::Readwrite)?;

        let store = transaction.object_store(STORE_NAME)?;

        // Put state with key
        let request = store.put_with_key(&js_state, &JsValue::from_str(STATE_KEY))?;

        // Wait for completion
        wait_for_request(&request).await?;

        Ok(())
    }

    /// Load state from IndexedDB
    pub async fn load(&self) -> Result<Option<RvLiteState>, JsValue> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Database not initialized. Call init() first."))?;

        // Start read transaction
        let transaction = db.transaction_with_str(STORE_NAME)?;
        let store = transaction.object_store(STORE_NAME)?;

        // Get state by key
        let request = store.get(&JsValue::from_str(STATE_KEY))?;

        // Wait for result
        let result = wait_for_request(&request).await?;

        if result.is_undefined() || result.is_null() {
            return Ok(None);
        }

        // Deserialize state
        let state: RvLiteState = serde_wasm_bindgen::from_value(result)?;
        Ok(Some(state))
    }

    /// Delete all stored state
    pub async fn clear(&self) -> Result<(), JsValue> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Database not initialized. Call init() first."))?;

        let store_names = js_sys::Array::new();
        store_names.push(&JsValue::from_str(STORE_NAME));

        let transaction =
            db.transaction_with_str_sequence_and_mode(&store_names, IdbTransactionMode::Readwrite)?;

        let store = transaction.object_store(STORE_NAME)?;
        let request = store.clear()?;

        wait_for_request(&request).await?;
        Ok(())
    }

    /// Check if state exists in storage
    pub async fn exists(&self) -> Result<bool, JsValue> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Database not initialized. Call init() first."))?;

        let transaction = db.transaction_with_str(STORE_NAME)?;
        let store = transaction.object_store(STORE_NAME)?;

        let request = store.count_with_key(&JsValue::from_str(STATE_KEY))?;
        let result = wait_for_request(&request).await?;

        let count = result.as_f64().unwrap_or(0.0) as u32;
        Ok(count > 0)
    }

    /// Get storage info (for debugging)
    pub async fn get_info(&self) -> Result<JsValue, JsValue> {
        let db = self
            .db
            .as_ref()
            .ok_or_else(|| JsValue::from_str("Database not initialized. Call init() first."))?;

        let transaction = db.transaction_with_str(STORE_NAME)?;
        let store = transaction.object_store(STORE_NAME)?;

        let count_request = store.count()?;
        let count = wait_for_request(&count_request).await?;

        let info = Object::new();
        Reflect::set(&info, &"database".into(), &DB_NAME.into())?;
        Reflect::set(&info, &"store".into(), &STORE_NAME.into())?;
        Reflect::set(&info, &"entries".into(), &count)?;

        Ok(info.into())
    }

    /// Close the database connection
    pub fn close(&mut self) {
        if let Some(db) = self.db.take() {
            db.close();
        }
    }
}

impl Default for IndexedDBStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IndexedDBStorage {
    fn drop(&mut self) {
        self.close();
    }
}

/// Wait for an IdbRequest to complete and return the result
async fn wait_for_request(request: &IdbRequest) -> Result<JsValue, JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        // Success handler
        let resolve_clone = resolve.clone();
        let onsuccess = Closure::once(Box::new(move |_event: web_sys::Event| {
            // Note: We can't access request here due to lifetime issues
            // The result will be passed through the event
            resolve_clone.call0(&JsValue::NULL).unwrap();
        }) as Box<dyn FnOnce(_)>);

        // Error handler
        let onerror = Closure::once(Box::new(move |_event: web_sys::Event| {
            reject
                .call1(&JsValue::NULL, &JsValue::from_str("IndexedDB error"))
                .unwrap();
        }) as Box<dyn FnOnce(_)>);

        request.set_onsuccess(Some(onsuccess.as_ref().unchecked_ref()));
        request.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        onsuccess.forget();
        onerror.forget();
    });

    JsFuture::from(promise).await?;

    // Get the result after the request completes
    request.result()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: IndexedDB tests require a browser environment
    // These are placeholder tests for compilation verification

    #[test]
    fn test_storage_new() {
        let storage = IndexedDBStorage::new();
        assert!(storage.db.is_none());
    }
}
