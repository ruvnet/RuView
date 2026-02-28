//! rvf-node -- Node.js N-API bindings for RuVector Format.
//!
//! Exposes `rvf-runtime` operations as native Node.js functions
//! via napi-rs, including insert, query, delete, compact, and status.

extern crate napi_derive;

use std::path::Path;
use std::sync::Mutex;

use napi::bindgen_prelude::*;
use napi_derive::napi;
use serde_json::Value as JsonValue;

use rvf_runtime::filter::{FilterExpr as RustFilterExpr, FilterValue as RustFilterValue};
use rvf_runtime::options::{
    DistanceMetric, MetadataEntry as RustMetadataEntry, MetadataValue as RustMetadataValue,
    QueryOptions as RustQueryOptions, RvfOptions as RustRvfOptions,
};
use rvf_runtime::RvfStore;
use rvf_types::RvfError;

// ── Error mapping ────────────────────────────────────────────────────

fn map_rvf_err(e: RvfError) -> napi::Error {
    let msg = match &e {
        RvfError::Code(code) => format!("RVF error 0x{:04X}: {:?}", *code as u16, code),
        RvfError::UnknownCode(v) => format!("Unknown RVF error 0x{v:04X}"),
        RvfError::BadMagic { expected, got } => {
            format!("Bad magic: expected 0x{expected:08X}, got 0x{got:08X}")
        }
        RvfError::SizeMismatch { expected, got } => {
            format!("Size mismatch: expected {expected}, got {got}")
        }
        RvfError::InvalidEnumValue { type_name, value } => {
            format!("Invalid {type_name} value: {value}")
        }
        RvfError::Security(e) => format!("Security error: {e}"),
        RvfError::QualityBelowThreshold { quality, reason } => {
            format!("Quality below threshold ({quality:?}): {reason}")
        }
    };
    napi::Error::from_reason(msg)
}

// ── TypeScript-facing option / result types ──────────────────────────

/// Options for creating a new RVF store.
#[napi(object)]
pub struct RvfOptions {
    /// Vector dimensionality (required).
    pub dimension: u32,
    /// Distance metric: "l2" | "inner_product" | "cosine". Defaults to "l2".
    pub metric: Option<String>,
    /// Hardware profile: 0=Generic, 1=Core, 2=Hot, 3=Full. Defaults to 0.
    pub profile: Option<u32>,
    /// Whether segment signing is enabled. Defaults to false.
    pub signing: Option<bool>,
    /// HNSW M parameter. Defaults to 16.
    pub m: Option<u32>,
    /// HNSW ef_construction parameter. Defaults to 200.
    pub ef_construction: Option<u32>,
}

/// Options for a query operation.
///
/// The `filter` field accepts a JSON string encoding a recursive filter tree.
/// See `RvfDatabase.query()` for the filter expression format.
#[napi(object)]
pub struct RvfQueryOptions {
    /// HNSW ef_search parameter. Defaults to 100.
    pub ef_search: Option<u32>,
    /// Optional filter expression as a JSON string.
    ///
    /// Format examples:
    ///   `{"op":"eq","fieldId":0,"valueType":"string","value":"cat_a"}`
    ///   `{"op":"and","children":[...]}`
    ///   `{"op":"not","child":{...}}`
    pub filter: Option<String>,
    /// Query timeout in milliseconds. 0 = no timeout.
    pub timeout_ms: Option<u32>,
}

/// A single search result returned from a query.
#[napi(object)]
pub struct RvfSearchResult {
    /// The vector's unique identifier.
    pub id: i64,
    /// Distance from the query vector (lower = more similar).
    pub distance: f64,
}

/// Result of a batch ingest operation.
#[napi(object)]
pub struct RvfIngestResult {
    /// Number of vectors successfully ingested.
    pub accepted: i64,
    /// Number of vectors rejected (dimension mismatch, etc.).
    pub rejected: i64,
    /// Manifest epoch after commit.
    pub epoch: u32,
}

/// Result of a delete operation.
#[napi(object)]
pub struct RvfDeleteResult {
    /// Number of vectors deleted.
    pub deleted: i64,
    /// Manifest epoch after commit.
    pub epoch: u32,
}

