//! RuVector File (RVF) format types for serialization.

use serde::{Deserialize, Serialize};

use crate::error::{Result, RuvNeuralError};

/// Magic bytes for the RVF file format.
pub const RVF_MAGIC: [u8; 4] = [b'R', b'V', b'F', 0x01];

/// Current RVF format version.
pub const RVF_VERSION: u8 = 1;

/// Maximum allowed metadata JSON length (16 MiB).
pub const MAX_METADATA_LEN: u32 = 16 * 1024 * 1024;

/// Maximum allowed payload length when reading (256 MiB).
pub const MAX_PAYLOAD_LEN: usize = 256 * 1024 * 1024;

/// Data type stored in an RVF file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RvfDataType {
    /// Brain connectivity graph.
    BrainGraph,
    /// Neural embedding vector.
    NeuralEmbedding,
    /// Topology metrics snapshot.
    TopologyMetrics,
    /// Mincut result.
    MincutResult,
    /// Time series chunk.
    TimeSeriesChunk,
}

impl RvfDataType {
    /// Convert to a byte tag for binary encoding.
    pub fn to_tag(&self) -> u8 {
        match self {
            RvfDataType::BrainGraph => 0,
            RvfDataType::NeuralEmbedding => 1,
            RvfDataType::TopologyMetrics => 2,
            RvfDataType::MincutResult => 3,
            RvfDataType::TimeSeriesChunk => 4,
        }
    }

    /// Parse a byte tag back to a data type.
    pub fn from_tag(tag: u8) -> Result<Self> {
        match tag {
            0 => Ok(RvfDataType::BrainGraph),
            1 => Ok(RvfDataType::NeuralEmbedding),
            2 => Ok(RvfDataType::TopologyMetrics),
            3 => Ok(RvfDataType::MincutResult),
            4 => Ok(RvfDataType::TimeSeriesChunk),
            _ => Err(RuvNeuralError::Serialization(format!(
                "Unknown RVF data type tag: {}",
                tag
            ))),
        }
    }
}

/// RVF file header (fixed-size, 20 bytes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvfHeader {
    /// Magic bytes: `b"RVF\x01"`.
    pub magic: [u8; 4],
    /// Format version.
    pub version: u8,
    /// Type of data stored.
    pub data_type: RvfDataType,
    /// Number of entries in the file.
    pub num_entries: u64,
    /// Embedding dimensionality (0 if not applicable).
    pub embedding_dim: u32,
    /// Length of the JSON metadata section in bytes.
    pub metadata_json_len: u32,
}

impl RvfHeader {
    /// Create a new header with default magic and version.
    pub fn new(data_type: RvfDataType, num_entries: u64, embedding_dim: u32) -> Self {
        Self {
            magic: RVF_MAGIC,
            version: RVF_VERSION,
            data_type,
            num_entries,
            embedding_dim,
            metadata_json_len: 0,
        }
    }

    /// Validate that this header has correct magic bytes and a known version.
    pub fn validate(&self) -> Result<()> {
        if self.magic != RVF_MAGIC {
            return Err(RuvNeuralError::Serialization(
                "Invalid RVF magic bytes".into(),
            ));
        }
        if self.version != RVF_VERSION {
            return Err(RuvNeuralError::Serialization(format!(
                "Unsupported RVF version: {} (expected {})",
                self.version, RVF_VERSION
            )));
        }
        Ok(())
    }

    /// Encode the header to bytes (little-endian).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(20);
        buf.extend_from_slice(&self.magic);
        buf.push(self.version);
        buf.push(self.data_type.to_tag());
        buf.extend_from_slice(&self.num_entries.to_le_bytes());
        buf.extend_from_slice(&self.embedding_dim.to_le_bytes());
        buf.extend_from_slice(&self.metadata_json_len.to_le_bytes());
        buf
    }

    /// Decode a header from bytes.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 22 {
            return Err(RuvNeuralError::Serialization(format!(
                "RVF header too short: {} bytes (need 22)",
                bytes.len()
            )));
        }
        let mut magic = [0u8; 4];
        magic.copy_from_slice(&bytes[0..4]);
        let version = bytes[4];
        let data_type = RvfDataType::from_tag(bytes[5])?;
        let num_entries = u64::from_le_bytes(bytes[6..14].try_into().unwrap());
        let embedding_dim = u32::from_le_bytes(bytes[14..18].try_into().unwrap());
        let metadata_json_len = u32::from_le_bytes(bytes[18..22].try_into().unwrap());

        Ok(Self {
            magic,
            version,
            data_type,
            num_entries,
            embedding_dim,
            metadata_json_len,
        })
    }
}

/// An RVF file containing header, metadata, and binary data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RvfFile {
    /// File header.
    pub header: RvfHeader,
    /// JSON metadata.
    pub metadata: serde_json::Value,
    /// Raw binary payload.
    pub data: Vec<u8>,
}

impl RvfFile {
    /// Create a new empty RVF file for a given data type.
    pub fn new(data_type: RvfDataType) -> Self {
        Self {
            header: RvfHeader::new(data_type, 0, 0),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            data: Vec::new(),
        }
    }

    /// Write the RVF file to a writer.
    pub fn write_to<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
        let meta_bytes = serde_json::to_vec(&self.metadata)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        let mut header = self.header.clone();
        header.metadata_json_len = meta_bytes.len() as u32;

        writer
            .write_all(&header.to_bytes())
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;
        writer
            .write_all(&meta_bytes)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;
        writer
            .write_all(&self.data)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        Ok(())
    }

    /// Read an RVF file from a reader.
    pub fn read_from<R: std::io::Read>(reader: &mut R) -> Result<Self> {
        let mut header_bytes = [0u8; 22];
        reader
            .read_exact(&mut header_bytes)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        let header = RvfHeader::from_bytes(&header_bytes)?;
        header.validate()?;

        if header.metadata_json_len > MAX_METADATA_LEN {
            return Err(RuvNeuralError::Serialization(format!(
                "RVF metadata length {} exceeds maximum {}",
                header.metadata_json_len, MAX_METADATA_LEN
            )));
        }

        let mut meta_bytes = vec![0u8; header.metadata_json_len as usize];
        reader
            .read_exact(&mut meta_bytes)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        let metadata: serde_json::Value = serde_json::from_slice(&meta_bytes)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        let mut data = Vec::new();
        reader
            .read_to_end(&mut data)
            .map_err(|e| RuvNeuralError::Serialization(e.to_string()))?;

        if data.len() > MAX_PAYLOAD_LEN {
            return Err(RuvNeuralError::Serialization(format!(
                "RVF payload length {} exceeds maximum {}",
                data.len(), MAX_PAYLOAD_LEN
            )));
        }

        Ok(Self {
            header,
            metadata,
            data,
        })
    }
}
