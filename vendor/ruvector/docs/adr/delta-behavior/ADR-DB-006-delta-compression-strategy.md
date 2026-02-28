# ADR-DB-006: Delta Compression Strategy

**Status**: Proposed
**Date**: 2026-01-28
**Authors**: RuVector Architecture Team
**Deciders**: Architecture Review Board
**Parent**: ADR-DB-001 Delta Behavior Core Architecture

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 0.1 | 2026-01-28 | Architecture Team | Initial proposal |

---

## Context and Problem Statement

### The Compression Challenge

Delta-first architecture generates significant data volume:
- Each delta includes metadata (IDs, clocks, timestamps)
- Delta chains accumulate over time
- Network transmission requires bandwidth
- Storage persists all deltas for history

### Compression Opportunities

| Data Type | Characteristics | Compression Potential |
|-----------|-----------------|----------------------|
| Delta values (f32) | Smooth distributions | 2-4x with quantization |
| Indices (u32) | Sparse, sorted | 3-5x with delta+varint |
| Metadata | Repetitive strings | 5-10x with dictionary |
| Batches | Similar patterns | 10-50x with deduplication |

### Requirements

1. **Speed**: Compression/decompression < 1ms for typical deltas
2. **Ratio**: >3x compression for storage, >5x for network
3. **Streaming**: Support for streaming compression/decompression
4. **Lossless Option**: Must support exact reconstruction
5. **WASM Compatible**: Must work in browser environment

---

## Decision

### Adopt Multi-Tier Compression Strategy

We implement a tiered compression system that adapts to data characteristics and use case requirements.

### Compression Tiers

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                  COMPRESSION TIER SELECTION                  │
                    └─────────────────────────────────────────────────────────────┘

                                           Input Delta
                                               │
                                               v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    TIER 0: ENCODING                          │
                    │        Format selection (Sparse/Dense/RLE/Dict)              │
                    │        Typical: 1-10x compression, <10us                     │
                    └─────────────────────────────────────────────────────────────┘
                                               │
                                               v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    TIER 1: VALUE COMPRESSION                 │
                    │        Quantization (f32 -> f16/i8/i4)                       │
                    │        Typical: 2-8x compression, <50us                      │
                    └─────────────────────────────────────────────────────────────┘
                                               │
                                               v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    TIER 2: ENTROPY CODING                    │
                    │        LZ4 (fast) / Zstd (balanced) / Brotli (max)          │
                    │        Typical: 1.5-3x additional, 10us-1ms                  │
                    └─────────────────────────────────────────────────────────────┘
                                               │
                                               v
                    ┌─────────────────────────────────────────────────────────────┐
                    │                    TIER 3: BATCH COMPRESSION                 │
                    │        Dictionary, deduplication, delta-of-deltas            │
                    │        Typical: 2-10x additional for batches                 │
                    └─────────────────────────────────────────────────────────────┘
```

### Tier 0: Encoding Layer

See ADR-DB-002 for format selection. This tier handles:
- Sparse vs Dense vs RLE vs Dictionary encoding
- Index delta-encoding
- Varint encoding for integers

### Tier 1: Value Compression

```rust
/// Value quantization for delta compression
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum QuantizationLevel {
    /// No quantization (f32)
    None,
    /// Half precision (f16)
    Float16,
    /// 8-bit scaled integers
    Int8 { scale: f32, offset: f32 },
    /// 4-bit scaled integers
    Int4 { scale: f32, offset: f32 },
    /// Binary (sign only)
    Binary,
}

