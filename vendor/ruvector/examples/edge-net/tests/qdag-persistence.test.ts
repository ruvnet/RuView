/**
 * QDAG Credit Persistence Test Suite
 *
 * Tests the Edge-Net QDAG credit persistence system to verify:
 * - Credits persist across sessions in Firestore
 * - Same public key returns same balance from different node IDs
 * - Ledger sync correctly retrieves balances from QDAG
 */

import WebSocket from 'ws';

interface EdgeNetMessage {
  type: string;
  from?: string;
  to?: string;
  payload?: any;
  [key: string]: any;
}

interface LedgerSyncResponse {
  type: 'ledger-sync';
  credits: number;
  publicKey: string;
  timestamp: number;
}

interface TestResult {
  success: boolean;
  balance: number | null;
  error?: string;
  timestamp: number;
}

const RELAY_URL = 'wss://edge-net-relay-875130704813.us-central1.run.app';
const TEST_PUBLIC_KEY = '38a3bcd1732fe04c4a0358a058fd8f81ed8325fcf6f372b91aab0f983f3a2ca5';
const CONNECTION_TIMEOUT = 10000; // 10 seconds
const RESPONSE_TIMEOUT = 15000; // 15 seconds

/**
 * Create a WebSocket connection with timeout
 */
function connectWithTimeout(url: string, timeout: number): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);
    const timer = setTimeout(() => {
      ws.close();
      reject(new Error('Connection timeout'));
    }, timeout);

    ws.on('open', () => {
      clearTimeout(timer);
      resolve(ws);
    });

    ws.on('error', (error) => {
      clearTimeout(timer);
      reject(error);
    });
  });
}

/**
 * Wait for a specific message type
 */
function waitForMessage(
  ws: WebSocket,
  messageType: string,
  timeout: number
): Promise<EdgeNetMessage> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      reject(new Error(`Timeout waiting for ${messageType}`));
    }, timeout);

    const handler = (data: WebSocket.Data) => {
      try {
        const message: EdgeNetMessage = JSON.parse(data.toString());
        if (message.type === messageType) {
          clearTimeout(timer);
          ws.removeListener('message', handler);
          resolve(message);
        }
      } catch (error) {
        // Ignore parse errors, wait for valid message
      }
    };

    ws.on('message', handler);
  });
}

/**
 * Test 1: Connect to relay and verify connection
 */
async function testConnection(): Promise<TestResult> {
  console.log('\n📡 Test 1: Testing relay connection...');
  const startTime = Date.now();

  try {
    const ws = await connectWithTimeout(RELAY_URL, CONNECTION_TIMEOUT);
    console.log('✅ Successfully connected to relay');
    ws.close();

    return {
      success: true,
      balance: null,
      timestamp: Date.now() - startTime
    };
  } catch (error) {
    console.error('❌ Connection failed:', error);
    return {
      success: false,
      balance: null,
      error: error instanceof Error ? error.message : String(error),
      timestamp: Date.now() - startTime
    };
  }
}

/**
 * Test 2: Register with public key and request ledger sync
 */
async function testLedgerSync(nodeId: string): Promise<TestResult> {
  console.log(`\n💳 Test 2: Testing ledger sync with node ID: ${nodeId.substring(0, 8)}...`);
  const startTime = Date.now();

  try {
    // Connect to relay
    const ws = await connectWithTimeout(RELAY_URL, CONNECTION_TIMEOUT);
    console.log('✅ Connected to relay');

    // Register with public key
    const registerMessage = {
      type: 'register',
      nodeId: nodeId,
      publicKey: TEST_PUBLIC_KEY,
      capabilities: ['test'],
      timestamp: Date.now()
    };

    ws.send(JSON.stringify(registerMessage));
    console.log('📤 Sent registration message');

    // Wait for welcome message (registration confirmation)
    await waitForMessage(ws, 'welcome', RESPONSE_TIMEOUT);
    console.log('✅ Registration confirmed (received welcome)');

    // Request ledger sync
    const ledgerSyncRequest = {
      type: 'ledger_sync',
      nodeId: nodeId,
      publicKey: TEST_PUBLIC_KEY
    };

    ws.send(JSON.stringify(ledgerSyncRequest));
    console.log('📤 Sent ledger sync request');

    // Wait for ledger sync response
    const response = await waitForMessage(ws, 'ledger_sync_response', RESPONSE_TIMEOUT);
    const ledgerData = response.ledger as any;
    const credits = BigInt(ledgerData.earned || '0') - BigInt(ledgerData.spent || '0');

    console.log(`✅ Received ledger sync response:`);
    console.log(`   Earned: ${ledgerData.earned}`);
    console.log(`   Spent: ${ledgerData.spent}`);
    console.log(`   Available: ${credits.toString()} credits`);

    ws.close();

    return {
      success: true,
      balance: Number(credits),
      timestamp: Date.now() - startTime
    };
  } catch (error) {
    console.error('❌ Ledger sync failed:', error);
    return {
      success: false,
      balance: null,
      error: error instanceof Error ? error.message : String(error),
      timestamp: Date.now() - startTime
    };
  }
}

