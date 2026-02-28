//! QR Cognitive Seed generator and parser for ADR-034.
//!
//! Builds RVQS payloads from RVF stores and parses them back.
//! The generator packs a 64-byte header, optional compressed WASM
//! microkernel, a TLV download manifest, and a signature into a
//! payload that fits within a single QR code (≤2,953 bytes).

use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use rvf_types::qr_seed::*;

use crate::compress;
use crate::seed_crypto;

/// Errors specific to QR seed operations.
#[derive(Debug)]
pub enum SeedError {
    /// Seed exceeds QR capacity.
    TooLarge { size: usize, max: usize },
    /// Header parse or validation failure.
    InvalidHeader(rvf_types::RvfError),
    /// Download manifest is malformed.
    InvalidManifest(String),
    /// Signature verification failed.
    SignatureInvalid,
    /// Unknown signature algorithm.
    UnknownSigAlgo(u16),
    /// I/O error.
    Io(io::Error),
    /// Missing required component.
    MissingComponent(&'static str),
}

impl From<io::Error> for SeedError {
    fn from(e: io::Error) -> Self {
        SeedError::Io(e)
    }
}

impl From<rvf_types::RvfError> for SeedError {
    fn from(e: rvf_types::RvfError) -> Self {
        SeedError::InvalidHeader(e)
    }
}

impl core::fmt::Display for SeedError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SeedError::TooLarge { size, max } => {
                write!(f, "seed too large: {size} bytes (max {max})")
            }
            SeedError::InvalidHeader(e) => write!(f, "invalid header: {e}"),
            SeedError::InvalidManifest(msg) => write!(f, "invalid manifest: {msg}"),
            SeedError::SignatureInvalid => write!(f, "signature verification failed"),
            SeedError::UnknownSigAlgo(algo) => write!(f, "unknown sig algo: {algo}"),
            SeedError::Io(e) => write!(f, "I/O error: {e}"),
            SeedError::MissingComponent(c) => write!(f, "missing component: {c}"),
        }
    }
}

/// TLV record for download manifest encoding.
struct TlvRecord {
    tag: u16,
    value: Vec<u8>,
}

impl TlvRecord {
    fn encoded_size(&self) -> usize {
        // tag(2) + length(2) + value
        4 + self.value.len()
    }

    fn write_to(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&self.tag.to_le_bytes());
        buf.extend_from_slice(&(self.value.len() as u16).to_le_bytes());
        buf.extend_from_slice(&self.value);
    }
}

/// Configuration for building a QR seed.
#[derive(Clone, Debug)]
pub struct SeedBuilder {
    /// Unique file identifier.
    pub file_id: [u8; 8],
    /// Expected total vectors.
    pub total_vector_count: u32,
    /// Vector dimensionality.
    pub dimension: u16,
    /// Base data type.
    pub base_dtype: u8,
    /// Domain profile id.
    pub profile_id: u8,
    /// Compressed WASM microkernel bytes (Brotli).
    pub microkernel: Option<Vec<u8>>,
    /// Download hosts.
    pub hosts: Vec<HostEntry>,
    /// SHAKE-256-256 hash of the full RVF file.
    pub content_hash_full: Option<[u8; 32]>,
    /// Total RVF file size.
    pub total_file_size: Option<u64>,
    /// Progressive layer entries.
    pub layers: Vec<LayerEntry>,
    /// Session token for authenticated download.
    pub session_token: Option<[u8; 16]>,
    /// Token TTL in seconds.
    pub token_ttl: Option<u32>,
    /// TLS cert pin (SHA-256 of SPKI).
    pub cert_pin: Option<[u8; 32]>,
    /// Signature algorithm (0=Ed25519, 1=ML-DSA-65).
    pub sig_algo: u16,
    /// Signature bytes (caller provides).
    pub signature: Option<Vec<u8>>,
    /// SHAKE-256-64 of complete expanded RVF.
    pub content_hash: [u8; 8],
    /// Whether seed should be offline-capable.
    pub offline_capable: bool,
    /// Whether seed can stream-upgrade.
    pub stream_upgrade: bool,
    /// Optional inline vector data (for tiny models).
    pub inline_vectors: Option<Vec<u8>>,
}

impl SeedBuilder {
    /// Create a new seed builder with required fields.
    pub fn new(file_id: [u8; 8], dimension: u16, total_vector_count: u32) -> Self {
        Self {
            file_id,
            total_vector_count,
            dimension,
            base_dtype: 0,
            profile_id: 0,
            microkernel: None,
            hosts: Vec::new(),
            content_hash_full: None,
            total_file_size: None,
            layers: Vec::new(),
            session_token: None,
            token_ttl: None,
            cert_pin: None,
            sig_algo: 0,
            signature: None,
            content_hash: [0u8; 8],
            offline_capable: false,
            stream_upgrade: false,
            inline_vectors: None,
        }
    }

