#!/usr/bin/env node
/**
 * Manual Credit Persistence Test Script
 *
 * This script manually tests the credit persistence flow:
 * 1. Connect to relay via WebSocket
 * 2. Register a node with test public key
 * 3. Submit a task and complete it
 * 4. Verify QDAG balance updated
 * 5. Reconnect and verify balance persisted
 *
 * Usage:
 *   node manual-credit-test.cjs [--relay-url <url>]
 *
 * Environment:
 *   RELAY_URL - WebSocket URL (default: ws://localhost:8080)
 */

const WebSocket = require('ws');
const crypto = require('crypto');

// Configuration
const RELAY_URL = process.env.RELAY_URL || 'ws://localhost:8080';
const TEST_PUBLIC_KEY = `manual-test-${crypto.randomBytes(8).toString('hex')}`;

// Colors for terminal output
const colors = {
  reset: '\x1b[0m',
  green: '\x1b[32m',
  red: '\x1b[31m',
  yellow: '\x1b[33m',
  cyan: '\x1b[36m',
  dim: '\x1b[2m',
};

function log(message, color = 'reset') {
  console.log(`${colors[color]}${message}${colors.reset}`);
}

function logSection(title) {
  console.log();
  log(`${'='.repeat(60)}`, 'cyan');
  log(`  ${title}`, 'cyan');
  log(`${'='.repeat(60)}`, 'cyan');
}

function logStep(step, description) {
  log(`\n[Step ${step}] ${description}`, 'yellow');
}

function logSuccess(message) {
  log(`  [SUCCESS] ${message}`, 'green');
}

function logError(message) {
  log(`  [ERROR] ${message}`, 'red');
}

function logInfo(message) {
  log(`  ${message}`, 'dim');
}

// Create WebSocket connection
function createConnection(nodeId, publicKey) {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(RELAY_URL);
    const messages = [];

    ws.on('open', () => {
      logInfo(`Connected to ${RELAY_URL}`);

      // Register with the relay
      ws.send(JSON.stringify({
        type: 'register',
        nodeId: nodeId,
        publicKey: publicKey,
        capabilities: ['compute', 'storage'],
        version: '0.1.0',
      }));
    });

    ws.on('message', (data) => {
      try {
        const msg = JSON.parse(data.toString());
        messages.push(msg);

        if (msg.type === 'welcome') {
          logSuccess(`Registered as ${nodeId}`);
          logInfo(`Peers: ${msg.peers?.length || 0}`);
          resolve({ ws, messages, nodeId, publicKey });
        }
      } catch (e) {
        // Ignore parse errors
      }
    });

    ws.on('error', (err) => {
      logError(`WebSocket error: ${err.message}`);
      reject(err);
    });

    ws.on('close', (code, reason) => {
      logInfo(`Connection closed: ${code} ${reason}`);
    });

    // Timeout
    setTimeout(() => {
      reject(new Error('Connection timeout'));
    }, 10000);
  });
}

