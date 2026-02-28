# ADR-008: API Design and Backend Services

## Status
Proposed

## Date
2025-01-15

## Context

7sense is a bioacoustics platform built on RuVector that enables researchers to upload audio recordings, extract call segments, generate embeddings via Perch 2.0, and query a graph database of acoustic relationships. The platform requires a comprehensive API layer that supports:

- Audio upload and processing workflows
- Real-time monitoring via WebSocket
- Complex graph queries for neighbor retrieval, clustering, and sequence analysis
- High-throughput batch operations for large-scale research projects
- Integration with existing bioacoustics tools and workflows

The backend must handle 1536-dimensional embeddings, HNSW-based similarity search, GNN-enhanced retrieval, and Cypher-style graph queries against the RuVector substrate.

## Decision

We will implement a multi-protocol API architecture consisting of:

1. **REST API** for CRUD operations and simple queries (versioned, hypermedia-driven)
2. **GraphQL API** for complex, flexible queries (neighbors, clusters, sequences)
3. **WebSocket API** for real-time monitoring and streaming results
4. **Cypher Query Endpoint** for direct graph database access

### 1. API Design Principles

#### 1.1 RESTful Design

- **Resource-oriented**: URLs represent resources, not actions
- **Stateless**: Each request contains all information needed to process it
- **Uniform interface**: Consistent use of HTTP methods (GET, POST, PUT, PATCH, DELETE)
- **HATEOAS**: Responses include hypermedia links for discoverability

#### 1.2 Versioning Strategy

```
/api/v1/recordings
/api/v2/recordings
```

- Major version in URL path for breaking changes
- Minor versions via `Accept-Version` header for backward-compatible changes
- Deprecation notices in response headers with sunset dates
- Minimum 6-month support window for deprecated versions

#### 1.3 Content Negotiation

```http
Accept: application/json
Accept: application/vnd.sevensense+json; version=1
Accept: application/x-ndjson  # For streaming responses
Content-Type: multipart/form-data  # For audio uploads
```

### 2. Core REST Endpoints

#### 2.1 Recordings

```yaml
# Upload audio recording
POST /api/v1/recordings
Content-Type: multipart/form-data

Request:
  file: <binary audio data>
  metadata:
    sensor_id: string
    latitude: number
    longitude: number
    recorded_at: ISO8601 timestamp
    habitat: string (optional)
    weather: object (optional)
    tags: string[] (optional)

Response: 201 Created
{
  "id": "rec_7f3a9b2c",
  "status": "processing",
  "created_at": "2025-01-15T10:30:00Z",
  "metadata": { ... },
  "segments_count": null,
  "_links": {
    "self": { "href": "/api/v1/recordings/rec_7f3a9b2c" },
    "segments": { "href": "/api/v1/recordings/rec_7f3a9b2c/segments" },
    "status": { "href": "/api/v1/recordings/rec_7f3a9b2c/status" }
  }
}
```

```yaml
# Get recording details
GET /api/v1/recordings/{id}

Response: 200 OK
{
  "id": "rec_7f3a9b2c",
  "status": "completed",
  "duration_ms": 300000,
  "sample_rate": 32000,
  "channels": 1,
  "segments_count": 47,
  "metadata": {
    "sensor_id": "sensor_001",
    "latitude": 37.7749,
    "longitude": -122.4194,
    "recorded_at": "2025-01-10T06:00:00Z",
    "habitat": "coastal_wetland"
  },
  "_links": {
    "self": { "href": "/api/v1/recordings/rec_7f3a9b2c" },
    "segments": { "href": "/api/v1/recordings/rec_7f3a9b2c/segments" },
    "spectrogram": { "href": "/api/v1/recordings/rec_7f3a9b2c/spectrogram" },
    "download": { "href": "/api/v1/recordings/rec_7f3a9b2c/audio" }
  }
}
```

```yaml
# List recordings with filtering
GET /api/v1/recordings?sensor_id=sensor_001&from=2025-01-01&to=2025-01-15&limit=50&cursor=abc123

Response: 200 OK
{
  "items": [ ... ],
  "pagination": {
    "cursor": "def456",
    "has_more": true,
    "total_estimate": 1250
  },
  "_links": {
    "self": { "href": "/api/v1/recordings?..." },
    "next": { "href": "/api/v1/recordings?cursor=def456&..." }
  }
}
```

