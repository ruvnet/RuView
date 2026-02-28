use moka::future::Cache;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use super::{
    jobs::JobQueue,
    middleware::{create_rate_limiter, AppRateLimiter},
};

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    /// Job queue for async PDF processing
    pub job_queue: Arc<JobQueue>,

    /// Result cache
    pub cache: Cache<String, String>,

    /// Rate limiter
    pub rate_limiter: AppRateLimiter,

    /// Whether authentication is enabled
    pub auth_enabled: bool,

    /// Map of app_id -> hashed API key
    /// Keys should be stored as SHA-256 hashes, never in plaintext
    pub api_keys: Arc<HashMap<String, String>>,
}

impl AppState {
    /// Create a new application state instance with authentication disabled
    pub fn new() -> Self {
        Self {
            job_queue: Arc::new(JobQueue::new()),
            cache: create_cache(),
            rate_limiter: create_rate_limiter(),
            auth_enabled: false,
            api_keys: Arc::new(HashMap::new()),
        }
    }

    /// Create state with custom configuration
    pub fn with_config(max_jobs: usize, cache_size: u64) -> Self {
        Self {
            job_queue: Arc::new(JobQueue::with_capacity(max_jobs)),
            cache: Cache::builder()
                .max_capacity(cache_size)
                .time_to_live(Duration::from_secs(3600))
                .time_to_idle(Duration::from_secs(600))
                .build(),
            rate_limiter: create_rate_limiter(),
            auth_enabled: false,
            api_keys: Arc::new(HashMap::new()),
        }
    }

    /// Create state with authentication enabled
    pub fn with_auth(api_keys: HashMap<String, String>) -> Self {
        // Hash all provided API keys
        let hashed_keys: HashMap<String, String> = api_keys
            .into_iter()
            .map(|(app_id, key)| (app_id, hash_api_key(&key)))
            .collect();

        Self {
            job_queue: Arc::new(JobQueue::new()),
            cache: create_cache(),
            rate_limiter: create_rate_limiter(),
            auth_enabled: true,
            api_keys: Arc::new(hashed_keys),
        }
    }

    /// Add an API key (hashes the key before storing)
    pub fn add_api_key(&mut self, app_id: String, api_key: &str) {
        let hashed = hash_api_key(api_key);
        Arc::make_mut(&mut self.api_keys).insert(app_id, hashed);
        self.auth_enabled = true;
    }

    /// Enable or disable authentication
    pub fn set_auth_enabled(&mut self, enabled: bool) {
        self.auth_enabled = enabled;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Hash an API key using SHA-256
fn hash_api_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Create a cache with default configuration
fn create_cache() -> Cache<String, String> {
    Cache::builder()
        // Max 10,000 entries
        .max_capacity(10_000)
        // Time to live: 1 hour
        .time_to_live(Duration::from_secs(3600))
        // Time to idle: 10 minutes
        .time_to_idle(Duration::from_secs(600))
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_creation() {
        let state = AppState::new();
        assert!(Arc::strong_count(&state.job_queue) >= 1);
    }

    #[tokio::test]
    async fn test_state_with_config() {
        let state = AppState::with_config(100, 5000);
        assert!(Arc::strong_count(&state.job_queue) >= 1);
    }

    #[tokio::test]
    async fn test_cache_operations() {
        let state = AppState::new();

        // Insert value
        state
            .cache
            .insert("key1".to_string(), "value1".to_string())
            .await;

        // Retrieve value
        let value = state.cache.get(&"key1".to_string()).await;
        assert_eq!(value, Some("value1".to_string()));

        // Non-existent key
        let missing = state.cache.get(&"missing".to_string()).await;
        assert_eq!(missing, None);
    }
}
