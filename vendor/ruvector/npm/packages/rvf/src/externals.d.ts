/**
 * Ambient module declarations for optional native/WASM backends.
 *
 * These let the SDK compile without the actual native packages installed.
 * At runtime the dynamic `import()` calls in backend.ts will resolve to the
 * real implementations (or throw, which is handled gracefully).
 */

declare module '@ruvector/rvf-node' {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export const RvfDatabase: any;
}

declare module '@ruvector/rvf-wasm' {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  const init: (...args: any[]) => Promise<any>;
  export default init;
}

declare module '@ruvector/rvf-solver' {
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  export const RvfSolver: any;
  export type TrainOptions = any;
  export type TrainResult = any;
  export type AcceptanceOptions = any;
  export type AcceptanceManifest = any;
  export type AcceptanceModeResult = any;
  export type CycleMetrics = any;
  export type PolicyState = any;
  export type SkipMode = any;
  export type SkipModeStats = any;
  export type CompiledConfig = any;
}
