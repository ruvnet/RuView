//! Witness bundle types for ADR-035 capability reports.
//!
//! A witness bundle is a signed, self-contained evidence record of a task
//! execution. It captures spec, plan, tool trace, diff, test log, cost,
//! latency, and governance mode — everything needed for deterministic replay
//! and audit.
//!
//! Wire format: 64-byte header + TLV sections + optional HMAC-SHA256 signature.

/// Magic bytes for witness bundle: "RVWW" (RuVector Witness).
pub const WITNESS_MAGIC: u32 = 0x5257_5657;

/// Size of the witness header in bytes.
pub const WITNESS_HEADER_SIZE: usize = 64;

// --- Flags ---

/// Witness bundle is signed (HMAC-SHA256).
pub const WIT_SIGNED: u16 = 0x0001;
/// Witness bundle has a spec section.
pub const WIT_HAS_SPEC: u16 = 0x0002;
/// Witness bundle has a plan section.
pub const WIT_HAS_PLAN: u16 = 0x0004;
/// Witness bundle has a tool trace section.
pub const WIT_HAS_TRACE: u16 = 0x0008;
/// Witness bundle has a diff section.
pub const WIT_HAS_DIFF: u16 = 0x0010;
/// Witness bundle has a test log section.
pub const WIT_HAS_TEST_LOG: u16 = 0x0020;
/// Witness bundle has a postmortem section.
pub const WIT_HAS_POSTMORTEM: u16 = 0x0040;

// --- TLV tags ---

/// Tag: task spec / prompt text.
pub const WIT_TAG_SPEC: u16 = 0x0001;
/// Tag: plan graph (text or structured).
pub const WIT_TAG_PLAN: u16 = 0x0002;
/// Tag: tool call trace (array of ToolCallEntry).
pub const WIT_TAG_TRACE: u16 = 0x0003;
/// Tag: code diff (unified diff text).
pub const WIT_TAG_DIFF: u16 = 0x0004;
/// Tag: test output log.
pub const WIT_TAG_TEST_LOG: u16 = 0x0005;
/// Tag: postmortem / failure analysis.
pub const WIT_TAG_POSTMORTEM: u16 = 0x0006;

// --- Enums ---

/// Task execution outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum TaskOutcome {
    /// Task completed with passing tests and merged diff.
    Solved = 0,
    /// Task attempted but tests fail or diff rejected.
    Failed = 1,
    /// Task skipped (precondition not met).
    Skipped = 2,
    /// Task errored (infrastructure or tool failure).
    Errored = 3,
}

impl TryFrom<u8> for TaskOutcome {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Solved),
            1 => Ok(Self::Failed),
            2 => Ok(Self::Skipped),
            3 => Ok(Self::Errored),
            other => Err(other),
        }
    }
}

/// Governance mode under which the task was executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum GovernanceMode {
    /// Read-only plus suggestions. No writes.
    Restricted = 0,
    /// Writes allowed with human confirmation gates.
    Approved = 1,
    /// Bounded authority with automatic rollback on violation.
    Autonomous = 2,
}

impl TryFrom<u8> for GovernanceMode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Restricted),
            1 => Ok(Self::Approved),
            2 => Ok(Self::Autonomous),
            other => Err(other),
        }
    }
}

/// Policy check result for a single tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum PolicyCheck {
    /// Tool call allowed by policy.
    Allowed = 0,
    /// Tool call denied by policy.
    Denied = 1,
    /// Tool call required human confirmation.
    Confirmed = 2,
}

impl TryFrom<u8> for PolicyCheck {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Allowed),
            1 => Ok(Self::Denied),
            2 => Ok(Self::Confirmed),
            other => Err(other),
        }
    }
}

