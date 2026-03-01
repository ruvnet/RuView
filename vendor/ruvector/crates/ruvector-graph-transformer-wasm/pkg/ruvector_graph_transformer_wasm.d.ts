/* tslint:disable */
/* eslint-disable */

/**
 * Graph transformer for the browser.
 *
 * Wraps the core `CoreGraphTransformer` and exposes proof-gated, sublinear,
 * physics, biological, verified-training, manifold, temporal, and economic
 * operations via wasm_bindgen.
 */
export class JsGraphTransformer {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Causal attention with temporal ordering over graph edges.
     *
     * `features` is a Float64Array, `timestamps` is a Float64Array,
     * `edges` is `[{ src, tgt }, ...]`.
     * Returns attention-weighted output features.
     */
    causal_attention(features: any, timestamps: any, edges: any): any;
    /**
     * Compose a chain of pipeline stages, verifying type compatibility.
     *
     * `stages` is a JS array of `{ name, input_type_id, output_type_id }`.
     * Returns a composed proof with the overall input/output types.
     */
    compose_proofs(stages: any): any;
    /**
     * Create a proof attestation for a given proof ID.
     *
     * Returns the attestation as a byte buffer (82 bytes).
     */
    create_attestation(proof_id: number): Uint8Array;
    /**
     * Create a proof gate for the given embedding dimension.
     *
     * Returns a serialized `ProofGate` object.
     */
    create_proof_gate(dim: number): any;
    /**
     * Game-theoretic attention: computes Nash equilibrium allocations.
     *
     * `features` is a Float64Array, `edges` is `[{ src, tgt }, ...]`.
     * Returns `{ allocations, utilities, nash_gap, converged }`.
     */
    game_theoretic_attention(features: any, edges: any): any;
    /**
     * Extract Granger causality DAG from attention history.
     *
     * `attention_history` is a flat Float64Array (T x N row-major).
     * Returns `{ edges: [{ source, target, f_statistic, is_causal }], num_nodes }`.
     */
    granger_extract(attention_history: any, num_nodes: number, num_steps: number): any;
    /**
     * Symplectic integrator step (leapfrog / Stormer-Verlet).
     *
     * `positions` and `momenta` are Float64Arrays, `edges` is
     * `[{ src, tgt }, ...]`. Returns `{ positions, momenta, energy,
     * energy_conserved }`.
     */
    hamiltonian_step(positions: any, momenta: any, edges: any): any;
    /**
     * Hebbian weight update.
     *
     * `pre`, `post`, `weights` are Float64Arrays. Returns updated weights.
     */
    hebbian_update(pre: any, post: any, weights: any): any;
    /**
     * Create a new graph transformer.
     *
     * `config` is an optional JS object (reserved for future use).
     */
    constructor(config: any);
    /**
     * Compute personalized PageRank scores from a source node.
     *
     * Returns array of PPR scores, one per node.
     */
    ppr_scores(source: number, adjacency: any, alpha: number): any;
    /**
     * Product manifold attention with mixed curvatures.
     *
     * `features` is a Float64Array, `edges` is `[{ src, tgt }, ...]`.
     * Optional `curvatures` (defaults to `[0.0, -1.0]`).
     * Returns `{ output, curvatures, distances }`.
     */
    product_manifold_attention(features: any, edges: any): any;
    /**
     * Product manifold distance between two points.
     *
     * `a` and `b` are Float64Arrays, `curvatures` is `[number, ...]`.
     */
    product_manifold_distance(a: any, b: any, curvatures: any): number;
    /**
     * Prove that two dimensions are equal.
     *
     * Returns `{ proof_id, expected, actual, verified }`.
     */
    prove_dimension(expected: number, actual: number): any;
    /**
     * Reset all internal state (caches, counters, gates).
     */
    reset(): void;
    /**
     * Spiking neural attention step over 2D features with adjacency.
     *
     * `features` is `[[f64, ...], ...]`, `adjacency` is a flat row-major
     * Float64Array (n x n). Returns `{ features, spikes, weights }`.
     */
    spiking_step(features: any, adjacency: any): any;
    /**
     * Return transformer statistics.
     *
     * Returns `{ proofs_constructed, proofs_verified, cache_hits,
     * cache_misses, attention_ops, physics_ops, bio_ops, training_steps }`.
     */
    stats(): any;
    /**
     * Sublinear graph attention using personalized PageRank sparsification.
     *
     * `query` is a Float64Array, `edges` is `[[u32, ...], ...]`.
     * Returns `{ scores, top_k_indices, sparsity_ratio }`.
     */
    sublinear_attention(query: any, edges: any, dim: number, k: number): any;
    /**
     * A single verified SGD step (raw weights + gradients).
     *
     * Returns `{ weights, proof_id, loss_before, loss_after, gradient_norm }`.
     */
    verified_step(weights: any, gradients: any, lr: number): any;
    /**
     * Verified training step with features, targets, and weights.
     *
     * `features`, `targets`, `weights` are Float64Arrays.
     * Returns `{ weights, certificate_id, loss, loss_monotonic,
     * lipschitz_satisfied }`.
     */
    verified_training_step(features: any, targets: any, weights: any): any;
    /**
     * Verify an attestation from its byte representation.
     *
     * Returns `true` if the attestation is structurally valid.
     */
    verify_attestation(bytes: Uint8Array): boolean;
    /**
     * Verify energy conservation between two states.
     *
     * Returns `{ conserved, delta, relative_error }`.
     */
    verify_energy_conservation(before: number, after: number, tolerance: number): any;
    /**
     * Get the library version string.
     */
    version(): string;
}

/**
 * Called automatically when the WASM module is loaded.
 */
export function init(): void;

/**
 * Return the crate version.
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_jsgraphtransformer_free: (a: number, b: number) => void;
    readonly init: () => void;
    readonly jsgraphtransformer_causal_attention: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_compose_proofs: (a: number, b: number, c: number) => void;
    readonly jsgraphtransformer_create_attestation: (a: number, b: number, c: number) => void;
    readonly jsgraphtransformer_create_proof_gate: (a: number, b: number, c: number) => void;
    readonly jsgraphtransformer_game_theoretic_attention: (a: number, b: number, c: number, d: number) => void;
    readonly jsgraphtransformer_granger_extract: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_hamiltonian_step: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_hebbian_update: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_new: (a: number, b: number) => void;
    readonly jsgraphtransformer_ppr_scores: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_product_manifold_attention: (a: number, b: number, c: number, d: number) => void;
    readonly jsgraphtransformer_product_manifold_distance: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_prove_dimension: (a: number, b: number, c: number, d: number) => void;
    readonly jsgraphtransformer_reset: (a: number) => void;
    readonly jsgraphtransformer_spiking_step: (a: number, b: number, c: number, d: number) => void;
    readonly jsgraphtransformer_stats: (a: number, b: number) => void;
    readonly jsgraphtransformer_sublinear_attention: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly jsgraphtransformer_verified_step: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_verified_training_step: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_verify_attestation: (a: number, b: number, c: number) => number;
    readonly jsgraphtransformer_verify_energy_conservation: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly jsgraphtransformer_version: (a: number, b: number) => void;
    readonly version: (a: number) => void;
    readonly __wbindgen_export: (a: number, b: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_export3: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