/// Quantize delta values
pub fn quantize_values(
    values: &[f32],
    level: QuantizationLevel,
) -> QuantizedValues {
    match level {
        QuantizationLevel::None => {
            QuantizedValues::Float32(values.to_vec())
        }

        QuantizationLevel::Float16 => {
            let quantized: Vec<u16> = values.iter()
                .map(|&v| half::f16::from_f32(v).to_bits())
                .collect();
            QuantizedValues::Float16(quantized)
        }

        QuantizationLevel::Int8 { scale, offset } => {
            let quantized: Vec<i8> = values.iter()
                .map(|&v| ((v - offset) / scale).round().clamp(-128.0, 127.0) as i8)
                .collect();
            QuantizedValues::Int8 {
                values: quantized,
                scale,
                offset,
            }
        }

        QuantizationLevel::Int4 { scale, offset } => {
            // Pack two 4-bit values per byte
            let packed: Vec<u8> = values.chunks(2)
                .map(|chunk| {
                    let v0 = ((chunk[0] - offset) / scale).round().clamp(-8.0, 7.0) as i8;
                    let v1 = chunk.get(1)
                        .map(|&v| ((v - offset) / scale).round().clamp(-8.0, 7.0) as i8)
                        .unwrap_or(0);
                    ((v0 as u8 & 0x0F) << 4) | (v1 as u8 & 0x0F)
                })
                .collect();
            QuantizedValues::Int4 {
                packed,
                count: values.len(),
                scale,
                offset,
            }
        }

        QuantizationLevel::Binary => {
            // Pack 8 signs per byte
            let packed: Vec<u8> = values.chunks(8)
                .map(|chunk| {
                    chunk.iter().enumerate().fold(0u8, |acc, (i, &v)| {
                        if v >= 0.0 {
                            acc | (1 << i)
                        } else {
                            acc
                        }
                    })
                })
                .collect();
            QuantizedValues::Binary {
                packed,
                count: values.len(),
            }
        }
    }
}

/// Adaptive quantization based on value distribution
pub fn select_quantization(values: &[f32], config: &QuantizationConfig) -> QuantizationLevel {
    // Compute statistics
    let min = values.iter().cloned().fold(f32::INFINITY, f32::min);
    let max = values.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let range = max - min;

    // Check if values are clustered enough for aggressive quantization
    let variance = compute_variance(values);
    let coefficient_of_variation = variance.sqrt() / (values.iter().sum::<f32>() / values.len() as f32).abs();

    if config.allow_lossy {
        if coefficient_of_variation < 0.01 {
            // Very uniform - use binary
            return QuantizationLevel::Binary;
        } else if range < 0.1 {
            // Small range - use int4
            return QuantizationLevel::Int4 {
                scale: range / 15.0,
                offset: min,
            };
        } else if range < 2.0 {
            // Medium range - use int8
            return QuantizationLevel::Int8 {
                scale: range / 255.0,
                offset: min,
            };
        } else {
            // Large range - use float16
            return QuantizationLevel::Float16;
        }
    }

    QuantizationLevel::None
}
```

### Tier 2: Entropy Coding

```rust
/// Entropy compression with algorithm selection
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum EntropyCodec {
    /// No entropy coding
    None,
    /// LZ4: Fastest, moderate compression
    Lz4 { level: i32 },
    /// Zstd: Balanced speed/compression
    Zstd { level: i32 },
    /// Brotli: Maximum compression (for cold storage)
    Brotli { level: u32 },
}

impl EntropyCodec {
    /// Compress data
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            EntropyCodec::None => Ok(data.to_vec()),

            EntropyCodec::Lz4 { level } => {
                let mut encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
                encoder.write_all(data)?;
                Ok(encoder.finish()?)
            }

            EntropyCodec::Zstd { level } => {
                Ok(zstd::encode_all(data, *level)?)
            }