/// Wire-format witness header (exactly 64 bytes, `repr(C)`).
///
/// ```text
/// Offset  Type        Field
/// 0x00    u32         magic (0x52575657 "RVWW")
/// 0x04    u16         version
/// 0x06    u16         flags
/// 0x08    [u8; 16]    task_id (UUID)
/// 0x18    [u8; 8]     policy_hash (SHA-256 truncated)
/// 0x20    u64         created_ns (UNIX epoch nanoseconds)
/// 0x28    u8          outcome (TaskOutcome)
/// 0x29    u8          governance_mode (GovernanceMode)
/// 0x2A    u16         tool_call_count
/// 0x2C    u32         total_cost_microdollars
/// 0x30    u32         total_latency_ms
/// 0x34    u32         total_tokens
/// 0x38    u16         retry_count
/// 0x3A    u16         section_count
/// 0x3C    u32         total_bundle_size
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct WitnessHeader {
    /// Magic bytes: WITNESS_MAGIC.
    pub magic: u32,
    /// Format version (currently 1).
    pub version: u16,
    /// Bitfield flags (WIT_SIGNED, WIT_HAS_SPEC, etc.).
    pub flags: u16,
    /// Unique task identifier (UUID).
    pub task_id: [u8; 16],
    /// SHA-256 of the policy file, truncated to 8 bytes.
    pub policy_hash: [u8; 8],
    /// Creation timestamp (nanoseconds since UNIX epoch).
    pub created_ns: u64,
    /// Task outcome discriminant.
    pub outcome: u8,
    /// Governance mode discriminant.
    pub governance_mode: u8,
    /// Number of tool calls recorded.
    pub tool_call_count: u16,
    /// Total cost in microdollars (1/1,000,000 USD).
    pub total_cost_microdollars: u32,
    /// Total wall-clock latency in milliseconds.
    pub total_latency_ms: u32,
    /// Total tokens consumed (prompt + completion).
    pub total_tokens: u32,
    /// Number of retries across all tool calls.
    pub retry_count: u16,
    /// Number of TLV sections in the payload.
    pub section_count: u16,
    /// Total size of the entire witness bundle (header + payload + sig).
    pub total_bundle_size: u32,
}

// Compile-time size assertion.
const _: () = assert!(core::mem::size_of::<WitnessHeader>() == 64);

impl WitnessHeader {
    /// Check magic bytes.
    pub const fn is_valid_magic(&self) -> bool {
        self.magic == WITNESS_MAGIC
    }

    /// Check if the bundle is signed.
    pub const fn is_signed(&self) -> bool {
        self.flags & WIT_SIGNED != 0
    }

    /// Serialize header to a 64-byte array.
    pub fn to_bytes(&self) -> [u8; WITNESS_HEADER_SIZE] {
        let mut buf = [0u8; WITNESS_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..6].copy_from_slice(&self.version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..24].copy_from_slice(&self.task_id);
        buf[24..32].copy_from_slice(&self.policy_hash);
        buf[32..40].copy_from_slice(&self.created_ns.to_le_bytes());
        buf[40] = self.outcome;
        buf[41] = self.governance_mode;
        buf[42..44].copy_from_slice(&self.tool_call_count.to_le_bytes());
        buf[44..48].copy_from_slice(&self.total_cost_microdollars.to_le_bytes());
        buf[48..52].copy_from_slice(&self.total_latency_ms.to_le_bytes());
        buf[52..56].copy_from_slice(&self.total_tokens.to_le_bytes());
        buf[56..58].copy_from_slice(&self.retry_count.to_le_bytes());
        buf[58..60].copy_from_slice(&self.section_count.to_le_bytes());
        buf[60..64].copy_from_slice(&self.total_bundle_size.to_le_bytes());
        buf
    }

