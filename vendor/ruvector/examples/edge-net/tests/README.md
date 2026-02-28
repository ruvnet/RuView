# Edge-Net Test Suite

Comprehensive security and persistence tests for the Edge-Net WebSocket relay server.

## Test Coverage

### Credit Persistence Tests

1. **Local IndexedDB Persistence** - Credits earned locally persist across sessions
2. **QDAG Sync** - Credits sync to QDAG (Firestore) as source of truth
3. **Refresh Recovery** - After browser refresh, QDAG balance is loaded
4. **Pending Credits** - Pending credits shown correctly before sync
5. **Double-Sync Prevention** - Prevents duplicate credit awards
6. **Rate Limiting** - Prevents abuse via message throttling
7. **Cross-Device Sync** - Same publicKey = same credits everywhere

### Attack Vectors Tested

1. **Task Completion Spoofing** - Prevents nodes from completing tasks not assigned to them
2. **Replay Attacks** - Prevents completing the same task multiple times
3. **Credit Self-Reporting** - Prevents clients from claiming their own credits
4. **Public Key Spoofing** - Tests identity verification and credit attribution
5. **Rate Limiting** - Validates per-node message throttling
6. **Message Size Limits** - Tests protection against oversized payloads
7. **Connection Limits** - Validates per-IP connection restrictions
8. **Task Expiration** - Tests cleanup of unfinished tasks

## Setup

### Prerequisites

1. Node.js 20+
2. Running Edge-Net relay server (or will start one for tests)

### Installation

```bash
cd /workspaces/ruvector/examples/edge-net/tests
npm install
```

### Environment Variables

```bash
# Optional: Override relay URL (default: ws://localhost:8080)
export RELAY_URL=ws://localhost:8080
```

## Running Tests

### Run all security tests

```bash
npm test
```

### Run specific test suite

```bash
# Security tests
npm run test:security

# Credit persistence tests
npm run test:credits

# All tests (in sequence)
npm run test:all
```

### Manual credit test (interactive)

```bash
npm run test:credits-manual

# With custom relay URL
RELAY_URL=wss://edge-net-relay.example.com npm run test:credits-manual
```

### Watch mode (development)

```bash
npm run test:watch
```

### Run with coverage

```bash
npm test -- --coverage
```

## Test Structure

```
tests/
├── relay-security.test.ts     # Security test suite (attack vectors)
├── credit-persistence.test.ts # Credit persistence test suite
├── manual-credit-test.cjs     # Manual interactive credit test
├── jest.config.js             # Jest configuration
├── package.json               # Test dependencies
├── tsconfig.json              # TypeScript configuration
└── README.md                  # This file
```

## Credit Persistence Flow

### Architecture Overview

```
[User Completes Task]
        |
        v
[Relay Verifies Assignment]  -----> [REJECT if not assigned]
        |
        v
[QDAG Credits Account (Firestore)]  <-- SOURCE OF TRUTH
        |
        v
[Relay Sends credit_earned + balance]
        |
        v
[Dashboard Updates Local State]
        |
        v
[IndexedDB Saves as Backup]
```

### Reconnection Flow

```
[Browser Refresh / App Reopen]
        |
        v
[Load from IndexedDB (backup)]
        |
        v
[Connect to Relay with publicKey]
        |
        v
[Request ledger_sync from QDAG]
        |
        v
[QDAG Returns Authoritative Balance]
        |
        v
[Update Local State to Match QDAG]
```

### Key Design Principles

1. **QDAG is Source of Truth** - Credits stored in Firestore, keyed by publicKey
2. **Same Key = Same Credits** - publicKey links all devices/CLI
3. **Server Awards Credits** - Only relay can credit accounts
4. **Client Cannot Self-Report** - ledger_update is rejected
5. **Double-Completion Prevented** - completedTasks set tracks finished work
6. **Task Assignment Verified** - assignedTasks map tracks who works on what

## Test Scenarios

### 1. Task Completion Spoofing

