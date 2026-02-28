# ruvector-economy-wasm

CRDT-based autonomous credit economy for distributed compute networks. Designed for WASM execution with P2P consistency guarantees.

## Installation

```bash
npm install ruvector-economy-wasm
```

## Quick Start

```javascript
import init, {
  CreditLedger,
  ReputationScore,
  StakeManager,
  contribution_multiplier,
  calculate_reward,
  get_tier_name
} from 'ruvector-economy-wasm';

// Initialize the WASM module
await init();

// Create a credit ledger for a node
const ledger = new CreditLedger("node-123");

// Earn credits
ledger.credit(100n, "task:abc");
console.log(`Balance: ${ledger.balance()}`);

// Check early adopter multiplier
const mult = contribution_multiplier(50000.0);
console.log(`Multiplier: ${mult}x`);  // ~9.5x for early network

// Track reputation
const rep = new ReputationScore(0.95, 0.98, 1000n);
console.log(`Composite score: ${rep.compositeScore()}`);
```

## Architecture

```
+------------------------+
|     CreditLedger       |  <-- CRDT-based P2P-safe ledger
|  +------------------+  |
|  | G-Counter: Earned|  |  <-- Monotonically increasing
|  | PN-Counter: Spent|  |  <-- Supports dispute resolution
|  | Stake: Locked    |  |  <-- Participation requirement
|  | State Root       |  |  <-- Merkle root for verification
|  +------------------+  |
+------------------------+
          |
          v
+------------------------+
|  ContributionCurve     |  <-- Exponential decay: 10x -> 1x
+------------------------+
          |
          v
+------------------------+
|   ReputationScore      |  <-- accuracy * uptime * stake_weight
+------------------------+
          |
          v
+------------------------+
|   StakeManager         |  <-- Delegation, slashing, lock periods
+------------------------+
```

## API Reference

### CreditLedger

The core CRDT ledger for tracking credits earned, spent, and staked.

```typescript
class CreditLedger {
  // Constructor
  constructor(node_id: string);

  // Balance operations
  balance(): bigint;           // Current available balance
  totalEarned(): bigint;       // Total credits ever earned
  totalSpent(): bigint;        // Total credits spent (net of refunds)
  stakedAmount(): bigint;      // Currently staked amount

  // Credit operations
  credit(amount: bigint, reason: string): string;  // Returns event_id
  creditWithMultiplier(base_amount: bigint, reason: string): string;
  deduct(amount: bigint): string;  // Returns event_id
  refund(event_id: string, amount: bigint): void;

  // Staking
  stake(amount: bigint): void;
  unstake(amount: bigint): void;
  slash(amount: bigint): bigint;  // Returns amount actually slashed

  // Early adopter multiplier
  currentMultiplier(): number;
  networkCompute(): number;
  updateNetworkCompute(hours: number): void;

  // State verification
  stateRoot(): Uint8Array;
  stateRootHex(): string;
  verifyStateRoot(expected_root: Uint8Array): boolean;

  // P2P sync (CRDT merge)
  merge(other_earned: Uint8Array, other_spent: Uint8Array): number;
  exportEarned(): Uint8Array;
  exportSpent(): Uint8Array;

  // Utilities
  nodeId(): string;
  eventCount(): number;
  free(): void;  // Release WASM memory
}
```

#### Example: CRDT Merge for P2P Sync

```javascript
// Node A creates ledger
const ledgerA = new CreditLedger("node-A");
ledgerA.credit(100n, "task:1");
ledgerA.credit(50n, "task:2");

// Node B creates ledger
const ledgerB = new CreditLedger("node-B");
ledgerB.credit(75n, "task:3");

// Export state for sync
const earnedA = ledgerA.exportEarned();
const spentA = ledgerA.exportSpent();

// Merge on node B (CRDT: associative, commutative, idempotent)
const mergedCount = ledgerB.merge(earnedA, spentA);
console.log(`Merged ${mergedCount} entries`);
```

