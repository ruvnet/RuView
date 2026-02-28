# @ruvector/economy-wasm - CRDT Credit Economy for Distributed Compute

[![npm version](https://img.shields.io/npm/v/ruvector-economy-wasm.svg)](https://www.npmjs.com/package/ruvector-economy-wasm)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/ruvnet/ruvector)
[![Bundle Size](https://img.shields.io/badge/bundle%20size-177KB%20gzip-green.svg)](https://www.npmjs.com/package/ruvector-economy-wasm)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-654FF0?logo=webassembly&logoColor=white)](https://webassembly.org/)

A **CRDT-based autonomous credit economy** for distributed compute networks. Provides conflict-free P2P credit tracking, stake/slash mechanics, and reputation scoring - a blockchain alternative for edge computing and AI agent coordination.

## Key Features

- **CRDT Ledger**: G-Counter and PN-Counter for P2P-safe credit tracking with guaranteed eventual consistency
- **10x Early Adopter Curve**: Contribution multiplier decaying from 10x to 1x baseline as network grows
- **Stake/Slash Mechanics**: Participation requirements with slashing for sybil attacks, double-spending, and bad behavior
- **Reputation Scoring**: Multi-factor composite score based on accuracy, uptime, and stake weight
- **Merkle State Root**: Fast ledger verification with cryptographic proofs
- **WASM-Optimized**: Runs in browsers, Node.js, and edge runtimes

## Installation

```bash
npm install ruvector-economy-wasm
# or
yarn add ruvector-economy-wasm
# or
pnpm add ruvector-economy-wasm
```

## Quick Start

### TypeScript/JavaScript

```typescript
import init, {
  CreditLedger,
  ReputationScore,
  StakeManager,
  contribution_multiplier,
  SlashReason
} from 'ruvector-economy-wasm';

// Initialize WASM module
await init();

// Create a credit ledger for a node
const ledger = new CreditLedger("node-123");

// Earn credits for completed tasks
ledger.credit(100n, "task:compute-job-456");
console.log(`Balance: ${ledger.balance()}`);

// Check early adopter multiplier
const mult = contribution_multiplier(50000.0);  // 50K network compute hours
console.log(`Multiplier: ${mult.toFixed(2)}x`);  // ~8.5x for early adopters

// Credit with multiplier applied
ledger.creditWithMultiplier(50n, "task:bonus-789");

// Track reputation
const rep = new ReputationScore(0.95, 0.98, 1000n);
console.log(`Composite score: ${rep.compositeScore()}`);
console.log(`Tier: ${rep.tierName()}`);
```

## Understanding CRDTs

**Conflict-free Replicated Data Types (CRDTs)** enable distributed systems to:
- Merge updates in any order with identical results
- Operate offline and sync later without conflicts
- Scale horizontally without coordination bottlenecks

This package uses:
- **G-Counter**: Grow-only counter for earned credits (monotonically increasing)
- **PN-Counter**: Positive-negative counter for spending (allows refunds/disputes)

```typescript
// P2P merge example - works regardless of message order
const nodeA = new CreditLedger("node-A");
const nodeB = new CreditLedger("node-B");

// Both nodes earn credits independently
nodeA.credit(100n, "job-1");
nodeB.credit(50n, "job-2");

// Export for P2P sync
const earnedA = nodeA.exportEarned();
const spentA = nodeA.exportSpent();

// Merge on node B - associative, commutative, idempotent
const merged = nodeB.merge(earnedA, spentA);
console.log(`Merged ${merged} updates`);
```

## Contribution Curve

Early network contributors receive higher rewards that decay as the network matures:

```
Multiplier = 1 + 9 * exp(-compute_hours / 100,000)

Network Hours  | Multiplier
---------------|------------
0              | 10.0x (Genesis)
10,000         | ~9.0x
50,000         | ~6.0x
100,000        | ~4.3x
200,000        | ~2.2x
500,000        | ~1.0x (Baseline)
```

```typescript
import { contribution_multiplier, get_tier_name, get_tiers_json } from 'ruvector-economy-wasm';

// Check current multiplier
const hours = 25000;
const mult = contribution_multiplier(hours);
console.log(`At ${hours} hours: ${mult.toFixed(2)}x multiplier`);

// Get tier name
const tier = get_tier_name(hours);  // "Pioneer"

// Get all tier definitions
const tiers = JSON.parse(get_tiers_json());
```

## Stake/Slash Mechanics

```typescript
import { StakeManager, SlashReason } from 'ruvector-economy-wasm';

const stakeManager = new StakeManager();

// Stake credits for network participation
stakeManager.stake("node-123", 1000n);
console.log(`Staked: ${stakeManager.getStake("node-123")}`);

// Check if node meets minimum stake
if (stakeManager.meetsMinimum("node-123")) {
  console.log("Node can participate");
}

// Delegate stake to another node
stakeManager.delegate("node-123", "validator-1", 500n);
console.log(`Effective stake: ${stakeManager.getEffectiveStake("validator-1")}`);

// Slash for bad behavior
const slashedAmount = stakeManager.slash(
  "bad-actor",
  SlashReason.DoubleSpend,
  "Evidence: duplicate transaction IDs"
);
console.log(`Slashed ${slashedAmount} credits`);
```

### Slash Reasons

| Reason | Severity | Description |
|--------|----------|-------------|
| `InvalidResult` | Medium | Submitted incorrect computation results |
| `DoubleSpend` | High | Attempted to spend same credits twice |
| `SybilAttack` | Critical | Multiple fake identities detected |
| `Downtime` | Low | Excessive offline periods |
| `Spam` | Medium | Flooding the network |
| `Malicious` | Critical | Intentional harmful behavior |

## Reputation System

```typescript
import { ReputationScore, composite_reputation } from 'ruvector-economy-wasm';

// Create reputation with tracking
const rep = ReputationScore.newWithTracking(
  950n,  // tasks completed
  50n,   // tasks failed
  BigInt(30 * 24 * 3600),  // uptime seconds
  BigInt(31 * 24 * 3600),  // total seconds
  5000n  // stake amount
);

// Record task outcomes
rep.recordSuccess();
rep.recordFailure();

// Calculate composite score
// Formula: accuracy^2 * uptime * stake_weight
const score = rep.compositeScore();
console.log(`Composite: ${(score * 100).toFixed(1)}%`);

// Get tier
console.log(`Tier: ${rep.tierName()}`);  // "Elite", "Trusted", "Standard", etc.

// Check participation eligibility
if (rep.meetsMinimum(0.9, 0.95, 100n)) {
  console.log("Eligible for premium tasks");
}

// Compare reputations
const rep2 = new ReputationScore(0.92, 0.96, 3000n);
console.log(`Better reputation: ${rep.isBetterThan(rep2) ? 'rep1' : 'rep2'}`);
```

### Reputation Tiers

| Tier | Score Range | Benefits |
|------|-------------|----------|
| Elite | >= 0.95 | Priority task assignment, lowest fees |
| Trusted | >= 0.85 | High-value tasks, reduced collateral |
| Standard | >= 0.70 | Normal participation |
| Probation | >= 0.50 | Limited task types |
| Restricted | < 0.50 | Basic tasks only, increased monitoring |

## Merkle State Verification

```typescript
const ledger = new CreditLedger("node-123");
ledger.credit(100n, "job-1");
ledger.credit(200n, "job-2");

// Get state root for verification
const stateRoot = ledger.stateRoot();
const stateRootHex = ledger.stateRootHex();
console.log(`State root: ${stateRootHex}`);

// Verify state integrity
const isValid = ledger.verifyStateRoot(expectedRoot);
```

## API Reference

### CreditLedger

| Method | Description |
|--------|-------------|
| `new(node_id)` | Create ledger for node |
| `credit(amount, reason)` | Earn credits |
| `creditWithMultiplier(base_amount, reason)` | Earn with network multiplier |
| `deduct(amount)` | Spend credits |
| `refund(event_id, amount)` | Refund a deduction |
| `balance()` | Get available balance |
| `stake(amount)` | Lock credits for participation |
| `slash(amount)` | Penalty for bad behavior |
| `merge(other_earned, other_spent)` | CRDT merge operation |
| `exportEarned()` / `exportSpent()` | Export for P2P sync |
| `stateRoot()` / `stateRootHex()` | Merkle verification |

### ReputationScore

| Method | Description |
|--------|-------------|
| `new(accuracy, uptime, stake)` | Create with scores |
| `newWithTracking(...)` | Create with detailed tracking |
| `compositeScore()` | Calculate composite (0.0-1.0) |
| `tierName()` | Get reputation tier |
| `recordSuccess()` / `recordFailure()` | Track task outcomes |
| `stakeWeight()` | Logarithmic stake weight |
| `meetsMinimum(accuracy, uptime, stake)` | Check eligibility |

### StakeManager

| Method | Description |
|--------|-------------|
| `new()` | Create manager |
| `stake(node_id, amount)` | Stake credits |
| `unstake(node_id, amount)` | Unstake (if unlocked) |
| `delegate(from, to, amount)` | Delegate to another node |
| `slash(node_id, reason, evidence)` | Slash for violation |
| `getEffectiveStake(node_id)` | Own + delegated stake |
| `meetsMinimum(node_id)` | Check stake requirement |

## Use Cases

- **Distributed AI Training**: Reward compute contributors fairly
- **Edge Computing Networks**: Track and reward edge node participation
- **Federated Learning**: Incentivize model training contributions
- **P2P Storage**: Credit-based storage allocation
- **Agent Coordination**: Economic layer for multi-agent systems
- **Decentralized Inference**: Pay-per-inference without blockchain overhead

## Bundle Size

- **WASM binary**: ~177KB (uncompressed)
- **Gzip compressed**: ~65KB
- **JavaScript glue**: ~8KB

## Related Packages

- [ruvector-learning-wasm](https://www.npmjs.com/package/ruvector-learning-wasm) - MicroLoRA adaptation
- [ruvector-exotic-wasm](https://www.npmjs.com/package/ruvector-exotic-wasm) - NAO governance, morphogenetic networks
- [ruvector-nervous-system-wasm](https://www.npmjs.com/package/ruvector-nervous-system-wasm) - Bio-inspired neural components

## License

MIT

## Links

- [GitHub Repository](https://github.com/ruvnet/ruvector)
- [Full Documentation](https://ruv.io)
- [Bug Reports](https://github.com/ruvnet/ruvector/issues)

---

**Keywords**: CRDT, distributed systems, credits, P2P, peer-to-peer, blockchain alternative, reputation, stake, slash, economy, WebAssembly, WASM, edge computing, decentralized, conflict-free, eventual consistency, G-Counter, PN-Counter
