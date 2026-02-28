# RVF Operations API

## 1. Scope

This document specifies the operational surface of an RVF runtime: error codes
returned by all operations, wire formats for batch queries, batch ingest, and
batch deletes, the network streaming protocol for progressive loading over HTTP
and TCP, and the compaction scheduling policy. It complements the segment model
(spec 01), manifest system (spec 02), and query optimization (spec 06).

All multi-byte integers are little-endian unless otherwise noted. All offsets
within messages are byte offsets from the start of the message payload.

## 2. Error Code Enumeration

Error codes are 16-bit unsigned integers. The high byte identifies the error
category; the low byte identifies the specific error within that category.
Implementations must preserve unrecognized codes in responses and must not
treat unknown codes as fatal unless the high byte is `0x01` (format error).

### Category 0x00: Success

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0000  OK                    Operation succeeded
0x0001  OK_PARTIAL            Partial success (some items failed)
```

`OK_PARTIAL` is returned when a batch operation succeeds for some items and
fails for others. The response body contains per-item status details.

### Category 0x01: Format Errors

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0100  INVALID_MAGIC         Segment magic mismatch (expected 0x52564653)
0x0101  INVALID_VERSION       Unsupported segment version
0x0102  INVALID_CHECKSUM      Segment hash verification failed
0x0103  INVALID_SIGNATURE     Cryptographic signature invalid
0x0104  TRUNCATED_SEGMENT     Segment payload shorter than declared length
0x0105  INVALID_MANIFEST      Root manifest validation failed
0x0106  MANIFEST_NOT_FOUND    No valid MANIFEST_SEG in file
0x0107  UNKNOWN_SEGMENT_TYPE  Segment type not recognized (warning, not fatal)
0x0108  ALIGNMENT_ERROR       Data not at expected 64B boundary
```

`UNKNOWN_SEGMENT_TYPE` is advisory. A reader encountering an unknown segment
type should skip it and continue. All other format errors in this category
are fatal for the affected segment.

### Category 0x02: Query Errors

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0200  DIMENSION_MISMATCH    Query vector dimension != index dimension
0x0201  EMPTY_INDEX           No index segments available
0x0202  METRIC_UNSUPPORTED    Requested distance metric not available
0x0203  FILTER_PARSE_ERROR    Invalid filter expression
0x0204  K_TOO_LARGE           Requested K exceeds available vectors
0x0205  TIMEOUT               Query exceeded time budget
```

When `K_TOO_LARGE` is returned, the response still contains all available
results. The result count will be less than the requested K.

### Category 0x03: Write Errors

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0300  LOCK_HELD             Another writer holds the lock
0x0301  LOCK_STALE            Lock file exists but owner process is dead
0x0302  DISK_FULL             Insufficient space for write
0x0303  FSYNC_FAILED          Durable write failed
0x0304  SEGMENT_TOO_LARGE     Segment exceeds 4 GB limit
0x0305  READ_ONLY             File opened in read-only mode
```

`LOCK_STALE` is informational. The runtime may attempt to break the stale
lock and retry. If recovery succeeds, the original operation proceeds with
an `OK` status.

### Category 0x04: Tile Errors (WASM Microkernel)

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0400  TILE_TRAP             WASM trap (OOB, unreachable, stack overflow)
0x0401  TILE_OOM              Tile exceeded scratch memory (64 KB)
0x0402  TILE_TIMEOUT          Tile computation exceeded time budget
0x0403  TILE_INVALID_MSG      Malformed hub-tile message
0x0404  TILE_UNSUPPORTED_OP   Operation not available on this profile
```

All tile errors trigger the fault isolation protocol described in
`microkernel/wasm-runtime.md` section 8. The hub reassigns the tile's
work and optionally restarts the faulted tile.

### Category 0x05: Crypto Errors

```
Code    Name                  Description
------  --------------------  ----------------------------------------
0x0500  KEY_NOT_FOUND         Referenced key_id not in CRYPTO_SEG
0x0501  KEY_EXPIRED           Key past valid_until timestamp
0x0502  DECRYPT_FAILED        Decryption or auth tag verification failed
0x0503  ALGO_UNSUPPORTED      Cryptographic algorithm not implemented
```

Crypto errors are always fatal for the affected segment. An implementation
must not serve data from a segment that fails signature or decryption checks.

## 3. Batch Query API

### Wire Format: Request

Batch queries amortize connection overhead and enable the runtime to
schedule vector block loads across multiple queries simultaneously.

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       query_count         Number of queries in batch (max 1024)
0x04    4       k                   Shared top-K parameter
0x08    1       metric              Distance metric: 0=L2, 1=IP, 2=cosine, 3=hamming
0x09    3       reserved            Must be zero
0x0C    4       ef_search           HNSW ef_search parameter
0x10    4       shared_filter_len   Byte length of shared filter (0 = no filter)
0x14    var     shared_filter       Filter expression (applies to all queries)
var     var     queries[]           Per-query entries (see below)
```

