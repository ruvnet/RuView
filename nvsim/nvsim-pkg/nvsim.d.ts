/* tslint:disable */
/* eslint-disable */

/**
 * In-browser pipeline. Wraps [`Pipeline`] with JS-friendly construction
 * (JSON for `Scene` and `PipelineConfig`) and `Vec<u8>` outputs (raw
 * concatenated [`MagFrame`] bytes — 60 bytes/frame, magic `0xC51A_6E70`).
 */
export class WasmPipeline {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * nvsim build version (semver from Cargo.toml).
     */
    static buildVersion(): string;
    /**
     * Bytes-per-frame for v1 — `60` today; surfaced so the dashboard
     * can advance its parse cursor without re-deriving the layout.
     */
    static frameBytes(): number;
    /**
     * Magic constant for the `MagFrame` v1 binary record. The dashboard's
     * hex-dump panel highlights these four bytes (`0xC51A_6E70` → `701A6EC5`
     * little-endian) as a sanity check.
     */
    static frameMagic(): number;
    /**
     * Construct from JSON strings + a `seed` (BigInt-friendly; passed in
     * as `f64` since wasm-bindgen does not yet ergonomically pass `u64`,
     * then bit-cast through `as u64`). The dashboard sends seeds as
     * `Number(seed_hex)` from a 32-bit value to fit cleanly.
     */
    constructor(scene_json: string, config_json: string, seed: number);
    /**
     * Run `n_samples` of the pipeline and return the concatenated raw
     * `MagFrame` bytes (`n_samples * sensors * 60` bytes). The dashboard
     * parses this into typed records on the main thread.
     */
    run(n_samples: number): Uint8Array;
    /**
     * Run + SHA-256 witness in one call. Returns a JS object
     * `{ frames: Uint8Array, witness: Uint8Array }`. Same
     * `(scene, config, seed)` produces byte-identical `witness` across
     * runs, machines, and transports — the regression dashboard pins.
     */
    runWithWitness(n_samples: number): any;
}

/**
 * Expected reference witness for `Proof::REFERENCE_SCENE_JSON @ seed=42,
 * N=256` — the bytes the dashboard's Verify panel compares against.
 */
export function expectedReferenceWitnessHex(): string;

/**
 * Hex-encode a 32-byte witness for display.
 */
export function hexWitness(witness: Uint8Array): string;

/**
 * Convenience: parse the bundled reference scene to JSON. Lets the
 * dashboard's "load reference scene" flow round-trip through the Rust
 * type system instead of duplicating the JSON literal in the JS code.
 */
export function referenceSceneJson(): string;

/**
 * Run the canonical reference pipeline (`Proof::generate`) end-to-end and
 * return the SHA-256 witness as a 32-byte `Uint8Array`. This is the
 * dashboard's source of truth for the Verify-witness panel.
 */
export function referenceWitness(): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmpipeline_free: (a: number, b: number) => void;
    readonly expectedReferenceWitnessHex: (a: number) => void;
    readonly hexWitness: (a: number, b: number, c: number) => void;
    readonly referenceSceneJson: (a: number) => void;
    readonly referenceWitness: (a: number) => void;
    readonly wasmpipeline_buildVersion: (a: number) => void;
    readonly wasmpipeline_frameBytes: () => number;
    readonly wasmpipeline_frameMagic: () => number;
    readonly wasmpipeline_new: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
    readonly wasmpipeline_run: (a: number, b: number, c: number) => void;
    readonly wasmpipeline_runWithWitness: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export: (a: number) => void;
    readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
    readonly __wbindgen_export2: (a: number, b: number, c: number) => void;
    readonly __wbindgen_export3: (a: number, b: number) => number;
    readonly __wbindgen_export4: (a: number, b: number, c: number, d: number) => number;
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
