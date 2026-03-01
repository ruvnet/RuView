//! Standalone RVF container builder and reader for WiFi-DensePose model packaging.
//!
//! Implements the RVF binary format (64-byte segment headers + payload) without
//! depending on the `rvf-wire` crate. Supports building `.rvf` files that package
//! model weights, metadata, and configuration into a single binary container.
//!
//! Wire format per segment:
//! - 64-byte header (see `SegmentHeader`)
//! - N-byte payload
//! - Zero-padding to next 64-byte boundary

use serde::{Deserialize, Serialize};
use std::io::Write;

// ── RVF format constants ────────────────────────────────────────────────────

/// Segment header magic: "RVFS" as big-endian u32 = 0x52564653.
const SEGMENT_MAGIC: u32 = 0x5256_4653;
/// Current segment format version.
const SEGMENT_VERSION: u8 = 1;
/// All segments are 64-byte aligned.
const SEGMENT_ALIGNMENT: usize = 64;
/// Fixed header size in bytes.
const SEGMENT_HEADER_SIZE: usize = 64;

// ── Segment type discriminators (subset relevant to DensePose models) ───────

/// Raw vector payloads (model weight embeddings).
const SEG_VEC: u8 = 0x01;
/// Segment directory / manifest.
const SEG_MANIFEST: u8 = 0x05;
/// Quantization dictionaries and codebooks.
const SEG_QUANT: u8 = 0x06;
/// Arbitrary key-value metadata (JSON).
const SEG_META: u8 = 0x07;
/// Capability manifests, proof of computation, audit trails.
const SEG_WITNESS: u8 = 0x0A;
/// Domain profile declarations.
const SEG_PROFILE: u8 = 0x0B;
/// Contrastive embedding model weights and configuration (ADR-024).
pub const SEG_EMBED: u8 = 0x0C;
/// LoRA adaptation profile (named LoRA weight sets for environment-specific fine-tuning).
pub const SEG_LORA: u8 = 0x0D;

// ── Pure-Rust CRC32 (IEEE 802.3 polynomial) ────────────────────────────────

/// CRC32 lookup table, computed at compile time via the IEEE 802.3 polynomial
/// 0xEDB88320 (bit-reversed representation of 0x04C11DB7).
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0u32;
    while i < 256 {
        let mut crc = i;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
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

/// Compute CRC32 (IEEE) over the given byte slice.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        let idx = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = (crc >> 8) ^ CRC32_TABLE[idx];
    }
    crc ^ 0xFFFF_FFFF
}

/// Produce a 16-byte content hash field from CRC32.
/// The 4-byte CRC is stored in the first 4 bytes (little-endian), remaining
/// 12 bytes are zeroed.
fn crc32_content_hash(data: &[u8]) -> [u8; 16] {
    let c = crc32(data);
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&c.to_le_bytes());
    out
}

// ── Segment header (mirrors rvf-types SegmentHeader layout) ─────────────────

/// 64-byte segment header matching the RVF wire format exactly.
///
/// Field offsets:
/// - 0x00: magic (u32)
/// - 0x04: version (u8)
/// - 0x05: seg_type (u8)
/// - 0x06: flags (u16)
/// - 0x08: segment_id (u64)
/// - 0x10: payload_length (u64)
/// - 0x18: timestamp_ns (u64)
/// - 0x20: checksum_algo (u8)
/// - 0x21: compression (u8)
/// - 0x22: reserved_0 (u16)
/// - 0x24: reserved_1 (u32)
/// - 0x28: content_hash ([u8; 16])
/// - 0x38: uncompressed_len (u32)
/// - 0x3C: alignment_pad (u32)
#[derive(Clone, Debug)]
pub struct SegmentHeader {
    pub magic: u32,
    pub version: u8,
    pub seg_type: u8,
    pub flags: u16,
    pub segment_id: u64,
    pub payload_length: u64,
    pub timestamp_ns: u64,
    pub checksum_algo: u8,
    pub compression: u8,
    pub reserved_0: u16,
    pub reserved_1: u32,
    pub content_hash: [u8; 16],
    pub uncompressed_len: u32,
    pub alignment_pad: u32,
}

impl SegmentHeader {
    /// Create a new header with the given type and segment ID.
    fn new(seg_type: u8, segment_id: u64) -> Self {
        Self {
            magic: SEGMENT_MAGIC,
            version: SEGMENT_VERSION,
            seg_type,
            flags: 0,
            segment_id,
            payload_length: 0,
            timestamp_ns: 0,
            checksum_algo: 0, // CRC32
            compression: 0,
            reserved_0: 0,
            reserved_1: 0,
            content_hash: [0u8; 16],
            uncompressed_len: 0,
            alignment_pad: 0,
        }
    }

