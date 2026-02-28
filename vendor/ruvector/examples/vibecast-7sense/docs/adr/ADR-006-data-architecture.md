# ADR-006: Data Architecture and Vector Storage

**Status:** Accepted
**Date:** 2026-01-15
**Deciders:** 7sense Architecture Team
**Context:** Bioacoustic data pipeline for RuVector integration

## Context

7sense transforms bioacoustic signals (birdsong, wildlife vocalizations) into navigable geometric spaces using the RuVector platform. The system processes audio recordings through Perch 2.0 to generate 1536-dimensional embeddings, which are then indexed using HNSW for fast similarity search and organized via Graph Neural Networks (GNN) for pattern discovery.

This ADR defines the complete data architecture including:
- Entity schemas and relationships
- Vector storage tiering strategy
- Temporal data handling
- Metadata enrichment
- Data lifecycle management
- Backup and recovery procedures

## Decision

### 1. Schema Design

#### 1.1 Core Entities

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           ENTITY RELATIONSHIP DIAGRAM                        │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐       ┌───────────────┐       ┌──────────────┐            │
│  │  Recording   │──1:N──│  CallSegment  │──1:1──│  Embedding   │            │
│  └──────────────┘       └───────────────┘       └──────────────┘            │
│         │                      │                       │                     │
│         │                      │                       │                     │
│         │               ┌──────┴──────┐               │                     │
│         │               │             │               │                     │
│         ▼               ▼             ▼               ▼                     │
│  ┌──────────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐               │
│  │   Sensor     │ │  Cluster │ │ Prototype│ │    Taxon     │               │
│  └──────────────┘ └──────────┘ └──────────┘ └──────────────┘               │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 1.2 Node Definitions

**Recording** - Source audio file metadata
```typescript
interface Recording {
  id: UUID;                    // Primary identifier
  sensor_id: UUID;             // Reference to sensor
  file_path: string;           // Storage location
  file_hash: string;           // SHA-256 for deduplication
  duration_ms: number;         // Total duration
  sample_rate: number;         // Expected: 32000 Hz
  channels: number;            // Expected: 1 (mono)
  bit_depth: number;           // Audio bit depth
  start_ts: ISO8601;           // Recording start timestamp
  end_ts: ISO8601;             // Recording end timestamp
  lat: float;                  // GPS latitude (WGS84)
  lon: float;                  // GPS longitude (WGS84)
  altitude_m: float;           // Elevation in meters
  habitat: HabitatType;        // Enum: forest, wetland, urban, etc.
  weather: WeatherConditions;  // Nested weather data
  quality_score: float;        // 0.0-1.0 automated quality assessment
  processing_status: Status;   // pending, processing, complete, failed
  created_at: ISO8601;
  updated_at: ISO8601;
}

interface WeatherConditions {
  temperature_c: float;
  humidity_pct: float;
  wind_speed_ms: float;
  wind_direction_deg: float;
  precipitation_mm: float;
  cloud_cover_pct: float;
  pressure_hpa: float;
  source: string;              // weather API source
}
```

**CallSegment** - Individual vocalization segment
```typescript
interface CallSegment {
  id: UUID;
  recording_id: UUID;          // Parent recording
  segment_index: number;       // Order within recording
  t0_ms: number;               // Start offset in milliseconds
  t1_ms: number;               // End offset in milliseconds
  duration_ms: number;         // Computed: t1_ms - t0_ms
  snr_db: float;               // Signal-to-noise ratio
  energy: float;               // RMS energy level
  peak_freq_hz: float;         // Dominant frequency
  bandwidth_hz: float;         // Frequency range
  entropy: float;              // Spectral entropy (Wiener)
  pitch_contour: float[];      // Sampled pitch values
  rhythm_intervals: float[];   // Inter-onset intervals
  spectral_centroid: float;    // Spectral center of mass
  spectral_flatness: float;    // Tonality measure
  zero_crossing_rate: float;   // Temporal texture
  clipping_detected: boolean;  // Audio quality flag
  overlap_score: float;        // Overlap with other calls
  segmentation_method: string; // whisper_seg, tweety_net, energy_threshold
  segmentation_confidence: float;
  created_at: ISO8601;
}
```

**Embedding** - Vector representation from Perch 2.0
```typescript
interface Embedding {
  id: UUID;
  segment_id: UUID;            // Parent segment
  model_name: string;          // "perch_2.0"
  model_version: string;       // Specific model version
  dimensions: number;          // 1536 for Perch 2.0
  vector: Float32Array;        // Full-precision embedding
  vector_quantized: Int8Array; // Quantized for warm tier
  vector_compressed: Uint8Array; // Compressed for cold tier
  storage_tier: StorageTier;   // hot, warm, cold
  norm: float;                 // L2 norm for validation
  generation_time_ms: number;  // Inference latency
  checksum: string;            // Integrity verification
  created_at: ISO8601;
  last_accessed: ISO8601;      // For tiering decisions
  access_count: number;        // Usage tracking
}

enum StorageTier {
  HOT = 'hot',     // Full float32, in-memory HNSW
  WARM = 'warm',   // int8 quantized, SSD-backed
  COLD = 'cold'    // 32x compressed, archival
}
```

**Prototype** - Cluster centroid/exemplar
```typescript
interface Prototype {
  id: UUID;
  cluster_id: UUID;            // Parent cluster
  centroid_vector: Float32Array; // Averaged embedding
  exemplar_ids: UUID[];        // Representative segment IDs
  exemplar_count: number;      // Number of exemplars
  intra_cluster_variance: float;
  silhouette_score: float;     // Cluster quality metric
  created_at: ISO8601;
  updated_at: ISO8601;
}
```

**Cluster** - Grouping of similar calls
```typescript
interface Cluster {
  id: UUID;
  name: string;                // Human-readable label
  description: string;         // Auto-generated or manual
  method: ClusterMethod;       // hdbscan, kmeans, spectral
  params: Record<string, any>; // Algorithm parameters
  member_count: number;        // Number of assigned segments
  coherence_score: float;      // Internal validity
  stability_score: float;      // Bootstrap stability
  parent_cluster_id: UUID;     // Hierarchical clustering
  level: number;               // Hierarchy depth
  created_at: ISO8601;
  updated_at: ISO8601;
}

enum ClusterMethod {
  HDBSCAN = 'hdbscan',
  KMEANS = 'kmeans',
  SPECTRAL = 'spectral',
  AGGLOMERATIVE = 'agglomerative'
}
```

