/* tslint:disable */
/* eslint-disable */

/**
 * Category theory engine
 */
export class CategoryEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Apply morphism to an object
     */
    applyMorphism(morphism_js: any, data_js: any): any;
    /**
     * Compose two morphisms
     */
    composeMorphisms(f_js: any, g_js: any): any;
    /**
     * Functorial retrieval: find similar objects
     */
    functorialRetrieve(category_js: any, query_js: any, k: number): any;
    /**
     * Create a new category engine
     */
    constructor();
    /**
     * Verify categorical laws
     */
    verifyCategoryLaws(category_js: any): boolean;
    /**
     * Check if functor preserves composition
     */
    verifyFunctoriality(functor_js: any, source_cat_js: any): boolean;
}

/**
 * Causal inference engine
 */
export class CausalEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Check d-separation between two variables
     */
    checkDSeparation(model_js: any, x: string, y: string, conditioning_js: any): any;
    /**
     * Compute causal effect via do-operator
     */
    computeCausalEffect(model_js: any, treatment: string, outcome: string, treatment_value: number): any;
    /**
     * Find all confounders between two variables
     */
    findConfounders(model_js: any, treatment: string, outcome: string): any;
    /**
     * Check if model is a valid DAG
     */
    isValidDag(model_js: any): boolean;
    /**
     * Create a new causal engine
     */
    constructor();
    /**
     * Get topological order of variables
     */
    topologicalOrder(model_js: any): any;
}

/**
 * Sheaf cohomology computation engine
 */
export class CohomologyEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute cohomology groups of a sheaf graph
     */
    computeCohomology(graph_js: any): any;
    /**
     * Compute global sections (H^0)
     */
    computeGlobalSections(graph_js: any): any;
    /**
     * Compute consistency energy
     */
    consistencyEnergy(graph_js: any): number;
    /**
     * Detect all obstructions to global consistency
     */
    detectObstructions(graph_js: any): any;
    /**
     * Create a new cohomology engine
     */
    constructor();
    /**
     * Create with custom tolerance
     */
    static withTolerance(tolerance: number): CohomologyEngine;
}

/**
 * HoTT type checking and path operations engine
 */
export class HoTTEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Check type equivalence (univalence-related)
     */
    checkTypeEquivalence(type1_js: any, type2_js: any): boolean;
    /**
     * Compose two paths
     */
    composePaths(path1_js: any, path2_js: any): any;
    /**
     * Create reflexivity path
     */
    createReflPath(type_js: any, point_js: any): any;
    /**
     * Infer type of a term
     */
    inferType(term_js: any): any;
    /**
     * Invert a path
     */
    invertPath(path_js: any): any;
    /**
     * Create a new HoTT engine
     */
    constructor();
    /**
     * Type check a term
     */
    typeCheck(term_js: any, expected_type_js: any): any;
    /**
     * Create with strict mode
     */
    static withStrictMode(strict: boolean): HoTTEngine;
}

/**
 * Quantum computing and topological analysis engine
 */
export class QuantumEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Simulate quantum circuit evolution
     */
    applyGate(state_js: any, gate_js: any, target_qubit: number): any;
    /**
     * Compute entanglement entropy
     */
    computeEntanglementEntropy(state_js: any, subsystem_size: number): number;
    /**
     * Compute quantum state fidelity
     */
    computeFidelity(state1_js: any, state2_js: any): any;
    /**
     * Compute topological invariants of a simplicial complex
     */
    computeTopologicalInvariants(simplices_js: any): any;
    /**
     * Create a GHZ state
     */
    createGHZState(num_qubits: number): any;
    /**
     * Create a W state
     */
    createWState(num_qubits: number): any;
    /**
     * Create a new quantum engine
     */
    constructor();
}

/**
 * Spectral analysis engine
 */
export class SpectralEngine {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Compute the algebraic connectivity (Fiedler value)
     */
    algebraicConnectivity(graph_js: any): number;
    /**
     * Compute Cheeger bounds for a graph
     */
    computeCheegerBounds(graph_js: any): any;
    /**
     * Compute eigenvalues of the graph Laplacian
     */
    computeEigenvalues(graph_js: any): any;
    /**
     * Compute Fiedler vector
     */
    computeFiedlerVector(graph_js: any): any;
    /**
     * Compute spectral gap
     */
    computeSpectralGap(graph_js: any): any;
    /**
     * Create a new spectral engine
     */
    constructor();
    /**
     * Predict minimum cut
     */
    predictMinCut(graph_js: any): any;
    /**
     * Create with configuration
     */
    static withConfig(num_eigenvalues: number, tolerance: number, max_iterations: number): SpectralEngine;
}

