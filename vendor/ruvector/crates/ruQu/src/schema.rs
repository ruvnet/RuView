//! Data Model and Schema for ruQu
//!
//! Defines the core data types and a versioned binary log format.
//!
//! ## Binary Format
//!
//! The log format is designed for speed and compactness:
//! - 4-byte magic header: "RUQU"
//! - 1-byte version
//! - Sequence of variable-length records
//!
//! Each record:
//! - 1-byte record type
//! - 4-byte length (little-endian)
//! - Payload bytes
//! - 4-byte CRC32 checksum

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Current schema version
pub const SCHEMA_VERSION: u8 = 1;

/// Magic header for binary logs
pub const LOG_MAGIC: &[u8; 4] = b"RUQU";

// ============================================================================
// CORE DATA TYPES
// ============================================================================

/// A single syndrome measurement round
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyndromeRound {
    /// Round number (monotonically increasing)
    pub round_id: u64,
    /// Timestamp in nanoseconds since epoch
    pub timestamp_ns: u64,
    /// Code distance
    pub code_distance: u8,
    /// Detector events in this round
    pub events: Vec<DetectorEvent>,
    /// Optional metadata
    #[serde(default)]
    pub metadata: RoundMetadata,
}

impl SyndromeRound {
    /// Create a new syndrome round
    pub fn new(round_id: u64, code_distance: u8) -> Self {
        Self {
            round_id,
            timestamp_ns: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(0),
            code_distance,
            events: Vec::new(),
            metadata: RoundMetadata::default(),
        }
    }

    /// Add a detector event
    pub fn add_event(&mut self, event: DetectorEvent) {
        self.events.push(event);
    }

    /// Get the number of fired detectors
    pub fn fired_count(&self) -> usize {
        self.events.iter().filter(|e| e.fired).count()
    }
}

/// A detector event within a syndrome round
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DetectorEvent {
    /// Detector index
    pub detector_id: u32,
    /// Whether the detector fired (syndrome bit = 1)
    pub fired: bool,
    /// Measurement confidence (0.0 to 1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    /// Spatial coordinates (if known)
    #[serde(default)]
    pub coords: Option<DetectorCoords>,
}

fn default_confidence() -> f32 {
    1.0
}

/// Spatial coordinates of a detector
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct DetectorCoords {
    /// X coordinate (column)
    pub x: i16,
    /// Y coordinate (row)
    pub y: i16,
    /// Time slice (for 3D codes)
    pub t: i16,
}

/// Round metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct RoundMetadata {
    /// Source identifier
    #[serde(default)]
    pub source: String,
    /// Error rate at this round (if known)
    #[serde(default)]
    pub error_rate: Option<f64>,
    /// Whether this round is from a hardware run
    #[serde(default)]
    pub is_hardware: bool,
    /// Injected fault (if any)
    #[serde(default)]
    pub injected_fault: Option<String>,
}

/// Boundary identifier for surface codes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum BoundaryId {
    /// Left boundary (X logical)
    Left,
    /// Right boundary (X logical)
    Right,
    /// Top boundary (Z logical)
    Top,
    /// Bottom boundary (Z logical)
    Bottom,
    /// Virtual boundary (for matching)
    Virtual,
    /// Custom boundary with ID
    Custom(u32),
}

/// A permit token issued by the gate
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermitToken {
    /// Unique token ID
    pub token_id: u64,
    /// Round at which permit was issued
    pub issued_at_round: u64,
    /// Timestamp when issued (ns since epoch)
    pub issued_at_ns: u64,
    /// Time-to-live in nanoseconds
    pub ttl_ns: u64,
    /// Permitted regions (bitmask)
    pub region_mask: u64,
    /// Confidence level
    pub confidence: f32,
    /// Min-cut value at issuance
    pub min_cut_value: f32,
}

impl PermitToken {
    /// Check if the token is still valid
    pub fn is_valid(&self, current_time_ns: u64) -> bool {
        current_time_ns < self.issued_at_ns.saturating_add(self.ttl_ns)
    }

    /// Remaining time-to-live in nanoseconds
    pub fn remaining_ttl_ns(&self, current_time_ns: u64) -> u64 {
        self.issued_at_ns
            .saturating_add(self.ttl_ns)
            .saturating_sub(current_time_ns)
    }
}

/// A gate decision record
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GateDecision {
    /// Round ID when decision was made
    pub round_id: u64,
    /// Timestamp of decision (ns since epoch)
    pub timestamp_ns: u64,
    /// Decision type
    pub decision: DecisionType,
    /// Processing latency in nanoseconds
    pub latency_ns: u64,
    /// Input metrics
    pub metrics: GateMetrics,
}

