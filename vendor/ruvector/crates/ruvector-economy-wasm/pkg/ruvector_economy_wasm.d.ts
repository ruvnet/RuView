/* tslint:disable */
/* eslint-disable */

export class CreditLedger {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get the state root (Merkle root of ledger state)
   */
  stateRoot(): Uint8Array;
  /**
   * Get event count
   */
  eventCount(): number;
  /**
   * Get total credits spent
   */
  totalSpent(): bigint;
  /**
   * Export spent counter for P2P sync
   */
  exportSpent(): Uint8Array;
  /**
   * Get total credits ever earned (before spending)
   */
  totalEarned(): bigint;
  /**
   * Export earned counter for P2P sync
   */
  exportEarned(): Uint8Array;
  /**
   * Get staked amount
   */
  stakedAmount(): bigint;
  /**
   * Get state root as hex string
   */
  stateRootHex(): string;
  /**
   * Get network compute hours
   */
  networkCompute(): number;
  /**
   * Verify state root matches current state
   */
  verifyStateRoot(expected_root: Uint8Array): boolean;
  /**
   * Get current contribution multiplier
   */
  currentMultiplier(): number;
  /**
   * Credit with multiplier applied (for task rewards)
   */
  creditWithMultiplier(base_amount: bigint, reason: string): string;
  /**
   * Update network compute hours (from P2P sync)
   */
  updateNetworkCompute(hours: number): void;
  /**
   * Create a new credit ledger for a node
   */
  constructor(node_id: string);
  /**
   * Merge with another ledger (CRDT merge operation)
   *
   * This is the core CRDT operation - associative, commutative, and idempotent.
   * Safe to apply in any order with any number of concurrent updates.
   */
  merge(other_earned: Uint8Array, other_spent: Uint8Array): number;
  /**
   * Slash staked credits (penalty for bad behavior)
   *
   * Returns the actual amount slashed (may be less if stake is insufficient)
   */
  slash(amount: bigint): bigint;
  /**
   * Stake credits for participation
   */
  stake(amount: bigint): void;
  /**
   * Credit the ledger (earn credits)
   *
   * This updates the G-Counter which is monotonically increasing.
   * Safe for concurrent P2P updates.
   */
  credit(amount: bigint, _reason: string): string;
  /**
   * Deduct from the ledger (spend credits)
   *
   * This updates the PN-Counter positive side.
   * Spending can be disputed/refunded by updating the negative side.
   */
  deduct(amount: bigint): string;
  /**
   * Refund a previous deduction (dispute resolution)
   *
   * This updates the PN-Counter negative side for the given event.
   */
  refund(event_id: string, amount: bigint): void;
  /**
   * Get current available balance (earned - spent - staked)
   */
  balance(): bigint;
  /**
   * Get the node ID
   */
  nodeId(): string;
  /**
   * Unstake credits
   */
  unstake(amount: bigint): void;
}

export class ReputationScore {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Get total tasks
   */
  totalTasks(): bigint;
  /**
   * Calculate stake weight using logarithmic scaling
   *
   * Uses log10(stake + 1) / 6 capped at 1.0
   * This means:
   * - 0 stake = 0.0 weight
   * - 100 stake = ~0.33 weight
   * - 10,000 stake = ~0.67 weight
   * - 1,000,000 stake = 1.0 weight (capped)
   */
  stakeWeight(): number;
  /**
   * Get tasks failed
   */
  tasksFailed(): bigint;
  /**
   * Update stake amount
   */
  updateStake(new_stake: bigint): void;
  /**
   * Check if node meets minimum reputation for participation
   */
  meetsMinimum(min_accuracy: number, min_uptime: number, min_stake: bigint): boolean;
  /**
   * Update uptime tracking
   */
  updateUptime(online_seconds: bigint, total_seconds: bigint): void;
  /**
   * Check if this reputation is better than another
   */
  isBetterThan(other: ReputationScore): boolean;
  /**
   * Record a failed/disputed task
   */
  recordFailure(): void;
  /**
   * Record a successful task completion
   */
  recordSuccess(): void;
  /**
   * Calculate composite reputation score
   *
   * Formula: accuracy^2 * uptime * stake_weight
   *
   * Returns a value between 0.0 and 1.0
   */
  compositeScore(): number;
  /**
   * Get tasks completed
   */
  tasksCompleted(): bigint;
  /**
   * Create with detailed tracking
   */
  static newWithTracking(tasks_completed: bigint, tasks_failed: bigint, uptime_seconds: bigint, total_seconds: bigint, stake: bigint): ReputationScore;
  /**
   * Create a new reputation score
   */
  constructor(accuracy: number, uptime: number, stake: bigint);
  /**
   * Serialize to JSON
   */
  toJson(): string;
  /**
   * Deserialize from JSON
   */
  static fromJson(json: string): ReputationScore;
  /**
   * Get reputation tier based on composite score
   */
  tierName(): string;
  /**
   * Get stake amount
   */
  readonly stake: bigint;
  /**
   * Get uptime score (0.0 - 1.0)
   */
  readonly uptime: number;
  /**
   * Get accuracy score (0.0 - 1.0)
   */
  readonly accuracy: number;
}

