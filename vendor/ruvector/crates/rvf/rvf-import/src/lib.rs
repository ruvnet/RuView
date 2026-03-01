//! rvf-import: Migration tools for importing data into RVF stores.
//!
//! Supports JSON, CSV/TSV, and NumPy `.npy` formats. Each importer
//! parses the source format and batch-ingests vectors into an
//! [`rvf_runtime::RvfStore`].

pub mod csv_import;
pub mod json;
pub mod numpy;
pub mod progress;

use rvf_runtime::{MetadataEntry, RvfOptions, RvfStore};
use rvf_types::RvfError;
use std::path::Path;

/// A single vector record ready for ingestion.
#[derive(Clone, Debug)]
pub struct VectorRecord {
    /// Unique identifier for this vector.
    pub id: u64,
    /// The embedding / feature vector.
    pub vector: Vec<f32>,
    /// Optional key-value metadata entries.
    pub metadata: Vec<MetadataEntry>,
}

/// Result summary returned after an import completes.
#[derive(Clone, Debug)]
pub struct ImportResult {
    /// Total records successfully ingested.
    pub total_imported: u64,
    /// Total records that failed validation (wrong dimension, etc.).
    pub total_rejected: u64,
    /// Number of batches written.
    pub batches: u32,
}

/// Batch-ingest a slice of [`VectorRecord`]s into an [`RvfStore`].
///
/// Records whose vector length does not match `dimension` are silently
/// rejected by the store. Returns an [`ImportResult`] summarising the
/// operation.
pub fn ingest_records(
    store: &mut RvfStore,
    records: &[VectorRecord],
    batch_size: usize,
    progress: Option<&dyn progress::ProgressReporter>,
) -> Result<ImportResult, RvfError> {
    let batch_size = batch_size.max(1);
    let mut total_imported = 0u64;
    let mut total_rejected = 0u64;
    let mut batches = 0u32;

    for chunk in records.chunks(batch_size) {
        let vec_data: Vec<Vec<f32>> = chunk.iter().map(|r| r.vector.clone()).collect();
        let vec_refs: Vec<&[f32]> = vec_data.iter().map(|v| v.as_slice()).collect();
        let ids: Vec<u64> = chunk.iter().map(|r| r.id).collect();

        let has_metadata = chunk.iter().any(|r| !r.metadata.is_empty());
        let metadata: Option<Vec<MetadataEntry>> = if has_metadata {
            Some(chunk.iter().flat_map(|r| r.metadata.clone()).collect())
        } else {
            None
        };

        let result = store.ingest_batch(&vec_refs, &ids, metadata.as_deref())?;

        total_imported += result.accepted;
        total_rejected += result.rejected;
        batches += 1;

        if let Some(p) = progress {
            p.report(total_imported, total_rejected, records.len() as u64);
        }
    }

    Ok(ImportResult {
        total_imported,
        total_rejected,
        batches,
    })
}

/// Create a new RVF store at `path` with the given dimension, then
/// ingest all `records` into it.
pub fn import_to_new_store(
    path: &Path,
    dimension: u16,
    records: &[VectorRecord],
    batch_size: usize,
    progress: Option<&dyn progress::ProgressReporter>,
) -> Result<ImportResult, RvfError> {
    let options = RvfOptions {
        dimension,
        ..Default::default()
    };
    let mut store = RvfStore::create(path, options)?;
    let result = ingest_records(&mut store, records, batch_size, progress)?;
    store.close()?;
    Ok(result)
}
