//! RVF-backed observation store for OSpipe state vectors.
//!
//! Maps OSpipe observation embeddings into RVF segments with metadata
//! stored via field IDs in META_SEG entries.
//!
//! # Field layout
//!
//! | field_id | type   | description            |
//! |----------|--------|------------------------|
//! | 0        | String | content_type           |
//! | 1        | String | app_name               |
//! | 2        | U64    | timestamp_secs (epoch) |
//! | 3        | U64    | monitor_id             |

use std::path::PathBuf;

use rvf_runtime::filter::FilterExpr;
use rvf_runtime::options::{
    DistanceMetric, MetadataEntry, MetadataValue, QueryOptions, RvfOptions,
};
use rvf_runtime::{IngestResult, RvfStore, SearchResult, StoreStatus};
use rvf_types::RvfError;

/// Well-known metadata field IDs for OSpipe observations.
pub mod fields {
    /// Content type (ocr, transcription, ui_event).
    pub const CONTENT_TYPE: u16 = 0;
    /// Application name.
    pub const APP_NAME: u16 = 1;
    /// Observation timestamp as seconds since UNIX epoch.
    pub const TIMESTAMP_SECS: u16 = 2;
    /// Monitor index.
    pub const MONITOR_ID: u16 = 3;
}

/// Metadata for an observation to be recorded.
#[derive(Clone, Debug)]
pub struct ObservationMeta {
    /// Content type label (e.g. "ocr", "transcription", "ui_event").
    pub content_type: String,
    /// Application name, if known.
    pub app_name: Option<String>,
    /// Observation timestamp as seconds since UNIX epoch.
    pub timestamp_secs: u64,
    /// Monitor index, if applicable.
    pub monitor_id: Option<u32>,
}

impl ObservationMeta {
    /// Convert to RVF metadata entries for a single vector.
    fn to_entries(&self) -> Vec<MetadataEntry> {
        let mut entries = Vec::with_capacity(4);

        entries.push(MetadataEntry {
            field_id: fields::CONTENT_TYPE,
            value: MetadataValue::String(self.content_type.clone()),
        });

        if let Some(ref app) = self.app_name {
            entries.push(MetadataEntry {
                field_id: fields::APP_NAME,
                value: MetadataValue::String(app.clone()),
            });
        }

        entries.push(MetadataEntry {
            field_id: fields::TIMESTAMP_SECS,
            value: MetadataValue::U64(self.timestamp_secs),
        });

        if let Some(monitor) = self.monitor_id {
            entries.push(MetadataEntry {
                field_id: fields::MONITOR_ID,
                value: MetadataValue::U64(monitor as u64),
            });
        }

        entries
    }
}

/// Configuration for the observation store.
#[derive(Clone, Debug)]
pub struct ObservationStoreConfig {
    /// Directory for RVF data files.
    pub data_dir: PathBuf,
    /// Vector embedding dimension.
    pub dimension: u16,
    /// Distance metric (defaults to Cosine for OSpipe embeddings).
    pub metric: DistanceMetric,
}

impl ObservationStoreConfig {
    /// Create with required parameters, using Cosine metric by default.
    pub fn new(data_dir: impl Into<PathBuf>, dimension: u16) -> Self {
        Self {
            data_dir: data_dir.into(),
            dimension,
            metric: DistanceMetric::Cosine,
        }
    }

    /// Set the distance metric.
    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    fn store_path(&self) -> PathBuf {
        self.data_dir.join("observations.rvf")
    }
}

/// RVF-backed observation store for OSpipe.
///
/// Wraps an `RvfStore` and provides observation-oriented APIs:
/// - `record_observation` -- ingest a state vector with metadata
/// - `query_similar_states` -- k-NN search over observation vectors
/// - `get_state_history` -- filtered query by time range
/// - `compact_history` -- reclaim dead space from deleted observations
pub struct RvfObservationStore {
    store: RvfStore,
    #[allow(dead_code)]
    config: ObservationStoreConfig,
    next_id: u64,
}

impl RvfObservationStore {
    /// Create a new observation store, creating the RVF file.
    pub fn create(config: ObservationStoreConfig) -> Result<Self, OspipeAdapterError> {
        if config.dimension == 0 {
            return Err(OspipeAdapterError::InvalidDimension);
        }
        std::fs::create_dir_all(&config.data_dir)
            .map_err(|e| OspipeAdapterError::Io(e.to_string()))?;

        let options = RvfOptions {
            dimension: config.dimension,
            metric: config.metric,
            ..Default::default()
        };

        let store = RvfStore::create(&config.store_path(), options)
            .map_err(OspipeAdapterError::Rvf)?;

        Ok(Self {
            store,
            config,
            next_id: 1,
        })
    }