/**
 * Reasons for slashing stake
 */
export enum SlashReason {
  /**
   * Invalid task result
   */
  InvalidResult = 0,
  /**
   * Double-spending attempt
   */
  DoubleSpend = 1,
  /**
   * Sybil attack detected
   */
  SybilAttack = 2,
  /**
   * Excessive downtime
   */
  Downtime = 3,
  /**
   * Spam/flooding
   */
  Spam = 4,
  /**
   * Malicious behavior
   */
  Malicious = 5,
}

export class StakeManager {
  free(): void;
  [Symbol.dispose](): void;
  /**
   * Undelegate stake
   */
  undelegate(from_node: string, to_node: string, amount: bigint): void;
  /**
   * Export stake data as JSON
   */
  exportJson(): string;
  /**
   * Get number of stakers
   */
  stakerCount(): number;
  /**
   * Get total network staked
   */
  totalStaked(): bigint;
  /**
   * Check if node meets minimum stake
   */
  meetsMinimum(node_id: string): boolean;
  /**
   * Get total slashed
   */
  totalSlashed(): bigint;
  /**
   * Get slash count for a node
   */
  getSlashCount(node_id: string): number;
  /**
   * Create with custom parameters
   */
  static newWithParams(min_stake: bigint, lock_period_ms: bigint): StakeManager;
  /**
   * Get lock timestamp for a node
   */
  getLockTimestamp(node_id: string): bigint;
  /**
   * Get delegator count
   */
  getDelegatorCount(node_id: string): number;
  /**
   * Get effective stake (own + delegated)
   */
  getEffectiveStake(node_id: string): bigint;
  /**
   * Get total amount slashed from a node
   */
  getNodeTotalSlashed(node_id: string): bigint;
  /**
   * Create a new stake manager
   */
  constructor();
  /**
   * Slash stake for bad behavior
   */
  slash(node_id: string, reason: SlashReason, evidence: string): bigint;
  /**
   * Stake credits for a node
   */
  stake(node_id: string, amount: bigint): void;
  /**
   * Unstake credits (if lock period has passed)
   */
  unstake(node_id: string, amount: bigint): bigint;
  /**
   * Delegate stake to another node
   */
  delegate(from_node: string, to_node: string, amount: bigint): void;
  /**
   * Get stake for a node
   */
  getStake(node_id: string): bigint;
  /**
   * Check if stake is locked
   */
  isLocked(node_id: string): boolean;
  /**
   * Get minimum stake requirement
   */
  minStake(): bigint;
}

/**
 * Calculate reward with multiplier (WASM export)
 */
export function calculate_reward(base_reward: bigint, network_compute_hours: number): bigint;

/**
 * Calculate composite reputation score (WASM export)
 */
export function composite_reputation(accuracy: number, uptime: number, stake: bigint): number;

/**
 * Calculate contribution multiplier (WASM export)
 *
 * Returns the reward multiplier based on total network compute hours.
 * Early adopters get up to 10x rewards, decaying to 1x as network grows.
 */
export function contribution_multiplier(network_compute_hours: number): number;

/**
 * Get tier name based on compute level (WASM export)
 */
export function get_tier_name(network_compute_hours: number): string;

/**
 * Get tier information as JSON (WASM export)
 */
export function get_tiers_json(): string;

/**
 * Initialize panic hook for better error messages in console
 */
export function init_panic_hook(): void;

/**
 * Calculate stake weight (WASM export)
 */
export function stake_weight(stake: bigint): number;

/**
 * Get the current version of the economy module
 */
