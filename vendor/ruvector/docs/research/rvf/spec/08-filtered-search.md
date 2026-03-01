# RVF Filtered Search

## 1. Motivation

Domain profiles declare metadata schemas with indexed fields (e.g., `"organism"` in
RVDNA, `"language"` in RVText, `"node_type"` in RVGraph), but the format provides no
specification for how those indexes are built, stored, or evaluated at query time.

Filtered search is the combination of vector similarity search with metadata
predicates. Without it, a caller must retrieve an over-sized result set and filter
client-side — wasting bandwidth, latency, and recall budget.

This specification adds:

1. **META_SEG** payload layout (segment type 0x07) for storing per-vector metadata
2. **Filter expression language** with a compact binary encoding
3. **Three evaluation strategies** (pre-, post-, and intra-filtering)
4. **METAIDX_SEG** (new segment type 0x0D) for inverted and bitmap indexes
5. **Manifest integration** via a new Level 1 TLV record
6. **Temperature tier coordination** for metadata segments

## 2. META_SEG Payload Layout (Segment Type 0x07)

META_SEG stores the actual metadata values associated with vectors. It uses the
standard 64-byte segment header (see `binary-layout.md` Section 3) with
`seg_type = 0x07`.

```
META_SEG Payload:

+------------------------------------------+
| Meta Header (64 bytes, padded)           |
|   schema_id:            u32              |  References PROFILE_SEG schema
|   vector_id_range_start: u64             |  First vector ID covered
|   vector_id_range_end:   u64             |  Last vector ID covered (inclusive)
|   field_count:           u16             |  Number of fields in this segment
|   encoding:              u8              |  0 = row-oriented, 1 = column-oriented
|   reserved:              [u8; 37]        |  Must be zero
|   [64B aligned]                          |
+------------------------------------------+
| Field Directory                          |
|   For each field (field_count entries):  |
|     field_id:       u16                  |
|     field_type:     u8                   |
|     flags:          u8                   |
|     field_offset:   u32                  |  Byte offset from payload start
|   [64B aligned]                          |
+------------------------------------------+
| Field Data (column-oriented)             |
|   (see Section 2.1 for per-type layout)  |
+------------------------------------------+
```

### Field Type Enum

```
Value   Type      Wire Size          Description
-----   ----      ---------          -----------
0x00    string    Variable           UTF-8, dictionary-encoded in column layout
0x01    u32       4 bytes            Unsigned 32-bit integer
0x02    u64       8 bytes            Unsigned 64-bit integer
0x03    f32       4 bytes            IEEE 754 single-precision float
0x04    enum      Variable (packed)  Enumeration with defined label set
0x05    bool      1 bit (packed)     Boolean
```

### Field Flags

```
Bit  Mask  Name       Meaning
---  ----  ----       -------
0    0x01  INDEXED    Field has a corresponding METAIDX_SEG
1    0x02  SORTED     Values are stored in sorted order
2    0x04  NULLABLE   Null bitmap present before values
3    0x08  STORED     Field value returned in query results (not just filterable)
4-7        reserved   Must be zero
```

### 2.1 Column-Oriented Field Layouts

Column-oriented encoding (encoding = 1) is the preferred layout. Each field's data
block starts at a 64-byte aligned boundary.

**String fields** (dictionary-encoded):

```
dict_size:    u32                           Number of distinct strings
For each dict entry:
  length:     u16                           Byte length of UTF-8 string
  bytes:      [u8; length]                  UTF-8 encoded string
[4B aligned after dictionary]
codes:        [varint; vector_count]        Dictionary code per vector
[64B aligned]
```

Dictionary codes are 0-indexed into the dictionary array. Code `0xFFFFFFFF` (max
varint value for u32 range) represents null if the NULLABLE flag is set.

**Numeric fields** (u32, u64, f32 -- direct array):