/// Decision type
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DecisionType {
    /// Operation permitted with token
    Permit(PermitToken),
    /// Operation deferred
    Defer {
        /// Wait time in nanoseconds
        wait_ns: u64,
        /// Uncertainty level
        uncertainty: f32,
    },
    /// Operation denied
    Deny {
        /// Risk level (0-1)
        risk_level: f32,
        /// Affected region bitmask
        affected_regions: u64,
    },
}

/// Metrics used for gate decision
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct GateMetrics {
    /// Min-cut value
    pub min_cut: f32,
    /// Cut value standard deviation
    pub cut_std: f32,
    /// Shift from baseline
    pub shift: f32,
    /// Evidence accumulation
    pub evidence: f32,
    /// Number of fired detectors
    pub fired_count: u32,
    /// Clustering score
    pub clustering: f32,
}

/// A mitigation action taken
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MitigationAction {
    /// Action ID
    pub action_id: u64,
    /// Timestamp when action was initiated
    pub timestamp_ns: u64,
    /// Action type
    pub action_type: ActionTypeSchema,
    /// Target regions
    pub target_regions: Vec<u32>,
    /// Duration in nanoseconds
    pub duration_ns: u64,
    /// Result of the action
    pub result: ActionResult,
}

/// Action types in schema format
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionTypeSchema {
    /// Quarantine a region to prevent error propagation
    QuarantineRegion,
    /// Increase syndrome measurement rounds for higher fidelity
    IncreaseSyndromeRounds,
    /// Switch decoder mode (e.g., from fast to accurate)
    SwitchDecodeMode,
    /// Trigger re-weighting of decoder graph
    TriggerReweight,
    /// Pause learning/write operations during instability
    PauseLearningWrites,
    /// Log event for audit trail
    LogEvent,
    /// Alert human operator
    AlertOperator,
}

/// Result of a mitigation action
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ActionResult {
    /// Action completed successfully
    Success,
    /// Action partially completed
    Partial {
        /// Fraction completed (0.0 to 1.0)
        completed: f32,
    },
    /// Action failed
    Failed {
        /// Reason for failure
        reason: String,
    },
    /// Action is pending execution
    Pending,
}

// ============================================================================
// BINARY LOG FORMAT
// ============================================================================

/// Record types for binary log
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RecordType {
    /// Syndrome round record
    SyndromeRound = 1,
    /// Gate decision record
    GateDecision = 2,
    /// Mitigation action record
    MitigationAction = 3,
    /// Checkpoint for log recovery
    Checkpoint = 4,
    /// Configuration snapshot
    Config = 5,
    /// Metrics snapshot
    Metrics = 6,
}

impl TryFrom<u8> for RecordType {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(RecordType::SyndromeRound),
            2 => Ok(RecordType::GateDecision),
            3 => Ok(RecordType::MitigationAction),
            4 => Ok(RecordType::Checkpoint),
            5 => Ok(RecordType::Config),
            6 => Ok(RecordType::Metrics),
            _ => Err(()),
        }
    }
}

/// Binary log writer
pub struct LogWriter<W: Write> {
    writer: W,
    record_count: u64,
}

impl<W: Write> LogWriter<W> {
    /// Create a new log writer
    pub fn new(mut writer: W) -> std::io::Result<Self> {
        // Write header
        writer.write_all(LOG_MAGIC)?;
        writer.write_all(&[SCHEMA_VERSION])?;
        Ok(Self {
            writer,
            record_count: 0,
        })
    }

    /// Write a syndrome round
    pub fn write_syndrome(&mut self, round: &SyndromeRound) -> std::io::Result<()> {
        let payload = serde_json::to_vec(round)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.write_record(RecordType::SyndromeRound, &payload)
    }

    /// Write a gate decision
    pub fn write_decision(&mut self, decision: &GateDecision) -> std::io::Result<()> {
        let payload = serde_json::to_vec(decision)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.write_record(RecordType::GateDecision, &payload)
    }

    /// Write a mitigation action
    pub fn write_action(&mut self, action: &MitigationAction) -> std::io::Result<()> {
        let payload = serde_json::to_vec(action)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.write_record(RecordType::MitigationAction, &payload)
    }

    fn write_record(&mut self, record_type: RecordType, payload: &[u8]) -> std::io::Result<()> {
        // Record type (1 byte)
        self.writer.write_all(&[record_type as u8])?;

        // Length (4 bytes, little-endian)
        let len = payload.len() as u32;
        self.writer.write_all(&len.to_le_bytes())?;

        // Payload
        self.writer.write_all(payload)?;

        // CRC32 checksum
        let crc = crc32fast::hash(payload);
        self.writer.write_all(&crc.to_le_bytes())?;

        self.record_count += 1;
        Ok(())
    }

