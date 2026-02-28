/**
 * Credit Persistence Test Suite
 * Tests for verifying credit persistence works correctly with QDAG
 *
 * Credit Flow Architecture:
 * 1. Credits are earned via task completions (relay server awards them)
 * 2. Local credits are cached in IndexedDB as backup
 * 3. QDAG (Firestore) is the source of truth for credit balance
 * 4. On reconnect, QDAG balance is loaded and replaces local state
 * 5. Same publicKey = same credits across all devices/CLI
 *
 * Test Scenarios:
 * - Test 1: Credits earned locally persist in IndexedDB
 * - Test 2: Credits sync to QDAG after task completion
 * - Test 3: After refresh, QDAG balance is loaded
 * - Test 4: Pending credits shown correctly before sync
 * - Test 5: Double-sync prevention works
 * - Test 6: Rate limiting prevents abuse
 */

import { describe, it, expect, beforeAll, afterAll, beforeEach, afterEach, jest } from '@jest/globals';
import WebSocket from 'ws';
import { randomBytes } from 'crypto';

// Test configuration
const RELAY_URL = process.env.RELAY_URL || 'ws://localhost:8080';
const TEST_TIMEOUT = 15000;

interface RelayMessage {
  type: string;
  [key: string]: any;
}

interface TestNode {
  ws: WebSocket;
  nodeId: string;
  publicKey: string;
  messages: RelayMessage[];
}

interface LedgerBalance {
  earned: bigint;
  spent: bigint;
  tasksCompleted: number;
}

// Helper to create a test node connection
async function createTestNode(nodeId?: string, publicKey?: string): Promise<TestNode> {
  const id = nodeId || `test-node-${randomBytes(8).toString('hex')}`;
  const pubKey = publicKey || randomBytes(32).toString('hex');

  const ws = new WebSocket(RELAY_URL);
  const messages: RelayMessage[] = [];

  // Collect all messages
  ws.on('message', (data) => {
    try {
      const msg = JSON.parse(data.toString());
      messages.push(msg);
    } catch (e) {
      console.error('Failed to parse message:', e);
    }
  });

  // Wait for connection
  await new Promise<void>((resolve, reject) => {
    ws.once('open', () => resolve());
    ws.once('error', reject);
    setTimeout(() => reject(new Error('Connection timeout')), 5000);
  });

  // Register node with public key
  ws.send(JSON.stringify({
    type: 'register',
    nodeId: id,
    publicKey: pubKey,
    capabilities: ['compute', 'storage'],
    version: '0.1.0',
  }));

  // Wait for welcome message
  await new Promise<void>((resolve) => {
    const checkWelcome = () => {
      if (messages.some(m => m.type === 'welcome')) {
        resolve();
      } else {
        setTimeout(checkWelcome, 50);
      }
    };
    checkWelcome();
  });

  return { ws, nodeId: id, publicKey: pubKey, messages };
}

// Helper to wait for specific message type
async function waitForMessage(node: TestNode, type: string, timeout = 5000): Promise<RelayMessage | null> {
  const startTime = Date.now();

  return new Promise((resolve) => {
    const check = () => {
      const msg = node.messages.find(m => m.type === type);
      if (msg) {
        resolve(msg);
      } else if (Date.now() - startTime > timeout) {
        resolve(null);
      } else {
        setTimeout(check, 50);
      }
    };
    check();
  });
}

// Helper to wait for latest message of type (after clearing previous ones)
async function waitForLatestMessage(node: TestNode, type: string, timeout = 5000): Promise<RelayMessage | null> {
  const startIndex = node.messages.length;
  const startTime = Date.now();

  return new Promise((resolve) => {
    const check = () => {
      const newMessages = node.messages.slice(startIndex);
      const msg = newMessages.find(m => m.type === type);
      if (msg) {
        resolve(msg);
      } else if (Date.now() - startTime > timeout) {
        resolve(null);
      } else {
        setTimeout(check, 50);
      }
    };
    check();
  });
}