            EntropyCodec::Brotli { level } => {
                let mut output = Vec::new();
                let mut params = brotli::enc::BrotliEncoderParams::default();
                params.quality = *level as i32;
                brotli::BrotliCompress(&mut data.as_ref(), &mut output, &params)?;
                Ok(output)
            }
        }
    }

    /// Decompress data
    pub fn decompress(&self, data: &[u8]) -> Result<Vec<u8>> {
        match self {
            EntropyCodec::None => Ok(data.to_vec()),

            EntropyCodec::Lz4 { .. } => {
                let mut decoder = lz4_flex::frame::FrameDecoder::new(data);
                let mut output = Vec::new();
                decoder.read_to_end(&mut output)?;
                Ok(output)
            }

            EntropyCodec::Zstd { .. } => {
                Ok(zstd::decode_all(data)?)
            }

            EntropyCodec::Brotli { .. } => {
                let mut output = Vec::new();
                brotli::BrotliDecompress(&mut data.as_ref(), &mut output)?;
                Ok(output)
            }
        }
    }
}

/// Select optimal entropy codec based on requirements
pub fn select_entropy_codec(
    size: usize,
    latency_budget: Duration,
    use_case: CompressionUseCase,
) -> EntropyCodec {
    match use_case {
        CompressionUseCase::RealTimeNetwork => {
            // Prioritize speed
            if size < 1024 {
                EntropyCodec::None // Overhead not worth it
            } else {
                EntropyCodec::Lz4 { level: 1 }
            }
        }

        CompressionUseCase::BatchNetwork => {
            // Balance speed and compression
            EntropyCodec::Zstd { level: 3 }
        }

        CompressionUseCase::HotStorage => {
            // Fast decompression
            EntropyCodec::Lz4 { level: 9 }
        }

        CompressionUseCase::ColdStorage => {
            // Maximum compression
            EntropyCodec::Brotli { level: 6 }
        }

        CompressionUseCase::Archive => {
            // Maximum compression, slow is OK
            EntropyCodec::Brotli { level: 11 }
        }
    }
}
```

### Tier 3: Batch Compression

```rust
/// Batch-level compression optimizations
pub struct BatchCompressor {
    /// Shared dictionary for string compression
    string_dict: DeltaDictionary,
    /// Value pattern dictionary
    value_patterns: PatternDictionary,
    /// Deduplication table
    dedup_table: DashMap<DeltaHash, DeltaId>,
    /// Configuration
    config: BatchCompressionConfig,
}

impl BatchCompressor {
    /// Compress a batch of deltas
    pub fn compress_batch(&self, deltas: &[VectorDelta]) -> Result<CompressedBatch> {
        // Step 1: Deduplication
        let (unique_deltas, dedup_refs) = self.deduplicate(deltas);

        // Step 2: Extract common patterns
        let patterns = self.extract_patterns(&unique_deltas);

        // Step 3: Build batch-specific dictionary
        let batch_dict = self.build_batch_dictionary(&unique_deltas);

        // Step 4: Encode deltas using patterns and dictionary
        let encoded: Vec<_> = unique_deltas.iter()
            .map(|d| self.encode_with_context(d, &patterns, &batch_dict))
            .collect();

        // Step 5: Pack into batch format
        let packed = self.pack_batch(&encoded, &patterns, &batch_dict, &dedup_refs);

        // Step 6: Apply entropy coding
        let compressed = self.config.entropy_codec.compress(&packed)?;

        Ok(CompressedBatch {
            compressed_data: compressed,
            original_count: deltas.len(),
            unique_count: unique_deltas.len(),
            compression_ratio: deltas.len() as f32 * std::mem::size_of::<VectorDelta>() as f32
                / compressed.len() as f32,
        })
    }

    /// Deduplicate deltas (same vector, same operation)
    fn deduplicate(&self, deltas: &[VectorDelta]) -> (Vec<VectorDelta>, Vec<DedupRef>) {
        let mut unique = Vec::new();
        let mut refs = Vec::new();

        for delta in deltas {
            let hash = compute_delta_hash(delta);

            if let Some(existing_id) = self.dedup_table.get(&hash) {
                refs.push(DedupRef::Existing(*existing_id));
            } else {
                self.dedup_table.insert(hash, delta.delta_id.clone());
                refs.push(DedupRef::New(unique.len()));
                unique.push(delta.clone());
            }
        }

        (unique, refs)
    }