    /// Open an existing observation store.
    pub fn open(config: ObservationStoreConfig) -> Result<Self, OspipeAdapterError> {
        let store = RvfStore::open(&config.store_path())
            .map_err(OspipeAdapterError::Rvf)?;

        let status = store.status();
        let next_id = status.total_vectors + status.current_epoch as u64 + 1;

        Ok(Self {
            store,
            config,
            next_id,
        })
    }

    /// Open an existing store in read-only mode.
    pub fn open_readonly(config: ObservationStoreConfig) -> Result<Self, OspipeAdapterError> {
        let store = RvfStore::open_readonly(&config.store_path())
            .map_err(OspipeAdapterError::Rvf)?;

        Ok(Self {
            store,
            config,
            next_id: 0,
        })
    }

    /// Record a single observation with its state vector and metadata.
    ///
    /// Returns the assigned vector ID and the ingest result.
    pub fn record_observation(
        &mut self,
        state_vector: &[f32],
        meta: &ObservationMeta,
    ) -> Result<(u64, IngestResult), OspipeAdapterError> {
        let id = self.next_id;
        self.next_id += 1;

        let entries = meta.to_entries();
        let result = self.store.ingest_batch(
            &[state_vector],
            &[id],
            Some(&entries),
        ).map_err(OspipeAdapterError::Rvf)?;

        Ok((id, result))
    }

    /// Record a batch of observations.
    ///
    /// `vectors` and `metas` must have the same length.
    /// Returns the assigned IDs and the ingest result.
    pub fn record_batch(
        &mut self,
        vectors: &[&[f32]],
        metas: &[ObservationMeta],
    ) -> Result<(Vec<u64>, IngestResult), OspipeAdapterError> {
        if vectors.len() != metas.len() {
            return Err(OspipeAdapterError::LengthMismatch {
                vectors: vectors.len(),
                metas: metas.len(),
            });
        }

        let start_id = self.next_id;
        let ids: Vec<u64> = (start_id..start_id + vectors.len() as u64).collect();
        self.next_id = start_id + vectors.len() as u64;

        // Flatten metadata entries: each vector gets its own entries.
        // RvfStore expects entries_per_id to be uniform, so we pad to
        // a consistent entry count per vector.
        let entries_per_vec: Vec<Vec<MetadataEntry>> =
            metas.iter().map(|m| m.to_entries()).collect();

        let max_entries = entries_per_vec.iter().map(|e| e.len()).max().unwrap_or(0);

        let mut flat_entries = Vec::with_capacity(vectors.len() * max_entries);
        for vec_entries in &entries_per_vec {
            for entry in vec_entries {
                flat_entries.push(entry.clone());
            }
            // Pad with dummy entries so every vector has the same count.
            for _ in vec_entries.len()..max_entries {
                flat_entries.push(MetadataEntry {
                    field_id: u16::MAX,
                    value: MetadataValue::U64(0),
                });
            }
        }

        let result = self.store.ingest_batch(
            vectors,
            &ids,
            if flat_entries.is_empty() { None } else { Some(&flat_entries) },
        ).map_err(OspipeAdapterError::Rvf)?;

        Ok((ids, result))
    }

    /// Query for the k most similar observation states.
    pub fn query_similar_states(
        &self,
        state_vector: &[f32],
        k: usize,
    ) -> Result<Vec<SearchResult>, OspipeAdapterError> {
        self.store
            .query(state_vector, k, &QueryOptions::default())
            .map_err(OspipeAdapterError::Rvf)
    }

    /// Query with a metadata filter expression.
    pub fn query_filtered(
        &self,
        state_vector: &[f32],
        k: usize,
        filter: FilterExpr,
    ) -> Result<Vec<SearchResult>, OspipeAdapterError> {
        let opts = QueryOptions {
            filter: Some(filter),
            ..Default::default()
        };
        self.store
            .query(state_vector, k, &opts)
            .map_err(OspipeAdapterError::Rvf)
    }