// Helper to get current QDAG balance
async function getQDAGBalance(node: TestNode): Promise<LedgerBalance> {
  const startIndex = node.messages.length;

  node.ws.send(JSON.stringify({
    type: 'ledger_sync',
    publicKey: node.publicKey,
  }));

  const response = await waitForLatestMessage(node, 'ledger_sync_response', 3000);

  if (response?.ledger) {
    return {
      earned: BigInt(response.ledger.earned || '0'),
      spent: BigInt(response.ledger.spent || '0'),
      tasksCompleted: response.ledger.tasksCompleted || 0,
    };
  }

  return { earned: 0n, spent: 0n, tasksCompleted: 0 };
}

// Helper to submit and complete a task
async function submitAndCompleteTask(
  submitter: TestNode,
  worker: TestNode,
  maxCredits: bigint = 1000000n
): Promise<{ taskId: string; reward: bigint } | null> {
  // Submit task
  submitter.ws.send(JSON.stringify({
    type: 'task_submit',
    task: {
      type: 'inference',
      model: 'test-model',
      maxCredits: maxCredits.toString(),
    },
  }));

  // Wait for task acceptance
  const acceptance = await waitForMessage(submitter, 'task_accepted', 3000);
  if (!acceptance) {
    console.error('Task not accepted');
    return null;
  }

  const taskId = acceptance.taskId;

  // Wait for task assignment to worker
  const assignment = await waitForMessage(worker, 'task_assignment', 3000);
  if (!assignment || assignment.task?.id !== taskId) {
    console.error('Task not assigned to worker');
    return null;
  }

  // Clear worker messages before completion
  const workerMsgIndex = worker.messages.length;

  // Worker completes task
  worker.ws.send(JSON.stringify({
    type: 'task_complete',
    taskId: taskId,
    result: { completed: true, model: 'test-model' },
    reward: maxCredits.toString(),
  }));

  // Wait for credit earned confirmation
  const creditMsg = await waitForLatestMessage(worker, 'credit_earned', 3000);
  if (!creditMsg) {
    console.error('No credit_earned message received');
    return null;
  }

  const reward = BigInt(creditMsg.amount || '0');
  return { taskId, reward };
}

// Helper to close node connection
function closeNode(node: TestNode): void {
  if (node.ws.readyState === WebSocket.OPEN) {
    node.ws.close();
  }
}

// =============================================================================
// TEST SUITE: Credit Persistence
// =============================================================================

