/**
 * Type declarations for the RVF Solver WASM module exports.
 */

export interface RvfSolverWasmExports {
  memory: WebAssembly.Memory;

  // Memory management
  rvf_solver_alloc(size: number): number;
  rvf_solver_free(ptr: number, size: number): void;

  // Lifecycle
  rvf_solver_create(): number;
  rvf_solver_destroy(handle: number): number;

  // Training
  rvf_solver_train(
    handle: number,
    count: number,
    min_diff: number,
    max_diff: number,
    seed_lo: number,
    seed_hi: number,
  ): number;

  // Acceptance testing
  rvf_solver_acceptance(
    handle: number,
    holdout: number,
    training: number,
    cycles: number,
    budget: number,
    seed_lo: number,
    seed_hi: number,
  ): number;

  // Result reads
  rvf_solver_result_len(handle: number): number;
  rvf_solver_result_read(handle: number, out_ptr: number): number;
  rvf_solver_policy_len(handle: number): number;
  rvf_solver_policy_read(handle: number, out_ptr: number): number;
  rvf_solver_witness_len(handle: number): number;
  rvf_solver_witness_read(handle: number, out_ptr: number): number;
}

export default function init(
  input?: ArrayBuffer | Uint8Array | WebAssembly.Module | string,
): Promise<RvfSolverWasmExports>;