export function version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly __wbg_creditledger_free: (a: number, b: number) => void;
  readonly __wbg_reputationscore_free: (a: number, b: number) => void;
  readonly __wbg_stakemanager_free: (a: number, b: number) => void;
  readonly calculate_reward: (a: bigint, b: number) => bigint;
  readonly composite_reputation: (a: number, b: number, c: bigint) => number;
  readonly contribution_multiplier: (a: number) => number;
  readonly creditledger_balance: (a: number) => bigint;
  readonly creditledger_credit: (a: number, b: number, c: bigint, d: number, e: number) => void;
  readonly creditledger_creditWithMultiplier: (a: number, b: number, c: bigint, d: number, e: number) => void;
  readonly creditledger_currentMultiplier: (a: number) => number;
  readonly creditledger_deduct: (a: number, b: number, c: bigint) => void;
  readonly creditledger_eventCount: (a: number) => number;
  readonly creditledger_exportEarned: (a: number, b: number) => void;
  readonly creditledger_exportSpent: (a: number, b: number) => void;
  readonly creditledger_merge: (a: number, b: number, c: number, d: number, e: number, f: number) => void;
  readonly creditledger_networkCompute: (a: number) => number;
  readonly creditledger_new: (a: number, b: number, c: number) => void;
  readonly creditledger_nodeId: (a: number, b: number) => void;
  readonly creditledger_refund: (a: number, b: number, c: number, d: number, e: bigint) => void;
  readonly creditledger_slash: (a: number, b: number, c: bigint) => void;
  readonly creditledger_stake: (a: number, b: number, c: bigint) => void;
  readonly creditledger_stakedAmount: (a: number) => bigint;
  readonly creditledger_stateRoot: (a: number, b: number) => void;
  readonly creditledger_stateRootHex: (a: number, b: number) => void;
  readonly creditledger_totalEarned: (a: number) => bigint;
  readonly creditledger_totalSpent: (a: number) => bigint;
  readonly creditledger_unstake: (a: number, b: number, c: bigint) => void;
  readonly creditledger_updateNetworkCompute: (a: number, b: number) => void;
  readonly creditledger_verifyStateRoot: (a: number, b: number, c: number) => number;
  readonly get_tier_name: (a: number, b: number) => void;
  readonly get_tiers_json: (a: number) => void;
  readonly reputationscore_accuracy: (a: number) => number;
  readonly reputationscore_compositeScore: (a: number) => number;
  readonly reputationscore_fromJson: (a: number, b: number, c: number) => void;
  readonly reputationscore_isBetterThan: (a: number, b: number) => number;
  readonly reputationscore_meetsMinimum: (a: number, b: number, c: number, d: bigint) => number;
  readonly reputationscore_new: (a: number, b: number, c: bigint) => number;
  readonly reputationscore_newWithTracking: (a: bigint, b: bigint, c: bigint, d: bigint, e: bigint) => number;
  readonly reputationscore_recordFailure: (a: number) => void;
  readonly reputationscore_recordSuccess: (a: number) => void;
  readonly reputationscore_stake: (a: number) => bigint;
  readonly reputationscore_stakeWeight: (a: number) => number;
  readonly reputationscore_tasksCompleted: (a: number) => bigint;
  readonly reputationscore_tasksFailed: (a: number) => bigint;
  readonly reputationscore_tierName: (a: number, b: number) => void;
  readonly reputationscore_toJson: (a: number, b: number) => void;
  readonly reputationscore_totalTasks: (a: number) => bigint;
  readonly reputationscore_updateStake: (a: number, b: bigint) => void;
  readonly reputationscore_updateUptime: (a: number, b: bigint, c: bigint) => void;
  readonly reputationscore_uptime: (a: number) => number;
  readonly stake_weight: (a: bigint) => number;
  readonly stakemanager_delegate: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
  readonly stakemanager_exportJson: (a: number, b: number) => void;
  readonly stakemanager_getDelegatorCount: (a: number, b: number, c: number) => number;
  readonly stakemanager_getEffectiveStake: (a: number, b: number, c: number) => bigint;
  readonly stakemanager_getLockTimestamp: (a: number, b: number, c: number) => bigint;
  readonly stakemanager_getNodeTotalSlashed: (a: number, b: number, c: number) => bigint;
  readonly stakemanager_getSlashCount: (a: number, b: number, c: number) => number;
  readonly stakemanager_getStake: (a: number, b: number, c: number) => bigint;
  readonly stakemanager_isLocked: (a: number, b: number, c: number) => number;
  readonly stakemanager_meetsMinimum: (a: number, b: number, c: number) => number;
  readonly stakemanager_new: () => number;
  readonly stakemanager_newWithParams: (a: bigint, b: bigint) => number;
  readonly stakemanager_slash: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly stakemanager_stake: (a: number, b: number, c: number, d: number, e: bigint) => void;
  readonly stakemanager_stakerCount: (a: number) => number;
  readonly stakemanager_totalSlashed: (a: number) => bigint;
  readonly stakemanager_totalStaked: (a: number) => bigint;
  readonly stakemanager_undelegate: (a: number, b: number, c: number, d: number, e: number, f: number, g: bigint) => void;
  readonly stakemanager_unstake: (a: number, b: number, c: number, d: number, e: bigint) => void;
  readonly version: (a: number) => void;
  readonly init_panic_hook: () => void;
  readonly stakemanager_minStake: (a: number) => bigint;
  readonly __wbindgen_export: (a: number, b: number, c: number) => void;
  readonly __wbindgen_export2: (a: number, b: number) => number;
  readonly __wbindgen_export3: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
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
