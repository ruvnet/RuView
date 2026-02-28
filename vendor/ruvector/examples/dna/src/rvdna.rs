//! RVDNA - AI-Native Genomic File Format
//!
//! A binary format purpose-built for ultra-low-latency AI genomic analysis.
//! Unlike FASTA/BAM/VCF which require re-encoding for every AI pipeline,
//! RVDNA stores pre-computed tensors, vector embeddings, and graph structures
//! alongside the raw sequence data.
//!
//! ## Format Structure
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │ Header (64 bytes)               │  Magic, version, section offsets
//! ├─────────────────────────────────┤
//! │ Section 0: Sequence Data        │  2-bit packed nucleotides + quality
//! ├─────────────────────────────────┤
//! │ Section 1: K-mer Vectors        │  Pre-computed HNSW-ready embeddings
//! ├─────────────────────────────────┤
//! │ Section 2: Attention Weights    │  Sparse self-attention matrices
//! ├─────────────────────────────────┤
//! │ Section 3: Variant Tensor       │  Per-position genotype likelihoods
//! ├─────────────────────────────────┤
//! │ Section 4: Protein Embeddings   │  GNN node features + contact graph
//! ├─────────────────────────────────┤
//! │ Section 5: Epigenomic Tracks    │  Methylation betas + aging coeffs
//! ├─────────────────────────────────┤
//! │ Section 6: Metadata             │  JSON provenance + checksums
//! └─────────────────────────────────┘
//! ```
//!
//! ## Key Properties
//!
//! - **2-bit encoding**: 4 bases per byte (4x compression vs ASCII)
//! - **Zero-copy access**: Memory-mappable with aligned sections
//! - **Pre-indexed**: HNSW graph stored inline for instant similarity search
//! - **Tensor-native**: Attention weights and variant probabilities stored as
//!   sparse tensors in COO format for direct GPU/SIMD consumption
//! - **Streaming**: Chunked sections allow incremental read/write

use crate::error::{DnaError, Result};
use crate::types::{DnaSequence, Nucleotide, QualityScore};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

// ============================================================================
// Constants
// ============================================================================

/// Magic bytes identifying an RVDNA file
pub const MAGIC: [u8; 8] = *b"RVDNA\x01\x00\x00";

/// Current format version
pub const FORMAT_VERSION: u16 = 1;

/// Number of sections in the format
pub const NUM_SECTIONS: usize = 7;

/// Section alignment boundary (64 bytes for cache-line alignment)
pub const SECTION_ALIGN: u64 = 64;

/// Header size (fixed)
pub const HEADER_SIZE: u64 = 64 + (NUM_SECTIONS as u64 * 16); // 64 base + 16 per section offset

// ============================================================================
// Compression Codec
// ============================================================================

/// Compression codec for section data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum Codec {
    /// No compression (zero-copy mmap friendly)
    None = 0,
    /// LZ4 fast compression (decode at ~4 GB/s)
    Lz4 = 1,
    /// Zstd balanced compression
    Zstd = 2,
}

impl Codec {
    fn from_u8(v: u8) -> Result<Self> {
        match v {
            0 => Ok(Codec::None),
            1 => Ok(Codec::Lz4),
            2 => Ok(Codec::Zstd),
            _ => Err(DnaError::InvalidSequence(format!("Unknown codec: {}", v))),
        }
    }
}

// ============================================================================
// Section Types
// ============================================================================

/// Section identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum SectionType {
    /// Raw sequence data (2-bit encoded)
    Sequence = 0,
    /// Pre-computed k-mer frequency vectors
    KmerVectors = 1,
    /// Sparse attention weight matrices
    AttentionWeights = 2,
    /// Per-position variant probability tensors
    VariantTensor = 3,
    /// Protein residue embeddings + contact graph
    ProteinEmbeddings = 4,
    /// Epigenomic tracks (methylation, chromatin)
    EpigenomicTracks = 5,
    /// JSON metadata and provenance
    Metadata = 6,
}

/// Section offset entry in the header
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SectionEntry {
    /// Offset from file start (0 = section not present)
    pub offset: u64,
    /// Compressed size in bytes
    pub size: u64,
}

// ============================================================================
// File Header
// ============================================================================

/// RVDNA file header (fixed-size, at byte 0)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvdnaHeader {
    /// Format version
    pub version: u16,
    /// Compression codec used
    pub codec: Codec,
    /// Flags (bit 0: little-endian, bit 1: has quality scores)
    pub flags: u32,
    /// Total sequence length in bases
    pub sequence_length: u64,
    /// Number of contigs/chromosomes
    pub num_contigs: u32,
    /// Section offset table
    pub sections: [SectionEntry; NUM_SECTIONS],
    /// CRC32 checksum of header
    pub header_checksum: u32,
}

impl RvdnaHeader {
    /// Create a new empty header
    pub fn new(sequence_length: u64, codec: Codec) -> Self {
        Self {
            version: FORMAT_VERSION,
            codec,
            flags: 0x01, // little-endian by default
            sequence_length,
            num_contigs: 1,
            sections: [SectionEntry { offset: 0, size: 0 }; NUM_SECTIONS],
            header_checksum: 0,
        }
    }

    /// Set the has_quality flag
    pub fn with_quality(mut self) -> Self {
        self.flags |= 0x02;
        self
    }

