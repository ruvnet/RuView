/* tslint:disable */
/* eslint-disable */

export class ExoticEcosystem {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get current cell count (from morphogenetic network)
   */
  cellCount(): number;
  /**
   * Crystallize the time crystal
   */
  crystallize(): void;
  /**
   * Get current step
   */
  currentStep(): number;
  /**
   * Get current member count (from NAO)
   */
  memberCount(): number;
  /**
   * Get ecosystem summary as JSON
   */
  summaryJson(): any;
  /**
   * Get current synchronization level (from time crystal)
   */
  synchronization(): number;
  /**
   * Create a new exotic ecosystem with interconnected mechanisms
   */
  constructor(agents: number, grid_size: number, oscillators: number);
  /**
   * Advance all systems by one step
   */
  step(): void;
  /**
   * Vote on a proposal
   */
  vote(proposal_id: string, agent_id: string, weight: number): boolean;
  /**
   * Execute a proposal
   */
  execute(proposal_id: string): boolean;
  /**
   * Propose an action in the NAO
   */
  propose(action: string): string;
}

export class WasmMorphogeneticNetwork {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get cell count
   */
  cellCount(): number;
  /**
   * Get all cells as JSON
   */
  cellsJson(): any;
  /**
   * Get statistics as JSON
   */
  statsJson(): any;
  /**
   * Get stem cell count
   */
  stemCount(): number;
  /**
   * Get current tick
   */
  currentTick(): number;
  /**
   * Get compute cell count
   */
  computeCount(): number;
  /**
   * Differentiate stem cells
   */
  differentiate(): void;
  /**
   * Seed a signaling cell at position
   */
  seedSignaling(x: number, y: number): number;
  /**
   * Get signaling cell count
   */
  signalingCount(): number;
  /**
   * Add a growth factor source
   */
  addGrowthSource(x: number, y: number, name: string, concentration: number): void;
  /**
   * Create a new morphogenetic network
   */
  constructor(width: number, height: number);
  /**
   * Grow the network
   */
  grow(dt: number): void;
  /**
   * Prune weak connections and dead cells
   */
  prune(threshold: number): void;
  /**
   * Seed a stem cell at position
   */
  seedStem(x: number, y: number): number;
}

export class WasmNAO {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Add a member agent with initial stake
   */
  addMember(agent_id: string, stake: number): void;
  /**
   * Get current tick
   */
  currentTick(): number;
  /**
   * Get member count
   */
  memberCount(): number;
  /**
   * Remove a member agent
   */
  removeMember(agent_id: string): void;
  /**
   * Get coherence between two agents (0-1)
   */
  agentCoherence(agent_a: string, agent_b: string): number;
  /**
   * Get current synchronization level (0-1)
   */
  synchronization(): number;
  /**
   * Get total voting power
   */
  totalVotingPower(): number;
  /**
   * Get active proposal count
   */
  activeProposalCount(): number;
  /**
   * Create a new NAO with the given quorum threshold (0.0 - 1.0)
   */
  constructor(quorum_threshold: number);
  /**
   * Advance simulation by one tick
   */
  tick(dt: number): void;
  /**
   * Vote on a proposal
   */
  vote(proposal_id: string, agent_id: string, weight: number): boolean;
  /**
   * Execute a proposal if consensus reached
   */
  execute(proposal_id: string): boolean;
  /**
   * Create a new proposal, returns proposal ID
   */
  propose(action: string): string;
  /**
   * Get all data as JSON
   */
  toJson(): any;
}

export class WasmTimeCrystal {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get robustness measure
   */
  robustness(): number;
  /**
   * Crystallize to establish periodic order
   */
  crystallize(): void;
  /**
   * Get phases as JSON array
   */
  phasesJson(): any;
  /**
   * Set driving strength
   */
  setDriving(strength: number): void;
  /**
   * Get current step
   */
  currentStep(): number;
  /**
   * Get current pattern type as string
   */
  patternType(): string;
  /**
   * Set coupling strength
   */
  setCoupling(coupling: number): void;
  /**
   * Set disorder level
   */
  setDisorder(disorder: number): void;
  /**
   * Get signals as JSON array
   */
  signalsJson(): any;
  /**
   * Create a synchronized crystal
   */
  static synchronized(n: number, period_ms: number): WasmTimeCrystal;
  /**
   * Get collective spin
   */
  collectiveSpin(): number;
  /**
   * Check if crystallized
   */
  isCrystallized(): boolean;
  /**
   * Get order parameter (synchronization level)
   */
  orderParameter(): number;
  /**
   * Get number of oscillators
   */
  oscillatorCount(): number;
  /**
   * Create a new time crystal with n oscillators
   */
  constructor(n: number, period_ms: number);
  /**
   * Advance one tick, returns coordination pattern as Uint8Array
   */
  tick(): Uint8Array;
  /**
   * Apply perturbation
   */
  perturb(strength: number): void;
  /**
   * Get period in milliseconds
   */
  periodMs(): number;
}

