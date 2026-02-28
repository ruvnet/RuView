//! RVF Self-Learning Solver WASM Module
//!
//! Exposes the complete AGI temporal reasoning engine as WASM exports:
//! - PolicyKernel with Thompson Sampling (two-signal model)
//! - Context-bucketed bandit (18 buckets: 3 range x 3 distractor x 2 noise)
//! - KnowledgeCompiler with signature-based pattern cache
//! - Speculative dual-path execution
//! - Three-loop adaptive solver (fast/medium/slow)
//! - Acceptance test with training/holdout cycles
//! - SHAKE-256 witness chain via rvf-crypto
//!
//! Target: wasm32-unknown-unknown, no_std + alloc.
//!
//! ## WASM Exports
//!
//! | Export | Description |
//! |--------|-------------|
//! | `rvf_solver_alloc` | Allocate WASM memory |
//! | `rvf_solver_free` | Free WASM memory |
//! | `rvf_solver_create` | Create solver instance → handle |
//! | `rvf_solver_destroy` | Destroy solver instance |
//! | `rvf_solver_train` | Train on generated puzzles |
//! | `rvf_solver_acceptance` | Run full acceptance test |
//! | `rvf_solver_result_len` | Get last result JSON length |
//! | `rvf_solver_result_read` | Read last result JSON |
//! | `rvf_solver_policy_len` | Get policy state JSON length |
//! | `rvf_solver_policy_read` | Read policy state JSON |
//! | `rvf_solver_witness_len` | Get witness chain byte length |
//! | `rvf_solver_witness_read` | Read witness chain bytes |

#![no_std]

extern crate alloc;

mod alloc_setup;
pub mod engine;
pub mod policy;
pub mod types;

use alloc::vec::Vec;

use engine::{AcceptanceConfig, AcceptanceResult, AdaptiveSolver, PuzzleGenerator, run_acceptance_mode};
use rvf_crypto::{create_witness_chain, WitnessEntry};

// ═════════════════════════════════════════════════════════════════════
// Instance registry (max 8 concurrent solvers)
// ═════════════════════════════════════════════════════════════════════

const MAX_INSTANCES: usize = 8;

struct SolverInstance {
    solver: AdaptiveSolver,
    last_result_json: Vec<u8>,
    policy_json: Vec<u8>,
    witness_chain: Vec<u8>,
}

struct Registry {
    slots: [Option<SolverInstance>; MAX_INSTANCES],
}

impl Registry {
    const fn new() -> Self {
        Self {
            slots: [const { None }; MAX_INSTANCES],
        }
    }

    fn create(&mut self) -> i32 {
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(SolverInstance {
                    solver: AdaptiveSolver::new(),
                    last_result_json: Vec::new(),
                    policy_json: Vec::new(),
                    witness_chain: Vec::new(),
                });
                return (i + 1) as i32;
            }
        }
        -1
    }

    fn get(&self, handle: i32) -> Option<&SolverInstance> {
        let idx = (handle - 1) as usize;
        if idx < MAX_INSTANCES {
            self.slots[idx].as_ref()
        } else {
            None
        }
    }

    fn get_mut(&mut self, handle: i32) -> Option<&mut SolverInstance> {
        let idx = (handle - 1) as usize;
        if idx < MAX_INSTANCES {
            self.slots[idx].as_mut()
        } else {
            None
        }
    }

    fn destroy(&mut self, handle: i32) -> i32 {
        let idx = (handle - 1) as usize;
        if idx < MAX_INSTANCES && self.slots[idx].is_some() {
            self.slots[idx] = None;
            0
        } else {
            -1
        }
    }
}

// Global mutable registry — safe in single-threaded WASM.
static mut REGISTRY: Registry = Registry::new();

#[allow(static_mut_refs)]
fn registry() -> &'static mut Registry {
    unsafe { &mut REGISTRY }
}

// ═════════════════════════════════════════════════════════════════════
// WASM Exports — Lifecycle
// ═════════════════════════════════════════════════════════════════════

/// Create a new solver instance. Returns handle (>0) or -1 on error.
#[no_mangle]
pub extern "C" fn rvf_solver_create() -> i32 {
    registry().create()
}

/// Destroy a solver instance. Returns 0 on success.
#[no_mangle]
pub extern "C" fn rvf_solver_destroy(handle: i32) -> i32 {
    registry().destroy(handle)
}