    /// Set the compressed WASM microkernel.
    pub fn with_microkernel(mut self, data: Vec<u8>) -> Self {
        self.microkernel = Some(data);
        self
    }

    /// Add a download host.
    pub fn add_host(mut self, host: HostEntry) -> Self {
        self.hosts.push(host);
        self
    }

    /// Add a progressive layer entry.
    pub fn add_layer(mut self, layer: LayerEntry) -> Self {
        self.layers.push(layer);
        self
    }

    /// Set signature data.
    pub fn with_signature(mut self, algo: u16, sig: Vec<u8>) -> Self {
        self.sig_algo = algo;
        self.signature = Some(sig);
        self
    }

    /// Set the content hash (SHAKE-256-64 of expanded RVF).
    pub fn with_content_hash(mut self, hash: [u8; 8]) -> Self {
        self.content_hash = hash;
        self
    }

    /// Compress raw WASM bytes using the built-in LZ compressor and set as microkernel.
    pub fn compress_microkernel(mut self, raw_wasm: &[u8]) -> Self {
        self.microkernel = Some(compress::compress(raw_wasm));
        self
    }

    /// Build the download manifest as TLV records.
    fn build_manifest(&self) -> Vec<u8> {
        let mut records: Vec<TlvRecord> = Vec::new();

        // Primary and fallback hosts.
        for (i, host) in self.hosts.iter().enumerate() {
            let tag = if i == 0 {
                DL_TAG_HOST_PRIMARY
            } else {
                DL_TAG_HOST_FALLBACK
            };
            records.push(TlvRecord {
                tag,
                value: host.to_bytes().to_vec(),
            });
        }

        // Content hash of full RVF.
        if let Some(hash) = &self.content_hash_full {
            records.push(TlvRecord {
                tag: DL_TAG_CONTENT_HASH,
                value: hash.to_vec(),
            });
        }

        // Total file size.
        if let Some(size) = self.total_file_size {
            records.push(TlvRecord {
                tag: DL_TAG_TOTAL_SIZE,
                value: size.to_le_bytes().to_vec(),
            });
        }

        // Layer manifest.
        if !self.layers.is_empty() {
            let mut layer_buf = Vec::new();
            layer_buf.push(self.layers.len() as u8);
            for layer in &self.layers {
                layer_buf.push(layer.layer_id);
                layer_buf.push(layer.priority);
                layer_buf.extend_from_slice(&layer.offset.to_le_bytes());
                layer_buf.extend_from_slice(&layer.size.to_le_bytes());
                layer_buf.extend_from_slice(&layer.content_hash);
                layer_buf.push(layer.required);
            }
            records.push(TlvRecord {
                tag: DL_TAG_LAYER_MANIFEST,
                value: layer_buf,
            });
        }

        // Session token.
        if let Some(token) = &self.session_token {
            records.push(TlvRecord {
                tag: DL_TAG_SESSION_TOKEN,
                value: token.to_vec(),
            });
        }

        // Token TTL.
        if let Some(ttl) = self.token_ttl {
            records.push(TlvRecord {
                tag: DL_TAG_TTL,
                value: ttl.to_le_bytes().to_vec(),
            });
        }

        // Cert pin.
        if let Some(pin) = &self.cert_pin {
            records.push(TlvRecord {
                tag: DL_TAG_CERT_PIN,
                value: pin.to_vec(),
            });
        }

        let total_size: usize = records.iter().map(|r| r.encoded_size()).sum();
        let mut buf = Vec::with_capacity(total_size);
        for record in &records {
            record.write_to(&mut buf);
        }
        buf
    }

    /// Compute flags from builder state.
    fn compute_flags(&self) -> u16 {
        let mut flags: u16 = 0;
        if self.microkernel.is_some() {
            flags |= SEED_HAS_MICROKERNEL | SEED_COMPRESSED;
        }
        if !self.hosts.is_empty() || !self.layers.is_empty() {
            flags |= SEED_HAS_DOWNLOAD;
        }
        if self.signature.is_some() {
            flags |= SEED_SIGNED;
        }
        if self.offline_capable {
            flags |= SEED_OFFLINE_CAPABLE;
        }
        if self.inline_vectors.is_some() {
            flags |= SEED_HAS_VECTORS;
        }
        if self.stream_upgrade {
            flags |= SEED_STREAM_UPGRADE;
        }
        flags
    }

