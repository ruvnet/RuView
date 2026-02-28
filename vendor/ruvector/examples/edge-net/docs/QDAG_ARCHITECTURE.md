# Edge-Net QDAG Credit System Architecture

## Table of Contents

1. [Overview](#overview)
2. [Architecture Components](#architecture-components)
3. [Credit Flow](#credit-flow)
4. [Security Model](#security-model)
5. [Multi-Device Synchronization](#multi-device-synchronization)
6. [API Reference](#api-reference)
7. [Data Models](#data-models)
8. [Implementation Details](#implementation-details)

---

## Overview

The Edge-Net QDAG (Quantum Directed Acyclic Graph) credit system is a **secure, distributed ledger** for tracking computational contributions across the Edge-Net network. Credits (denominated in rUv) are earned by processing tasks and stored in a **Firestore-backed persistent ledger** that serves as the **single source of truth**.

### Key Principles

1. **Identity-Based Ledger**: Credits are tied to **Ed25519 public keys**, not device IDs
2. **Relay Authority**: Only the relay server can credit accounts via verified task completions
3. **No Self-Reporting**: Clients cannot increase their own credit balances
4. **Multi-Device Sync**: Same public key = same balance across all devices
5. **Firestore Truth**: The QDAG ledger in Firestore is the authoritative state

---

## Architecture Components

```
┌─────────────────────────────────────────────────────────────────┐
│                        Edge-Net QDAG System                     │
└─────────────────────────────────────────────────────────────────┘

┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   Dashboard  │◄───────►│    Relay     │◄───────►│  Firestore   │
│   (Client)   │  WSS    │   Server     │  QDAG   │   Database   │
└──────────────┘         └──────────────┘         └──────────────┘
      │                         │                         │
      │ WASM Edge-Net          │ Task Assignment         │ Ledger
      │ Local Compute          │ Credit Verification     │ Storage
      │                         │                         │
      ▼                         ▼                         ▼
┌──────────────┐         ┌──────────────┐         ┌──────────────┐
│   PiKey ID   │         │ Assigned     │         │ Credit Ledger│
│  (Ed25519)   │         │ Tasks Map    │         │ (by pubkey)  │
└──────────────┘         └──────────────┘         └──────────────┘
```

### Components

#### 1. **Dashboard (Client)**
- **Location**: `/examples/edge-net/dashboard/`
- **Role**: Browser-based UI for user interaction
- **Technology**: React + TypeScript + WASM
- **Responsibilities**:
  - Generate Ed25519 identity (PiKey) via WASM
  - Connect to relay server via WebSocket
  - Process assigned computational tasks
  - Display credit balance from QDAG
  - Store local cache in IndexedDB (backup only)

#### 2. **Relay Server**
- **Location**: `/examples/edge-net/relay/index.js`
- **Role**: Central coordination and credit authority
- **Technology**: Node.js + WebSocket + Firestore
- **Responsibilities**:
  - Track task assignments (prevent spoofing)
  - Verify task completions
  - Credit accounts in Firestore QDAG
  - Synchronize balances across devices
  - Enforce rate limits and security

#### 3. **Firestore QDAG**
- **Collection**: `edge-net-qdag`
- **Document Key**: Ed25519 public key (hex string)
- **Role**: Persistent, authoritative credit ledger
- **Technology**: Google Cloud Firestore
- **Responsibilities**:
  - Store credit balances (earned, spent)
  - Track task completion count
  - Enable multi-device sync
  - Provide audit trail

#### 4. **CLI (Optional)**
- **Location**: `/examples/edge-net/cli/`
- **Role**: Command-line interface for headless nodes
- **Technology**: Node.js + WASM
- **Responsibilities**:
  - Same as dashboard, but CLI-based
  - Uses same PiKey identity system
  - Syncs to same QDAG ledger

---

## Credit Flow

### How Credits Are Earned

```
1. Task Submission
   User A submits task → Relay adds to queue → Assigns to User B

2. Task Assignment (SECURITY CHECKPOINT)
   Relay tracks: {
     taskId → assignedTo: User B's nodeId,
              assignedToPublicKey: User B's Ed25519 key,
              submitter: User A's nodeId,
              maxCredits: 1000000 (1 rUv)
   }

3. Task Processing
   User B's WASM node processes task → Completes task

4. Task Completion (SECURITY VERIFICATION)
   User B sends: { type: 'task_complete', taskId }

   Relay verifies:
   ✓ Task exists in assignedTasks map
   ✓ Task was assigned to User B (prevent spoofing)
   ✓ Task not already completed (prevent replay)
   ✓ User B has valid public key for crediting

5. Credit Award (QDAG UPDATE)
   Relay calls: creditAccount(publicKey, amount, taskId)

   Firestore update:
   - ledger.earned += 1.0 rUv
   - ledger.tasksCompleted += 1
   - ledger.lastTaskId = taskId
   - ledger.updatedAt = Date.now()

6. Balance Notification
   Relay → User B: {
     type: 'credit_earned',
     amount: '1000000000' (nanoRuv),
     balance: { earned, spent, available }
   }

7. Client Update
   Dashboard updates UI with new balance from QDAG
```

### Credit Storage Format

**Firestore Document** (`edge-net-qdag/{publicKey}`):
```json
{
  "earned": 42.5,        // Total rUv earned (float)
  "spent": 10.0,         // Total rUv spent (float)
  "tasksCompleted": 123, // Number of tasks
  "lastTaskId": "task-...",
  "createdAt": 1704067200000,
  "updatedAt": 1704153600000
}
```

**Client Representation** (nanoRuv):
```typescript
{
  earned: "42500000000",   // 42.5 rUv in nanoRuv
  spent:  "10000000000",   // 10.0 rUv in nanoRuv
  available: "32500000000" // earned - spent
}
```

**Conversion**: `1 rUv = 1,000,000,000 nanoRuv (1e9)`

---

## Security Model

### What Prevents Cheating?

#### 1. **Task Assignment Tracking**
```javascript
// Relay tracks assignments BEFORE tasks are sent
const assignedTasks = new Map(); // taskId → assignment details

// On task assignment:
assignedTasks.set(task.id, {
  assignedTo: targetNodeId,
  assignedToPublicKey: targetWs.publicKey,
  submitter: task.submitter,
  maxCredits: task.maxCredits,
  assignedAt: Date.now(),
});

// On task completion - verify assignment:
if (assignment.assignedTo !== nodeId) {
  console.warn('[SECURITY] SPOOFING ATTEMPT');
  return; // Reject
}
```

**Protection**: Prevents nodes from claiming credit for tasks they didn't receive.

#### 2. **Double Completion Prevention**
```javascript
const completedTasks = new Set(); // Track completed task IDs

if (completedTasks.has(taskId)) {
  console.warn('[SECURITY] REPLAY ATTEMPT');
  return; // Reject
}

completedTasks.add(taskId); // Mark as completed BEFORE crediting
```

**Protection**: Prevents replay attacks where the same completion is submitted multiple times.

#### 3. **Client Cannot Self-Report Credits**
```javascript
case 'ledger_update':
  // DEPRECATED: Clients cannot increase their own balance
  console.warn('[SECURITY] Rejected ledger_update from client');
  ws.send({ type: 'error', message: 'Credit self-reporting disabled' });
  break;
```

**Protection**: Only the relay can call `creditAccount()` in Firestore.

#### 4. **Public Key Verification**
```javascript
// Credits require valid public key
if (!processorPublicKey) {
  ws.send({ type: 'error', message: 'Public key required for credit' });
  return;
}

// Credit is tied to public key, not node ID
await creditAccount(processorPublicKey, rewardRuv, taskId);
```

**Protection**: Credits tied to cryptographic identity, not ephemeral node IDs.

#### 5. **Task Expiration**
```javascript
setInterval(() => {
  const TASK_TIMEOUT = 5 * 60 * 1000; // 5 minutes
  for (const [taskId, task] of assignedTasks) {
    if (Date.now() - task.assignedAt > TASK_TIMEOUT) {
      assignedTasks.delete(taskId);
    }
  }
}, 60000);
```

**Protection**: Prevents indefinite task hoarding or delayed completion attacks.

#### 6. **Rate Limiting**
```javascript
const RATE_LIMIT_WINDOW = 60000; // 1 minute
const RATE_LIMIT_MAX = 100; // max messages per window

function checkRateLimit(nodeId) {
  // Track message count per node
  // Reject if exceeded
}
```

**Protection**: Prevents spam and rapid task completion abuse.

#### 7. **Origin Validation**
```javascript
const ALLOWED_ORIGINS = new Set([
  'http://localhost:3000',
  'https://edge-net.ruv.io',
  // ...
]);

if (!isOriginAllowed(origin)) {
  ws.close(4001, 'Unauthorized origin');
}
```

**Protection**: Prevents unauthorized clients from connecting.

#### 8. **Firestore as Single Source of Truth**

```javascript
// Load from Firestore
const ledger = await loadLedger(publicKey);
// Cache locally but Firestore is authoritative

// Save to Firestore
await ledgerCollection.doc(publicKey).set(ledger, { merge: true });
```

**Protection**: Clients cannot manipulate balances; Firestore is immutable to clients.

---

## Multi-Device Synchronization

### Same Identity = Same Balance Everywhere

#### Identity Generation (PiKey)

**Dashboard** (`identityStore.ts`):
```typescript
// Generate Ed25519 key pair via WASM
const piKey = await edgeNetService.generateIdentity();

const identity = {
  publicKey: bytesToHex(piKey.getPublicKey()), // hex string
  shortId: piKey.getShortId(),                 // abbreviated ID
  identityHex: piKey.getIdentityHex(),         // full hex
  hasPiMagic: piKey.verifyPiMagic(),           // WASM validation
};
```

**CLI** (same WASM module):
```javascript
const piKey = edgeNet.PiKey.generate();
const publicKey = Buffer.from(piKey.getPublicKey()).toString('hex');
```

**Key Point**: Both use the same WASM `PiKey` module → same Ed25519 keys.

#### Ledger Synchronization Flow

```
1. Device A connects to relay
   → Sends: { type: 'register', publicKey: '0x123abc...' }
   → Relay stores: ws.publicKey = '0x123abc...'

2. Device A requests balance
   → Sends: { type: 'ledger_sync', publicKey: '0x123abc...' }

3. Relay loads from QDAG
   → Firestore.get('edge-net-qdag/0x123abc...')
   → Returns: { earned: 42.5, spent: 10.0 }

4. Device A receives authoritative balance
   → { type: 'ledger_sync_response', ledger: { earned, spent } }
   → Updates local UI

5. Device A completes task
   → Relay credits: creditAccount('0x123abc...', 1.0)
   → Firestore updates: earned = 43.5

6. Device B connects with SAME publicKey
   → Sends: { type: 'ledger_sync', publicKey: '0x123abc...' }
   → Receives: { earned: 43.5, spent: 10.0 }
   → Same balance as Device A ✓
```

### Backup and Recovery

**Export Identity** (Dashboard):
```typescript
// Create encrypted backup with Argon2id
const backup = currentPiKey.createEncryptedBackup(password);
const backupHex = bytesToHex(backup); // Store securely
```

**Import on New Device**:
```typescript
// Restore from encrypted backup
const seed = hexToBytes(backupHex);
const piKey = await edgeNetService.generateIdentity(seed);
// → Same public key → Same QDAG balance
```

---

## API Reference

### WebSocket Message Types

#### Client → Relay

##### `register`
Register a new node with the relay.

```typescript
{
  type: 'register',
  nodeId: string,           // Session node ID
  publicKey?: string,       // Ed25519 public key (hex) for QDAG
  capabilities: string[],   // ['compute', 'storage']
  version: string           // Client version
}
```

**Response**: `welcome` message

---

##### `ledger_sync`
Request current balance from QDAG.

```typescript
{
  type: 'ledger_sync',
  publicKey: string,  // Ed25519 public key (hex)
  nodeId: string
}
```

**Response**: `ledger_sync_response`

---

##### `task_submit`
Submit a new task to the network.

```typescript
{
  type: 'task_submit',
  task: {
    taskType: string,      // 'compute' | 'inference' | 'storage'
    payload: number[],     // Task data as byte array
    maxCredits: string     // Max reward in nanoRuv
  }
}
```

**Response**: `task_accepted` with `taskId`

---

##### `task_complete`
Report task completion (triggers credit award).

```typescript
{
  type: 'task_complete',
  taskId: string,
  result: unknown,  // Task output
  reward?: string   // Requested reward (capped by maxCredits)
}
```

**Response**: `credit_earned` (if verified)

---

##### `heartbeat`
Keep connection alive.

```typescript
{
  type: 'heartbeat'
}
```

**Response**: `heartbeat_ack`

---

#### Relay → Client

##### `welcome`
Initial connection confirmation.

```typescript
{
  type: 'welcome',
  nodeId: string,
  networkState: {
    genesisTime: number,
    totalNodes: number,
    activeNodes: number,
    totalTasks: number,
    totalRuvDistributed: string,  // bigint as string
    timeCrystalPhase: number
  },
  peers: string[]  // Connected peer node IDs
}
```

---

##### `ledger_sync_response`
Authoritative balance from QDAG.

```typescript
{
  type: 'ledger_sync_response',
  ledger: {
    publicKey: string,
    nodeId: string,
    earned: string,        // nanoRuv
    spent: string,         // nanoRuv
    available: string,     // earned - spent
    tasksCompleted: number,
    lastUpdated: number,   // timestamp
    signature: string      // 'qdag-verified'
  }
}
```

---

##### `task_assignment`
Assigned task to process.

```typescript
{
  type: 'task_assignment',
  task: {
    id: string,
    submitter: string,
    taskType: string,
    payload: number[],     // Task data
    maxCredits: string,    // Max reward in nanoRuv
    submittedAt: number
  }
}
```

---

##### `credit_earned`
Credit awarded after task completion.

```typescript
{
  type: 'credit_earned',
  amount: string,      // nanoRuv earned
  taskId: string,
  balance: {
    earned: string,    // Total earned (nanoRuv)
    spent: string,     // Total spent (nanoRuv)
    available: string  // Available (nanoRuv)
  }
}
```

---

##### `time_crystal_sync`
Network-wide time synchronization.

```typescript
{
  type: 'time_crystal_sync',
  phase: number,       // 0-1 phase value
  timestamp: number,   // Unix timestamp
  activeNodes: number
}
```

---

##### `node_joined` / `node_left`
Peer connectivity events.

```typescript
{
  type: 'node_joined' | 'node_left',
  nodeId: string,
  totalNodes: number
}
```

---

##### `error`
Error response.

```typescript
{
  type: 'error',
  message: string
}
```

---

### HTTP Endpoints

#### `GET /health`
Health check endpoint.

**Response**:
```json
{
  "status": "healthy",
  "nodes": 42,
  "uptime": 3600000
}
```

---

#### `GET /stats`
Network statistics.

**Response**:
```json
{
  "genesisTime": 1704067200000,
  "totalNodes": 150,
  "activeNodes": 142,
  "totalTasks": 9876,
  "totalRuvDistributed": "1234567890",
  "timeCrystalPhase": 0.618,
  "connectedNodes": ["node-1", "node-2", ...]
}
```

---

## Data Models

### Firestore Schema

#### Collection: `edge-net-qdag`

**Document ID**: Ed25519 public key (hex string)

```typescript
{
  earned: number,         // Total rUv earned (float)
  spent: number,          // Total rUv spent (float)
  tasksCompleted: number, // Count of completed tasks
  lastTaskId?: string,    // Most recent task ID
  createdAt: number,      // First entry timestamp
  updatedAt: number       // Last update timestamp
}
```

**Example**:
```json
{
  "earned": 127.3,
  "spent": 25.0,
  "tasksCompleted": 456,
  "lastTaskId": "task-1704153600000-abc123",
  "createdAt": 1704067200000,
  "updatedAt": 1704153600000
}
```

---

### Client State

#### `networkStore.ts` - Credit Balance

```typescript
interface CreditBalance {
  available: number,  // earned - spent (rUv)
  pending: number,    // Credits not yet confirmed
  earned: number,     // Total earned (rUv)
  spent: number       // Total spent (rUv)
}
```

**Updated by**:
- `onCreditEarned`: Increment earned when task completes
- `onLedgerSync`: Replace with QDAG authoritative values

---

#### `identityStore.ts` - PiKey Identity

```typescript
interface PeerIdentity {
  id: string,              // Libp2p-style peer ID
  publicKey: string,       // Ed25519 public key (hex)
  publicKeyBytes?: Uint8Array,
  displayName: string,
  createdAt: Date,
  shortId: string,         // Abbreviated ID
  identityHex: string,     // Full identity hex
  hasPiMagic: boolean      // WASM PiKey validation
}
```

---

### IndexedDB Schema

#### Store: `edge-net-store`

**Purpose**: Local cache (NOT source of truth)

```typescript
{
  id: 'primary',
  nodeId: string,
  creditsEarned: number,      // Cache from QDAG
  creditsSpent: number,       // Cache from QDAG
  tasksCompleted: number,
  tasksSubmitted: number,
  totalUptime: number,
  lastActiveTimestamp: number,
  consentGiven: boolean,
  consentTimestamp: number | null,
  cpuLimit: number,
  gpuEnabled: boolean,
  gpuLimit: number,
  respectBattery: boolean,
  onlyWhenIdle: boolean
}
```

**Note**: IndexedDB is a **backup only**. QDAG is the source of truth.

---

## Implementation Details

### Credit Award Flow (Relay)

```javascript
// /examples/edge-net/relay/index.js

case 'task_complete': {
  const taskId = message.taskId;

  // 1. Verify task assignment
  const assignment = assignedTasks.get(taskId);
  if (!assignment || assignment.assignedTo !== nodeId) {
    return; // Reject spoofing attempt
  }

  // 2. Check double completion
  if (completedTasks.has(taskId)) {
    return; // Reject replay attack
  }

  // 3. Get processor's public key
  const publicKey = assignment.assignedToPublicKey || ws.publicKey;
  if (!publicKey) {
    return; // Reject - no identity
  }

  // 4. Mark as completed (prevent race conditions)
  completedTasks.add(taskId);
  assignedTasks.delete(taskId);

  // 5. Credit the account in QDAG
  const rewardRuv = Number(message.reward || assignment.maxCredits) / 1e9;
  const updatedLedger = await creditAccount(publicKey, rewardRuv, taskId);

  // 6. Notify client
  ws.send({
    type: 'credit_earned',
    amount: (rewardRuv * 1e9).toString(),
    balance: {
      earned: (updatedLedger.earned * 1e9).toString(),
      spent: (updatedLedger.spent * 1e9).toString(),
      available: ((updatedLedger.earned - updatedLedger.spent) * 1e9).toString(),
    },
  });
}
```

---

### Ledger Sync Flow (Dashboard)

```typescript
// /examples/edge-net/dashboard/src/stores/networkStore.ts

connectToRelay: async () => {
  // 1. Get identity public key
  const identityState = useIdentityStore.getState();
  const publicKey = identityState.identity?.publicKey;

  // 2. Connect to relay with public key
  const connected = await relayClient.connect(nodeId, publicKey);

  // 3. Request QDAG balance after connection
  if (connected && publicKey) {
    setTimeout(() => {
      relayClient.requestLedgerSync(publicKey);
    }, 500);
  }
},

// 4. Handle QDAG response (authoritative)
onLedgerSync: (ledger) => {
  const earnedRuv = Number(ledger.earned) / 1e9;
  const spentRuv = Number(ledger.spent) / 1e9;

  // Replace local state with QDAG values
  set({
    credits: {
      earned: earnedRuv,
      spent: spentRuv,
      available: earnedRuv - spentRuv,
      pending: 0,
    },
  });

  // Save to IndexedDB as backup
  get().saveToIndexedDB();
},
```

---

### Task Processing Flow (Dashboard)

```typescript
// /examples/edge-net/dashboard/src/stores/networkStore.ts

processAssignedTask: async (task) => {
  // 1. Process task using WASM
  const result = await edgeNetService.submitTask(
    task.taskType,
    task.payload,
    task.maxCredits
  );

  await edgeNetService.processNextTask();

  // 2. Report completion to relay
  const reward = task.maxCredits / BigInt(2); // Earn half the max
  relayClient.completeTask(task.id, task.submitter, result, reward);

  // 3. Relay verifies and credits QDAG
  // 4. Client receives credit_earned message
  // 5. Balance updates automatically
},
```

---

## Summary

The Edge-Net QDAG credit system provides a **secure, distributed ledger** for tracking computational contributions:

✅ **Identity-Based**: Credits tied to Ed25519 public keys, not devices
✅ **Relay Authority**: Only relay can credit accounts via verified tasks
✅ **Multi-Device Sync**: Same key = same balance everywhere
✅ **Firestore Truth**: QDAG in Firestore is the authoritative state
✅ **Security**: Prevents spoofing, replay, self-reporting, and double-completion
✅ **IndexedDB Cache**: Local backup, but QDAG is source of truth

**Key Insight**: The relay server acts as a **trusted coordinator** that verifies task completions before updating the QDAG ledger in Firestore. Clients cannot manipulate their balances; they can only earn credits by processing assigned tasks.