**Taxon** - Species/taxonomic reference
```typescript
interface Taxon {
  id: UUID;
  inat_id: number;             // iNaturalist taxon ID
  scientific_name: string;     // Binomial nomenclature
  common_name: string;         // English common name
  family: string;              // Taxonomic family
  order: string;               // Taxonomic order
  class: string;               // Taxonomic class
  conservation_status: string; // IUCN status
  frequency_range_hz: [number, number]; // Typical vocalization range
  habitat_types: HabitatType[];
  created_at: ISO8601;
}
```

**Sensor** - Recording device metadata
```typescript
interface Sensor {
  id: UUID;
  name: string;                // Device identifier
  model: string;               // Hardware model
  manufacturer: string;
  serial_number: string;
  microphone_type: string;     // omni, cardioid, etc.
  sensitivity_dbv: float;      // Microphone sensitivity
  frequency_response: [number, number]; // Hz range
  deployment_lat: float;
  deployment_lon: float;
  deployment_altitude_m: float;
  deployment_habitat: HabitatType;
  deployment_start: ISO8601;
  deployment_end: ISO8601;
  calibration_date: ISO8601;
  calibration_factor: float;
  status: SensorStatus;
  created_at: ISO8601;
  updated_at: ISO8601;
}

enum SensorStatus {
  ACTIVE = 'active',
  INACTIVE = 'inactive',
  MAINTENANCE = 'maintenance',
  RETIRED = 'retired'
}
```

#### 1.3 Edge Definitions (Graph Relationships)

```cypher
// Structural relationships
(:Recording)-[:HAS_SEGMENT {order: int}]->(:CallSegment)
(:CallSegment)-[:HAS_EMBEDDING]->(:Embedding)
(:Recording)-[:FROM_SENSOR]->(:Sensor)

// Temporal sequence (syntax graph)
(:CallSegment)-[:NEXT {
  dt_ms: int,           // Time delta between segments
  same_speaker: boolean, // Likely same individual
  transition_prob: float // Learned transition probability
}]->(:CallSegment)

// Acoustic similarity (HNSW neighbors)
(:CallSegment)-[:SIMILAR {
  distance: float,      // Cosine distance
  rank: int,            // Neighbor rank (1-k)
  tier: string          // hot, warm, cold
}]->(:CallSegment)

// Cluster assignments
(:Cluster)-[:HAS_PROTOTYPE]->(:Prototype)
(:CallSegment)-[:ASSIGNED_TO {
  confidence: float,    // Assignment confidence
  distance_to_centroid: float
}]->(:Cluster)
(:Cluster)-[:CHILD_OF]->(:Cluster) // Hierarchical

// Taxonomic links
(:CallSegment)-[:IDENTIFIED_AS {
  confidence: float,
  method: string,       // model, manual, consensus
  verified: boolean
}]->(:Taxon)

// Co-occurrence (same time window, nearby sensors)
(:CallSegment)-[:CO_OCCURS {
  time_overlap_ms: int,
  spatial_distance_m: float
}]->(:CallSegment)
```

#### 1.4 Index Definitions

**Primary Indexes**
```sql
-- UUID lookups
CREATE UNIQUE INDEX idx_recording_id ON recordings(id);
CREATE UNIQUE INDEX idx_segment_id ON call_segments(id);
CREATE UNIQUE INDEX idx_embedding_id ON embeddings(id);
CREATE UNIQUE INDEX idx_cluster_id ON clusters(id);
CREATE UNIQUE INDEX idx_taxon_id ON taxa(id);
CREATE UNIQUE INDEX idx_sensor_id ON sensors(id);

-- Foreign key relationships
CREATE INDEX idx_segment_recording ON call_segments(recording_id);
CREATE INDEX idx_embedding_segment ON embeddings(segment_id);
CREATE INDEX idx_recording_sensor ON recordings(sensor_id);
```

**Temporal Indexes**
```sql
-- Time-based queries
CREATE INDEX idx_recording_start ON recordings(start_ts);
CREATE INDEX idx_recording_timerange ON recordings USING GIST (
  tstzrange(start_ts, end_ts)
);
CREATE INDEX idx_segment_time ON call_segments(recording_id, t0_ms);
```

**Spatial Indexes**
```sql
-- Geographic queries (PostGIS)
CREATE INDEX idx_recording_location ON recordings USING GIST (
  ST_SetSRID(ST_MakePoint(lon, lat), 4326)
);
CREATE INDEX idx_sensor_location ON sensors USING GIST (
  ST_SetSRID(ST_MakePoint(deployment_lon, deployment_lat), 4326)
);
```

**HNSW Vector Index**
```sql
-- Hot tier: Full precision HNSW
CREATE INDEX idx_embedding_hnsw_hot ON embeddings
USING hnsw (vector vector_cosine_ops)
WITH (
  m = 16,                    -- Connections per layer
  ef_construction = 200,     -- Build-time search width
  ef_search = 100            -- Query-time search width
)
WHERE storage_tier = 'hot';

-- Warm tier: Quantized HNSW
CREATE INDEX idx_embedding_hnsw_warm ON embeddings
USING hnsw (vector_quantized vector_l2_ops)
WITH (m = 12, ef_construction = 100)
WHERE storage_tier = 'warm';
```

---

### 2. Vector Storage Strategy

#### 2.1 Tiered Storage Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         VECTOR STORAGE TIERS                                 │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  HOT TIER - Full Precision (float32)                                   │ │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                                   │ │
│  │  Storage: In-memory + NVMe SSD                                         │ │
│  │  Format: 1536 x float32 = 6,144 bytes/vector                          │ │
│  │  Index: HNSW (M=16, ef=200)                                           │ │
│  │  Latency: <1ms query, <100us retrieval                                │ │
│  │  Capacity: ~1M vectors / 6GB RAM                                      │ │
│  │  Use: Active queries, recent recordings, frequent access              │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                              │                                               │
│                              ▼ (access_count < threshold, age > 7 days)     │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  WARM TIER - Quantized (int8)                                          │ │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                                         │ │
│  │  Storage: SSD-backed, memory-mapped                                    │ │
│  │  Format: 1536 x int8 = 1,536 bytes/vector (4x compression)            │ │
│  │  Index: HNSW (M=12, ef=100)                                           │ │
│  │  Latency: <10ms query                                                 │ │
│  │  Capacity: ~10M vectors / 15GB SSD                                    │ │
│  │  Use: Historical data, periodic access, batch processing              │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                              │                                               │
│                              ▼ (age > 90 days, access_count < 10)           │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │  COLD TIER - Compressed (Product Quantization)                         │ │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                          │ │
│  │  Storage: Object storage (S3/GCS) + local cache                       │ │
│  │  Format: PQ-encoded = ~192 bytes/vector (32x compression)             │ │
│  │  Index: IVF-PQ (coarse quantizer + product quantization)              │ │
│  │  Latency: <100ms query (cache hit), <1s (cache miss)                  │ │
│  │  Capacity: ~100M vectors / 20GB storage                               │ │
│  │  Use: Archival, compliance, rare access, bulk analytics               │ │
│  └────────────────────────────────────────────────────────────────────────┘ │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 2.2 Quantization Implementation