### ContributionCurve (via standalone functions)

Early adopter reward multiplier with exponential decay.

```typescript
// Get multiplier for network compute level
function contribution_multiplier(network_compute_hours: number): number;

// Calculate reward with multiplier applied
function calculate_reward(base_reward: bigint, network_compute_hours: number): bigint;

// Get tier name for UI display
function get_tier_name(network_compute_hours: number): string;

// Get all tier thresholds as JSON
function get_tiers_json(): string;
```

#### Multiplier Curve

```
Multiplier
10x |*
    | *
 8x |  *
    |   *
 6x |    *
    |     *
 4x |      *
    |       **
 2x |         ***
    |            *****
 1x |                 ****************************
    +--+--+--+--+--+--+--+--+--+--+--+--+--+--+---> Network Compute (M hours)
    0  1  2  3  4  5  6  7  8  9  10
```

#### Tier Reference

| Tier | Network Compute | Multiplier |
|------|-----------------|------------|
| Genesis | 0 - 100K hours | ~10x |
| Pioneer | 100K - 500K hours | ~9x - 6x |
| Early Adopter | 500K - 1M hours | ~6x - 4x |
| Established | 1M - 5M hours | ~4x - 1.5x |
| Baseline | 5M+ hours | ~1x |

#### Example: Early Adopter Rewards

```javascript
// Genesis contributor (first on network)
const genesisMultiplier = contribution_multiplier(0);
console.log(genesisMultiplier);  // 10.0

// Task completion reward
const baseReward = 100n;
const actualReward = calculate_reward(baseReward, 50000.0);
console.log(actualReward);  // ~950 (9.5x for early network)

// Display tier to user
const tier = get_tier_name(500000.0);
console.log(tier);  // "Early Adopter"
```

### ReputationScore

Multi-factor reputation scoring for node quality assessment.

```typescript
class ReputationScore {
  // Constructors
  constructor(accuracy: number, uptime: number, stake: bigint);
  static newWithTracking(
    tasks_completed: bigint,
    tasks_failed: bigint,
    uptime_seconds: bigint,
    total_seconds: bigint,
    stake: bigint
  ): ReputationScore;

  // Core scores
  readonly accuracy: number;   // 0.0 - 1.0
  readonly uptime: number;     // 0.0 - 1.0
  readonly stake: bigint;

  // Calculated scores
  compositeScore(): number;    // accuracy^2 * uptime * stake_weight
  stakeWeight(): number;       // log10(stake + 1) / 6, capped at 1.0
  tierName(): string;          // "Elite", "Reliable", "Standard", "Novice"

  // Task tracking
  recordSuccess(): void;
  recordFailure(): void;
  tasksCompleted(): bigint;
  tasksFailed(): bigint;
  totalTasks(): bigint;

  // Uptime tracking
  updateUptime(online_seconds: bigint, total_seconds: bigint): void;

  // Stake management
  updateStake(new_stake: bigint): void;

  // Comparisons
  isBetterThan(other: ReputationScore): boolean;
  meetsMinimum(min_accuracy: number, min_uptime: number, min_stake: bigint): boolean;

  // Serialization
  toJson(): string;
  static fromJson(json: string): ReputationScore;

  free(): void;
}
```

#### Composite Score Formula

```
composite_score = accuracy^2 * uptime * stake_weight
```

Where:
- `accuracy` = tasks_completed / total_tasks
- `uptime` = online_seconds / total_seconds
- `stake_weight` = min(1.0, log10(stake + 1) / 6)

#### Example: Reputation Tracking