```
If NULLABLE:
  null_bitmap: [u8; ceil(vector_count / 8)]  Bit-packed, 1 = present, 0 = null
  [8B aligned]
values:       [field_type; vector_count]     Dense array of values
[64B aligned]
```

Values for null entries are zero-filled but must not be relied upon.

**Enum fields** (bit-packed):

```
enum_count:   u8                            Number of enum labels
For each enum label:
  length:     u8                            Byte length of label
  bytes:      [u8; length]                  UTF-8 label string
bits_per_code: u8                           ceil(log2(enum_count))
codes:        packed bit array              bits_per_code bits per vector
              [ceil(vector_count * bits_per_code / 8) bytes]
[64B aligned]
```

For example, an enum with 3 values (`"+", "-", "."`) uses 2 bits per vector.
1M vectors = 250 KB.

**Bool fields** (bit-packed):

```
If NULLABLE:
  null_bitmap: [u8; ceil(vector_count / 8)]
  [8B aligned]
values:       [u8; ceil(vector_count / 8)]  Bit-packed, 1 = true, 0 = false
[64B aligned]
```

### 2.2 Sorted Index (Inline)

For fields with the SORTED flag, an additional sorted permutation index follows
the field data:

```
sorted_count:   u32                         Must equal vector_count
sorted_order:   [varint delta-encoded]      Vector IDs in ascending value order
restart_interval: u16                       Restart every N entries (default 128)
restart_offsets:  [u32; ceil(sorted_count / restart_interval)]
[64B aligned]
```

This enables binary search over field values for range queries without requiring
a separate METAIDX_SEG. It is suitable for fields where a full inverted index
would be wasteful (high cardinality numeric fields like `position_start`).

## 3. Filter Expression Language

### 3.1 Abstract Syntax

A filter expression is a tree of predicates combined with boolean logic:

```
expr ::= field_ref CMP literal         -- comparison
       | field_ref IN literal_set       -- set membership
       | field_ref PREFIX string_lit    -- string prefix match
       | field_ref CONTAINS string_lit  -- substring containment
       | expr AND expr                  -- conjunction
       | expr OR expr                   -- disjunction
       | NOT expr                       -- negation
```

### 3.2 Binary Encoding (Postfix / RPN)

Filter expressions are encoded as a postfix (Reverse Polish Notation) token stream
for stack-based evaluation. This avoids the need for recursive parsing and enables
single-pass evaluation with a fixed-size stack.

```
Filter Expression Binary Layout:

header:
  node_count:     u16                   Total number of tokens
  stack_depth:    u8                    Maximum stack depth required
  reserved:       u8                    Must be zero

tokens (postfix order):
  For each token:
    node_type:    u8                    Token type (see enum below)
    payload:      type-specific         Variable-size payload
```

### Token Type Enum

```
Value   Name        Stack Effect   Payload
-----   ----        ------------   -------
0x01    FIELD_REF   push +1        field_id: u16
0x02    LIT_U32     push +1        value: u32
0x03    LIT_U64     push +1        value: u64
0x04    LIT_F32     push +1        value: f32
0x05    LIT_STR     push +1        length: u16, bytes: [u8; length]
0x06    LIT_BOOL    push +1        value: u8 (0 or 1)
0x07    LIT_NULL    push +1        (no payload)

0x10    CMP_EQ      pop 2, push 1  (no payload) -- a == b
0x11    CMP_NE      pop 2, push 1  (no payload) -- a != b
0x12    CMP_LT      pop 2, push 1  (no payload) -- a < b
0x13    CMP_LE      pop 2, push 1  (no payload) -- a <= b
0x14    CMP_GT      pop 2, push 1  (no payload) -- a > b
0x15    CMP_GE      pop 2, push 1  (no payload) -- a >= b

0x20    IN_SET      pop 1, push 1  set_size: u16, [encoded values]
0x21    PREFIX      pop 2, push 1  (no payload) -- string prefix
0x22    CONTAINS    pop 2, push 1  (no payload) -- substring match

0x30    AND         pop 2, push 1  (no payload)
0x31    OR          pop 2, push 1  (no payload)
0x32    NOT         pop 1, push 1  (no payload)
```