```typescript
// Scenario: Attacker tries to complete victim's task
const victim = await createTestNode();
const attacker = await createTestNode();

// Task assigned to victim
// Attacker tries to complete it
attacker.ws.send({ type: 'task_complete', taskId: victimTask });

// Expected: Error "Task not assigned to you"
```

### 2. Replay Attacks

```typescript
// Scenario: Node tries to complete same task twice
worker.ws.send({ type: 'task_complete', taskId });  // First time: succeeds
worker.ws.send({ type: 'task_complete', taskId });  // Second time: fails

// Expected: Error "Task already completed"
```

### 3. Credit Self-Reporting

```typescript
// Scenario: Client tries to report their own credits
attacker.ws.send({
  type: 'ledger_update',
  ledger: { earned: 999999999, spent: 0 }
});

// Expected: Error "Credit self-reporting disabled"
```

### 4. Public Key Spoofing

```typescript
// Scenario: Attacker uses victim's public key
const attacker = await createTestNode('attacker', 'victim-public-key');

// Expected: Cannot steal existing credits (assigned at task time)
// Note: Current implementation allows read-only access to balances
```

## Expected Results

All tests should pass:

```
✓ Task Completion Spoofing
  ✓ should reject task completion from node not assigned
  ✓ should only allow assigned worker to complete task

✓ Replay Attacks
  ✓ should reject duplicate task completion

✓ Credit Self-Reporting
  ✓ should reject ledger_update messages
  ✓ should only credit via verified completions

✓ Public Key Spoofing
  ✓ should not allow stealing credits via key spoofing
  ✓ should maintain separate ledgers

✓ Rate Limiting
  ✓ should enforce rate limits per node

✓ Message Size Limits
  ✓ should reject oversized messages

✓ Connection Limits
  ✓ should limit connections per IP

✓ Combined Attack Scenario
  ✓ should defend against combined attacks
```

## Known Issues / Limitations

### Current Implementation Gaps

1. **No cryptographic signature verification** - Public key ownership not proven
   - Impact: Medium (cannot verify identity)
   - Status: Documented in security audit
   - Recommendation: Implement Ed25519 signature validation

2. **Unbounded completed tasks set** - Memory leak over time
   - Impact: Low (development only)
   - Status: Needs cleanup implementation
   - Recommendation: Periodic cleanup of old tasks

3. **Read-only public key spoofing** - Can check another user's balance
   - Impact: Low (privacy leak only)
   - Status: By design (multi-device support)
   - Recommendation: Rate-limit balance queries

## Security Audit

See comprehensive security audit report:
- **Report**: `/workspaces/ruvector/examples/edge-net/docs/SECURITY_AUDIT_REPORT.md`
- **Date**: 2026-01-03
- **Rating**: ✅ GOOD (with recommendations)

## Debugging Failed Tests

### Relay server not running

```bash
# Start relay server in separate terminal
cd /workspaces/ruvector/examples/edge-net/relay
npm start
```

### Connection timeout errors

```bash
# Check if relay is accessible
curl http://localhost:8080/health

# Expected: {"status":"healthy","nodes":0,"uptime":...}
```

### Firestore errors

```bash
# Tests use in-memory ledger cache
# Firestore failures won't break tests, but will log errors
# Set up Firestore credentials for full integration testing
export GOOGLE_CLOUD_PROJECT=your-project-id
export GOOGLE_APPLICATION_CREDENTIALS=/path/to/service-account.json
```

## Contributing

When adding new security tests:

1. Add test to appropriate `describe()` block
2. Use helper functions (`createTestNode`, `waitForMessage`, etc.)
3. Always clean up connections in `finally` blocks
4. Document the attack vector being tested
5. Update this README with new test scenarios

## Resources

- [WebSocket Security Best Practices](https://cheatsheetseries.owasp.org/cheatsheets/WebSocket_Security_Cheat_Sheet.html)
- [Edge-Net Architecture](/workspaces/ruvector/examples/edge-net/README.md)
- [QDAG Ledger Design](/workspaces/ruvector/examples/edge-net/docs/architecture/)