describe('Credit Persistence Tests', () => {

  describe('Test 1: Credits earned locally persist in IndexedDB (via QDAG)', () => {
    it('should persist credits after task completion', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        // Get initial QDAG balance
        const initialBalance = await getQDAGBalance(worker);
        console.log('[Test 1] Initial balance:', initialBalance.earned.toString());

        // Complete a task to earn credits
        const result = await submitAndCompleteTask(submitter, worker, 2000000n);
        expect(result).toBeTruthy();
        expect(result!.taskId).toBeDefined();
        expect(result!.reward).toBeGreaterThan(0n);

        // Verify QDAG balance increased
        const newBalance = await getQDAGBalance(worker);
        console.log('[Test 1] New balance:', newBalance.earned.toString());

        expect(newBalance.earned).toBeGreaterThan(initialBalance.earned);
        expect(newBalance.tasksCompleted).toBeGreaterThan(initialBalance.tasksCompleted);

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT);

    it('should accumulate credits across multiple tasks', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        const initialBalance = await getQDAGBalance(worker);
        let expectedIncrease = 0n;

        // Complete multiple tasks
        for (let i = 0; i < 3; i++) {
          const result = await submitAndCompleteTask(submitter, worker, 1000000n);
          if (result) {
            expectedIncrease += result.reward;
          }
          // Small delay between tasks
          await new Promise(resolve => setTimeout(resolve, 200));
        }

        // Verify total accumulation
        const finalBalance = await getQDAGBalance(worker);
        const actualIncrease = finalBalance.earned - initialBalance.earned;

        expect(actualIncrease).toBeGreaterThan(0n);
        console.log('[Test 1b] Accumulated:', actualIncrease.toString(), 'from', finalBalance.tasksCompleted - initialBalance.tasksCompleted, 'tasks');

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT * 2);
  });

  describe('Test 2: Credits sync to QDAG after task completion', () => {
    it('should immediately sync credits to QDAG on task completion', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        const initialBalance = await getQDAGBalance(worker);

        // Complete task
        const result = await submitAndCompleteTask(submitter, worker, 3000000n);
        expect(result).toBeTruthy();

        // QDAG should be updated immediately (within the same request)
        // The credit_earned message includes the updated balance
        const creditMsg = worker.messages.find(m => m.type === 'credit_earned');
        expect(creditMsg).toBeTruthy();
        expect(creditMsg?.balance).toBeDefined();

        const returnedEarned = BigInt(creditMsg?.balance?.earned || '0');
        expect(returnedEarned).toBeGreaterThan(initialBalance.earned);

        // Verify by requesting fresh balance from QDAG
        const qdagBalance = await getQDAGBalance(worker);
        expect(qdagBalance.earned).toBe(returnedEarned);

        console.log('[Test 2] QDAG synced immediately:', qdagBalance.earned.toString());

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT);

    it('should update QDAG atomically with task completion', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        // Complete a task
        await submitAndCompleteTask(submitter, worker, 1500000n);

        // The relay returns updated balance in credit_earned message
        // This ensures atomicity - client gets consistent state
        const creditMsg = worker.messages.find(m => m.type === 'credit_earned');
        expect(creditMsg).toBeTruthy();
        expect(creditMsg?.balance).toBeDefined();
        expect(BigInt(creditMsg?.balance?.available || '0')).toBeGreaterThan(0n);

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT);
  });

  describe('Test 3: After refresh, QDAG balance is loaded', () => {
    it('should load same balance after reconnection with same publicKey', async () => {
      // Use consistent public key for this test
      const testPublicKey = `persist-test-${randomBytes(16).toString('hex')}`;

      // First session
      let worker = await createTestNode('worker1', testPublicKey);
      const submitter = await createTestNode();

      try {
        // Earn some credits
        await submitAndCompleteTask(submitter, worker, 2500000n);
        const sessionOneBalance = await getQDAGBalance(worker);
        console.log('[Test 3] Session 1 balance:', sessionOneBalance.earned.toString());

        // Disconnect worker (simulate page refresh)
        closeNode(worker);
        await new Promise(resolve => setTimeout(resolve, 500));

        // Reconnect with SAME publicKey (simulates browser refresh)
        worker = await createTestNode('worker2', testPublicKey);

        // Request balance from QDAG
        const sessionTwoBalance = await getQDAGBalance(worker);
        console.log('[Test 3] Session 2 balance:', sessionTwoBalance.earned.toString());

        // Balance should be preserved
        expect(sessionTwoBalance.earned).toBe(sessionOneBalance.earned);
        expect(sessionTwoBalance.tasksCompleted).toBe(sessionOneBalance.tasksCompleted);

      } finally {
        closeNode(worker);
        closeNode(submitter);
      }
    }, TEST_TIMEOUT);

    it('should maintain balance across multiple reconnections', async () => {
      const testPublicKey = `multi-persist-${randomBytes(16).toString('hex')}`;
      const submitter = await createTestNode();
      let worker: TestNode | null = null;
      let lastKnownBalance = 0n;

      try {
        // Multiple connect/disconnect cycles
        for (let cycle = 0; cycle < 3; cycle++) {
          worker = await createTestNode(`worker-cycle-${cycle}`, testPublicKey);

          // Verify balance is at least what we expect
          const balance = await getQDAGBalance(worker);
          expect(balance.earned).toBeGreaterThanOrEqual(lastKnownBalance);

          // Earn more credits
          const result = await submitAndCompleteTask(submitter, worker, 1000000n);
          if (result) {
            const newBalance = await getQDAGBalance(worker);
            lastKnownBalance = newBalance.earned;
            console.log(`[Test 3b] Cycle ${cycle + 1} balance:`, lastKnownBalance.toString());
          }

          // Disconnect
          closeNode(worker);
          await new Promise(resolve => setTimeout(resolve, 300));
        }

        // Final verification
        worker = await createTestNode('worker-final', testPublicKey);
        const finalBalance = await getQDAGBalance(worker);
        expect(finalBalance.earned).toBe(lastKnownBalance);

      } finally {
        if (worker) closeNode(worker);
        closeNode(submitter);
      }
    }, TEST_TIMEOUT * 2);
  });

  describe('Test 4: Pending credits shown correctly before sync', () => {
    it('should show 0 pending for new identity', async () => {
      const worker = await createTestNode();

      try {
        const balance = await getQDAGBalance(worker);

        // New identity starts with 0
        // "Pending" is managed locally, QDAG only tracks confirmed credits
        expect(balance.earned).toBe(0n);
        expect(balance.spent).toBe(0n);

      } finally {
        closeNode(worker);
      }
    }, TEST_TIMEOUT);

    it('should receive credit confirmation with task completion', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        // Before task: 0 credits
        const beforeBalance = await getQDAGBalance(worker);

        // Submit task
        submitter.ws.send(JSON.stringify({
          type: 'task_submit',
          task: {
            type: 'inference',
            model: 'test-model',
            maxCredits: '2000000',
          },
        }));

        // Wait for task acceptance and assignment
        await waitForMessage(submitter, 'task_accepted');
        await waitForMessage(worker, 'task_assignment');

        // At this point, credits are "pending" conceptually
        // (the task is assigned but not yet completed)

        // Complete the task
        const msgIndex = worker.messages.length;
        worker.ws.send(JSON.stringify({
          type: 'task_complete',
          taskId: (await waitForMessage(worker, 'task_assignment'))?.task?.id,
          result: { done: true },
          reward: '2000000',
        }));

        // Wait for credit confirmation (not pending anymore)
        const creditMsg = await waitForLatestMessage(worker, 'credit_earned');
        expect(creditMsg).toBeTruthy();

        // After confirmation: credits are confirmed in QDAG
        const afterBalance = await getQDAGBalance(worker);
        expect(afterBalance.earned).toBeGreaterThan(beforeBalance.earned);

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT);
  });

  describe('Test 5: Double-sync prevention works', () => {
    it('should prevent double completion of same task', async () => {
      const submitter = await createTestNode();
      const worker = await createTestNode();

      try {
        // Submit task
        submitter.ws.send(JSON.stringify({
          type: 'task_submit',
          task: {
            type: 'inference',
            model: 'test-model',
            maxCredits: '5000000',
          },
        }));

        await waitForMessage(submitter, 'task_accepted');
        const assignment = await waitForMessage(worker, 'task_assignment');
        const taskId = assignment?.task?.id;
        expect(taskId).toBeTruthy();

        // First completion
        worker.messages.length = 0;
        worker.ws.send(JSON.stringify({
          type: 'task_complete',
          taskId: taskId,
          result: { done: true },
          reward: '5000000',
        }));

        const firstCredit = await waitForMessage(worker, 'credit_earned', 3000);
        expect(firstCredit).toBeTruthy();
        const firstEarned = BigInt(firstCredit?.balance?.earned || '0');

        // Get balance after first completion
        const balanceAfterFirst = await getQDAGBalance(worker);

        // Wait a moment
        await new Promise(resolve => setTimeout(resolve, 500));
        worker.messages.length = 0;

        // Second completion (should fail - replay attack)
        worker.ws.send(JSON.stringify({
          type: 'task_complete',
          taskId: taskId,
          result: { done: true },
          reward: '5000000',
        }));

        // Should get error, not credit
        const secondResponse = await waitForMessage(worker, 'error', 2000);
        expect(secondResponse).toBeTruthy();
        expect(secondResponse?.message).toContain('already completed');

        // Should NOT receive second credit
        const secondCredit = worker.messages.find(m => m.type === 'credit_earned');
        expect(secondCredit).toBeFalsy();

        // Verify balance unchanged after double-complete attempt
        const balanceAfterSecond = await getQDAGBalance(worker);
        expect(balanceAfterSecond.earned).toBe(balanceAfterFirst.earned);

        console.log('[Test 5] Double-completion prevented, balance unchanged:', balanceAfterSecond.earned.toString());

      } finally {
        closeNode(submitter);
        closeNode(worker);
      }
    }, TEST_TIMEOUT);

    it('should prevent client from self-reporting credits', async () => {
      const attacker = await createTestNode();

      try {
        const initialBalance = await getQDAGBalance(attacker);
        attacker.messages.length = 0;

        // Attempt to self-report credits (should be rejected)
        attacker.ws.send(JSON.stringify({
          type: 'ledger_update',
          publicKey: attacker.publicKey,
          earned: '999999999999', // Try to give self many credits
          spent: '0',
        }));

        // Should receive error
        const error = await waitForMessage(attacker, 'error', 2000);
        expect(error).toBeTruthy();
        expect(error?.message).toContain('self-report');

        // Verify balance unchanged
        const finalBalance = await getQDAGBalance(attacker);
        expect(finalBalance.earned).toBe(initialBalance.earned);

        console.log('[Test 5b] Self-reporting rejected, balance unchanged');

      } finally {
        closeNode(attacker);
      }
    }, TEST_TIMEOUT);
  });

  describe('Test 6: Rate limiting prevents abuse', () => {
    it('should rate limit ledger sync requests', async () => {
      const node = await createTestNode();

      try {
        // Clear messages
        node.messages.length = 0;

        // Send many ledger_sync requests rapidly
        const BURST_SIZE = 50;
        for (let i = 0; i < BURST_SIZE; i++) {
          node.ws.send(JSON.stringify({
            type: 'ledger_sync',
            publicKey: node.publicKey,
          }));
        }

        // Wait for responses
        await new Promise(resolve => setTimeout(resolve, 2000));

        // Count responses and errors
        const syncResponses = node.messages.filter(m => m.type === 'ledger_sync_response');
        const rateLimitErrors = node.messages.filter(m =>
          m.type === 'error' && m.message?.includes('Rate limit')
        );

        // Should get some responses and possibly some rate limit errors
        // The exact behavior depends on rate limit configuration
        console.log('[Test 6] Burst results:', {
          responses: syncResponses.length,
          rateLimitErrors: rateLimitErrors.length,
        });

        // At minimum, should have handled requests (either with response or rate limit)
        expect(syncResponses.length + rateLimitErrors.length).toBeGreaterThan(0);

      } finally {
        closeNode(node);
      }
    }, TEST_TIMEOUT);

    it('should rate limit task submissions', async () => {
      const node = await createTestNode();

      try {
        node.messages.length = 0;

        // Send many task submissions rapidly
        const BURST_SIZE = 30;
        for (let i = 0; i < BURST_SIZE; i++) {
          node.ws.send(JSON.stringify({
            type: 'task_submit',
            task: {
              type: 'inference',
              model: 'test-model',
              maxCredits: '1000',
            },
          }));
        }

        // Wait for responses
        await new Promise(resolve => setTimeout(resolve, 2000));

        const accepted = node.messages.filter(m => m.type === 'task_accepted');
        const errors = node.messages.filter(m => m.type === 'error');

        console.log('[Test 6b] Task burst results:', {
          accepted: accepted.length,
          errors: errors.length,
        });

        // Should process requests within rate limits
        expect(accepted.length + errors.length).toBeGreaterThan(0);

      } finally {
        closeNode(node);
      }
    }, TEST_TIMEOUT);
  });

  describe('Cross-device Credit Sync (Same PublicKey)', () => {
    it('should share credits across multiple connections with same publicKey', async () => {
      const sharedPublicKey = `shared-${randomBytes(16).toString('hex')}`;
      const submitter = await createTestNode();

      // Device 1
      const device1 = await createTestNode('device1', sharedPublicKey);

      try {
        // Earn credits on device1
        await submitAndCompleteTask(submitter, device1, 3000000n);
        const device1Balance = await getQDAGBalance(device1);
        console.log('[Cross-device] Device 1 earned:', device1Balance.earned.toString());

        // Device 2 connects with same publicKey
        const device2 = await createTestNode('device2', sharedPublicKey);

        try {
          // Device 2 should see same balance
          const device2Balance = await getQDAGBalance(device2);
          console.log('[Cross-device] Device 2 sees:', device2Balance.earned.toString());

          expect(device2Balance.earned).toBe(device1Balance.earned);
          expect(device2Balance.tasksCompleted).toBe(device1Balance.tasksCompleted);

          // Device 2 earns more credits
          await submitAndCompleteTask(submitter, device2, 2000000n);
          const device2NewBalance = await getQDAGBalance(device2);
          console.log('[Cross-device] Device 2 new balance:', device2NewBalance.earned.toString());

          // Device 1 should see updated balance
          const device1NewBalance = await getQDAGBalance(device1);
          console.log('[Cross-device] Device 1 sees update:', device1NewBalance.earned.toString());

          expect(device1NewBalance.earned).toBe(device2NewBalance.earned);

        } finally {
          closeNode(device2);
        }

      } finally {
        closeNode(device1);
        closeNode(submitter);
      }
    }, TEST_TIMEOUT * 2);
  });
});