    /// Extract common patterns from deltas
    fn extract_patterns(&self, deltas: &[VectorDelta]) -> Vec<DeltaPattern> {
        // Find common index sets
        let mut index_freq: HashMap<Vec<u32>, u32> = HashMap::new();

        for delta in deltas {
            if let DeltaOperation::Sparse { indices, .. } = &delta.operation {
                *index_freq.entry(indices.clone()).or_insert(0) += 1;
            }
        }

        // Patterns that appear > threshold times
        index_freq.into_iter()
            .filter(|(_, count)| *count >= self.config.pattern_threshold)
            .map(|(indices, count)| DeltaPattern {
                indices,
                frequency: count,
            })
            .collect()
    }
}
```

---

## Compression Ratios and Speed

### Single Delta Compression

| Configuration | Ratio | Compress Time | Decompress Time |
|---------------|-------|---------------|-----------------|
| Encoding only | 1-10x | 5us | 2us |
| + Float16 | 2-20x | 15us | 8us |
| + Int8 | 4-40x | 20us | 10us |
| + LZ4 | 6-50x | 50us | 20us |
| + Zstd | 8-60x | 200us | 50us |

### Batch Compression (100 deltas)

| Configuration | Ratio | Compress Time | Decompress Time |
|---------------|-------|---------------|-----------------|
| Individual Zstd | 8x | 20ms | 5ms |
| Batch + Dedup | 15x | 5ms | 2ms |
| Batch + Patterns + Zstd | 25x | 8ms | 3ms |
| Batch + Full Pipeline | 40x | 12ms | 4ms |

### Network vs Storage Tradeoffs

| Use Case | Target Ratio | Max Latency | Recommended |
|----------|--------------|-------------|-------------|
| Real-time sync | >3x | <1ms | Encode + LZ4 |
| Batch sync | >10x | <100ms | Batch + Zstd |
| Hot storage | >5x | <10ms | Encode + Zstd |
| Cold storage | >20x | <1s | Full pipeline + Brotli |
| Archive | >50x | N/A | Max compression |

---

## Considered Options

### Option 1: Single Codec (LZ4/Zstd)

**Description**: Apply one compression algorithm to everything.

**Pros**:
- Simple implementation
- Predictable performance
- No decision overhead

**Cons**:
- Suboptimal for varied data
- Misses domain-specific opportunities
- Either too slow or poor ratio

**Verdict**: Rejected - vectors benefit from tiered approach.

### Option 2: Learned Compression

**Description**: ML model learns optimal compression.

**Pros**:
- Potentially optimal compression
- Adapts to data patterns

**Cons**:
- Training complexity
- Inference overhead
- Hard to debug

**Verdict**: Deferred - consider for future version.

### Option 3: Delta-Specific Codecs

**Description**: Custom codec designed for vector deltas.

**Pros**:
- Maximum compression for vectors
- No general overhead

**Cons**:
- Development effort
- Maintenance burden
- Limited reuse

**Verdict**: Partially adopted - value quantization is delta-specific.

### Option 4: Multi-Tier Pipeline (Selected)

**Description**: Layer encoding, quantization, and entropy coding.

**Pros**:
- Each tier optimized for its purpose
- Configurable tradeoffs
- Reuses proven components

**Cons**:
- Configuration complexity
- Multiple code paths

**Verdict**: Adopted - best balance of compression and flexibility.

---

## Technical Specification

### Compression API

```rust
/// Delta compression pipeline
pub struct CompressionPipeline {
    /// Encoding configuration
    encoding: EncodingConfig,
    /// Quantization settings
    quantization: QuantizationConfig,
    /// Entropy codec
    entropy: EntropyCodec,
    /// Batch compression (optional)
    batch: Option<BatchCompressor>,
}