#### 2.2 Segments

```yaml
# Get segment details
GET /api/v1/segments/{id}

Response: 200 OK
{
  "id": "seg_a1b2c3d4",
  "recording_id": "rec_7f3a9b2c",
  "start_ms": 12500,
  "end_ms": 17500,
  "duration_ms": 5000,
  "snr_db": 24.5,
  "energy": 0.78,
  "embedding_id": "emb_x9y8z7",
  "cluster_id": "cls_dawn_chorus_01",
  "features": {
    "spectral_centroid": 3200.5,
    "pitch_mean": 2800.0,
    "pitch_std": 450.2,
    "rhythm_regularity": 0.85
  },
  "_links": {
    "self": { "href": "/api/v1/segments/seg_a1b2c3d4" },
    "recording": { "href": "/api/v1/recordings/rec_7f3a9b2c" },
    "neighbors": { "href": "/api/v1/segments/seg_a1b2c3d4/neighbors" },
    "spectrogram": { "href": "/api/v1/segments/seg_a1b2c3d4/spectrogram" },
    "audio": { "href": "/api/v1/segments/seg_a1b2c3d4/audio" },
    "next": { "href": "/api/v1/segments/seg_a1b2c3d5" },
    "previous": { "href": "/api/v1/segments/seg_a1b2c3d3" }
  }
}
```

```yaml
# Get similar segments (HNSW neighbors)
GET /api/v1/segments/{id}/neighbors?k=20&min_similarity=0.7&include_cross_recording=true

Response: 200 OK
{
  "query_segment_id": "seg_a1b2c3d4",
  "neighbors": [
    {
      "segment_id": "seg_e5f6g7h8",
      "distance": 0.12,
      "similarity": 0.94,
      "recording_id": "rec_9k8j7h6g",
      "cluster_id": "cls_dawn_chorus_01",
      "_links": {
        "segment": { "href": "/api/v1/segments/seg_e5f6g7h8" }
      }
    },
    ...
  ],
  "metadata": {
    "search_method": "hnsw",
    "ef_search": 200,
    "gnn_reranked": true,
    "latency_ms": 12
  },
  "_links": {
    "self": { "href": "/api/v1/segments/seg_a1b2c3d4/neighbors?k=20" }
  }
}
```

#### 2.3 Clusters

```yaml
# List discovered clusters (call types)
GET /api/v1/clusters?method=hdbscan&min_size=10&limit=50

Response: 200 OK
{
  "items": [
    {
      "id": "cls_dawn_chorus_01",
      "label": "Dawn Chorus Type A",
      "method": "hdbscan",
      "member_count": 847,
      "centroid_embedding_id": "emb_centroid_01",
      "prototype_segments": ["seg_p1", "seg_p2", "seg_p3"],
      "stability_score": 0.92,
      "temporal_distribution": {
        "peak_hours": [5, 6, 7],
        "seasonal_pattern": "spring_peak"
      },
      "_links": {
        "self": { "href": "/api/v1/clusters/cls_dawn_chorus_01" },
        "members": { "href": "/api/v1/clusters/cls_dawn_chorus_01/members" },
        "prototypes": { "href": "/api/v1/clusters/cls_dawn_chorus_01/prototypes" }
      }
    },
    ...
  ],
  "pagination": { ... }
}
```

```yaml
# Get cluster details with prototypes
GET /api/v1/clusters/{id}

Response: 200 OK
{
  "id": "cls_dawn_chorus_01",
  "label": "Dawn Chorus Type A",
  "description": "High-frequency trill pattern common in spring mornings",
  "method": "hdbscan",
  "params": {
    "min_cluster_size": 10,
    "min_samples": 5,
    "metric": "cosine"
  },
  "statistics": {
    "member_count": 847,
    "unique_recordings": 124,
    "unique_sensors": 8,
    "date_range": {
      "earliest": "2024-03-15",
      "latest": "2025-01-10"
    }
  },
  "prototypes": [
    {
      "id": "proto_001",
      "segment_id": "seg_p1",
      "centroid_distance": 0.05,
      "exemplar_rank": 1
    },
    ...
  ],
  "feature_summary": {
    "pitch_mean": { "mean": 2800, "std": 200 },
    "spectral_centroid": { "mean": 3200, "std": 150 },
    "duration_ms": { "mean": 450, "std": 80 }
  },
  "_links": {
    "self": { "href": "/api/v1/clusters/cls_dawn_chorus_01" },
    "members": { "href": "/api/v1/clusters/cls_dawn_chorus_01/members" },
    "similar_clusters": { "href": "/api/v1/clusters/cls_dawn_chorus_01/similar" },
    "transitions": { "href": "/api/v1/clusters/cls_dawn_chorus_01/transitions" }
  }
}
```