### 3.3 Encoding Example

Filter: `organism = "E. coli" AND position_start >= 1000`

```
Token 0: FIELD_REF   field_id=0 (organism)          stack: [organism_val]
Token 1: LIT_STR     "E. coli"                      stack: [organism_val, "E. coli"]
Token 2: CMP_EQ                                     stack: [true/false]
Token 3: FIELD_REF   field_id=3 (position_start)    stack: [bool, pos_val]
Token 4: LIT_U64     1000                           stack: [bool, pos_val, 1000]
Token 5: CMP_GE                                     stack: [bool, true/false]
Token 6: AND                                        stack: [result]

Binary: node_count=7, stack_depth=3
  01 00:00  05 00:07 "E. coli"  10  01 00:03  03 00:00:00:00:00:00:03:E8  15  30
```

### 3.4 Evaluation

Evaluation processes tokens left to right using a fixed-size boolean/value stack:

```python
def evaluate(tokens, vector_id, metadata):
    stack = []
    for token in tokens:
        if token.type == FIELD_REF:
            stack.append(metadata.get_value(vector_id, token.field_id))
        elif token.type in (LIT_U32, LIT_U64, LIT_F32, LIT_STR, LIT_BOOL, LIT_NULL):
            stack.append(token.value)
        elif token.type in (CMP_EQ, CMP_NE, CMP_LT, CMP_LE, CMP_GT, CMP_GE):
            b, a = stack.pop(), stack.pop()
            stack.append(compare(a, token.type, b))
        elif token.type == IN_SET:
            a = stack.pop()
            stack.append(a in token.value_set)
        elif token.type in (PREFIX, CONTAINS):
            b, a = stack.pop(), stack.pop()
            stack.append(string_match(a, token.type, b))
        elif token.type == AND:
            b, a = stack.pop(), stack.pop()
            stack.append(a and b)
        elif token.type == OR:
            b, a = stack.pop(), stack.pop()
            stack.append(a or b)
        elif token.type == NOT:
            stack.append(not stack.pop())
    return stack[0]
```

Maximum stack depth is declared in the header so the evaluator can pre-allocate.
Implementations must reject expressions with `stack_depth > 16`.

## 4. Filter Evaluation Strategies

The runtime selects one of three strategies based on the estimated **selectivity**
of the filter (the fraction of vectors passing the filter).

### 4.1 Pre-Filtering (Selectivity < 1%)

Build the candidate ID set from metadata indexes first, then run vector search
only on the filtered subset.

```
1. Evaluate filter using METAIDX_SEG inverted/bitmap indexes
2. Collect matching vector IDs into a candidate set C
3. If |C| < ef_search:
     Flat scan all candidates, return top-K
   Else:
     Build temporary flat index over C, run HNSW search restricted to C
4. Return top-K results
```

**Tradeoffs**:
- Optimal when the candidate set is very small (hundreds to low thousands)
- Risk: if the candidate set is disconnected in the HNSW graph, search cannot
  traverse from entry points to candidates. The flat scan fallback handles this.
- Memory: candidate set bitmap = `ceil(total_vectors / 8)` bytes

### 4.2 Post-Filtering (Selectivity > 20%)

Run standard HNSW search with over-retrieval, then filter results.

```
1. Compute over_retrieval_factor = min(1.0 / selectivity, 10.0)
2. Set ef_search_adj = ef_search * over_retrieval_factor
3. Run standard HNSW search with ef_search_adj
4. Filter result set by evaluating filter expression per candidate
5. Return top-K from filtered results
```

**Tradeoffs**:
- Optimal when the filter passes most vectors (minimal wasted computation)
- Risk: if over-retrieval factor is too low, fewer than K results survive filtering.
  The caller should retry with a higher factor or fall back to intra-filtering.
