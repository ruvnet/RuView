/**
 * Distance metric for vector similarity search.
 *
 * - `l2`         Squared Euclidean distance.
 * - `cosine`     Cosine distance (1 - cosine_similarity).
 * - `dotproduct`  Negated inner (dot) product.
 */
export type DistanceMetric = 'l2' | 'cosine' | 'dotproduct';

/**
 * Compression profile for stored vectors.
 *
 * - `none`    Raw fp32 vectors.
 * - `scalar`  Scalar quantization (int8).
 * - `product` Product quantization.
 */
export type CompressionProfile = 'none' | 'scalar' | 'product';

/**
 * Hardware profile selector.
 *
 * 0 = Generic, 1 = Core, 2 = Hot, 3 = Full.
 */
export type HardwareProfile = 0 | 1 | 2 | 3;

/** Options for creating a new RVF store. */
export interface RvfOptions {
  /** Vector dimensionality (required, must be > 0). */
  dimensions: number;
  /** Distance metric for similarity search. Default: `'l2'`. */
  metric?: DistanceMetric;
  /** Hardware profile identifier. Default: `0` (Generic). */
  profile?: HardwareProfile;
  /** Compression profile. Default: `'none'`. */
  compression?: CompressionProfile;
  /** Enable segment signing. Default: `false`. */
  signing?: boolean;
  /** HNSW M parameter: max edges per node per layer. Default: `16`. */
  m?: number;
  /** HNSW ef_construction: beam width during index build. Default: `200`. */
  efConstruction?: number;
}

// ---------------------------------------------------------------------------
// Filter expressions
// ---------------------------------------------------------------------------

/** Primitive value types usable in filter expressions. */
export type RvfFilterValue = number | string | boolean;

/**
 * A filter expression for metadata-based vector filtering.
 *
 * Leaf operators compare a `fieldId` against a literal `value`.
 * Composite operators combine sub-expressions with boolean logic.
 */
export type RvfFilterExpr =
  | { op: 'eq'; fieldId: number; value: RvfFilterValue }
  | { op: 'ne'; fieldId: number; value: RvfFilterValue }
  | { op: 'lt'; fieldId: number; value: RvfFilterValue }
  | { op: 'le'; fieldId: number; value: RvfFilterValue }
  | { op: 'gt'; fieldId: number; value: RvfFilterValue }
  | { op: 'ge'; fieldId: number; value: RvfFilterValue }
  | { op: 'in'; fieldId: number; values: RvfFilterValue[] }
  | { op: 'range'; fieldId: number; low: RvfFilterValue; high: RvfFilterValue }
  | { op: 'and'; exprs: RvfFilterExpr[] }
  | { op: 'or'; exprs: RvfFilterExpr[] }
  | { op: 'not'; expr: RvfFilterExpr };

// ---------------------------------------------------------------------------
// Query options
// ---------------------------------------------------------------------------

/** Options controlling a query operation. */
export interface RvfQueryOptions {
  /** HNSW ef_search parameter (beam width during search). Default: `100`. */
  efSearch?: number;
  /** Optional metadata filter expression. */
  filter?: RvfFilterExpr;
  /** Query timeout in milliseconds (0 = no timeout). Default: `0`. */
  timeoutMs?: number;
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/** A single search result: vector ID and distance. */
export interface RvfSearchResult {
  /** The vector's unique identifier (string-encoded u64). */
  id: string;
  /** Distance from the query vector (lower = more similar). */
  distance: number;
}

/** Result of a batch ingest operation. */
export interface RvfIngestResult {
  /** Number of vectors successfully ingested. */
  accepted: number;
  /** Number of vectors rejected. */
  rejected: number;
  /** Manifest epoch after the ingest commit. */
  epoch: number;
}

/** Result of a delete operation. */
export interface RvfDeleteResult {
  /** Number of vectors soft-deleted. */
  deleted: number;
  /** Manifest epoch after the delete commit. */
  epoch: number;
}

/** Result of a compaction operation. */
export interface RvfCompactionResult {
  /** Number of segments compacted. */
  segmentsCompacted: number;
  /** Bytes of dead space reclaimed. */
  bytesReclaimed: number;
  /** Manifest epoch after compaction commit. */
  epoch: number;
}

/** Compaction state as reported in store status. */
export type CompactionState = 'idle' | 'running' | 'emergency';

/** A snapshot of the store's current state. */
export interface RvfStatus {
  /** Total number of live (non-deleted) vectors. */
  totalVectors: number;
  /** Total number of segments in the file. */
  totalSegments: number;
  /** Total file size in bytes. */
  fileSizeBytes: number;
  /** Current manifest epoch. */
  epoch: number;
  /** Hardware profile identifier. */
  profileId: number;
  /** Current compaction state. */
  compactionState: CompactionState;
  /** Ratio of dead space to total (0.0 - 1.0). */
  deadSpaceRatio: number;
  /** Whether the store is open in read-only mode. */
  readOnly: boolean;
}

// ---------------------------------------------------------------------------
// Ingest entry
// ---------------------------------------------------------------------------

/** A single entry for batch ingestion. */
export interface RvfIngestEntry {
  /** Unique vector identifier. */
  id: string;
  /** The embedding vector (must match store dimensions). */
  vector: Float32Array | number[];
  /** Optional per-vector metadata fields. */
  metadata?: Record<string, RvfFilterValue>;
}

// ---------------------------------------------------------------------------
// Lineage types
// ---------------------------------------------------------------------------

/** Derivation type for creating derived stores. */
export type DerivationType = 'filter' | 'merge' | 'snapshot' | 'transform';

// ---------------------------------------------------------------------------
// Kernel / eBPF types
// ---------------------------------------------------------------------------

/** Data returned from kernel extraction. */
export interface RvfKernelData {
  /** Serialized KernelHeader bytes. */
  header: Uint8Array;
  /** Raw kernel image bytes. */
  image: Uint8Array;
}

/** Data returned from eBPF extraction. */
export interface RvfEbpfData {
  /** Serialized EbpfHeader bytes. */
  header: Uint8Array;
  /** Program bytecode + optional BTF. */
  payload: Uint8Array;
}

// ---------------------------------------------------------------------------
// Segment inspection
// ---------------------------------------------------------------------------

/** Information about a segment in the store. */
export interface RvfSegmentInfo {
  /** Segment ID. */
  id: number;
  /** File offset of the segment. */
  offset: number;
  /** Payload length in bytes. */
  payloadLength: number;
  /** Segment type name (e.g. "vec", "manifest", "kernel"). */
  segType: string;
}

// ---------------------------------------------------------------------------
// Backend identifier
// ---------------------------------------------------------------------------

/** Identifies which backend implementation to use. */
export type BackendType = 'node' | 'wasm' | 'auto';

// ---------------------------------------------------------------------------
// Solver / AGI types (re-exported from @ruvector/rvf-solver)
// ---------------------------------------------------------------------------

/** HNSW index statistics. */
export interface RvfIndexStats {
  /** Number of indexed vectors. */
  indexedVectors: number;
  /** Number of HNSW layers. */
  layers: number;
  /** M parameter (max edges per node per layer). */
  m: number;
  /** ef_construction parameter. */
  efConstruction: number;
  /** Whether the index needs rebuilding. */
  needsRebuild: boolean;
}

/** Result of witness chain verification. */
export interface RvfWitnessResult {
  /** Whether the chain is valid. */
  valid: boolean;
  /** Number of entries in the chain. */
  entries: number;
  /** Error message if invalid. */
  error?: string;
}
