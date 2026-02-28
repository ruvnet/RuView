//! Raft log implementation
//!
//! Manages the replicated log with support for:
//! - Appending entries
//! - Truncation and conflict resolution
//! - Snapshots and compaction
//! - Persistence

use crate::{LogIndex, RaftError, RaftResult, Term};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// A single entry in the Raft log
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogEntry {
    /// Term when entry was received by leader
    pub term: Term,

    /// Index position in the log
    pub index: LogIndex,

    /// State machine command
    pub command: Vec<u8>,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(term: Term, index: LogIndex, command: Vec<u8>) -> Self {
        Self {
            term,
            index,
            command,
        }
    }
}

/// Snapshot metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    /// Index of last entry in snapshot
    pub last_included_index: LogIndex,

    /// Term of last entry in snapshot
    pub last_included_term: Term,

    /// Snapshot data
    pub data: Vec<u8>,

    /// Configuration at the time of snapshot
    pub configuration: Vec<String>,
}

/// The Raft replicated log
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaftLog {
    /// Log entries (index starts at 1)
    entries: VecDeque<LogEntry>,

    /// Current snapshot (if any)
    snapshot: Option<Snapshot>,

    /// Base index from snapshot (0 if no snapshot)
    base_index: LogIndex,

    /// Base term from snapshot (0 if no snapshot)
    base_term: Term,
}

impl RaftLog {
    /// Create a new empty log
    pub fn new() -> Self {
        Self {
            entries: VecDeque::new(),
            snapshot: None,
            base_index: 0,
            base_term: 0,
        }
    }

    /// Get the index of the last log entry
    pub fn last_index(&self) -> LogIndex {
        if let Some(entry) = self.entries.back() {
            entry.index
        } else {
            self.base_index
        }
    }

    /// Get the term of the last log entry
    pub fn last_term(&self) -> Term {
        if let Some(entry) = self.entries.back() {
            entry.term
        } else {
            self.base_term
        }
    }

    /// Get the term at a specific index
    pub fn term_at(&self, index: LogIndex) -> Option<Term> {
        if index == self.base_index {
            return Some(self.base_term);
        }

        if index < self.base_index {
            return None;
        }

        let offset = (index - self.base_index - 1) as usize;
        self.entries.get(offset).map(|entry| entry.term)
    }

    /// Get a log entry at a specific index
    pub fn get(&self, index: LogIndex) -> Option<&LogEntry> {
        if index <= self.base_index {
            return None;
        }

        let offset = (index - self.base_index - 1) as usize;
        self.entries.get(offset)
    }

    /// Get entries starting from an index
    pub fn entries_from(&self, start_index: LogIndex) -> Vec<LogEntry> {
        if start_index <= self.base_index {
            return self.entries.iter().cloned().collect();
        }

        let offset = (start_index - self.base_index - 1) as usize;
        self.entries.iter().skip(offset).cloned().collect()
    }

    /// Append a new entry to the log
    pub fn append(&mut self, term: Term, command: Vec<u8>) -> LogIndex {
        let index = self.last_index() + 1;
        let entry = LogEntry::new(term, index, command);
        self.entries.push_back(entry);
        index
    }

    /// Append multiple entries (for replication)
    pub fn append_entries(&mut self, entries: Vec<LogEntry>) -> RaftResult<()> {
        for entry in entries {
            // Verify index is sequential
            let expected_index = self.last_index() + 1;
            if entry.index != expected_index {
                return Err(RaftError::LogInconsistency);
            }
            self.entries.push_back(entry);
        }
        Ok(())
    }

    /// Truncate log from a given index (delete entries >= index)
    pub fn truncate_from(&mut self, index: LogIndex) -> RaftResult<()> {
        if index <= self.base_index {
            return Err(RaftError::InvalidLogIndex(index));
        }

        let offset = (index - self.base_index - 1) as usize;
        self.entries.truncate(offset);
        Ok(())
    }

    /// Check if log contains an entry at index with the given term
    pub fn matches(&self, index: LogIndex, term: Term) -> bool {
        if index == 0 {
            return true;
        }

        if index == self.base_index {
            return term == self.base_term;
        }

        match self.term_at(index) {
            Some(entry_term) => entry_term == term,
            None => false,
        }
    }