Each query entry:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       query_id            Client-assigned correlation ID
0x04    2       dim                 Vector dimensionality
0x06    1       dtype               Data type: 0=fp32, 1=fp16, 2=i8, 3=binary
0x07    1       flags               Bit 0: has per-query filter
0x08    var     vector              Query vector (dim * sizeof(dtype) bytes)
var     4       filter_len          Byte length of per-query filter (if flags bit 0)
var     var     filter              Per-query filter (overrides shared filter)
```

When both a shared filter and a per-query filter are present, the per-query
filter takes precedence. A per-query filter of zero length inherits the
shared filter.

### Wire Format: Response

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       query_count         Number of query results
0x04    var     results[]           Per-query result entries
```

Each result entry:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       query_id            Correlation ID from request
0x04    2       status              Error code (0x0000 = OK)
0x06    2       reserved            Must be zero
0x08    4       result_count        Number of results returned
0x0C    var     results[]           Array of (vector_id: u64, distance: f32) pairs
```

Each result pair is 12 bytes: 8 bytes for the vector ID followed by 4 bytes
for the distance value. Results are sorted by distance ascending (nearest first).

### Batch Scheduling

The runtime should process batch queries using the following strategy:

1. Parse all query vectors and load them into memory
2. Identify shared segments across queries (block deduplication)
3. Load each vector block once and evaluate all relevant queries against it
4. Merge per-query top-K heaps independently
5. Return results as soon as each query completes (streaming response)

This amortizes I/O: if N queries touch the same vector block, the block is
read once instead of N times.

## 4. Batch Ingest API

### Wire Format: Request

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       vector_count        Number of vectors to ingest (max 65536)
0x04    2       dim                 Vector dimensionality
0x06    1       dtype               Data type: 0=fp32, 1=fp16, 2=i8, 3=binary
0x07    1       flags               Bit 0: metadata_included
0x08    var     vectors[]           Vector entries
var     var     metadata[]          Metadata entries (if flags bit 0)
```

Each vector entry:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    8       vector_id           Globally unique vector ID
0x08    var     vector              Vector data (dim * sizeof(dtype) bytes)
```

Each metadata entry (when metadata_included is set):

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    2       field_count         Number of metadata fields
0x02    var     fields[]            Field entries
```

Each metadata field:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    2       field_id            Field identifier (application-defined)
0x02    1       value_type          0=u64, 1=i64, 2=f64, 3=string, 4=bytes
0x03    var     value               Encoded value (u64/i64/f64: 8B; string/bytes: 4B length + data)
```

### Wire Format: Response

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       accepted_count      Number of vectors accepted
0x04    4       rejected_count      Number of vectors rejected
0x08    4       manifest_epoch      Epoch of manifest after commit
0x0C    var     rejected_ids[]      Array of rejected vector IDs (u64 * rejected_count)
var     var     rejected_reasons[]  Array of error codes (u16 * rejected_count)
```

The `manifest_epoch` field is the epoch of the MANIFEST_SEG written after the
ingest is committed. Clients can use this value to confirm that a subsequent
read will include the ingested vectors.

### Ingest Commit Semantics

1. The runtime writes vectors to a new VEC_SEG (append-only)
2. If metadata is included, a META_SEG is appended
3. Both segments are fsynced
4. A new MANIFEST_SEG is written referencing the new segments
5. The manifest is fsynced
6. The response is sent with the new manifest_epoch

Vectors are visible to queries only after step 6 completes.

## 5. Batch Delete API

### Wire Format: Request

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    1       delete_type         0=by_id, 1=by_range, 2=by_filter
0x01    3       reserved            Must be zero
0x04    var     payload             Type-specific payload (see below)
```

Delete by ID (`delete_type = 0`):

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       count               Number of IDs to delete
0x04    var     ids[]               Array of vector IDs (u64 * count)
```

Delete by range (`delete_type = 1`):

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    8       start_id            Start of range (inclusive)
0x08    8       end_id              End of range (exclusive)
```

Delete by filter (`delete_type = 2`):

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       filter_len          Byte length of filter expression
0x04    var     filter              Filter expression
```

