# RVF Domain Profiles

## 1. Profile Architecture

A domain profile is a **semantic overlay** on the universal RVF substrate. It does
not change the wire format — every profile-specific file is a valid RVF file. The
profile adds:

1. **Semantic type annotations** for vector dimensions
2. **Domain-specific distance metrics**
3. **Custom quantization strategies** optimized for the domain
4. **Metadata schemas** for domain-specific labels and provenance
5. **Query preprocessing** conventions

Profiles are declared in a PROFILE_SEG and referenced by the root manifest's
`profile_id` field.

```
+-- RVF Universal Substrate --+
| Segments, manifests, tiers  |
| HNSW index, overlays        |
| Temperature, compaction     |
+-----------------------------+
        |
        | profile_id
        v
+-- Domain Profile Layer --+
| Semantic types            |
| Custom distances          |
| Metadata schema           |
| Query conventions         |
+---------------------------+
```

## 2. PROFILE_SEG Binary Layout

```
Offset  Size  Field              Description
------  ----  -----              -----------
0x00    4     profile_magic      Profile-specific magic number
0x04    2     profile_version    Profile spec version
0x06    2     profile_id         Same as root manifest profile_id
0x08    32    profile_name       UTF-8 null-terminated name
0x28    8     schema_length      Length of metadata schema
0x30    var   metadata_schema    JSON or binary schema for META_SEG entries
var     8     distance_config_len Length of distance configuration
var     var   distance_config    Distance metric parameters
var     8     quant_config_len   Length of quantization configuration
var     var   quant_config       Domain-specific quantization parameters
var     8     preprocess_len     Length of preprocessing spec
var     var   preprocess_spec    Query preprocessing pipeline description
```

## 3. RVDNA Profile (Genomics)

### Profile Declaration

```
profile_magic:    0x52444E41 ("RDNA")
profile_id:       0x01
profile_name:     "rvdna"
```

### Semantic Types

RVDNA vectors encode biological sequences at multiple granularities:

| Granularity | Dimensions | Encoding | Use Case |
|------------|-----------|----------|----------|
| Codon | 64 | Frequency of each codon in reading frame | Gene-level comparison |
| K-mer (k=6) | 4096 | 6-mer frequency spectrum | Species identification |
| Motif | 128-512 | Learned motif embeddings (transformer) | Regulatory element search |
| Structure | 256 | Protein secondary structure embedding | Fold similarity |
| Epigenetic | 384 | Methylation + histone mark embedding | Epigenomic comparison |

### Distance Metrics

```
Codon frequency:     Jensen-Shannon divergence (symmetric KL)
K-mer spectrum:      Cosine similarity (normalized frequency vectors)
Motif embedding:     L2 distance (Euclidean in learned space)
Structure:           L2 distance with structure-aware weighting
Epigenetic:          Weighted cosine (CpG density as weight)
```

### Quantization Strategy

Genomic vectors have specific statistical properties:

- **Codon frequencies**: Sparse, non-negative, sum-to-1. Use **scalar quantization
  with log transform**: `q = round(log2(freq + epsilon) * scale)`. 8-bit covers
  6 orders of magnitude.

- **K-mer spectra**: Very sparse (most 6-mers absent in short reads). Use
  **sparse encoding**: store only non-zero k-mer indices + values. Typical
  compression: 20-50x over dense.

- **Learned embeddings**: Gaussian-distributed. Standard PQ works well.
  M=32 subspaces, K=256 centroids (8-bit codes).

### Metadata Schema

```json
{
  "type": "rvdna",
  "fields": {
    "organism": { "type": "string", "indexed": true },
    "gene_id": { "type": "string", "indexed": true },
    "chromosome": { "type": "string", "indexed": true },
    "position_start": { "type": "u64", "indexed": true },
    "position_end": { "type": "u64", "indexed": true },
    "strand": { "type": "enum", "values": ["+", "-"] },
    "quality_score": { "type": "f32" },
    "source_format": { "type": "enum", "values": ["FASTA", "FASTQ", "BAM", "VCF"] },
    "read_depth": { "type": "u32" },
    "gc_content": { "type": "f32" }
  }
}
```

### Query Preprocessing