    /// Query for observations within a time range.
    ///
    /// `start_secs` and `end_secs` are UNIX epoch seconds. The query
    /// vector is used for similarity ranking among the time-filtered results.
    pub fn get_state_history(
        &self,
        state_vector: &[f32],
        k: usize,
        start_secs: u64,
        end_secs: u64,
    ) -> Result<Vec<SearchResult>, OspipeAdapterError> {
        use rvf_runtime::filter::FilterValue;

        let filter = FilterExpr::And(vec![
            FilterExpr::Ge(fields::TIMESTAMP_SECS, FilterValue::U64(start_secs)),
            FilterExpr::Le(fields::TIMESTAMP_SECS, FilterValue::U64(end_secs)),
        ]);

        self.query_filtered(state_vector, k, filter)
    }

    /// Run compaction to reclaim space from deleted observations.
    pub fn compact_history(&mut self) -> Result<rvf_runtime::CompactionResult, OspipeAdapterError> {
        self.store.compact().map_err(OspipeAdapterError::Rvf)
    }

    /// Delete observations by their IDs.
    pub fn delete_observations(
        &mut self,
        ids: &[u64],
    ) -> Result<rvf_runtime::DeleteResult, OspipeAdapterError> {
        self.store.delete(ids).map_err(OspipeAdapterError::Rvf)
    }

    /// Delete observations matching a filter expression.
    pub fn delete_by_filter(
        &mut self,
        filter: &FilterExpr,
    ) -> Result<rvf_runtime::DeleteResult, OspipeAdapterError> {
        self.store.delete_by_filter(filter).map_err(OspipeAdapterError::Rvf)
    }

    /// Get the current store status.
    pub fn status(&self) -> StoreStatus {
        self.store.status()
    }

    /// Close the store, releasing locks.
    pub fn close(self) -> Result<(), OspipeAdapterError> {
        self.store.close().map_err(OspipeAdapterError::Rvf)
    }
}

/// Errors produced by the OSpipe adapter.
#[derive(Clone, Debug)]
pub enum OspipeAdapterError {
    /// Underlying RVF error.
    Rvf(RvfError),
    /// IO error (directory creation, etc.).
    Io(String),
    /// Vector dimension must be > 0.
    InvalidDimension,
    /// Batch vectors and metadata have different lengths.
    LengthMismatch { vectors: usize, metas: usize },
}

impl std::fmt::Display for OspipeAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rvf(e) => write!(f, "RVF error: {e}"),
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::InvalidDimension => write!(f, "vector dimension must be > 0"),
            Self::LengthMismatch { vectors, metas } => {
                write!(f, "vectors ({vectors}) and metas ({metas}) length mismatch")
            }
        }
    }
}

