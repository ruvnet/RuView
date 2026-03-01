//! Pipeline integration helpers for OSpipe.
//!
//! Provides [`RvfPipelineAdapter`] which wraps [`RvfObservationStore`] and
//! exposes a simplified interface for OSpipe's ingestion pipeline to push
//! captured frames directly into the RVF store.

use std::path::PathBuf;

use rvf_runtime::options::DistanceMetric;

use crate::observation_store::{
    ObservationMeta, ObservationStoreConfig, OspipeAdapterError, RvfObservationStore,
};

/// Configuration for the pipeline adapter.
#[derive(Clone, Debug)]
pub struct PipelineConfig {
    /// Directory for RVF data files.
    pub data_dir: PathBuf,
    /// Vector embedding dimension.
    pub dimension: u16,
    /// Distance metric for similarity search.
    pub metric: DistanceMetric,
    /// Automatically compact when dead-space ratio exceeds this threshold.
    pub auto_compact_threshold: f64,
}

impl PipelineConfig {
    /// Create a new pipeline config with required parameters.
    pub fn new(data_dir: impl Into<PathBuf>, dimension: u16) -> Self {
        Self {
            data_dir: data_dir.into(),
            dimension,
            metric: DistanceMetric::Cosine,
            auto_compact_threshold: 0.3,
        }
    }
}

/// High-level adapter that OSpipe's ingestion pipeline can use to persist
/// observation vectors into an RVF store.
///
/// Handles store lifecycle, auto-compaction, and provides convenience
/// methods that accept OSpipe-domain types directly.
pub struct RvfPipelineAdapter {
    store: RvfObservationStore,
    config: PipelineConfig,
    ingest_count: u64,
}

impl RvfPipelineAdapter {
    /// Create a new pipeline adapter, creating the underlying RVF file.
    pub fn create(config: PipelineConfig) -> Result<Self, OspipeAdapterError> {
        let store_config = ObservationStoreConfig {
            data_dir: config.data_dir.clone(),
            dimension: config.dimension,
            metric: config.metric,
        };

        let store = RvfObservationStore::create(store_config)?;

        Ok(Self {
            store,
            config,
            ingest_count: 0,
        })
    }

    /// Open an existing pipeline adapter.
    pub fn open(config: PipelineConfig) -> Result<Self, OspipeAdapterError> {
        let store_config = ObservationStoreConfig {
            data_dir: config.data_dir.clone(),
            dimension: config.dimension,
            metric: config.metric,
        };

        let store = RvfObservationStore::open(store_config)?;

        Ok(Self {
            store,
            config,
            ingest_count: 0,
        })
    }

    /// Ingest a single observation from the pipeline.
    ///
    /// This is the primary entry point for OSpipe's ingestion pipeline.
    /// After ingestion, may trigger auto-compaction if the dead-space
    /// ratio exceeds the configured threshold.
    pub fn ingest(
        &mut self,
        embedding: &[f32],
        content_type: &str,
        app_name: Option<&str>,
        timestamp_secs: u64,
        monitor_id: Option<u32>,
    ) -> Result<u64, OspipeAdapterError> {
        let meta = ObservationMeta {
            content_type: content_type.to_string(),
            app_name: app_name.map(|s| s.to_string()),
            timestamp_secs,
            monitor_id,
        };

        let (id, _result) = self.store.record_observation(embedding, &meta)?;
        self.ingest_count += 1;

        self.maybe_compact()?;

        Ok(id)
    }

    /// Search for similar observations.
    pub fn search(
        &self,
        query: &[f32],
        k: usize,
    ) -> Result<Vec<rvf_runtime::SearchResult>, OspipeAdapterError> {
        self.store.query_similar_states(query, k)
    }

    /// Search for observations within a time window.
    pub fn search_time_range(
        &self,
        query: &[f32],
        k: usize,
        start_secs: u64,
        end_secs: u64,
    ) -> Result<Vec<rvf_runtime::SearchResult>, OspipeAdapterError> {
        self.store.get_state_history(query, k, start_secs, end_secs)
    }

    /// Expire observations older than the given timestamp.
    ///
    /// Scans for observations with timestamps before `before_secs` and
    /// soft-deletes them. Returns the number of observations deleted.
    pub fn expire_before(
        &mut self,
        before_secs: u64,
    ) -> Result<u64, OspipeAdapterError> {
        use rvf_runtime::filter::{FilterExpr, FilterValue};

        let filter = FilterExpr::Lt(
            crate::observation_store::fields::TIMESTAMP_SECS,
            FilterValue::U64(before_secs),
        );

        let result = self.store.delete_by_filter(&filter)?;

        Ok(result.deleted)
    }

    /// Force a compaction cycle.
    pub fn compact(&mut self) -> Result<rvf_runtime::CompactionResult, OspipeAdapterError> {
        self.store.compact_history()
    }

    /// Get the total number of live observations.
    pub fn observation_count(&self) -> u64 {
        self.store.status().total_vectors
    }

    /// Close the adapter and release resources.
    pub fn close(self) -> Result<(), OspipeAdapterError> {
        self.store.close()
    }

    /// Check if auto-compaction should run, and run it if so.
    fn maybe_compact(&mut self) -> Result<(), OspipeAdapterError> {
        let status = self.store.status();
        if status.dead_space_ratio > self.config.auto_compact_threshold {
            self.store.compact_history()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_vector(dim: usize, seed: u64) -> Vec<f32> {
        let mut v = Vec::with_capacity(dim);
        let mut x = seed;
        for _ in 0..dim {
            x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            v.push(((x >> 33) as f32) / (u32::MAX as f32) - 0.5);
        }
        v
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    #[test]
    fn pipeline_ingest_and_search() {
        let dir = TempDir::new().unwrap();
        let config = PipelineConfig::new(dir.path(), 32);
        let mut adapter = RvfPipelineAdapter::create(config).unwrap();

        let ts = now_secs();

        for i in 0..5u64 {
            let vec = make_vector(32, i);
            adapter
                .ingest(&vec, "ocr", Some("VSCode"), ts + i, Some(0))
                .unwrap();
        }

        assert_eq!(adapter.observation_count(), 5);

        let query = make_vector(32, 2);
        let results = adapter.search(&query, 2).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].id, 3); // seed=2 -> id=3 (1-indexed)

        adapter.close().unwrap();
    }

    #[test]
    fn pipeline_time_range_search() {
        let dir = TempDir::new().unwrap();
        let config = PipelineConfig::new(dir.path(), 16);
        let mut adapter = RvfPipelineAdapter::create(config).unwrap();

        let base = 1_700_000_000u64;
        for i in 0..4u64 {
            let vec = make_vector(16, i);
            adapter
                .ingest(&vec, "transcription", None, base + i * 3600, None)
                .unwrap();
        }

        let query = make_vector(16, 0);
        let results = adapter
            .search_time_range(&query, 10, base + 3600, base + 7200)
            .unwrap();

        // Should get observations at base+3600 (id=2) and base+7200 (id=3).
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn pipeline_open_existing() {
        let dir = TempDir::new().unwrap();
        let config = PipelineConfig::new(dir.path(), 16);

        {
            let mut adapter = RvfPipelineAdapter::create(config.clone()).unwrap();
            let vec = make_vector(16, 0);
            adapter.ingest(&vec, "ocr", None, now_secs(), None).unwrap();
            adapter.close().unwrap();
        }

        {
            let adapter = RvfPipelineAdapter::open(config).unwrap();
            assert_eq!(adapter.observation_count(), 1);
            adapter.close().unwrap();
        }
    }
}