#### 2.4 Cypher Query Endpoint

```yaml
# Execute graph query
POST /api/v1/queries/cypher
Content-Type: application/json

Request:
{
  "query": "MATCH (s:CallSegment)-[:SIMILAR {dist}]->(n:CallSegment) WHERE s.id = $segment_id AND dist < $threshold RETURN n, dist ORDER BY dist LIMIT $limit",
  "parameters": {
    "segment_id": "seg_a1b2c3d4",
    "threshold": 0.3,
    "limit": 50
  },
  "options": {
    "timeout_ms": 5000,
    "include_embeddings": false,
    "explain": false
  }
}

Response: 200 OK
{
  "columns": ["n", "dist"],
  "rows": [
    {
      "n": {
        "id": "seg_e5f6g7h8",
        "recording_id": "rec_9k8j7h6g",
        "start_ms": 45000,
        "snr_db": 22.3
      },
      "dist": 0.12
    },
    ...
  ],
  "metadata": {
    "rows_returned": 50,
    "execution_time_ms": 45,
    "nodes_scanned": 1250,
    "cache_hit": true
  }
}
```

Supported Cypher patterns:

```cypher
-- Find call sequences (motifs)
MATCH path = (s1:CallSegment)-[:NEXT*2..5]->(s2:CallSegment)
WHERE s1.cluster_id = $cluster_id
RETURN path, length(path) as motif_length

-- Find co-occurring calls within time window
MATCH (s1:CallSegment)-[:SIMILAR]->(s2:CallSegment)
WHERE s1.recording_id = s2.recording_id
  AND abs(s1.start_ms - s2.start_ms) < 10000
RETURN s1, s2

-- Temporal pattern analysis
MATCH (r:Recording)-[:HAS_SEGMENT]->(s:CallSegment)-[:ASSIGNED_TO]->(c:Cluster)
WHERE r.metadata.habitat = $habitat
RETURN c.id, count(s) as call_count, collect(DISTINCT r.id) as recordings
ORDER BY call_count DESC
```

#### 2.5 Sequence and Motif Analysis

```yaml
# Get sequence patterns for a segment
GET /api/v1/sequences/{segment_id}/motifs?min_length=3&max_length=8&min_occurrences=5

Response: 200 OK
{
  "segment_id": "seg_a1b2c3d4",
  "motifs": [
    {
      "id": "motif_001",
      "pattern": ["cls_01", "cls_03", "cls_01", "cls_02"],
      "length": 4,
      "occurrences": 23,
      "recordings": ["rec_1", "rec_2", "rec_5"],
      "mean_interval_ms": 1250,
      "interval_regularity": 0.89,
      "examples": [
        {
          "recording_id": "rec_1",
          "start_segment_id": "seg_x1",
          "segments": ["seg_x1", "seg_x2", "seg_x3", "seg_x4"]
        }
      ]
    },
    ...
  ],
  "_links": {
    "self": { "href": "/api/v1/sequences/seg_a1b2c3d4/motifs" }
  }
}
```

```yaml
# Analyze transition probabilities
GET /api/v1/sequences/transitions?cluster_id=cls_01&depth=3

Response: 200 OK
{
  "source_cluster": "cls_01",
  "transitions": {
    "first_order": [
      { "target": "cls_03", "probability": 0.45, "count": 234 },
      { "target": "cls_02", "probability": 0.30, "count": 156 },
      { "target": "cls_01", "probability": 0.15, "count": 78 }
    ],
    "second_order": {
      "cls_01->cls_03": [
        { "target": "cls_01", "probability": 0.52, "count": 122 },
        { "target": "cls_04", "probability": 0.28, "count": 65 }
      ]
    }
  },
  "entropy": {
    "first_order": 1.42,
    "second_order": 1.18
  }
}
```