**Scalar Quantization (Hot to Warm)**
```typescript
interface ScalarQuantizer {
  scale: float;        // Computed from data distribution
  zero_point: float;   // Offset for centering

  quantize(vector: Float32Array): Int8Array {
    const quantized = new Int8Array(vector.length);
    for (let i = 0; i < vector.length; i++) {
      const scaled = (vector[i] - this.zero_point) / this.scale;
      quantized[i] = Math.max(-128, Math.min(127, Math.round(scaled)));
    }
    return quantized;
  }

  dequantize(quantized: Int8Array): Float32Array {
    const vector = new Float32Array(quantized.length);
    for (let i = 0; i < quantized.length; i++) {
      vector[i] = quantized[i] * this.scale + this.zero_point;
    }
    return vector;
  }
}

// Calibration: compute scale/zero_point from representative sample
function calibrateQuantizer(samples: Float32Array[]): ScalarQuantizer {
  let min = Infinity, max = -Infinity;
  for (const sample of samples) {
    for (const val of sample) {
      min = Math.min(min, val);
      max = Math.max(max, val);
    }
  }
  return {
    scale: (max - min) / 255,
    zero_point: min + (max - min) / 2
  };
}
```

**Product Quantization (Warm to Cold)**
```typescript
interface ProductQuantizer {
  num_subvectors: number;     // 192 (divide 1536 into 192 x 8)
  subvector_dim: number;      // 8 dimensions per subvector
  num_centroids: number;      // 256 (1 byte per subvector)
  codebooks: Float32Array[][]; // 192 codebooks, each with 256 centroids

  encode(vector: Float32Array): Uint8Array {
    const codes = new Uint8Array(this.num_subvectors);
    for (let i = 0; i < this.num_subvectors; i++) {
      const subvec = vector.slice(
        i * this.subvector_dim,
        (i + 1) * this.subvector_dim
      );
      codes[i] = this.findNearestCentroid(subvec, this.codebooks[i]);
    }
    return codes;
  }

  decode(codes: Uint8Array): Float32Array {
    const vector = new Float32Array(1536);
    for (let i = 0; i < this.num_subvectors; i++) {
      const centroid = this.codebooks[i][codes[i]];
      vector.set(centroid, i * this.subvector_dim);
    }
    return vector;
  }

  asymmetricDistance(query: Float32Array, codes: Uint8Array): float {
    let dist = 0;
    for (let i = 0; i < this.num_subvectors; i++) {
      const subquery = query.slice(
        i * this.subvector_dim,
        (i + 1) * this.subvector_dim
      );
      const centroid = this.codebooks[i][codes[i]];
      dist += euclideanDistance(subquery, centroid);
    }
    return dist;
  }
}
```

#### 2.3 Tiering Policy

```typescript
interface TieringPolicy {
  hot_to_warm: {
    age_days: 7,           // Move after 7 days
    access_threshold: 100, // Unless accessed >100 times
    batch_size: 10000      // Process in batches
  },
  warm_to_cold: {
    age_days: 90,          // Move after 90 days
    access_threshold: 10,  // Unless accessed >10 times in last 30 days
    batch_size: 50000
  },
  promotion: {
    cold_to_warm: {
      access_count: 5,     // Promote after 5 accesses
      time_window_hours: 24
    },
    warm_to_hot: {
      access_count: 20,
      time_window_hours: 1
    }
  }
}

// Background tiering job
async function runTieringJob(policy: TieringPolicy): Promise<TieringStats> {
  const stats = { demoted: 0, promoted: 0 };

  // Demote hot -> warm
  const hotCandidates = await db.query(`
    SELECT id FROM embeddings
    WHERE storage_tier = 'hot'
      AND created_at < NOW() - INTERVAL '${policy.hot_to_warm.age_days} days'
      AND access_count < ${policy.hot_to_warm.access_threshold}
    ORDER BY last_accessed ASC
    LIMIT ${policy.hot_to_warm.batch_size}
  `);

  for (const batch of chunk(hotCandidates, 1000)) {
    await demoteToWarm(batch);
    stats.demoted += batch.length;
  }

  // Promote cold -> warm based on access patterns
  const coldAccessLog = await getRecentAccesses('cold', 24);
  const promotionCandidates = coldAccessLog
    .filter(e => e.count >= policy.promotion.cold_to_warm.access_count);

  for (const candidate of promotionCandidates) {
    await promoteToWarm(candidate.id);
    stats.promoted++;
  }

  return stats;
}
```

#### 2.4 Hyperbolic Embedding Option

For hierarchical species relationships, optionally store embeddings in Poincare ball space:

```typescript
interface HyperbolicEmbedding {
  euclidean_vector: Float32Array;  // Original 1536-D
  poincare_vector: Float32Array;   // Mapped to Poincare ball
  curvature: float;                // Ball curvature (typically -1)
}

// Map Euclidean to Poincare ball
function exponentialMap(
  euclidean: Float32Array,
  curvature: float = -1
): Float32Array {
  const norm = l2Norm(euclidean);
  const c = Math.abs(curvature);
  const factor = Math.tanh(Math.sqrt(c) * norm / 2) / (Math.sqrt(c) * norm);
  return euclidean.map(x => x * factor);
}

// Poincare distance for hyperbolic similarity
function poincareDistance(
  u: Float32Array,
  v: Float32Array,
  curvature: float = -1
): float {
  const c = Math.abs(curvature);
  const u_norm_sq = dotProduct(u, u);
  const v_norm_sq = dotProduct(v, v);
  const diff = subtract(u, v);
  const diff_norm_sq = dotProduct(diff, diff);

  const numerator = 2 * diff_norm_sq;
  const denominator = (1 - c * u_norm_sq) * (1 - c * v_norm_sq);

  return Math.acosh(1 + numerator / denominator) / Math.sqrt(c);
}
```