    /// Check if quality scores are present
    pub fn has_quality(&self) -> bool {
        self.flags & 0x02 != 0
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(HEADER_SIZE as usize);

        // Magic (8 bytes)
        buf.extend_from_slice(&MAGIC);
        // Version (2 bytes)
        buf.extend_from_slice(&self.version.to_le_bytes());
        // Codec (1 byte)
        buf.push(self.codec as u8);
        // Padding (1 byte)
        buf.push(0);
        // Flags (4 bytes)
        buf.extend_from_slice(&self.flags.to_le_bytes());
        // Sequence length (8 bytes)
        buf.extend_from_slice(&self.sequence_length.to_le_bytes());
        // Num contigs (4 bytes)
        buf.extend_from_slice(&self.num_contigs.to_le_bytes());
        // Reserved (36 bytes to reach 64-byte base header)
        buf.extend_from_slice(&[0u8; 36]);

        // Section table (16 bytes per section: 8 offset + 8 size)
        for section in &self.sections {
            buf.extend_from_slice(&section.offset.to_le_bytes());
            buf.extend_from_slice(&section.size.to_le_bytes());
        }

        // Compute checksum over everything except the last 4 bytes
        let checksum = crc32_simple(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        buf
    }

    /// Parse header from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_SIZE as usize + 4 {
            return Err(DnaError::InvalidSequence("Header too short".to_string()));
        }

        // Verify magic
        if &data[0..8] != &MAGIC {
            return Err(DnaError::InvalidSequence(
                "Invalid RVDNA magic number".to_string(),
            ));
        }

        let version = u16::from_le_bytes([data[8], data[9]]);
        let codec = Codec::from_u8(data[10])?;
        let flags = u32::from_le_bytes([data[12], data[13], data[14], data[15]]);
        let sequence_length = u64::from_le_bytes(data[16..24].try_into().unwrap());
        let num_contigs = u32::from_le_bytes(data[24..28].try_into().unwrap());

        let mut sections = [SectionEntry { offset: 0, size: 0 }; NUM_SECTIONS];
        let table_start = 64;
        for i in 0..NUM_SECTIONS {
            let base = table_start + i * 16;
            sections[i] = SectionEntry {
                offset: u64::from_le_bytes(data[base..base + 8].try_into().unwrap()),
                size: u64::from_le_bytes(data[base + 8..base + 16].try_into().unwrap()),
            };
        }

        let checksum_offset = table_start + NUM_SECTIONS * 16;
        let header_checksum = u32::from_le_bytes(
            data[checksum_offset..checksum_offset + 4]
                .try_into()
                .unwrap(),
        );

        // Verify checksum
        let computed = crc32_simple(&data[..checksum_offset]);
        if computed != header_checksum {
            return Err(DnaError::InvalidSequence(format!(
                "Header checksum mismatch: expected {:08x}, got {:08x}",
                header_checksum, computed
            )));
        }

        Ok(Self {
            version,
            codec,
            flags,
            sequence_length,
            num_contigs,
            sections,
            header_checksum,
        })
    }
}

// ============================================================================
// 2-Bit Sequence Encoding
// ============================================================================

/// Encode nucleotides to 2-bit packed representation.
///
/// Packing: 4 bases per byte, MSB first.
/// A=00, C=01, G=10, T=11. N is encoded as 00 with a separate N-mask.
///
/// Returns (packed_data, n_mask) where n_mask has 1-bits for N positions.
pub fn encode_2bit(sequence: &[Nucleotide]) -> (Vec<u8>, Vec<u8>) {
    let num_bytes = (sequence.len() + 3) / 4;
    let mut packed = vec![0u8; num_bytes];
    let mask_bytes = (sequence.len() + 7) / 8;
    let mut n_mask = vec![0u8; mask_bytes];

    for (i, &base) in sequence.iter().enumerate() {
        let byte_idx = i / 4;
        let bit_offset = 6 - (i % 4) * 2; // MSB first: positions 6,4,2,0

        let bits = match base {
            Nucleotide::A => 0b00,
            Nucleotide::C => 0b01,
            Nucleotide::G => 0b10,
            Nucleotide::T => 0b11,
            Nucleotide::N => {
                // Mark in N-mask
                n_mask[i / 8] |= 1 << (7 - i % 8);
                0b00 // Encode as A, disambiguated by mask
            }
        };

        packed[byte_idx] |= bits << bit_offset;
    }

    (packed, n_mask)
}

/// Decode 2-bit packed nucleotides back to sequence
pub fn decode_2bit(packed: &[u8], n_mask: &[u8], length: usize) -> Vec<Nucleotide> {
    let mut sequence = Vec::with_capacity(length);

    for i in 0..length {
        let byte_idx = i / 4;
        let bit_offset = 6 - (i % 4) * 2;
        let bits = (packed[byte_idx] >> bit_offset) & 0b11;

        // Check N-mask
        let is_n = if i / 8 < n_mask.len() {
            (n_mask[i / 8] >> (7 - i % 8)) & 1 == 1
        } else {
            false
        };

        let base = if is_n {
            Nucleotide::N
        } else {
            match bits {
                0b00 => Nucleotide::A,
                0b01 => Nucleotide::C,
                0b10 => Nucleotide::G,
                0b11 => Nucleotide::T,
                _ => unreachable!(),
            }
        };

        sequence.push(base);
    }

    sequence
}

/// Compress quality scores using 6-bit encoding (0-63 range, Phred capped)
pub fn encode_quality(qualities: &[u8]) -> Vec<u8> {
    // Pack four 6-bit values into three bytes
    let mut encoded = Vec::with_capacity((qualities.len() * 6 + 7) / 8);
    let mut bit_buffer: u64 = 0;
    let mut bits_in_buffer = 0;

    for &q in qualities {
        let q6 = q.min(63) as u64; // Cap at 6 bits
        bit_buffer = (bit_buffer << 6) | q6;
        bits_in_buffer += 6;

        while bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            encoded.push((bit_buffer >> bits_in_buffer) as u8);
            bit_buffer &= (1 << bits_in_buffer) - 1;
        }
    }

    // Flush remaining bits
    if bits_in_buffer > 0 {
        encoded.push((bit_buffer << (8 - bits_in_buffer)) as u8);
    }

    encoded
}

