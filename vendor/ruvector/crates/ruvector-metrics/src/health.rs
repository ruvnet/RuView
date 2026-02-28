use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ReadinessResponse {
    pub status: HealthStatus,
    pub collections_count: usize,
    pub total_vectors: usize,
    pub details: HashMap<String, CollectionHealth>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CollectionHealth {
    pub status: HealthStatus,
    pub vectors_count: usize,
    pub last_updated: Option<String>,
}

#[derive(Debug)]
pub struct CollectionStats {
    pub name: String,
    pub vectors_count: usize,
    pub last_updated: Option<chrono::DateTime<chrono::Utc>>,
}

pub struct HealthChecker {
    start_time: Instant,
    version: String,
}

impl HealthChecker {
    /// Create a new health checker
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Create a health checker with custom version
    pub fn with_version(version: String) -> Self {
        Self {
            start_time: Instant::now(),
            version,
        }
    }

    /// Get basic health status
    pub fn health(&self) -> HealthResponse {
        HealthResponse {
            status: HealthStatus::Healthy,
            version: self.version.clone(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
        }
    }

    /// Get detailed readiness status
    pub fn readiness(&self, collections: &[CollectionStats]) -> ReadinessResponse {
        let total_vectors: usize = collections.iter().map(|c| c.vectors_count).sum();

        let mut details = HashMap::new();
        for collection in collections {
            let status = if collection.vectors_count > 0 {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            };

            details.insert(
                collection.name.clone(),
                CollectionHealth {
                    status,
                    vectors_count: collection.vectors_count,
                    last_updated: collection.last_updated.map(|dt| dt.to_rfc3339()),
                },
            );
        }

        let overall_status = if collections.is_empty() {
            HealthStatus::Degraded
        } else if details.values().all(|c| c.status == HealthStatus::Healthy) {
            HealthStatus::Healthy
        } else if details.values().any(|c| c.status == HealthStatus::Healthy) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Unhealthy
        };

        ReadinessResponse {
            status: overall_status,
            collections_count: collections.len(),
            total_vectors,
            details,
        }
    }
}

impl Default for HealthChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_checker_new() {
        let checker = HealthChecker::new();
        let health = checker.health();

        assert_eq!(health.status, HealthStatus::Healthy);
        assert_eq!(health.version, env!("CARGO_PKG_VERSION"));
        // Uptime is always >= 0 for u64, so just check it exists
        let _ = health.uptime_seconds;
    }

    #[test]
    fn test_readiness_empty_collections() {
        let checker = HealthChecker::new();
        let readiness = checker.readiness(&[]);

        assert_eq!(readiness.status, HealthStatus::Degraded);
        assert_eq!(readiness.collections_count, 0);
        assert_eq!(readiness.total_vectors, 0);
    }

    #[test]
    fn test_readiness_with_collections() {
        let checker = HealthChecker::new();
        let collections = vec![
            CollectionStats {
                name: "test1".to_string(),
                vectors_count: 100,
                last_updated: Some(chrono::Utc::now()),
            },
            CollectionStats {
                name: "test2".to_string(),
                vectors_count: 200,
                last_updated: None,
            },
        ];

        let readiness = checker.readiness(&collections);

        assert_eq!(readiness.status, HealthStatus::Healthy);
        assert_eq!(readiness.collections_count, 2);
        assert_eq!(readiness.total_vectors, 300);
        assert_eq!(readiness.details.len(), 2);
    }

    #[test]
    fn test_readiness_with_empty_collection() {
        let checker = HealthChecker::new();
        let collections = vec![CollectionStats {
            name: "empty".to_string(),
            vectors_count: 0,
            last_updated: None,
        }];

        let readiness = checker.readiness(&collections);

        // Collection exists but is empty (degraded), so overall is Unhealthy
        // since no collections are in healthy state
        assert_eq!(readiness.status, HealthStatus::Unhealthy);
        assert_eq!(readiness.collections_count, 1);
        assert_eq!(readiness.total_vectors, 0);
    }

    #[test]
    fn test_collection_health_status() {
        let checker = HealthChecker::new();
        let collections = vec![
            CollectionStats {
                name: "healthy".to_string(),
                vectors_count: 100,
                last_updated: Some(chrono::Utc::now()),
            },
            CollectionStats {
                name: "degraded".to_string(),
                vectors_count: 0,
                last_updated: None,
            },
        ];

        let readiness = checker.readiness(&collections);

        assert_eq!(
            readiness.details.get("healthy").unwrap().status,
            HealthStatus::Healthy
        );
        assert_eq!(
            readiness.details.get("degraded").unwrap().status,
            HealthStatus::Degraded
        );
    }
}
