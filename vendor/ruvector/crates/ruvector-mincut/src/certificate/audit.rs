//! Audit trail for cut changes
//!
//! Logs every witness change with full provenance.

use super::{CertLocalKCutQuery, LocalKCutResponse, LocalKCutResultSummary, UpdateTrigger};
use crate::instance::WitnessHandle;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Audit log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    /// Entry ID
    pub id: u64,
    /// Timestamp (seconds since UNIX epoch)
    pub timestamp: u64,
    /// Type of entry
    pub entry_type: AuditEntryType,
    /// Associated data
    pub data: AuditData,
}

impl AuditEntry {
    /// Create a new audit entry
    pub fn new(id: u64, entry_type: AuditEntryType, data: AuditData) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self {
            id,
            timestamp,
            entry_type,
            data,
        }
    }
}

/// Type of audit entry
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuditEntryType {
    /// A witness was created
    WitnessCreated,
    /// A witness was updated
    WitnessUpdated,
    /// A witness was evicted from the cache
    WitnessEvicted,
    /// A LocalKCut query was made
    LocalKCutQuery,
    /// A LocalKCut response was received
    LocalKCutResponse,
    /// A certificate was created
    CertificateCreated,
    /// The minimum cut value changed
    MinCutChanged,
}

/// Data associated with an audit entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditData {
    /// Witness-related data
    Witness {
        /// Hash of the witness
        hash: u64,
        /// Boundary value
        boundary: u64,
        /// Seed vertex
        seed: u64,
    },
    /// Query data
    Query {
        /// Budget parameter
        budget: u64,
        /// Search radius
        radius: usize,
        /// Seed vertices
        seeds: Vec<u64>,
    },
    /// Response data
    Response {
        /// Whether a cut was found
        found: bool,
        /// Cut value if found
        value: Option<u64>,
    },
    /// Minimum cut change
    MinCut {
        /// Old minimum cut value
        old_value: u64,
        /// New minimum cut value
        new_value: u64,
        /// Update that triggered the change
        trigger: UpdateTrigger,
    },
    /// Certificate creation
    Certificate {
        /// Number of witnesses
        num_witnesses: usize,
        /// Number of responses
        num_responses: usize,
        /// Certified value
        certified_value: Option<u64>,
    },
}

/// Thread-safe audit logger
pub struct AuditLogger {
    /// Circular buffer of entries
    entries: Arc<RwLock<VecDeque<AuditEntry>>>,
    /// Maximum number of entries to keep
    max_entries: usize,
    /// Next entry ID
    next_id: Arc<RwLock<u64>>,
}