    /// Serialize the header into exactly 64 bytes (little-endian).
    fn to_bytes(&self) -> [u8; 64] {
        let mut buf = [0u8; 64];
        buf[0x00..0x04].copy_from_slice(&self.magic.to_le_bytes());
        buf[0x04] = self.version;
        buf[0x05] = self.seg_type;
        buf[0x06..0x08].copy_from_slice(&self.flags.to_le_bytes());
        buf[0x08..0x10].copy_from_slice(&self.segment_id.to_le_bytes());
        buf[0x10..0x18].copy_from_slice(&self.payload_length.to_le_bytes());
        buf[0x18..0x20].copy_from_slice(&self.timestamp_ns.to_le_bytes());
        buf[0x20] = self.checksum_algo;
        buf[0x21] = self.compression;
        buf[0x22..0x24].copy_from_slice(&self.reserved_0.to_le_bytes());
        buf[0x24..0x28].copy_from_slice(&self.reserved_1.to_le_bytes());
        buf[0x28..0x38].copy_from_slice(&self.content_hash);
        buf[0x38..0x3C].copy_from_slice(&self.uncompressed_len.to_le_bytes());
        buf[0x3C..0x40].copy_from_slice(&self.alignment_pad.to_le_bytes());
        buf
    }

    /// Deserialize a header from exactly 64 bytes (little-endian).
    fn from_bytes(data: &[u8; 64]) -> Self {
        let mut content_hash = [0u8; 16];
        content_hash.copy_from_slice(&data[0x28..0x38]);

        Self {
            magic: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            version: data[0x04],
            seg_type: data[0x05],
            flags: u16::from_le_bytes([data[0x06], data[0x07]]),
            segment_id: u64::from_le_bytes(data[0x08..0x10].try_into().unwrap()),
            payload_length: u64::from_le_bytes(data[0x10..0x18].try_into().unwrap()),
            timestamp_ns: u64::from_le_bytes(data[0x18..0x20].try_into().unwrap()),
            checksum_algo: data[0x20],
            compression: data[0x21],
            reserved_0: u16::from_le_bytes([data[0x22], data[0x23]]),
            reserved_1: u32::from_le_bytes(data[0x24..0x28].try_into().unwrap()),
            content_hash,
            uncompressed_len: u32::from_le_bytes(data[0x38..0x3C].try_into().unwrap()),
            alignment_pad: u32::from_le_bytes(data[0x3C..0x40].try_into().unwrap()),
        }
    }
}

// ── Vital sign detector config ──────────────────────────────────────────────

/// Configuration for the WiFi-based vital sign detector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VitalSignConfig {
    /// Breathing rate band low bound (Hz).
    pub breathing_low_hz: f64,
    /// Breathing rate band high bound (Hz).
    pub breathing_high_hz: f64,
    /// Heart rate band low bound (Hz).
    pub heartrate_low_hz: f64,
    /// Heart rate band high bound (Hz).
    pub heartrate_high_hz: f64,
    /// Minimum subcarrier count for valid detection.
    pub min_subcarriers: u32,
    /// Window size in samples for spectral analysis.
    pub window_size: u32,
    /// Confidence threshold (0.0 - 1.0).
    pub confidence_threshold: f64,
}

impl Default for VitalSignConfig {
    fn default() -> Self {
        Self {
            breathing_low_hz: 0.1,
            breathing_high_hz: 0.5,
            heartrate_low_hz: 0.8,
            heartrate_high_hz: 2.0,
            min_subcarriers: 52,
            window_size: 512,
            confidence_threshold: 0.6,
        }
    }
}

// ── RVF container info (returned by the REST API) ───────────────────────────

/// Summary of a loaded RVF container, exposed via `/api/v1/model/info`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvfContainerInfo {
    pub segment_count: usize,
    pub total_size: usize,
    pub manifest: Option<serde_json::Value>,
    pub metadata: Option<serde_json::Value>,
    pub has_weights: bool,
    pub has_vital_config: bool,
    pub has_quant_info: bool,
    pub has_witness: bool,
}

// ── RVF Builder ─────────────────────────────────────────────────────────────

/// Builds an RVF container by accumulating segments and serializing them
/// into the binary format: `[header(64) | payload | padding]*`.
pub struct RvfBuilder {
    segments: Vec<(SegmentHeader, Vec<u8>)>,
    next_id: u64,
}