- No modification to HNSW traversal logic required.

### 4.3 Intra-Filtering (1% <= Selectivity <= 20%)

Evaluate the filter during HNSW traversal, skipping nodes that fail the predicate.

```python
def filtered_hnsw_search(query, filter_expr, entry_point, ef_search, k):
    candidates = MaxHeap()       # top-K results (max-heap by distance)
    worklist = MinHeap()         # exploration frontier (min-heap by distance)
    visited = BitSet()
    filtered_skips = 0
    max_skips = ef_search * 3    # backoff threshold

    worklist.push((distance(query, entry_point), entry_point))
    visited.add(entry_point)

    while worklist and filtered_skips < max_skips:
        dist, node = worklist.pop()

        # Check filter predicate
        if not evaluate(filter_expr, node, metadata):
            filtered_skips += 1
            # Still expand neighbors (maintain graph connectivity)
            neighbors = get_neighbors(node)
            for n in neighbors:
                if n not in visited:
                    visited.add(n)
                    d = distance(query, get_vector(n))
                    worklist.push((d, n))
            continue

        filtered_skips = 0  # reset skip counter on successful match
        candidates.push((dist, node))
        if len(candidates) > k:
            candidates.pop()  # evict worst

        # Expand neighbors
        neighbors = get_neighbors(node)
        for n in neighbors:
            if n not in visited:
                visited.add(n)
                d = distance(query, get_vector(n))
                if len(candidates) < ef_search or d < candidates.max():
                    worklist.push((d, n))

    return candidates.top_k(k)
```

**Key design decisions**:

1. **Skipped nodes still expand neighbors**: This preserves graph connectivity.
   A node that fails the filter may have neighbors that pass it.

2. **Skip counter with backoff**: If too many consecutive nodes fail the filter,
   the search is exhausting the local neighborhood without finding matches. The
   `max_skips` threshold triggers termination to avoid unbounded traversal.

3. **Adaptive ef expansion**: When `filtered_skips > ef_search`, the effective
   search frontier is larger than requested, compensating for filtered-out nodes.

### 4.4 Strategy Selection

```
selectivity = estimate_selectivity(filter_expr, metaidx_stats)

if selectivity < 0.01:
    strategy = PRE_FILTER
elif selectivity > 0.20:
    strategy = POST_FILTER
else:
    strategy = INTRA_FILTER
```

Selectivity estimation uses statistics stored in the METAIDX_SEG header:

- **Inverted index**: `posting_list_length / total_vectors` per term
- **Bitmap index**: `popcount(bitmap) / total_vectors` per enum value
- **Range tree**: count of values in range / total_vectors

For compound filters (AND/OR), selectivity is estimated using independence
assumption: `P(A AND B) = P(A) * P(B)`, `P(A OR B) = P(A) + P(B) - P(A) * P(B)`.

## 5. METAIDX_SEG (Segment Type 0x0D)

METAIDX_SEG stores secondary indexes over metadata fields for fast predicate
evaluation. Each METAIDX_SEG covers one field. The segment type enum value 0x0D
is allocated from the reserved range (see `binary-layout.md` Section 3).

```
METAIDX_SEG Payload:

+------------------------------------------+
| Index Header (64 bytes, padded)          |
|   field_id:         u16                  |  Field being indexed
|   index_type:       u8                   |  0=inverted, 1=range_tree, 2=bitmap
|   field_type:       u8                   |  Mirrors META_SEG field_type
|   total_vectors:    u64                  |  Vectors covered by this index
|   unique_values:    u64                  |  Cardinality (distinct values)
|   reserved:         [u8; 42]             |
|   [64B aligned]                          |
+------------------------------------------+
| Index Data (type-specific)               |
+------------------------------------------+
```

### 5.1 Inverted Index (index_type = 0)

Best for: string fields with moderate cardinality (100 to 100K distinct values).