    /// Build the complete RVQS seed payload.
    ///
    /// Returns the binary payload and the header for inspection.
    pub fn build(self) -> Result<(Vec<u8>, SeedHeader), SeedError> {
        let manifest = self.build_manifest();
        let microkernel_data = self.microkernel.as_deref().unwrap_or(&[]);
        let sig_data = self.signature.as_deref().unwrap_or(&[]);
        let flags = self.compute_flags();

        let microkernel_offset = SEED_HEADER_SIZE as u32;
        let microkernel_size = microkernel_data.len() as u32;

        let download_manifest_offset = microkernel_offset + microkernel_size;
        let download_manifest_size = manifest.len() as u32;

        let total_seed_size = SEED_HEADER_SIZE as u32
            + microkernel_size
            + download_manifest_size
            + sig_data.len() as u32;

        if total_seed_size as usize > QR_MAX_BYTES {
            return Err(SeedError::TooLarge {
                size: total_seed_size as usize,
                max: QR_MAX_BYTES,
            });
        }

        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let header = SeedHeader {
            seed_magic: SEED_MAGIC,
            seed_version: 1,
            flags,
            file_id: self.file_id,
            total_vector_count: self.total_vector_count,
            dimension: self.dimension,
            base_dtype: self.base_dtype,
            profile_id: self.profile_id,
            created_ns,
            microkernel_offset,
            microkernel_size,
            download_manifest_offset,
            download_manifest_size,
            sig_algo: self.sig_algo,
            sig_length: sig_data.len() as u16,
            total_seed_size,
            content_hash: self.content_hash,
        };

        let mut payload = Vec::with_capacity(total_seed_size as usize);
        payload.extend_from_slice(&header.to_bytes());
        payload.extend_from_slice(microkernel_data);
        payload.extend_from_slice(&manifest);
        payload.extend_from_slice(sig_data);

        debug_assert_eq!(payload.len(), total_seed_size as usize);

        Ok((payload, header))
    }

    /// Build with automatic SHA-256 content hashing and HMAC-SHA256 signing.
    ///
    /// This is the production path: content hash is computed from the payload,
    /// and the signature covers the entire unsigned payload.
    /// Uses sig_algo=2 (HMAC-SHA256, 32-byte signature).
    pub fn build_and_sign(self, signing_key: &[u8]) -> Result<(Vec<u8>, SeedHeader), SeedError> {
        let manifest = self.build_manifest();
        let microkernel_data = self.microkernel.as_deref().unwrap_or(&[]);
        let flags = self.compute_flags() | SEED_SIGNED;

        // Compute content hash over data (microkernel + manifest).
        let mut hash_input = Vec::with_capacity(microkernel_data.len() + manifest.len());
        hash_input.extend_from_slice(microkernel_data);
        hash_input.extend_from_slice(&manifest);
        let content_hash = seed_crypto::seed_content_hash(&hash_input);

        let sig_length: u16 = 32; // HMAC-SHA256.
        let microkernel_offset = SEED_HEADER_SIZE as u32;
        let microkernel_size = microkernel_data.len() as u32;
        let download_manifest_offset = microkernel_offset + microkernel_size;
        let download_manifest_size = manifest.len() as u32;
        let total_seed_size =
            SEED_HEADER_SIZE as u32 + microkernel_size + download_manifest_size + sig_length as u32;

        if total_seed_size as usize > QR_MAX_BYTES {
            return Err(SeedError::TooLarge {
                size: total_seed_size as usize,
                max: QR_MAX_BYTES,
            });
        }

        let created_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let header = SeedHeader {
            seed_magic: SEED_MAGIC,
            seed_version: 1,
            flags,
            file_id: self.file_id,
            total_vector_count: self.total_vector_count,
            dimension: self.dimension,
            base_dtype: self.base_dtype,
            profile_id: self.profile_id,
            created_ns,
            microkernel_offset,
            microkernel_size,
            download_manifest_offset,
            download_manifest_size,
            sig_algo: seed_crypto::SIG_ALGO_HMAC_SHA256,
            sig_length,
            total_seed_size,
            content_hash,
        };

        // Build unsigned payload.
        let unsigned_size = total_seed_size as usize - sig_length as usize;
        let mut unsigned = Vec::with_capacity(unsigned_size);
        unsigned.extend_from_slice(&header.to_bytes());
        unsigned.extend_from_slice(microkernel_data);
        unsigned.extend_from_slice(&manifest);
        debug_assert_eq!(unsigned.len(), unsigned_size);

        // Sign.
        let signature = seed_crypto::sign_seed(signing_key, &unsigned);

        // Final payload.
        let mut payload = unsigned;
        payload.extend_from_slice(&signature);
        debug_assert_eq!(payload.len(), total_seed_size as usize);

        Ok((payload, header))
    }
}