impl RvfBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            next_id: 0,
        }
    }

    /// Add a manifest segment with model metadata.
    pub fn add_manifest(&mut self, model_id: &str, version: &str, description: &str) {
        let manifest = serde_json::json!({
            "model_id": model_id,
            "version": version,
            "description": description,
            "format": "wifi-densepose-rvf",
            "created_at": chrono::Utc::now().to_rfc3339(),
        });
        let payload = serde_json::to_vec(&manifest).unwrap_or_default();
        self.push_segment(SEG_MANIFEST, &payload);
    }

    /// Add model weights as a Vec segment. Weights are serialized as
    /// little-endian f32 values.
    pub fn add_weights(&mut self, weights: &[f32]) {
        let mut payload = Vec::with_capacity(weights.len() * 4);
        for &w in weights {
            payload.extend_from_slice(&w.to_le_bytes());
        }
        self.push_segment(SEG_VEC, &payload);
    }

    /// Add metadata (arbitrary JSON key-value pairs).
    pub fn add_metadata(&mut self, metadata: &serde_json::Value) {
        let payload = serde_json::to_vec(metadata).unwrap_or_default();
        self.push_segment(SEG_META, &payload);
    }

    /// Add vital sign detector configuration as a Profile segment.
    pub fn add_vital_config(&mut self, config: &VitalSignConfig) {
        let payload = serde_json::to_vec(config).unwrap_or_default();
        self.push_segment(SEG_PROFILE, &payload);
    }

    /// Add quantization info as a Quant segment.
    pub fn add_quant_info(&mut self, quant_type: &str, scale: f32, zero_point: i32) {
        let info = serde_json::json!({
            "quant_type": quant_type,
            "scale": scale,
            "zero_point": zero_point,
        });
        let payload = serde_json::to_vec(&info).unwrap_or_default();
        self.push_segment(SEG_QUANT, &payload);
    }

    /// Add a raw segment with arbitrary type and payload.
    /// Used by `rvf_pipeline` for extended segment types.
    pub fn add_raw_segment(&mut self, seg_type: u8, payload: &[u8]) {
        self.push_segment(seg_type, payload);
    }

    /// Add a named LoRA adaptation profile (ADR-024 Phase 7).
    ///
    /// Segment format: `[name_len: u16 LE][name_bytes: UTF-8][weights: f32 LE...]`
    pub fn add_lora_profile(&mut self, name: &str, lora_weights: &[f32]) {
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u16;
        let mut payload = Vec::with_capacity(2 + name_bytes.len() + lora_weights.len() * 4);
        payload.extend_from_slice(&name_len.to_le_bytes());
        payload.extend_from_slice(name_bytes);
        for &w in lora_weights {
            payload.extend_from_slice(&w.to_le_bytes());
        }
        self.push_segment(SEG_LORA, &payload);
    }

    /// Add contrastive embedding config and projection head weights (ADR-024).
    /// Serializes embedding config as JSON followed by projection weights as f32 LE.
    pub fn add_embedding(&mut self, config_json: &serde_json::Value, proj_weights: &[f32]) {
        let config_bytes = serde_json::to_vec(config_json).unwrap_or_default();
        let config_len = config_bytes.len() as u32;
        let mut payload = Vec::with_capacity(4 + config_bytes.len() + proj_weights.len() * 4);
        payload.extend_from_slice(&config_len.to_le_bytes());
        payload.extend_from_slice(&config_bytes);
        for &w in proj_weights {
            payload.extend_from_slice(&w.to_le_bytes());
        }
        self.push_segment(SEG_EMBED, &payload);
    }

    /// Add witness/proof data as a Witness segment.
    pub fn add_witness(&mut self, training_hash: &str, metrics: &serde_json::Value) {
        let witness = serde_json::json!({
            "training_hash": training_hash,
            "metrics": metrics,
        });
        let payload = serde_json::to_vec(&witness).unwrap_or_default();
        self.push_segment(SEG_WITNESS, &payload);
    }

    /// Build the final `.rvf` file as a byte vector.
    pub fn build(&self) -> Vec<u8> {
        let total: usize = self
            .segments
            .iter()
            .map(|(_, p)| align_up(SEGMENT_HEADER_SIZE + p.len()))
            .sum();

        let mut buf = Vec::with_capacity(total);
        for (header, payload) in &self.segments {
            buf.extend_from_slice(&header.to_bytes());
            buf.extend_from_slice(payload);
            // Zero-pad to the next 64-byte boundary
            let written = SEGMENT_HEADER_SIZE + payload.len();
            let target = align_up(written);
            let pad = target - written;
            buf.extend(std::iter::repeat(0u8).take(pad));
        }
        buf
    }

    /// Write the container to a file.
    pub fn write_to_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let data = self.build();
        let mut file = std::fs::File::create(path)?;
        file.write_all(&data)?;
        file.flush()?;
        Ok(())
    }

    // ── internal helpers ────────────────────────────────────────────────────

    fn push_segment(&mut self, seg_type: u8, payload: &[u8]) {
        let id = self.next_id;
        self.next_id += 1;

        let content_hash = crc32_content_hash(payload);
        let raw = SEGMENT_HEADER_SIZE + payload.len();
        let aligned = align_up(raw);
        let pad = (aligned - raw) as u32;

        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);

        let header = SegmentHeader {
            magic: SEGMENT_MAGIC,
            version: SEGMENT_VERSION,
            seg_type,
            flags: 0,
            segment_id: id,
            payload_length: payload.len() as u64,
            timestamp_ns: now_ns,
            checksum_algo: 0, // CRC32
            compression: 0,
            reserved_0: 0,
            reserved_1: 0,
            content_hash,
            uncompressed_len: 0,
            alignment_pad: pad,
        };

        self.segments.push((header, payload.to_vec()));
    }
}

