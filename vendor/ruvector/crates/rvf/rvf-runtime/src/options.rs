//! Configuration types for the RVF runtime.

use crate::filter::FilterExpr;
use rvf_types::quality::{
    BudgetReport, DegradationReport, QualityPreference, ResponseQuality, SafetyNetBudget,
    SearchEvidenceSummary,
};
use rvf_types::security::SecurityPolicy;

/// Distance metric used for vector similarity search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Squared Euclidean distance (L2).
    #[default]
    L2,
    /// Inner (dot) product distance (negated).
    InnerProduct,
    /// Cosine distance (1 - cosine_similarity).
    Cosine,
}

/// Compression profile for stored vectors.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CompressionProfile {
    /// No compression — raw fp32 vectors.
    #[default]
    None,
    /// Scalar quantization (int8).
    Scalar,
    /// Product quantization.
    Product,
}

/// Configuration for automatic witness segment generation.
#[derive(Clone, Debug)]
pub struct WitnessConfig {
    /// Append a witness entry after each ingest operation. Default: true.
    pub witness_ingest: bool,
    /// Append a witness entry after each delete operation. Default: true.
    pub witness_delete: bool,
    /// Append a witness entry after each compact operation. Default: true.
    pub witness_compact: bool,
    /// Append a witness entry after each query operation. Default: false.
    /// Enable this for audit-trail compliance; it adds I/O to the hot path.
    pub audit_queries: bool,
}

impl Default for WitnessConfig {
    fn default() -> Self {
        Self {
            witness_ingest: true,
            witness_delete: true,
            witness_compact: true,
            audit_queries: false,
        }
    }
}

/// Options for creating a new RVF store.
#[derive(Clone, Debug)]
pub struct RvfOptions {
    /// Vector dimensionality (required).
    pub dimension: u16,
    /// Distance metric for similarity search.
    pub metric: DistanceMetric,
    /// Hardware profile identifier (0=Generic, 1=Core, 2=Hot, 3=Full).
    pub profile: u8,
    /// Domain profile for the file (determines canonical extension).
    pub domain_profile: rvf_types::DomainProfile,
    /// Compression profile for stored vectors.
    pub compression: CompressionProfile,
    /// Whether segment signing is enabled.
    pub signing: bool,
    /// HNSW M parameter: max edges per node per layer.
    pub m: u16,
    /// HNSW ef_construction: beam width during index build.
    pub ef_construction: u16,
    /// Witness auto-generation configuration.
    pub witness: WitnessConfig,
    /// Security policy for manifest signature verification (ADR-033 §4).
    pub security_policy: SecurityPolicy,
}

impl Default for RvfOptions {
    fn default() -> Self {
        Self {
            dimension: 0,
            metric: DistanceMetric::L2,
            profile: 0,
            domain_profile: rvf_types::DomainProfile::Generic,
            compression: CompressionProfile::None,
            signing: false,
            m: 16,
            ef_construction: 200,
            witness: WitnessConfig::default(),
            security_policy: SecurityPolicy::Strict,
        }
    }
}

/// Options controlling a query operation.
#[derive(Clone, Debug)]
pub struct QueryOptions {
    /// HNSW ef_search parameter (beam width during search).
    pub ef_search: u16,
    /// Optional metadata filter expression.
    pub filter: Option<FilterExpr>,
    /// Query timeout in milliseconds (0 = no timeout).
    pub timeout_ms: u32,
    /// Quality vs latency preference (ADR-033).
    pub quality_preference: QualityPreference,
    /// Safety net budget caps. Callers may tighten but not loosen
    /// beyond the mode default (unless PreferQuality, which extends to 4x).
    pub safety_net_budget: SafetyNetBudget,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            ef_search: 100,
            filter: None,
            timeout_ms: 0,
            quality_preference: QualityPreference::Auto,
            safety_net_budget: SafetyNetBudget::LAYER_A,
        }
    }
}

/// A single search result: vector ID and distance.
#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    /// The vector's unique identifier.
    pub id: u64,
    /// Distance from the query vector (lower = more similar).
    pub distance: f32,
    /// Per-candidate retrieval quality (ADR-033).
    pub retrieval_quality: rvf_types::quality::RetrievalQuality,
}

/// The mandatory outer return type for all query APIs (ADR-033 §2.4).
///
/// This is not optional. This is not a nested field.
/// JSON flattening cannot discard it. gRPC serialization cannot drop it.
/// MCP tool responses must include it.
#[derive(Clone, Debug)]
pub struct QualityEnvelope {
    /// The search results.
    pub results: Vec<SearchResult>,
    /// Top-level quality signal. Consumers MUST inspect this.
    pub quality: ResponseQuality,
    /// Structured evidence for why the quality is what it is.
    pub evidence: SearchEvidenceSummary,
    /// Resource consumption report for this query.
    pub budgets: BudgetReport,
    /// If quality is degraded, the structured reason.
    pub degradation: Option<DegradationReport>,
}

/// Result of a batch ingest operation.
#[derive(Clone, Debug)]
pub struct IngestResult {
    /// Number of vectors successfully ingested.
    pub accepted: u64,
    /// Number of vectors rejected.
    pub rejected: u64,
    /// Manifest epoch after the ingest commit.
    pub epoch: u32,
}

/// Result of a delete operation.
#[derive(Clone, Debug)]
pub struct DeleteResult {
    /// Number of vectors soft-deleted.
    pub deleted: u64,
    /// Manifest epoch after the delete commit.
    pub epoch: u32,
}

/// Result of a compaction operation.
#[derive(Clone, Debug)]
pub struct CompactionResult {
    /// Number of segments compacted.
    pub segments_compacted: u32,
    /// Bytes of dead space reclaimed.
    pub bytes_reclaimed: u64,
    /// Manifest epoch after compaction commit.
    pub epoch: u32,
}

/// A single metadata entry for a vector.
#[derive(Clone, Debug)]
pub struct MetadataEntry {
    /// Metadata field identifier.
    pub field_id: u16,
    /// The metadata value.
    pub value: MetadataValue,
}

/// Metadata value types matching the spec.
#[derive(Clone, Debug)]
pub enum MetadataValue {
    U64(u64),
    I64(i64),
    F64(f64),
    String(String),
    Bytes(Vec<u8>),
}
