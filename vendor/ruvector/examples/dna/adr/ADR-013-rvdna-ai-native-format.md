# ADR-013: RVDNA -- AI-Native Genomic File Format

**Status:** Accepted | **Date:** 2026-02-11 | **Authors:** RuVector Genomics Architecture Team
**Parents:** ADR-001 (Vision), ADR-003 (HNSW Index), ADR-004 (Attention), ADR-005 (GNN Protein), ADR-006 (Epigenomic)

## Context

Every AI genomics pipeline re-encodes from text formats (FASTA, BAM, VCF) into tensors on every run. For a human genome (~3.2 Gbp), this costs 30-120 seconds and dominates latency. No existing format co-locates raw sequence data with pre-computed embeddings, attention matrices, graph adjacencies, or vector indices in a single zero-copy binary.

| Format | Era  | AI-Ready? | Why Not |
|--------|------|-----------|---------|
| FASTA  | 1985 | No | Text, 1 byte/base, no tensors |
| BAM    | 2009 | Partial | Binary but row-oriented, no embeddings |
| VCF    | 2011 | No | Text, no graph structures |
| CRAM   | 2012 | No | Reference-based compression, no AI artifacts |

The RuVector DNA crate already implements 2-bit encoding (`kmer.rs`), HNSW indexing (`ruvector-core`), attention analysis, GNN protein folding, and epigenomic tracks as in-memory runtime structures. Every restart means full recomputation.

## Decision: The RVDNA Binary Format

We define `.rvdna` -- a sectioned, memory-mappable binary format for `mmap(2)` + zero-copy access via `memmap2`. Design principles: (1) zero-copy mmap access, (2) pre-computed AI embeddings co-located with sequences, (3) columnar SIMD-friendly layout, (4) hierarchical indexing (chromosome/region/k-mer/base), (5) native tensor/graph storage (COO, CSR, dense), (6) streaming-compatible chunked encoding. All sections 64-byte aligned.

### File Layout Overview

```
0x0000  64 B    File Header
0x0040  var     Section Directory (16 B per entry, up to 16)
        var     Sec 0: Sequence Data    Sec 1: K-mer Vector Index
        var     Sec 2: Attention        Sec 3: Variant Tensor
        var     Sec 4: Protein Embed    Sec 5: Epigenomic Tracks
        var     Sec 6: Metadata         Footer (16 B)
```

### Header (64 bytes, offset 0x0000)

```
Off   Sz  Type    Field               Notes
0x00   8  u8[8]   magic               "RVDNA\x01\x00\x00"
0x08   2  u16     version_major       1
0x0A   2  u16     version_minor       0
0x0C   4  u32     flags               bit field (below)
0x10   8  u64     total_file_size
0x18   8  u64     sequence_length     total bases
0x20   4  u32     num_sections        1-7
0x24   4  u32     section_dir_offset
0x28   1  u8      compression         0=none 1=LZ4 2=Zstd 3=Zstd+dict
0x29   1  u8      endianness          0xEF = little-endian (required)
0x2A   2  u16     ref_genome_id       0=none 1=GRCh38 2=T2T-CHM13
0x2C   4  u32     num_chromosomes
0x30   8  u64     creation_timestamp  Unix epoch seconds
0x38   4  u32     creator_version
0x3C   4  u32     header_checksum     CRC32C of 0x00-0x3B
```

**Flags:** bit 0=HAS_QUALITY, 1=HAS_KMER_INDEX, 2=HAS_ATTENTION, 3=HAS_VARIANTS, 4=HAS_PROTEIN, 5=HAS_EPIGENOMIC, 6=IS_PAIRED_END, 7=IS_PHASED, 8=KMER_QUANTIZED, 9=ATTENTION_SPARSE, 10=MMAP_SAFE.

### Section Directory (16 bytes per entry)

```
u64 section_offset    u32 compressed_size    u32 uncompressed_size
```

### Section 0: Sequence Data (columnar, block-compressed in 16 KB blocks)

**Block header (16 B):** `u32 block_bases | u32 compressed_size | u32 checksum_crc32c | u16 chromosome_id | u16 reserved`

**Nucleotide encoding:** 2 bits/base packed 4 per byte (A=00, C=01, G=10, T=11). N-bases tracked in a separate 1-bit-per-position mask array.

**Quality scores (optional, HAS_QUALITY):** 6-bit Phred per position, packed `ceil(n*6/8)` bytes. Range 0-63.

**Chromosome index table:** per chrom: `u32 id | u32 name_offset | u64 start_base_offset` (16 B each).

Storage per Mb: ~251 KB seq-only, ~1,001 KB with quality.

### Section 1: K-mer Vector Index (HNSW-Ready)

**Header (32 B):**
```
u32 num_k_values | u32 num_windows | u32 window_stride
u16 vector_dtype(0=f32,1=f16,2=int8,3=binary) | u16 hnsw_M | u16 hnsw_ef_construction
u16 hnsw_num_layers | u32 hnsw_graph_offset | u64 reserved
```