/// A parsed and verified QR seed.
#[derive(Debug)]
pub struct ParsedSeed<'a> {
    /// The parsed header.
    pub header: SeedHeader,
    /// Compressed microkernel bytes (if present).
    pub microkernel: Option<&'a [u8]>,
    /// Raw download manifest bytes (TLV).
    pub manifest_bytes: Option<&'a [u8]>,
    /// Signature bytes (if present).
    pub signature: Option<&'a [u8]>,
}

impl<'a> ParsedSeed<'a> {
    /// Parse a QR seed payload into its components.
    pub fn parse(data: &'a [u8]) -> Result<Self, SeedError> {
        if data.len() < SEED_HEADER_SIZE {
            return Err(SeedError::InvalidHeader(
                rvf_types::RvfError::SizeMismatch {
                    expected: SEED_HEADER_SIZE,
                    got: data.len(),
                },
            ));
        }

        let header = SeedHeader::from_bytes(data)?;

        if (header.total_seed_size as usize) > data.len() {
            return Err(SeedError::InvalidHeader(
                rvf_types::RvfError::SizeMismatch {
                    expected: header.total_seed_size as usize,
                    got: data.len(),
                },
            ));
        }

        let microkernel = if header.has_microkernel() && header.microkernel_size > 0 {
            let start = header.microkernel_offset as usize;
            let end = start + header.microkernel_size as usize;
            if end > data.len() {
                return Err(SeedError::MissingComponent(
                    "microkernel extends beyond payload",
                ));
            }
            Some(&data[start..end])
        } else {
            None
        };

        let manifest_bytes = if header.has_download_manifest() && header.download_manifest_size > 0
        {
            let start = header.download_manifest_offset as usize;
            let end = start + header.download_manifest_size as usize;
            if end > data.len() {
                return Err(SeedError::MissingComponent(
                    "manifest extends beyond payload",
                ));
            }
            Some(&data[start..end])
        } else {
            None
        };

        let signature = if header.is_signed() && header.sig_length > 0 {
            let sig_start = header.total_seed_size as usize - header.sig_length as usize;
            let sig_end = header.total_seed_size as usize;
            if sig_end > data.len() {
                return Err(SeedError::MissingComponent(
                    "signature extends beyond payload",
                ));
            }
            Some(&data[sig_start..sig_end])
        } else {
            None
        };

        Ok(ParsedSeed {
            header,
            microkernel,
            manifest_bytes,
            signature,
        })
    }