### 3. GraphQL Schema

```graphql
type Query {
  # Recordings
  recording(id: ID!): Recording
  recordings(
    filter: RecordingFilter
    pagination: PaginationInput
  ): RecordingConnection!

  # Segments
  segment(id: ID!): Segment
  segments(
    filter: SegmentFilter
    pagination: PaginationInput
  ): SegmentConnection!

  # Neighbors and similarity
  neighbors(
    segmentId: ID!
    k: Int = 20
    minSimilarity: Float = 0.5
    crossRecording: Boolean = true
    gnnRerank: Boolean = true
  ): NeighborResult!

  # Clusters
  cluster(id: ID!): Cluster
  clusters(
    filter: ClusterFilter
    pagination: PaginationInput
  ): ClusterConnection!

  # Sequences
  motifs(
    segmentId: ID!
    minLength: Int = 2
    maxLength: Int = 10
    minOccurrences: Int = 3
  ): [Motif!]!

  transitions(
    clusterId: ID!
    depth: Int = 2
  ): TransitionAnalysis!

  # Evidence Pack for RAB
  evidencePack(
    segmentId: ID!
    k: Int = 10
    includeSpectrogram: Boolean = true
  ): EvidencePack!

  # Graph query
  cypher(
    query: String!
    parameters: JSON
  ): CypherResult!
}

type Mutation {
  # Upload recording
  createRecording(input: CreateRecordingInput!): RecordingUploadResult!

  # Manual annotation
  annotateSegment(
    segmentId: ID!
    annotation: AnnotationInput!
  ): Segment!

  # Cluster management
  mergeCluster(
    sourceIds: [ID!]!
    targetLabel: String!
  ): Cluster!

  splitCluster(
    clusterId: ID!
    method: SplitMethod!
  ): [Cluster!]!

  # Batch operations
  batchEmbedRecordings(recordingIds: [ID!]!): BatchJob!
  rebuildNeighborGraph(options: RebuildOptions): BatchJob!
}

type Subscription {
  # Real-time processing updates
  recordingProcessing(recordingId: ID!): ProcessingUpdate!

  # Live monitoring
  segmentDetected(sensorIds: [ID!]): SegmentDetection!

  # Anomaly alerts
  anomalyDetected(
    thresholds: AnomalyThresholds
  ): AnomalyAlert!
}

# Core Types
type Recording {
  id: ID!
  status: ProcessingStatus!
  durationMs: Int!
  sampleRate: Int!
  metadata: RecordingMetadata!
  segments(
    pagination: PaginationInput
  ): SegmentConnection!
  createdAt: DateTime!
  updatedAt: DateTime!
}

type Segment {
  id: ID!
  recording: Recording!
  startMs: Int!
  endMs: Int!
  durationMs: Int!
  snrDb: Float!
  energy: Float!
  embedding: Embedding
  cluster: Cluster
  features: SegmentFeatures!
  neighbors(k: Int = 10): [Neighbor!]!
  next: Segment
  previous: Segment
  spectrogram: SpectrogramData
}

type Embedding {
  id: ID!
  model: String!
  dimension: Int!
  vector: [Float!]  # Only if explicitly requested
  norm: Float!
}

type Cluster {
  id: ID!
  label: String
  method: ClusteringMethod!
  memberCount: Int!
  stability: Float!
  prototypes: [Prototype!]!
  members(pagination: PaginationInput): SegmentConnection!
  featureSummary: FeatureSummary!
  temporalDistribution: TemporalDistribution!
  transitions: [ClusterTransition!]!
}

type Neighbor {
  segment: Segment!
  distance: Float!
  similarity: Float!
  edgeType: EdgeType!
}

type Motif {
  id: ID!
  pattern: [Cluster!]!
  length: Int!
  occurrences: Int!
  meanIntervalMs: Float!
  intervalRegularity: Float!
  examples: [MotifExample!]!
}

type EvidencePack {
  querySegment: Segment!
  neighbors: [Neighbor!]!
  clusterExemplars: [Prototype!]!
  predictions: [TaxonPrediction!]
  sequenceContext: SequenceContext!
  signalQuality: SignalQuality!
  spectrograms: [SpectrogramData!]
}

# Input Types
input RecordingFilter {
  sensorIds: [ID!]
  dateRange: DateRangeInput
  habitat: String
  minDurationMs: Int
  status: ProcessingStatus
}

input SegmentFilter {
  recordingIds: [ID!]
  clusterIds: [ID!]
  minSnrDb: Float
  minEnergy: Float
  dateRange: DateRangeInput
}

input ClusterFilter {
  method: ClusteringMethod
  minMembers: Int
  minStability: Float
  labels: [String!]
}

input PaginationInput {
  cursor: String
  limit: Int = 50
}

input AnnotationInput {
  label: String
  taxonId: ID
  confidence: Float
  notes: String
}

# Enums
enum ProcessingStatus {
  PENDING
  PROCESSING
  COMPLETED
  FAILED
}

enum ClusteringMethod {
  HDBSCAN
  KMEANS
  SPECTRAL
  AGGLOMERATIVE
}

enum EdgeType {
  SIMILAR
  NEXT
  COOCCURRENCE
}

enum SplitMethod {
  KMEANS
  SPECTRAL
  MANUAL
}
```