**Per k-value descriptor (16 B):** `u8 k | u8 dim_log2 | u16 vector_dim | u32 num_vectors | u64 data_offset`

**Vector data:** contiguous per k. f32: `n*dim*4` B. f16: `n*dim*2` B. int8: `n*dim` B + `n*8` B (f32 scale + f32 zero per vector; dequant: `f32 = (int8 - zero) * scale`).

**HNSW graph:** per layer top-down: `u32 num_nodes`, then per node: `u16 num_neighbors | u16[neighbors]`. Entry point: first u32 after layer count.

### Section 2: Attention Matrices (Sparse COO)

**Header (24 B):** `u32 num_windows | u32 window_size | u32 num_heads | u16 value_dtype(0=f32,1=f16,2=bf16) | u16 index_dtype(0=u16,1=u32) | u32 total_nnz | u32 sparsity_threshold`

**Per window (16 B):** `u64 genomic_start | u32 nnz | u32 data_offset`

**COO triplets:** index_dtype=u16: `u16 row | u16 col | f16 value` (6 B). index_dtype=u32: `u32 row | u32 col | f32 value` (12 B).

**Cross-attention pairs (optional):** per pair header (24 B): `u64 query_start | u64 ref_start | u32 nnz | u32 data_offset`, followed by COO triplets.

### Section 3: Variant Tensor (Probabilistic)

**Header (24 B):** `u32 num_variant_sites | u32 max_alleles | u32 num_haplotype_blocks | u16 likelihood_dtype | u16 ploidy | u32 calibration_points | u32 reserved`

**Per variant site:** `u64 position | u8 ref_allele(2-bit) | u8 num_alt | u8[num_alt] alts | f16[G] genotype_likelihoods | f16 allele_freq | u8 filter_flags` where G=(num_alt+1)*(num_alt+2)/2 for diploid.

**Haplotype blocks (24 B each):** `u64 start | u64 end | u32 num_variants | u16 phase_set_id | u16 phase_quality`

**Calibration (8 B each):** `f32 reported_quality | f32 empirical_quality`

### Section 4: Protein Embeddings (GNN-Ready)

**Header (24 B):** `u32 num_proteins | u16 embedding_dim | u16 dtype | u32 total_residues | u32 total_contacts | u32 ss_present | u32 binding_present`

**Per protein (32 B):** `u32 protein_id | u32 gene_id | u32 num_residues | u32 embed_offset | u32 csr_rowptr_off | u32 csr_colidx_off | u32 csr_values_off | u32 annotation_off`

**Embeddings:** row-major `num_residues * dim * sizeof(dtype)`. **CSR graph:** `row_ptr: u32[n+1]`, `col_idx: u32[edges]`, `values: f16[edges]`. **SS:** `u8[n]` (0=coil, 1=helix, 2=sheet, 3=turn). **Binding:** `u8[n]` bit flags (0=DNA, 1=ligand, 2=protein-protein, 3=metal).

### Section 5: Epigenomic Tracks (Temporal)

**Header (20 B):** `u32 num_cpg | u32 num_access | u32 num_histone | u32 num_clock | u32 num_timepoints`

**CpG (12 B each):** `u64 position | f16 beta | u16 coverage`. **ATAC peaks (16 B):** `u64 start | u32 width | f16 score | u16 reserved`. **Histone (6 B):** `u32 bin_index | f16 signal`. **Clock (12 B):** `u32 cpg_idx | f32 coeff | f32 intercept_contrib`.

### Section 6: Metadata & Provenance

**Header (8 B):** `u32 msgpack_size | u32 string_table_size`

MessagePack-encoded metadata (sample ID, species, reference assembly, source files, pipeline version, per-section CRC32C checksums, model parameters). String table: concatenated null-terminated UTF-8 for chromosome names and identifiers.

### Footer (16 bytes)

```
u64 magic_footer ("RVDNA_END" = 0x444E455F414E4456)
u32 global_checksum (XOR of all section CRC32Cs)
u32 footer_offset (self-offset from file start)
```

## Indexing Structures

| Index | Location | Lookup Time | Format |
|-------|----------|-------------|--------|
| B+ tree | Sec 0 trailer | <500 ns | 64 B nodes: `u16 num_keys, u16 is_leaf, u32 rsv, u64[3] keys, u32[4] children, u8[8] pad` |
| HNSW | Sec 1 inline | <10 us | Layered neighbor lists (see Sec 1) |
| Bloom filter | Sec 0 trailer | <100 ns | `u32 num_bits, u32 num_hashes, u8[ceil(bits/8)]` |
| Interval tree | Sec 3 inline | O(log n + k) | Augmented BST for variant overlap queries |

## Performance Targets

| Operation | Target | Mechanism |
|-----------|--------|-----------|
| Random access 1 KB region | <1 us | mmap + B+ tree |
| K-mer similarity top-10 | <10 us | Pre-built HNSW, ef_search=50 |
| Attention matrix 10 KB window | <100 us | Pre-computed COO |
| Variant at position | <500 ns | B+ tree + block binary search |
| FASTA conversion (1 Mb) | <1 s | 2-bit encode + LZ4 |
| File open + header | <10 us | 64 B fixed read |