    /// Parse the download manifest TLV records.
    pub fn parse_manifest(&self) -> Result<DownloadManifest, SeedError> {
        let data = match self.manifest_bytes {
            Some(d) => d,
            None => return Ok(DownloadManifest::default()),
        };

        let mut manifest = DownloadManifest::default();
        let mut pos = 0;

        while pos + 4 <= data.len() {
            let tag = u16::from_le_bytes([data[pos], data[pos + 1]]);
            let length = u16::from_le_bytes([data[pos + 2], data[pos + 3]]) as usize;
            pos += 4;

            if pos + length > data.len() {
                return Err(SeedError::InvalidManifest(
                    "TLV record extends beyond manifest".into(),
                ));
            }

            let value = &data[pos..pos + length];

            match tag {
                DL_TAG_HOST_PRIMARY | DL_TAG_HOST_FALLBACK => {
                    if length >= 150 {
                        let url_length = u16::from_le_bytes([value[0], value[1]]);
                        let mut url = [0u8; 128];
                        url.copy_from_slice(&value[2..130]);
                        let priority = u16::from_le_bytes([value[130], value[131]]);
                        let region = u16::from_le_bytes([value[132], value[133]]);
                        let mut host_key_hash = [0u8; 16];
                        host_key_hash.copy_from_slice(&value[134..150]);
                        manifest.hosts.push(HostEntry {
                            url,
                            url_length,
                            priority,
                            region,
                            host_key_hash,
                        });
                    }
                }
                DL_TAG_CONTENT_HASH => {
                    if length >= 32 {
                        let mut hash = [0u8; 32];
                        hash.copy_from_slice(&value[..32]);
                        manifest.content_hash = Some(hash);
                    }
                }
                DL_TAG_TOTAL_SIZE => {
                    if length >= 8 {
                        manifest.total_file_size = Some(u64::from_le_bytes([
                            value[0], value[1], value[2], value[3], value[4], value[5], value[6],
                            value[7],
                        ]));
                    }
                }
                DL_TAG_LAYER_MANIFEST => {
                    if !value.is_empty() {
                        let layer_count = value[0] as usize;
                        let mut lpos = 1;
                        for _ in 0..layer_count {
                            if lpos + 27 > value.len() {
                                break;
                            }
                            let layer_id = value[lpos];
                            let priority = value[lpos + 1];
                            let offset = u32::from_le_bytes([
                                value[lpos + 2],
                                value[lpos + 3],
                                value[lpos + 4],
                                value[lpos + 5],
                            ]);
                            let size = u32::from_le_bytes([
                                value[lpos + 6],
                                value[lpos + 7],
                                value[lpos + 8],
                                value[lpos + 9],
                            ]);
                            let mut content_hash = [0u8; 16];
                            content_hash.copy_from_slice(&value[lpos + 10..lpos + 26]);
                            let required = value[lpos + 26];
                            manifest.layers.push(LayerEntry {
                                layer_id,
                                priority,
                                offset,
                                size,
                                content_hash,
                                required,
                                _pad: 0,
                            });
                            lpos += 27;
                        }
                    }
                }
                DL_TAG_SESSION_TOKEN => {
                    if length >= 16 {
                        let mut token = [0u8; 16];
                        token.copy_from_slice(&value[..16]);
                        manifest.session_token = Some(token);
                    }
                }
                DL_TAG_TTL => {
                    if length >= 4 {
                        manifest.token_ttl =
                            Some(u32::from_le_bytes([value[0], value[1], value[2], value[3]]));
                    }
                }
                DL_TAG_CERT_PIN => {
                    if length >= 32 {
                        let mut pin = [0u8; 32];
                        pin.copy_from_slice(&value[..32]);
                        manifest.cert_pin = Some(pin);
                    }
                }
                _ => {
                    // Unknown tags are ignored (forward-compatibility).
                }
            }

            pos += length;
        }

        Ok(manifest)
    }

    /// Get the signed payload (everything before the signature).
    pub fn signed_payload<'b>(&self, full_data: &'b [u8]) -> Option<&'b [u8]> {
        if self.header.is_signed() && self.header.sig_length > 0 {
            let sig_start = self.header.total_seed_size as usize - self.header.sig_length as usize;
            Some(&full_data[..sig_start])
        } else {
            None
        }
    }

    /// Verify the HMAC-SHA256 signature against the unsigned payload.
    pub fn verify_signature(&self, key: &[u8], full_data: &[u8]) -> Result<(), SeedError> {
        let signature = self
            .signature
            .ok_or(SeedError::MissingComponent("signature"))?;
        let signed_payload = self
            .signed_payload(full_data)
            .ok_or(SeedError::MissingComponent("signed payload"))?;
        if seed_crypto::verify_seed(key, signed_payload, signature) {
            Ok(())
        } else {
            Err(SeedError::SignatureInvalid)
        }
    }

    /// Verify the content hash matches the microkernel + manifest data.
    pub fn verify_content_hash(&self) -> bool {
        let microkernel = self.microkernel.unwrap_or(&[]);
        let manifest = self.manifest_bytes.unwrap_or(&[]);

        let mut hash_input = Vec::with_capacity(microkernel.len() + manifest.len());
        hash_input.extend_from_slice(microkernel);
        hash_input.extend_from_slice(manifest);

        seed_crypto::verify_content_hash(&self.header.content_hash, &hash_input)
    }

    /// Decompress the microkernel using the built-in LZ decompressor.
    pub fn decompress_microkernel(&self) -> Result<Vec<u8>, SeedError> {
        match self.microkernel {
            Some(data) => compress::decompress(data)
                .map_err(|e| SeedError::InvalidManifest(format!("decompress: {e}"))),
            None => Err(SeedError::MissingComponent("microkernel")),
        }
    }

    /// Full verification: check magic, content hash, and signature in one call.
    pub fn verify_all(&self, key: &[u8], full_data: &[u8]) -> Result<(), SeedError> {
        if !self.header.is_valid_magic() {
            return Err(SeedError::InvalidHeader(rvf_types::RvfError::BadMagic {
                expected: SEED_MAGIC,
                got: self.header.seed_magic,
            }));
        }
        if !self.verify_content_hash() {
            return Err(SeedError::InvalidManifest("content hash mismatch".into()));
        }
        if self.header.is_signed() {
            self.verify_signature(key, full_data)?;
        }
        Ok(())
    }
}