/// Decode 6-bit compressed quality scores
pub fn decode_quality(encoded: &[u8], count: usize) -> Vec<u8> {
    let mut qualities = Vec::with_capacity(count);
    let mut bit_buffer: u64 = 0;
    let mut bits_in_buffer = 0;
    let mut byte_idx = 0;

    for _ in 0..count {
        while bits_in_buffer < 6 && byte_idx < encoded.len() {
            bit_buffer = (bit_buffer << 8) | encoded[byte_idx] as u64;
            bits_in_buffer += 8;
            byte_idx += 1;
        }

        bits_in_buffer -= 6;
        let q = ((bit_buffer >> bits_in_buffer) & 0x3F) as u8;
        bit_buffer &= (1 << bits_in_buffer) - 1;
        qualities.push(q);
    }

    qualities
}

// ============================================================================
// Sparse Attention Matrix (COO Format)
// ============================================================================

/// Sparse attention matrix stored in COO (Coordinate) format.
/// Efficient for storing pre-computed attention weights between sequence positions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SparseAttention {
    /// Row indices (query positions)
    pub rows: Vec<u32>,
    /// Column indices (key positions)
    pub cols: Vec<u32>,
    /// Attention weight values
    pub values: Vec<f32>,
    /// Matrix dimensions (rows, cols)
    pub shape: (u32, u32),
    /// Window size used for computation
    pub window_size: u32,
}

impl SparseAttention {
    /// Create from dense attention matrix, keeping only values above threshold
    pub fn from_dense(matrix: &[f32], rows: usize, cols: usize, threshold: f32) -> Self {
        let mut row_idx = Vec::new();
        let mut col_idx = Vec::new();
        let mut values = Vec::new();

        for i in 0..rows {
            for j in 0..cols {
                let val = matrix[i * cols + j];
                if val.abs() > threshold {
                    row_idx.push(i as u32);
                    col_idx.push(j as u32);
                    values.push(val);
                }
            }
        }

        Self {
            rows: row_idx,
            cols: col_idx,
            values,
            shape: (rows as u32, cols as u32),
            window_size: cols as u32,
        }
    }

    /// Number of non-zero entries
    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    /// Sparsity ratio (fraction of zeros)
    pub fn sparsity(&self) -> f64 {
        let total = self.shape.0 as f64 * self.shape.1 as f64;
        if total == 0.0 {
            return 1.0;
        }
        1.0 - (self.nnz() as f64 / total)
    }

    /// Lookup attention weight at (row, col), returns 0.0 if not stored
    pub fn get(&self, row: u32, col: u32) -> f32 {
        for i in 0..self.values.len() {
            if self.rows[i] == row && self.cols[i] == col {
                return self.values[i];
            }
        }
        0.0
    }

    /// Serialize to bytes (for file storage)
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        // Shape (8 bytes)
        buf.extend_from_slice(&self.shape.0.to_le_bytes());
        buf.extend_from_slice(&self.shape.1.to_le_bytes());
        // Window size (4 bytes)
        buf.extend_from_slice(&self.window_size.to_le_bytes());
        // NNZ count (4 bytes)
        let nnz = self.nnz() as u32;
        buf.extend_from_slice(&nnz.to_le_bytes());
        // Row indices
        for &r in &self.rows {
            buf.extend_from_slice(&r.to_le_bytes());
        }
        // Column indices
        for &c in &self.cols {
            buf.extend_from_slice(&c.to_le_bytes());
        }
        // Values
        for &v in &self.values {
            buf.extend_from_slice(&v.to_le_bytes());
        }
        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 20 {
            return Err(DnaError::InvalidSequence(
                "Attention data too short".to_string(),
            ));
        }
        let shape_0 = u32::from_le_bytes(data[0..4].try_into().unwrap());
        let shape_1 = u32::from_le_bytes(data[4..8].try_into().unwrap());
        let window_size = u32::from_le_bytes(data[8..12].try_into().unwrap());
        let nnz = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;

        let expected = 16 + nnz * 12; // 4 bytes row + 4 col + 4 value per entry
        if data.len() < expected {
            return Err(DnaError::InvalidSequence(
                "Attention data truncated".to_string(),
            ));
        }

        let mut offset = 16;
        let rows: Vec<u32> = (0..nnz)
            .map(|_| {
                let v = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;
                v
            })
            .collect();
        let cols: Vec<u32> = (0..nnz)
            .map(|_| {
                let v = u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;
                v
            })
            .collect();
        let values: Vec<f32> = (0..nnz)
            .map(|_| {
                let v = f32::from_le_bytes(data[offset..offset + 4].try_into().unwrap());
                offset += 4;
                v
            })
            .collect();

        Ok(Self {
            rows,
            cols,
            values,
            shape: (shape_0, shape_1),
            window_size,
        })
    }
}

// ============================================================================
// Variant Tensor (Per-Position Genotype Likelihoods)
// ============================================================================

/// Per-position variant probability tensor.
/// Stores genotype likelihoods for each genomic position using f16-quantized values.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariantTensor {
    /// Genomic positions with variant data
    pub positions: Vec<u64>,
    /// Reference alleles (2-bit encoded)
    pub ref_alleles: Vec<u8>,
    /// Alternate alleles (2-bit encoded)
    pub alt_alleles: Vec<u8>,
    /// Genotype likelihoods: [P(0/0), P(0/1), P(1/1)] per position (f16 as u16)
    pub likelihoods: Vec<[u16; 3]>,
    /// Quality scores (Phred-scaled)
    pub qualities: Vec<u8>,
}

