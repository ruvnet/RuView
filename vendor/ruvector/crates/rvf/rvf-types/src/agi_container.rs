//! AGI Cognitive Container types (ADR-036).
//!
//! An AGI container is a single RVF file that packages the complete intelligence
//! runtime: micro Linux kernel, Claude Code orchestrator config, Claude Flow
//! swarm manager, RuVector world model, evaluation harness, witness chains,
//! and tool adapters.
//!
//! Wire format: 64-byte `AgiContainerHeader` + TLV manifest sections.
//! The header is stored as a META segment (SegmentType::Meta) in the RVF file,
//! alongside the KERNEL_SEG, WASM_SEG, VEC_SEG, INDEX_SEG, WITNESS_SEG, and
//! CRYPTO_SEG that hold the actual payload data.

/// Magic bytes for AGI container manifest: "RVAG" (RuVector AGI).
pub const AGI_MAGIC: u32 = 0x5256_4147;

/// Size of the AGI container header in bytes.
pub const AGI_HEADER_SIZE: usize = 64;

/// Maximum container size: 16 GiB. Prevents unbounded resource consumption.
pub const AGI_MAX_CONTAINER_SIZE: u64 = 16 * 1024 * 1024 * 1024;

// --- Flags ---

/// Container includes a KERNEL_SEG with micro Linux kernel.
pub const AGI_HAS_KERNEL: u16 = 1 << 0;
/// Container includes WASM_SEG modules.
pub const AGI_HAS_WASM: u16 = 1 << 1;
/// Container includes Claude Code + Claude Flow orchestrator config.
pub const AGI_HAS_ORCHESTRATOR: u16 = 1 << 2;
/// Container includes VEC_SEG + INDEX_SEG world model data.
pub const AGI_HAS_WORLD_MODEL: u16 = 1 << 3;
/// Container includes evaluation harness (task suite + graders).
pub const AGI_HAS_EVAL: u16 = 1 << 4;
/// Container includes promoted skill library.
pub const AGI_HAS_SKILLS: u16 = 1 << 5;
/// Container includes ADR-035 witness chain.
pub const AGI_HAS_WITNESS: u16 = 1 << 6;
/// Container is cryptographically signed (HMAC-SHA256 or Ed25519).
pub const AGI_SIGNED: u16 = 1 << 7;
/// All tool outputs stored — container supports replay mode.
pub const AGI_REPLAY_CAPABLE: u16 = 1 << 8;
/// Container can run without network (offline-first).
pub const AGI_OFFLINE_CAPABLE: u16 = 1 << 9;
/// Container includes MCP tool adapter registry.
pub const AGI_HAS_TOOLS: u16 = 1 << 10;
/// Container includes coherence gate configuration.
pub const AGI_HAS_COHERENCE_GATES: u16 = 1 << 11;
/// Container includes cross-domain transfer learning data.
pub const AGI_HAS_DOMAIN_EXPANSION: u16 = 1 << 12;

// --- TLV tags for the manifest payload ---