/// Current store status snapshot.
#[napi(object)]
pub struct RvfStatus {
    /// Total number of live (non-deleted) vectors.
    pub total_vectors: i64,
    /// Total number of segments in the file.
    pub total_segments: u32,
    /// Total file size in bytes.
    pub file_size: i64,
    /// Current manifest epoch.
    pub current_epoch: u32,
    /// Hardware profile identifier.
    pub profile_id: u32,
    /// Current compaction state: "idle" | "running" | "emergency".
    pub compaction_state: String,
    /// Ratio of dead (deleted) space to total (0.0 - 1.0).
    pub dead_space_ratio: f64,
    /// Whether the store is open in read-only mode.
    pub read_only: bool,
}

/// Result of a compaction operation.
#[napi(object)]
pub struct RvfCompactionResult {
    /// Number of segments compacted.
    pub segments_compacted: u32,
    /// Bytes of dead space reclaimed.
    pub bytes_reclaimed: i64,
    /// Manifest epoch after compaction.
    pub epoch: u32,
}

/// A metadata entry for ingest: { fieldId, valueType, value }.
#[napi(object)]
pub struct RvfMetadataEntry {
    /// Metadata field identifier.
    pub field_id: u32,
    /// Value type: "u64" | "i64" | "f64" | "string".
    pub value_type: String,
    /// The value as a string representation (parsed based on value_type).
    pub value: String,
}

// ── Conversion helpers ───────────────────────────────────────────────

fn parse_metric(s: &str) -> Result<DistanceMetric> {
    match s {
        "l2" | "L2" => Ok(DistanceMetric::L2),
        "inner_product" | "InnerProduct" | "ip" => Ok(DistanceMetric::InnerProduct),
        "cosine" | "Cosine" => Ok(DistanceMetric::Cosine),
        _ => Err(napi::Error::from_reason(format!(
            "Invalid metric '{s}'. Expected 'l2', 'inner_product', or 'cosine'."
        ))),
    }
}

fn js_options_to_rust(opts: &RvfOptions) -> Result<RustRvfOptions> {
    let metric = match &opts.metric {
        Some(m) => parse_metric(m)?,
        None => DistanceMetric::L2,
    };

    Ok(RustRvfOptions {
        dimension: opts.dimension as u16,
        metric,
        profile: opts.profile.unwrap_or(0) as u8,
        signing: opts.signing.unwrap_or(false),
        m: opts.m.unwrap_or(16) as u16,
        ef_construction: opts.ef_construction.unwrap_or(200) as u16,
        ..Default::default()
    })
}