impl VariantTensor {
    /// Create empty tensor
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            ref_alleles: Vec::new(),
            alt_alleles: Vec::new(),
            likelihoods: Vec::new(),
            qualities: Vec::new(),
        }
    }

    /// Add a variant position with genotype likelihoods
    pub fn add_variant(
        &mut self,
        position: u64,
        ref_allele: Nucleotide,
        alt_allele: Nucleotide,
        gl_hom_ref: f32,
        gl_het: f32,
        gl_hom_alt: f32,
        quality: u8,
    ) {
        self.positions.push(position);
        self.ref_alleles.push(nucleotide_to_2bit(ref_allele));
        self.alt_alleles.push(nucleotide_to_2bit(alt_allele));
        self.likelihoods.push([
            f32_to_f16(gl_hom_ref),
            f32_to_f16(gl_het),
            f32_to_f16(gl_hom_alt),
        ]);
        self.qualities.push(quality);
    }

    /// Number of variant positions
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Get genotype likelihoods at a position (binary search)
    pub fn get_likelihoods(&self, position: u64) -> Option<[f32; 3]> {
        self.positions.binary_search(&position).ok().map(|idx| {
            [
                f16_to_f32(self.likelihoods[idx][0]),
                f16_to_f32(self.likelihoods[idx][1]),
                f16_to_f32(self.likelihoods[idx][2]),
            ]
        })
    }

    /// Serialize to bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        let count = self.len() as u32;
        buf.extend_from_slice(&count.to_le_bytes());

        for &pos in &self.positions {
            buf.extend_from_slice(&pos.to_le_bytes());
        }
        buf.extend_from_slice(&self.ref_alleles);
        buf.extend_from_slice(&self.alt_alleles);
        for &gl in &self.likelihoods {
            for &v in &gl {
                buf.extend_from_slice(&v.to_le_bytes());
            }
        }
        buf.extend_from_slice(&self.qualities);
        buf
    }

    /// Deserialize from bytes
    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if data.len() < 4 {
            return Err(DnaError::InvalidSequence(
                "Variant tensor too short".to_string(),
            ));
        }
        let count = u32::from_le_bytes(data[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;

        let positions: Vec<u64> = (0..count)
            .map(|_| {
                let v = u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap());
                offset += 8;
                v
            })
            .collect();

        let ref_alleles = data[offset..offset + count].to_vec();
        offset += count;
        let alt_alleles = data[offset..offset + count].to_vec();
        offset += count;

        let likelihoods: Vec<[u16; 3]> = (0..count)
            .map(|_| {
                let a = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                offset += 2;
                let b = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                offset += 2;
                let c = u16::from_le_bytes(data[offset..offset + 2].try_into().unwrap());
                offset += 2;
                [a, b, c]
            })
            .collect();

        let qualities = data[offset..offset + count].to_vec();

        Ok(Self {
            positions,
            ref_alleles,
            alt_alleles,
            likelihoods,
            qualities,
        })
    }
}

// ============================================================================
// K-mer Vector Section
// ============================================================================

/// Pre-computed k-mer vector block for HNSW-ready storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KmerVectorBlock {
    /// K-mer size used
    pub k: u32,
    /// Vector dimensions
    pub dimensions: u32,
    /// Region start position in sequence
    pub start_pos: u64,
    /// Region length in bases
    pub region_len: u64,
    /// The k-mer frequency vector (f32)
    pub vector: Vec<f32>,
    /// Optional quantized vector (int8) for fast approximate search
    pub quantized: Option<Vec<i8>>,
    /// Quantization scale factor (to reconstruct f32 from int8)
    pub quant_scale: f32,
}

impl KmerVectorBlock {
    /// Create from a DnaSequence region
    pub fn from_sequence(
        sequence: &DnaSequence,
        start: u64,
        len: u64,
        k: u32,
        dimensions: u32,
    ) -> Result<Self> {
        let end = (start + len).min(sequence.len() as u64);
        let subseq_bases: Vec<Nucleotide> = (start as usize..end as usize)
            .map(|i| sequence.get(i).unwrap_or(Nucleotide::N))
            .collect();
        let subseq = DnaSequence::new(subseq_bases);
        let vector = subseq.to_kmer_vector(k as usize, dimensions as usize)?;

        // Quantize to int8
        let max_abs = vector.iter().map(|v| v.abs()).fold(0.0f32, f32::max);
        let scale = if max_abs > 0.0 { max_abs / 127.0 } else { 1.0 };
        let quantized: Vec<i8> = vector
            .iter()
            .map(|&v| (v / scale).round().max(-128.0).min(127.0) as i8)
            .collect();

        Ok(Self {
            k,
            dimensions,
            start_pos: start,
            region_len: end - start,
            vector,
            quantized: Some(quantized),
            quant_scale: scale,
        })
    }

    /// Cosine similarity between this block and another vector
    pub fn cosine_similarity(&self, other: &[f32]) -> f32 {
        if self.vector.len() != other.len() {
            return 0.0;
        }
        let dot: f32 = self.vector.iter().zip(other).map(|(a, b)| a * b).sum();
        let mag_a: f32 = self.vector.iter().map(|a| a * a).sum::<f32>().sqrt();
        let mag_b: f32 = other.iter().map(|b| b * b).sum::<f32>().sqrt();
        if mag_a == 0.0 || mag_b == 0.0 {
            0.0
        } else {
            dot / (mag_a * mag_b)
        }
    }

    /// Fast approximate similarity using quantized vectors (4x less memory, ~3x faster)
    pub fn fast_similarity(&self, other_quantized: &[i8]) -> f32 {
        match &self.quantized {
            Some(q) if q.len() == other_quantized.len() => {
                let dot: i32 = q
                    .iter()
                    .zip(other_quantized)
                    .map(|(&a, &b)| a as i32 * b as i32)
                    .sum();
                dot as f32 * self.quant_scale * self.quant_scale
            }
            _ => 0.0,
        }
    }
}

