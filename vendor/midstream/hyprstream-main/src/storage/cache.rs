use std::sync::Arc;
use std::time::{SystemTime, Duration, UNIX_EPOCH};
use tokio::sync::RwLock;
use tonic::Status;

/// Shared cache eviction manager that can be used across different storage backends.
#[derive(Clone)]
pub struct CacheManager {
    ttl: Option<u64>,
    last_eviction: Arc<RwLock<SystemTime>>,
    min_eviction_interval: Duration,
}

impl CacheManager {
    /// Creates a new cache manager with the specified TTL.
    pub fn new(ttl: Option<u64>) -> Self {
        Self {
            ttl,
            last_eviction: Arc::new(RwLock::new(SystemTime::now())),
            min_eviction_interval: Duration::from_secs(60), // Default 60s between evictions
        }
    }

    /// Sets a custom minimum interval between evictions.
    pub fn set_min_eviction_interval(&mut self, interval: Duration) {
        self.min_eviction_interval = interval;
    }

    /// Checks if eviction should be performed based on TTL and rate limiting.
    /// Returns the cutoff timestamp if eviction should proceed, None otherwise.
    pub async fn should_evict(&self) -> Result<Option<i64>, Status> {
        match self.ttl {
            None | Some(0) => Ok(None), // No TTL or TTL=0 means no eviction
            Some(ttl) => {
                let now = SystemTime::now();
                let last = *self.last_eviction.read().await;
                
                // Check if enough time has passed since last eviction
                if now.duration_since(last).unwrap_or(Duration::from_secs(0)) < self.min_eviction_interval {
                    return Ok(None);
                }

                // Calculate cutoff timestamp
                let cutoff = now
                    .duration_since(UNIX_EPOCH)
                    .map_err(|e| Status::internal(e.to_string()))?
                    .as_secs() as i64
                    - ttl as i64;

                // Update last eviction time
                *self.last_eviction.write().await = now;

                Ok(Some(cutoff))
            }
        }
    }

    /// Generates an optimized SQL query for evicting expired entries.
    pub fn eviction_query(&self, cutoff: i64) -> String {
        format!(
            "DELETE FROM metrics USING (
                SELECT timestamp 
                FROM metrics 
                WHERE timestamp < {} 
                LIMIT 10000
            ) as expired 
            WHERE metrics.timestamp = expired.timestamp",
            cutoff
        )
    }
}

/// Trait for storage backends that support cache eviction.
#[async_trait::async_trait]
pub trait CacheEviction {
    /// Executes the eviction query in the background.
    async fn execute_eviction(&self, query: &str) -> Result<(), Status>;
} 