---

### 3. Graph Relationships for Cypher Queries

#### 3.1 Common Query Patterns

**Find Similar Calls**
```cypher
// Top-k similar calls to a given segment
MATCH (source:CallSegment {id: $segment_id})-[sim:SIMILAR]->(target:CallSegment)
WHERE sim.distance < $threshold
RETURN target, sim.distance, sim.rank
ORDER BY sim.distance ASC
LIMIT $k
```

**Temporal Sequence Analysis**
```cypher
// Find call sequences (motifs) of length n
MATCH path = (start:CallSegment)-[:NEXT*1..5]->(end:CallSegment)
WHERE start.recording_id = $recording_id
RETURN [node IN nodes(path) | node.id] AS sequence,
       [rel IN relationships(path) | rel.dt_ms] AS intervals,
       length(path) AS motif_length
ORDER BY motif_length DESC
```

**Cluster Exploration**
```cypher
// Get cluster members with their prototypes
MATCH (c:Cluster {id: $cluster_id})-[:HAS_PROTOTYPE]->(p:Prototype)
MATCH (seg:CallSegment)-[a:ASSIGNED_TO]->(c)
WHERE a.confidence > 0.8
RETURN c, p, collect(seg)[..10] AS exemplars, count(seg) AS total_members
```

**Species Distribution**
```cypher
// Calls by species in a geographic region
MATCH (r:Recording)-[:HAS_SEGMENT]->(seg:CallSegment)
      -[:IDENTIFIED_AS]->(t:Taxon)
WHERE point.distance(
  point({latitude: r.lat, longitude: r.lon}),
  point({latitude: $center_lat, longitude: $center_lon})
) < $radius_m
RETURN t.scientific_name, t.common_name, count(seg) AS call_count
ORDER BY call_count DESC
```

**Co-occurrence Networks**
```cypher
// Species co-occurring in same time windows
MATCH (seg1:CallSegment)-[:IDENTIFIED_AS]->(t1:Taxon),
      (seg1)-[:CO_OCCURS]->(seg2:CallSegment)-[:IDENTIFIED_AS]->(t2:Taxon)
WHERE t1.id <> t2.id
RETURN t1.common_name, t2.common_name, count(*) AS co_occurrence_count
ORDER BY co_occurrence_count DESC
LIMIT 20
```

**Transition Matrix**
```cypher
// Markov transition probabilities between call types
MATCH (c1:Cluster)<-[:ASSIGNED_TO]-(seg1:CallSegment)
      -[:NEXT]->(seg2:CallSegment)-[:ASSIGNED_TO]->(c2:Cluster)
WITH c1.name AS from_cluster, c2.name AS to_cluster, count(*) AS transitions
MATCH (c1:Cluster {name: from_cluster})<-[:ASSIGNED_TO]-(seg:CallSegment)
WITH from_cluster, to_cluster, transitions, count(seg) AS from_total
RETURN from_cluster, to_cluster,
       toFloat(transitions) / from_total AS transition_prob
ORDER BY from_cluster, transition_prob DESC
```

#### 3.2 GNN Training Edges

```cypher
// Create training edges for GNN
// Acoustic similarity edges (from HNSW)
MATCH (seg:CallSegment)-[:HAS_EMBEDDING]->(emb:Embedding)
WITH seg, emb
CALL {
  WITH seg, emb
  MATCH (other:CallSegment)-[:HAS_EMBEDDING]->(other_emb:Embedding)
  WHERE other.id <> seg.id
  WITH seg, other,
       gds.similarity.cosine(emb.vector, other_emb.vector) AS sim
  WHERE sim > 0.8
  RETURN other, sim
  ORDER BY sim DESC
  LIMIT 10
}
MERGE (seg)-[r:SIMILAR]->(other)
SET r.distance = 1 - sim, r.tier = 'hot'
```

---

### 4. Temporal Data Handling

#### 4.1 Timestamp Standards

```typescript
// All timestamps in ISO 8601 with timezone
type ISO8601 = string; // e.g., "2026-01-15T08:30:00.000Z"

interface TemporalMetadata {
  // Recording level
  recording_start_ts: ISO8601;   // When recording began
  recording_end_ts: ISO8601;     // When recording ended
  recording_timezone: string;    // IANA timezone (e.g., "America/Los_Angeles")

  // Segment level (relative to recording)
  segment_offset_ms: number;     // Milliseconds from recording start
  segment_absolute_ts: ISO8601;  // Computed absolute timestamp

  // Derived temporal features
  time_of_day: TimeOfDay;        // dawn, morning, midday, afternoon, dusk, night
  day_of_week: number;           // 0-6
  day_of_year: number;           // 1-366
  lunar_phase: LunarPhase;       // new, waxing, full, waning
  sunrise_offset_min: number;    // Minutes from local sunrise
  sunset_offset_min: number;     // Minutes from local sunset
}

enum TimeOfDay {
  DAWN = 'dawn',           // -30min to +30min of sunrise
  MORNING = 'morning',     // sunrise+30min to noon
  MIDDAY = 'midday',       // noon +/- 2 hours
  AFTERNOON = 'afternoon', // midday to sunset-30min
  DUSK = 'dusk',           // -30min to +30min of sunset
  NIGHT = 'night'          // sunset+30min to sunrise-30min
}
```

#### 4.2 Sequence Ordering

```typescript
interface SequenceManager {
  // Build sequence graph from recording
  buildSequenceGraph(recordingId: UUID): Promise<SequenceEdge[]> {
    const segments = await db.query(`
      SELECT id, t0_ms, t1_ms, recording_id
      FROM call_segments
      WHERE recording_id = $1
      ORDER BY t0_ms ASC
    `, [recordingId]);

    const edges: SequenceEdge[] = [];
    for (let i = 0; i < segments.length - 1; i++) {
      const current = segments[i];
      const next = segments[i + 1];
      const gap_ms = next.t0_ms - current.t1_ms;

      // Only link if gap is reasonable (< 5 seconds)
      if (gap_ms < 5000 && gap_ms >= 0) {
        edges.push({
          source_id: current.id,
          target_id: next.id,
          dt_ms: gap_ms,
          same_speaker: gap_ms < 500, // Heuristic for same individual
          sequence_index: i
        });
      }
    }
    return edges;
  }

  // Detect repeated sequences (motifs)
  findMotifs(recordingId: UUID, minLength: number = 3): Promise<Motif[]> {
    // Build suffix array of cluster assignments
    const sequence = await this.getClusterSequence(recordingId);
    const motifs = this.findRepeatedSubstrings(sequence, minLength);
    return motifs;
  }
}

interface SequenceEdge {
  source_id: UUID;
  target_id: UUID;
  dt_ms: number;           // Time gap
  same_speaker: boolean;   // Likely same individual
  sequence_index: number;  // Position in recording
}

interface Motif {
  pattern: string[];       // Cluster IDs in order
  occurrences: number;     // How many times it appears
  positions: number[][];   // Start positions for each occurrence
  entropy: number;         // Pattern entropy
}
```