impl std::error::Error for OspipeAdapterError {}

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
    fn create_and_record_observation() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 64);
        let mut store = RvfObservationStore::create(config).unwrap();

        let vec = make_vector(64, 42);
        let meta = ObservationMeta {
            content_type: "ocr".into(),
            app_name: Some("VSCode".into()),
            timestamp_secs: now_secs(),
            monitor_id: Some(0),
        };

        let (id, result) = store.record_observation(&vec, &meta).unwrap();
        assert_eq!(id, 1);
        assert_eq!(result.accepted, 1);
        assert_eq!(result.rejected, 0);

        store.close().unwrap();
    }

    #[test]
    fn query_similar_states() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 32);
        let mut store = RvfObservationStore::create(config).unwrap();

        // Insert 10 observations.
        for i in 0..10u64 {
            let vec = make_vector(32, i);
            let meta = ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: now_secs() + i,
                monitor_id: None,
            };
            store.record_observation(&vec, &meta).unwrap();
        }

        let query = make_vector(32, 5);
        let results = store.query_similar_states(&query, 3).unwrap();
        assert_eq!(results.len(), 3);

        // Closest should be the same vector (id 6, since first id is 1).
        assert_eq!(results[0].id, 6);
        assert!(results[0].distance < 1e-5);

        // Results are sorted by distance ascending.
        for i in 1..results.len() {
            assert!(results[i].distance >= results[i - 1].distance);
        }

        store.close().unwrap();
    }

    #[test]
    fn get_state_history_filters_by_time() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 16);
        let mut store = RvfObservationStore::create(config).unwrap();

        let base_time = 1_700_000_000u64;

        // Insert observations at different times.
        for i in 0..5u64 {
            let vec = make_vector(16, i);
            let meta = ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: base_time + i * 100,
                monitor_id: None,
            };
            store.record_observation(&vec, &meta).unwrap();
        }

        // Query for observations in the range [base+100, base+300].
        let query = make_vector(16, 0);
        let results = store
            .get_state_history(&query, 10, base_time + 100, base_time + 300)
            .unwrap();

        // Should get ids 2, 3, 4 (timestamps base+100, base+200, base+300).
        assert_eq!(results.len(), 3);
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert!(ids.contains(&2));
        assert!(ids.contains(&3));
        assert!(ids.contains(&4));

        store.close().unwrap();
    }

    #[test]
    fn record_batch_and_query() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 16);
        let mut store = RvfObservationStore::create(config).unwrap();

        let vecs: Vec<Vec<f32>> = (0..5).map(|i| make_vector(16, i)).collect();
        let vec_refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let metas: Vec<ObservationMeta> = (0..5)
            .map(|i| ObservationMeta {
                content_type: if i % 2 == 0 { "ocr" } else { "transcription" }.into(),
                app_name: Some("TestApp".into()),
                timestamp_secs: now_secs() + i,
                monitor_id: None,
            })
            .collect();

        let (ids, result) = store.record_batch(&vec_refs, &metas).unwrap();
        assert_eq!(ids.len(), 5);
        assert_eq!(result.accepted, 5);

        let query = make_vector(16, 2);
        let results = store.query_similar_states(&query, 1).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, 3); // id starts at 1, so seed=2 -> id=3

        store.close().unwrap();
    }

    #[test]
    fn delete_and_compact() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 8);
        let mut store = RvfObservationStore::create(config).unwrap();

        // Insert 4 observations.
        for i in 0..4u64 {
            let vec = make_vector(8, i);
            let meta = ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: now_secs(),
                monitor_id: None,
            };
            store.record_observation(&vec, &meta).unwrap();
        }

        let status = store.status();
        assert_eq!(status.total_vectors, 4);

        // Delete 2 observations.
        let del = store.delete_observations(&[1, 3]).unwrap();
        assert_eq!(del.deleted, 2);

        let status = store.status();
        assert_eq!(status.total_vectors, 2);

        // Compact.
        let compact = store.compact_history().unwrap();
        assert_eq!(compact.segments_compacted, 2);

        // Verify remaining vectors are queryable.
        let query = make_vector(8, 1); // seed=1 -> was id=2
        let results = store.query_similar_states(&query, 10).unwrap();
        assert_eq!(results.len(), 2);
        let ids: Vec<u64> = results.iter().map(|r| r.id).collect();
        assert!(ids.contains(&2));
        assert!(ids.contains(&4));

        store.close().unwrap();
    }

    #[test]
    fn open_existing_store() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 16);

        // Create and populate.
        {
            let mut store = RvfObservationStore::create(config.clone()).unwrap();
            let vec = make_vector(16, 99);
            let meta = ObservationMeta {
                content_type: "transcription".into(),
                app_name: Some("Zoom".into()),
                timestamp_secs: now_secs(),
                monitor_id: None,
            };
            store.record_observation(&vec, &meta).unwrap();
            store.close().unwrap();
        }

        // Reopen.
        {
            let store = RvfObservationStore::open(config).unwrap();
            let query = make_vector(16, 99);
            let results = store.query_similar_states(&query, 1).unwrap();
            assert_eq!(results.len(), 1);
            assert!(results[0].distance < 1e-5);
            store.close().unwrap();
        }
    }

    #[test]
    fn readonly_mode() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 8);

        {
            let mut store = RvfObservationStore::create(config.clone()).unwrap();
            let vec = make_vector(8, 0);
            let meta = ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: now_secs(),
                monitor_id: None,
            };
            store.record_observation(&vec, &meta).unwrap();
            store.close().unwrap();
        }

        let store = RvfObservationStore::open_readonly(config).unwrap();
        let status = store.status();
        assert!(status.read_only);
        assert_eq!(status.total_vectors, 1);
    }

    #[test]
    fn invalid_dimension_rejected() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 0);
        let result = RvfObservationStore::create(config);
        assert!(result.is_err());
    }

    #[test]
    fn batch_length_mismatch_rejected() {
        let dir = TempDir::new().unwrap();
        let config = ObservationStoreConfig::new(dir.path(), 8);
        let mut store = RvfObservationStore::create(config).unwrap();

        let vecs = [make_vector(8, 0)];
        let vec_refs: Vec<&[f32]> = vecs.iter().map(|v| v.as_slice()).collect();
        let metas = vec![
            ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: 0,
                monitor_id: None,
            },
            ObservationMeta {
                content_type: "ocr".into(),
                app_name: None,
                timestamp_secs: 0,
                monitor_id: None,
            },
        ];

        let result = store.record_batch(&vec_refs, &metas);
        assert!(result.is_err());

        store.close().unwrap();
    }
}