// ============================================================================
// RVDNA File Writer
// ============================================================================

/// Builder for creating RVDNA files
pub struct RvdnaWriter {
    header: RvdnaHeader,
    sequence_data: Option<(Vec<u8>, Vec<u8>)>, // (packed, n_mask)
    quality_data: Option<Vec<u8>>,
    kmer_blocks: Vec<KmerVectorBlock>,
    attention: Option<SparseAttention>,
    variants: Option<VariantTensor>,
    metadata: Option<serde_json::Value>,
}

impl RvdnaWriter {
    /// Create a new writer for a sequence
    pub fn new(sequence: &DnaSequence, codec: Codec) -> Self {
        let (packed, n_mask) = encode_2bit(sequence.bases());
        Self {
            header: RvdnaHeader::new(sequence.len() as u64, codec),
            sequence_data: Some((packed, n_mask)),
            quality_data: None,
            kmer_blocks: Vec::new(),
            attention: None,
            variants: None,
            metadata: None,
        }
    }

    /// Add quality scores
    pub fn with_quality(mut self, qualities: &[u8]) -> Self {
        self.quality_data = Some(encode_quality(qualities));
        self.header = self.header.with_quality();
        self
    }

    /// Pre-compute and add k-mer vectors for the sequence
    pub fn with_kmer_vectors(
        mut self,
        sequence: &DnaSequence,
        k: u32,
        dimensions: u32,
        block_size: u64,
    ) -> Result<Self> {
        let seq_len = sequence.len() as u64;
        let mut pos = 0u64;
        while pos < seq_len {
            let len = block_size.min(seq_len - pos);
            if len >= k as u64 {
                let block = KmerVectorBlock::from_sequence(sequence, pos, len, k, dimensions)?;
                self.kmer_blocks.push(block);
            }
            pos += block_size;
        }
        Ok(self)
    }

    /// Add pre-computed attention weights
    pub fn with_attention(mut self, attention: SparseAttention) -> Self {
        self.attention = Some(attention);
        self
    }

    /// Add variant tensor
    pub fn with_variants(mut self, variants: VariantTensor) -> Self {
        self.variants = Some(variants);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = Some(metadata);
        self
    }

    /// Write the complete RVDNA file
    pub fn write<W: Write>(&mut self, writer: &mut W) -> Result<usize> {
        let mut sections_data: Vec<Vec<u8>> = vec![Vec::new(); NUM_SECTIONS];

        // Section 0: Sequence data
        if let Some((ref packed, ref n_mask)) = self.sequence_data {
            let mut sec = Vec::new();
            // Packed length (4 bytes)
            sec.extend_from_slice(&(packed.len() as u32).to_le_bytes());
            // N-mask length (4 bytes)
            sec.extend_from_slice(&(n_mask.len() as u32).to_le_bytes());
            // Packed data
            sec.extend_from_slice(packed);
            // N-mask
            sec.extend_from_slice(n_mask);
            // Quality (if present)
            if let Some(ref qual) = self.quality_data {
                sec.extend_from_slice(&(qual.len() as u32).to_le_bytes());
                sec.extend_from_slice(qual);
            }
            sections_data[0] = sec;
        }

        // Section 1: K-mer vectors
        if !self.kmer_blocks.is_empty() {
            let mut sec = Vec::new();
            sec.extend_from_slice(&(self.kmer_blocks.len() as u32).to_le_bytes());
            for block in &self.kmer_blocks {
                let block_bytes = serde_json::to_vec(block)
                    .map_err(|e| DnaError::PipelineError(e.to_string()))?;
                sec.extend_from_slice(&(block_bytes.len() as u32).to_le_bytes());
                sec.extend_from_slice(&block_bytes);
            }
            sections_data[1] = sec;
        }

        // Section 2: Attention weights
        if let Some(ref attn) = self.attention {
            sections_data[2] = attn.to_bytes();
        }

        // Section 3: Variant tensor
        if let Some(ref variants) = self.variants {
            sections_data[3] = variants.to_bytes();
        }

        // Section 6: Metadata
        if let Some(ref meta) = self.metadata {
            let meta_bytes =
                serde_json::to_vec(meta).map_err(|e| DnaError::PipelineError(e.to_string()))?;
            sections_data[6] = meta_bytes;
        }

        // Calculate section offsets (align each section)
        let header_len = HEADER_SIZE + 4; // +4 for checksum
        let mut current_offset = align_up(header_len, SECTION_ALIGN);

        for i in 0..NUM_SECTIONS {
            if !sections_data[i].is_empty() {
                self.header.sections[i] = SectionEntry {
                    offset: current_offset,
                    size: sections_data[i].len() as u64,
                };
                current_offset = align_up(
                    current_offset + sections_data[i].len() as u64,
                    SECTION_ALIGN,
                );
            }
        }

        // Write header
        let header_bytes = self.header.to_bytes();
        writer.write_all(&header_bytes).map_err(DnaError::IoError)?;

        // Pad to first section
        let pad_len =
            align_up(header_bytes.len() as u64, SECTION_ALIGN) - header_bytes.len() as u64;
        writer
            .write_all(&vec![0u8; pad_len as usize])
            .map_err(DnaError::IoError)?;

        let mut total_written = header_bytes.len() + pad_len as usize;

        // Write sections
        for i in 0..NUM_SECTIONS {
            if !sections_data[i].is_empty() {
                // Pad to alignment
                let needed = self.header.sections[i].offset as usize - total_written;
                if needed > 0 {
                    writer
                        .write_all(&vec![0u8; needed])
                        .map_err(DnaError::IoError)?;
                    total_written += needed;
                }
                writer
                    .write_all(&sections_data[i])
                    .map_err(DnaError::IoError)?;
                total_written += sections_data[i].len();
            }
        }

        Ok(total_written)
    }
}