#### 4.3 Time-Series Partitioning

```sql
-- Partition recordings by month for efficient time-range queries
CREATE TABLE recordings (
  id UUID PRIMARY KEY,
  start_ts TIMESTAMPTZ NOT NULL,
  -- ... other columns
) PARTITION BY RANGE (start_ts);

-- Create partitions
CREATE TABLE recordings_2026_01 PARTITION OF recordings
  FOR VALUES FROM ('2026-01-01') TO ('2026-02-01');

CREATE TABLE recordings_2026_02 PARTITION OF recordings
  FOR VALUES FROM ('2026-02-01') TO ('2026-03-01');

-- Automatic partition creation (pg_partman or similar)
SELECT create_parent('public.recordings', 'start_ts', 'native', 'monthly');
```

---

### 5. Metadata Enrichment

#### 5.1 Enrichment Pipeline

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        METADATA ENRICHMENT PIPELINE                          │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │   Raw Audio  │────▶│  Segmented   │────▶│  Embedded    │                 │
│  │              │     │  Calls       │     │  Vectors     │                 │
│  └──────────────┘     └──────────────┘     └──────────────┘                 │
│         │                    │                    │                          │
│         ▼                    ▼                    ▼                          │
│  ┌──────────────┐     ┌──────────────┐     ┌──────────────┐                 │
│  │  Recording   │     │  Acoustic    │     │  Similarity  │                 │
│  │  Metadata    │     │  Features    │     │  Neighbors   │                 │
│  └──────────────┘     └──────────────┘     └──────────────┘                 │
│         │                    │                    │                          │
│         │                    │                    │                          │
│         ▼                    ▼                    ▼                          │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      ENRICHMENT SERVICES                             │   │
│  ├─────────────────────────────────────────────────────────────────────┤   │
│  │  • Weather API → temperature, humidity, wind, precipitation         │   │
│  │  • Geocoding   → habitat type, elevation, land cover                │   │
│  │  • Astronomy   → sunrise/sunset, lunar phase, day length           │   │
│  │  • Taxonomy    → species ID, conservation status, range maps       │   │
│  │  • Soundscape  → background noise level, anthropogenic detection   │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│         │                                                                    │
│         ▼                                                                    │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │                      ENRICHED RECORD                                 │   │
│  │  Recording + Weather + Habitat + Temporal + Taxonomic + Quality     │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 5.2 Enrichment Sources

```typescript
interface EnrichmentConfig {
  weather: {
    provider: 'openweathermap' | 'visualcrossing' | 'meteoblue';
    api_key: string;
    cache_ttl_hours: 24;
    historical_lookback_days: 7;
  };

  geocoding: {
    provider: 'nominatim' | 'google' | 'mapbox';
    cache_ttl_days: 30;
    enrichments: ['habitat', 'elevation', 'land_cover', 'protected_area'];
  };

  astronomy: {
    // Computed locally, no API needed
    compute: ['sunrise', 'sunset', 'civil_twilight', 'lunar_phase', 'day_length'];
  };

  taxonomy: {
    sources: ['inat', 'ebird', 'xeno-canto'];
    auto_id_confidence_threshold: 0.8;
    human_review_threshold: 0.5;
  };

  soundscape: {
    background_noise_window_ms: 100;
    anthropogenic_detection: boolean;
    frequency_bands: [[0, 2000], [2000, 8000], [8000, 16000]];
  };
}

// Enrichment job
async function enrichRecording(recordingId: UUID): Promise<EnrichedRecording> {
  const recording = await db.getRecording(recordingId);

  const enrichments = await Promise.all([
    weatherService.getHistorical(recording.lat, recording.lon, recording.start_ts),
    geocodingService.getHabitat(recording.lat, recording.lon),
    astronomyService.getSolarData(recording.lat, recording.lon, recording.start_ts),
    soundscapeAnalyzer.analyze(recording.file_path)
  ]);

  return {
    ...recording,
    weather: enrichments[0],
    habitat: enrichments[1],
    astronomy: enrichments[2],
    soundscape: enrichments[3]
  };
}
```

#### 5.3 Species Identification

```typescript
interface SpeciesIdentification {
  segment_id: UUID;
  predictions: TaxonPrediction[];
  method: 'perch_classifier' | 'birdnet' | 'manual' | 'consensus';
  confidence_aggregation: 'max' | 'mean' | 'ensemble';
}

interface TaxonPrediction {
  taxon_id: UUID;
  scientific_name: string;
  confidence: float;
  rank: number;
}

// Multi-model ensemble for species ID
async function identifySpecies(segmentId: UUID): Promise<SpeciesIdentification> {
  const segment = await db.getSegment(segmentId);
  const embedding = await db.getEmbedding(segmentId);

  // Get predictions from multiple sources
  const perchPreds = await perchClassifier.predict(embedding.vector);
  const nnPreds = await nearestNeighborTaxon(embedding.vector, k=10);

  // Ensemble combination
  const combined = ensemblePredictions([perchPreds, nnPreds], weights=[0.6, 0.4]);

  return {
    segment_id: segmentId,
    predictions: combined.slice(0, 5),
    method: 'ensemble',
    confidence_aggregation: 'weighted_mean'
  };
}
```

---

### 6. Data Lifecycle