### Wire Format: Response

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    8       deleted_count       Number of vectors deleted
0x08    2       status              Error code (0x0000 = OK)
0x0A    2       reserved            Must be zero
0x0C    4       manifest_epoch      Epoch of manifest after delete committed
```

### Delete Mechanics

Deletes are logical. The runtime appends a JOURNAL_SEG containing tombstone
entries for the deleted vector IDs. The new MANIFEST_SEG marks affected
VEC_SEGs as partially dead. Physical reclamation happens during compaction.

## 6. Network Streaming Protocol

### 6.1 HTTP Range Requests (Read-Only Access)

RVF's progressive loading model maps naturally to HTTP byte-range requests.
A client can boot from a remote `.rvf` file and become queryable without
downloading the entire file.

**Phase 1: Boot (mandatory)**

```
GET /file.rvf  Range: bytes=-4096
```

Retrieves the last 4 KB of the file. This contains the Level 0 root manifest
(MANIFEST_SEG). The client parses hotset pointers, the segment directory, and
the profile ID.

If the file is smaller than 4 KB, the entire file is returned. If the last
4 KB does not contain a valid MANIFEST_SEG, the client extends the range
backward in 4 KB increments until one is found or 1 MB is scanned (at which
point it returns `MANIFEST_NOT_FOUND`).

**Phase 2: Hotset (parallel, mandatory for queries)**

Using offsets from the Level 0 manifest, the client issues up to 5 parallel
range requests:

```
GET /file.rvf  Range: bytes=<entrypoint_offset>-<entrypoint_end>
GET /file.rvf  Range: bytes=<toplayer_offset>-<toplayer_end>
GET /file.rvf  Range: bytes=<centroid_offset>-<centroid_end>
GET /file.rvf  Range: bytes=<quantdict_offset>-<quantdict_end>
GET /file.rvf  Range: bytes=<hotcache_offset>-<hotcache_end>
```

These fetch the HNSW entry point, top-layer graph, routing centroids,
quantization dictionary, and the hot cache (HOT_SEG). After these 5 requests
complete, the system is queryable with recall >= 0.7.

**Phase 3: Level 1 (background)**

```
GET /file.rvf  Range: bytes=<l1_offset>-<l1_end>
```

Fetches the Level 1 manifest containing the full segment directory. This
enables the client to discover all segments and plan on-demand fetches.

**Phase 4: On-demand (per query)**

For queries that require cold data not yet fetched:

```
GET /file.rvf  Range: bytes=<segment_offset>-<segment_end>
```

The client caches fetched segments locally. Repeated queries against the
same data region do not trigger additional requests.

### HTTP Requirements

- Server must support `Accept-Ranges: bytes`
- Server must return `206 Partial Content` for range requests
- Server should support multiple ranges in a single request (`multipart/byteranges`)
- Client should use `If-None-Match` with the file's ETag to detect stale caches

### 6.2 TCP Streaming Protocol (Real-Time Access)

For real-time ingest and low-latency queries, RVF defines a binary TCP
protocol over TLS 1.3.

**Connection Setup**

```
1. Client opens TCP connection to server
2. TLS 1.3 handshake (mandatory, no plaintext mode)
3. Client sends HELLO message with protocol version and capabilities
4. Server responds with HELLO_ACK confirming capabilities
5. Connection is ready for messages
```

**Framing**

All messages are length-prefixed:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       frame_length        Payload length (big-endian, max 16 MB)
0x04    1       msg_type            Message type (see below)
0x05    3       msg_id              Correlation ID (big-endian, wraps at 2^24)
0x08    var     payload             Message-specific payload
```

Frame length is big-endian (network byte order) for consistency with TLS
framing. The 16 MB maximum prevents a single message from monopolizing the
connection. Payloads larger than 16 MB must be split across multiple messages
using continuation framing (see section 6.4).

**Message Types**

```
Client -> Server:
  0x01  QUERY           Batch query (payload = Batch Query Request)
  0x02  INGEST          Batch ingest (payload = Batch Ingest Request)
  0x03  DELETE          Batch delete (payload = Batch Delete Request)
  0x04  STATUS          Request server status (no payload)
  0x05  SUBSCRIBE       Subscribe to update notifications

Server -> Client:
  0x81  QUERY_RESULT    Batch query result
  0x82  INGEST_ACK      Batch ingest acknowledgment
  0x83  DELETE_ACK      Batch delete acknowledgment
  0x84  STATUS_RESP     Server status response
  0x85  UPDATE_NOTIFY   Push notification of new data
  0xFF  ERROR           Error with code and description
```

