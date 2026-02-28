//! Certificate system for cut verification
//!
//! Provides provable certificates that a minimum cut is correct.
//! Each certificate includes:
//! - The witnesses that define the cut
//! - The LocalKCut responses that prove no smaller cut exists
//! - A proof structure for verification

use crate::graph::{EdgeId, VertexId};
use crate::instance::WitnessHandle;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

pub mod audit;

pub use audit::{AuditData, AuditEntry, AuditEntryType, AuditLogger};

/// Witness summary for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WitnessSummary {
    /// Seed vertex
    pub seed: u64,
    /// Boundary size
    pub boundary: u64,
    /// Number of vertices in the cut
    pub cardinality: u64,
}

/// A certificate proving a minimum cut value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CutCertificate {
    /// The witnesses (candidate cuts) that were maintained (non-serializable)
    #[serde(skip)]
    pub witnesses: Vec<WitnessHandle>,
    /// Witness summaries for serialization
    pub witness_summaries: Vec<WitnessSummary>,
    /// LocalKCut responses that prove no smaller cut exists
    pub localkcut_responses: Vec<LocalKCutResponse>,
    /// Index of the best witness (smallest boundary)
    pub best_witness_idx: Option<usize>,
    /// Timestamp when certificate was created
    #[serde(with = "system_time_serde")]
    pub timestamp: SystemTime,
    /// Certificate version for compatibility
    pub version: u32,
}

/// Serde serialization for SystemTime
mod system_time_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::{SystemTime, UNIX_EPOCH};

    pub fn serialize<S>(time: &SystemTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        duration.as_secs().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<SystemTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(UNIX_EPOCH + std::time::Duration::from_secs(secs))
    }
}

/// A response from the LocalKCut oracle
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalKCutResponse {
    /// The query that was made
    pub query: CertLocalKCutQuery,
    /// The result returned
    pub result: LocalKCutResultSummary,
    /// Timestamp of the query
    pub timestamp: u64,
    /// Optional provenance (which update triggered this)
    pub trigger: Option<UpdateTrigger>,
}

impl LocalKCutResponse {
    /// Create a new LocalKCut response
    pub fn new(
        query: CertLocalKCutQuery,
        result: LocalKCutResultSummary,
        timestamp: u64,
        trigger: Option<UpdateTrigger>,
    ) -> Self {
        Self {
            query,
            result,
            timestamp,
            trigger,
        }
    }
}

/// A query to the LocalKCut oracle (certificate version)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertLocalKCutQuery {
    /// Seed vertices for the search
    pub seed_vertices: Vec<VertexId>,
    /// Budget k for cut size
    pub budget_k: u64,
    /// Search radius
    pub radius: usize,
}

impl CertLocalKCutQuery {
    /// Create a new LocalKCut query
    pub fn new(seed_vertices: Vec<VertexId>, budget_k: u64, radius: usize) -> Self {
        Self {
            seed_vertices,
            budget_k,
            radius,
        }
    }
}

/// Summary of LocalKCut result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LocalKCutResultSummary {
    /// Found a cut within the budget
    Found {
        /// The cut value found
        cut_value: u64,
        /// Hash of the witness for verification
        witness_hash: u64,
    },
    /// No cut found in the local neighborhood
    NoneInLocality,
}

/// Trigger for an update operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTrigger {
    /// Type of update (insert or delete)
    pub update_type: UpdateType,
    /// Edge ID involved
    pub edge_id: EdgeId,
    /// Endpoints of the edge
    pub endpoints: (VertexId, VertexId),
    /// Timestamp of the update
    pub time: u64,
}

impl UpdateTrigger {
    /// Create a new update trigger
    pub fn new(
        update_type: UpdateType,
        edge_id: EdgeId,
        endpoints: (VertexId, VertexId),
        time: u64,
    ) -> Self {
        Self {
            update_type,
            edge_id,
            endpoints,
            time,
        }
    }
}

/// Type of graph update
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateType {
    /// Edge insertion
    Insert,
    /// Edge deletion
    Delete,
}

/// Current certificate version
pub const CERTIFICATE_VERSION: u32 = 1;

impl CutCertificate {
    /// Create a new empty certificate
    pub fn new() -> Self {
        Self {
            witnesses: Vec::new(),
            witness_summaries: Vec::new(),
            localkcut_responses: Vec::new(),
            best_witness_idx: None,
            timestamp: SystemTime::now(),
            version: CERTIFICATE_VERSION,
        }
    }

    /// Create a certificate with initial witnesses
    pub fn with_witnesses(witnesses: Vec<WitnessHandle>) -> Self {
        let mut cert = Self::new();
        let summaries: Vec<WitnessSummary> = witnesses
            .iter()
            .map(|w| WitnessSummary {
                seed: w.seed(),
                boundary: w.boundary_size(),
                cardinality: w.cardinality(),
            })
            .collect();
        cert.witnesses = witnesses;
        cert.witness_summaries = summaries;
        cert
    }

