//! Hyper-optimized RVF example with Linux kernel embedding and formal verification.
//!
//! Demonstrates `ruvector-verified` as the optimization layer for a kernel-embedded
//! RVF container. Every vector operation passes through verified proofs using:
//! - `FastTermArena` — O(1) bump allocation with 4-wide dedup cache
//! - `ConversionCache` — open-addressing conversion equality cache
//! - Gated proof routing — 3-tier Reflex/Standard/Deep with auto-escalation
//! - Thread-local pools — zero-contention resource reuse
//! - `ProofAttestation` — 82-byte formal proof witness (type 0x0E)

pub mod kernel_embed;
pub mod verified_ingest;

/// Default vector dimension (384 = 48x8 AVX2 / 96x4 NEON aligned).
pub const DEFAULT_DIM: u32 = 384;

/// Default vector count for benchmarks.
pub const DEFAULT_VEC_COUNT: usize = 10_000;

/// Optimized kernel cmdline for vector workload microVMs.
///
/// - `nokaslr nosmp`: deterministic single-core execution
/// - `transparent_hugepage=always`: 2MB pages for vector arrays
/// - `isolcpus=1 nohz_full=1 rcu_nocbs=1`: CPU isolation, no timer ticks
/// - `mitigations=off`: full speed in trusted microVM
pub const KERNEL_CMDLINE: &str = "console=ttyS0 quiet nokaslr nosmp \
    transparent_hugepage=always isolcpus=1 nohz_full=1 rcu_nocbs=1 mitigations=off";

/// Configuration for the verified RVF pipeline.
pub struct VerifiedRvfConfig {
    /// Vector dimensionality.
    pub dim: u32,
    /// Number of vectors to ingest.
    pub vec_count: usize,
    /// Embed precompiled eBPF programs (XDP, socket, TC).
    pub enable_ebpf: bool,
    /// Max reduction steps for Deep-tier proofs.
    pub proof_fuel: usize,
}

impl Default for VerifiedRvfConfig {
    fn default() -> Self {
        Self {
            dim: DEFAULT_DIM,
            vec_count: 1_000,
            enable_ebpf: true,
            proof_fuel: 10_000,
        }
    }
}