// ═════════════════════════════════════════════════════════════════════
// WASM Exports — Training
// ═════════════════════════════════════════════════════════════════════

/// Train the solver on `count` generated puzzles.
///
/// Uses the three-loop architecture: fast (solve), medium (PolicyKernel),
/// slow (KnowledgeCompiler). Returns number of puzzles solved correctly.
///
/// Parameters:
/// - handle: solver instance
/// - count: number of puzzles to generate and solve
/// - min_diff: minimum puzzle difficulty (1-10)
/// - max_diff: maximum puzzle difficulty (1-10)
/// - seed_lo: lower 32 bits of RNG seed
/// - seed_hi: upper 32 bits of RNG seed
#[no_mangle]
pub extern "C" fn rvf_solver_train(
    handle: i32,
    count: i32,
    min_diff: i32,
    max_diff: i32,
    seed_lo: i32,
    seed_hi: i32,
) -> i32 {
    let inst = match registry().get_mut(handle) {
        Some(i) => i,
        None => return -1,
    };

    let seed = ((seed_hi as u64) << 32) | (seed_lo as u64 & 0xFFFFFFFF);
    let mut gen = PuzzleGenerator::new(seed, min_diff as u8, max_diff as u8);
    let puzzles = gen.generate_batch(count as usize);

    inst.solver.compiler_enabled = true;
    inst.solver.router_enabled = true;

    let mut correct = 0i32;
    for puzzle in &puzzles {
        let result = inst.solver.solve(puzzle);
        if result.correct {
            correct += 1;
        }
    }

    // Promote learned patterns
    inst.solver.bank.promote();
    inst.solver.bank.compile_to(&mut inst.solver.compiler);

    // Serialize result
    let result_json = serde_json::to_vec(&AcceptanceSummary {
        trained: count as usize,
        correct: correct as usize,
        accuracy: correct as f64 / count as f64,
        patterns_learned: inst.solver.bank.patterns_learned,
    })
    .unwrap_or_default();
    inst.last_result_json = result_json;

    correct
}

#[derive(serde::Serialize)]
struct AcceptanceSummary {
    trained: usize,
    correct: usize,
    accuracy: f64,
    patterns_learned: usize,
}

// ═════════════════════════════════════════════════════════════════════
// WASM Exports — Acceptance Test
// ═════════════════════════════════════════════════════════════════════

/// Run the full acceptance test with training/holdout cycles.
///
/// Runs all three ablation modes (A/B/C) and produces a manifest.
/// Returns: 1 = passed, 0 = failed, -1 = error.
///
/// After this call, use `rvf_solver_result_len` / `rvf_solver_result_read`
/// to retrieve the full manifest JSON.
#[no_mangle]
pub extern "C" fn rvf_solver_acceptance(
    handle: i32,
    holdout: i32,
    training: i32,
    cycles: i32,
    budget: i32,
    seed_lo: i32,
    seed_hi: i32,
) -> i32 {
    let inst = match registry().get_mut(handle) {
        Some(i) => i,
        None => return -1,
    };

    let seed = ((seed_hi as u64) << 32) | (seed_lo as u64 & 0xFFFFFFFF);
    let config = AcceptanceConfig {
        holdout_size: holdout as usize,
        training_per_cycle: training as usize,
        cycles: cycles as usize,
        step_budget: budget as usize,
        holdout_seed: seed,
        training_seed: seed.wrapping_add(1),
        noise_rate: 0.25,
        min_accuracy: 0.80,
    };

    // Run all three modes
    let mode_a = run_acceptance_mode(&config, false, false);
    let mode_b = run_acceptance_mode(&config, true, false);
    let mode_c = run_acceptance_mode(&config, true, true);

    // Build witness chain from cycle metrics
    let mut witness_entries: Vec<WitnessEntry> = Vec::new();
    let mut seq: u64 = 0;
    for (label, result) in [("A", &mode_a), ("B", &mode_b), ("C", &mode_c)] {
        for cm in &result.cycles {
            let mut action_data = Vec::with_capacity(64);
            action_data.extend_from_slice(label.as_bytes());
            action_data.extend_from_slice(&(cm.cycle as u64).to_le_bytes());
            action_data.extend_from_slice(&cm.accuracy.to_le_bytes());
            action_data.extend_from_slice(&cm.cost_per_solve.to_le_bytes());
            let action_hash = rvf_crypto::shake256_256(&action_data);
            witness_entries.push(WitnessEntry {
                prev_hash: [0u8; 32],
                action_hash,
                timestamp_ns: seq,
                witness_type: 0x02,
            });
            seq += 1;
        }
    }

    // Create SHAKE-256 witness chain
    inst.witness_chain = create_witness_chain(&witness_entries);

    // Build manifest JSON
    let manifest = AcceptanceManifest {
        version: 2,
        mode_a,
        mode_b,
        mode_c: mode_c.clone(),
        all_passed: mode_c.passed, // C is the full mode
        witness_entries: witness_entries.len(),
        witness_chain_bytes: inst.witness_chain.len(),
    };

    inst.last_result_json = serde_json::to_vec(&manifest).unwrap_or_default();

    // Update solver state with Mode C results
    inst.solver.compiler_enabled = true;
    inst.solver.router_enabled = true;

    // Serialize policy state
    inst.policy_json = serde_json::to_vec(&inst.solver.policy_kernel).unwrap_or_default();

    if mode_c.passed { 1 } else { 0 }
}