impl AuditLogger {
    /// Create a new audit logger with specified capacity
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(VecDeque::with_capacity(max_entries))),
            max_entries,
            next_id: Arc::new(RwLock::new(0)),
        }
    }

    /// Log a new entry
    pub fn log(&self, entry_type: AuditEntryType, data: AuditData) {
        let mut entries = self.entries.write().unwrap();
        let mut next_id = self.next_id.write().unwrap();

        let entry = AuditEntry::new(*next_id, entry_type, data);
        *next_id += 1;

        entries.push_back(entry);

        // Maintain maximum size
        while entries.len() > self.max_entries {
            entries.pop_front();
        }
    }

    /// Log witness creation
    pub fn log_witness_created(&self, witness: &WitnessHandle) {
        self.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: self.compute_witness_hash(witness),
                boundary: witness.boundary_size(),
                seed: witness.seed(),
            },
        );
    }

    /// Log witness update
    pub fn log_witness_updated(&self, witness: &WitnessHandle) {
        self.log(
            AuditEntryType::WitnessUpdated,
            AuditData::Witness {
                hash: self.compute_witness_hash(witness),
                boundary: witness.boundary_size(),
                seed: witness.seed(),
            },
        );
    }

    /// Log witness eviction
    pub fn log_witness_evicted(&self, witness: &WitnessHandle) {
        self.log(
            AuditEntryType::WitnessEvicted,
            AuditData::Witness {
                hash: self.compute_witness_hash(witness),
                boundary: witness.boundary_size(),
                seed: witness.seed(),
            },
        );
    }

    /// Log LocalKCut query
    pub fn log_query(&self, budget: u64, radius: usize, seeds: Vec<u64>) {
        self.log(
            AuditEntryType::LocalKCutQuery,
            AuditData::Query {
                budget,
                radius,
                seeds,
            },
        );
    }

    /// Log LocalKCut response
    pub fn log_response(&self, response: &LocalKCutResponse) {
        let (found, value) = match &response.result {
            super::LocalKCutResultSummary::Found { cut_value, .. } => (true, Some(*cut_value)),
            super::LocalKCutResultSummary::NoneInLocality => (false, None),
        };

        self.log(
            AuditEntryType::LocalKCutResponse,
            AuditData::Response { found, value },
        );
    }

    /// Log minimum cut change
    pub fn log_mincut_changed(&self, old_value: u64, new_value: u64, trigger: UpdateTrigger) {
        self.log(
            AuditEntryType::MinCutChanged,
            AuditData::MinCut {
                old_value,
                new_value,
                trigger,
            },
        );
    }

    /// Log certificate creation
    pub fn log_certificate_created(
        &self,
        num_witnesses: usize,
        num_responses: usize,
        certified_value: Option<u64>,
    ) {
        self.log(
            AuditEntryType::CertificateCreated,
            AuditData::Certificate {
                num_witnesses,
                num_responses,
                certified_value,
            },
        );
    }

    /// Get recent entries (up to count)
    pub fn recent(&self, count: usize) -> Vec<AuditEntry> {
        let entries = self.entries.read().unwrap();
        let start = entries.len().saturating_sub(count);
        entries.iter().skip(start).cloned().collect()
    }

    /// Get entries by type
    pub fn by_type(&self, entry_type: AuditEntryType) -> Vec<AuditEntry> {
        let entries = self.entries.read().unwrap();
        entries
            .iter()
            .filter(|e| e.entry_type == entry_type)
            .cloned()
            .collect()
    }

    /// Export full log
    pub fn export(&self) -> Vec<AuditEntry> {
        let entries = self.entries.read().unwrap();
        entries.iter().cloned().collect()
    }

    /// Export log to JSON
    pub fn to_json(&self) -> Result<String, String> {
        let entries = self.export();
        serde_json::to_string_pretty(&entries).map_err(|e| e.to_string())
    }

    /// Clear the log
    pub fn clear(&self) {
        let mut entries = self.entries.write().unwrap();
        entries.clear();
        let mut next_id = self.next_id.write().unwrap();
        *next_id = 0;
    }

    /// Get number of entries
    pub fn len(&self) -> usize {
        let entries = self.entries.read().unwrap();
        entries.len()
    }

    /// Check if log is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Get maximum capacity
    pub fn capacity(&self) -> usize {
        self.max_entries
    }

    /// Compute a simple hash for a witness
    fn compute_witness_hash(&self, witness: &WitnessHandle) -> u64 {
        // Simple hash combining seed and boundary
        let seed = witness.seed();
        let boundary = witness.boundary_size();
        seed.wrapping_mul(31).wrapping_add(boundary)
    }
}

impl Default for AuditLogger {
    fn default() -> Self {
        Self::new(1000)
    }
}

impl Clone for AuditLogger {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
            max_entries: self.max_entries,
            next_id: Arc::clone(&self.next_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::certificate::{CertLocalKCutQuery, LocalKCutResultSummary, UpdateType};
    use roaring::RoaringBitmap;

    #[test]
    fn test_new_logger() {
        let logger = AuditLogger::new(100);
        assert_eq!(logger.capacity(), 100);
        assert_eq!(logger.len(), 0);
        assert!(logger.is_empty());
    }

    #[test]
    fn test_log_entry() {
        let logger = AuditLogger::new(10);
        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 123,
                boundary: 5,
                seed: 1,
            },
        );

        assert_eq!(logger.len(), 1);
        assert!(!logger.is_empty());

