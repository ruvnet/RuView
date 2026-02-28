//! SQLite/AgentDB integration for persistent storage

use crate::error::{Result, TinyDancerError};
use crate::types::Candidate;
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Arc;

/// Storage backend for candidates and routing history
pub struct Storage {
    conn: Arc<Mutex<Connection>>,
}

impl Storage {
    /// Create a new storage instance
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for concurrent access
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=1000000000;
             PRAGMA temp_store=memory;",
        )?;

        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };

        storage.init_schema()?;
        Ok(storage)
    }

    /// Create an in-memory storage instance
    pub fn in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let storage = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        storage.init_schema()?;
        Ok(storage)
    }

    /// Initialize database schema
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute(
            "CREATE TABLE IF NOT EXISTS candidates (
                id TEXT PRIMARY KEY,
                embedding BLOB NOT NULL,
                metadata TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                access_count INTEGER DEFAULT 0,
                success_rate REAL DEFAULT 0.0,
                last_accessed INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS routing_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                candidate_id TEXT NOT NULL,
                query_embedding BLOB NOT NULL,
                confidence REAL NOT NULL,
                use_lightweight INTEGER NOT NULL,
                uncertainty REAL NOT NULL,
                timestamp INTEGER NOT NULL,
                inference_time_us INTEGER NOT NULL,
                FOREIGN KEY(candidate_id) REFERENCES candidates(id)
            )",
            [],
        )?;

        // Create indexes
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_candidates_created_at ON candidates(created_at)",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_routing_timestamp ON routing_history(timestamp)",
            [],
        )?;

        Ok(())
    }

    /// Insert a candidate
    pub fn insert_candidate(&self, candidate: &Candidate) -> Result<()> {
        let conn = self.conn.lock();

        let embedding_bytes = bytemuck::cast_slice::<f32, u8>(&candidate.embedding);
        let metadata_json = serde_json::to_string(&candidate.metadata)?;

        conn.execute(
            "INSERT OR REPLACE INTO candidates
             (id, embedding, metadata, created_at, access_count, success_rate, last_accessed)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                &candidate.id,
                embedding_bytes,
                metadata_json,
                candidate.created_at,
                candidate.access_count,
                candidate.success_rate,
                chrono::Utc::now().timestamp()
            ],
        )?;

        Ok(())
    }

    /// Get a candidate by ID
    pub fn get_candidate(&self, id: &str) -> Result<Option<Candidate>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, embedding, metadata, created_at, access_count, success_rate
             FROM candidates WHERE id = ?1",
        )?;

        let mut rows = stmt.query(params![id])?;

        if let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let embedding_bytes: Vec<u8> = row.get(1)?;
            let metadata_json: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            let access_count: u64 = row.get(4)?;
            let success_rate: f32 = row.get(5)?;

            let embedding = bytemuck::cast_slice::<u8, f32>(&embedding_bytes).to_vec();
            let metadata = serde_json::from_str(&metadata_json)?;

            Ok(Some(Candidate {
                id,
                embedding,
                metadata,
                created_at,
                access_count,
                success_rate,
            }))
        } else {
            Ok(None)
        }
    }

    /// Query candidates with vector similarity search
    pub fn query_candidates(&self, limit: usize) -> Result<Vec<Candidate>> {
        let conn = self.conn.lock();

        let mut stmt = conn.prepare(
            "SELECT id, embedding, metadata, created_at, access_count, success_rate
             FROM candidates
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let rows = stmt.query_map(params![limit], |row| {
            let id: String = row.get(0)?;
            let embedding_bytes: Vec<u8> = row.get(1)?;
            let metadata_json: String = row.get(2)?;
            let created_at: i64 = row.get(3)?;
            let access_count: u64 = row.get(4)?;
            let success_rate: f32 = row.get(5)?;

            let embedding = bytemuck::cast_slice::<u8, f32>(&embedding_bytes).to_vec();
            let metadata = serde_json::from_str(&metadata_json).unwrap_or_default();

            Ok(Candidate {
                id,
                embedding,
                metadata,
                created_at,
                access_count,
                success_rate,
            })
        })?;

        let candidates: Result<Vec<Candidate>> = rows
            .map(|r| r.map_err(|e| TinyDancerError::DatabaseError(e)))
            .collect();

        candidates
    }

    /// Update access count for a candidate
    pub fn increment_access_count(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock();

        conn.execute(
            "UPDATE candidates
             SET access_count = access_count + 1,
                 last_accessed = ?1
             WHERE id = ?2",
            params![chrono::Utc::now().timestamp(), id],
        )?;

        Ok(())
    }

    /// Record routing history
    pub fn record_routing(
        &self,
        candidate_id: &str,
        query_embedding: &[f32],
        confidence: f32,
        use_lightweight: bool,
        uncertainty: f32,
        inference_time_us: u64,
    ) -> Result<()> {
        let conn = self.conn.lock();

        let query_bytes = bytemuck::cast_slice::<f32, u8>(query_embedding);

        conn.execute(
            "INSERT INTO routing_history
             (candidate_id, query_embedding, confidence, use_lightweight, uncertainty, timestamp, inference_time_us)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                candidate_id,
                query_bytes,
                confidence,
                use_lightweight as i32,
                uncertainty,
                chrono::Utc::now().timestamp(),
                inference_time_us as i64
            ],
        )?;

        Ok(())
    }

    /// Get routing statistics
    pub fn get_statistics(&self) -> Result<RoutingStatistics> {
        let conn = self.conn.lock();

        let total_routes: i64 =
            conn.query_row("SELECT COUNT(*) FROM routing_history", [], |row| row.get(0))?;

        let lightweight_routes: i64 = conn.query_row(
            "SELECT COUNT(*) FROM routing_history WHERE use_lightweight = 1",
            [],
            |row| row.get(0),
        )?;

        let avg_inference_time: f64 = conn
            .query_row(
                "SELECT AVG(inference_time_us) FROM routing_history",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        Ok(RoutingStatistics {
            total_routes: total_routes as u64,
            lightweight_routes: lightweight_routes as u64,
            powerful_routes: (total_routes - lightweight_routes) as u64,
            avg_inference_time_us: avg_inference_time,
        })
    }
}

/// Routing statistics from storage
#[derive(Debug, Clone)]
pub struct RoutingStatistics {
    /// Total routes recorded
    pub total_routes: u64,
    /// Routes to lightweight model
    pub lightweight_routes: u64,
    /// Routes to powerful model
    pub powerful_routes: u64,
    /// Average inference time in microseconds
    pub avg_inference_time_us: f64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_storage_creation() {
        let storage = Storage::in_memory().unwrap();
        let stats = storage.get_statistics().unwrap();
        assert_eq!(stats.total_routes, 0);
    }

    #[test]
    fn test_candidate_insertion() {
        let storage = Storage::in_memory().unwrap();

        let candidate = Candidate {
            id: "test-1".to_string(),
            embedding: vec![0.5; 384],
            metadata: HashMap::new(),
            created_at: chrono::Utc::now().timestamp(),
            access_count: 0,
            success_rate: 0.0,
        };

        storage.insert_candidate(&candidate).unwrap();
        let retrieved = storage.get_candidate("test-1").unwrap();
        assert!(retrieved.is_some());
    }
}