    /// Flush and get record count
    pub fn finish(mut self) -> std::io::Result<u64> {
        self.writer.flush()?;
        Ok(self.record_count)
    }
}

/// Binary log reader
pub struct LogReader<R: Read> {
    reader: R,
    version: u8,
}

impl<R: Read> LogReader<R> {
    /// Open a log for reading
    pub fn new(mut reader: R) -> std::io::Result<Self> {
        // Read and verify header
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic)?;
        if &magic != LOG_MAGIC {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid magic header",
            ));
        }

        let mut version = [0u8; 1];
        reader.read_exact(&mut version)?;

        Ok(Self {
            reader,
            version: version[0],
        })
    }

    /// Get schema version
    pub fn version(&self) -> u8 {
        self.version
    }

    /// Read next record
    pub fn read_record(&mut self) -> std::io::Result<Option<LogRecord>> {
        // Read record type
        let mut type_byte = [0u8; 1];
        match self.reader.read_exact(&mut type_byte) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e),
        }

        let record_type = RecordType::try_from(type_byte[0]).map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, "Unknown record type")
        })?;

        // Read length
        let mut len_bytes = [0u8; 4];
        self.reader.read_exact(&mut len_bytes)?;
        let len = u32::from_le_bytes(len_bytes) as usize;

        // Read payload
        let mut payload = vec![0u8; len];
        self.reader.read_exact(&mut payload)?;

        // Read and verify checksum
        let mut crc_bytes = [0u8; 4];
        self.reader.read_exact(&mut crc_bytes)?;
        let stored_crc = u32::from_le_bytes(crc_bytes);
        let computed_crc = crc32fast::hash(&payload);

        if stored_crc != computed_crc {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "CRC mismatch",
            ));
        }

        // Parse payload
        let record = match record_type {
            RecordType::SyndromeRound => {
                let round: SyndromeRound = serde_json::from_slice(&payload)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                LogRecord::Syndrome(round)
            }
            RecordType::GateDecision => {
                let decision: GateDecision = serde_json::from_slice(&payload)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                LogRecord::Decision(decision)
            }
            RecordType::MitigationAction => {
                let action: MitigationAction = serde_json::from_slice(&payload)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
                LogRecord::Action(action)
            }
            _ => LogRecord::Unknown(payload),
        };

        Ok(Some(record))
    }
}

/// A record from the log
#[derive(Debug, Clone)]
pub enum LogRecord {
    /// Syndrome round record
    Syndrome(SyndromeRound),
    /// Gate decision record
    Decision(GateDecision),
    /// Mitigation action record
    Action(MitigationAction),
    /// Unknown record type (for forward compatibility)
    Unknown(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_syndrome_round() {
        let mut round = SyndromeRound::new(1, 5);
        round.add_event(DetectorEvent {
            detector_id: 0,
            fired: true,
            confidence: 0.99,
            coords: Some(DetectorCoords { x: 0, y: 0, t: 0 }),
        });
        round.add_event(DetectorEvent {
            detector_id: 1,
            fired: false,
            confidence: 1.0,
            coords: None,
        });

        assert_eq!(round.fired_count(), 1);
    }

    #[test]
    fn test_permit_token_validity() {
        let token = PermitToken {
            token_id: 1,
            issued_at_round: 100,
            issued_at_ns: 1000000,
            ttl_ns: 100000,
            region_mask: 0xFF,
            confidence: 0.95,
            min_cut_value: 5.5,
        };

        assert!(token.is_valid(1050000));
        assert!(!token.is_valid(1200000));
        assert_eq!(token.remaining_ttl_ns(1050000), 50000);
    }

    #[test]
    fn test_log_roundtrip() {
        let mut buffer = Vec::new();

        // Write
        {
            let mut writer = LogWriter::new(&mut buffer).unwrap();
            let round = SyndromeRound::new(1, 5);
            writer.write_syndrome(&round).unwrap();
            writer.finish().unwrap();
        }

        // Read
        {
            let mut reader = LogReader::new(Cursor::new(&buffer)).unwrap();
            assert_eq!(reader.version(), SCHEMA_VERSION);

            let record = reader.read_record().unwrap().unwrap();
            match record {
                LogRecord::Syndrome(round) => {
                    assert_eq!(round.round_id, 1);
                    assert_eq!(round.code_distance, 5);
                }
                _ => panic!("Expected syndrome record"),
            }
        }
    }
}