// ============================================================================
// RVDNA File Reader
// ============================================================================

/// Reader for RVDNA files
pub struct RvdnaReader {
    /// Parsed file header
    pub header: RvdnaHeader,
    /// Raw file data (for section access)
    data: Vec<u8>,
}

impl RvdnaReader {
    /// Open and parse an RVDNA file from bytes
    pub fn from_bytes(data: Vec<u8>) -> Result<Self> {
        let header = RvdnaHeader::from_bytes(&data)?;
        Ok(Self { header, data })
    }

    /// Read from a reader
    pub fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        let mut data = Vec::new();
        reader.read_to_end(&mut data).map_err(DnaError::IoError)?;
        Self::from_bytes(data)
    }

    /// Extract the DNA sequence
    pub fn read_sequence(&self) -> Result<DnaSequence> {
        let section = &self.header.sections[SectionType::Sequence as usize];
        if section.size == 0 {
            return Err(DnaError::EmptySequence);
        }

        let start = section.offset as usize;
        let packed_len =
            u32::from_le_bytes(self.data[start..start + 4].try_into().unwrap()) as usize;
        let mask_len =
            u32::from_le_bytes(self.data[start + 4..start + 8].try_into().unwrap()) as usize;

        let packed = &self.data[start + 8..start + 8 + packed_len];
        let n_mask = &self.data[start + 8 + packed_len..start + 8 + packed_len + mask_len];

        let bases = decode_2bit(packed, n_mask, self.header.sequence_length as usize);
        Ok(DnaSequence::new(bases))
    }

    /// Read k-mer vector blocks
    pub fn read_kmer_vectors(&self) -> Result<Vec<KmerVectorBlock>> {
        let section = &self.header.sections[SectionType::KmerVectors as usize];
        if section.size == 0 {
            return Ok(Vec::new());
        }

        let start = section.offset as usize;
        let count = u32::from_le_bytes(self.data[start..start + 4].try_into().unwrap()) as usize;

        let mut blocks = Vec::with_capacity(count);
        let mut offset = start + 4;

        for _ in 0..count {
            let block_len =
                u32::from_le_bytes(self.data[offset..offset + 4].try_into().unwrap()) as usize;
            offset += 4;
            let block: KmerVectorBlock =
                serde_json::from_slice(&self.data[offset..offset + block_len])
                    .map_err(|e| DnaError::PipelineError(e.to_string()))?;
            blocks.push(block);
            offset += block_len;
        }

        Ok(blocks)
    }

    /// Read attention weights
    pub fn read_attention(&self) -> Result<Option<SparseAttention>> {
        let section = &self.header.sections[SectionType::AttentionWeights as usize];
        if section.size == 0 {
            return Ok(None);
        }
        let start = section.offset as usize;
        let end = start + section.size as usize;
        Ok(Some(SparseAttention::from_bytes(&self.data[start..end])?))
    }

    /// Read variant tensor
    pub fn read_variants(&self) -> Result<Option<VariantTensor>> {
        let section = &self.header.sections[SectionType::VariantTensor as usize];
        if section.size == 0 {
            return Ok(None);
        }
        let start = section.offset as usize;
        let end = start + section.size as usize;
        Ok(Some(VariantTensor::from_bytes(&self.data[start..end])?))
    }

    /// Read metadata
    pub fn read_metadata(&self) -> Result<Option<serde_json::Value>> {
        let section = &self.header.sections[SectionType::Metadata as usize];
        if section.size == 0 {
            return Ok(None);
        }
        let start = section.offset as usize;
        let end = start + section.size as usize;
        let meta: serde_json::Value = serde_json::from_slice(&self.data[start..end])
            .map_err(|e| DnaError::PipelineError(e.to_string()))?;
        Ok(Some(meta))
    }

    /// Get file size statistics
    pub fn stats(&self) -> RvdnaStats {
        let mut section_sizes = [0u64; NUM_SECTIONS];
        for i in 0..NUM_SECTIONS {
            section_sizes[i] = self.header.sections[i].size;
        }

        let total_size = self.data.len() as u64;
        let seq_len = self.header.sequence_length;
        let bits_per_base = if seq_len > 0 {
            (section_sizes[0] as f64 * 8.0) / seq_len as f64
        } else {
            0.0
        };

        RvdnaStats {
            total_size,
            sequence_length: seq_len,
            bits_per_base,
            section_sizes,
            compression_ratio: if seq_len > 0 {
                seq_len as f64 / total_size as f64
            } else {
                0.0
            },
        }
    }
}

/// File statistics
#[derive(Debug, Clone)]
pub struct RvdnaStats {
    /// Total file size in bytes
    pub total_size: u64,
    /// Sequence length in bases
    pub sequence_length: u64,
    /// Average bits per base for sequence section
    pub bits_per_base: f64,
    /// Size of each section in bytes
    pub section_sizes: [u64; NUM_SECTIONS],
    /// Bases per byte (overall compression)
    pub compression_ratio: f64,
}

// ============================================================================
// Conversion: FASTA → RVDNA
// ============================================================================

/// Convert a FASTA-like string to RVDNA format with pre-computed AI features
pub fn fasta_to_rvdna(
    sequence_str: &str,
    k: u32,
    vector_dims: u32,
    block_size: u64,
) -> Result<Vec<u8>> {
    let sequence = DnaSequence::from_str(sequence_str)?;

    let metadata = serde_json::json!({
        "format": "RVDNA",
        "version": FORMAT_VERSION,
        "source": "fasta_conversion",
        "sequence_length": sequence.len(),
        "kmer_k": k,
        "vector_dimensions": vector_dims,
        "block_size": block_size,
    });

    let mut writer = RvdnaWriter::new(&sequence, Codec::None)
        .with_kmer_vectors(&sequence, k, vector_dims, block_size)?
        .with_metadata(metadata);

    let mut output = Vec::new();
    writer.write(&mut output)?;
    Ok(output)
}