For RVDNA queries:
1. Input: Raw sequence string (ACGT...)
2. Compute k-mer frequency spectrum
3. Apply log transform for codon/k-mer queries
4. Normalize to unit length for cosine metrics
5. Encode as fp16 vector
6. Submit to RVF query path

## 4. RVText Profile (Language)

### Profile Declaration

```
profile_magic:    0x52545854 ("RTXT")
profile_id:       0x02
profile_name:     "rvtext"
```

### Semantic Types

| Granularity | Dimensions | Source | Use Case |
|------------|-----------|--------|----------|
| Token | 768-1536 | Transformer last hidden state | Semantic search |
| Sentence | 384-768 | Sentence transformer pooled output | Document retrieval |
| Paragraph | 384-1024 | Long-context model embedding | Passage ranking |
| Document | 256-512 | Document-level embedding | Collection search |
| Sparse | 30522 | BM25/SPLADE term weights | Lexical matching |

### Distance Metrics

```
Dense embeddings:    Cosine similarity (normalized dot product)
Sparse (SPLADE):     Dot product on sparse vectors
Hybrid:              alpha * dense_score + (1-alpha) * sparse_score
Matryoshka:          Cosine on truncated prefix (adaptive dimensionality)
```

### Quantization Strategy

Text embeddings are well-suited to aggressive quantization:

- **Dense (384-768 dim)**: Binary quantization achieves 0.95+ recall on
  normalized embeddings. 384 dims -> 48 bytes. Use binary for cold tier,
  int8 for hot.

- **Sparse (SPLADE)**: Store as sorted (term_id, weight) pairs with
  delta-encoded term_ids. Typical sparsity: 100-300 non-zero terms out
  of 30K vocabulary. Compression: ~100x over dense.

- **Matryoshka**: Store full-dimension vectors but index only the first
  D/4 dimensions. Progressive refinement uses more dimensions.

### Metadata Schema

```json
{
  "type": "rvtext",
  "fields": {
    "text": { "type": "string", "stored": true, "max_length": 8192 },
    "source_url": { "type": "string", "indexed": true },
    "language": { "type": "string", "indexed": true },
    "model_id": { "type": "string" },
    "chunk_index": { "type": "u32" },
    "total_chunks": { "type": "u32" },
    "token_count": { "type": "u32" },
    "timestamp": { "type": "u64" }
  }
}
```

### Query Preprocessing

1. Input: Raw text string
2. Tokenize with model-specific tokenizer
3. Encode through embedding model (or receive pre-computed embedding)
4. L2-normalize for cosine similarity
5. Optionally: compute SPLADE sparse expansion
6. Submit dense + sparse to hybrid query path

## 5. RVGraph Profile (Networks)

### Profile Declaration

```
profile_magic:    0x52475248 ("RGRH")
profile_id:       0x03
profile_name:     "rvgraph"
```

### Semantic Types

| Granularity | Dimensions | Source | Use Case |
|------------|-----------|--------|----------|
| Node | 64-256 | Node2Vec / GCN embedding | Node similarity |
| Edge | 64-128 | Edge feature embedding | Link prediction |
| Subgraph | 128-512 | Graph kernel embedding | Subgraph matching |
| Community | 64-256 | Community embedding | Community detection |
| Spectral | 32-128 | Laplacian eigenvectors | Graph structure |

### Distance Metrics

```
Node embedding:      L2 distance
Edge embedding:      Cosine similarity
Subgraph:            Wasserstein distance (approximated by L2 on sorted features)
Community:           Cosine similarity
Spectral:            L2 on normalized eigenvectors
```

### Integration with Overlay System

RVGraph uniquely integrates with the RVF overlay epoch system:

- **Graph structure** is stored in OVERLAY_SEGs (not just as metadata)
- **Node embeddings** are stored in VEC_SEGs
- **Edge weights** are overlay deltas
- **Community assignments** are partition summaries
- **Min-cut witnesses** directly serve graph partitioning queries

This means RVGraph files are simultaneously vector stores AND graph databases.
The overlay system provides dynamic graph operations (add/remove edges,
rebalance partitions) while the vector system provides similarity search.

### Metadata Schema