/**
 * JavaScript-friendly error type
 */
export class WasmError {
    private constructor();
    free(): void;
    [Symbol.dispose](): void;
    readonly code: string;
    readonly message: string;
}

/**
 * Get library version
 */
export function getVersion(): string;

/**
 * Initialize the WASM module
 */
export function initModule(): void;

export function start(): void;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_categoryengine_free: (a: number, b: number) => void;
    readonly __wbg_hottengine_free: (a: number, b: number) => void;
    readonly __wbg_spectralengine_free: (a: number, b: number) => void;
    readonly __wbg_wasmerror_free: (a: number, b: number) => void;
    readonly categoryengine_applyMorphism: (a: number, b: any, c: any) => [number, number, number];
    readonly categoryengine_composeMorphisms: (a: number, b: any, c: any) => [number, number, number];
    readonly categoryengine_functorialRetrieve: (a: number, b: any, c: any, d: number) => [number, number, number];
    readonly categoryengine_new: () => number;
    readonly categoryengine_verifyCategoryLaws: (a: number, b: any) => [number, number, number];
    readonly categoryengine_verifyFunctoriality: (a: number, b: any, c: any) => [number, number, number];
    readonly causalengine_checkDSeparation: (a: number, b: any, c: number, d: number, e: number, f: number, g: any) => [number, number, number];
    readonly causalengine_computeCausalEffect: (a: number, b: any, c: number, d: number, e: number, f: number, g: number) => [number, number, number];
    readonly causalengine_findConfounders: (a: number, b: any, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly causalengine_isValidDag: (a: number, b: any) => [number, number, number];
    readonly causalengine_topologicalOrder: (a: number, b: any) => [number, number, number];
    readonly cohomologyengine_computeCohomology: (a: number, b: any) => [number, number, number];
    readonly cohomologyengine_computeGlobalSections: (a: number, b: any) => [number, number, number];
    readonly cohomologyengine_consistencyEnergy: (a: number, b: any) => [number, number, number];
    readonly cohomologyengine_detectObstructions: (a: number, b: any) => [number, number, number];
    readonly cohomologyengine_withTolerance: (a: number) => number;
    readonly getVersion: () => [number, number];
    readonly hottengine_checkTypeEquivalence: (a: number, b: any, c: any) => [number, number, number];
    readonly hottengine_composePaths: (a: number, b: any, c: any) => [number, number, number];
    readonly hottengine_createReflPath: (a: number, b: any, c: any) => [number, number, number];
    readonly hottengine_inferType: (a: number, b: any) => [number, number, number];
    readonly hottengine_invertPath: (a: number, b: any) => [number, number, number];
    readonly hottengine_new: () => number;
    readonly hottengine_typeCheck: (a: number, b: any, c: any) => [number, number, number];
    readonly hottengine_withStrictMode: (a: number) => number;
    readonly initModule: () => [number, number];
    readonly quantumengine_applyGate: (a: number, b: any, c: any, d: number) => [number, number, number];
    readonly quantumengine_computeEntanglementEntropy: (a: number, b: any, c: number) => [number, number, number];
    readonly quantumengine_computeFidelity: (a: number, b: any, c: any) => [number, number, number];
    readonly quantumengine_computeTopologicalInvariants: (a: number, b: any) => [number, number, number];
    readonly quantumengine_createGHZState: (a: number, b: number) => [number, number, number];
    readonly quantumengine_createWState: (a: number, b: number) => [number, number, number];
    readonly spectralengine_algebraicConnectivity: (a: number, b: any) => [number, number, number];
    readonly spectralengine_computeCheegerBounds: (a: number, b: any) => [number, number, number];
    readonly spectralengine_computeEigenvalues: (a: number, b: any) => [number, number, number];
    readonly spectralengine_computeFiedlerVector: (a: number, b: any) => [number, number, number];
    readonly spectralengine_computeSpectralGap: (a: number, b: any) => [number, number, number];
    readonly spectralengine_new: () => number;
    readonly spectralengine_predictMinCut: (a: number, b: any) => [number, number, number];
    readonly spectralengine_withConfig: (a: number, b: number, c: number) => number;
    readonly start: () => void;
    readonly wasmerror_code: (a: number) => [number, number];
    readonly wasmerror_message: (a: number) => [number, number];
    readonly causalengine_new: () => number;
    readonly cohomologyengine_new: () => number;
    readonly quantumengine_new: () => number;
    readonly __wbg_causalengine_free: (a: number, b: number) => void;
    readonly __wbg_cohomologyengine_free: (a: number, b: number) => void;
    readonly __wbg_quantumengine_free: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
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
