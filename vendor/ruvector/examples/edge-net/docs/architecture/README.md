# @ruvector/edge-net: Distributed Compute Intelligence Network

## Executive Summary

A JavaScript library that website owners embed to contribute compute power to a shared intelligence network. Contributors earn credits based on compute donated, which they can use to access the network's collective processing power. Early adopters receive bonus rewards via a contribution curve, creating a self-sustaining P2P compute marketplace.

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        EDGE-NET: SHARED COMPUTE INTELLIGENCE                │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                             │
│    Website A              Website B              Website C                  │
│    ┌─────────┐            ┌─────────┐            ┌─────────┐               │
│    │ Visitor │            │ Visitor │            │ Visitor │               │
│    │ Browser │            │ Browser │            │ Browser │               │
│    └────┬────┘            └────┬────┘            └────┬────┘               │
│         │                      │                      │                     │
│    ┌────▼────┐            ┌────▼────┐            ┌────▼────┐               │
│    │edge-net │◄──────────►│edge-net │◄──────────►│edge-net │               │
│    │ Worker  │    P2P     │ Worker  │    P2P     │ Worker  │               │
│    └────┬────┘            └────┬────┘            └────┬────┘               │
│         │                      │                      │                     │
│         └──────────────────────┼──────────────────────┘                     │
│                                │                                            │
│                    ┌───────────▼───────────┐                                │
│                    │   Shared Task Queue   │                                │
│                    │   (P2P via GUN.js)    │                                │
│                    └───────────────────────┘                                │
│                                                                             │
│    CONTRIBUTION                TASK TYPES                  REWARDS          │
│    ────────────                ──────────                  ───────          │
│    CPU cycles    ───►    Vector search                     Credits          │
│    Memory        ───►    Embeddings                        Priority         │
│    Bandwidth     ───►    Neural inference                  Multiplier       │
│    Uptime        ───►    Data processing                   Reputation       │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Table of Contents