/// Parsed download manifest with structured fields.
#[derive(Clone, Debug, Default)]
pub struct DownloadManifest {
    /// Download hosts (primary first, then fallbacks).
    pub hosts: Vec<HostEntry>,
    /// SHAKE-256-256 hash of the full RVF file.
    pub content_hash: Option<[u8; 32]>,
    /// Expected total RVF file size.
    pub total_file_size: Option<u64>,
    /// Progressive layer entries.
    pub layers: Vec<LayerEntry>,
    /// Session token for authenticated download.
    pub session_token: Option<[u8; 16]>,
    /// Token TTL in seconds.
    pub token_ttl: Option<u32>,
    /// TLS cert pin.
    pub cert_pin: Option<[u8; 32]>,
}

/// Bootstrap progress tracker for progressive download.
#[derive(Clone, Debug)]
pub struct BootstrapProgress {
    /// Current phase (0=Parse, 1=Download, 2=Full).
    pub phase: u8,
    /// Layers downloaded so far.
    pub layers_downloaded: Vec<u8>,
    /// Total bytes downloaded.
    pub bytes_downloaded: u64,
    /// Total expected bytes.
    pub bytes_total: u64,
    /// Whether the seed is query-ready (at least hot cache loaded).
    pub query_ready: bool,
    /// Current estimated recall.
    pub estimated_recall: f32,
}

impl BootstrapProgress {
    /// Create a new progress tracker from a manifest.
    pub fn new(manifest: &DownloadManifest) -> Self {
        let bytes_total = manifest.total_file_size.unwrap_or(0);
        Self {
            phase: 0,
            layers_downloaded: Vec::new(),
            bytes_downloaded: 0,
            bytes_total,
            query_ready: false,
            estimated_recall: 0.0,
        }
    }

    /// Record that a layer has been downloaded.
    pub fn record_layer(&mut self, layer: &LayerEntry) {
        self.layers_downloaded.push(layer.layer_id);
        self.bytes_downloaded += layer.size as u64;

        // Update estimated recall based on layer type.
        use rvf_types::qr_seed::layer_id::*;
        match layer.layer_id {
            HOT_CACHE => {
                self.query_ready = true;
                self.estimated_recall = 0.50;
            }
            HNSW_LAYER_A => self.estimated_recall = 0.70,
            HNSW_LAYER_B => self.estimated_recall = 0.85,
            HNSW_LAYER_C => self.estimated_recall = 0.95,
            FULL_VECTORS => self.estimated_recall = self.estimated_recall.max(0.90),
            _ => {}
        }

        // Update phase.
        if self.bytes_total > 0 && self.bytes_downloaded >= self.bytes_total {
            self.phase = 2; // Full intelligence.
        } else if self.query_ready {
            self.phase = 1; // Progressive download.
        }
    }

    /// Fraction of total download complete.
    pub fn progress_fraction(&self) -> f32 {
        if self.bytes_total == 0 {
            return 0.0;
        }
        (self.bytes_downloaded as f64 / self.bytes_total as f64) as f32
    }
}