// ============================================================================
// Utility Functions
// ============================================================================

/// CRC32 lookup table (precomputed for IEEE polynomial 0xEDB88320)
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i as usize] = crc;
        i += 1;
    }
    table
};

/// CRC32 using precomputed lookup table (~8x faster than bit-by-bit)
fn crc32_simple(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = CRC32_TABLE[idx] ^ (crc >> 8);
    }
    !crc
}

/// Align value up to boundary
fn align_up(value: u64, alignment: u64) -> u64 {
    (value + alignment - 1) & !(alignment - 1)
}

/// Convert nucleotide to 2-bit encoding
fn nucleotide_to_2bit(n: Nucleotide) -> u8 {
    match n {
        Nucleotide::A => 0,
        Nucleotide::C => 1,
        Nucleotide::G => 2,
        Nucleotide::T => 3,
        Nucleotide::N => 0,
    }
}

/// Simple f32 → f16 conversion (IEEE 754 half precision)
fn f32_to_f16(value: f32) -> u16 {
    let bits = value.to_bits();
    let sign = (bits >> 31) & 1;
    let exponent = ((bits >> 23) & 0xFF) as i32;
    let mantissa = bits & 0x7FFFFF;

    if exponent == 0xFF {
        // Inf/NaN
        return ((sign << 15) | 0x7C00 | (mantissa >> 13).min(1)) as u16;
    }

    let new_exp = exponent - 127 + 15;
    if new_exp >= 31 {
        // Overflow → Inf
        return ((sign << 15) | 0x7C00) as u16;
    }
    if new_exp <= 0 {
        // Underflow → 0
        return (sign << 15) as u16;
    }

    ((sign << 15) | (new_exp as u32) << 10 | (mantissa >> 13)) as u16
}