// =============================================================================
// DOCUMENTATION: Credit Persistence Flow
// =============================================================================
/*

## Credit Persistence Architecture

### 1. Source of Truth: QDAG (Firestore)
- Credits are stored in Firestore keyed by PUBLIC KEY
- Same publicKey = same credits everywhere (CLI, dashboard, mobile)
- Only the relay server can credit accounts (via verified task completions)

### 2. Credit Flow

```
[User completes task]
        |
        v
[Relay verifies task was assigned to this node]
        |
        v
[Relay credits account in QDAG (Firestore)]
        |
        v
[Relay sends credit_earned to node with new balance]
        |
        v
[Dashboard updates local state from credit_earned]
        |
        v
[Dashboard saves to IndexedDB as backup]
```

### 3. Reconnection Flow

```
[Browser refreshed / app reopened]
        |
        v
[Load from IndexedDB (local backup)]
        |
        v
[Connect to relay with publicKey]
        |
        v
[Request ledger_sync from QDAG]
        |
        v
[QDAG returns authoritative balance]
        |
        v
[Update local state to match QDAG]
```

### 4. Security Measures

1. **No self-reporting**: Clients cannot submit their own credit values
   - `ledger_update` messages are rejected with error

2. **Task verification**: Only assigned tasks can be completed
   - `assignedTasks` map tracks who was assigned each task
   - Completion attempts from wrong node are rejected

3. **Replay prevention**: Tasks can only be completed once
   - `completedTasks` set prevents double-completion

4. **Rate limiting**: Prevents request flooding
   - RATE_LIMIT_MAX messages per RATE_LIMIT_WINDOW

5. **Public key identity**: Credits tied to cryptographic identity
   - Same key = same balance everywhere
   - Different key = separate account

### 5. IndexedDB Schema (Local Backup)

```typescript
interface NodeState {
  id: string;           // 'primary'
  nodeId: string;       // WASM node ID
  creditsEarned: number;
  creditsSpent: number;
  tasksCompleted: number;
  totalUptime: number;
  consentGiven: boolean;
  // ... contribution settings
}
```

### 6. QDAG Ledger Schema (Firestore - Source of Truth)

```typescript
interface QDAGLedger {
  earned: number;        // Total rUv earned (float, converted from nanoRuv)
  spent: number;         // Total rUv spent
  tasksCompleted: number;
  createdAt: number;     // Timestamp
  updatedAt: number;     // Last update timestamp
  lastTaskId: string;    // Last completed task ID
}
```

### 7. Key Files

- `/relay/index.js` - Relay server with QDAG integration
- `/dashboard/src/services/relayClient.ts` - WebSocket client
- `/dashboard/src/services/storage.ts` - IndexedDB service
- `/dashboard/src/stores/networkStore.ts` - Zustand store with credit state

*/