```
term_count:       u32
For each term (sorted by encoded value):
  term_length:    u16
  term_bytes:     [u8; term_length]         Encoded value (UTF-8 for strings)
  posting_length: u32                       Number of vector IDs
  postings:       [varint delta-encoded]    Sorted vector IDs
  [8B aligned after postings]
[64B aligned]
```

Posting lists use varint delta encoding identical to the ID encoding in VEC_SEG
(see `binary-layout.md` Section 5). Restart points every 128 entries enable
binary search within a posting list for intersection operations.

### 5.2 Range Tree (index_type = 1)

Best for: numeric fields requiring range queries (u32, u64, f32).

```
page_size:        u32                       Fixed 4096 bytes (4 KB, one disk page)
page_count:       u32
root_page:        u32                       Page index of B+ tree root
tree_height:      u8
reserved:         [u8; 47]
[64B aligned]

Internal Page (4096 bytes):
  page_type:      u8 (0 = internal)
  key_count:      u16
  keys:           [field_type; key_count]   Separator keys
  children:       [u32; key_count + 1]      Child page indices
  [zero-padded to 4096]

Leaf Page (4096 bytes):
  page_type:      u8 (1 = leaf)
  entry_count:    u16
  prev_leaf:      u32                       Linked-list pointer for range scan
  next_leaf:      u32
  entries:
    For each entry:
      value:      field_type                The metadata value
      vector_id:  u64                       Associated vector ID
  [zero-padded to 4096]
```

Leaf pages form a doubly-linked list for efficient range scans. A range query
`position_start >= 1000 AND position_start <= 5000` descends the tree to find
the first leaf with value >= 1000, then scans forward until value > 5000.

### 5.3 Bitmap Index (index_type = 2)

Best for: enum and bool fields with low cardinality (< 64 distinct values).

```
value_count:      u8                        Number of distinct enum/bool values
For each value:
  value_label_len: u8
  value_label:    [u8; value_label_len]     The enum label or "true"/"false"
  bitmap_format:  u8                        0 = raw, 1 = roaring
  bitmap_length:  u32                       Byte length of bitmap data
  bitmap_data:    [u8; bitmap_length]       Bitmap of matching vector IDs
  [8B aligned]
[64B aligned]
```

**Raw bitmaps** are used when `total_vectors < 8192` (1 KB per bitmap).

**Roaring bitmaps** are used for larger datasets. The roaring format stores
the bitmap as a set of containers (array, bitmap, or run-length) per 64K chunk.
This matches the industry-standard Roaring bitmap serialization (compatible with
CRoaring / roaring-rs wire format).

Bitmap intersection and union operations map directly to AND/OR filter predicates
using SIMD bitwise operations. For 10M vectors:

```
Raw bitmap:    ~1.2 MB per value (impractical for many values)
Roaring bitmap: 100 KB - 1 MB per value depending on density
AND/OR:        ~0.1 ms per operation (AVX-512 on 1 MB bitmap)
```

## 6. Level 1 Manifest Addition

### Tag 0x000F: METADATA_INDEX_DIR

A new TLV record in the Level 1 manifest (see `02-manifest-system.md` Section 3)
that maps indexed metadata fields to their METAIDX_SEG segment IDs.

```
Tag:     0x000F
Name:    METADATA_INDEX_DIR

Payload:
  entry_count:    u16
  For each entry:
    field_id:     u16                       Matches META_SEG field_id
    field_name_len: u8
    field_name:   [u8; field_name_len]      UTF-8 field name for debugging
    index_seg_id: u64                       Segment ID of METAIDX_SEG
    index_type:   u8                        0=inverted, 1=range_tree, 2=bitmap
    stats:
      total_vectors: u64
      unique_values: u64
      min_posting_len: u32                  Smallest posting list size
      max_posting_len: u32                  Largest posting list size
```