/// Container UUID.
pub const AGI_TAG_CONTAINER_ID: u16 = 0x0100;
/// Build UUID.
pub const AGI_TAG_BUILD_ID: u16 = 0x0101;
/// Pinned model identifier (UTF-8 string, e.g. "claude-opus-4-6").
pub const AGI_TAG_MODEL_ID: u16 = 0x0102;
/// Serialized governance policy (binary, per ADR-035).
pub const AGI_TAG_POLICY: u16 = 0x0103;
/// Claude Code + Claude Flow orchestrator config (JSON or TOML).
pub const AGI_TAG_ORCHESTRATOR: u16 = 0x0104;
/// MCP tool adapter registry (JSON array of tool schemas).
pub const AGI_TAG_TOOL_REGISTRY: u16 = 0x0105;
/// Agent role prompts (one per agent type).
pub const AGI_TAG_AGENT_PROMPTS: u16 = 0x0106;
/// Evaluation task suite (JSON array of task specs).
pub const AGI_TAG_EVAL_TASKS: u16 = 0x0107;
/// Grading rules (JSON or binary grader config).
pub const AGI_TAG_EVAL_GRADERS: u16 = 0x0108;
/// Promoted skill library (serialized skill nodes).
pub const AGI_TAG_SKILL_LIBRARY: u16 = 0x0109;
/// Replay automation script.
pub const AGI_TAG_REPLAY_SCRIPT: u16 = 0x010A;
/// Kernel boot parameters (command line, initrd config).
pub const AGI_TAG_KERNEL_CONFIG: u16 = 0x010B;
/// Network configuration (ports, endpoints, TLS).
pub const AGI_TAG_NETWORK_CONFIG: u16 = 0x010C;
/// Coherence gate thresholds and rules.
pub const AGI_TAG_COHERENCE_CONFIG: u16 = 0x010D;
/// Claude.md project instructions.
pub const AGI_TAG_PROJECT_INSTRUCTIONS: u16 = 0x010E;
/// Dependency snapshot hashes (pinned repos, packages).
pub const AGI_TAG_DEPENDENCY_SNAPSHOT: u16 = 0x010F;
/// Authority level and resource budget configuration.
pub const AGI_TAG_AUTHORITY_CONFIG: u16 = 0x0110;
/// Target domain profile identifier.
pub const AGI_TAG_DOMAIN_PROFILE: u16 = 0x0111;
/// Cross-domain transfer prior (posterior summaries).
pub const AGI_TAG_TRANSFER_PRIOR: u16 = 0x0112;
/// Policy kernel configuration and performance history.
pub const AGI_TAG_POLICY_KERNEL: u16 = 0x0113;
/// Cost curve convergence and acceleration data.
pub const AGI_TAG_COST_CURVE: u16 = 0x0114;
/// Counterexample archive (failed solutions for future decisions).
pub const AGI_TAG_COUNTEREXAMPLES: u16 = 0x0115;

// --- Execution mode ---

/// Container execution mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum ExecutionMode {
    /// Replay: no external tool calls, use stored receipts.
    /// All graders must match exactly. Witness chain must match.
    Replay = 0,
    /// Verify: live tool calls, outputs stored and hashed.
    /// Outputs must pass same tests. Costs within expected bounds.
    Verify = 1,
    /// Live: full autonomous operation with governance controls.
    Live = 2,
}

impl TryFrom<u8> for ExecutionMode {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Replay),
            1 => Ok(Self::Verify),
            2 => Ok(Self::Live),
            other => Err(other),
        }
    }
}

// --- Authority level ---

/// Authority level controlling what actions a container execution can perform.
///
/// Each action in the world model must reference a policy decision node
/// that grants at least the required authority level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[repr(u8)]
pub enum AuthorityLevel {
    /// Read-only: query vectors, graphs, memories. No mutations.
    ReadOnly = 0,
    /// Write to internal memory: commit world model deltas behind
    /// coherence gates. No external tool calls.
    WriteMemory = 1,
    /// Execute tools: run sandboxed tools (file read/write, tests,
    /// code generation). External side effects gated by policy.
    ExecuteTools = 2,
    /// Write external: push code, create PRs, send messages, modify
    /// infrastructure. Requires explicit policy grant per action class.
    WriteExternal = 3,
}

impl TryFrom<u8> for AuthorityLevel {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::ReadOnly),
            1 => Ok(Self::WriteMemory),
            2 => Ok(Self::ExecuteTools),
            3 => Ok(Self::WriteExternal),
            other => Err(other),
        }
    }
}

impl AuthorityLevel {
    /// Default authority for the given execution mode.
    pub const fn default_for_mode(mode: ExecutionMode) -> Self {
        match mode {
            ExecutionMode::Replay => Self::ReadOnly,
            ExecutionMode::Verify => Self::ExecuteTools,
            ExecutionMode::Live => Self::WriteMemory,
        }
    }

    /// Check if this authority level permits a given required level.
    pub const fn permits(&self, required: AuthorityLevel) -> bool {
        (*self as u8) >= (required as u8)
    }
}

// --- Resource budgets ---