/**
 * Get information about available exotic mechanisms
 */
export function available_mechanisms(): any;

/**
 * Initialize the WASM module with panic hook
 */
export function init(): void;

/**
 * Get the version of the ruvector-exotic-wasm crate
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_exoticecosystem_free: (a: number, b: number) => void;
  readonly __wbg_wasmmorphogeneticnetwork_free: (a: number, b: number) => void;
  readonly __wbg_wasmnao_free: (a: number, b: number) => void;
  readonly __wbg_wasmtimecrystal_free: (a: number, b: number) => void;
  readonly available_mechanisms: () => number;
  readonly exoticecosystem_cellCount: (a: number) => number;
  readonly exoticecosystem_crystallize: (a: number) => void;
  readonly exoticecosystem_currentStep: (a: number) => number;
  readonly exoticecosystem_execute: (a: number, b: number, c: number) => number;
  readonly exoticecosystem_memberCount: (a: number) => number;
  readonly exoticecosystem_new: (a: number, b: number, c: number) => number;
  readonly exoticecosystem_propose: (a: number, b: number, c: number, d: number) => void;
  readonly exoticecosystem_step: (a: number) => void;
  readonly exoticecosystem_summaryJson: (a: number, b: number) => void;
  readonly exoticecosystem_synchronization: (a: number) => number;
  readonly exoticecosystem_vote: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
  readonly init: () => void;
  readonly version: (a: number) => void;
  readonly wasmmorphogeneticnetwork_addGrowthSource: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly wasmmorphogeneticnetwork_cellCount: (a: number) => number;
  readonly wasmmorphogeneticnetwork_cellsJson: (a: number, b: number) => void;
  readonly wasmmorphogeneticnetwork_computeCount: (a: number) => number;
  readonly wasmmorphogeneticnetwork_currentTick: (a: number) => number;
  readonly wasmmorphogeneticnetwork_differentiate: (a: number) => void;
  readonly wasmmorphogeneticnetwork_grow: (a: number, b: number) => void;
  readonly wasmmorphogeneticnetwork_new: (a: number, b: number) => number;
  readonly wasmmorphogeneticnetwork_prune: (a: number, b: number) => void;
  readonly wasmmorphogeneticnetwork_seedSignaling: (a: number, b: number, c: number) => number;
  readonly wasmmorphogeneticnetwork_seedStem: (a: number, b: number, c: number) => number;
  readonly wasmmorphogeneticnetwork_signalingCount: (a: number) => number;
  readonly wasmmorphogeneticnetwork_statsJson: (a: number, b: number) => void;
  readonly wasmmorphogeneticnetwork_stemCount: (a: number) => number;
  readonly wasmnao_activeProposalCount: (a: number) => number;
  readonly wasmnao_addMember: (a: number, b: number, c: number, d: number) => void;
  readonly wasmnao_agentCoherence: (a: number, b: number, c: number, d: number, e: number) => number;
  readonly wasmnao_currentTick: (a: number) => number;
  readonly wasmnao_execute: (a: number, b: number, c: number) => number;
  readonly wasmnao_memberCount: (a: number) => number;
  readonly wasmnao_new: (a: number) => number;
  readonly wasmnao_propose: (a: number, b: number, c: number, d: number) => void;
  readonly wasmnao_removeMember: (a: number, b: number, c: number) => void;
  readonly wasmnao_synchronization: (a: number) => number;
  readonly wasmnao_tick: (a: number, b: number) => void;
  readonly wasmnao_toJson: (a: number, b: number) => void;
  readonly wasmnao_totalVotingPower: (a: number) => number;
  readonly wasmnao_vote: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
  readonly wasmtimecrystal_collectiveSpin: (a: number) => number;
  readonly wasmtimecrystal_crystallize: (a: number) => void;
  readonly wasmtimecrystal_currentStep: (a: number) => number;
  readonly wasmtimecrystal_isCrystallized: (a: number) => number;
  readonly wasmtimecrystal_new: (a: number, b: number) => number;
  readonly wasmtimecrystal_oscillatorCount: (a: number) => number;
  readonly wasmtimecrystal_patternType: (a: number, b: number) => void;
  readonly wasmtimecrystal_periodMs: (a: number) => number;
  readonly wasmtimecrystal_perturb: (a: number, b: number) => void;
  readonly wasmtimecrystal_phasesJson: (a: number, b: number) => void;
  readonly wasmtimecrystal_robustness: (a: number) => number;
  readonly wasmtimecrystal_setCoupling: (a: number, b: number) => void;
  readonly wasmtimecrystal_setDisorder: (a: number, b: number) => void;
  readonly wasmtimecrystal_setDriving: (a: number, b: number) => void;
  readonly wasmtimecrystal_signalsJson: (a: number, b: number) => void;
  readonly wasmtimecrystal_synchronized: (a: number, b: number) => number;
  readonly wasmtimecrystal_tick: (a: number, b: number) => void;
  readonly wasmtimecrystal_orderParameter: (a: number) => number;
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