```json
{
  "type": "rvgraph",
  "fields": {
    "node_type": { "type": "string", "indexed": true },
    "edge_type": { "type": "string", "indexed": true },
    "node_label": { "type": "string", "indexed": true },
    "degree": { "type": "u32", "indexed": true },
    "community_id": { "type": "u32", "indexed": true },
    "pagerank": { "type": "f32" },
    "clustering_coeff": { "type": "f32" },
    "source_graph": { "type": "string" }
  }
}
```

## 6. RVVision Profile (Imagery)

### Profile Declaration

```
profile_magic:    0x52564953 ("RVIS")
profile_id:       0x04
profile_name:     "rvvision"
```

### Semantic Types

| Granularity | Dimensions | Source | Use Case |
|------------|-----------|--------|----------|
| Patch | 64-256 | ViT patch embedding | Region search |
| Image | 512-2048 | CLIP / DINOv2 global embedding | Image retrieval |
| Object | 256-512 | Object detection crop embedding | Object search |
| Scene | 128-512 | Scene classification embedding | Scene matching |
| Multi-scale | 256 * N | Pyramid of embeddings at scales | Scale-invariant search |

### Distance Metrics

```
CLIP embedding:      Cosine similarity (model-normalized)
DINOv2:              Cosine similarity
Patch:               L2 distance (not normalized)
Multi-scale:         Weighted sum of per-scale cosine similarities
```

### Quantization Strategy

Vision embeddings have high intrinsic dimensionality but are compressible:

- **CLIP (512-dim)**: PQ with M=64, K=256 works well. Binary quantization
  achieves 0.90+ recall.

- **DINOv2 (768-dim)**: Similar to CLIP. PQ M=96, K=256.

- **Patch embeddings**: Large volume (196+ patches per image). Aggressive
  quantization to 4-bit scalar. Use residual PQ for high-recall applications.

### Spatial Metadata

RVVision supports spatial queries through metadata:

```json
{
  "type": "rvvision",
  "fields": {
    "image_id": { "type": "string", "indexed": true },
    "patch_row": { "type": "u16" },
    "patch_col": { "type": "u16" },
    "scale": { "type": "f32" },
    "bbox_x": { "type": "f32" },
    "bbox_y": { "type": "f32" },
    "bbox_w": { "type": "f32" },
    "bbox_h": { "type": "f32" },
    "object_class": { "type": "string", "indexed": true },
    "confidence": { "type": "f32" },
    "model_id": { "type": "string" }
  }
}
```

## 7. Custom Profile Registration

New profiles can be registered by writing a PROFILE_SEG:

```
1. Choose a unique profile_id (0x10-0xEF for custom profiles)
2. Define a 4-byte profile_magic
3. Define metadata schema
4. Define distance metric configuration
5. Define quantization recommendations
6. Write PROFILE_SEG into the RVF file
7. Set profile_id in root manifest
```

The profile system is open — any domain can define its own profile as long
as it maps onto the RVF substrate. The substrate does not need to understand
the domain semantics; it only needs to store vectors, compute distances,
and maintain indexes.

## 8. Cross-Profile Queries

RVF files with different profiles can be queried together if their vectors
share a compatible embedding space. This is common in multimodal applications:

```
Query: "Find images similar to this text description"

1. Text embedding (RVText profile) -> 512-dim CLIP text vector
2. Image database (RVVision profile) -> 512-dim CLIP image vectors
3. Distance metric: Cosine similarity (shared CLIP space)
4. Result: Images ranked by text-image similarity
```

The query path treats both files as RVF files. The profile only affects
preprocessing and metadata interpretation — the core distance computation
and indexing are profile-agnostic.

## 9. Profile Compatibility Matrix

| Source Profile | Target Profile | Compatible? | Condition |
|---------------|---------------|------------|-----------|
| RVDNA | RVDNA | Yes | Same granularity |
| RVText | RVText | Yes | Same model or compatible space |
| RVVision | RVVision | Yes | Same model or compatible space |
| RVText | RVVision | Yes | If both use CLIP or shared space |
| RVDNA | RVText | No* | Unless mapped through protein language model |
| RVGraph | Any | Partial | Node embeddings may share space |

*Cross-domain compatibility requires explicit embedding space alignment,
which is outside the scope of the format spec but enabled by it.