/// Per-task resource budget with hard caps.
///
/// Budget exhaustion triggers graceful degradation: the task enters `Skipped`
/// outcome with a `BudgetExhausted` postmortem in the witness bundle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Maximum wall-clock time per task in seconds. Default: 300.
    pub max_time_secs: u32,
    /// Maximum total model tokens per task. Default: 200,000.
    pub max_tokens: u32,
    /// Maximum cost per task in microdollars. Default: 1,000,000 ($1.00).
    pub max_cost_microdollars: u32,
    /// Maximum tool calls per task. Default: 50.
    pub max_tool_calls: u16,
    /// Maximum external write actions per task. Default: 0.
    pub max_external_writes: u16,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl ResourceBudget {
    /// Default resource budget for a single task.
    pub const DEFAULT: Self = Self {
        max_time_secs: 300,
        max_tokens: 200_000,
        max_cost_microdollars: 1_000_000,
        max_tool_calls: 50,
        max_external_writes: 0,
    };

    /// Extended budget (4x default) for high-value tasks.
    pub const EXTENDED: Self = Self {
        max_time_secs: 1200,
        max_tokens: 800_000,
        max_cost_microdollars: 4_000_000,
        max_tool_calls: 200,
        max_external_writes: 10,
    };

    /// Maximum configurable budget (hard ceiling, not overridable).
    pub const MAX: Self = Self {
        max_time_secs: 3600,
        max_tokens: 1_000_000,
        max_cost_microdollars: 10_000_000,
        max_tool_calls: 500,
        max_external_writes: 50,
    };

    /// Clamp this budget to not exceed the MAX limits.
    pub const fn clamped(self) -> Self {
        Self {
            max_time_secs: if self.max_time_secs > Self::MAX.max_time_secs {
                Self::MAX.max_time_secs
            } else {
                self.max_time_secs
            },
            max_tokens: if self.max_tokens > Self::MAX.max_tokens {
                Self::MAX.max_tokens
            } else {
                self.max_tokens
            },
            max_cost_microdollars: if self.max_cost_microdollars > Self::MAX.max_cost_microdollars {
                Self::MAX.max_cost_microdollars
            } else {
                self.max_cost_microdollars
            },
            max_tool_calls: if self.max_tool_calls > Self::MAX.max_tool_calls {
                Self::MAX.max_tool_calls
            } else {
                self.max_tool_calls
            },
            max_external_writes: if self.max_external_writes > Self::MAX.max_external_writes {
                Self::MAX.max_external_writes
            } else {
                self.max_external_writes
            },
        }
    }
}

// --- Coherence thresholds ---

/// Configurable coherence thresholds for structural health gating.
///
/// These map to ADR-033's quality framework: the coherence score is analogous
/// to `ResponseQuality` -- it signals whether the system's internal state is
/// trustworthy enough to act on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CoherenceThresholds {
    /// Minimum coherence score (0.0 to 1.0). Below this, block all commits
    /// and enter repair mode. Default: 0.70.
    pub min_coherence_score: f32,
    /// Maximum contradiction rate (contradictions per 100 events).
    /// Above this, freeze skill promotion. Default: 5.0.
    pub max_contradiction_rate: f32,
    /// Maximum rollback ratio (fraction of tasks that required rollback).
    /// Above this, halt Live execution; require human review. Default: 0.20.
    pub max_rollback_ratio: f32,
}

impl Default for CoherenceThresholds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl CoherenceThresholds {
    /// Default coherence thresholds.
    pub const DEFAULT: Self = Self {
        min_coherence_score: 0.70,
        max_contradiction_rate: 5.0,
        max_rollback_ratio: 0.20,
    };

    /// Strict thresholds for production.
    pub const STRICT: Self = Self {
        min_coherence_score: 0.85,
        max_contradiction_rate: 2.0,
        max_rollback_ratio: 0.10,
    };

    /// Validate that threshold values are within valid ranges.
    pub fn validate(&self) -> Result<(), ContainerError> {
        if self.min_coherence_score < 0.0 || self.min_coherence_score > 1.0 {
            return Err(ContainerError::InvalidConfig(
                "min_coherence_score must be in [0.0, 1.0]",
            ));
        }
        if self.max_contradiction_rate < 0.0 {
            return Err(ContainerError::InvalidConfig(
                "max_contradiction_rate must be >= 0.0",
            ));
        }
        if self.max_rollback_ratio < 0.0 || self.max_rollback_ratio > 1.0 {
            return Err(ContainerError::InvalidConfig(
                "max_rollback_ratio must be in [0.0, 1.0]",
            ));
        }
        Ok(())
    }
}