### 4. Rate Limiting and Quotas

#### 4.1 Rate Limit Tiers

| Tier | Requests/min | Uploads/day | Query Complexity | WebSocket Connections |
|------|-------------|-------------|------------------|----------------------|
| Free | 60 | 10 | 100 nodes | 1 |
| Researcher | 300 | 100 | 10,000 nodes | 5 |
| Institution | 1,000 | 1,000 | 100,000 nodes | 20 |
| Enterprise | Custom | Custom | Custom | Custom |

#### 4.2 Rate Limit Headers

```http
X-RateLimit-Limit: 300
X-RateLimit-Remaining: 245
X-RateLimit-Reset: 1705312800
X-RateLimit-Policy: researcher
Retry-After: 45  # Only on 429 responses
```

#### 4.3 Query Complexity Limits

GraphQL queries are scored based on:
- Number of fields requested
- Depth of nested queries
- Pagination limits
- Inclusion of expensive fields (embeddings, spectrograms)

```json
{
  "complexity": {
    "score": 847,
    "limit": 10000,
    "breakdown": {
      "base": 100,
      "neighbors_k20": 400,
      "spectrogram_include": 200,
      "pagination_50": 147
    }
  }
}
```

### 5. Pagination Strategy

#### 5.1 Cursor-Based Pagination (Primary)

Used for all list endpoints to ensure consistent results during concurrent updates.

```json
{
  "items": [...],
  "pagination": {
    "cursor": "eyJpZCI6InNlZ18xMjM0IiwidHMiOjE3MDUzMTI4MDB9",
    "has_more": true,
    "total_estimate": 15000
  }
}
```

Cursor format: Base64-encoded JSON containing:
- Primary sort field value
- Secondary sort field (timestamp) for stability
- Optional filter hash for validation

#### 5.2 Offset Pagination (Legacy Support)

Available for backward compatibility but discouraged.

```yaml
GET /api/v1/segments?offset=100&limit=50

# Returns warning header
X-Pagination-Warning: Offset pagination may return inconsistent results. Consider cursor-based pagination.
```

#### 5.3 Keyset Pagination for Large Result Sets

For queries returning >100,000 results:

```yaml
GET /api/v1/segments?after_id=seg_abc123&limit=1000

Response includes:
{
  "items": [...],
  "pagination": {
    "last_id": "seg_xyz789",
    "estimated_remaining": 89500,
    "streaming_available": true,
    "_links": {
      "stream": { "href": "/api/v1/segments/stream?after_id=seg_xyz789" }
    }
  }
}
```

### 6. Error Handling and Response Formats

#### 6.1 Error Response Structure

```json
{
  "error": {
    "code": "SEGMENT_NOT_FOUND",
    "message": "Segment with ID 'seg_invalid' does not exist",
    "details": {
      "segment_id": "seg_invalid",
      "suggestion": "Check the segment ID format or use /api/v1/segments to list available segments"
    },
    "request_id": "req_7f8a9b2c3d4e",
    "timestamp": "2025-01-15T10:30:00Z",
    "_links": {
      "documentation": { "href": "https://docs.sevensense.io/errors/SEGMENT_NOT_FOUND" }
    }
  }
}
```