    /// Add a LocalKCut response to the certificate
    pub fn add_response(&mut self, response: LocalKCutResponse) {
        self.localkcut_responses.push(response);
    }

    /// Update the best witness
    pub fn set_best_witness(&mut self, idx: usize, witness: WitnessHandle) {
        let summary = WitnessSummary {
            seed: witness.seed(),
            boundary: witness.boundary_size(),
            cardinality: witness.cardinality(),
        };

        if idx < self.witnesses.len() {
            self.witnesses[idx] = witness;
            self.witness_summaries[idx] = summary;
            self.best_witness_idx = Some(idx);
        } else {
            self.witnesses.push(witness);
            self.witness_summaries.push(summary);
            self.best_witness_idx = Some(self.witnesses.len() - 1);
        }
    }

    /// Verify the certificate is internally consistent
    pub fn verify(&self) -> Result<(), CertificateError> {
        // Check version compatibility
        if self.version > CERTIFICATE_VERSION {
            return Err(CertificateError::IncompatibleVersion {
                found: self.version,
                expected: CERTIFICATE_VERSION,
            });
        }

        // Check if we have at least one witness summary (for deserialized certs)
        // or witness (for in-memory certs)
        if self.witnesses.is_empty() && self.witness_summaries.is_empty() {
            return Err(CertificateError::NoWitness);
        }

        // Verify best witness index is valid
        if let Some(idx) = self.best_witness_idx {
            let max_idx = self.witnesses.len().max(self.witness_summaries.len());
            if max_idx > 0 && idx >= max_idx {
                return Err(CertificateError::InvalidWitnessIndex {
                    index: idx,
                    max: max_idx - 1,
                });
            }
        }

        // Verify consistency of LocalKCut responses
        for response in &self.localkcut_responses {
            if response.query.budget_k == 0 {
                return Err(CertificateError::InvalidQuery {
                    reason: "Budget k must be positive".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Get the certified minimum cut value
    pub fn certified_value(&self) -> Option<u64> {
        self.best_witness_idx
            .and_then(|idx| self.witnesses.get(idx).map(|w| w.boundary_size()))
    }

    /// Get the best witness
    pub fn best_witness(&self) -> Option<&WitnessHandle> {
        self.best_witness_idx
            .and_then(|idx| self.witnesses.get(idx))
    }

    /// Export to JSON for external verification
    pub fn to_json(&self) -> Result<String, CertificateError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| CertificateError::SerializationError(e.to_string()))
    }

    /// Import from JSON
    pub fn from_json(json: &str) -> Result<Self, CertificateError> {
        serde_json::from_str(json)
            .map_err(|e| CertificateError::DeserializationError(e.to_string()))
    }

    /// Get number of witnesses
    pub fn num_witnesses(&self) -> usize {
        self.witnesses.len()
    }

    /// Get number of LocalKCut responses
    pub fn num_responses(&self) -> usize {
        self.localkcut_responses.len()
    }

    /// Get all witnesses
    pub fn witnesses(&self) -> &[WitnessHandle] {
        &self.witnesses
    }

    /// Get all LocalKCut responses
    pub fn responses(&self) -> &[LocalKCutResponse] {
        &self.localkcut_responses
    }
}

impl Default for CutCertificate {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during certificate operations
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateError {
    /// No witness available in certificate
    NoWitness,
    /// Inconsistent boundary calculation
    InconsistentBoundary {
        /// Expected boundary value
        expected: u64,
        /// Actual boundary value
        actual: u64,
    },
    /// Missing LocalKCut proof for a required operation
    MissingLocalKCutProof {
        /// Description of missing proof
        operation: String,
    },
    /// Invalid witness index
    InvalidWitnessIndex {
        /// The invalid index
        index: usize,
        /// Maximum valid index
        max: usize,
    },
    /// Invalid query parameters
    InvalidQuery {
        /// Reason for invalidity
        reason: String,
    },
    /// Incompatible certificate version
    IncompatibleVersion {
        /// Version found in certificate
        found: u32,
        /// Expected version
        expected: u32,
    },
    /// Serialization error
    SerializationError(String),
    /// Deserialization error
    DeserializationError(String),
}

impl std::fmt::Display for CertificateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoWitness => write!(f, "No witness available in certificate"),
            Self::InconsistentBoundary { expected, actual } => {
                write!(
                    f,
                    "Inconsistent boundary: expected {}, got {}",
                    expected, actual
                )
            }
            Self::MissingLocalKCutProof { operation } => {
                write!(f, "Missing LocalKCut proof for operation: {}", operation)
            }
            Self::InvalidWitnessIndex { index, max } => {
                write!(f, "Invalid witness index {} (max: {})", index, max)
            }
            Self::InvalidQuery { reason } => {
                write!(f, "Invalid query: {}", reason)
            }
            Self::IncompatibleVersion { found, expected } => {
                write!(
                    f,
                    "Incompatible version: found {}, expected {}",
                    found, expected
                )
            }
            Self::SerializationError(msg) => {
                write!(f, "Serialization error: {}", msg)
            }
            Self::DeserializationError(msg) => {
                write!(f, "Deserialization error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CertificateError {}

#[cfg(test)]
mod tests {
    use super::*;
    use roaring::RoaringBitmap;

    #[test]
    fn test_new_certificate() {
        let cert = CutCertificate::new();
        assert_eq!(cert.num_witnesses(), 0);
        assert_eq!(cert.num_responses(), 0);
        assert_eq!(cert.version, CERTIFICATE_VERSION);
        assert!(cert.best_witness_idx.is_none());
    }

    #[test]
    fn test_add_witness() {
        let mut cert = CutCertificate::new();
        let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);

        cert.set_best_witness(0, witness.clone());
        assert_eq!(cert.num_witnesses(), 1);
        assert_eq!(cert.best_witness_idx, Some(0));
        assert_eq!(cert.certified_value(), Some(5));
    }

    #[test]
    fn test_add_response() {
        let mut cert = CutCertificate::new();
        let query = CertLocalKCutQuery::new(vec![1, 2], 10, 5);
        let result = LocalKCutResultSummary::Found {
            cut_value: 5,
            witness_hash: 12345,
        };
        let response = LocalKCutResponse::new(query, result, 100, None);

        cert.add_response(response);
        assert_eq!(cert.num_responses(), 1);
    }

    #[test]
    fn test_verify_empty() {
        let cert = CutCertificate::new();
        assert!(matches!(cert.verify(), Err(CertificateError::NoWitness)));
    }

    #[test]
    fn test_verify_valid() {
        let mut cert = CutCertificate::new();
        let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 3);
        cert.set_best_witness(0, witness);

        assert!(cert.verify().is_ok());
    }

    #[test]
    fn test_verify_invalid_index() {
        let mut cert = CutCertificate::new();
        // Add a witness so the certificate is not empty
        let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 5);
        cert.set_best_witness(0, witness);
        // Now set an invalid index
        cert.best_witness_idx = Some(5);

        let result = cert.verify();
        assert!(matches!(
            result,
            Err(CertificateError::InvalidWitnessIndex { .. })
        ));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut cert = CutCertificate::new();
        let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);
        cert.set_best_witness(0, witness);

        let query = CertLocalKCutQuery::new(vec![1], 5, 2);
        let result = LocalKCutResultSummary::Found {
            cut_value: 3,
            witness_hash: 999,
        };
        let response = LocalKCutResponse::new(query, result, 100, None);
        cert.add_response(response);

        let json = cert.to_json().unwrap();
        let cert2 = CutCertificate::from_json(&json).unwrap();

        // Witnesses are not serialized, only summaries
        assert_eq!(cert2.witness_summaries.len(), 1);
        assert_eq!(cert2.num_responses(), 1);
        assert_eq!(cert2.version, cert.version);
    }

    #[test]
    fn test_best_witness() {
        let mut cert = CutCertificate::new();
        let witness1 = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2]), 10);
        let witness2 = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 3, 4]), 5);

        cert.set_best_witness(0, witness1);
        cert.set_best_witness(1, witness2);

        let best = cert.best_witness().unwrap();
        assert_eq!(best.boundary_size(), 5);
    }

    #[test]
    fn test_update_trigger() {
        let trigger = UpdateTrigger::new(UpdateType::Insert, 123, (1, 2), 1000);
        assert_eq!(trigger.update_type, UpdateType::Insert);
        assert_eq!(trigger.edge_id, 123);
        assert_eq!(trigger.endpoints, (1, 2));
        assert_eq!(trigger.time, 1000);
    }

    #[test]
    fn test_local_kcut_query() {
        let query = CertLocalKCutQuery::new(vec![1, 2, 3], 10, 5);
        assert_eq!(query.seed_vertices.len(), 3);
        assert_eq!(query.budget_k, 10);
        assert_eq!(query.radius, 5);
    }

    #[test]
    fn test_local_kcut_response() {
        let query = CertLocalKCutQuery::new(vec![1], 5, 2);
        let result = LocalKCutResultSummary::Found {
            cut_value: 3,
            witness_hash: 999,
        };
        let response = LocalKCutResponse::new(query, result, 500, None);

        assert_eq!(response.timestamp, 500);
        assert!(response.trigger.is_none());
    }

    #[test]
    fn test_certificate_error_display() {
        let err = CertificateError::NoWitness;
        assert!(err.to_string().contains("No witness"));

        let err = CertificateError::InvalidWitnessIndex { index: 5, max: 3 };
        assert!(err.to_string().contains("Invalid witness index"));
    }
}
