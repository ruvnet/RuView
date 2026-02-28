//! PostgreSQL storage backend for hooks intelligence data
//!
//! This module provides PostgreSQL-based storage for the hooks system,
//! using the ruvector extension for vector operations.
//!
//! Enable with the `postgres` feature flag.

#[cfg(feature = "postgres")]
use deadpool_postgres::{Config, Pool, Runtime};
#[cfg(feature = "postgres")]
use tokio_postgres::NoTls;

use std::env;

/// PostgreSQL storage configuration
#[derive(Debug, Clone)]
pub struct PostgresConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub dbname: String,
}

impl PostgresConfig {
    /// Create config from environment variables
    pub fn from_env() -> Option<Self> {
        // Try RUVECTOR_POSTGRES_URL first, then DATABASE_URL
        if let Ok(url) = env::var("RUVECTOR_POSTGRES_URL").or_else(|_| env::var("DATABASE_URL")) {
            return Self::from_url(&url);
        }

        // Try individual environment variables
        let host = env::var("RUVECTOR_PG_HOST").unwrap_or_else(|_| "localhost".to_string());
        let port = env::var("RUVECTOR_PG_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(5432);
        let user = env::var("RUVECTOR_PG_USER").ok()?;
        let password = env::var("RUVECTOR_PG_PASSWORD").ok();
        let dbname = env::var("RUVECTOR_PG_DATABASE").unwrap_or_else(|_| "ruvector".to_string());

        Some(Self {
            host,
            port,
            user,
            password,
            dbname,
        })
    }

    /// Parse PostgreSQL connection URL
    pub fn from_url(url: &str) -> Option<Self> {
        // Parse postgres://user:password@host:port/dbname
        let url = url
            .strip_prefix("postgres://")
            .or_else(|| url.strip_prefix("postgresql://"))?;

        let (auth, rest) = url.split_once('@')?;
        let (user, password) = if auth.contains(':') {
            let (u, p) = auth.split_once(':')?;
            (u.to_string(), Some(p.to_string()))
        } else {
            (auth.to_string(), None)
        };

        let (host_port, dbname) = rest.split_once('/')?;
        let dbname = dbname.split('?').next()?.to_string();

        let (host, port) = if host_port.contains(':') {
            let (h, p) = host_port.split_once(':')?;
            (h.to_string(), p.parse().ok()?)
        } else {
            (host_port.to_string(), 5432)
        };

        Some(Self {
            host,
            port,
            user,
            password,
            dbname,
        })
    }
}

/// PostgreSQL storage backend for hooks
#[cfg(feature = "postgres")]
pub struct PostgresStorage {
    pool: Pool,
}

#[cfg(feature = "postgres")]
impl PostgresStorage {
    /// Create a new PostgreSQL storage backend
    pub async fn new(config: PostgresConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let mut cfg = Config::new();
        cfg.host = Some(config.host);
        cfg.port = Some(config.port);
        cfg.user = Some(config.user);
        cfg.password = config.password;
        cfg.dbname = Some(config.dbname);

        let pool = cfg.create_pool(Some(Runtime::Tokio1), NoTls)?;

        Ok(Self { pool })
    }

    /// Update Q-value for state-action pair
    pub async fn update_q(
        &self,
        state: &str,
        action: &str,
        reward: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute(
                "SELECT ruvector_hooks_update_q($1, $2, $3)",
                &[&state, &action, &reward],
            )
            .await?;
        Ok(())
    }

    /// Get best action for state
    pub async fn best_action(
        &self,
        state: &str,
        actions: &[String],
    ) -> Result<Option<(String, f32, f32)>, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let row = client
            .query_opt(
                "SELECT action, q_value, confidence FROM ruvector_hooks_best_action($1, $2)",
                &[&state, &actions],
            )
            .await?;

        Ok(row.map(|r| (r.get(0), r.get(1), r.get(2))))
    }

    /// Store content in semantic memory
    pub async fn remember(
        &self,
        memory_type: &str,
        content: &str,
        embedding: Option<&[f32]>,
        metadata: &serde_json::Value,
    ) -> Result<i32, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let metadata_str = serde_json::to_string(metadata)?;
        let row = client
            .query_one(
                "SELECT ruvector_hooks_remember($1, $2, $3, $4::jsonb)",
                &[&memory_type, &content, &embedding, &metadata_str],
            )
            .await?;