## Format Comparison

| Property | FASTA | BAM | VCF | CRAM | **RVDNA** |
|----------|-------|-----|-----|------|-----------|
| Storage/Mb (seq) | 1,000 KB | 300 KB | N/A | 50 KB | **251 KB** |
| Storage/Mb (seq+AI) | N/A | N/A | N/A | N/A | **~5,000 KB** |
| Random access | O(n) | ~10 us | O(n) | ~50 us | **<1 us** |
| AI-ready | No | No | No | No | **Yes** |
| Streaming | Yes | No | Yes | No | **Yes** |
| Vector search | No | No | No | No | **HNSW** |
| Tensor/graph | No | No | No | No | **COO/CSR** |
| Zero-copy mmap | No | Partial | No | No | **Full** |

## Consequences

**Positive:** Eliminates 30-120s re-encoding tax. Sub-microsecond random access. Pre-built HNSW enables real-time population-scale similarity. Single file -- no sidecar indices. Columnar SIMD access. Partial section loading. 64-byte alignment for cache efficiency.

**Negative:** Larger than CRAM for sequence-only storage (~4x from AI sections). Requires re-encoding during transition. Pre-computed tensors stale on model updates. No existing tool support (samtools, IGV).

**Neutral:** MessagePack metadata less human-readable than JSON. Write-once/read-many by design. Per-section compression optional.

## Options Considered

1. **Extend BAM with custom tags** -- rejected: row-oriented layout blocks SIMD; 2-char tag namespace; no sparse tensors; BGZF 64 KB blocks too coarse.
2. **HDF5 with genomic schema** -- rejected: not zero-copy mmap-friendly; C library global locks; no HNSW; not `no_std` Rust compatible.
3. **Arrow/Parquet genomic schema** -- rejected: row groups too coarse; no sparse tensor type; no graph adjacency; heavy C++ dependency.
4. **Custom binary (RVDNA)** -- selected: purpose-built for AI genomics access patterns; zero-copy; native HNSW/B+/Bloom; WASM-compatible; 100-1000x latency improvement justifies ecosystem investment.

## Implementation Strategy

**Phase 1 (Weeks 1-4):** Header, section directory, footer. Section 0 (sequence + B+ tree). Section 6 (metadata). `rvdna-encode` CLI. `ruvector-rvdna` crate with mmap reader.

**Phase 2 (Weeks 5-8):** Section 1 (k-mer + HNSW). Section 2 (attention COO). Section 3 (variant tensor). Integration with `kmer.rs`, `pipeline.rs`, `variant.rs`.

**Phase 3 (Weeks 9-12):** Section 4 (protein CSR graphs). Section 5 (epigenomic tracks). GNN integration. End-to-end benchmarks vs BAM/CRAM.

## Rust API Sketch

```rust
pub struct RvdnaFile { mmap: Mmap, header: &'static RvdnaHeader, sections: Vec<SectionEntry> }

impl RvdnaFile {
    pub fn open(path: &Path) -> Result<Self, RvdnaError>;
    pub fn sequence(&self, chrom: u16, start: u64, len: u64) -> &[u8];       // zero-copy
    pub fn kmer_vectors(&self, k: u8, region: GenomicRange) -> &[f32];       // zero-copy
    pub fn kmer_search(&self, query: &[f32], k: u8, top_n: usize) -> Vec<SearchResult>;
    pub fn attention(&self, window_idx: u32) -> SparseCooMatrix<f16>;
    pub fn variant_at(&self, position: u64) -> Option<VariantRecord>;
    pub fn protein_embedding(&self, id: u32) -> &[f16];                      // zero-copy
    pub fn contact_graph(&self, id: u32) -> CsrGraph<f16>;
    pub fn methylation(&self, region: GenomicRange) -> &[CpgSite];
}
```

## Related Decisions

- **ADR-003**: HNSW genomic vector index -- Section 1 serializes this
- **ADR-004**: Attention architecture -- Section 2 persists attention matrices
- **ADR-005**: GNN protein engine -- Section 4 stores protein graphs
- **ADR-006**: Epigenomic engine -- Section 5 stores methylation/histone tracks
- **ADR-011**: Performance targets -- RVDNA must meet latency budgets defined there

## References

- [SAM/BAM v1.6](https://samtools.github.io/hts-specs/SAMv1.pdf) | [VCF v4.3](https://samtools.github.io/hts-specs/VCFv4.3.pdf) | [CRAM v3.1](https://samtools.github.io/hts-specs/CRAMv3.pdf)
- [HNSW paper](https://arxiv.org/abs/1603.09320) | [ESM-2](https://www.science.org/doi/10.1126/science.ade2574)
- [memmap2](https://docs.rs/memmap2) | [LZ4 frame format](https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md) | [MessagePack](https://msgpack.org) | [CRC32C](https://tools.ietf.org/html/rfc3720#appendix-B.4)