1. [Problem Statement](#1-problem-statement)
2. [Solution Overview](#2-solution-overview)
3. [Architecture](#3-architecture)
4. [Credit & Reward System](#4-credit--reward-system)
5. [Task Distribution](#5-task-distribution)
6. [Security Model](#6-security-model)
7. [API Design](#7-api-design)
8. [Implementation Plan](#8-implementation-plan)
9. [Package Structure](#9-package-structure)
10. [Performance Targets](#10-performance-targets)

---

## 1. Problem Statement

### Current State
- AI compute is expensive ($200-2000/month for meaningful workloads)
- Billions of browser CPU cycles go unused while users read content
- Edge compute exists but has no incentive model for contributors
- Centralized compute creates vendor lock-in and privacy concerns

### Opportunity
- Average webpage visit: 2-5 minutes of idle browser time
- Modern browsers support Web Workers, WASM, WebGPU
- P2P networks (GUN, libp2p, WebRTC) enable serverless coordination
- Contribution-based economics can align incentives

### Goal
Create a library where:
1. Website owners add one `<script>` tag
2. Visitors automatically contribute idle compute
3. Both earn credits based on contribution
4. Credits unlock access to the network's collective intelligence

---

## 2. Solution Overview

### 2.1 The One-Liner Integration

```html
<!-- Add to any website to participate in the compute network -->
<script src="https://cdn.jsdelivr.net/npm/@ruvector/edge-net@latest"></script>
<script>
  EdgeNet.init({
    siteId: 'your-site-id',           // Your identity
    contribution: 0.3,                 // Use 30% of idle CPU
    tasks: ['vectors', 'embeddings'], // Task types to accept
    onCredit: (credits) => console.log(`Earned: ${credits}`)
  });
</script>
```

### 2.2 What Happens

```
1. INITIALIZATION
   ├── Load WASM modules (364KB)
   ├── Start Web Worker pool
   ├── Connect to P2P network
   └── Begin idle detection

2. CONTRIBUTING (Background)
   ├── Receive tasks from network
   ├── Execute in Web Workers
   ├── Return results to requestor
   └── Earn credits per task

3. CONSUMING (On-Demand)
   ├── Submit task to network
   ├── Pay credits from balance
   ├── Receive results from contributors
   └── Verify result integrity
```

### 2.3 Value Proposition

| Stakeholder | Contribution | Benefit |
|-------------|--------------|---------|
| **Site Owner** | Embeds script, visitor CPU | Credits for AI compute, analytics |
| **Visitor** | Idle CPU cycles | Faster site (precomputed results) |
| **Task Submitter** | Credits | Distributed AI inference |
| **Network** | Coordination | Self-sustaining ecosystem |

---

## 3. Architecture

### 3.1 System Components

```
┌──────────────────────────────────────────────────────────────────────────┐
│                            @ruvector/edge-net                             │
├──────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────────────────────────────────────────────────────────────────┐ │
│  │                         CORE LAYER (Rust/WASM)                      │ │
│  │                                                                     │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │ │
│  │  │ Identity │  │  Credit  │  │   Task   │  │  Proof   │            │ │
│  │  │ Manager  │  │  Ledger  │  │ Executor │  │ Verifier │            │ │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │ │
│  │                                                                     │ │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐            │ │
│  │  │  Vector  │  │ Encrypt  │  │ Compress │  │ Scheduler│            │ │
│  │  │  Engine  │  │  Engine  │  │  Engine  │  │  Engine  │            │ │
│  │  └──────────┘  └──────────┘  └──────────┘  └──────────┘            │ │
│  │                                                                     │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                    │                                      │
│  ┌─────────────────────────────────▼───────────────────────────────────┐ │
│  │                       WORKER LAYER (JavaScript)                     │ │
│  │                                                                     │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │ │
│  │  │   Compute    │  │   Compute    │  │   Compute    │  ...         │ │
│  │  │   Worker 1   │  │   Worker 2   │  │   Worker N   │              │ │
│  │  │  (WASM Exec) │  │  (WASM Exec) │  │  (WASM Exec) │              │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │ │
│  │                                                                     │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                    │                                      │
│  ┌─────────────────────────────────▼───────────────────────────────────┐ │
│  │                      NETWORK LAYER (P2P)                            │ │
│  │                                                                     │ │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │ │
│  │  │  Task Queue  │  │   Credit    │  │  Discovery   │              │ │
│  │  │  (GUN.js)    │  │   Sync      │  │  (DHT/MDNS)  │              │ │
│  │  └──────────────┘  └──────────────┘  └──────────────┘              │ │
│  │                                                                     │ │
│  └─────────────────────────────────────────────────────────────────────┘ │
│                                                                          │
└──────────────────────────────────────────────────────────────────────────┘
```

### 3.2 Data Flow

```
TASK SUBMISSION:

Submitter                    Network                     Contributors
    │                           │                              │
    │  1. Submit Task           │                              │
    │  ─────────────────►       │                              │
    │  {task, credits, sig}     │                              │
    │                           │  2. Broadcast Task           │
    │                           │  ────────────────────►       │
    │                           │                              │
    │                           │         3. Claim Task        │
    │                           │  ◄────────────────────       │
    │                           │  {worker_id, stake}          │
    │                           │                              │
    │                           │  4. Assign + Encrypt         │
    │                           │  ────────────────────►       │
    │                           │  {encrypted_payload}         │
    │                           │                              │
    │                           │         5. Execute           │
    │                           │              │                │
    │                           │              ▼                │
    │                           │         ┌────────┐           │
    │                           │         │ WASM   │           │
    │                           │         │ Worker │           │
    │                           │         └────────┘           │
    │                           │              │                │
    │                           │         6. Return Result     │
    │                           │  ◄────────────────────       │
    │                           │  {result, proof, sig}        │
    │                           │                              │
    │  7. Deliver Result        │                              │
    │  ◄─────────────────       │                              │
    │  {verified_result}        │                              │
    │                           │  8. Credit Transfer          │
    │                           │  ────────────────────►       │
    │                           │  {credits + bonus}           │
    │                           │                              │
```

### 3.3 Idle Detection & Throttling

```javascript
// Smart idle detection to avoid impacting user experience
class IdleDetector {
  constructor(options) {
    this.maxCpu = options.contribution;  // 0.0 - 1.0
    this.currentLoad = 0;
  }

  // Monitor user activity
  isUserIdle() {
    return (
      !document.hasFocus() ||           // Tab not focused
      performance.now() - lastInteraction > 5000 ||  // 5s since interaction
      document.visibilityState === 'hidden'          // Tab hidden
    );
  }

  // Adaptive throttling based on page performance
  getThrottle() {
    const fps = this.measureFPS();
    if (fps < 30) return 0.1;      // Page struggling, back off
    if (fps < 50) return 0.3;      // Moderate load
    if (this.isUserIdle()) return this.maxCpu;  // Full contribution
    return 0.2;                     // User active, light load
  }
}
```

---

## 4. Credit & Reward System

### 4.1 Credit Economics

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         CREDIT FLOW MODEL                               │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│   EARNING                          SPENDING                             │
│   ───────                          ────────                             │
│                                                                         │
│   ┌─────────────┐                  ┌─────────────┐                     │
│   │ Compute     │ ──► 1 credit/    │ Submit Task │ ──► Pay credits     │
│   │ Task        │     task unit    │             │     based on        │
│   └─────────────┘                  └─────────────┘     complexity      │
│                                                                         │
│   ┌─────────────┐                  ┌─────────────┐                     │
│   │ Uptime      │ ──► 0.1 credit/  │ Priority    │ ──► 2x credits     │
│   │ Bonus       │     hour online  │ Execution   │     for fast lane   │
│   └─────────────┘                  └─────────────┘                     │
│                                                                         │
│   ┌─────────────┐                  ┌─────────────┐                     │
│   │ Referral    │ ──► 10% of       │ Storage     │ ──► 0.01 credit/   │
│   │ Bonus       │     referee      │ (Vectors)   │     MB/day         │
│   └─────────────┘                  └─────────────┘                     │
│                                                                         │
│   ┌─────────────┐                                                      │
│   │ Early       │ ──► Multiplier                                       │
│   │ Adopter     │     (see curve)                                      │
│   └─────────────┘                                                      │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### 4.2 Contribution Curve

The reward multiplier decreases as the network grows, incentivizing early adoption:

```
Reward Multiplier Formula:
─────────────────────────

multiplier = 1 + (MAX_BONUS - 1) * e^(-network_compute / DECAY_CONSTANT)

Where:
  - MAX_BONUS = 10x (first contributors get up to 10x rewards)
  - DECAY_CONSTANT = 1,000,000 CPU-hours (half-life of bonus)
  - network_compute = total CPU-hours contributed to date

Example progression:
┌─────────────────────┬─────────────┬─────────────────────────────────────┐
│ Network Stage       │ Multiplier  │ Meaning                             │
├─────────────────────┼─────────────┼─────────────────────────────────────┤
│ Genesis (0 hours)   │ 10.0x       │ First contributors get 10x rewards  │
│ 100K CPU-hours      │ 9.1x        │ Still very early                    │
│ 500K CPU-hours      │ 6.1x        │ Early majority joining              │
│ 1M CPU-hours        │ 4.0x        │ Network maturing                    │
│ 5M CPU-hours        │ 1.4x        │ Established network                 │
│ 10M+ CPU-hours      │ 1.0x        │ Baseline rewards                    │
└─────────────────────┴─────────────┴─────────────────────────────────────┘

Visual:

  10x ┤●
      │ ╲
   8x ┤  ╲
      │   ╲
   6x ┤    ╲
      │     ╲
   4x ┤      ╲
      │       ╲
   2x ┤        ╲___
      │            ╲_____
   1x ┤                  ─────────────────────────────────────
      │
      └────┬────┬────┬────┬────┬────┬────┬────┬────┬────────►
           0   1M   2M   3M   4M   5M   6M   7M   8M   Network
                        CPU-Hours                      Compute
```

### 4.3 Credit Ledger (CRDT-based)

Credits are tracked via a conflict-free replicated data type for P2P consistency:

```rust
// Rust/WASM implementation
pub struct CreditLedger {
    // G-Counter: monotonically increasing credits earned
    earned: HashMap<NodeId, u64>,

    // PN-Counter: credits spent (can be disputed)
    spent: HashMap<NodeId, (u64, u64)>,  // (positive, negative)

    // Merkle root for quick verification
    state_root: [u8; 32],

    // Last sync timestamp
    last_sync: u64,
}

impl CreditLedger {
    pub fn balance(&self, node: &NodeId) -> i64 {
        let earned: u64 = self.earned.values().sum();
        let (pos, neg) = self.spent.get(node).unwrap_or(&(0, 0));
        (earned as i64) - ((pos - neg) as i64)
    }

    pub fn merge(&mut self, other: &CreditLedger) {
        // CRDT merge: take max of each counter
        for (node, value) in &other.earned {
            self.earned.entry(*node)
                .and_modify(|v| *v = (*v).max(*value))
                .or_insert(*value);
        }
        // ... similar for spent
        self.recompute_root();
    }
}
```

### 4.4 Anti-Gaming Measures

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        SYBIL RESISTANCE                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  1. STAKE REQUIREMENT                                                   │
│     ├── New nodes must stake 100 credits to participate                 │
│     ├── Stake slashed for invalid results                               │
│     └── Prevents costless identity creation                             │
│                                                                         │
│  2. PROOF OF WORK                                                       │
│     ├── Tasks include verification challenges                           │
│     ├── Random spot-checks with known solutions                         │
│     └── Reputation score based on accuracy                              │
│                                                                         │
│  3. RATE LIMITING                                                       │
│     ├── Max tasks/hour per identity                                     │
│     ├── Exponential backoff for failures                                │
│     └── Geographic diversity requirements                               │
│                                                                         │
│  4. BROWSER FINGERPRINTING (Privacy-Preserving)                         │
│     ├── WebGL renderer hash                                             │
│     ├── AudioContext fingerprint                                        │
│     ├── Canvas fingerprint                                              │
│     └── Combined into anonymous uniqueness score                        │
│                                                                         │
│  5. ECONOMIC DISINCENTIVES                                              │
│     ├── Cost of attack > benefit                                        │
│     ├── Delayed reward payout (1 hour lock)                             │
│     └── Reputation takes time to build                                  │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 5. Task Distribution

### 5.1 Supported Task Types

| Task Type | Description | Credit Cost | Complexity |
|-----------|-------------|-------------|------------|
| `vector_search` | k-NN search in HNSW index | 1 credit / 1K vectors | Low |
| `vector_insert` | Add vectors to distributed index | 0.5 credit / 100 vectors | Low |
| `embedding` | Generate embeddings (MiniLM, BGE) | 5 credits / 100 texts | Medium |
| `semantic_match` | Task-to-agent routing | 1 credit / 10 queries | Low |
| `neural_inference` | Spiking network forward pass | 3 credits / batch | Medium |
| `encryption` | AES-256-GCM encrypt/decrypt | 0.1 credit / MB | Low |
| `compression` | Adaptive quantization | 0.2 credit / MB | Low |
| `custom_wasm` | User-provided WASM module | Varies | High |

### 5.2 Task Queue Design

```javascript
// P2P Task Queue via GUN.js
class TaskQueue {
  constructor(gun, identity) {
    this.gun = gun;
    this.identity = identity;
    this.queue = gun.get('edge-net').get('tasks');
    this.claims = gun.get('edge-net').get('claims');
  }

  // Submit a task
  async submit(task) {
    const taskId = crypto.randomUUID();
    const envelope = {
      id: taskId,
      type: task.type,
      payload: await this.encrypt(task.payload),
      credits: task.credits,
      priority: task.priority || 'normal',
      submitter: this.identity.agent_id(),
      signature: await this.identity.sign(task.payload),
      expires: Date.now() + (task.ttl || 60000),
      redundancy: task.redundancy || 1,  // How many workers
    };

    await this.queue.get(taskId).put(envelope);
    return taskId;
  }

  // Claim a task for execution
  async claim(taskId) {
    const claim = {
      worker: this.identity.agent_id(),
      stake: 10,  // Credits at risk
      claimed_at: Date.now(),
    };

    // Atomic claim via GUN's conflict resolution
    await this.claims.get(taskId).get(this.identity.agent_id()).put(claim);

    // Check if we won the claim (first N workers)
    const allClaims = await this.getClaims(taskId);
    return this.didWinClaim(allClaims, claim);
  }
}
```

### 5.3 Result Verification

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     RESULT VERIFICATION STRATEGIES                      │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  REDUNDANT EXECUTION (Default)                                          │
│  ─────────────────────────────                                          │
│  ├── Same task sent to N workers (default N=3)                          │
│  ├── Results compared for consensus                                     │
│  ├── Majority result accepted                                           │
│  ├── Outliers penalized (stake slashed)                                 │
│  └── High accuracy, higher cost                                         │
│                                                                         │
│  SPOT-CHECK (Optimistic)                                                │
│  ───────────────────────                                                │
│  ├── Random 10% of tasks include known-answer challenges                │
│  ├── Worker doesn't know which are spot-checks                          │
│  ├── Failed spot-check = reputation penalty                             │
│  └── Lower cost, relies on reputation                                   │
│                                                                         │
│  CRYPTOGRAPHIC PROOF (Future)                                           │
│  ───────────────────────────                                            │
│  ├── ZK-SNARK proof of correct execution                                │
│  ├── Verifiable computation                                             │
│  ├── Single worker sufficient                                           │
│  └── Complex, high overhead                                             │
│                                                                         │
│  REPUTATION-WEIGHTED                                                    │
│  ───────────────────                                                    │
│  ├── High-reputation workers trusted with single execution              │
│  ├── New workers require redundancy                                     │
│  ├── Reputation built over time                                         │
│  └── Balances cost and security                                         │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## 6. Security Model

### 6.1 Threat Model

| Threat | Mitigation |
|--------|------------|
| **Malicious Worker** | Redundant execution, stake slashing, spot-checks |
| **Sybil Attack** | Stake requirement, browser fingerprinting, rate limits |
| **Task Injection** | Cryptographic signatures, submitter verification |
| **Data Exfiltration** | End-to-end encryption, WASM sandboxing |
| **Credit Inflation** | CRDT ledger, consensus on balances, proof-of-work |
| **DoS on Network** | Rate limiting, reputation gating, proof-of-stake |

### 6.2 Encryption Flow

```
Task Submission:

  Submitter                                          Contributor
      │                                                   │
      │  1. Generate ephemeral X25519 keypair             │
      │  ◄────────────────────────────────                │
      │                                                   │
      │  2. Encrypt payload with contributor pubkey       │
      │  ────────────────────────────────►                │
      │  { task_encrypted, submitter_pubkey }             │
      │                                                   │
      │                                                   │  3. Decrypt with
      │                                                   │     private key
      │                                                   │
      │                                                   │  4. Execute task
      │                                                   │
      │  5. Result encrypted with submitter pubkey        │
      │  ◄────────────────────────────────                │
      │  { result_encrypted, proof }                      │
      │                                                   │
      │  6. Decrypt result                                │
      │  ◄────────────────────────────────                │

Key point: Only submitter and assigned contributor can read task/result.
Network sees only encrypted blobs.
```

### 6.3 WASM Sandbox Security

```rust
// Tasks execute in isolated WASM sandbox
pub struct SandboxedExecutor {
    // Memory limits
    max_memory: usize,        // 256MB default
    max_execution_time: u64,  // 30 seconds default

    // Capability restrictions
    allow_network: bool,      // false - no network access
    allow_fs: bool,           // false - no filesystem
    allow_crypto: bool,       // true - crypto primitives only
}

impl SandboxedExecutor {
    pub fn execute(&self, wasm_module: &[u8], input: &[u8]) -> Result<Vec<u8>> {
        // Create isolated instance
        let instance = self.create_instance(wasm_module)?;

        // Set resource limits
        instance.set_memory_limit(self.max_memory);
        instance.set_fuel(self.max_execution_time);

        // Execute with timeout
        let result = tokio::time::timeout(
            Duration::from_secs(30),
            instance.call("execute", input)
        ).await??;

        Ok(result)
    }
}
```

---

## 7. API Design

### 7.1 Contributor API (Website Owners)

```javascript
// Initialize as a contributor
const node = await EdgeNet.init({
  // Identity
  siteId: 'my-site-123',              // Your unique identifier
  privateKey: localStorage.getItem('edgenet_key'),  // Persistent identity

  // Contribution settings
  contribution: {
    cpuLimit: 0.3,                    // Max 30% CPU when idle
    memoryLimit: 256 * 1024 * 1024,   // 256MB max
    bandwidthLimit: 1024 * 1024,      // 1MB/s max
    tasks: ['vectors', 'embeddings', 'encryption'],  // Allowed task types
  },

  // Idle detection
  idle: {
    focusRequired: false,             // Contribute even when focused
    minIdleTime: 5000,                // 5s before considering idle
    respectBattery: true,             // Reduce on battery power
  },

  // Network
  relays: [
    'https://gun-manhattan.herokuapp.com/gun',
    'wss://relay.edgenet.dev',
  ],

  // Callbacks
  onCredit: (credits, total) => {
    console.log(`Earned ${credits}, total: ${total}`);
  },
  onTask: (task) => {
    console.log(`Processing: ${task.type}`);
  },
  onError: (error) => {
    console.error('EdgeNet error:', error);
  },
});

// Check status
console.log(node.stats());
// { credits: 1250, tasksCompleted: 847, uptime: 3600, reputation: 0.95 }

// Pause/resume contribution
node.pause();
node.resume();

// Disconnect
node.disconnect();
```

### 7.2 Consumer API (Task Submitters)

```javascript
// Submit tasks to the network
const result = await EdgeNet.submit({
  type: 'embedding',
  payload: {
    texts: ['Hello world', 'How are you?'],
    model: 'minilm',
  },
  options: {
    priority: 'high',          // 'low' | 'normal' | 'high'
    redundancy: 3,             // Workers for verification
    maxCredits: 10,            // Max credits willing to pay
    timeout: 30000,            // 30s timeout
  },
});

console.log(result);
// {
//   embeddings: [[0.1, 0.2, ...], [0.3, 0.4, ...]],
//   cost: 5,
//   workers: ['node-1', 'node-2', 'node-3'],
//   verified: true
// }

// Batch submission
const results = await EdgeNet.submitBatch([
  { type: 'vector_search', payload: { query: [...], k: 10 } },
  { type: 'semantic_match', payload: { task: 'write code', agents: [...] } },
  { type: 'encryption', payload: { data: [...], key: [...] } },
]);
```

### 7.3 Dashboard Widget

```javascript
// Embed a contribution dashboard
EdgeNet.createWidget({
  container: '#edgenet-widget',
  theme: 'dark',
  showCredits: true,
  showStats: true,
  showLeaderboard: true,
});
```

```html
<!-- Renders as: -->
<div id="edgenet-widget">
  ┌────────────────────────────────────┐
  │ EdgeNet Contributor                │
  ├────────────────────────────────────┤
  │ Credits: 1,250                     │
  │ Tasks:   847 completed             │
  │ Rank:    #1,234 of 50,000          │
  │ Uptime:  12h 34m                   │
  │                                    │
  │ [■■■■■■■□□□] 70% CPU donated       │
  │                                    │
  │ Multiplier: 4.2x (early adopter)   │
  └────────────────────────────────────┘
</div>
```

---

## 8. Implementation Plan

### Phase 1: Core Infrastructure (Week 1-2)

| Task | Description | Files |
|------|-------------|-------|
| 1.1 | Project setup, Cargo.toml, package.json | `Cargo.toml`, `package.json` |
| 1.2 | Identity system (Ed25519 + WASM bindings) | `src/identity.rs` |
| 1.3 | Credit ledger (CRDT implementation) | `src/credits/ledger.rs` |
| 1.4 | Web Worker pool manager | `pkg/worker-pool.js` |
| 1.5 | Basic P2P via GUN.js | `src/network/gun.rs`, `pkg/network.js` |

### Phase 2: Task System (Week 3-4)

| Task | Description | Files |
|------|-------------|-------|
| 2.1 | Task queue (submit, claim, complete) | `src/tasks/queue.rs` |
| 2.2 | Task executor (sandboxed WASM) | `src/tasks/executor.rs` |
| 2.3 | Vector operations (from edge-wasm) | `src/tasks/vectors.rs` |
| 2.4 | Encryption tasks | `src/tasks/crypto.rs` |
| 2.5 | Result verification system | `src/tasks/verify.rs` |

### Phase 3: Credit System (Week 5-6)

| Task | Description | Files |
|------|-------------|-------|
| 3.1 | Contribution curve calculation | `src/credits/curve.rs` |
| 3.2 | Credit transfer protocol | `src/credits/transfer.rs` |
| 3.3 | Stake/slash mechanics | `src/credits/stake.rs` |
| 3.4 | Balance sync (CRDT merge) | `src/credits/sync.rs` |
| 3.5 | Anti-sybil measures | `src/security/sybil.rs` |

### Phase 4: Integration (Week 7-8)

| Task | Description | Files |
|------|-------------|-------|
| 4.1 | JavaScript API wrapper | `pkg/edge-net.js` |
| 4.2 | CDN build (minified, tree-shaken) | `pkg/edge-net.min.js` |
| 4.3 | Dashboard widget | `pkg/widget.js` |
| 4.4 | Example applications | `examples/` |
| 4.5 | Documentation | `README.md` |

### Phase 5: Testing & Launch (Week 9-10)

| Task | Description | Files |
|------|-------------|-------|
| 5.1 | Unit tests (Rust) | `tests/` |
| 5.2 | Integration tests (Browser) | `tests/browser/` |
| 5.3 | Load testing (simulated network) | `tests/load/` |
| 5.4 | Security audit | `SECURITY.md` |
| 5.5 | npm publish | CI/CD |

---

## 9. Package Structure

```
examples/edge-net/
├── Cargo.toml                    # Rust workspace config
├── Cargo.lock
├── README.md                     # Package documentation
├── DESIGN.md                     # This file
├── LICENSE                       # MIT
│
├── src/                          # Rust source
│   ├── lib.rs                    # Main entry point
│   │
│   ├── identity/                 # Identity management
│   │   ├── mod.rs
│   │   ├── keypair.rs            # Ed25519 keypairs
│   │   └── fingerprint.rs        # Browser fingerprinting
│   │
│   ├── credits/                  # Credit system
│   │   ├── mod.rs
│   │   ├── ledger.rs             # CRDT ledger
│   │   ├── curve.rs              # Contribution curve
│   │   ├── transfer.rs           # Credit transfers
│   │   ├── stake.rs              # Staking mechanics
│   │   └── sync.rs               # Balance synchronization
│   │
│   ├── tasks/                    # Task execution
│   │   ├── mod.rs
│   │   ├── queue.rs              # Task queue
│   │   ├── executor.rs           # Sandboxed executor
│   │   ├── vectors.rs            # Vector operations
│   │   ├── embeddings.rs         # Embedding generation
│   │   ├── crypto.rs             # Encryption tasks
│   │   └── verify.rs             # Result verification
│   │
│   ├── network/                  # P2P networking
│   │   ├── mod.rs
│   │   ├── discovery.rs          # Peer discovery
│   │   ├── gun.rs                # GUN.js bridge
│   │   └── protocol.rs           # Wire protocol
│   │
│   ├── scheduler/                # Work scheduling
│   │   ├── mod.rs
│   │   ├── idle.rs               # Idle detection
│   │   ├── throttle.rs           # CPU throttling
│   │   └── priority.rs           # Task prioritization
│   │
│   └── security/                 # Security measures
│       ├── mod.rs
│       ├── sybil.rs              # Anti-sybil
│       ├── sandbox.rs            # WASM sandbox
│       └── audit.rs              # Audit logging
│
├── pkg/                          # Built JavaScript package
│   ├── package.json              # npm package config
│   ├── edge-net.js               # Main entry (ESM)
│   ├── edge-net.min.js           # Minified for CDN
│   ├── edge-net.d.ts             # TypeScript definitions
│   ├── edge-net_bg.wasm          # WASM binary
│   ├── edge-net_bg.wasm.d.ts     # WASM types
│   ├── worker.js                 # Web Worker
│   ├── worker-pool.js            # Worker pool manager
│   ├── network.js                # GUN.js integration
│   ├── widget.js                 # Dashboard widget
│   ├── widget.css                # Widget styles
│   └── README.md                 # npm README
│
├── examples/                     # Example applications
│   ├── contributor.html          # Simple contributor
│   ├── consumer.html             # Task consumer
│   ├── dashboard.html            # Full dashboard
│   ├── chatbot.html              # Distributed chatbot
│   └── vector-search.html        # Distributed search
│
├── tests/                        # Tests
│   ├── unit/                     # Rust unit tests
│   ├── integration/              # Integration tests
│   ├── browser/                  # Browser tests (Playwright)
│   └── load/                     # Load tests
│
└── scripts/                      # Build scripts
    ├── build.sh                  # Build WASM + JS
    ├── bundle.sh                 # Create CDN bundle
    └── publish.sh                # Publish to npm
```

---

## 10. Performance Targets

### 10.1 Metrics

| Metric | Target | Rationale |
|--------|--------|-----------|
| **WASM Load Time** | < 100ms | Minimal impact on page load |
| **Memory Usage** | < 50MB idle | Won't impact browser |
| **CPU Usage (Idle)** | < 5% | Unnoticeable when not contributing |
| **CPU Usage (Active)** | Configurable 10-50% | User control |
| **Task Latency** | < 100ms (local) | Responsive feel |
| **Network Overhead** | < 10KB/min | Minimal bandwidth |
| **Credit Sync** | < 1s eventual | Fast balance updates |
| **Task Throughput** | 100+ tasks/min | Useful compute |

### 10.2 Bundle Size

| Component | Size | Notes |
|-----------|------|-------|
| Core WASM | ~200KB | Compressed |
| JavaScript | ~30KB | Minified + gzipped |
| Worker | ~10KB | Separate chunk |
| Widget | ~15KB | Optional |
| **Total (min)** | **~230KB** | Core only |
| **Total (full)** | **~255KB** | With widget |

### 10.3 Scalability

```
Network Size    Task Throughput    P2P Connections    Credit Sync
────────────    ───────────────    ───────────────    ───────────
100 nodes       1K tasks/min       ~5 per node        < 1s
1K nodes        10K tasks/min      ~10 per node       < 2s
10K nodes       100K tasks/min     ~20 per node       < 5s
100K nodes      1M tasks/min       ~30 per node       < 10s
1M nodes        10M tasks/min      ~50 per node       < 30s
```

---

## Appendix A: Contribution Curve Derivation

The contribution curve follows an exponential decay:

```
R(x) = 1 + (M - 1) * e^(-x/D)

Where:
  R(x) = Reward multiplier at network compute level x
  M    = Maximum multiplier for genesis contributors (10x)
  D    = Decay constant (1,000,000 CPU-hours)
  x    = Total network CPU-hours contributed

Derivation:
  - At x=0: R(0) = 1 + 9*1 = 10x (maximum reward)
  - At x=D: R(D) = 1 + 9/e ≈ 4.3x (36.8% of bonus remaining)
  - At x=2D: R(2D) = 1 + 9/e² ≈ 2.2x
  - At x→∞: R(∞) → 1x (baseline reward)

Properties:
  - Smooth decay (no cliff)
  - Never goes below 1x
  - Predictable for planning
  - Fair to late adopters (still get baseline)
```

---

## Appendix B: CRDT Ledger Specification

```rust
// G-Set: Grow-only set of credit events
struct CreditEvent {
    id: Uuid,
    from: NodeId,
    to: NodeId,
    amount: u64,
    reason: CreditReason,
    timestamp: u64,
    signature: Signature,
}

enum CreditReason {
    TaskCompleted { task_id: Uuid },
    UptimeReward { hours: f32 },
    Referral { referee: NodeId },
    Stake { direction: StakeDirection },
    Transfer { memo: String },
}

// LWW-Register: Last-writer-wins for reputation
struct ReputationRegister {
    node: NodeId,
    score: f32,        // 0.0 - 1.0
    timestamp: u64,
    evidence: Vec<ReputationEvent>,
}

// Merge function (associative, commutative, idempotent)
fn merge(a: &Ledger, b: &Ledger) -> Ledger {
    Ledger {
        events: a.events.union(&b.events),  // G-Set merge
        reputation: merge_lww(&a.reputation, &b.reputation),
    }
}
```

---

## Appendix C: Security Considerations

### C.1 Browser Fingerprinting (Privacy-Preserving)

```javascript
// Generate anonymous uniqueness score without tracking
async function generateAnonymousFingerprint() {
  const components = [
    // Hardware signals
    navigator.hardwareConcurrency,
    screen.width * screen.height,

    // WebGL (hashed)
    hashWebGLRenderer(),

    // Audio (hashed)
    hashAudioContext(),

    // Canvas (hashed)
    hashCanvas(),
  ];

  // Hash all components together
  const fingerprint = await crypto.subtle.digest(
    'SHA-256',
    new TextEncoder().encode(components.join('|'))
  );

  // Only use for uniqueness, not tracking
  return bufferToHex(fingerprint);
}
```

### C.2 Task Payload Encryption

All task payloads are encrypted end-to-end:

1. Submitter generates ephemeral X25519 keypair
2. Task encrypted with contributor's public key
3. Only assigned contributor can decrypt
4. Result encrypted with submitter's public key
5. Network only sees encrypted blobs

### C.3 WASM Sandbox Restrictions

- No network access (fetch, WebSocket, etc.)
- No filesystem access
- No DOM access
- Memory limited to configured maximum
- Execution time limited with fuel metering
- Only pure computation allowed

---

## Next Steps

1. **Review this design** - Gather feedback on architecture
2. **Create project structure** - Set up Cargo workspace and npm package
3. **Implement core identity** - Start with Ed25519 + WASM bindings
4. **Build task executor** - Sandboxed WASM execution
5. **Integrate P2P** - GUN.js for task queue and credit sync
6. **Test with real sites** - Deploy beta to willing participants