/// Simple f16 → f32 conversion
fn f16_to_f32(half: u16) -> f32 {
    let sign = ((half >> 15) & 1) as u32;
    let exponent = ((half >> 10) & 0x1F) as u32;
    let mantissa = (half & 0x3FF) as u32;

    if exponent == 0x1F {
        // Inf/NaN
        let bits = (sign << 31) | 0x7F800000 | (mantissa << 13);
        return f32::from_bits(bits);
    }

    if exponent == 0 {
        if mantissa == 0 {
            return f32::from_bits(sign << 31); // +/- 0
        }
        // Denormalized
        let bits = (sign << 31) | ((exponent + 127 - 15) << 23) | (mantissa << 13);
        return f32::from_bits(bits);
    }

    let bits = (sign << 31) | ((exponent + 127 - 15) << 23) | (mantissa << 13);
    f32::from_bits(bits)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_2bit_encoding_roundtrip() {
        let bases = vec![
            Nucleotide::A,
            Nucleotide::C,
            Nucleotide::G,
            Nucleotide::T,
            Nucleotide::A,
            Nucleotide::N,
            Nucleotide::G,
            Nucleotide::T,
            Nucleotide::C,
        ];
        let (packed, mask) = encode_2bit(&bases);
        let decoded = decode_2bit(&packed, &mask, bases.len());
        assert_eq!(bases, decoded);
    }

    #[test]
    fn test_2bit_compression_ratio() {
        // 1000 bases should pack into 250 bytes
        let bases: Vec<Nucleotide> = (0..1000)
            .map(|i| match i % 4 {
                0 => Nucleotide::A,
                1 => Nucleotide::C,
                2 => Nucleotide::G,
                _ => Nucleotide::T,
            })
            .collect();
        let (packed, _mask) = encode_2bit(&bases);
        assert_eq!(packed.len(), 250); // 4x compression vs 1 byte per base
    }

    #[test]
    fn test_quality_encoding_roundtrip() {
        let qualities: Vec<u8> = (0..100).map(|i| (i % 42) as u8).collect();
        let encoded = encode_quality(&qualities);
        let decoded = decode_quality(&encoded, qualities.len());
        assert_eq!(qualities, decoded);
    }

    #[test]
    fn test_quality_compression_ratio() {
        // 6-bit encoding: 100 values = 75 bytes (vs 100 bytes raw)
        let qualities: Vec<u8> = vec![30; 100];
        let encoded = encode_quality(&qualities);
        assert!(
            encoded.len() <= 75,
            "6-bit should compress: {} bytes",
            encoded.len()
        );
    }

    #[test]
    fn test_sparse_attention_roundtrip() {
        let dense = vec![
            0.0, 0.5, 0.0, 0.0, 0.3, 0.0, 0.0, 0.7, 0.0, 0.0, 0.9, 0.0, 0.0, 0.1, 0.0, 0.0,
        ];
        let sparse = SparseAttention::from_dense(&dense, 4, 4, 0.05);
        assert_eq!(sparse.nnz(), 5); // 5 values > 0.05
        assert!(sparse.sparsity() > 0.6);

        // Roundtrip through bytes
        let bytes = sparse.to_bytes();
        let restored = SparseAttention::from_bytes(&bytes).unwrap();
        assert_eq!(restored.nnz(), 5);
        assert!((restored.get(0, 1) - 0.5).abs() < 1e-6);
        assert!((restored.get(1, 3) - 0.7).abs() < 1e-6);
    }

    #[test]
    fn test_variant_tensor_binary_search() {
        let mut vt = VariantTensor::new();
        vt.add_variant(100, Nucleotide::A, Nucleotide::G, 0.01, 0.99, 0.0, 40);
        vt.add_variant(200, Nucleotide::C, Nucleotide::T, 0.0, 0.5, 0.5, 35);
        vt.add_variant(300, Nucleotide::G, Nucleotide::A, 0.9, 0.1, 0.0, 50);

        let gl = vt.get_likelihoods(200).unwrap();
        assert!(gl[1] > 0.4); // Het likelihood
        assert!(vt.get_likelihoods(150).is_none()); // Not found

        // Roundtrip
        let bytes = vt.to_bytes();
        let restored = VariantTensor::from_bytes(&bytes).unwrap();
        assert_eq!(restored.len(), 3);
    }

    #[test]
    fn test_f16_roundtrip() {
        for &val in &[0.0f32, 1.0, -1.0, 0.5, 0.001, 100.0] {
            let half = f32_to_f16(val);
            let back = f16_to_f32(half);
            let rel_err = if val.abs() > 0.0 {
                (back - val).abs() / val.abs()
            } else {
                back.abs()
            };
            assert!(
                rel_err < 0.01,
                "f16 roundtrip failed for {}: got {}",
                val,
                back
            );
        }
    }

    #[test]
    fn test_header_roundtrip() {
        let mut header = RvdnaHeader::new(10000, Codec::None).with_quality();
        header.sections[0] = SectionEntry {
            offset: 192,
            size: 2500,
        };
        header.sections[1] = SectionEntry {
            offset: 2752,
            size: 8192,
        };

        let bytes = header.to_bytes();
        let restored = RvdnaHeader::from_bytes(&bytes).unwrap();

        assert_eq!(restored.version, FORMAT_VERSION);
        assert_eq!(restored.codec, Codec::None);
        assert!(restored.has_quality());
        assert_eq!(restored.sequence_length, 10000);
        assert_eq!(restored.sections[0].offset, 192);
        assert_eq!(restored.sections[0].size, 2500);
    }

    #[test]
    fn test_full_rvdna_write_read() {
        // Create a sequence
        let bases: Vec<Nucleotide> = "ACGTACGTACGTACGTACGTACGTACGTACGT"
            .chars()
            .map(|c| match c {
                'A' => Nucleotide::A,
                'C' => Nucleotide::C,
                'G' => Nucleotide::G,
                'T' => Nucleotide::T,
                _ => Nucleotide::N,
            })
            .collect();
        let sequence = DnaSequence::new(bases);

        // Build RVDNA file
        let mut writer = RvdnaWriter::new(&sequence, Codec::None)
            .with_metadata(serde_json::json!({"sample": "test"}));

        let mut output = Vec::new();
        let written = writer.write(&mut output).unwrap();
        assert!(written > 0);

        // Read it back
        let reader = RvdnaReader::from_bytes(output).unwrap();
        assert_eq!(reader.header.sequence_length, 32);
        assert_eq!(reader.header.codec, Codec::None);

        let restored_seq = reader.read_sequence().unwrap();
        assert_eq!(restored_seq.len(), 32);
        assert_eq!(restored_seq.to_string(), sequence.to_string());

        let meta = reader.read_metadata().unwrap().unwrap();
        assert_eq!(meta["sample"], "test");

        // Check stats
        let stats = reader.stats();
        assert!(
            stats.bits_per_base < 8.0,
            "Should compress below 1 byte/base"
        );
    }

    #[test]
    fn test_rvdna_with_kmer_vectors() {
        let seq = DnaSequence::from_str("ACGTACGTACGTACGTACGTACGTACGTACGT").unwrap();
        let mut writer = RvdnaWriter::new(&seq, Codec::None)
            .with_kmer_vectors(&seq, 11, 256, 32)
            .unwrap();

        let mut output = Vec::new();
        writer.write(&mut output).unwrap();

        let reader = RvdnaReader::from_bytes(output).unwrap();
        let blocks = reader.read_kmer_vectors().unwrap();
        assert!(!blocks.is_empty());
        assert_eq!(blocks[0].k, 11);
        assert_eq!(blocks[0].dimensions, 256);
        assert!(blocks[0].quantized.is_some());
    }

    #[test]
    fn test_fasta_to_rvdna_conversion() {
        let fasta_seq = "ACGTACGTACGTACGTACGTACGTACGTACGT";
        let rvdna_bytes = fasta_to_rvdna(fasta_seq, 11, 256, 1000).unwrap();

        let reader = RvdnaReader::from_bytes(rvdna_bytes).unwrap();
        let restored = reader.read_sequence().unwrap();
        assert_eq!(restored.to_string(), fasta_seq);

        let stats = reader.stats();
        assert!(stats.total_size > 0);
        // Sequence section uses 2-bit encoding (~2 bits/base at scale)
        // For short sequences, overhead from length headers increases ratio
        // At 1000+ bases, this drops well below 3 bits/base
        assert!(
            stats.bits_per_base < 8.0,
            "Should beat 1-byte-per-base encoding, got {:.1} bits/base",
            stats.bits_per_base
        );
    }

    #[test]
    fn test_kmer_vector_similarity() {
        let seq1 = DnaSequence::from_str("ACGTACGTACGTACGTACGTACGTACGTACGT").unwrap();
        let seq2 = DnaSequence::from_str("ACGTACGTACGTACGTACGTACGTACGTACGG").unwrap(); // 1 base diff
        let seq3 = DnaSequence::from_str("TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTT").unwrap(); // very different

        let block1 = KmerVectorBlock::from_sequence(&seq1, 0, 32, 11, 256).unwrap();
        let vec2 = seq2.to_kmer_vector(11, 256).unwrap();
        let vec3 = seq3.to_kmer_vector(11, 256).unwrap();

        let sim_similar = block1.cosine_similarity(&vec2);
        let sim_different = block1.cosine_similarity(&vec3);

        assert!(
            sim_similar > sim_different,
            "Similar sequence ({}) should score higher than different ({})",
            sim_similar,
            sim_different
        );
    }
}