    /// Deserialize header from a byte slice (>= 64 bytes).
    pub fn from_bytes(data: &[u8]) -> Result<Self, crate::RvfError> {
        if data.len() < WITNESS_HEADER_SIZE {
            return Err(crate::RvfError::SizeMismatch {
                expected: WITNESS_HEADER_SIZE,
                got: data.len(),
            });
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != WITNESS_MAGIC {
            return Err(crate::RvfError::BadMagic {
                expected: WITNESS_MAGIC,
                got: magic,
            });
        }
        let mut task_id = [0u8; 16];
        task_id.copy_from_slice(&data[8..24]);
        let mut policy_hash = [0u8; 8];
        policy_hash.copy_from_slice(&data[24..32]);

        Ok(Self {
            magic,
            version: u16::from_le_bytes([data[4], data[5]]),
            flags: u16::from_le_bytes([data[6], data[7]]),
            task_id,
            policy_hash,
            created_ns: u64::from_le_bytes([
                data[32], data[33], data[34], data[35], data[36], data[37], data[38], data[39],
            ]),
            outcome: data[40],
            governance_mode: data[41],
            tool_call_count: u16::from_le_bytes([data[42], data[43]]),
            total_cost_microdollars: u32::from_le_bytes([data[44], data[45], data[46], data[47]]),
            total_latency_ms: u32::from_le_bytes([data[48], data[49], data[50], data[51]]),
            total_tokens: u32::from_le_bytes([data[52], data[53], data[54], data[55]]),
            retry_count: u16::from_le_bytes([data[56], data[57]]),
            section_count: u16::from_le_bytes([data[58], data[59]]),
            total_bundle_size: u32::from_le_bytes([data[60], data[61], data[62], data[63]]),
        })
    }
}

/// A single tool call record within a witness trace.
///
/// Requires the `alloc` feature because the variable-length `action` field
/// uses `Vec<u8>`.
///
/// Variable-length: 32 bytes fixed header + action_len bytes.
///
/// ```text
/// Offset  Type      Field
/// 0x00    u16       action_len
/// 0x02    u8        policy_check (PolicyCheck)
/// 0x03    u8        _pad
/// 0x04    [u8; 8]   args_hash (SHA-256 truncated)
/// 0x0C    [u8; 8]   result_hash (SHA-256 truncated)
/// 0x14    u32       latency_ms
/// 0x18    u32       cost_microdollars
/// 0x1C    u32       tokens
/// 0x20    [u8; action_len]  action (UTF-8 tool name)
/// ```
#[cfg(any(feature = "alloc", test))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallEntry {
    /// Tool name / action (e.g. "Bash", "Edit", "Read").
    pub action: alloc::vec::Vec<u8>,
    /// SHA-256 of args, truncated to 8 bytes.
    pub args_hash: [u8; 8],
    /// SHA-256 of result, truncated to 8 bytes.
    pub result_hash: [u8; 8],
    /// Wall-clock latency in milliseconds.
    pub latency_ms: u32,
    /// Cost in microdollars.
    pub cost_microdollars: u32,
    /// Tokens consumed.
    pub tokens: u32,
    /// Policy check result.
    pub policy_check: PolicyCheck,
}

/// Fixed header size for a ToolCallEntry (before the action string).
#[cfg(any(feature = "alloc", test))]
pub const TOOL_CALL_FIXED_SIZE: usize = 32;

#[cfg(any(feature = "alloc", test))]
impl ToolCallEntry {
    /// Total serialized size.
    pub fn wire_size(&self) -> usize {
        TOOL_CALL_FIXED_SIZE + self.action.len()
    }

    /// Serialize to bytes.
    pub fn to_bytes(&self) -> alloc::vec::Vec<u8> {
        let mut buf = alloc::vec::Vec::with_capacity(self.wire_size());
        buf.extend_from_slice(&(self.action.len() as u16).to_le_bytes());
        buf.push(self.policy_check as u8);
        buf.push(0); // pad
        buf.extend_from_slice(&self.args_hash);
        buf.extend_from_slice(&self.result_hash);
        buf.extend_from_slice(&self.latency_ms.to_le_bytes());
        buf.extend_from_slice(&self.cost_microdollars.to_le_bytes());
        buf.extend_from_slice(&self.tokens.to_le_bytes());
        buf.extend_from_slice(&self.action);
        buf
    }

    /// Deserialize from bytes. Returns (entry, bytes_consumed).
    pub fn from_bytes(data: &[u8]) -> Option<(Self, usize)> {
        if data.len() < TOOL_CALL_FIXED_SIZE {
            return None;
        }
        let action_len = u16::from_le_bytes([data[0], data[1]]) as usize;
        let total = TOOL_CALL_FIXED_SIZE + action_len;
        if data.len() < total {
            return None;
        }
        let policy_check = PolicyCheck::try_from(data[2]).ok()?;
        let mut args_hash = [0u8; 8];
        args_hash.copy_from_slice(&data[4..12]);
        let mut result_hash = [0u8; 8];
        result_hash.copy_from_slice(&data[12..20]);
        let latency_ms = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let cost_microdollars = u32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        let tokens = u32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        let action = data[TOOL_CALL_FIXED_SIZE..total].to_vec();

        Some((
            Self {
                action,
                args_hash,
                result_hash,
                latency_ms,
                cost_microdollars,
                tokens,
                policy_check,
            },
            total,
        ))
    }
}