#### 6.2 HTTP Status Code Usage

| Status | Usage |
|--------|-------|
| 200 | Successful GET, PUT, PATCH |
| 201 | Successful POST (resource created) |
| 202 | Accepted for async processing |
| 204 | Successful DELETE |
| 400 | Invalid request syntax or parameters |
| 401 | Missing or invalid authentication |
| 403 | Valid auth but insufficient permissions |
| 404 | Resource not found |
| 409 | Conflict (duplicate, version mismatch) |
| 422 | Validation error (well-formed but invalid) |
| 429 | Rate limit exceeded |
| 500 | Internal server error |
| 502 | Upstream service error (RuVector, Perch) |
| 503 | Service temporarily unavailable |

#### 6.3 Error Codes

```typescript
enum ErrorCode {
  // Authentication/Authorization
  AUTH_REQUIRED = "AUTH_REQUIRED",
  TOKEN_EXPIRED = "TOKEN_EXPIRED",
  INSUFFICIENT_PERMISSIONS = "INSUFFICIENT_PERMISSIONS",

  // Resource errors
  RECORDING_NOT_FOUND = "RECORDING_NOT_FOUND",
  SEGMENT_NOT_FOUND = "SEGMENT_NOT_FOUND",
  CLUSTER_NOT_FOUND = "CLUSTER_NOT_FOUND",

  // Validation errors
  INVALID_AUDIO_FORMAT = "INVALID_AUDIO_FORMAT",
  AUDIO_TOO_SHORT = "AUDIO_TOO_SHORT",
  AUDIO_TOO_LONG = "AUDIO_TOO_LONG",
  INVALID_SAMPLE_RATE = "INVALID_SAMPLE_RATE",
  INVALID_QUERY_SYNTAX = "INVALID_QUERY_SYNTAX",

  // Processing errors
  EMBEDDING_FAILED = "EMBEDDING_FAILED",
  SEGMENTATION_FAILED = "SEGMENTATION_FAILED",
  CLUSTERING_FAILED = "CLUSTERING_FAILED",

  // Rate limiting
  RATE_LIMIT_EXCEEDED = "RATE_LIMIT_EXCEEDED",
  QUOTA_EXCEEDED = "QUOTA_EXCEEDED",
  QUERY_TOO_COMPLEX = "QUERY_TOO_COMPLEX",

  // System errors
  INTERNAL_ERROR = "INTERNAL_ERROR",
  SERVICE_UNAVAILABLE = "SERVICE_UNAVAILABLE",
  UPSTREAM_ERROR = "UPSTREAM_ERROR"
}
```

### 7. OpenAPI Specification Structure