    /// Install a snapshot and compact the log
    pub fn install_snapshot(&mut self, snapshot: Snapshot) -> RaftResult<()> {
        let last_index = snapshot.last_included_index;
        let last_term = snapshot.last_included_term;

        // Remove all entries up to and including the snapshot's last index
        while let Some(entry) = self.entries.front() {
            if entry.index <= last_index {
                self.entries.pop_front();
            } else {
                break;
            }
        }

        self.base_index = last_index;
        self.base_term = last_term;
        self.snapshot = Some(snapshot);

        Ok(())
    }

    /// Create a snapshot up to the given index
    pub fn create_snapshot(
        &mut self,
        up_to_index: LogIndex,
        data: Vec<u8>,
        configuration: Vec<String>,
    ) -> RaftResult<Snapshot> {
        if up_to_index <= self.base_index {
            return Err(RaftError::InvalidLogIndex(up_to_index));
        }

        let term = self
            .term_at(up_to_index)
            .ok_or(RaftError::InvalidLogIndex(up_to_index))?;

        let snapshot = Snapshot {
            last_included_index: up_to_index,
            last_included_term: term,
            data,
            configuration,
        };

        // Compact the log by removing entries before the snapshot
        self.install_snapshot(snapshot.clone())?;

        Ok(snapshot)
    }

    /// Get the current snapshot
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// Get the number of entries in memory
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.base_index == 0
    }

    /// Get the base index from snapshot
    pub fn base_index(&self) -> LogIndex {
        self.base_index
    }

    /// Get the base term from snapshot
    pub fn base_term(&self) -> Term {
        self.base_term
    }
}

impl Default for RaftLog {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_append() {
        let mut log = RaftLog::new();
        assert_eq!(log.last_index(), 0);

        let idx1 = log.append(1, b"cmd1".to_vec());
        assert_eq!(idx1, 1);
        assert_eq!(log.last_index(), 1);
        assert_eq!(log.last_term(), 1);

        let idx2 = log.append(1, b"cmd2".to_vec());
        assert_eq!(idx2, 2);
        assert_eq!(log.last_index(), 2);
    }

    #[test]
    fn test_log_get() {
        let mut log = RaftLog::new();
        log.append(1, b"cmd1".to_vec());
        log.append(1, b"cmd2".to_vec());
        log.append(2, b"cmd3".to_vec());

        let entry = log.get(2).unwrap();
        assert_eq!(entry.term, 1);
        assert_eq!(entry.command, b"cmd2");

        assert!(log.get(0).is_none());
        assert!(log.get(10).is_none());
    }

    #[test]
    fn test_log_truncate() {
        let mut log = RaftLog::new();
        log.append(1, b"cmd1".to_vec());
        log.append(1, b"cmd2".to_vec());
        log.append(2, b"cmd3".to_vec());

        log.truncate_from(2).unwrap();
        assert_eq!(log.last_index(), 1);
        assert!(log.get(2).is_none());
    }

    #[test]
    fn test_log_matches() {
        let mut log = RaftLog::new();
        log.append(1, b"cmd1".to_vec());
        log.append(1, b"cmd2".to_vec());
        log.append(2, b"cmd3".to_vec());

        assert!(log.matches(1, 1));
        assert!(log.matches(2, 1));
        assert!(log.matches(3, 2));
        assert!(!log.matches(3, 1));
        assert!(!log.matches(10, 1));
    }

    #[test]
    fn test_snapshot_creation() {
        let mut log = RaftLog::new();
        log.append(1, b"cmd1".to_vec());
        log.append(1, b"cmd2".to_vec());
        log.append(2, b"cmd3".to_vec());

        let snapshot = log
            .create_snapshot(2, b"state".to_vec(), vec!["node1".to_string()])
            .unwrap();

        assert_eq!(snapshot.last_included_index, 2);
        assert_eq!(snapshot.last_included_term, 1);
        assert_eq!(log.base_index(), 2);
        assert_eq!(log.len(), 1); // Only entry 3 remains
    }

    #[test]
    fn test_entries_from() {
        let mut log = RaftLog::new();
        log.append(1, b"cmd1".to_vec());
        log.append(1, b"cmd2".to_vec());
        log.append(2, b"cmd3".to_vec());

        let entries = log.entries_from(2);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 2);
        assert_eq!(entries[1].index, 3);
    }
}