```javascript
// Create with detailed tracking
const rep = ReputationScore.newWithTracking(
  95n,    // tasks completed
  5n,     // tasks failed
  86400n, // uptime seconds (24 hours)
  90000n, // total seconds (25 hours)
  10000n  // stake amount
);

console.log(`Accuracy: ${rep.accuracy}`);           // 0.95
console.log(`Uptime: ${rep.uptime}`);               // 0.96
console.log(`Stake Weight: ${rep.stakeWeight()}`);  // ~0.67
console.log(`Composite: ${rep.compositeScore()}`);  // ~0.58
console.log(`Tier: ${rep.tierName()}`);             // "Reliable"

// Track ongoing performance
rep.recordSuccess();
rep.recordSuccess();
rep.recordFailure();
console.log(`New accuracy: ${rep.tasksCompleted()} / ${rep.totalTasks()}`);

// Check if meets minimum requirements
const eligible = rep.meetsMinimum(0.9, 0.95, 1000n);
console.log(`Eligible for premium tasks: ${eligible}`);
```

### StakeManager

Network-wide stake management with delegation and slashing.

```typescript
class StakeManager {
  // Constructors
  constructor();
  static newWithParams(min_stake: bigint, lock_period_ms: bigint): StakeManager;

  // Staking
  stake(node_id: string, amount: bigint): void;
  unstake(node_id: string, amount: bigint): bigint;  // Returns actual unstaked
  getStake(node_id: string): bigint;

  // Delegation
  delegate(from_node: string, to_node: string, amount: bigint): void;
  undelegate(from_node: string, to_node: string, amount: bigint): void;
  getEffectiveStake(node_id: string): bigint;  // own + delegated
  getDelegatorCount(node_id: string): number;

  // Slashing
  slash(node_id: string, reason: SlashReason, evidence: string): bigint;
  getSlashCount(node_id: string): number;
  getNodeTotalSlashed(node_id: string): bigint;

  // Lock management
  isLocked(node_id: string): boolean;
  getLockTimestamp(node_id: string): bigint;

  // Network stats
  totalStaked(): bigint;
  totalSlashed(): bigint;
  stakerCount(): number;
  minStake(): bigint;
  meetsMinimum(node_id: string): boolean;

  // Export
  exportJson(): string;

  free(): void;
}

enum SlashReason {
  InvalidResult = 0,
  DoubleSpend = 1,
  SybilAttack = 2,
  Downtime = 3,
  Spam = 4,
  Malicious = 5
}
```

#### Example: Stake Delegation

```javascript
const manager = StakeManager.newWithParams(100n, 86400000n);  // 100 min, 24h lock

// Nodes stake
manager.stake("validator-1", 10000n);
manager.stake("delegator-1", 500n);

// Delegator delegates to validator
manager.delegate("delegator-1", "validator-1", 500n);

// Check effective stake
const effective = manager.getEffectiveStake("validator-1");
console.log(`Validator effective stake: ${effective}`);  // 10500

// Slash for bad behavior
const slashed = manager.slash("validator-1", SlashReason.InvalidResult, "proof:xyz");
console.log(`Slashed: ${slashed}`);
```

## Standalone Functions

```typescript
// Contribution curve
function contribution_multiplier(network_compute_hours: number): number;
function calculate_reward(base_reward: bigint, network_compute_hours: number): bigint;
function get_tier_name(network_compute_hours: number): string;
function get_tiers_json(): string;

// Reputation helpers
function composite_reputation(accuracy: number, uptime: number, stake: bigint): number;
function stake_weight(stake: bigint): number;

// Module info
function version(): string;
function init_panic_hook(): void;
```

## WASM Bundle Information

| File | Size | Description |
|------|------|-------------|
| `ruvector_economy_wasm_bg.wasm` | 178 KB | WebAssembly binary |
| `ruvector_economy_wasm.js` | 47 KB | JavaScript bindings |
| `ruvector_economy_wasm.d.ts` | 15 KB | TypeScript definitions |

## Browser Compatibility

- Chrome 89+ (WebAssembly bulk memory, nontrapping-fptoint)
- Firefox 89+
- Safari 15+
- Edge 89+

## License

MIT