```yaml
openapi: 3.1.0
info:
  title: 7sense Bioacoustics API
  version: 1.0.0
  description: |
    API for bioacoustic analysis using Perch 2.0 embeddings and RuVector graph database.
    Supports audio upload, embedding generation, similarity search, clustering, and sequence analysis.
  contact:
    name: 7sense API Support
    email: api@sevensense.io
  license:
    name: Apache 2.0
    url: https://www.apache.org/licenses/LICENSE-2.0

servers:
  - url: https://api.sevensense.io/v1
    description: Production
  - url: https://staging-api.sevensense.io/v1
    description: Staging
  - url: http://localhost:8080/v1
    description: Local development

tags:
  - name: Recordings
    description: Audio recording management
  - name: Segments
    description: Call segment operations
  - name: Clusters
    description: Discovered call type clusters
  - name: Sequences
    description: Sequence and motif analysis
  - name: Queries
    description: Graph database queries

paths:
  /recordings:
    post:
      tags: [Recordings]
      summary: Upload audio recording
      operationId: createRecording
      # ... full specification

  /segments/{id}/neighbors:
    get:
      tags: [Segments]
      summary: Get similar segments
      operationId: getSegmentNeighbors
      # ... full specification

  /clusters:
    get:
      tags: [Clusters]
      summary: List discovered clusters
      operationId: listClusters
      # ... full specification

  /queries/cypher:
    post:
      tags: [Queries]
      summary: Execute Cypher graph query
      operationId: executeCypherQuery
      # ... full specification

  /sequences/{id}/motifs:
    get:
      tags: [Sequences]
      summary: Get sequence motifs
      operationId: getSequenceMotifs
      # ... full specification

components:
  securitySchemes:
    bearerAuth:
      type: http
      scheme: bearer
      bearerFormat: JWT

    apiKey:
      type: apiKey
      in: header
      name: X-API-Key

  schemas:
    Recording:
      type: object
      properties:
        id:
          type: string
          pattern: "^rec_[a-z0-9]{8}$"
        status:
          $ref: "#/components/schemas/ProcessingStatus"
        # ... full schema

    Segment:
      type: object
      # ... full schema

    Cluster:
      type: object
      # ... full schema

    Error:
      type: object
      required: [error]
      properties:
        error:
          type: object
          required: [code, message, request_id, timestamp]
          properties:
            code:
              type: string
            message:
              type: string
            details:
              type: object
            request_id:
              type: string
            timestamp:
              type: string
              format: date-time

  responses:
    NotFound:
      description: Resource not found
      content:
        application/json:
          schema:
            $ref: "#/components/schemas/Error"

    RateLimited:
      description: Rate limit exceeded
      headers:
        X-RateLimit-Limit:
          schema:
            type: integer
        X-RateLimit-Reset:
          schema:
            type: integer
        Retry-After:
          schema:
            type: integer
      content:
        application/json:
          schema:
            $ref: "#/components/schemas/Error"

security:
  - bearerAuth: []
  - apiKey: []
```

### 8. Backend Service Decomposition

```
+-------------------------------------------------------------------+
|                        API Gateway (Kong/Envoy)                    |
|  - Rate limiting, authentication, routing, request transformation  |
+-------------------------------------------------------------------+
                                    |
        +---------------------------+---------------------------+
        |                           |                           |
+---------------+         +------------------+         +----------------+
|  REST API     |         |   GraphQL API    |         |  WebSocket     |
|  Service      |         |   Service        |         |  Service       |
|  (Axum)       |         |  (async-graphql) |         |  (tokio-tungstenite)|
+---------------+         +------------------+         +----------------+
        |                           |                           |
        +---------------------------+---------------------------+
                                    |
+-------------------------------------------------------------------+
|                     Core Services Layer                            |
+-------------------------------------------------------------------+
|                                                                    |
|  +----------------+  +----------------+  +-------------------+     |
|  | Recording      |  | Segment        |  | Cluster           |     |
|  | Service        |  | Service        |  | Service           |     |
|  | - Upload       |  | - Detection    |  | - HDBSCAN         |     |
|  | - Validation   |  | - Features     |  | - K-means         |     |
|  | - Storage      |  | - HNSW insert  |  | - Prototype mgmt  |     |
|  +----------------+  +----------------+  +-------------------+     |
|                                                                    |
|  +----------------+  +----------------+  +-------------------+     |
|  | Embedding      |  | Query          |  | Sequence          |     |
|  | Service        |  | Service        |  | Service           |     |
|  | - Perch ONNX   |  | - Cypher parse |  | - Motif detection |     |
|  | - Batch embed  |  | - Optimization |  | - Transitions     |     |
|  | - Cache mgmt   |  | - Execution    |  | - Entropy calc    |     |
|  +----------------+  +----------------+  +-------------------+     |
|                                                                    |
+-------------------------------------------------------------------+
                                    |
+-------------------------------------------------------------------+
|                     Data Layer                                     |
+-------------------------------------------------------------------+
|                                                                    |
|  +------------------+  +------------------+  +----------------+    |
|  | RuVector         |  | PostgreSQL       |  | Object Storage |    |
|  | - HNSW index     |  | - Metadata       |  | (S3/MinIO)     |    |
|  | - Graph store    |  | - User data      |  | - Audio files  |    |
|  | - GNN weights    |  | - Audit logs     |  | - Spectrograms |    |
|  +------------------+  +------------------+  +----------------+    |
|                                                                    |
|  +------------------+  +------------------+                        |
|  | Redis            |  | Message Queue    |                        |
|  | - Session cache  |  | (NATS/RabbitMQ)  |                        |
|  | - Rate limits    |  | - Job queue      |                        |
|  | - Query cache    |  | - Events         |                        |
|  +------------------+  +------------------+                        |
|                                                                    |
+-------------------------------------------------------------------+
                                    |
+-------------------------------------------------------------------+
|                     Background Workers                             |
+-------------------------------------------------------------------+
|                                                                    |
|  +----------------+  +----------------+  +-------------------+     |
|  | Audio          |  | Embedding      |  | Clustering        |     |
|  | Processor      |  | Worker         |  | Worker            |     |
|  | - Segmentation |  | - Batch embed  |  | - Periodic recluster|   |
|  | - Feature ext  |  | - Model update |  | - Stability check |     |
|  +----------------+  +----------------+  +-------------------+     |
|                                                                    |
|  +----------------+  +----------------+                            |
|  | Graph          |  | Anomaly        |                            |
|  | Maintenance    |  | Detector       |                            |
|  | - HNSW rebuild |  | - Real-time    |                            |
|  | - Edge update  |  | - Alerting     |                            |
|  +----------------+  +----------------+                            |
|                                                                    |
+-------------------------------------------------------------------+
```