        Ok(row.get(0))
    }

    /// Search memory semantically
    pub async fn recall(
        &self,
        query_embedding: &[f32],
        limit: i32,
    ) -> Result<Vec<MemoryResult>, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT id, memory_type, content, metadata::text, similarity
                 FROM ruvector_hooks_recall($1, $2)",
                &[&query_embedding, &limit],
            )
            .await?;

        Ok(rows
            .iter()
            .map(|r| {
                let metadata_str: String = r.get(3);
                MemoryResult {
                    id: r.get(0),
                    memory_type: r.get(1),
                    content: r.get(2),
                    metadata: serde_json::from_str(&metadata_str).unwrap_or_default(),
                    similarity: r.get(4),
                }
            })
            .collect())
    }

    /// Record file sequence
    pub async fn record_sequence(
        &self,
        from_file: &str,
        to_file: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute(
                "SELECT ruvector_hooks_record_sequence($1, $2)",
                &[&from_file, &to_file],
            )
            .await?;
        Ok(())
    }

    /// Get suggested next files
    pub async fn suggest_next(
        &self,
        file: &str,
        limit: i32,
    ) -> Result<Vec<(String, i32)>, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let rows = client
            .query(
                "SELECT to_file, count FROM ruvector_hooks_suggest_next($1, $2)",
                &[&file, &limit],
            )
            .await?;

        Ok(rows.iter().map(|r| (r.get(0), r.get(1))).collect())
    }

    /// Record error pattern
    pub async fn record_error(
        &self,
        code: &str,
        error_type: &str,
        message: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute(
                "SELECT ruvector_hooks_record_error($1, $2, $3)",
                &[&code, &error_type, &message],
            )
            .await?;
        Ok(())
    }

    /// Register agent in swarm
    pub async fn swarm_register(
        &self,
        agent_id: &str,
        agent_type: &str,
        capabilities: &[String],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute(
                "SELECT ruvector_hooks_swarm_register($1, $2, $3)",
                &[&agent_id, &agent_type, &capabilities],
            )
            .await?;
        Ok(())
    }

    /// Record coordination between agents
    pub async fn swarm_coordinate(
        &self,
        source: &str,
        target: &str,
        weight: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute(
                "SELECT ruvector_hooks_swarm_coordinate($1, $2, $3)",
                &[&source, &target, &weight],
            )
            .await?;
        Ok(())
    }

    /// Get swarm statistics
    pub async fn swarm_stats(&self) -> Result<SwarmStats, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT * FROM ruvector_hooks_swarm_stats()", &[])
            .await?;

        Ok(SwarmStats {
            total_agents: row.get(0),
            active_agents: row.get(1),
            total_edges: row.get(2),
            avg_success_rate: row.get(3),
        })
    }

    /// Get overall statistics
    pub async fn get_stats(&self) -> Result<IntelligenceStats, Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        let row = client
            .query_one("SELECT * FROM ruvector_hooks_get_stats()", &[])
            .await?;

        Ok(IntelligenceStats {
            total_patterns: row.get(0),
            total_memories: row.get(1),
            total_trajectories: row.get(2),
            total_errors: row.get(3),
            session_count: row.get(4),
        })
    }

    /// Start a new session
    pub async fn session_start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let client = self.pool.get().await?;
        client
            .execute("SELECT ruvector_hooks_session_start()", &[])
            .await?;
        Ok(())
    }
}

/// Memory search result
#[derive(Debug)]
pub struct MemoryResult {
    pub id: i32,
    pub memory_type: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub similarity: f32,
}

/// Swarm statistics
#[derive(Debug)]
pub struct SwarmStats {
    pub total_agents: i64,
    pub active_agents: i64,
    pub total_edges: i64,
    pub avg_success_rate: f32,
}

/// Intelligence statistics
#[derive(Debug)]
pub struct IntelligenceStats {
    pub total_patterns: i64,
    pub total_memories: i64,
    pub total_trajectories: i64,
    pub total_errors: i64,
    pub session_count: i64,
}

/// Check if PostgreSQL is available
pub fn is_postgres_available() -> bool {
    PostgresConfig::from_env().is_some()
}

/// Storage backend selector
pub enum StorageBackend {
    #[cfg(feature = "postgres")]
    Postgres(PostgresStorage),
    Json(super::Intelligence),
}

impl StorageBackend {
    /// Create storage backend from environment
    #[cfg(feature = "postgres")]
    pub async fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        if let Some(config) = PostgresConfig::from_env() {
            match PostgresStorage::new(config).await {
                Ok(pg) => return Ok(Self::Postgres(pg)),
                Err(e) => {
                    eprintln!(
                        "Warning: PostgreSQL unavailable ({}), using JSON fallback",
                        e
                    );
                }
            }
        }
        Ok(Self::Json(super::Intelligence::new(
            super::get_intelligence_path(),
        )))
    }

    #[cfg(not(feature = "postgres"))]
    pub fn from_env() -> Self {
        Self::Json(super::Intelligence::new(super::get_intelligence_path()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_url() {
        let config =
            PostgresConfig::from_url("postgres://user:pass@localhost:5432/ruvector").unwrap();
        assert_eq!(config.host, "localhost");
        assert_eq!(config.port, 5432);
        assert_eq!(config.user, "user");
        assert_eq!(config.password, Some("pass".to_string()));
        assert_eq!(config.dbname, "ruvector");
    }

    #[test]
    fn test_config_from_url_no_password() {
        let config = PostgresConfig::from_url("postgres://user@localhost/ruvector").unwrap();
        assert_eq!(config.user, "user");
        assert_eq!(config.password, None);
    }

    #[test]
    fn test_config_from_url_with_query() {
        let config = PostgresConfig::from_url(
            "postgres://user:pass@localhost:5432/ruvector?sslmode=require",
        )
        .unwrap();
        assert_eq!(config.dbname, "ruvector");
    }
}