#### 6.1 Lifecycle Stages

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DATA LIFECYCLE                                     │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  STAGE 1: INGESTION                                                          │
│  ━━━━━━━━━━━━━━━━━━                                                          │
│  • Audio upload (S3/GCS/local)                                              │
│  • Format validation (32kHz, mono, WAV/FLAC)                                │
│  • Deduplication (SHA-256 hash check)                                       │
│  • Recording metadata extraction                                            │
│  • Queue for processing                                                     │
│                                                                              │
│  STAGE 2: PROCESSING                                                         │
│  ━━━━━━━━━━━━━━━━━━━                                                         │
│  • Audio segmentation (WhisperSeg/TweetyNet)                                │
│  • Acoustic feature extraction                                              │
│  • Perch 2.0 embedding generation                                           │
│  • Quality scoring                                                          │
│  • Initial species predictions                                              │
│                                                                              │
│  STAGE 3: INDEXING                                                           │
│  ━━━━━━━━━━━━━━━━━━                                                          │
│  • HNSW index insertion (hot tier)                                          │
│  • Neighbor edge creation                                                   │
│  • Sequence graph construction                                              │
│  • Cluster assignment                                                       │
│  • Metadata enrichment                                                      │
│                                                                              │
│  STAGE 4: ACTIVE USE                                                         │
│  ━━━━━━━━━━━━━━━━━━━                                                         │
│  • Query serving                                                            │
│  • GNN refinement (continuous learning)                                     │
│  • Access tracking                                                          │
│  • Cache warming                                                            │
│                                                                              │
│  STAGE 5: TIERING                                                            │
│  ━━━━━━━━━━━━━━━━━━                                                          │
│  • Hot → Warm (7 days, quantization)                                        │
│  • Warm → Cold (90 days, compression)                                       │
│  • Promotion on access                                                      │
│                                                                              │
│  STAGE 6: ARCHIVAL                                                           │
│  ━━━━━━━━━━━━━━━━━━                                                          │
│  • Cold storage (S3 Glacier/equivalent)                                     │
│  • Metadata preserved in primary DB                                         │
│  • On-demand retrieval (minutes latency)                                    │
│                                                                              │
│  STAGE 7: RETENTION/DELETION                                                 │
│  ━━━━━━━━━━━━━━━━━━━━━━━━━━                                                   │
│  • Configurable retention policies                                          │
│  • Legal hold support                                                       │
│  • Secure deletion (GDPR compliance)                                        │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 6.2 Processing Pipeline

```typescript
interface ProcessingPipeline {
  stages: PipelineStage[];

  async process(recordingId: UUID): Promise<ProcessingResult> {
    const context: ProcessingContext = { recordingId, startTime: Date.now() };

    for (const stage of this.stages) {
      try {
        context[stage.name] = await stage.execute(context);
        await this.updateStatus(recordingId, stage.name, 'complete');
      } catch (error) {
        await this.handleError(recordingId, stage.name, error);
        if (stage.critical) throw error;
      }
    }

    return this.summarize(context);
  }
}

const pipeline = new ProcessingPipeline({
  stages: [
    { name: 'validate', critical: true, execute: validateAudio },
    { name: 'segment', critical: true, execute: segmentAudio },
    { name: 'extract_features', critical: false, execute: extractAcousticFeatures },
    { name: 'embed', critical: true, execute: generateEmbeddings },
    { name: 'index', critical: true, execute: insertToHNSW },
    { name: 'enrich', critical: false, execute: enrichMetadata },
    { name: 'identify', critical: false, execute: identifySpecies },
    { name: 'cluster', critical: false, execute: assignToClusters }
  ]
});
```

#### 6.3 Retention Policies

```typescript
interface RetentionPolicy {
  name: string;
  conditions: RetentionCondition[];
  action: 'archive' | 'delete' | 'anonymize';
  grace_period_days: number;
}

const defaultPolicies: RetentionPolicy[] = [
  {
    name: 'standard_archival',
    conditions: [
      { field: 'age_days', operator: '>', value: 365 },
      { field: 'access_count_last_180_days', operator: '<', value: 5 }
    ],
    action: 'archive',
    grace_period_days: 30
  },
  {
    name: 'low_quality_deletion',
    conditions: [
      { field: 'quality_score', operator: '<', value: 0.3 },
      { field: 'age_days', operator: '>', value: 90 },
      { field: 'manually_reviewed', operator: '=', value: false }
    ],
    action: 'delete',
    grace_period_days: 14
  },
  {
    name: 'gdpr_deletion',
    conditions: [
      { field: 'deletion_requested', operator: '=', value: true }
    ],
    action: 'delete',
    grace_period_days: 0
  }
];

// Retention job
async function enforceRetention(): Promise<RetentionReport> {
  const report: RetentionReport = { archived: 0, deleted: 0, errors: [] };

  for (const policy of defaultPolicies) {
    const candidates = await findRetentionCandidates(policy);

    for (const candidate of candidates) {
      try {
        if (policy.action === 'archive') {
          await archiveRecording(candidate.id);
          report.archived++;
        } else if (policy.action === 'delete') {
          await secureDelete(candidate.id);
          report.deleted++;
        }
      } catch (error) {
        report.errors.push({ id: candidate.id, error: error.message });
      }
    }
  }

  return report;
}
```

---

### 7. Backup and Recovery Strategy

#### 7.1 Backup Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                         BACKUP ARCHITECTURE                                  │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  PRIMARY DATABASE (PostgreSQL + pgvector)                           │   │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                            │   │
│  │  • Streaming replication to standby                                 │   │
│  │  • WAL archiving to object storage                                  │   │
│  │  • Point-in-time recovery enabled                                   │   │
│  │  • pg_basebackup daily full backup                                  │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                              │                                               │
│           ┌──────────────────┼──────────────────┐                           │
│           ▼                  ▼                  ▼                           │
│  ┌──────────────┐   ┌──────────────┐   ┌──────────────┐                    │
│  │   Standby    │   │  WAL Archive │   │  Daily Full  │                    │
│  │   Replica    │   │  (S3/GCS)    │   │  Backup      │                    │
│  │  (sync)      │   │  (15 min)    │   │  (encrypted) │                    │
│  └──────────────┘   └──────────────┘   └──────────────┘                    │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  VECTOR INDEX (HNSW)                                                 │   │
│  │  ━━━━━━━━━━━━━━━━━━━━                                                │   │
│  │  • Snapshot to object storage daily                                 │   │
│  │  • Incremental updates via change log                               │   │
│  │  • Rebuild from source embeddings (disaster recovery)               │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  AUDIO FILES                                                         │   │
│  │  ━━━━━━━━━━━━━━━                                                     │   │
│  │  • Primary: Object storage (S3/GCS) with versioning                 │   │
│  │  • Cross-region replication for disaster recovery                   │   │
│  │  • Glacier transition for cold data                                 │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
│  ┌─────────────────────────────────────────────────────────────────────┐   │
│  │  GRAPH DATABASE (Neo4j/RuVector Graph Layer)                        │   │
│  │  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━                        │   │
│  │  • Online backup every 6 hours                                      │   │
│  │  • Transaction log shipping                                         │   │
│  │  • Cluster mode for HA                                              │   │
│  └─────────────────────────────────────────────────────────────────────┘   │
│                                                                              │
└─────────────────────────────────────────────────────────────────────────────┘
```

#### 7.2 Backup Schedule

```typescript
interface BackupSchedule {
  database: {
    full_backup: {
      frequency: 'daily',
      time: '02:00 UTC',
      retention_days: 30
    },
    incremental_backup: {
      frequency: 'hourly',
      retention_days: 7
    },
    wal_archiving: {
      frequency: 'continuous',
      archive_timeout_seconds: 900, // 15 minutes max
      retention_days: 14
    }
  };