impl Default for RvfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Round `size` up to the next multiple of `SEGMENT_ALIGNMENT` (64).
fn align_up(size: usize) -> usize {
    (size + SEGMENT_ALIGNMENT - 1) & !(SEGMENT_ALIGNMENT - 1)
}

// ── RVF Reader ──────────────────────────────────────────────────────────────

/// Reads and parses an RVF container from bytes, providing access to
/// individual segments.
#[derive(Debug)]
pub struct RvfReader {
    segments: Vec<(SegmentHeader, Vec<u8>)>,
    raw_size: usize,
}

impl RvfReader {
    /// Parse an RVF container from a byte slice.
    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut segments = Vec::new();
        let mut offset = 0;

        while offset + SEGMENT_HEADER_SIZE <= data.len() {
            // Read the 64-byte header
            let header_bytes: &[u8; 64] = data[offset..offset + 64]
                .try_into()
                .map_err(|_| "truncated header".to_string())?;

            let header = SegmentHeader::from_bytes(header_bytes);

            // Validate magic
            if header.magic != SEGMENT_MAGIC {
                return Err(format!(
                    "invalid magic at offset {offset}: expected 0x{SEGMENT_MAGIC:08X}, \
                     got 0x{:08X}",
                    header.magic
                ));
            }

            // Validate version
            if header.version != SEGMENT_VERSION {
                return Err(format!(
                    "unsupported version at offset {offset}: expected {SEGMENT_VERSION}, \
                     got {}",
                    header.version
                ));
            }

            let payload_len = header.payload_length as usize;
            let payload_start = offset + SEGMENT_HEADER_SIZE;
            let payload_end = payload_start + payload_len;

            if payload_end > data.len() {
                return Err(format!(
                    "truncated payload at offset {offset}: need {payload_len} bytes, \
                     only {} available",
                    data.len() - payload_start
                ));
            }

            let payload = data[payload_start..payload_end].to_vec();

            // Verify CRC32 content hash
            let expected_hash = crc32_content_hash(&payload);
            if expected_hash != header.content_hash {
                return Err(format!(
                    "content hash mismatch at segment {} (offset {offset})",
                    header.segment_id
                ));
            }

            segments.push((header, payload));

            // Advance past header + payload + padding to next 64-byte boundary
            let raw = SEGMENT_HEADER_SIZE + payload_len;
            offset += align_up(raw);
        }