/// Wire-format AGI container header (exactly 64 bytes, `repr(C)`).
///
/// ```text
/// Offset  Type        Field
/// 0x00    u32         magic (0x52564147 "RVAG")
/// 0x04    u16         version
/// 0x06    u16         flags
/// 0x08    [u8; 16]    container_id (UUID)
/// 0x18    [u8; 16]    build_id (UUID)
/// 0x28    u64         created_ns (UNIX epoch nanoseconds)
/// 0x30    [u8; 8]     model_id_hash (SHA-256 truncated)
/// 0x38    [u8; 8]     policy_hash (SHA-256 truncated)
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct AgiContainerHeader {
    /// Magic bytes: AGI_MAGIC.
    pub magic: u32,
    /// Format version (currently 1).
    pub version: u16,
    /// Bitfield flags indicating which segments are present.
    pub flags: u16,
    /// Unique container identifier (UUID).
    pub container_id: [u8; 16],
    /// Build identifier (UUID, changes on each repackaging).
    pub build_id: [u8; 16],
    /// Creation timestamp (nanoseconds since UNIX epoch).
    pub created_ns: u64,
    /// SHA-256 of the pinned model identifier, truncated to 8 bytes.
    pub model_id_hash: [u8; 8],
    /// SHA-256 of the governance policy, truncated to 8 bytes.
    pub policy_hash: [u8; 8],
}

// Compile-time size assertion.
const _: () = assert!(core::mem::size_of::<AgiContainerHeader>() == 64);

impl AgiContainerHeader {
    /// Check magic bytes.
    pub const fn is_valid_magic(&self) -> bool {
        self.magic == AGI_MAGIC
    }

    /// Check if the container is signed.
    pub const fn is_signed(&self) -> bool {
        self.flags & AGI_SIGNED != 0
    }

    /// Check if the container has a micro Linux kernel.
    pub const fn has_kernel(&self) -> bool {
        self.flags & AGI_HAS_KERNEL != 0
    }

    /// Check if the container has an orchestrator config.
    pub const fn has_orchestrator(&self) -> bool {
        self.flags & AGI_HAS_ORCHESTRATOR != 0
    }

    /// Check if the container supports replay mode.
    pub const fn is_replay_capable(&self) -> bool {
        self.flags & AGI_REPLAY_CAPABLE != 0
    }

    /// Check if the container can run offline.
    pub const fn is_offline_capable(&self) -> bool {
        self.flags & AGI_OFFLINE_CAPABLE != 0
    }

    /// Check if the container has a world model (VEC + INDEX segments).
    pub const fn has_world_model(&self) -> bool {
        self.flags & AGI_HAS_WORLD_MODEL != 0
    }

    /// Check if the container has coherence gate configuration.
    pub const fn has_coherence_gates(&self) -> bool {
        self.flags & AGI_HAS_COHERENCE_GATES != 0
    }

    /// Check if the container has domain expansion data.
    pub const fn has_domain_expansion(&self) -> bool {
        self.flags & AGI_HAS_DOMAIN_EXPANSION != 0
    }

    /// Serialize header to a 64-byte array.
    pub fn to_bytes(&self) -> [u8; AGI_HEADER_SIZE] {
        let mut buf = [0u8; AGI_HEADER_SIZE];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4..6].copy_from_slice(&self.version.to_le_bytes());
        buf[6..8].copy_from_slice(&self.flags.to_le_bytes());
        buf[8..24].copy_from_slice(&self.container_id);
        buf[24..40].copy_from_slice(&self.build_id);
        buf[40..48].copy_from_slice(&self.created_ns.to_le_bytes());
        buf[48..56].copy_from_slice(&self.model_id_hash);
        buf[56..64].copy_from_slice(&self.policy_hash);
        buf
    }

    /// Deserialize header from a byte slice (>= 64 bytes).
    pub fn from_bytes(data: &[u8]) -> Result<Self, crate::RvfError> {
        if data.len() < AGI_HEADER_SIZE {
            return Err(crate::RvfError::SizeMismatch {
                expected: AGI_HEADER_SIZE,
                got: data.len(),
            });
        }
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != AGI_MAGIC {
            return Err(crate::RvfError::BadMagic {
                expected: AGI_MAGIC,
                got: magic,
            });
        }
        let mut container_id = [0u8; 16];
        container_id.copy_from_slice(&data[8..24]);
        let mut build_id = [0u8; 16];
        build_id.copy_from_slice(&data[24..40]);
        let mut model_id_hash = [0u8; 8];
        model_id_hash.copy_from_slice(&data[48..56]);
        let mut policy_hash = [0u8; 8];
        policy_hash.copy_from_slice(&data[56..64]);

        Ok(Self {
            magic,
            version: u16::from_le_bytes([data[4], data[5]]),
            flags: u16::from_le_bytes([data[6], data[7]]),
            container_id,
            build_id,
            created_ns: u64::from_le_bytes([
                data[40], data[41], data[42], data[43], data[44], data[45], data[46], data[47],
            ]),
            model_id_hash,
            policy_hash,
        })
    }
}