#### 8.1 Service Descriptions

**API Gateway**
- Kong or Envoy for routing, rate limiting, and authentication
- JWT validation and API key management
- Request/response transformation
- Circuit breaker for downstream services

**REST API Service**
- Built with Axum (Rust async web framework)
- Handles CRUD operations for recordings, segments, clusters
- Implements HATEOAS with hypermedia links
- OpenAPI spec generation via `utoipa`

**GraphQL API Service**
- Built with `async-graphql` (Rust)
- Handles complex nested queries
- Query complexity analysis and limits
- DataLoader pattern for N+1 prevention

**WebSocket Service**
- Built with `tokio-tungstenite`
- Real-time processing status updates
- Live segment detection streaming
- Anomaly alerts

**Recording Service**
- Audio validation (format, sample rate, duration)
- Storage management (S3/MinIO)
- Triggers processing pipeline via message queue

**Segment Service**
- Audio segmentation (energy-based, ML-based)
- Feature extraction (SNR, spectral features, pitch)
- HNSW vector insertion
- Neighbor graph maintenance

**Embedding Service**
- Perch 2.0 ONNX inference
- Batch embedding generation
- Embedding cache management
- Model version management

**Cluster Service**
- HDBSCAN, K-means, spectral clustering
- Prototype selection and management
- Cluster stability monitoring
- Automatic relabeling suggestions

**Query Service**
- Cypher query parsing and validation
- Query plan optimization
- Execution against RuVector
- Result caching

**Sequence Service**
- Motif detection algorithms
- Transition probability calculation
- Entropy rate computation
- DTW validation for high-precision subset

## Consequences

### Positive

1. **Flexibility**: Multiple API protocols support diverse client needs
2. **Scalability**: Service decomposition allows independent scaling
3. **Performance**: HNSW + GNN provides sub-50ms neighbor queries
4. **Discoverability**: HATEOAS and GraphQL introspection aid API exploration
5. **Reliability**: Rate limiting and circuit breakers prevent cascade failures

### Negative

1. **Complexity**: Multiple protocols increase maintenance burden
2. **Consistency**: Must ensure REST, GraphQL, and WebSocket return consistent data
3. **Latency**: Service decomposition adds network hops
4. **Learning curve**: Teams must understand multiple query paradigms

### Mitigations

- Shared service layer ensures data consistency across protocols
- Internal gRPC for low-latency inter-service communication
- Comprehensive API documentation and SDKs
- Monitoring and distributed tracing (Jaeger/Zipkin)

## References

- [RuVector GitHub](https://github.com/ruvnet/ruvector) - Graph database with HNSW and GNN
- [Perch 2.0 Paper](https://arxiv.org/abs/2508.04665) - Embedding model architecture
- [HATEOAS](https://en.wikipedia.org/wiki/HATEOAS) - Hypermedia design principle
- [GraphQL Best Practices](https://graphql.org/learn/best-practices/)
- [OpenAPI Specification 3.1](https://spec.openapis.org/oas/v3.1.0)