/**
 * Test 3: Verify same balance from different node IDs
 */
async function testBalanceConsistency(): Promise<TestResult> {
  console.log('\n🔄 Test 3: Testing balance consistency across node IDs...');
  const startTime = Date.now();

  try {
    // Generate multiple random node IDs
    const nodeIds = [
      `test-node-${Math.random().toString(36).substring(7)}`,
      `test-node-${Math.random().toString(36).substring(7)}`,
      `test-node-${Math.random().toString(36).substring(7)}`
    ];

    const balances: number[] = [];

    // Test with each node ID
    for (const nodeId of nodeIds) {
      console.log(`\n  Testing with node ID: ${nodeId}`);
      const result = await testLedgerSync(nodeId);

      if (!result.success || result.balance === null) {
        throw new Error(`Failed to get balance for node ${nodeId}`);
      }

      balances.push(result.balance);
      console.log(`  Balance: ${result.balance} credits`);

      // Wait a bit between requests
      await new Promise(resolve => setTimeout(resolve, 1000));
    }

    // Verify all balances are the same
    const allSame = balances.every(balance => balance === balances[0]);

    if (allSame) {
      console.log(`\n✅ Balance consistency verified: All node IDs returned ${balances[0]} credits`);
      return {
        success: true,
        balance: balances[0],
        timestamp: Date.now() - startTime
      };
    } else {
      console.error(`\n❌ Balance inconsistency detected: ${balances.join(', ')}`);
      return {
        success: false,
        balance: null,
        error: `Inconsistent balances: ${balances.join(', ')}`,
        timestamp: Date.now() - startTime
      };
    }
  } catch (error) {
    console.error('❌ Balance consistency test failed:', error);
    return {
      success: false,
      balance: null,
      error: error instanceof Error ? error.message : String(error),
      timestamp: Date.now() - startTime
    };
  }
}

/**
 * Main test runner
 */
async function runTests() {
  console.log('🧪 Edge-Net QDAG Credit Persistence Test Suite');
  console.log('='.repeat(60));
  console.log(`📋 Testing public key: ${TEST_PUBLIC_KEY}`);
  console.log(`🌐 Relay URL: ${RELAY_URL}`);
  console.log('='.repeat(60));

  const results: TestResult[] = [];

  // Test 1: Connection
  const connectionResult = await testConnection();
  results.push(connectionResult);

  if (!connectionResult.success) {
    console.error('\n❌ Connection test failed. Aborting remaining tests.');
    printSummary(results);
    return;
  }

  // Test 2: Single ledger sync
  const nodeId = `test-node-${Math.random().toString(36).substring(7)}`;
  const ledgerSyncResult = await testLedgerSync(nodeId);
  results.push(ledgerSyncResult);

  if (!ledgerSyncResult.success) {
    console.error('\n❌ Ledger sync test failed. Aborting remaining tests.');
    printSummary(results);
    return;
  }

  // Test 3: Balance consistency
  const consistencyResult = await testBalanceConsistency();
  results.push(consistencyResult);

  // Print summary
  printSummary(results);
}

/**
 * Print test summary
 */
function printSummary(results: TestResult[]) {
  console.log('\n' + '='.repeat(60));
  console.log('📊 Test Summary');
  console.log('='.repeat(60));

  const totalTests = results.length;
  const passedTests = results.filter(r => r.success).length;
  const failedTests = totalTests - passedTests;

  results.forEach((result, index) => {
    const status = result.success ? '✅ PASS' : '❌ FAIL';
    const testName = ['Connection Test', 'Ledger Sync Test', 'Balance Consistency Test'][index];
    console.log(`\n${status} - ${testName}`);
    console.log(`  Duration: ${result.timestamp}ms`);
    if (result.balance !== null) {
      console.log(`  Balance: ${result.balance} credits`);
    }
    if (result.error) {
      console.log(`  Error: ${result.error}`);
    }
  });

  console.log('\n' + '='.repeat(60));
  console.log(`Total: ${totalTests} | Passed: ${passedTests} | Failed: ${failedTests}`);
  console.log('='.repeat(60));

  // Report final balance
  const balanceResult = results.find(r => r.balance !== null);
  if (balanceResult && balanceResult.balance !== null) {
    console.log(`\n🏆 Final Balance for public key ${TEST_PUBLIC_KEY.substring(0, 16)}...:`);
    console.log(`   ${balanceResult.balance} credits`);
  }

  // Exit with appropriate code
  process.exit(failedTests > 0 ? 1 : 0);
}

// Run tests
runTests().catch(error => {
  console.error('Fatal error running tests:', error);
  process.exit(1);
});