// Wait for specific message type
function waitForMessage(node, type, timeout = 5000) {
  return new Promise((resolve) => {
    const startIndex = node.messages.length;
    const startTime = Date.now();

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

// Get current QDAG balance
async function getBalance(node) {
  node.ws.send(JSON.stringify({
    type: 'ledger_sync',
    publicKey: node.publicKey,
  }));

  const response = await waitForMessage(node, 'ledger_sync_response', 5000);
  if (response?.ledger) {
    return {
      earned: BigInt(response.ledger.earned || '0'),
      spent: BigInt(response.ledger.spent || '0'),
      tasksCompleted: response.ledger.tasksCompleted || 0,
    };
  }
  return { earned: 0n, spent: 0n, tasksCompleted: 0 };
}

// Format rUv amount
function formatRuv(nanoRuv) {
  const ruv = Number(nanoRuv) / 1e9;
  return `${ruv.toFixed(6)} rUv`;
}

// Main test flow
async function runTest() {
  logSection('Edge-Net Credit Persistence Manual Test');

  log(`Relay URL: ${RELAY_URL}`);
  log(`Test Public Key: ${TEST_PUBLIC_KEY}`);

  let submitter = null;
  let worker = null;
  let initialBalance = null;

  try {
    // Step 1: Connect as submitter
    logStep(1, 'Connect task submitter');
    submitter = await createConnection(
      `submitter-${crypto.randomBytes(4).toString('hex')}`,
      `submitter-key-${crypto.randomBytes(8).toString('hex')}`
    );

    // Step 2: Connect as worker with test public key
    logStep(2, 'Connect worker with test identity');
    worker = await createConnection(
      `worker-${crypto.randomBytes(4).toString('hex')}`,
      TEST_PUBLIC_KEY
    );

    // Step 3: Get initial QDAG balance
    logStep(3, 'Get initial QDAG balance');
    initialBalance = await getBalance(worker);
    logInfo(`Earned: ${formatRuv(initialBalance.earned)}`);
    logInfo(`Spent: ${formatRuv(initialBalance.spent)}`);
    logInfo(`Tasks Completed: ${initialBalance.tasksCompleted}`);
    logSuccess('Initial balance retrieved');

    // Step 4: Submit a task
    logStep(4, 'Submit a task');
    submitter.ws.send(JSON.stringify({
      type: 'task_submit',
      task: {
        type: 'inference',
        model: 'test-model',
        data: 'test-data',
        maxCredits: '5000000000', // 5 rUv
      },
    }));

    const acceptance = await waitForMessage(submitter, 'task_accepted', 5000);
    if (!acceptance) {
      logError('Task not accepted');
      throw new Error('Task submission failed');
    }
    logSuccess(`Task accepted: ${acceptance.taskId}`);

    // Step 5: Wait for task assignment
    logStep(5, 'Wait for task assignment');
    const assignment = await waitForMessage(worker, 'task_assignment', 5000);
    if (!assignment) {
      logError('Task not assigned to worker');
      logInfo('This may happen if there are multiple workers - try again');
      throw new Error('Task not assigned');
    }
    logSuccess(`Task assigned: ${assignment.task.id}`);

    // Step 6: Complete the task
    logStep(6, 'Complete the task');
    worker.ws.send(JSON.stringify({
      type: 'task_complete',
      taskId: assignment.task.id,
      result: { completed: true, model: 'test-model' },
      reward: '5000000000', // 5 rUv
    }));

    const creditMsg = await waitForMessage(worker, 'credit_earned', 5000);
    if (!creditMsg) {
      logError('No credit_earned message received');
      throw new Error('Credit not earned');
    }
    const earnedAmount = BigInt(creditMsg.amount || '0');
    logSuccess(`Credit earned: ${formatRuv(earnedAmount)}`);

    // Step 7: Verify QDAG balance updated
    logStep(7, 'Verify QDAG balance updated');
    const afterBalance = await getBalance(worker);
    logInfo(`New Earned: ${formatRuv(afterBalance.earned)}`);
    logInfo(`New Tasks: ${afterBalance.tasksCompleted}`);

    if (afterBalance.earned > initialBalance.earned) {
      logSuccess('Balance increased correctly!');
    } else {
      logError('Balance did not increase');
    }

    // Step 8: Disconnect worker
    logStep(8, 'Disconnect worker (simulating refresh)');
    worker.ws.close();
    await new Promise(resolve => setTimeout(resolve, 1000));
    logSuccess('Worker disconnected');

    // Step 9: Reconnect with same identity
    logStep(9, 'Reconnect with same identity');
    worker = await createConnection(
      `worker-reconnect-${crypto.randomBytes(4).toString('hex')}`,
      TEST_PUBLIC_KEY
    );
    logSuccess('Worker reconnected');

    // Step 10: Verify balance persisted
    logStep(10, 'Verify balance persisted in QDAG');
    const persistedBalance = await getBalance(worker);
    logInfo(`Persisted Earned: ${formatRuv(persistedBalance.earned)}`);
    logInfo(`Persisted Tasks: ${persistedBalance.tasksCompleted}`);

    if (persistedBalance.earned === afterBalance.earned) {
      logSuccess('Balance persisted correctly across reconnection!');
    } else {
      logError(`Balance mismatch: expected ${formatRuv(afterBalance.earned)}, got ${formatRuv(persistedBalance.earned)}`);
    }

    // Summary
    logSection('Test Summary');
    const deltaEarned = persistedBalance.earned - initialBalance.earned;
    log(`Public Key: ${TEST_PUBLIC_KEY}`);
    log(`Credits Earned This Session: ${formatRuv(deltaEarned)}`);
    log(`Total Credits: ${formatRuv(persistedBalance.earned)}`);
    log(`Total Tasks: ${persistedBalance.tasksCompleted}`);

    if (deltaEarned > 0n && persistedBalance.earned === afterBalance.earned) {
      logSuccess('\nALL TESTS PASSED!');
      log('Credit persistence is working correctly.');
    } else {
      logError('\nSOME TESTS FAILED');
    }

  } catch (error) {
    logError(`Test failed: ${error.message}`);
    console.error(error);
  } finally {
    // Cleanup
    if (submitter?.ws?.readyState === WebSocket.OPEN) {
      submitter.ws.close();
    }
    if (worker?.ws?.readyState === WebSocket.OPEN) {
      worker.ws.close();
    }

    // Give connections time to close
    await new Promise(resolve => setTimeout(resolve, 500));
  }
}

// Additional test: Self-reporting prevention
async function testSelfReportingPrevention() {
  logSection('Security Test: Self-Reporting Prevention');

  let attacker = null;

  try {
    // Connect as attacker
    logStep(1, 'Connect as attacker');
    attacker = await createConnection(
      `attacker-${crypto.randomBytes(4).toString('hex')}`,
      `attacker-key-${crypto.randomBytes(8).toString('hex')}`
    );

    // Get initial balance
    logStep(2, 'Get initial balance');
    const initialBalance = await getBalance(attacker);
    logInfo(`Initial: ${formatRuv(initialBalance.earned)}`);

    // Attempt to self-report credits
    logStep(3, 'Attempt to self-report 1000 rUv');
    attacker.ws.send(JSON.stringify({
      type: 'ledger_update',
      publicKey: attacker.publicKey,
      earned: '1000000000000', // 1000 rUv
      spent: '0',
    }));

    const error = await waitForMessage(attacker, 'error', 3000);
    if (error) {
      logSuccess(`Attack blocked: ${error.message}`);
    } else {
      logError('No error received - attack may have succeeded!');
    }

    // Verify balance unchanged
    logStep(4, 'Verify balance unchanged');
    const finalBalance = await getBalance(attacker);
    logInfo(`Final: ${formatRuv(finalBalance.earned)}`);

    if (finalBalance.earned === initialBalance.earned) {
      logSuccess('Self-reporting attack prevented!');
    } else {
      logError('SECURITY BREACH: Balance was modified!');
    }

  } catch (error) {
    logError(`Security test failed: ${error.message}`);
  } finally {
    if (attacker?.ws?.readyState === WebSocket.OPEN) {
      attacker.ws.close();
    }
    await new Promise(resolve => setTimeout(resolve, 500));
  }
}

// Additional test: Double-completion prevention
async function testDoubleCompletionPrevention() {
  logSection('Security Test: Double-Completion Prevention');

  let submitter = null;
  let worker = null;

  try {
    // Connect nodes
    logStep(1, 'Connect nodes');
    submitter = await createConnection(
      `submitter-${crypto.randomBytes(4).toString('hex')}`,
      `submitter-key-${crypto.randomBytes(8).toString('hex')}`
    );
    worker = await createConnection(
      `worker-${crypto.randomBytes(4).toString('hex')}`,
      `worker-key-${crypto.randomBytes(8).toString('hex')}`
    );

    // Submit task
    logStep(2, 'Submit task');
    submitter.ws.send(JSON.stringify({
      type: 'task_submit',
      task: {
        type: 'inference',
        model: 'test-model',
        maxCredits: '10000000000', // 10 rUv
      },
    }));

    const acceptance = await waitForMessage(submitter, 'task_accepted');
    if (!acceptance) throw new Error('Task not accepted');
    logSuccess(`Task accepted: ${acceptance.taskId}`);

    // Wait for assignment
    const assignment = await waitForMessage(worker, 'task_assignment');
    if (!assignment) throw new Error('Task not assigned');
    const taskId = assignment.task.id;
    logSuccess(`Task assigned: ${taskId}`);

    // First completion
    logStep(3, 'Complete task (first time)');
    worker.ws.send(JSON.stringify({
      type: 'task_complete',
      taskId: taskId,
      result: { done: true },
      reward: '10000000000',
    }));

    const credit1 = await waitForMessage(worker, 'credit_earned', 3000);
    if (credit1) {
      logSuccess(`First completion: earned ${formatRuv(credit1.amount)}`);
    } else {
      logError('First completion failed');
    }

    // Second completion (should fail)
    logStep(4, 'Attempt second completion (should fail)');
    await new Promise(resolve => setTimeout(resolve, 500));
    const startIdx = worker.messages.length;

    worker.ws.send(JSON.stringify({
      type: 'task_complete',
      taskId: taskId,
      result: { done: true },
      reward: '10000000000',
    }));

    const error = await waitForMessage(worker, 'error', 3000);
    if (error && error.message.includes('already completed')) {
      logSuccess(`Double-completion blocked: ${error.message}`);
    } else {
      // Check if credit_earned was received (bad)
      const credit2 = worker.messages.slice(startIdx).find(m => m.type === 'credit_earned');
      if (credit2) {
        logError('SECURITY BREACH: Double-completion earned credits!');
      } else {
        logInfo('No credit earned (expected behavior)');
        logSuccess('Double-completion prevented');
      }
    }

  } catch (error) {
    logError(`Security test failed: ${error.message}`);
  } finally {
    if (submitter?.ws?.readyState === WebSocket.OPEN) submitter.ws.close();
    if (worker?.ws?.readyState === WebSocket.OPEN) worker.ws.close();
    await new Promise(resolve => setTimeout(resolve, 500));
  }
}

// Run all tests
async function main() {
  const args = process.argv.slice(2);

  if (args.includes('--help') || args.includes('-h')) {
    console.log(`
Edge-Net Credit Persistence Manual Test

Usage:
  node manual-credit-test.cjs [options]

Options:
  --relay-url <url>    WebSocket URL (default: ws://localhost:8080)
  --security-only      Only run security tests
  --help, -h           Show this help

Environment Variables:
  RELAY_URL            Alternative to --relay-url
`);
    process.exit(0);
  }

  // Override relay URL from args
  const urlIdx = args.indexOf('--relay-url');
  if (urlIdx !== -1 && args[urlIdx + 1]) {
    process.env.RELAY_URL = args[urlIdx + 1];
  }

  if (args.includes('--security-only')) {
    await testSelfReportingPrevention();
    await testDoubleCompletionPrevention();
  } else {
    await runTest();
    await testSelfReportingPrevention();
    await testDoubleCompletionPrevention();
  }

  logSection('All Tests Complete');
}

main().catch(console.error);