**ERROR Message Payload**

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    2       error_code          Error code from section 2
0x02    2       description_len     Byte length of description string
0x04    var     description         UTF-8 error description (human-readable)
```

### 6.3 Streaming Ingest Protocol

The TCP protocol supports continuous ingest where the client streams vectors
without waiting for per-batch acknowledgments.

**Flow**

```
Client                              Server
  |                                    |
  |--- INGEST (batch 0) ------------->|
  |--- INGEST (batch 1) ------------->|  Pipelining: send without waiting
  |--- INGEST (batch 2) ------------->|
  |                                    | Server writes VEC_SEGs, appends manifest
  |<--- INGEST_ACK (batch 0) ---------|
  |<--- INGEST_ACK (batch 1) ---------|
  |                                    | Backpressure: server delays ACK
  |--- INGEST (batch 3) ------------->|  Client respects window
  |<--- INGEST_ACK (batch 2) ---------|
  |                                    |
```

**Backpressure**

The server controls ingest rate by delaying INGEST_ACK responses. The client
must limit its in-flight (unacknowledged) ingest messages to a configurable
window size (default: 8 messages). When the window is full, the client must
wait for an ACK before sending the next batch.

The server should send backpressure when:
- Write queue exceeds 80% capacity
- Compaction is falling behind (dead space > 50%)
- Available disk space drops below 10%

**Commit Semantics**

Each INGEST_ACK contains the `manifest_epoch` after commit. The server
guarantees that all vectors acknowledged with epoch E are visible to any
query that reads the manifest at epoch >= E.

### 6.4 Continuation Framing

For payloads exceeding the 16 MB frame limit:

```
Frame 0: msg_type = original type, flags bit 0 = CONTINUATION_START
Frame 1: msg_type = 0x00 (CONTINUATION), flags bit 0 = 0
Frame 2: msg_type = 0x00 (CONTINUATION), flags bit 0 = 0
Frame N: msg_type = 0x00 (CONTINUATION), flags bit 1 = CONTINUATION_END
```

The receiver reassembles the payload from all continuation frames before
processing. The msg_id is shared across all frames of a continuation sequence.

### 6.5 SUBSCRIBE and UPDATE_NOTIFY

The SUBSCRIBE message registers the client for push notifications when new
data is committed:

```
SUBSCRIBE payload:
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       min_epoch           Only notify for epochs > this value
0x04    1       notify_flags        Bit 0: ingest, Bit 1: delete, Bit 2: compaction
0x05    3       reserved            Must be zero
```

The server sends UPDATE_NOTIFY whenever a new MANIFEST_SEG is committed that
matches the subscription criteria:

```
UPDATE_NOTIFY payload:
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    4       epoch               New manifest epoch
0x04    1       event_type          0=ingest, 1=delete, 2=compaction
0x05    3       reserved            Must be zero
0x08    4       affected_count      Number of vectors affected
0x0C    8       new_total           Total vector count after event
```

## 7. Compaction Scheduling Policy

Compaction merges small, overlapping, or partially-dead segments into larger,
sealed segments. Because compaction competes with queries and ingest for I/O
bandwidth, the runtime enforces a scheduling policy.

### 7.1 IO Budget

Compaction must consume at most 30% of available IOPS. The runtime measures
IOPS over a 5-second sliding window and throttles compaction I/O to stay
within budget.

```
available_iops = measured_iops_capacity (from benchmarking at startup)
compaction_budget = available_iops * 0.30
compaction_throttle = max(compaction_budget - current_compaction_iops, 0)
```

### 7.2 Priority Ordering

When I/O bandwidth is contended, operations are prioritized:

```
Priority 1 (highest):  Queries (reads from VEC_SEG, INDEX_SEG, HOT_SEG)
Priority 2:            Ingest (writes to VEC_SEG, META_SEG, MANIFEST_SEG)
Priority 3 (lowest):   Compaction (reads + writes of sealed segments)
```

Compaction yields to queries and ingest. If a compaction I/O operation would
cause a query to exceed its time budget, the compaction operation is deferred.

### 7.3 Scheduling Triggers

Compaction runs when all of the following conditions are met:

| Condition | Threshold | Rationale |
|-----------|-----------|-----------|
| Query load | < 50% of capacity | Avoid competing with active queries |
| Dead space ratio | > 20% of total file size | Not worth compacting small amounts |
| Segment count | > 32 active segments | Many small segments hurt read performance |
| Time since last compaction | > 60 seconds | Prevent compaction storms |

The runtime evaluates these conditions every 10 seconds.

### 7.4 Emergency Compaction

If dead space exceeds 70% of total file size, compaction enters emergency mode:

```
Emergency compaction rules:
  1. Compaction preempts ingest (ingest is paused, not rejected)
  2. IO budget increases to 60% of available IOPS
  3. Compaction runs regardless of query load
  4. Ingest resumes after dead space drops below 50%