This allows the query planner to estimate selectivity without reading the
METAIDX_SEG segments themselves. The `min_posting_len` and `max_posting_len`
fields provide bounds for cardinality estimation.

### Updated Record Types Table

```
Tag     Name                    Description
---     ----                    -----------
0x0001  SEGMENT_DIR             Array of segment directory entries
0x0002  TEMP_TIER_MAP           Temperature tier assignments per block
...
0x000D  KEY_DIRECTORY           Encryption key references
0x000E  (reserved)
0x000F  METADATA_INDEX_DIR      Metadata field -> METAIDX_SEG mapping
```

## 7. Performance Analysis

### 7.1 Filter Strategy vs Selectivity vs Recall

| Selectivity | Strategy | Recall@10 | Latency (10M vectors) | Notes |
|-------------|----------|-----------|----------------------|-------|
| 0.001% (100 matches) | Pre-filter | 1.00 | 0.02 ms | Flat scan on 100 candidates |
| 0.01% (1K matches) | Pre-filter | 0.99 | 0.08 ms | Flat scan on 1K candidates |
| 0.1% (10K matches) | Pre-filter | 0.98 | 0.5 ms | Mini-HNSW on 10K candidates |
| 1% (100K matches) | Intra-filter | 0.96 | 0.12 ms | ~10% node skip overhead |
| 5% (500K matches) | Intra-filter | 0.95 | 0.08 ms | ~5% node skip overhead |
| 10% (1M matches) | Intra-filter | 0.94 | 0.06 ms | Minimal skip overhead |
| 20% (2M matches) | Post-filter | 0.95 | 0.10 ms | 5x over-retrieval |
| 50% (5M matches) | Post-filter | 0.97 | 0.06 ms | 2x over-retrieval |
| 100% (no filter) | None | 0.98 | 0.04 ms | Baseline unfiltered |

### 7.2 Memory Overhead of Metadata Indexes

For 10M vectors with the RVDNA profile (5 indexed fields):

| Field | Type | Cardinality | Index Type | Size |
|-------|------|-------------|------------|------|
| organism | string | ~50K | Inverted | ~80 MB |
| gene_id | string | ~500K | Inverted | ~120 MB |
| chromosome | string | ~25 | Bitmap (roaring) | ~12 MB |
| position_start | u64 | ~10M | Range tree | ~160 MB |
| position_end | u64 | ~10M | Range tree | ~160 MB |
| **Total** | | | | **~532 MB** |

As a fraction of vector data (10M * 384 dim * fp16 = 7.2 GB): **~7.4% overhead**.

For the RVText profile (2 indexed fields, typically lower cardinality):

| Field | Type | Cardinality | Index Type | Size |
|-------|------|-------------|------------|------|
| source_url | string | ~100K | Inverted | ~90 MB |
| language | string | ~50 | Bitmap (roaring) | ~8 MB |
| **Total** | | | | **~98 MB** |

Overhead: **~1.4%** of vector data.

### 7.3 Query Latency Breakdown (Filtered Intra-Search)

```
Phase                         Time        Notes
-----                         ----        -----
Parse filter expression       0.5 us      Stack-based, no allocation
Estimate selectivity          1.0 us      Read manifest stats
Load METAIDX_SEG (if cold)    50-200 us   First query only; cached after
HNSW traversal (150 steps)    45 us       Baseline unfiltered
  + filter eval per node      +12 us      ~80 ns per eval * 150 nodes
  + skip expansion            +8 us       ~20% more nodes visited at 5% sel.
Top-K collection              10 us       Heap operations
                              --------
Total (warm cache)            ~76 us
Total (cold start)            ~276 us
```

## 8. Integration with Temperature Tiering

Metadata follows the same temperature model as vector data (see
`03-temperature-tiering.md`), but with its own tier assignments.

### 8.1 Hot Metadata

Indexed fields for hot-tier vectors are kept resident in memory:

- **Bitmap indexes** for low-cardinality fields (enum, bool) are always hot.
  Total size is bounded: `cardinality * ceil(hot_vectors / 8)` bytes. For 100K
  hot vectors and 25 enum values: 25 * 12.5 KB = 312 KB.

- **Inverted index posting lists** are cached using an LRU policy keyed by
  (field_id, term). Frequently queried terms (e.g., `language = "en"`) remain
  resident.

- **Range tree pages** follow the standard B+ tree buffer pool model. Hot pages
  (root + first two levels) are pinned. Leaf pages are demand-paged.

### 8.2 Cold Metadata

Cold metadata covers vectors that are rarely accessed:

- META_SEG data for cold vectors is compressed with ZSTD (level 9+) and stored
  in cold-tier segments.
- METAIDX_SEG posting lists for cold vectors are not loaded until a query
  specifically requests them.
- When a filter matches only cold vectors (detected via the temperature tier
  map), the runtime issues a warning: filtered search on cold data may require
  decompression latency of 10-100 ms.

### 8.3 Compaction Coordination

When temperature-aware compaction reorganizes vector segments (see
`03-temperature-tiering.md` Section 4), metadata must follow:

```
1. Identify vectors moving between tiers
2. Rewrite META_SEG for affected vector ID ranges
3. Rebuild METAIDX_SEG posting lists (vector IDs may be renumbered during
   compaction if the COMPACTION_RENUMBER flag is set)
4. Update METADATA_INDEX_DIR in the new manifest
5. Tombstone old META_SEG and METAIDX_SEG segments
```

Metadata compaction piggybacks on vector compaction -- it never triggers
independently. This ensures metadata and vector segments remain in consistent
temperature tiers.

### 8.4 Metadata-Aware Promotion

When a filter query frequently accesses metadata for warm-tier vectors, those
metadata segments are candidates for promotion to hot tier. The access sketch
(SKETCH_SEG) tracks metadata segment accesses alongside vector accesses:

```
sketch_key = (META_SEG_ID << 32) | block_id
```

This reuses the existing sketch infrastructure without modification.

## 9. Wire Protocol: Filtered Query Message

For completeness, the filter expression is carried in the query message as a
tagged field. The query wire format is outside the scope of the storage spec,
but the filter payload is defined here for interoperability.

```
Query Message Filter Field:
  tag:              u16 (0x0040 = FILTER)
  length:           u32
  filter_version:   u8 (1)
  filter_payload:   [u8; length - 1]       Binary filter expression (Section 3.2)
```

Implementations that do not support filtered search must ignore tag 0x0040 and
return unfiltered results. This preserves backward compatibility.

## 10. Implementation Notes

### 10.1 Index Selection Heuristics

When building indexes for a new META_SEG field, implementations should select
the index type automatically:

```
if field_type in (enum, bool) and cardinality < 64:
    index_type = BITMAP
elif field_type in (u32, u64, f32):
    index_type = RANGE_TREE
else:
    index_type = INVERTED
```

Fields without the `"indexed": true` property in the profile schema must not
have METAIDX_SEG segments built. They are stored in META_SEG for retrieval
only (the STORED flag).

### 10.2 Posting List Intersection

For AND filters on multiple indexed fields, posting list intersection is
performed using a merge-based algorithm on sorted, delta-decoded posting lists:

```
Sorted Intersection (two-pointer merge):
  Time: O(min(|A|, |B|)) with skip-ahead via restart points
  Practical: ~100 ns per 1000 common elements (SIMD comparison)
```

For OR filters, posting list union uses a similar merge with deduplication.

### 10.3 Null Handling

- `FIELD_REF` for a null value pushes a sentinel NULL onto the stack
- `CMP_EQ NULL` returns true only for null values
- `CMP_NE NULL` returns true for all non-null values
- All other comparisons against NULL return false (SQL-style three-valued logic)
- `IN_SET` never matches NULL unless NULL is explicitly in the set