        let entries = logger.export();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].entry_type, AuditEntryType::WitnessCreated);
    }

    #[test]
    fn test_log_witness_created() {
        let logger = AuditLogger::new(10);
        let witness = WitnessHandle::new(1, RoaringBitmap::from_iter([1, 2, 3]), 5);

        logger.log_witness_created(&witness);
        assert_eq!(logger.len(), 1);

        let entries = logger.by_type(AuditEntryType::WitnessCreated);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_log_witness_updated() {
        let logger = AuditLogger::new(10);
        let witness = WitnessHandle::new(2, RoaringBitmap::from_iter([2, 3]), 3);

        logger.log_witness_updated(&witness);
        assert_eq!(logger.len(), 1);

        let entries = logger.by_type(AuditEntryType::WitnessUpdated);
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_log_query() {
        let logger = AuditLogger::new(10);
        logger.log_query(10, 5, vec![1, 2, 3]);

        let entries = logger.by_type(AuditEntryType::LocalKCutQuery);
        assert_eq!(entries.len(), 1);

        if let AuditData::Query {
            budget,
            radius,
            seeds,
        } = &entries[0].data
        {
            assert_eq!(*budget, 10);
            assert_eq!(*radius, 5);
            assert_eq!(seeds.len(), 3);
        } else {
            panic!("Wrong data type");
        }
    }

    #[test]
    fn test_log_response() {
        let logger = AuditLogger::new(10);
        let query = CertLocalKCutQuery::new(vec![1], 5, 2);
        let result = LocalKCutResultSummary::Found {
            cut_value: 3,
            witness_hash: 999,
        };
        let response = LocalKCutResponse::new(query, result, 100, None);

        logger.log_response(&response);

        let entries = logger.by_type(AuditEntryType::LocalKCutResponse);
        assert_eq!(entries.len(), 1);

        if let AuditData::Response { found, value } = &entries[0].data {
            assert!(found);
            assert_eq!(*value, Some(3));
        } else {
            panic!("Wrong data type");
        }
    }

    #[test]
    fn test_log_mincut_changed() {
        let logger = AuditLogger::new(10);
        let trigger = UpdateTrigger::new(UpdateType::Insert, 123, (1, 2), 1000);

        logger.log_mincut_changed(10, 8, trigger);

        let entries = logger.by_type(AuditEntryType::MinCutChanged);
        assert_eq!(entries.len(), 1);

        if let AuditData::MinCut {
            old_value,
            new_value,
            ..
        } = &entries[0].data
        {
            assert_eq!(*old_value, 10);
            assert_eq!(*new_value, 8);
        } else {
            panic!("Wrong data type");
        }
    }

    #[test]
    fn test_log_certificate_created() {
        let logger = AuditLogger::new(10);
        logger.log_certificate_created(5, 10, Some(8));

        let entries = logger.by_type(AuditEntryType::CertificateCreated);
        assert_eq!(entries.len(), 1);

        if let AuditData::Certificate {
            num_witnesses,
            num_responses,
            certified_value,
        } = &entries[0].data
        {
            assert_eq!(*num_witnesses, 5);
            assert_eq!(*num_responses, 10);
            assert_eq!(*certified_value, Some(8));
        } else {
            panic!("Wrong data type");
        }
    }

    #[test]
    fn test_max_entries() {
        let logger = AuditLogger::new(3);

        for i in 0..5 {
            logger.log(
                AuditEntryType::WitnessCreated,
                AuditData::Witness {
                    hash: i,
                    boundary: i,
                    seed: i,
                },
            );
        }

        // Should only keep last 3 entries
        assert_eq!(logger.len(), 3);

        let entries = logger.export();
        // First entry should have id 2 (0 and 1 were evicted)
        assert!(entries[0].id >= 2);
    }

    #[test]
    fn test_recent() {
        let logger = AuditLogger::new(10);

        for i in 0..5 {
            logger.log(
                AuditEntryType::WitnessCreated,
                AuditData::Witness {
                    hash: i,
                    boundary: i,
                    seed: i,
                },
            );
        }

        let recent = logger.recent(3);
        assert_eq!(recent.len(), 3);

        // Should be the last 3 entries
        assert_eq!(recent[0].id, 2);
        assert_eq!(recent[1].id, 3);
        assert_eq!(recent[2].id, 4);
    }

    #[test]
    fn test_by_type() {
        let logger = AuditLogger::new(10);

        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 1,
                boundary: 1,
                seed: 1,
            },
        );
        logger.log(
            AuditEntryType::WitnessUpdated,
            AuditData::Witness {
                hash: 2,
                boundary: 2,
                seed: 2,
            },
        );
        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 3,
                boundary: 3,
                seed: 3,
            },
        );

        let created = logger.by_type(AuditEntryType::WitnessCreated);
        let updated = logger.by_type(AuditEntryType::WitnessUpdated);

        assert_eq!(created.len(), 2);
        assert_eq!(updated.len(), 1);
    }

    #[test]
    fn test_clear() {
        let logger = AuditLogger::new(10);

        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 1,
                boundary: 1,
                seed: 1,
            },
        );

        assert_eq!(logger.len(), 1);

        logger.clear();

        assert_eq!(logger.len(), 0);
        assert!(logger.is_empty());
    }

    #[test]
    fn test_json_export() {
        let logger = AuditLogger::new(10);

        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 1,
                boundary: 5,
                seed: 2,
            },
        );

        let json = logger.to_json().unwrap();
        assert!(json.contains("WitnessCreated"));
        // JSON might have spaces, check for "boundary" and "5" separately
        assert!(json.contains("boundary"));
        assert!(json.contains("5"));
    }

    #[test]
    fn test_clone() {
        let logger = AuditLogger::new(10);
        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 1,
                boundary: 1,
                seed: 1,
            },
        );

        let cloned = logger.clone();
        assert_eq!(cloned.len(), 1);
        assert_eq!(cloned.capacity(), 10);

        // Both should share the same data
        logger.log(
            AuditEntryType::WitnessUpdated,
            AuditData::Witness {
                hash: 2,
                boundary: 2,
                seed: 2,
            },
        );

        assert_eq!(cloned.len(), 2);
    }

    #[test]
    fn test_entry_timestamps() {
        let logger = AuditLogger::new(10);

        logger.log(
            AuditEntryType::WitnessCreated,
            AuditData::Witness {
                hash: 1,
                boundary: 1,
                seed: 1,
            },
        );

        let entries = logger.export();
        assert!(entries[0].timestamp > 0);
    }
}