```

During emergency compaction, the server responds to INGEST messages with
delayed ACKs (backpressure) rather than rejecting them. Queries continue to
be served at highest priority.

### 7.5 Compaction Progress Reporting

The STATUS response includes compaction state:

```
STATUS_RESP compaction fields:
Offset  Size    Field                 Description
------  ------  -------------------   ----------------------------------------
0x00    1       compaction_state      0=idle, 1=running, 2=emergency
0x01    1       progress_pct          Completion percentage (0-100)
0x02    2       reserved              Must be zero
0x04    8       dead_bytes            Total dead space in bytes
0x0C    8       total_bytes           Total file size in bytes
0x14    4       segments_remaining    Segments left to compact
0x18    4       segments_completed    Segments compacted in current run
0x1C    4       estimated_seconds     Estimated time to completion
0x20    4       io_budget_pct         Current IO budget percentage (30 or 60)
```

### 7.6 Compaction Segment Selection

The runtime selects segments for compaction using a tiered strategy:

```
1. Tombstoned segments:       Always compacted first (reclaim dead space)
2. Small VEC_SEGs:            Segments < 1 MB merged into larger segments
3. High-overlap INDEX_SEGs:   Index segments covering the same ID range
4. Cold OVERLAY_SEGs:         Overlay deltas merged into base segments
```

The compaction output is always a sealed segment (SEALED flag set). Sealed
segments are immutable and can be verified independently.

## 8. STATUS Response Format

The STATUS message provides a snapshot of the server state for monitoring
and diagnostics.

```
STATUS_RESP payload:
Offset  Size    Field                 Description
------  ------  -------------------   ----------------------------------------
0x00    4       protocol_version      Protocol version (currently 1)
0x04    4       manifest_epoch        Current manifest epoch
0x08    8       total_vectors         Total vector count
0x10    8       total_segments        Total segment count
0x18    8       file_size_bytes       Total file size
0x20    4       query_qps             Queries per second (last 5s window)
0x24    4       ingest_vps            Vectors ingested per second (last 5s window)
0x28    24      compaction            Compaction state (see section 7.5)
0x40    1       profile_id            Active hardware profile (0x00-0x03)
0x41    1       health                0=healthy, 1=degraded, 2=read_only
0x42    2       reserved              Must be zero
0x44    4       uptime_seconds        Server uptime
```

## 9. Filter Expression Format

Filter expressions used in batch queries and batch deletes share a common
binary encoding:

```
Offset  Size    Field               Description
------  ------  ------------------  ----------------------------------------
0x00    1       op                  Operator enum (see below)
0x01    2       field_id            Metadata field to filter on
0x03    1       value_type          Value type (matches metadata field types)
0x04    var     value               Comparison value
var     var     children[]          Sub-expressions (for AND/OR/NOT)
```

Operator enum:

```
0x00  EQ          field == value
0x01  NE          field != value
0x02  LT          field < value
0x03  LE          field <= value
0x04  GT          field > value
0x05  GE          field >= value
0x06  IN          field in [values]
0x07  RANGE       field in [low, high)
0x10  AND         All children must match
0x11  OR          Any child must match
0x12  NOT         Negate single child
```

Filters are evaluated during the query scan phase. Vectors that do not match
the filter are excluded from distance computation entirely (pre-filtering) or
from the result set (post-filtering), depending on the runtime's cost model.

## 10. Invariants

1. Error codes are stable across versions; new codes are additive only
2. Batch operations are atomic per-item, not per-batch (partial success is valid)
3. TCP connections are always TLS 1.3; plaintext is not permitted
4. Frame length is big-endian; all other multi-byte fields are little-endian
5. HTTP progressive loading must succeed with at most 7 round trips to become queryable
6. Compaction never runs at more than 60% of available IOPS, even in emergency mode
7. The STATUS response is always available, even during emergency compaction
8. Filter expressions are limited to 64 levels of nesting depth