impl CompressionPipeline {
    /// Compress a single delta
    pub fn compress(&self, delta: &VectorDelta) -> Result<CompressedDelta> {
        // Tier 0: Encoding
        let encoded = encode_delta(&delta.operation, &self.encoding);

        // Tier 1: Quantization
        let quantized = quantize_encoded(&encoded, &self.quantization);

        // Tier 2: Entropy coding
        let compressed = self.entropy.compress(&quantized.to_bytes())?;

        Ok(CompressedDelta {
            delta_id: delta.delta_id.clone(),
            vector_id: delta.vector_id.clone(),
            metadata: compress_metadata(&delta, &self.encoding),
            compressed_data: compressed,
            original_size: estimated_delta_size(delta),
        })
    }

    /// Decompress a single delta
    pub fn decompress(&self, compressed: &CompressedDelta) -> Result<VectorDelta> {
        // Reverse: entropy -> quantization -> encoding
        let decoded_bytes = self.entropy.decompress(&compressed.compressed_data)?;
        let dequantized = dequantize(&decoded_bytes, &self.quantization);
        let operation = decode_delta(&dequantized, &self.encoding)?;

        Ok(VectorDelta {
            delta_id: compressed.delta_id.clone(),
            vector_id: compressed.vector_id.clone(),
            operation,
            ..decompress_metadata(&compressed.metadata)?
        })
    }

    /// Compress batch of deltas
    pub fn compress_batch(&self, deltas: &[VectorDelta]) -> Result<CompressedBatch> {
        match &self.batch {
            Some(batch_compressor) => batch_compressor.compress_batch(deltas),
            None => {
                // Fall back to individual compression
                let compressed: Vec<_> = deltas.iter()
                    .map(|d| self.compress(d))
                    .collect::<Result<_>>()?;
                Ok(CompressedBatch::from_individuals(compressed))
            }
        }
    }
}
```

### Configuration

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressionConfig {
    /// Enable/disable tiers
    pub enable_quantization: bool,
    pub enable_entropy: bool,
    pub enable_batch: bool,

    /// Quantization settings
    pub quantization: QuantizationConfig,

    /// Entropy codec selection
    pub entropy_codec: EntropyCodec,

    /// Batch compression settings
    pub batch_config: BatchCompressionConfig,

    /// Compression level presets
    pub preset: CompressionPreset,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum CompressionPreset {
    /// Minimize latency
    Fastest,
    /// Balance speed and ratio
    Balanced,
    /// Maximize compression
    Maximum,
    /// Custom configuration
    Custom,
}

impl Default for CompressionConfig {
    fn default() -> Self {
        Self {
            enable_quantization: true,
            enable_entropy: true,
            enable_batch: true,
            quantization: QuantizationConfig::default(),
            entropy_codec: EntropyCodec::Zstd { level: 3 },
            batch_config: BatchCompressionConfig::default(),
            preset: CompressionPreset::Balanced,
        }
    }
}
```

---

## Consequences

### Benefits

1. **High Compression**: 5-50x reduction in storage and network
2. **Configurable**: Choose speed vs ratio tradeoff
3. **Adaptive**: Automatic format selection
4. **Streaming**: Works with real-time delta flows
5. **WASM Compatible**: All codecs work in browser

### Risks and Mitigations

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Compression overhead | Medium | Medium | Fast path for small deltas |
| Quality loss | Low | High | Lossless option always available |
| Codec incompatibility | Low | Medium | Version headers, fallback |
| Memory pressure | Medium | Medium | Streaming decompression |

---

## References

1. Lemire, D., & Boytsov, L. "Decoding billions of integers per second through vectorization."
2. LZ4 Frame Format. https://github.com/lz4/lz4/blob/dev/doc/lz4_Frame_format.md
3. Zstandard Compression. https://facebook.github.io/zstd/
4. ADR-DB-002: Delta Encoding Format

---

## Related Decisions

- **ADR-DB-001**: Delta Behavior Core Architecture
- **ADR-DB-002**: Delta Encoding Format
- **ADR-DB-003**: Delta Propagation Protocol