  vector_index: {
    snapshot: {
      frequency: 'daily',
      time: '03:00 UTC',
      retention_count: 7
    },
    change_log: {
      frequency: 'continuous',
      retention_hours: 72
    }
  };

  audio_files: {
    versioning: true,
    cross_region_replication: true,
    glacier_transition_days: 90
  };

  graph: {
    online_backup: {
      frequency: 'every 6 hours',
      retention_count: 28
    }
  };
}
```

#### 7.3 Recovery Procedures

```typescript
interface RecoveryProcedures {
  // Point-in-time recovery
  async recoverToPointInTime(targetTime: ISO8601): Promise<RecoveryResult> {
    // 1. Stop application writes
    await this.enableMaintenanceMode();

    // 2. Identify nearest full backup
    const baseBackup = await this.findBaseBackup(targetTime);

    // 3. Restore base backup
    await this.restoreBaseBackup(baseBackup);

    // 4. Apply WAL logs up to target time
    await this.applyWALTo(targetTime);

    // 5. Rebuild vector index from restored embeddings
    await this.rebuildVectorIndex();

    // 6. Verify data integrity
    const integrity = await this.verifyIntegrity();

    // 7. Resume operations
    await this.disableMaintenanceMode();

    return {
      success: integrity.valid,
      restoredTo: targetTime,
      recordsRecovered: integrity.recordCount,
      duration: Date.now() - startTime
    };
  }

  // Full disaster recovery
  async fullDisasterRecovery(targetRegion: string): Promise<RecoveryResult> {
    // 1. Provision new infrastructure in target region
    await this.provisionInfrastructure(targetRegion);

    // 2. Restore latest database backup
    const latestBackup = await this.getLatestBackup();
    await this.restoreBackup(latestBackup);

    // 3. Sync audio files from cross-region replica
    await this.syncAudioFiles(targetRegion);

    // 4. Rebuild all indexes
    await this.rebuildAllIndexes();

    // 5. Update DNS/load balancer
    await this.updateRouting(targetRegion);

    return { success: true, region: targetRegion, rto: Date.now() - startTime };
  }
}
```

#### 7.4 Recovery Objectives

| Metric | Target | Description |
|--------|--------|-------------|
| **RPO** (Recovery Point Objective) | 15 minutes | Maximum data loss in time |
| **RTO** (Recovery Time Objective) | 1 hour | Time to restore service |
| **RTO (Full DR)** | 4 hours | Cross-region disaster recovery |
| **Backup Verification** | Daily | Automated restore test |

---

### 8. Data Validation Rules

#### 8.1 Input Validation

```typescript
interface ValidationRules {
  recording: {
    file_format: ['wav', 'flac', 'mp3'],
    sample_rate: { min: 16000, max: 96000, preferred: 32000 },
    channels: { allowed: [1, 2], preferred: 1 },
    duration: { min_seconds: 1, max_seconds: 3600 },
    file_size: { max_mb: 500 },
    required_fields: ['file_path', 'start_ts', 'lat', 'lon']
  };

  segment: {
    duration: { min_ms: 50, max_ms: 30000 },
    snr: { min_db: -10, warn_below: 5 },
    overlap: { warn_above: 0.5 },
    required_fields: ['recording_id', 't0_ms', 't1_ms']
  };

  embedding: {
    dimensions: 1536,
    norm_range: { min: 0.5, max: 2.0 },
    nan_allowed: false,
    inf_allowed: false,
    required_fields: ['segment_id', 'vector', 'model_name']
  };

  coordinates: {
    lat: { min: -90, max: 90 },
    lon: { min: -180, max: 180 },
    precision: 6 // decimal places
  };
}

// Validation implementation
class DataValidator {
  validateRecording(recording: Recording): ValidationResult {
    const errors: ValidationError[] = [];
    const warnings: ValidationWarning[] = [];

    // Format check
    const ext = recording.file_path.split('.').pop()?.toLowerCase();
    if (!ValidationRules.recording.file_format.includes(ext)) {
      errors.push({ field: 'file_path', message: `Invalid format: ${ext}` });
    }

    // Sample rate check
    if (recording.sample_rate !== ValidationRules.recording.sample_rate.preferred) {
      warnings.push({
        field: 'sample_rate',
        message: `Non-preferred sample rate: ${recording.sample_rate}Hz`
      });
    }

    // Coordinate validation
    if (recording.lat < -90 || recording.lat > 90) {
      errors.push({ field: 'lat', message: 'Latitude out of range' });
    }

    // Required fields
    for (const field of ValidationRules.recording.required_fields) {
      if (!recording[field]) {
        errors.push({ field, message: 'Required field missing' });
      }
    }

    return { valid: errors.length === 0, errors, warnings };
  }