        Ok(Self {
            segments,
            raw_size: data.len(),
        })
    }

    /// Read an RVF container from a file.
    pub fn from_file(path: &std::path::Path) -> Result<Self, String> {
        let data = std::fs::read(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        Self::from_bytes(&data)
    }

    /// Find the first segment with the given type and return its payload.
    pub fn find_segment(&self, seg_type: u8) -> Option<&[u8]> {
        self.segments
            .iter()
            .find(|(h, _)| h.seg_type == seg_type)
            .map(|(_, p)| p.as_slice())
    }

    /// Parse and return the manifest JSON, if present.
    pub fn manifest(&self) -> Option<serde_json::Value> {
        self.find_segment(SEG_MANIFEST)
            .and_then(|data| serde_json::from_slice(data).ok())
    }

    /// Decode and return model weights from the Vec segment, if present.
    pub fn weights(&self) -> Option<Vec<f32>> {
        let data = self.find_segment(SEG_VEC)?;
        if data.len() % 4 != 0 {
            return None;
        }
        let weights: Vec<f32> = data
            .chunks_exact(4)
            .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
            .collect();
        Some(weights)
    }

    /// Parse and return the metadata JSON, if present.
    pub fn metadata(&self) -> Option<serde_json::Value> {
        self.find_segment(SEG_META)
            .and_then(|data| serde_json::from_slice(data).ok())
    }

    /// Parse and return the vital sign config, if present.
    pub fn vital_config(&self) -> Option<VitalSignConfig> {
        self.find_segment(SEG_PROFILE)
            .and_then(|data| serde_json::from_slice(data).ok())
    }

    /// Parse and return the quantization info, if present.
    pub fn quant_info(&self) -> Option<serde_json::Value> {
        self.find_segment(SEG_QUANT)
            .and_then(|data| serde_json::from_slice(data).ok())
    }

    /// Parse and return the witness data, if present.
    pub fn witness(&self) -> Option<serde_json::Value> {
        self.find_segment(SEG_WITNESS)
            .and_then(|data| serde_json::from_slice(data).ok())
    }

    /// Parse and return the embedding config JSON and projection weights, if present.
    pub fn embedding(&self) -> Option<(serde_json::Value, Vec<f32>)> {
        let data = self.find_segment(SEG_EMBED)?;
        if data.len() < 4 {
            return None;
        }
        let config_len = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if 4 + config_len > data.len() {
            return None;
        }
        let config: serde_json::Value = serde_json::from_slice(&data[4..4 + config_len]).ok()?;
        let weight_data = &data[4 + config_len..];
        if weight_data.len() % 4 != 0 {
            return None;
        }
        let weights: Vec<f32> = weight_data
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        Some((config, weights))
    }

    /// Retrieve a named LoRA profile's weights, if present.
    /// Returns None if no profile with the given name exists.
    pub fn lora_profile(&self, name: &str) -> Option<Vec<f32>> {
        for (h, payload) in &self.segments {
            if h.seg_type != SEG_LORA || payload.len() < 2 {
                continue;
            }
            let name_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            if 2 + name_len > payload.len() {
                continue;
            }
            let seg_name = std::str::from_utf8(&payload[2..2 + name_len]).ok()?;
            if seg_name == name {
                let weight_data = &payload[2 + name_len..];
                if weight_data.len() % 4 != 0 {
                    return None;
                }
                let weights: Vec<f32> = weight_data
                    .chunks_exact(4)
                    .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                    .collect();
                return Some(weights);
            }
        }
        None
    }

    /// List all stored LoRA profile names.
    pub fn lora_profiles(&self) -> Vec<String> {
        let mut names = Vec::new();
        for (h, payload) in &self.segments {
            if h.seg_type != SEG_LORA || payload.len() < 2 {
                continue;
            }
            let name_len = u16::from_le_bytes([payload[0], payload[1]]) as usize;
            if 2 + name_len > payload.len() {
                continue;
            }
            if let Ok(name) = std::str::from_utf8(&payload[2..2 + name_len]) {
                names.push(name.to_string());
            }
        }
        names
    }

    /// Number of segments in the container.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Total byte size of the original container data.
    pub fn total_size(&self) -> usize {
        self.raw_size
    }

    /// Build a summary info struct for the REST API.
    pub fn info(&self) -> RvfContainerInfo {
        RvfContainerInfo {
            segment_count: self.segment_count(),
            total_size: self.total_size(),
            manifest: self.manifest(),
            metadata: self.metadata(),
            has_weights: self.find_segment(SEG_VEC).is_some(),
            has_vital_config: self.find_segment(SEG_PROFILE).is_some(),
            has_quant_info: self.find_segment(SEG_QUANT).is_some(),
            has_witness: self.find_segment(SEG_WITNESS).is_some(),
        }
    }

    /// Return an iterator over all segment headers and their payloads.
    pub fn segments(&self) -> impl Iterator<Item = (&SegmentHeader, &[u8])> {
        self.segments.iter().map(|(h, p)| (h, p.as_slice()))
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_known_values() {
        // "hello" CRC32 (IEEE) = 0x3610A686
        let c = crc32(b"hello");
        assert_eq!(c, 0x3610_A686);
    }

    #[test]
    fn crc32_empty() {
        let c = crc32(b"");
        assert_eq!(c, 0x0000_0000);
    }

    #[test]
    fn header_round_trip() {
        let header = SegmentHeader::new(SEG_MANIFEST, 42);
        let bytes = header.to_bytes();
        assert_eq!(bytes.len(), 64);
        let parsed = SegmentHeader::from_bytes(&bytes);
        assert_eq!(parsed.magic, SEGMENT_MAGIC);
        assert_eq!(parsed.version, SEGMENT_VERSION);
        assert_eq!(parsed.seg_type, SEG_MANIFEST);
        assert_eq!(parsed.segment_id, 42);
    }

    #[test]
    fn header_size_is_64() {
        let header = SegmentHeader::new(0x01, 0);
        assert_eq!(header.to_bytes().len(), 64);
    }

    #[test]
    fn header_field_offsets() {
        let mut header = SegmentHeader::new(SEG_VEC, 0x1234_5678_9ABC_DEF0);
        header.flags = 0x0009; // COMPRESSED | SEALED
        header.payload_length = 0xAABB_CCDD_EEFF_0011;
        let bytes = header.to_bytes();

        // Magic at offset 0x00
        assert_eq!(
            u32::from_le_bytes(bytes[0x00..0x04].try_into().unwrap()),
            SEGMENT_MAGIC
        );
        // Version at 0x04
        assert_eq!(bytes[0x04], SEGMENT_VERSION);
        // seg_type at 0x05
        assert_eq!(bytes[0x05], SEG_VEC);
        // flags at 0x06
        assert_eq!(
            u16::from_le_bytes(bytes[0x06..0x08].try_into().unwrap()),
            0x0009
        );
        // segment_id at 0x08
        assert_eq!(
            u64::from_le_bytes(bytes[0x08..0x10].try_into().unwrap()),
            0x1234_5678_9ABC_DEF0
        );
        // payload_length at 0x10
        assert_eq!(
            u64::from_le_bytes(bytes[0x10..0x18].try_into().unwrap()),
            0xAABB_CCDD_EEFF_0011
        );
    }

    #[test]
    fn build_empty_container() {
        let builder = RvfBuilder::new();
        let data = builder.build();
        assert!(data.is_empty());

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 0);
        assert_eq!(reader.total_size(), 0);
    }

    #[test]
    fn manifest_round_trip() {
        let mut builder = RvfBuilder::new();
        builder.add_manifest("test-model", "1.0.0", "A test model");
        let data = builder.build();

        assert_eq!(data.len() % SEGMENT_ALIGNMENT, 0);

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 1);

        let manifest = reader.manifest().expect("manifest should be present");
        assert_eq!(manifest["model_id"], "test-model");
        assert_eq!(manifest["version"], "1.0.0");
        assert_eq!(manifest["description"], "A test model");
    }

    #[test]
    fn weights_round_trip() {
        let weights: Vec<f32> = vec![1.0, -2.5, 3.14, 0.0, f32::MAX, f32::MIN];

        let mut builder = RvfBuilder::new();
        builder.add_weights(&weights);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let decoded = reader.weights().expect("weights should be present");
        assert_eq!(decoded.len(), weights.len());
        for (a, b) in decoded.iter().zip(weights.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn metadata_round_trip() {
        let meta = serde_json::json!({
            "task": "wifi-densepose",
            "input_dim": 56,
            "output_dim": 17,
            "hidden_layers": [128, 64],
        });

        let mut builder = RvfBuilder::new();
        builder.add_metadata(&meta);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let decoded = reader.metadata().expect("metadata should be present");
        assert_eq!(decoded["task"], "wifi-densepose");
        assert_eq!(decoded["input_dim"], 56);
    }

    #[test]
    fn vital_config_round_trip() {
        let config = VitalSignConfig {
            breathing_low_hz: 0.15,
            breathing_high_hz: 0.45,
            heartrate_low_hz: 0.9,
            heartrate_high_hz: 1.8,
            min_subcarriers: 64,
            window_size: 1024,
            confidence_threshold: 0.7,
        };

        let mut builder = RvfBuilder::new();
        builder.add_vital_config(&config);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let decoded = reader.vital_config().expect("vital config should be present");
        assert!((decoded.breathing_low_hz - 0.15).abs() < f64::EPSILON);
        assert_eq!(decoded.min_subcarriers, 64);
        assert_eq!(decoded.window_size, 1024);
    }

    #[test]
    fn quant_info_round_trip() {
        let mut builder = RvfBuilder::new();
        builder.add_quant_info("int8", 0.0078125, -128);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let qi = reader.quant_info().expect("quant info should be present");
        assert_eq!(qi["quant_type"], "int8");
        assert_eq!(qi["zero_point"], -128);
    }

    #[test]
    fn witness_round_trip() {
        let metrics = serde_json::json!({
            "accuracy": 0.95,
            "loss": 0.032,
            "epochs": 100,
        });

        let mut builder = RvfBuilder::new();
        builder.add_witness("sha256:abcdef1234567890", &metrics);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let w = reader.witness().expect("witness should be present");
        assert_eq!(w["training_hash"], "sha256:abcdef1234567890");
        assert_eq!(w["metrics"]["accuracy"], 0.95);
    }

    #[test]
    fn full_container_round_trip() {
        let mut builder = RvfBuilder::new();

        builder.add_manifest("wifi-densepose-v1", "0.1.0", "WiFi DensePose model");
        builder.add_weights(&[0.1, 0.2, 0.3, -0.5, 1.0]);
        builder.add_metadata(&serde_json::json!({
            "architecture": "mlp",
            "input_dim": 56,
        }));
        builder.add_vital_config(&VitalSignConfig::default());
        builder.add_quant_info("fp32", 1.0, 0);
        builder.add_witness("sha256:deadbeef", &serde_json::json!({"loss": 0.01}));

        let data = builder.build();

        // Every segment starts at a 64-byte boundary
        assert_eq!(data.len() % SEGMENT_ALIGNMENT, 0);

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 6);

        // All segments present
        assert!(reader.manifest().is_some());
        assert!(reader.weights().is_some());
        assert!(reader.metadata().is_some());
        assert!(reader.vital_config().is_some());
        assert!(reader.quant_info().is_some());
        assert!(reader.witness().is_some());

        // Verify weights data
        let w = reader.weights().unwrap();
        assert_eq!(w.len(), 5);
        assert!((w[0] - 0.1).abs() < f32::EPSILON);
        assert!((w[3] - (-0.5)).abs() < f32::EPSILON);

        // Info struct for API
        let info = reader.info();
        assert_eq!(info.segment_count, 6);
        assert!(info.has_weights);
        assert!(info.has_vital_config);
        assert!(info.has_quant_info);
        assert!(info.has_witness);
    }

    #[test]
    fn file_round_trip() {
        let dir = std::env::temp_dir().join("rvf_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test_model.rvf");

        let mut builder = RvfBuilder::new();
        builder.add_manifest("file-test", "1.0.0", "File I/O test");
        builder.add_weights(&[42.0, -1.0]);
        builder.write_to_file(&path).unwrap();

        let reader = RvfReader::from_file(&path).unwrap();
        assert_eq!(reader.segment_count(), 2);

        let manifest = reader.manifest().unwrap();
        assert_eq!(manifest["model_id"], "file-test");

        let w = reader.weights().unwrap();
        assert_eq!(w.len(), 2);
        assert!((w[0] - 42.0).abs() < f32::EPSILON);

        // Cleanup
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn invalid_magic_rejected() {
        let mut data = vec![0u8; 128];
        // Write bad magic
        data[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        let result = RvfReader::from_bytes(&data);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("invalid magic"));
    }

    #[test]
    fn truncated_payload_rejected() {
        let mut builder = RvfBuilder::new();
        builder.add_metadata(&serde_json::json!({"key": "a]long value that goes beyond the header boundary for sure to make truncation detectable"}));
        let data = builder.build();

        // Chop off the last half of the container
        let cut = SEGMENT_HEADER_SIZE + 5;
        let truncated = &data[..cut];
        let result = RvfReader::from_bytes(truncated);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("truncated payload"));
    }

    #[test]
    fn content_hash_integrity() {
        let mut builder = RvfBuilder::new();
        builder.add_metadata(&serde_json::json!({"key": "value"}));
        let mut data = builder.build();

        // Corrupt one byte in the payload area (after the 64-byte header)
        if data.len() > 65 {
            data[65] ^= 0xFF;
            let result = RvfReader::from_bytes(&data);
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("hash mismatch"));
        }
    }

    #[test]
    fn alignment_for_various_payload_sizes() {
        for payload_size in [0, 1, 10, 63, 64, 65, 127, 128, 256, 1000] {
            let payload = vec![0xABu8; payload_size];
            let mut builder = RvfBuilder::new();
            builder.push_segment(SEG_META, &payload);
            let data = builder.build();
            assert_eq!(
                data.len() % SEGMENT_ALIGNMENT,
                0,
                "not aligned for payload_size={payload_size}"
            );
        }
    }

    #[test]
    fn segment_ids_are_monotonic() {
        let mut builder = RvfBuilder::new();
        builder.add_manifest("m", "1", "d");
        builder.add_weights(&[1.0]);
        builder.add_metadata(&serde_json::json!({}));

        let data = builder.build();
        let reader = RvfReader::from_bytes(&data).unwrap();

        let ids: Vec<u64> = reader.segments().map(|(h, _)| h.segment_id).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn empty_weights() {
        let mut builder = RvfBuilder::new();
        builder.add_weights(&[]);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let w = reader.weights().unwrap();
        assert!(w.is_empty());
    }

    #[test]
    fn info_reports_correctly() {
        let mut builder = RvfBuilder::new();
        builder.add_manifest("info-test", "2.0", "info test");
        builder.add_weights(&[1.0, 2.0, 3.0]);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        let info = reader.info();
        assert_eq!(info.segment_count, 2);
        assert!(info.total_size > 0);
        assert!(info.manifest.is_some());
        assert!(info.has_weights);
        assert!(!info.has_vital_config);
        assert!(!info.has_quant_info);
        assert!(!info.has_witness);
    }

    #[test]
    fn test_rvf_embedding_segment_roundtrip() {
        let config = serde_json::json!({
            "d_model": 64,
            "d_proj": 128,
            "temperature": 0.07,
            "normalize": true,
        });
        let weights: Vec<f32> = (0..256).map(|i| (i as f32 * 0.13).sin()).collect();

        let mut builder = RvfBuilder::new();
        builder.add_manifest("embed-test", "1.0", "embedding test");
        builder.add_embedding(&config, &weights);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 2);

        let (decoded_config, decoded_weights) = reader.embedding()
            .expect("embedding segment should be present");
        assert_eq!(decoded_config["d_model"], 64);
        assert_eq!(decoded_config["d_proj"], 128);
        assert!((decoded_config["temperature"].as_f64().unwrap() - 0.07).abs() < 1e-4);
        assert_eq!(decoded_weights.len(), weights.len());
        for (a, b) in decoded_weights.iter().zip(weights.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "weight mismatch");
        }
    }

    // ── Phase 7: RVF LoRA profile tests ───────────────────────────────

    #[test]
    fn test_rvf_lora_profile_roundtrip() {
        let weights: Vec<f32> = (0..100).map(|i| (i as f32 * 0.37).sin()).collect();

        let mut builder = RvfBuilder::new();
        builder.add_manifest("lora-test", "1.0", "LoRA profile test");
        builder.add_lora_profile("office-env", &weights);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 2);

        let profiles = reader.lora_profiles();
        assert_eq!(profiles, vec!["office-env"]);

        let decoded = reader.lora_profile("office-env")
            .expect("LoRA profile should be present");
        assert_eq!(decoded.len(), weights.len());
        for (a, b) in decoded.iter().zip(weights.iter()) {
            assert_eq!(a.to_bits(), b.to_bits(), "LoRA weight mismatch");
        }

        // Non-existent profile returns None
        assert!(reader.lora_profile("nonexistent").is_none());
    }

    #[test]
    fn test_rvf_multiple_lora_profiles() {
        let w1: Vec<f32> = vec![1.0, 2.0, 3.0];
        let w2: Vec<f32> = vec![4.0, 5.0, 6.0, 7.0];
        let w3: Vec<f32> = vec![-1.0, -2.0];

        let mut builder = RvfBuilder::new();
        builder.add_lora_profile("office", &w1);
        builder.add_lora_profile("home", &w2);
        builder.add_lora_profile("outdoor", &w3);
        let data = builder.build();

        let reader = RvfReader::from_bytes(&data).unwrap();
        assert_eq!(reader.segment_count(), 3);

        let profiles = reader.lora_profiles();
        assert_eq!(profiles.len(), 3);
        assert!(profiles.contains(&"office".to_string()));
        assert!(profiles.contains(&"home".to_string()));
        assert!(profiles.contains(&"outdoor".to_string()));

        // Verify each profile's weights
        let d1 = reader.lora_profile("office").unwrap();
        assert_eq!(d1, w1);
        let d2 = reader.lora_profile("home").unwrap();
        assert_eq!(d2, w2);
        let d3 = reader.lora_profile("outdoor").unwrap();
        assert_eq!(d3, w3);
    }
}