/// Helper to build a HostEntry from a URL string.
pub fn make_host_entry(
    url: &str,
    priority: u16,
    region: u16,
    host_key_hash: [u8; 16],
) -> Result<HostEntry, SeedError> {
    let url_bytes = url.as_bytes();
    if url_bytes.len() > 128 {
        return Err(SeedError::InvalidManifest("URL exceeds 128 bytes".into()));
    }
    let mut url_buf = [0u8; 128];
    url_buf[..url_bytes.len()].copy_from_slice(url_bytes);
    Ok(HostEntry {
        url: url_buf,
        url_length: url_bytes.len() as u16,
        priority,
        region,
        host_key_hash,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_layers() -> Vec<LayerEntry> {
        vec![
            LayerEntry {
                layer_id: layer_id::LEVEL0,
                priority: 0,
                offset: 0,
                size: 4096,
                content_hash: [0x11; 16],
                required: 1,
                _pad: 0,
            },
            LayerEntry {
                layer_id: layer_id::HOT_CACHE,
                priority: 1,
                offset: 4096,
                size: 51200,
                content_hash: [0x22; 16],
                required: 1,
                _pad: 0,
            },
            LayerEntry {
                layer_id: layer_id::HNSW_LAYER_A,
                priority: 2,
                offset: 55296,
                size: 204800,
                content_hash: [0x33; 16],
                required: 0,
                _pad: 0,
            },
        ]
    }

    #[test]
    fn build_minimal_seed() {
        let builder = SeedBuilder::new([0x01; 8], 384, 100_000);
        let (payload, header) = builder.build().unwrap();
        assert_eq!(header.seed_magic, SEED_MAGIC);
        assert_eq!(payload.len(), SEED_HEADER_SIZE);
        assert!(header.fits_in_qr());
    }

    #[test]
    fn build_seed_with_microkernel() {
        let microkernel = vec![0xAA; 2100]; // Simulated compressed WASM.
        let builder =
            SeedBuilder::new([0x02; 8], 384, 100_000).with_microkernel(microkernel.clone());
        let (payload, header) = builder.build().unwrap();
        assert!(header.has_microkernel());
        assert_eq!(header.microkernel_size, 2100);
        assert_eq!(header.microkernel_offset, SEED_HEADER_SIZE as u32);
        assert_eq!(
            &payload[SEED_HEADER_SIZE..SEED_HEADER_SIZE + 2100],
            &microkernel[..]
        );
    }

    #[test]
    fn build_seed_with_hosts_and_layers() {
        let host =
            make_host_entry("https://cdn.example.com/rvf/brain.rvf", 0, 1, [0xBB; 16]).unwrap();

        let mut builder = SeedBuilder::new([0x03; 8], 384, 100_000)
            .add_host(host)
            .with_content_hash([0xCC; 8]);

        builder.content_hash_full = Some([0xDD; 32]);
        builder.total_file_size = Some(10_000_000);

        for layer in default_layers() {
            builder = builder.add_layer(layer);
        }

        let (payload, header) = builder.build().unwrap();
        assert!(header.has_download_manifest());
        assert!(header.download_manifest_size > 0);
        assert!(header.fits_in_qr());
    }

    #[test]
    fn build_seed_with_signature() {
        let sig = vec![0xEE; 64]; // Ed25519 sig.
        let builder = SeedBuilder::new([0x04; 8], 384, 100_000).with_signature(0, sig.clone());
        let (payload, header) = builder.build().unwrap();
        assert!(header.is_signed());
        assert_eq!(header.sig_length, 64);
        assert_eq!(&payload[payload.len() - 64..], &sig[..]);
    }

    #[test]
    fn build_full_seed_fits_in_qr() {
        let microkernel = vec![0xAA; 2100];
        let host =
            make_host_entry("https://cdn.example.com/rvf/brain.rvf", 0, 1, [0xBB; 16]).unwrap();
        let sig = vec![0xEE; 64];

        let mut builder = SeedBuilder::new([0x05; 8], 384, 100_000)
            .with_microkernel(microkernel)
            .add_host(host)
            .with_signature(0, sig)
            .with_content_hash([0xCC; 8]);

        builder.content_hash_full = Some([0xDD; 32]);
        builder.total_file_size = Some(10_000_000);
        builder.stream_upgrade = true;

        for layer in default_layers() {
            builder = builder.add_layer(layer);
        }

        let (payload, header) = builder.build().unwrap();
        assert!(header.fits_in_qr());
        assert!(payload.len() <= QR_MAX_BYTES);
        assert!(header.has_microkernel());
        assert!(header.has_download_manifest());
        assert!(header.is_signed());
    }

    #[test]
    fn seed_too_large_rejected() {
        let microkernel = vec![0xAA; 2900]; // Too large.
        let builder = SeedBuilder::new([0x06; 8], 384, 100_000).with_microkernel(microkernel);
        let result = builder.build();
        assert!(result.is_err());
        match result {
            Err(SeedError::TooLarge { .. }) => {}
            _ => panic!("expected TooLarge"),
        }
    }

    #[test]
    fn parse_round_trip() {
        let microkernel = vec![0xAA; 512];
        let host =
            make_host_entry("https://cdn.example.com/rvf/brain.rvf", 0, 1, [0xBB; 16]).unwrap();
        let sig = vec![0xEE; 64];

        let mut builder = SeedBuilder::new([0x07; 8], 384, 100_000)
            .with_microkernel(microkernel.clone())
            .add_host(host.clone())
            .with_signature(0, sig.clone())
            .with_content_hash([0xCC; 8]);

        builder.content_hash_full = Some([0xDD; 32]);
        builder.total_file_size = Some(10_000_000);

        for layer in default_layers() {
            builder = builder.add_layer(layer);
        }

        let (payload, header) = builder.build().unwrap();

        // Parse it back.
        let parsed = ParsedSeed::parse(&payload).unwrap();
        assert_eq!(parsed.header, header);
        assert_eq!(parsed.microkernel.unwrap(), &microkernel[..]);
        assert_eq!(parsed.signature.unwrap(), &sig[..]);

        // Parse manifest.
        let manifest = parsed.parse_manifest().unwrap();
        assert_eq!(manifest.hosts.len(), 1);
        assert_eq!(manifest.hosts[0].url_str(), host.url_str());
        assert_eq!(manifest.content_hash, Some([0xDD; 32]));
        assert_eq!(manifest.total_file_size, Some(10_000_000));
        assert_eq!(manifest.layers.len(), 3);
        assert_eq!(manifest.layers[0].layer_id, layer_id::LEVEL0);
        assert_eq!(manifest.layers[1].layer_id, layer_id::HOT_CACHE);
        assert_eq!(manifest.layers[2].layer_id, layer_id::HNSW_LAYER_A);
    }

    #[test]
    fn signed_payload_extraction() {
        let sig = vec![0xEE; 64];
        let builder = SeedBuilder::new([0x08; 8], 384, 100_000).with_signature(0, sig.clone());
        let (payload, _) = builder.build().unwrap();

        let parsed = ParsedSeed::parse(&payload).unwrap();
        let signed = parsed.signed_payload(&payload).unwrap();
        assert_eq!(signed.len(), payload.len() - 64);
    }

    #[test]
    fn bootstrap_progress_tracking() {
        let layers = default_layers();
        let manifest = DownloadManifest {
            hosts: vec![],
            content_hash: None,
            total_file_size: Some(260096),
            layers: layers.clone(),
            session_token: None,
            token_ttl: None,
            cert_pin: None,
        };

        let mut progress = BootstrapProgress::new(&manifest);
        assert_eq!(progress.phase, 0);
        assert!(!progress.query_ready);

        // Download Level 0.
        progress.record_layer(&layers[0]);
        assert_eq!(progress.phase, 0);
        assert!(!progress.query_ready);

        // Download Hot Cache.
        progress.record_layer(&layers[1]);
        assert_eq!(progress.phase, 1);
        assert!(progress.query_ready);
        assert!((progress.estimated_recall - 0.50).abs() < f32::EPSILON);

        // Download HNSW Layer A.
        progress.record_layer(&layers[2]);
        assert!((progress.estimated_recall - 0.70).abs() < f32::EPSILON);
        assert!(progress.progress_fraction() > 0.99); // All layers downloaded.
        assert_eq!(progress.phase, 2); // Full.
    }

    #[test]
    fn make_host_entry_too_long() {
        let long_url = "x".repeat(200);
        assert!(make_host_entry(&long_url, 0, 0, [0; 16]).is_err());
    }

    #[test]
    fn manifest_unknown_tags_ignored() {
        // Build a manifest with an unknown tag.
        let mut manifest_bytes = Vec::new();
        // Unknown tag 0xFF00.
        manifest_bytes.extend_from_slice(&0xFF00u16.to_le_bytes());
        manifest_bytes.extend_from_slice(&4u16.to_le_bytes());
        manifest_bytes.extend_from_slice(&[0x01, 0x02, 0x03, 0x04]);
        // Known tag: total size.
        manifest_bytes.extend_from_slice(&DL_TAG_TOTAL_SIZE.to_le_bytes());
        manifest_bytes.extend_from_slice(&8u16.to_le_bytes());
        manifest_bytes.extend_from_slice(&42u64.to_le_bytes());

        // Build a minimal seed with this manifest injected.
        let header = SeedHeader {
            seed_magic: SEED_MAGIC,
            seed_version: 1,
            flags: SEED_HAS_DOWNLOAD,
            file_id: [0x09; 8],
            total_vector_count: 100,
            dimension: 128,
            base_dtype: 0,
            profile_id: 0,
            created_ns: 0,
            microkernel_offset: SEED_HEADER_SIZE as u32,
            microkernel_size: 0,
            download_manifest_offset: SEED_HEADER_SIZE as u32,
            download_manifest_size: manifest_bytes.len() as u32,
            sig_algo: 0,
            sig_length: 0,
            total_seed_size: SEED_HEADER_SIZE as u32 + manifest_bytes.len() as u32,
            content_hash: [0; 8],
        };

        let mut payload = Vec::new();
        payload.extend_from_slice(&header.to_bytes());
        payload.extend_from_slice(&manifest_bytes);

        let parsed = ParsedSeed::parse(&payload).unwrap();
        let manifest = parsed.parse_manifest().unwrap();
        assert_eq!(manifest.total_file_size, Some(42));
        assert!(manifest.hosts.is_empty()); // Unknown tag was skipped.
    }

    #[test]
    fn seed_error_display() {
        let e = SeedError::TooLarge {
            size: 3000,
            max: 2953,
        };
        assert!(format!("{e}").contains("3000"));
        assert!(format!("{e}").contains("2953"));
    }
}