/// Required segments for a valid AGI container.
///
/// Used by the container builder/validator to ensure completeness.
#[derive(Clone, Debug, Default)]
pub struct ContainerSegments {
    /// KERNEL_SEG: micro Linux kernel (e.g. Firecracker-compatible vmlinux).
    pub kernel_present: bool,
    /// KERNEL_SEG size in bytes.
    pub kernel_size: u64,
    /// WASM_SEG: interpreter + microkernel modules.
    pub wasm_count: u16,
    /// Total WASM_SEG size in bytes.
    pub wasm_total_size: u64,
    /// VEC_SEG: world model vector count.
    pub vec_segment_count: u16,
    /// INDEX_SEG: HNSW index count.
    pub index_segment_count: u16,
    /// WITNESS_SEG: witness bundle count.
    pub witness_count: u32,
    /// CRYPTO_SEG: present.
    pub crypto_present: bool,
    /// META segment with AGI manifest: present.
    pub manifest_present: bool,
    /// Orchestrator configuration present.
    pub orchestrator_present: bool,
    /// World model data present (VEC + INDEX segments).
    pub world_model_present: bool,
    /// Domain expansion (transfer priors, policy kernels, cost curves) present.
    pub domain_expansion_present: bool,
    /// Total container size in bytes.
    pub total_size: u64,
}

impl ContainerSegments {
    /// Validate that the container has all required segments for a given
    /// execution mode.
    pub fn validate(&self, mode: ExecutionMode) -> Result<(), ContainerError> {
        // All modes require the manifest.
        if !self.manifest_present {
            return Err(ContainerError::MissingSegment("AGI manifest"));
        }

        // Size check.
        if self.total_size > AGI_MAX_CONTAINER_SIZE {
            return Err(ContainerError::TooLarge {
                size: self.total_size,
            });
        }

        match mode {
            ExecutionMode::Replay => {
                // Replay needs witness chains.
                if self.witness_count == 0 {
                    return Err(ContainerError::MissingSegment("witness chain"));
                }
            }
            ExecutionMode::Verify | ExecutionMode::Live => {
                // Verify/Live need at least kernel or WASM.
                if !self.kernel_present && self.wasm_count == 0 {
                    return Err(ContainerError::MissingSegment("kernel or WASM runtime"));
                }
                // Verify/Live need world model data for meaningful operation.
                if !self.world_model_present
                    && self.vec_segment_count == 0
                    && self.index_segment_count == 0
                {
                    return Err(ContainerError::MissingSegment(
                        "world model (VEC or INDEX segments)",
                    ));
                }
            }
        }

        Ok(())
    }

    /// Compute the flags bitfield from present segments.
    pub fn to_flags(&self) -> u16 {
        let mut flags: u16 = 0;
        if self.kernel_present {
            flags |= AGI_HAS_KERNEL;
        }
        if self.wasm_count > 0 {
            flags |= AGI_HAS_WASM;
        }
        if self.witness_count > 0 {
            flags |= AGI_HAS_WITNESS;
        }
        if self.crypto_present {
            flags |= AGI_SIGNED;
        }
        if self.orchestrator_present {
            flags |= AGI_HAS_ORCHESTRATOR;
        }
        if self.world_model_present || self.vec_segment_count > 0 || self.index_segment_count > 0 {
            flags |= AGI_HAS_WORLD_MODEL;
        }
        if self.domain_expansion_present {
            flags |= AGI_HAS_DOMAIN_EXPANSION;
        }
        flags
    }
}