#[derive(serde::Serialize)]
struct AcceptanceManifest {
    version: u32,
    mode_a: AcceptanceResult,
    mode_b: AcceptanceResult,
    mode_c: AcceptanceResult,
    all_passed: bool,
    witness_entries: usize,
    witness_chain_bytes: usize,
}

// ═════════════════════════════════════════════════════════════════════
// WASM Exports — Result / Policy / Witness reads
// ═════════════════════════════════════════════════════════════════════

/// Get the byte length of the last result JSON.
#[no_mangle]
pub extern "C" fn rvf_solver_result_len(handle: i32) -> i32 {
    registry()
        .get(handle)
        .map(|i| i.last_result_json.len() as i32)
        .unwrap_or(-1)
}

/// Copy the last result JSON into `out_ptr`. Returns bytes written.
#[no_mangle]
pub extern "C" fn rvf_solver_result_read(handle: i32, out_ptr: i32) -> i32 {
    let inst = match registry().get(handle) {
        Some(i) => i,
        None => return -1,
    };
    let data = &inst.last_result_json;
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), out_ptr as *mut u8, data.len());
    }
    data.len() as i32
}

/// Get the byte length of the policy state JSON.
#[no_mangle]
pub extern "C" fn rvf_solver_policy_len(handle: i32) -> i32 {
    let inst = match registry().get_mut(handle) {
        Some(i) => i,
        None => return -1,
    };
    // Refresh policy JSON
    inst.policy_json = serde_json::to_vec(&inst.solver.policy_kernel).unwrap_or_default();
    inst.policy_json.len() as i32
}

/// Copy the policy state JSON into `out_ptr`. Returns bytes written.
#[no_mangle]
pub extern "C" fn rvf_solver_policy_read(handle: i32, out_ptr: i32) -> i32 {
    let inst = match registry().get(handle) {
        Some(i) => i,
        None => return -1,
    };
    let data = &inst.policy_json;
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), out_ptr as *mut u8, data.len());
    }
    data.len() as i32
}

/// Get the byte length of the witness chain (73 bytes per entry).
#[no_mangle]
pub extern "C" fn rvf_solver_witness_len(handle: i32) -> i32 {
    registry()
        .get(handle)
        .map(|i| i.witness_chain.len() as i32)
        .unwrap_or(-1)
}

/// Copy the raw witness chain bytes into `out_ptr`.
///
/// The witness chain is in native rvf-crypto format: 73 bytes per entry,
/// verifiable by `rvf_witness_verify` in the rvf-wasm microkernel.
#[no_mangle]
pub extern "C" fn rvf_solver_witness_read(handle: i32, out_ptr: i32) -> i32 {
    let inst = match registry().get(handle) {
        Some(i) => i,
        None => return -1,
    };
    let data = &inst.witness_chain;
    unsafe {
        core::ptr::copy_nonoverlapping(data.as_ptr(), out_ptr as *mut u8, data.len());
    }
    data.len() as i32
}

// ═════════════════════════════════════════════════════════════════════
// Panic handler
// ═════════════════════════════════════════════════════════════════════

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