fn parse_filter_value(value_type: &str, raw: &str) -> Result<RustFilterValue> {
    match value_type {
        "u64" => raw
            .parse::<u64>()
            .map(RustFilterValue::U64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{raw}' as u64"))),
        "i64" => raw
            .parse::<i64>()
            .map(RustFilterValue::I64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{raw}' as i64"))),
        "f64" => raw
            .parse::<f64>()
            .map(RustFilterValue::F64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{raw}' as f64"))),
        "string" => Ok(RustFilterValue::String(raw.to_string())),
        "bool" => match raw {
            "true" => Ok(RustFilterValue::Bool(true)),
            "false" => Ok(RustFilterValue::Bool(false)),
            _ => Err(napi::Error::from_reason(format!(
                "Cannot parse '{raw}' as bool"
            ))),
        },
        _ => Err(napi::Error::from_reason(format!(
            "Unknown value_type '{value_type}'"
        ))),
    }
}

fn json_str_field(obj: &JsonValue, key: &str) -> Result<String> {
    obj.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| napi::Error::from_reason(format!("filter requires string field '{key}'")))
}

fn json_u16_field(obj: &JsonValue, key: &str) -> Result<u16> {
    obj.get(key)
        .and_then(|v| v.as_u64())
        .map(|v| v as u16)
        .ok_or_else(|| napi::Error::from_reason(format!("filter requires numeric field '{key}'")))
}

/// Parse a filter expression from a JSON value (recursive).
fn json_to_filter(val: &JsonValue) -> Result<RustFilterExpr> {
    let op = json_str_field(val, "op")?;
    match op.as_str() {
        "eq" | "ne" | "lt" | "le" | "gt" | "ge" => {
            let field_id = json_u16_field(val, "fieldId")?;
            let vt = json_str_field(val, "valueType")?;
            let raw = json_str_field(val, "value")?;
            let fv = parse_filter_value(&vt, &raw)?;
            Ok(match op.as_str() {
                "eq" => RustFilterExpr::Eq(field_id, fv),
                "ne" => RustFilterExpr::Ne(field_id, fv),
                "lt" => RustFilterExpr::Lt(field_id, fv),
                "le" => RustFilterExpr::Le(field_id, fv),
                "gt" => RustFilterExpr::Gt(field_id, fv),
                "ge" => RustFilterExpr::Ge(field_id, fv),
                _ => unreachable!(),
            })
        }
        "in" => {
            let field_id = json_u16_field(val, "fieldId")?;
            let vt = json_str_field(val, "valueType")?;
            let arr = val
                .get("values")
                .and_then(|v| v.as_array())
                .ok_or_else(|| napi::Error::from_reason("filter 'in' requires 'values' array"))?;
            let vals: Vec<RustFilterValue> = arr
                .iter()
                .map(|v| {
                    let s = v
                        .as_str()
                        .ok_or_else(|| napi::Error::from_reason("'values' entries must be strings"))?;
                    parse_filter_value(&vt, s)
                })
                .collect::<Result<_>>()?;
            Ok(RustFilterExpr::In(field_id, vals))
        }
        "range" => {
            let field_id = json_u16_field(val, "fieldId")?;
            let vt = json_str_field(val, "valueType")?;
            let low = json_str_field(val, "low")?;
            let high = json_str_field(val, "high")?;
            let lo = parse_filter_value(&vt, &low)?;
            let hi = parse_filter_value(&vt, &high)?;
            Ok(RustFilterExpr::Range(field_id, lo, hi))
        }
        "and" | "or" => {
            let arr = val
                .get("children")
                .and_then(|v| v.as_array())
                .ok_or_else(|| {
                    napi::Error::from_reason(format!("filter '{op}' requires 'children' array"))
                })?;
            let exprs: Vec<RustFilterExpr> =
                arr.iter().map(json_to_filter).collect::<Result<_>>()?;
            Ok(if op == "and" {
                RustFilterExpr::And(exprs)
            } else {
                RustFilterExpr::Or(exprs)
            })
        }
        "not" => {
            let child = val
                .get("child")
                .ok_or_else(|| napi::Error::from_reason("filter 'not' requires 'child'"))?;
            let expr = json_to_filter(child)?;
            Ok(RustFilterExpr::Not(Box::new(expr)))
        }
        other => Err(napi::Error::from_reason(format!(
            "Unknown filter op '{other}'"
        ))),
    }
}

fn parse_filter_json(json_str: &str) -> Result<RustFilterExpr> {
    let val: JsonValue = serde_json::from_str(json_str)
        .map_err(|e| napi::Error::from_reason(format!("Invalid filter JSON: {e}")))?;
    json_to_filter(&val)
}

fn parse_metadata_entry(e: &RvfMetadataEntry) -> Result<RustMetadataEntry> {
    let value = match e.value_type.as_str() {
        "u64" => e
            .value
            .parse::<u64>()
            .map(RustMetadataValue::U64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{}' as u64", e.value)))?,
        "i64" => e
            .value
            .parse::<i64>()
            .map(RustMetadataValue::I64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{}' as i64", e.value)))?,
        "f64" => e
            .value
            .parse::<f64>()
            .map(RustMetadataValue::F64)
            .map_err(|_| napi::Error::from_reason(format!("Cannot parse '{}' as f64", e.value)))?,
        "string" => RustMetadataValue::String(e.value.clone()),
        other => {
            return Err(napi::Error::from_reason(format!(
                "Unknown metadata value_type '{other}'"
            )));
        }
    };

    Ok(RustMetadataEntry {
        field_id: e.field_id as u16,
        value,
    })
}

/// Data returned from kernel extraction.
#[napi(object)]
pub struct RvfKernelData {
    /// Serialized 128-byte KernelHeader.
    pub header: Buffer,
    /// Raw kernel image bytes.
    pub image: Buffer,
}

/// Data returned from eBPF extraction.
#[napi(object)]
pub struct RvfEbpfData {
    /// Serialized 64-byte EbpfHeader.
    pub header: Buffer,
    /// Program bytecode + optional BTF.
    pub payload: Buffer,
}

/// Information about a segment in the store.
#[napi(object)]
pub struct RvfSegmentInfo {
    /// Segment ID.
    pub id: i64,
    /// File offset of the segment.
    pub offset: i64,
    /// Payload length in bytes.
    pub payload_length: i64,
    /// Segment type as a string (e.g. "vec", "manifest", "kernel").
    pub seg_type: String,
}

// ── AGI-adjacent result types ────────────────────────────────────────

/// HNSW index statistics.
#[napi(object)]
pub struct RvfIndexStats {
    /// Number of indexed vectors.
    pub indexed_vectors: i64,
    /// Number of HNSW layers.
    pub layers: u32,
    /// M parameter (max edges per node per layer).
    pub m: u32,
    /// ef_construction parameter.
    pub ef_construction: u32,
    /// Whether the index needs rebuilding.
    pub needs_rebuild: bool,
}

/// Result of witness chain verification.
#[napi(object)]
pub struct RvfWitnessResult {
    /// Whether the witness chain is valid.
    pub valid: bool,
    /// Number of entries in the chain.
    pub entries: u32,
    /// Error message if invalid.
    pub error: Option<String>,
}

/// Quantization configuration.
#[napi(object)]
pub struct RvfQuantConfig {
    /// Quantization mode: "none" | "scalar" | "product" | "binary".
    pub mode: String,
    /// Number of subquantizers (for product quantization).
    pub num_subquantizers: Option<u32>,
    /// Number of centroids per subquantizer.
    pub num_centroids: Option<u32>,
}

// ── Main RvfDatabase class ───────────────────────────────────────────

/// The main RVF database handle exposed to Node.js.
///
/// All mutating methods acquire an internal mutex so the handle is safe
/// to share across async operations (though RVF itself is single-writer).
#[napi]
pub struct RvfDatabase {
    inner: Mutex<Option<RvfStore>>,
}

#[napi]
impl RvfDatabase {
    /// Create a new RVF store at the given file path.
    #[napi(factory)]
    pub fn create(path: String, options: RvfOptions) -> Result<Self> {
        let rust_opts = js_options_to_rust(&options)?;
        let store = RvfStore::create(Path::new(&path), rust_opts).map_err(map_rvf_err)?;
        Ok(Self {
            inner: Mutex::new(Some(store)),
        })
    }

    /// Open an existing RVF store for read-write access.
    #[napi(factory)]
    pub fn open(path: String) -> Result<Self> {
        let store = RvfStore::open(Path::new(&path)).map_err(map_rvf_err)?;
        Ok(Self {
            inner: Mutex::new(Some(store)),
        })
    }

    /// Open an existing RVF store for read-only access (no lock required).
    #[napi(factory)]
    pub fn open_readonly(path: String) -> Result<Self> {
        let store = RvfStore::open_readonly(Path::new(&path)).map_err(map_rvf_err)?;
        Ok(Self {
            inner: Mutex::new(Some(store)),
        })
    }

    /// Ingest a batch of vectors.
    ///
    /// `vectors` is a flat Float32Array of length `n * dimension`.
    /// `ids` is a number[] of vector IDs.
    /// `metadata` is an optional array of metadata entries.
    #[napi]
    pub fn ingest_batch(
        &self,
        vectors: Float32Array,
        ids: Vec<i64>,
        metadata: Option<Vec<RvfMetadataEntry>>,
    ) -> Result<RvfIngestResult> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let n = ids.len();
        if n == 0 {
            return Ok(RvfIngestResult {
                accepted: 0,
                rejected: 0,
                epoch: 0,
            });
        }

        let vec_data: &[f32] = &vectors;
        let total_floats = vec_data.len();
        if !total_floats.is_multiple_of(n) {
            return Err(napi::Error::from_reason(format!(
                "vectors length ({total_floats}) must be divisible by ids length ({n})"
            )));
        }
        let dim = total_floats / n;

        let vec_slices: Vec<&[f32]> = (0..n).map(|i| &vec_data[i * dim..(i + 1) * dim]).collect();
        let rust_ids: Vec<u64> = ids.iter().map(|&id| id as u64).collect();

        let rust_metadata: Option<Vec<RustMetadataEntry>> = match metadata {
            Some(entries) => {
                let parsed: Vec<RustMetadataEntry> = entries
                    .iter()
                    .map(parse_metadata_entry)
                    .collect::<Result<_>>()?;
                Some(parsed)
            }
            None => None,
        };

        let result = store
            .ingest_batch(&vec_slices, &rust_ids, rust_metadata.as_deref())
            .map_err(map_rvf_err)?;

        Ok(RvfIngestResult {
            accepted: result.accepted as i64,
            rejected: result.rejected as i64,
            epoch: result.epoch,
        })
    }

    /// Query for the k nearest neighbors of the given vector.
    ///
    /// `vector` is a Float32Array of length `dimension`.
    /// `k` is the number of neighbors to return.
    /// `options` provides optional filter (as JSON string) and search parameters.
    ///
    /// Filter expression JSON format:
    /// - Leaf: `{"op":"eq","fieldId":0,"valueType":"string","value":"cat_a"}`
    /// - Boolean: `{"op":"and","children":[...]}`
    /// - Not: `{"op":"not","child":{...}}`
    /// - In: `{"op":"in","fieldId":0,"valueType":"u64","values":["1","2"]}`
    /// - Range: `{"op":"range","fieldId":1,"valueType":"u64","low":"10","high":"50"}`
    #[napi]
    pub fn query(
        &self,
        vector: Float32Array,
        k: u32,
        options: Option<RvfQueryOptions>,
    ) -> Result<Vec<RvfSearchResult>> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let rust_opts = match options {
            Some(ref opts) => {
                let filter = match &opts.filter {
                    Some(json_str) => Some(parse_filter_json(json_str)?),
                    None => None,
                };
                RustQueryOptions {
                    ef_search: opts.ef_search.unwrap_or(100) as u16,
                    filter,
                    timeout_ms: opts.timeout_ms.unwrap_or(0),
                    ..RustQueryOptions::default()
                }
            }
            None => RustQueryOptions::default(),
        };

        let results = store
            .query(&vector, k as usize, &rust_opts)
            .map_err(map_rvf_err)?;

        Ok(results
            .into_iter()
            .map(|r| RvfSearchResult {
                id: r.id as i64,
                distance: r.distance as f64,
            })
            .collect())
    }

    /// Soft-delete vectors by ID.
    #[napi]
    pub fn delete(&self, ids: Vec<i64>) -> Result<RvfDeleteResult> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let rust_ids: Vec<u64> = ids.iter().map(|&id| id as u64).collect();
        let result = store.delete(&rust_ids).map_err(map_rvf_err)?;

        Ok(RvfDeleteResult {
            deleted: result.deleted as i64,
            epoch: result.epoch,
        })
    }

    /// Soft-delete vectors matching a filter expression (passed as JSON string).
    ///
    /// See `query()` for the filter expression JSON format.
    #[napi]
    pub fn delete_by_filter(&self, filter_json: String) -> Result<RvfDeleteResult> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let rust_filter = parse_filter_json(&filter_json)?;
        let result = store.delete_by_filter(&rust_filter).map_err(map_rvf_err)?;

        Ok(RvfDeleteResult {
            deleted: result.deleted as i64,
            epoch: result.epoch,
        })
    }

    /// Run compaction to reclaim dead space from deleted vectors.
    #[napi]
    pub fn compact(&self) -> Result<RvfCompactionResult> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let result = store.compact().map_err(map_rvf_err)?;

        Ok(RvfCompactionResult {
            segments_compacted: result.segments_compacted,
            bytes_reclaimed: result.bytes_reclaimed as i64,
            epoch: result.epoch,
        })
    }

    /// Get a snapshot of the current store status.
    #[napi]
    pub fn status(&self) -> Result<RvfStatus> {
        let guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let s = store.status();
        let compaction_state = match s.compaction_state {
            rvf_runtime::status::CompactionState::Idle => "idle",
            rvf_runtime::status::CompactionState::Running => "running",
            rvf_runtime::status::CompactionState::Emergency => "emergency",
        };

        Ok(RvfStatus {
            total_vectors: s.total_vectors as i64,
            total_segments: s.total_segments,
            file_size: s.file_size as i64,
            current_epoch: s.current_epoch,
            profile_id: s.profile_id as u32,
            compaction_state: compaction_state.to_string(),
            dead_space_ratio: s.dead_space_ratio,
            read_only: s.read_only,
        })
    }

    /// Close the store, releasing the writer lock and flushing data.
    ///
    /// After calling close(), all other methods will return an error.
    #[napi]
    pub fn close(&self) -> Result<()> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard
            .take()
            .ok_or_else(|| napi::Error::from_reason("Store is already closed"))?;

        store.close().map_err(map_rvf_err)
    }

    // ── Lineage methods ──────────────────────────────────────────────

    /// Get this file's unique identifier as a hex string.
    #[napi]
    pub fn file_id(&self) -> Result<String> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;
        Ok(hex_encode(store.file_id()))
    }

    /// Get the parent file's identifier as a hex string (all zeros if root).
    #[napi]
    pub fn parent_id(&self) -> Result<String> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;
        Ok(hex_encode(store.parent_id()))
    }

    /// Get the lineage depth (0 for root files).
    #[napi]
    pub fn lineage_depth(&self) -> Result<u32> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;
        Ok(store.lineage_depth())
    }

    /// Derive a child store from this parent.
    #[napi]
    pub fn derive(&self, child_path: String, options: Option<RvfOptions>) -> Result<RvfDatabase> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let child_opts = match options {
            Some(ref o) => Some(js_options_to_rust(o)?),
            None => None,
        };

        let child_store = store.derive(
            Path::new(&child_path),
            rvf_types::DerivationType::Filter,
            child_opts,
        ).map_err(map_rvf_err)?;

        Ok(RvfDatabase {
            inner: Mutex::new(Some(child_store)),
        })
    }

    // ── Kernel / eBPF methods ────────────────────────────────────────

    /// Embed a kernel image into this RVF file.
    /// Returns the segment ID of the new kernel segment.
    #[napi]
    pub fn embed_kernel(
        &self,
        arch: u32,
        kernel_type: u32,
        flags: u32,
        image: Buffer,
        api_port: u32,
        cmdline: Option<String>,
    ) -> Result<i64> {
        let mut guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let seg_id = store.embed_kernel(
            arch as u8,
            kernel_type as u8,
            flags,
            &image,
            api_port as u16,
            cmdline.as_deref(),
        ).map_err(map_rvf_err)?;

        Ok(seg_id as i64)
    }

    /// Extract the kernel image from this RVF file.
    /// Returns null if no kernel segment is present.
    #[napi]
    pub fn extract_kernel(&self) -> Result<Option<RvfKernelData>> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        match store.extract_kernel().map_err(map_rvf_err)? {
            Some((header, image)) => Ok(Some(RvfKernelData {
                header: Buffer::from(header),
                image: Buffer::from(image),
            })),
            None => Ok(None),
        }
    }

    /// Embed an eBPF program into this RVF file.
    /// Returns the segment ID of the new eBPF segment.
    #[napi]
    pub fn embed_ebpf(
        &self,
        program_type: u32,
        attach_type: u32,
        max_dimension: u32,
        bytecode: Buffer,
        btf: Option<Buffer>,
    ) -> Result<i64> {
        let mut guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let btf_ref = btf.as_ref().map(|b| b.as_ref());
        let seg_id = store.embed_ebpf(
            program_type as u8,
            attach_type as u8,
            max_dimension as u16,
            &bytecode,
            btf_ref,
        ).map_err(map_rvf_err)?;

        Ok(seg_id as i64)
    }

    /// Extract the eBPF program from this RVF file.
    /// Returns null if no eBPF segment is present.
    #[napi]
    pub fn extract_ebpf(&self) -> Result<Option<RvfEbpfData>> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        match store.extract_ebpf().map_err(map_rvf_err)? {
            Some((header, payload)) => Ok(Some(RvfEbpfData {
                header: Buffer::from(header),
                payload: Buffer::from(payload),
            })),
            None => Ok(None),
        }
    }

    // ── Inspection methods ───────────────────────────────────────────

    /// Get the list of segments in the store.
    #[napi]
    pub fn segments(&self) -> Result<Vec<RvfSegmentInfo>> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let seg_dir = store.segment_dir();
        Ok(seg_dir.iter().map(|&(id, offset, payload_len, seg_type)| {
            RvfSegmentInfo {
                id: id as i64,
                offset: offset as i64,
                payload_length: payload_len as i64,
                seg_type: segment_type_name(seg_type),
            }
        }).collect())
    }

    /// Get the vector dimensionality of this store.
    #[napi]
    pub fn dimension(&self) -> Result<u32> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;
        Ok(store.dimension() as u32)
    }

    // ── AGI-adjacent methods ────────────────────────────────────────

    /// Get HNSW index statistics for this store.
    #[napi]
    pub fn index_stats(&self) -> Result<RvfIndexStats> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let status = store.status();
        let opts = store.options();
        Ok(RvfIndexStats {
            indexed_vectors: status.total_vectors as i64,
            layers: 0, // Populated when HNSW index is active
            m: opts.m as u32,
            ef_construction: opts.ef_construction as u32,
            needs_rebuild: status.dead_space_ratio > 0.3,
        })
    }

    /// Verify the tamper-evident witness chain in this store.
    ///
    /// Counts the witness segments present in the segment directory and
    /// checks whether the chain has been initialised (non-zero terminal
    /// hash).  Returns the number of witness entries and validity status.
    #[napi]
    pub fn verify_witness(&self) -> Result<RvfWitnessResult> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        // Witness segment type discriminator (0x0A).
        const WITNESS_SEG_TYPE: u8 = 0x0A;

        let witness_count = store.segment_dir().iter()
            .filter(|&&(_, _, _, seg_type)| seg_type == WITNESS_SEG_TYPE)
            .count() as u32;

        let last_hash = store.last_witness_hash();
        let has_chain = last_hash != &[0u8; 32];

        if witness_count > 0 && !has_chain {
            Ok(RvfWitnessResult {
                valid: false,
                entries: witness_count,
                error: Some("Witness segments exist but chain hash is zero (corrupt or reset)".to_string()),
            })
        } else {
            Ok(RvfWitnessResult {
                valid: true,
                entries: witness_count,
                error: None,
            })
        }
    }

    /// Snapshot-freeze the current state of the store.
    ///
    /// Sets the store to read-only mode, preventing further writes.
    /// Returns the manifest epoch at freeze time.
    #[napi]
    pub fn freeze(&self) -> Result<u32> {
        let mut guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_mut()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let epoch = store.epoch();
        store.freeze().map_err(map_rvf_err)?;
        Ok(epoch)
    }

    /// Get the distance metric used by this store.
    #[napi]
    pub fn metric(&self) -> Result<String> {
        let guard = self.inner.lock()
            .map_err(|_| napi::Error::from_reason("Lock poisoned"))?;
        let store = guard.as_ref()
            .ok_or_else(|| napi::Error::from_reason("Store is closed"))?;

        let metric_str = match store.metric() {
            DistanceMetric::L2 => "l2",
            DistanceMetric::InnerProduct => "inner_product",
            DistanceMetric::Cosine => "cosine",
        };
        Ok(metric_str.to_string())
    }
}

// ── Helper functions ─────────────────────────────────────────────────

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX_CHARS[(b >> 4) as usize]);
        s.push(HEX_CHARS[(b & 0x0f) as usize]);
    }
    s
}

const HEX_CHARS: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7',
    '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn segment_type_name(seg_type: u8) -> String {
    match seg_type {
        0x00 => "invalid".to_string(),
        0x01 => "vec".to_string(),
        0x02 => "index".to_string(),
        0x03 => "overlay".to_string(),
        0x04 => "journal".to_string(),
        0x05 => "manifest".to_string(),
        0x06 => "quant".to_string(),
        0x07 => "meta".to_string(),
        0x08 => "hot".to_string(),
        0x09 => "sketch".to_string(),
        0x0A => "witness".to_string(),
        0x0B => "profile".to_string(),
        0x0C => "crypto".to_string(),
        0x0D => "meta_idx".to_string(),
        0x0E => "kernel".to_string(),
        0x0F => "ebpf".to_string(),
        other => format!("unknown(0x{:02X})", other),
    }
}