/// Error type for AGI container operations.
#[derive(Debug, PartialEq, Eq)]
pub enum ContainerError {
    /// A required segment is missing.
    MissingSegment(&'static str),
    /// Container exceeds size limit.
    TooLarge { size: u64 },
    /// Invalid segment configuration.
    InvalidConfig(&'static str),
    /// Signature verification failed.
    SignatureInvalid,
    /// Authority level insufficient for the requested action.
    InsufficientAuthority { required: u8, granted: u8 },
    /// Resource budget exceeded.
    BudgetExhausted(&'static str),
}

impl core::fmt::Display for ContainerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ContainerError::MissingSegment(s) => write!(f, "missing segment: {s}"),
            ContainerError::TooLarge { size } => {
                write!(f, "container too large: {size} bytes")
            }
            ContainerError::InvalidConfig(s) => write!(f, "invalid config: {s}"),
            ContainerError::SignatureInvalid => {
                write!(f, "signature verification failed")
            }
            ContainerError::InsufficientAuthority { required, granted } => {
                write!(
                    f,
                    "insufficient authority: required level {required}, granted {granted}"
                )
            }
            ContainerError::BudgetExhausted(resource) => {
                write!(f, "resource budget exhausted: {resource}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn agi_header_size() {
        assert_eq!(core::mem::size_of::<AgiContainerHeader>(), 64);
    }

    #[test]
    fn agi_header_round_trip() {
        let hdr = AgiContainerHeader {
            magic: AGI_MAGIC,
            version: 1,
            flags: AGI_HAS_KERNEL
                | AGI_HAS_ORCHESTRATOR
                | AGI_HAS_WORLD_MODEL
                | AGI_HAS_EVAL
                | AGI_SIGNED
                | AGI_REPLAY_CAPABLE,
            container_id: [0x42; 16],
            build_id: [0x43; 16],
            created_ns: 1_700_000_000_000_000_000,
            model_id_hash: [0xAA; 8],
            policy_hash: [0xBB; 8],
        };
        let bytes = hdr.to_bytes();
        assert_eq!(bytes.len(), AGI_HEADER_SIZE);
        let decoded = AgiContainerHeader::from_bytes(&bytes).unwrap();
        assert_eq!(decoded, hdr);
    }

    #[test]
    fn agi_header_bad_magic() {
        let mut bytes = [0u8; 64];
        bytes[0..4].copy_from_slice(&0xDEADBEEFu32.to_le_bytes());
        assert!(AgiContainerHeader::from_bytes(&bytes).is_err());
    }

    #[test]
    fn agi_header_too_short() {
        assert!(AgiContainerHeader::from_bytes(&[0u8; 32]).is_err());
    }

    #[test]
    fn agi_flags() {
        let hdr = AgiContainerHeader {
            magic: AGI_MAGIC,
            version: 1,
            flags: AGI_HAS_KERNEL | AGI_HAS_ORCHESTRATOR | AGI_SIGNED,
            container_id: [0; 16],
            build_id: [0; 16],
            created_ns: 0,
            model_id_hash: [0; 8],
            policy_hash: [0; 8],
        };
        assert!(hdr.has_kernel());
        assert!(hdr.has_orchestrator());
        assert!(hdr.is_signed());
        assert!(!hdr.is_replay_capable());
        assert!(!hdr.is_offline_capable());
        assert!(!hdr.has_world_model());
        assert!(!hdr.has_coherence_gates());
    }

    #[test]
    fn execution_mode_round_trip() {
        for raw in 0..=2u8 {
            let m = ExecutionMode::try_from(raw).unwrap();
            assert_eq!(m as u8, raw);
        }
        assert!(ExecutionMode::try_from(3).is_err());
    }

    #[test]
    fn segments_validate_replay_needs_witness() {
        let segs = ContainerSegments {
            manifest_present: true,
            witness_count: 0,
            ..Default::default()
        };
        assert_eq!(
            segs.validate(ExecutionMode::Replay),
            Err(ContainerError::MissingSegment("witness chain"))
        );
    }

    #[test]
    fn segments_validate_live_needs_runtime() {
        let segs = ContainerSegments {
            manifest_present: true,
            kernel_present: false,
            wasm_count: 0,
            ..Default::default()
        };
        assert_eq!(
            segs.validate(ExecutionMode::Live),
            Err(ContainerError::MissingSegment("kernel or WASM runtime"))
        );
    }

    #[test]
    fn segments_validate_live_needs_world_model() {
        let segs = ContainerSegments {
            manifest_present: true,
            kernel_present: true,
            vec_segment_count: 0,
            index_segment_count: 0,
            world_model_present: false,
            ..Default::default()
        };
        assert_eq!(
            segs.validate(ExecutionMode::Live),
            Err(ContainerError::MissingSegment(
                "world model (VEC or INDEX segments)"
            ))
        );
    }

    #[test]
    fn segments_validate_live_with_kernel_and_world_model() {
        let segs = ContainerSegments {
            manifest_present: true,
            kernel_present: true,
            world_model_present: true,
            ..Default::default()
        };
        assert!(segs.validate(ExecutionMode::Live).is_ok());
    }

    #[test]
    fn segments_validate_live_with_wasm_and_vec() {
        let segs = ContainerSegments {
            manifest_present: true,
            wasm_count: 2,
            vec_segment_count: 1,
            ..Default::default()
        };
        assert!(segs.validate(ExecutionMode::Live).is_ok());
    }

    #[test]
    fn segments_validate_replay_with_witness() {
        let segs = ContainerSegments {
            manifest_present: true,
            witness_count: 10,
            ..Default::default()
        };
        assert!(segs.validate(ExecutionMode::Replay).is_ok());
    }

    #[test]
    fn segments_validate_too_large() {
        let segs = ContainerSegments {
            manifest_present: true,
            total_size: AGI_MAX_CONTAINER_SIZE + 1,
            ..Default::default()
        };
        assert_eq!(
            segs.validate(ExecutionMode::Replay),
            Err(ContainerError::TooLarge {
                size: AGI_MAX_CONTAINER_SIZE + 1
            })
        );
    }

    #[test]
    fn segments_to_flags() {
        let segs = ContainerSegments {
            kernel_present: true,
            wasm_count: 1,
            witness_count: 5,
            crypto_present: true,
            orchestrator_present: true,
            vec_segment_count: 3,
            ..Default::default()
        };
        let flags = segs.to_flags();
        assert_ne!(flags & AGI_HAS_KERNEL, 0);
        assert_ne!(flags & AGI_HAS_WASM, 0);
        assert_ne!(flags & AGI_HAS_WITNESS, 0);
        assert_ne!(flags & AGI_SIGNED, 0);
        assert_ne!(flags & AGI_HAS_ORCHESTRATOR, 0);
        assert_ne!(flags & AGI_HAS_WORLD_MODEL, 0);
    }

    #[test]
    fn container_error_display() {
        let e = ContainerError::MissingSegment("kernel");
        assert!(format!("{e}").contains("kernel"));
        let e2 = ContainerError::TooLarge { size: 999 };
        assert!(format!("{e2}").contains("999"));
        let e3 = ContainerError::InsufficientAuthority {
            required: 3,
            granted: 1,
        };
        assert!(format!("{e3}").contains("required level 3"));
        let e4 = ContainerError::BudgetExhausted("tokens");
        assert!(format!("{e4}").contains("tokens"));
    }

    // --- Authority level tests ---

    #[test]
    fn authority_level_round_trip() {
        for raw in 0..=3u8 {
            let a = AuthorityLevel::try_from(raw).unwrap();
            assert_eq!(a as u8, raw);
        }
        assert!(AuthorityLevel::try_from(4).is_err());
    }

    #[test]
    fn authority_level_ordering() {
        assert!(AuthorityLevel::ReadOnly < AuthorityLevel::WriteMemory);
        assert!(AuthorityLevel::WriteMemory < AuthorityLevel::ExecuteTools);
        assert!(AuthorityLevel::ExecuteTools < AuthorityLevel::WriteExternal);
    }

    #[test]
    fn authority_permits() {
        assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::ReadOnly));
        assert!(AuthorityLevel::WriteExternal.permits(AuthorityLevel::WriteExternal));
        assert!(AuthorityLevel::ExecuteTools.permits(AuthorityLevel::WriteMemory));
        assert!(!AuthorityLevel::ReadOnly.permits(AuthorityLevel::WriteMemory));
        assert!(!AuthorityLevel::WriteMemory.permits(AuthorityLevel::ExecuteTools));
    }

    #[test]
    fn authority_default_for_mode() {
        assert_eq!(
            AuthorityLevel::default_for_mode(ExecutionMode::Replay),
            AuthorityLevel::ReadOnly
        );
        assert_eq!(
            AuthorityLevel::default_for_mode(ExecutionMode::Verify),
            AuthorityLevel::ExecuteTools
        );
        assert_eq!(
            AuthorityLevel::default_for_mode(ExecutionMode::Live),
            AuthorityLevel::WriteMemory
        );
    }

    // --- Resource budget tests ---

    #[test]
    fn resource_budget_default() {
        let b = ResourceBudget::default();
        assert_eq!(b.max_time_secs, 300);
        assert_eq!(b.max_tokens, 200_000);
        assert_eq!(b.max_cost_microdollars, 1_000_000);
        assert_eq!(b.max_tool_calls, 50);
        assert_eq!(b.max_external_writes, 0);
    }

    #[test]
    fn resource_budget_clamped() {
        let over = ResourceBudget {
            max_time_secs: 999_999,
            max_tokens: 999_999_999,
            max_cost_microdollars: 999_999_999,
            max_tool_calls: 60_000,
            max_external_writes: 60_000,
        };
        let clamped = over.clamped();
        assert_eq!(clamped.max_time_secs, ResourceBudget::MAX.max_time_secs);
        assert_eq!(clamped.max_tokens, ResourceBudget::MAX.max_tokens);
        assert_eq!(
            clamped.max_cost_microdollars,
            ResourceBudget::MAX.max_cost_microdollars
        );
        assert_eq!(clamped.max_tool_calls, ResourceBudget::MAX.max_tool_calls);
        assert_eq!(
            clamped.max_external_writes,
            ResourceBudget::MAX.max_external_writes
        );
    }

    #[test]
    fn resource_budget_within_max_unchanged() {
        let within = ResourceBudget::DEFAULT;
        let clamped = within.clamped();
        assert_eq!(clamped, within);
    }

    // --- Coherence threshold tests ---

    #[test]
    fn coherence_thresholds_default() {
        let ct = CoherenceThresholds::default();
        assert!((ct.min_coherence_score - 0.70).abs() < f32::EPSILON);
        assert!((ct.max_contradiction_rate - 5.0).abs() < f32::EPSILON);
        assert!((ct.max_rollback_ratio - 0.20).abs() < f32::EPSILON);
    }

    #[test]
    fn coherence_thresholds_strict() {
        let ct = CoherenceThresholds::STRICT;
        assert!((ct.min_coherence_score - 0.85).abs() < f32::EPSILON);
        assert!((ct.max_contradiction_rate - 2.0).abs() < f32::EPSILON);
        assert!((ct.max_rollback_ratio - 0.10).abs() < f32::EPSILON);
    }

    #[test]
    fn coherence_thresholds_validate_valid() {
        assert!(CoherenceThresholds::DEFAULT.validate().is_ok());
        assert!(CoherenceThresholds::STRICT.validate().is_ok());
    }

    #[test]
    fn coherence_thresholds_validate_bad_score() {
        let ct = CoherenceThresholds {
            min_coherence_score: 1.5,
            ..CoherenceThresholds::DEFAULT
        };
        assert_eq!(
            ct.validate(),
            Err(ContainerError::InvalidConfig(
                "min_coherence_score must be in [0.0, 1.0]"
            ))
        );
    }

    #[test]
    fn coherence_thresholds_validate_negative_rate() {
        let ct = CoherenceThresholds {
            max_contradiction_rate: -1.0,
            ..CoherenceThresholds::DEFAULT
        };
        assert_eq!(
            ct.validate(),
            Err(ContainerError::InvalidConfig(
                "max_contradiction_rate must be >= 0.0"
            ))
        );
    }

    #[test]
    fn coherence_thresholds_validate_bad_ratio() {
        let ct = CoherenceThresholds {
            max_rollback_ratio: 2.0,
            ..CoherenceThresholds::DEFAULT
        };
        assert_eq!(
            ct.validate(),
            Err(ContainerError::InvalidConfig(
                "max_rollback_ratio must be in [0.0, 1.0]"
            ))
        );
    }
}