/// Capability scorecard — aggregate metrics across witness bundles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scorecard {
    /// Total tasks attempted.
    pub total_tasks: u32,
    /// Tasks solved with passing tests.
    pub solved: u32,
    /// Tasks that failed (tests don't pass).
    pub failed: u32,
    /// Tasks skipped.
    pub skipped: u32,
    /// Tasks that errored (infra failure).
    pub errors: u32,
    /// Policy violations detected.
    pub policy_violations: u32,
    /// Rollbacks performed.
    pub rollback_count: u32,
    /// Total cost in microdollars.
    pub total_cost_microdollars: u64,
    /// Median latency in milliseconds.
    pub median_latency_ms: u32,
    /// 95th percentile latency in milliseconds.
    pub p95_latency_ms: u32,
    /// Total tokens consumed.
    pub total_tokens: u64,
    /// Total retries across all tasks.
    pub total_retries: u32,
    /// Fraction of solved tasks with complete witness bundles.
    pub evidence_coverage: f32,
    /// Cost per solved task in microdollars.
    pub cost_per_solve_microdollars: u32,
    /// Solve rate (solved / total_tasks).
    pub solve_rate: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate alloc;

    #[test]
    fn witness_header_size() {
        assert_eq!(core::mem::size_of::<WitnessHeader>(), 64);
    }

    #[test]
    fn witness_header_round_trip() {
        let hdr = WitnessHeader {
            magic: WITNESS_MAGIC,
            version: 1,
            flags: WIT_SIGNED | WIT_HAS_SPEC | WIT_HAS_DIFF,
            task_id: [0x42; 16],
            policy_hash: [0xAA; 8],
            created_ns: 1_700_000_000_000_000_000,
            outcome: TaskOutcome::Solved as u8,
            governance_mode: GovernanceMode::Approved as u8,
            tool_call_count: 12,
            total_cost_microdollars: 15_000,
            total_latency_ms: 4_500,
            total_tokens: 8_000,
            retry_count: 2,
            section_count: 3,
            total_bundle_size: 2048,
        };
        let bytes = hdr.to_bytes();
        assert_eq!(bytes.len(), WITNESS_HEADER_SIZE);
        let decoded = WitnessHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, hdr);
    }

    #[test]
    fn witness_header_bad_magic() {
        let mut bytes = [0u8; 64];
        bytes[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        assert!(WitnessHeader::from_bytes(&bytes).is_err());
    }

    #[test]
    fn witness_header_too_short() {
        assert!(WitnessHeader::from_bytes(&[0u8; 32]).is_err());
    }

    #[test]
    fn task_outcome_round_trip() {
        for raw in 0..=3u8 {
            let o = TaskOutcome::try_from(raw).unwrap();
            assert_eq!(o as u8, raw);
        }
        assert!(TaskOutcome::try_from(4).is_err());
    }

    #[test]
    fn governance_mode_round_trip() {
        for raw in 0..=2u8 {
            let g = GovernanceMode::try_from(raw).unwrap();
            assert_eq!(g as u8, raw);
        }
        assert!(GovernanceMode::try_from(3).is_err());
    }

    #[test]
    fn policy_check_round_trip() {
        for raw in 0..=2u8 {
            let p = PolicyCheck::try_from(raw).unwrap();
            assert_eq!(p as u8, raw);
        }
        assert!(PolicyCheck::try_from(3).is_err());
    }

    #[test]
    fn tool_call_entry_round_trip() {
        let entry = ToolCallEntry {
            action: b"Bash".to_vec(),
            args_hash: [0x11; 8],
            result_hash: [0x22; 8],
            latency_ms: 150,
            cost_microdollars: 500,
            tokens: 200,
            policy_check: PolicyCheck::Allowed,
        };
        let bytes = entry.to_bytes();
        assert_eq!(bytes.len(), TOOL_CALL_FIXED_SIZE + 4);
        let (decoded, consumed) = ToolCallEntry::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, entry);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn tool_call_entry_too_short() {
        assert!(ToolCallEntry::from_bytes(&[0u8; 10]).is_none());
    }

    #[test]
    fn witness_flags() {
        let flags = WIT_SIGNED | WIT_HAS_SPEC | WIT_HAS_DIFF | WIT_HAS_TEST_LOG;
        assert_ne!(flags & WIT_SIGNED, 0);
        assert_ne!(flags & WIT_HAS_SPEC, 0);
        assert_eq!(flags & WIT_HAS_PLAN, 0);
        assert_ne!(flags & WIT_HAS_DIFF, 0);
        assert_ne!(flags & WIT_HAS_TEST_LOG, 0);
        assert_eq!(flags & WIT_HAS_POSTMORTEM, 0);
    }

    #[test]
    fn scorecard_default_is_zero() {
        let s = Scorecard::default();
        assert_eq!(s.total_tasks, 0);
        assert_eq!(s.solved, 0);
        assert_eq!(s.solve_rate, 0.0);
        assert_eq!(s.evidence_coverage, 0.0);
    }
}