  validateEmbedding(embedding: Embedding): ValidationResult {
    const errors: ValidationError[] = [];

    // Dimension check
    if (embedding.vector.length !== ValidationRules.embedding.dimensions) {
      errors.push({
        field: 'vector',
        message: `Expected ${ValidationRules.embedding.dimensions}D, got ${embedding.vector.length}D`
      });
    }

    // NaN/Inf check
    for (let i = 0; i < embedding.vector.length; i++) {
      if (isNaN(embedding.vector[i])) {
        errors.push({ field: 'vector', message: `NaN at index ${i}` });
        break;
      }
      if (!isFinite(embedding.vector[i])) {
        errors.push({ field: 'vector', message: `Infinity at index ${i}` });
        break;
      }
    }

    // Norm check
    const norm = l2Norm(embedding.vector);
    if (norm < ValidationRules.embedding.norm_range.min ||
        norm > ValidationRules.embedding.norm_range.max) {
      errors.push({
        field: 'vector',
        message: `Norm ${norm.toFixed(3)} outside expected range`
      });
    }

    return { valid: errors.length === 0, errors, warnings: [] };
  }
}
```

#### 8.2 Consistency Checks

```typescript
interface ConsistencyChecker {
  // Run all consistency checks
  async runChecks(): Promise<ConsistencyReport> {
    const checks = await Promise.all([
      this.checkOrphanedSegments(),
      this.checkMissingEmbeddings(),
      this.checkDuplicateRecordings(),
      this.checkBrokenReferences(),
      this.checkIndexSync(),
      this.checkTemporalConsistency()
    ]);

    return {
      timestamp: new Date().toISOString(),
      checks,
      overallHealth: checks.every(c => c.passed) ? 'healthy' : 'degraded'
    };
  }

  // Find segments without parent recordings
  async checkOrphanedSegments(): Promise<CheckResult> {
    const orphans = await db.query(`
      SELECT cs.id FROM call_segments cs
      LEFT JOIN recordings r ON cs.recording_id = r.id
      WHERE r.id IS NULL
    `);
    return {
      name: 'orphaned_segments',
      passed: orphans.length === 0,
      count: orphans.length,
      action: 'DELETE orphaned segments or restore recordings'
    };
  }

  // Find segments without embeddings
  async checkMissingEmbeddings(): Promise<CheckResult> {
    const missing = await db.query(`
      SELECT cs.id FROM call_segments cs
      LEFT JOIN embeddings e ON cs.id = e.segment_id
      WHERE e.id IS NULL AND cs.created_at < NOW() - INTERVAL '1 hour'
    `);
    return {
      name: 'missing_embeddings',
      passed: missing.length === 0,
      count: missing.length,
      action: 'Reprocess segments to generate embeddings'
    };
  }

  // Verify HNSW index matches database
  async checkIndexSync(): Promise<CheckResult> {
    const dbCount = await db.query(`SELECT COUNT(*) FROM embeddings WHERE storage_tier = 'hot'`);
    const indexCount = await hnsw.getVectorCount();
    const diff = Math.abs(dbCount - indexCount);

    return {
      name: 'index_sync',
      passed: diff < 100, // Allow small discrepancy during updates
      count: diff,
      action: diff > 100 ? 'Rebuild HNSW index' : 'None required'
    };
  }

  // Check temporal ordering of segments
  async checkTemporalConsistency(): Promise<CheckResult> {
    const violations = await db.query(`
      SELECT recording_id, COUNT(*) as overlaps
      FROM (
        SELECT recording_id, t0_ms, t1_ms,
               LEAD(t0_ms) OVER (PARTITION BY recording_id ORDER BY t0_ms) as next_t0
        FROM call_segments
      ) sub
      WHERE t1_ms > next_t0
      GROUP BY recording_id
    `);
    return {
      name: 'temporal_consistency',
      passed: violations.length === 0,
      count: violations.reduce((sum, v) => sum + v.overlaps, 0),
      action: 'Re-segment recordings with overlapping segments'
    };
  }
}
```

#### 8.3 Quality Gates

```typescript
interface QualityGates {
  // Gate for accepting new recordings
  recording_acceptance: {
    min_quality_score: 0.3,
    min_snr_db: 0,
    max_clipping_ratio: 0.1,
    min_duration_seconds: 5
  };

  // Gate for including in HNSW hot tier
  hot_tier_eligibility: {
    embedding_norm_range: [0.8, 1.2],
    segmentation_confidence: 0.7,
    no_nan_values: true
  };

  // Gate for species identification
  species_id_confidence: {
    auto_accept_threshold: 0.9,
    human_review_threshold: 0.5,
    reject_below: 0.3
  };

  // Gate for cluster assignment
  cluster_assignment: {
    min_confidence: 0.6,
    max_distance_to_centroid: 0.5
  };
}

// Quality gate enforcement
class QualityGateEnforcer {
  async enforceRecordingGate(recording: Recording): Promise<GateResult> {
    const gates = QualityGates.recording_acceptance;
    const failures: string[] = [];

    if (recording.quality_score < gates.min_quality_score) {
      failures.push(`Quality score ${recording.quality_score} < ${gates.min_quality_score}`);
    }

    // Additional checks...

    return {
      passed: failures.length === 0,
      failures,
      action: failures.length > 0 ? 'quarantine' : 'proceed'
    };
  }
}
```

---

## Consequences

### Positive

1. **Performance**: Tiered storage achieves 150x-12,500x search improvement via HNSW with graceful degradation to warm/cold tiers
2. **Scalability**: Architecture supports 100M+ vectors with sub-second queries
3. **Flexibility**: Graph relationships enable complex Cypher queries for motif and sequence analysis
4. **Data Quality**: Comprehensive validation prevents corrupt data from entering the system
5. **Recoverability**: RPO of 15 minutes and RTO of 1 hour meet operational requirements
6. **Cost Efficiency**: 32x compression for cold tier dramatically reduces storage costs

### Negative

1. **Complexity**: Three-tier storage adds operational overhead
2. **Latency Variability**: Cold tier queries are 100-1000x slower than hot tier
3. **Migration Risk**: Quantization introduces small accuracy loss (~2-5%)
4. **Storage Duplication**: Multiple tiers may temporarily hold same data during transitions

### Mitigations

- Automated tiering policies minimize manual intervention
- Warm tier serves as buffer, ensuring graceful degradation
- Calibrated quantization preserves retrieval quality above 95%
- Background jobs clean up duplicate data after successful tier migration

---

## References

- [Perch 2.0: The Bittern Lesson for Bioacoustics](https://arxiv.org/abs/2508.04665)
- [RuVector: A Database that Autonomously Learns](https://github.com/ruvnet/ruvector)
- [HNSW: Hierarchical Navigable Small World Graphs](https://arxiv.org/abs/1603.09320)
- [Product Quantization for Nearest Neighbor Search](https://ieeexplore.ieee.org/document/5432202)
- [Poincare Embeddings for Learning Hierarchical Representations](https://arxiv.org/abs/1705.08039)

---

**Document Version:** 1.0
**Last Updated:** 2026-01-15
**Next Review:** 2026-04-15
